#include <Arduino.h>
#include <ESP8266WiFi.h>
#include <ESP8266WebServer.h>
#include <ESP8266mDNS.h>

#define LED 2

// DSMR 5.0
#define BUF_SIZE 1024 * 8

const char *DELIMITERS = "()*:";
const char *DATA_ID = "1-0";
char buffer[BUF_SIZE];

// WiFi credentials
const char *ssid = "";
const char *password = "";
bool isConnecting = false;

WiFiClient client;
bool isConnectingToTCP = false;

void flashLightIndicator(uint16 count)
{
    for (uint16 i = 0; i < count; i++)
    {
        digitalWrite(LED, LOW);
        delay(300);
        digitalWrite(LED, HIGH);
        delay(300);
    }

    delay(1000);
}

void connectToWiFi()
{
    isConnecting = true; // Set the state to connecting

    WiFi.persistent(false);
    WiFi.begin(ssid, password); // Connect to the network
    flashLightIndicator(1);

    while (WiFi.status() != WL_CONNECTED)
    { // Wait for the Wi-Fi to connect
        delay(1000);
    }

    flashLightIndicator(6);
    isConnecting = false;

    digitalWrite(LED, LOW);
}

void connectToTcpServer()
{
    if (client.connect("192.168.1.50", 6969))
    {
        flashLightIndicator(2);
        flashLightIndicator(2);
        flashLightIndicator(2);
    }
    else
    {
        flashLightIndicator(5);
    }
}

void setup()
{
    pinMode(LED, OUTPUT);

    Serial.setRxBufferSize(BUF_SIZE);
    Serial.begin(115200); // Begin serial for reading from p1 meter

    delay(10);

    connectToWiFi();

    // Connect to tcp server running on 192.168.1.141:6969
    connectToTcpServer();
}

void loop()
{
    // Verify that we are still connected
    if (WiFi.status() != WL_CONNECTED && !isConnecting)
    { // Check if the ESP is still connected to the WiFi and not currently trying to connect
        digitalWrite(LED, HIGH);
        connectToWiFi(); // Reconnect to WiFi network
        memset(buffer, 0, sizeof(buffer));
        return;
    }

    if (!client.connected() && !isConnectingToTCP)
    {
        digitalWrite(LED, HIGH);
        connectToTcpServer();
        memset(buffer, 0, sizeof(buffer));
        return;
    }

    // Read data from p1 meter
    if (Serial.available())
    {
        int len = Serial.readBytes(buffer, BUF_SIZE);

        if (len > 0)
        {
            client.write(buffer);
            client.flush();
        }

        memset(buffer, 0, sizeof(buffer));
    }
}
