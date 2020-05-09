use futures::stream::StreamExt;
use rumq_client::{eventloop, MqttOptions, Notification, QoS, Request, Subscribe};
use std::env;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time;

pub(crate) async fn connect(
    address: &str,
    topic: &'static str,
) -> (Sender<Request>, Receiver<Notification>) {
    let address: SocketAddr = address.parse().unwrap();
    let mut mqtt_options = MqttOptions::new("thermostat", address.ip().to_string(), address.port());
    mqtt_options
        .set_clean_session(true)
        .set_keep_alive(5)
        .set_credentials(
            env::var("MQTT_USER").expect("MQTT_USER environment variable lookup failed"),
            env::var("MQTT_PASSWORD").expect("MQTT_PASSWORD environment variable lookup failed"),
        );
    let (requests_tx, requests_rx) = channel(5);
    let (notifications_tx, notifications_rx) = channel(10);

    tokio::spawn(reconnect_loop(
        mqtt_options,
        requests_tx.clone(),
        requests_rx,
        topic,
        notifications_tx,
    ));

    (requests_tx, notifications_rx)
}

async fn reconnect_loop(
    mqtt_options: MqttOptions,
    requests_tx: Sender<Request>,
    requests_rx: Receiver<Request>,
    topic: &'static str,
    mut notifications_tx: Sender<Notification>,
) {
    let mut event_loop = eventloop(mqtt_options, requests_rx);

    loop {
        match event_loop.connect().await {
            Ok(mut stream) => {
                println!("Connected to broker");
                let mut requests_tx = requests_tx.clone();
                tokio::spawn(async move {
                    let subscription = Subscribe::new(topic, QoS::AtLeastOnce);
                    requests_tx.send(subscription.into()).await.unwrap();
                });

                while let Some(item) = stream.next().await {
                    notifications_tx.send(item).await.unwrap();
                }
            }
            Err(e) => eprintln!("Got error trying to connect: {}", e),
        };

        time::delay_for(Duration::from_secs(5)).await;
        println!("Attempting to reconnect MQTT");
    }
}
