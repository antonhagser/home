use std::{net::ToSocketAddrs, sync::Arc};

use futures::stream;
use influxdb2_client::{models::DataPoint, Client as InfluxClient};
use modbus::{Client, Config, Transport};
use once_cell::sync::Lazy;
use regex::Regex;
use tokio::{io::AsyncReadExt, net::TcpStream, sync::Mutex};
use tracing::{debug, error, info, trace, warn};

static INFLUX_HOST: Lazy<String> =
    Lazy::new(|| std::env::var("INFLUX_HOST").unwrap_or_else(|_| "localhost:8086".to_string()));

static INFLUX_TOKEN: Lazy<String> = Lazy::new(|| std::env::var("INFLUX_TOKEN").unwrap());

static INVERTER_HOST: Lazy<String> =
    Lazy::new(|| std::env::var("INVERTER_HOST").unwrap_or_else(|_| "localhost:1502".to_string()));

#[derive(Debug, thiserror::Error)]
pub enum EnergyRecordError {
    #[error("failed to parse energy record")]
    ParseError(),
    #[error("some other error")]
    OtherError(),
}

#[derive(Debug)]
pub struct EnergyRecord {
    pub obis_code: String,
    pub value: String,
    pub unit: String,
}

impl TryFrom<String> for EnergyRecord {
    type Error = EnergyRecordError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let regex = Regex::new(r"(?m)1-0:(.+)\((.+)\*(.+)\)").unwrap();
        let mut captures = regex.captures_iter(&value);

        match captures.next() {
            Some(capture) => {
                let (_, [obis_code, value, unit]) = capture.extract();
                Ok(Self {
                    obis_code: obis_code.to_string(),
                    value: value.to_string(),
                    unit: unit.to_string(),
                })
            }
            None => Err(EnergyRecordError::ParseError()),
        }
    }
}

fn get_modbus_pipe() -> Transport {
    let addr = INVERTER_HOST
        .to_socket_addrs()
        .expect("invalid modbus client address")
        .next()
        .expect("invalid modbus client address");

    info!(address = ?addr, "connecting to modbus client");

    modbus::tcp::Transport::new_with_cfg(
        addr.ip().to_string().as_str(),
        Config {
            tcp_port: addr.port(),
            ..Default::default()
        },
    )
    .unwrap()
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!("starting server");

    // Connect to the modbus client
    let client = get_modbus_pipe();
    let modbus_client = Arc::new(Mutex::new(client));

    // Connect to the influxdb client
    let influx_client =
        influxdb2_client::Client::new(INFLUX_HOST.to_string(), INFLUX_TOKEN.as_str());
    let influx_client = Arc::new(influx_client);

    // Bind to the TCP port
    let addr = "0.0.0.0:36082";
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to listen to tcp port");

    // Accept connections
    loop {
        let (socket, _) = listener.accept().await.expect("failed to accept socket");
        let influx_client = influx_client.clone();
        let modbus_client = modbus_client.clone();

        // Spawn a new task to handle the connection
        tokio::spawn(async move {
            info!(address = ?socket.peer_addr(), "accepted new connection");
            handle_connection(socket, influx_client, modbus_client).await;
        });
    }
}

