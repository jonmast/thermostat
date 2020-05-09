#![warn(missing_debug_implementations)]

use rppal::gpio::{Gpio, IoPin, Mode, OutputPin};
use rumq_client::{Notification, Publish, QoS, Request};
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::str;
use std::time::Duration;
use tokio::stream::StreamExt;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::delay_for;

mod buttons;
mod client;
mod dht;
mod display;

use display::Display;

const SENSOR_PIN: u8 = 16;
const RELAY_PIN: u8 = 4;
const UP_BUTTON_PIN: u8 = 7;
const DOWN_BUTTON_PIN: u8 = 8;
const DEFAULT_TARGET: f32 = 70.0;
const SAVE_FILE: &str = "target.txt";
const VARIANCE: f32 = 1.0;
const I2CDEVICE: &str = "/dev/i2c-1";
const LCDBUS: u16 = 0x27;

const MQTT_HOST: &str = "192.168.1.25:1883";
const TEMPERATURE_TOPIC: &str = "bedroom/heat/current_temperature/get";
const HUMIDITY_TOPIC: &str = "bedroom/heat/current_humidity/get";
const SET_TARGET_TOPIC: &str = "bedroom/heat/target_temperature/set";
const GET_TARGET_TOPIC: &str = "bedroom/heat/target_temperature/get";
const MODE_TOPIC: &str = "bedroom/heat/mode/state";

#[derive(Debug, Default, Clone)]
pub struct Status {
    temperature: f32,
    humidity: f32,
    target_temperature: f32,
    running: bool,
}

#[derive(Debug)]
enum Event {
    UpdateTarget(f32),
    Reading { temperature: f32, humidity: f32 },
}

#[tokio::main(basic_scheduler)]
async fn main() -> Result<(), Box<dyn Error>> {
    let gpio = Gpio::new()?;
    let mut pin = gpio.get(SENSOR_PIN)?.into_io(Mode::Input);
    let relay_pin = gpio.get(RELAY_PIN)?.into_output();

    let display = display::Display::new(I2CDEVICE, LCDBUS)?;

    let _button_handler =
        buttons::ButtonHandler::new(&gpio, UP_BUTTON_PIN, DOWN_BUTTON_PIN, display.clone())?;

    let status = Status {
        target_temperature: initial_target(),
        running: relay_pin.is_set_high(),
        ..Default::default()
    };

    let (requests_tx, notifications_rx) = client::connect(MQTT_HOST, SET_TARGET_TOPIC).await;

    let (events_tx, events_rx) = channel(50);

    tokio::task::spawn(process_mqtt_stream(notifications_rx, events_tx.clone()));

    tokio::task::spawn(process_events(
        events_rx,
        requests_tx,
        status,
        display,
        relay_pin,
    ));

    poll_sensor(events_tx, &mut pin).await;

    Ok(())
}

async fn process_mqtt_stream(
    mut notifications_rx: Receiver<Notification>,
    mut events_tx: Sender<Event>,
) {
    while let Some(notification) = notifications_rx.next().await {
        match notification {
            Notification::Publish(message) => match message.topic_name.as_ref() {
                SET_TARGET_TOPIC => {
                    if let Ok(Ok(new_target)) = str::from_utf8(&message.payload).map(|t| t.parse())
                    {
                        events_tx
                            .send(Event::UpdateTarget(new_target))
                            .await
                            .unwrap();
                    }
                }
                _ => eprintln!("Unrecognized topic event: {:?}", message),
            },
            // Every publish we do gets an ack but that's not something we care about,
            // ignore these completely
            Notification::Puback(_) => {}
            _ => {
                eprintln!("Unhandled event {:?}", notification);
            }
        }
    }
}

async fn process_events(
    mut events_rx: Receiver<Event>,
    requests_tx: Sender<Request>,
    mut status: Status,
    display: Display,
    mut relay_pin: OutputPin,
) {
    while let Some(event) = events_rx.next().await {
        match event {
            Event::UpdateTarget(new_target) => {
                println!("got new target: {}", new_target);
                status.target_temperature = new_target;

                if let Err(e) = write_target_to_file(new_target) {
                    eprintln!("Failed to persist target, got ({})", e);
                }

                if let Err(e) = display.update_status(&status) {
                    eprintln!("LCD Error: {:?}", e);
                };

                println!("New target: {}", new_target);

                mqtt_publish(
                    requests_tx.clone(),
                    GET_TARGET_TOPIC,
                    &status.target_temperature.to_string(),
                )
                .await;

                toggle_state(&mut relay_pin, &mut status);

                mqtt_publish(
                    requests_tx.clone(),
                    MODE_TOPIC,
                    if status.running { "heat" } else { "off" },
                )
                .await;
            }
            Event::Reading {
                temperature,
                humidity,
            } => {
                status.temperature = temperature;
                status.humidity = humidity;

                push_state(requests_tx.clone(), &status).await;

                toggle_state(&mut relay_pin, &mut status);

                println!(
                    "Temp: {:.2}, Humidity: {:.2}, Target: {}, Running: {}",
                    status.temperature, status.humidity, status.target_temperature, status.running
                );

                mqtt_publish(
                    requests_tx.clone(),
                    MODE_TOPIC,
                    if status.running { "heat" } else { "off" },
                )
                .await;

                if let Err(e) = display.update_status(&status) {
                    eprintln!("LCD Error: {:?}", e);
                };
            }
        }
    }
}

async fn poll_sensor(mut events_tx: Sender<Event>, pin: &mut IoPin) {
    loop {
        let result = dht::read(pin);
        match result {
            Ok(reading) => {
                let temperature = celcius_to_farenheit(reading.temperature);
                let humidity = reading.humidity;
                events_tx
                    .send(Event::Reading {
                        temperature,
                        humidity,
                    })
                    .await
                    .unwrap();
            }
            Err(e) => eprintln!("Error: {:?}", e),
        }
        delay_for(Duration::from_secs(2)).await;
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

async fn mqtt_publish(mut requests_tx: Sender<Request>, topic: &str, payload: &str) {
    let message = Publish::new(topic, QoS::AtLeastOnce, payload);
    requests_tx.send(message.into()).await.unwrap();
}

async fn push_state(requests_tx: Sender<Request>, status: &Status) {
    let tx2 = requests_tx.clone();
    let temperature = status.temperature.to_string();
    let humidity = status.humidity.to_string();
    tokio::join!(
        mqtt_publish(tx2, TEMPERATURE_TOPIC, &temperature),
        mqtt_publish(requests_tx, HUMIDITY_TOPIC, &humidity),
    );
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
