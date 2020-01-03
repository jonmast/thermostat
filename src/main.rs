use paho_mqtt as mqtt;
use rppal::gpio::{Gpio, Mode, OutputPin};
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::str;
use std::sync::mpsc::Receiver;
use std::thread::sleep;
use std::time::Duration;

mod dht;

const SENSOR_PIN: u8 = 2;
const RELAY_PIN: u8 = 4;
const DEFAULT_TARGET: f32 = 70.0;
const SAVE_FILE: &str = "target.txt";
const MQTT_HOST: &str = "tcp://192.168.1.25:1883";
const TEMPERATURE_TOPIC: &str = "bedroom/heat/current_temperature/get";
const SET_TARGET_TOPIC: &str = "bedroom/heat/target_temperature/set";
const GET_TARGET_TOPIC: &str = "bedroom/heat/target_temperature/get";
const MODE_TOPIC: &str = "bedroom/heat/mode/state";
const VARIANCE: f32 = 1.5;

#[derive(Debug, Default)]
pub struct Status {
    temperature: f32,
    humidity: f32,
    target_temperature: f32,
    running: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let gpio = Gpio::new()?;
    let mut pin = gpio.get(SENSOR_PIN)?.into_io(Mode::Input);
    let mut relay_pin = gpio.get(RELAY_PIN)?.into_output();

    let mut status = Status {
        target_temperature: initial_target(),
        running: relay_pin.is_set_high(),
        ..Default::default()
    };

    let mut client = mqtt::Client::new(MQTT_HOST)?;
    let message_stream = client.start_consuming();

    let connect_opts = mqtt::ConnectOptionsBuilder::new()
        .user_name(env::var("MQTT_USER").expect("MQTT_USER environment variable lookup failed"))
        .password(
            env::var("MQTT_PASSWORD").expect("MQTT_PASSWORD environment variable lookup failed"),
        )
        .finalize();
    let (_, _, session_present) = client.connect(connect_opts)?;
    if !session_present {
        println!("Subscribing");
        client.subscribe(SET_TARGET_TOPIC, 1)?;
    }

    loop {
        let result = dht::read(&mut pin);
        match result {
            Ok(reading) => {
                let temperature = celcius_to_farenheit(reading.temperature);
                status.temperature = temperature;

                match mqtt_sync(&client, &message_stream, &mut status) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Push Error: {:?}", e);

                        try_reconnect(&client);
                    }
                }

                toggle_state(&mut relay_pin, &mut status);

                println!(
                    "Temp: {}, Humidity: {}, Target: {}",
                    status.temperature, reading.humidity, status.target_temperature
                )
            }
            Err(e) => eprintln!("Error: {:?}", e),
        }
        sleep(Duration::from_secs(2));
    }
}

fn initial_target() -> f32 {
    read_target_from_file().unwrap_or(DEFAULT_TARGET)
}

fn read_target_from_file() -> Result<f32, Box<dyn Error>> {
    let mut file = File::open(SAVE_FILE)?;
    let mut str_target = String::new();

    file.read_to_string(&mut str_target)?;

    let target = str_target.parse()?;

    Ok(target)
}

fn write_target_to_file(target: f32) -> Result<(), std::io::Error> {
    let mut file = File::create(SAVE_FILE)?;
    file.write_all(target.to_string().as_bytes())
}

fn celcius_to_farenheit(celcius: f32) -> f32 {
    (celcius * 1.8) + 32f32
}

fn mqtt_sync(
    client: &mqtt::Client,
    message_stream: &Receiver<Option<mqtt::Message>>,
    status: &mut Status,
) -> Result<(), Box<dyn Error>> {
    publish_message(client, TEMPERATURE_TOPIC, &status.temperature.to_string())?;

    while let Some(message) = try_receive(client, message_stream) {
        match message.topic() {
            SET_TARGET_TOPIC => {
                if let Ok(Ok(new_target)) = str::from_utf8(message.payload()).map(|t| t.parse()) {
                    status.target_temperature = new_target;
                    if let Err(e) = write_target_to_file(new_target) {
                        eprintln!("Failed to persist target, got ({})", e);
                    }
                }
            }
            _ => eprintln!("Unrecognized message: {:?}", message),
        }
    }

    publish_message(
        client,
        MODE_TOPIC,
        if status.running { "heat" } else { "off" },
    )?;
    publish_message(
        client,
        GET_TARGET_TOPIC,
        &status.target_temperature.to_string(),
    )?;

    Ok(())
}

fn try_receive(
    client: &mqtt::Client,
    message_stream: &Receiver<Option<mqtt::Message>>,
) -> Option<mqtt::Message> {
    if let Ok(message) = message_stream.try_recv() {
        match message {
            Some(message) => Some(message),
            None => {
                try_reconnect(client);
                None
            }
        }
    } else {
        None
    }
}

fn publish_message(client: &mqtt::Client, topic: &str, payload: &str) -> mqtt::MqttResult<()> {
    let msg = mqtt::MessageBuilder::new()
        .topic(topic)
        .payload(payload)
        .qos(1)
        .finalize();

    client.publish(msg)
}

fn try_reconnect(client: &mqtt::Client) {
    if let Err(_) = client.reconnect() {
        eprintln!("Failed to reconnect");
    }
}

fn toggle_state(pin: &mut OutputPin, status: &mut Status) {
    if status.running && status.temperature > (status.target_temperature + VARIANCE) {
        pin.set_low();
        status.running = false;
    } else if !status.running && status.temperature < (status.target_temperature - VARIANCE) {
        pin.set_high();
        status.running = true
    }
}
