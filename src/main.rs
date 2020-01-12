use rppal::gpio::{Gpio, Mode, OutputPin};
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::str;
use std::thread::sleep;
use std::time::Duration;

mod client;
mod dht;

use client::Client;

const SENSOR_PIN: u8 = 2;
const RELAY_PIN: u8 = 4;
const DEFAULT_TARGET: f32 = 70.0;
const SAVE_FILE: &str = "target.txt";
const MQTT_HOST: &str = "tcp://192.168.1.25:1883";
const TEMPERATURE_TOPIC: &str = "bedroom/heat/current_temperature/get";
const HUMIDITY_TOPIC: &str = "bedroom/heat/current_humidity/get";
const SET_TARGET_TOPIC: &str = "bedroom/heat/target_temperature/set";
const GET_TARGET_TOPIC: &str = "bedroom/heat/target_temperature/get";
const MODE_TOPIC: &str = "bedroom/heat/mode/state";
const VARIANCE: f32 = 1.0;

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

    let client = Client::new(MQTT_HOST, SET_TARGET_TOPIC)?;

    loop {
        let result = dht::read(&mut pin);
        match result {
            Ok(reading) => {
                let temperature = celcius_to_farenheit(reading.temperature);
                status.temperature = temperature;
                status.humidity = reading.humidity;

                match mqtt_sync(&client, &mut status) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Push Error: {:?}", e);
                    }
                }

                toggle_state(&mut relay_pin, &mut status);

                println!(
                    "Temp: {:.2}, Humidity: {:.2}, Target: {}, Running: {}",
                    status.temperature, status.humidity, status.target_temperature, status.running
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

fn mqtt_sync(client: &Client, status: &mut Status) -> Result<(), Box<dyn Error>> {
    client.publish_message(TEMPERATURE_TOPIC, &status.temperature.to_string())?;
    client.publish_message(HUMIDITY_TOPIC, &status.humidity.to_string())?;

    if let Some(message) = client.latest_message() {
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

    client.publish_message(MODE_TOPIC, if status.running { "heat" } else { "off" })?;
    client.publish_message(GET_TARGET_TOPIC, &status.target_temperature.to_string())?;

    Ok(())
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
