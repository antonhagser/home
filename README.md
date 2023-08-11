# Smart Home

Services for smart home automation.

## Services and devices

The following services are currently in use:

### [Electricity](./electricity/README.md) - Electricity consumption monitoring

The `electricity` service monitors the electricity consumption of the house, it does this by aggregating the data from the `p1meter` service and from a modbus connection available on the solaredge smart solar inverter.

### [P1 Meter](./p1meter/README.md) - P1 energy meter reader

The `p1meter` is powered by an ESP-12F and runs firmware which connects to the LAN and transmits the latest DSMR 5.0.2 data from the P1 han-port of the smart meter to a TCP socket running on the `electricity` ingest service.