#[tracing::instrument(skip_all, fields(address = ?socket.peer_addr()))]
async fn handle_connection(
    mut socket: TcpStream,
    influx_client: Arc<InfluxClient>,
    modbus_client: Arc<Mutex<Transport>>,
) {
    loop {
        // Packages are larger than the TCP buffer, so we need to read in chunks and combine them
        let mut buffer = [0; 2048 * 8];
        let mut full_packet = String::new();

        while let Ok(n) = socket.read(&mut buffer).await {
            trace!(n, "read bytes");
            if n == 0 {
                break;
            }

            let tokens = match String::from_utf8(buffer[..n].to_vec()) {
                Ok(s) => s,
                Err(e) => {
                    error!(error = ?e, "failed to parse buffer");
                    continue;
                }
            };

            if tokens.starts_with('/') {
                full_packet.clear();
                full_packet.push_str(&tokens);
                continue;
            }

            full_packet.push_str(&tokens);

            // Get production value from solaredge, used to calculate consumption
            // (yes I know, data is not completely accurate due to the transmit delay from the esp12f and modbus polling delay)
            let production = get_ac_production(&modbus_client).await;

            // Split the packet with regex
            let full = full_packet.clone().replace("\r\n", "");
            let regex = Regex::new(r"(?m)1-0:(.*?)\((.*?)\*(.*?)\)").unwrap();

            let mut data_point_builder = DataPoint::builder("energy");

            // export and import for the current packet
            let mut export = 0;
            let mut import = 0;

            // imported lifetime, exported lifetime, production lifetime
            let mut imported_lifetime = 0;
            let mut exported_lifetime = 0;

            // Get the production lifetime
            let mut cl = modbus_client.lock().await;
            let production_lifetime = cl.read_holding_registers(93, 2).unwrap();
            let production_lifetime_scale = cl.read_holding_registers(95, 1).unwrap();

            // Convert Vec<u16> to a single value
            let production_lifetime = if production_lifetime.len() == 2 {
                ((production_lifetime[0] as u32) << 16) | (production_lifetime[1] as u32)
            } else {
                panic!("Unexpected vector size");
            };

            let production_lifetime_scale = *production_lifetime_scale.first().unwrap() as i16;
            let production_lifetime = ((production_lifetime as f64)
                * (10_f64.powf(production_lifetime_scale as f64)).floor())
                as i64;

            // Parse the packet
            for capture in regex.captures_iter(&full) {
                let (_, [obis_code, value, unit]) = capture.extract();
                debug!(?obis_code, ?value, ?unit);

                let value = value.parse::<f64>().unwrap();

                // Convert to W and extract values
                match obis_code {
                    "1.7.0" => import = (value * 1000.0).floor() as i64,
                    "2.7.0" => export = (value * 1000.0).floor() as i64,
                    "1.8.0" => imported_lifetime = (value * 1000.0).floor() as i64,
                    "2.8.0" => exported_lifetime = (value * 1000.0).floor() as i64,
                    _ => {}
                }

                data_point_builder = data_point_builder.field(obis_code, value);
            }

            // Calculate consumption
            let usage = if import > 0 && export > 0 {
                production + import - export
            } else if import > 0 {
                import + production
            } else {
                production - export
            };

            data_point_builder = data_point_builder.field("production", production);
            data_point_builder = data_point_builder.field("usage", usage);
            debug!(?production, ?usage);

            // Calculate lifetime consumption
            let lifetime_usage = production_lifetime + imported_lifetime - exported_lifetime;
            data_point_builder = data_point_builder.field("lifetime_usage", lifetime_usage);
            debug!(?lifetime_usage);

            // Build the data point
            let data_point = match data_point_builder.build() {
                Ok(dp) => dp,
                Err(e) => {
                    error!(error = ?e, "failed to build data point");
                    continue;
                }
            };

            // Write the data point to influxdb
            let client = influx_client.clone();
            info!("attempting to write packet...");
            match client
                .write("home", "electricity", stream::iter(vec![data_point]))
                .await
            {
                Ok(_) => info!("successfully wrote to influxdb"),
                Err(e) => error!(error = ?e, "failed to write to influxdb"),
            };
        }

        warn!("connection closed");
    }
}

async fn get_ac_production(client: &Arc<Mutex<Transport>>) -> i64 {
    let mut cl = client.lock().await;

    // Read the power value, if it fails reconnect to the modbus client
    let (power_value, power_scale_factor) = match cl.read_holding_registers(83, 2) {
        Ok(v) => (*v.first().unwrap() as f64, *v.last().unwrap()),
        Err(e) => {
            let _ = std::mem::replace(&mut *cl, get_modbus_pipe());
            error!(error = ?e, "failed to read power value");
            panic!("failed to read power value");
        }
    };

    let power_scale_factor = power_scale_factor as i16;

    // Calculate the actual AC value
    (power_value * (10_f64.powf(power_scale_factor as f64))).floor() as i64
}
