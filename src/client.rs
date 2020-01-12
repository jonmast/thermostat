use paho_mqtt::AsyncClient;
use std::env;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

/// MQTT client
pub struct Client {
    async_client: paho_mqtt::AsyncClient,
    message_stream: Receiver<Option<paho_mqtt::Message>>,
    timeout: Duration,
}

type Error = paho_mqtt::errors::MqttError;

const TIMEOUT: u64 = 3;

impl Client {
    pub(crate) fn new(address: &str, topic: &'static str) -> Result<Self, Error> {
        let mut async_client = paho_mqtt::AsyncClient::new(address)?;

        let connect_opts = paho_mqtt::ConnectOptionsBuilder::new()
            .user_name(env::var("MQTT_USER").expect("MQTT_USER environment variable lookup failed"))
            .password(
                env::var("MQTT_PASSWORD")
                    .expect("MQTT_PASSWORD environment variable lookup failed"),
            )
            .finalize();
        let timeout = std::time::Duration::from_secs(TIMEOUT);
        async_client
            .connect_with_callbacks(
                connect_opts,
                Self::success_callback(topic),
                Self::failure_callback(topic),
            )
            .wait_for(timeout)?;
        let message_stream = async_client.start_consuming();

        async_client.set_connection_lost_callback(move |client| {
            println!("Connection lost. Attempting reconnect.");
            thread::sleep(Duration::from_millis(2500));
            client.reconnect_with_callbacks(
                Self::success_callback(topic),
                Self::failure_callback(topic),
            );
        });

        Ok(Self {
            async_client,
            message_stream,
            timeout,
        })
    }

    fn success_callback(topic: &'static str) -> impl Fn(&AsyncClient, u16) + 'static {
        move |client: &AsyncClient, _msgid: u16| {
            println!("Subscribing");
            client.subscribe(topic, 1);
        }
    }

    fn failure_callback(topic: &'static str) -> impl Fn(&AsyncClient, u16, i32) + 'static {
        move |client, _msgid, error_code| {
            eprintln!("connect failed with code {}", error_code);
            thread::sleep(Duration::from_millis(2500));
            client.reconnect_with_callbacks(
                Client::success_callback(topic),
                Client::failure_callback(topic),
            );
        }
    }

    pub(crate) fn latest_message(&self) -> Option<paho_mqtt::Message> {
        let mut latest = None;

        while let Ok(Some(message)) = self.message_stream.try_recv() {
            latest = Some(message);
        }

        latest
    }

    pub(crate) fn publish_message(&self, topic: &str, payload: &str) -> paho_mqtt::MqttResult<()> {
        let msg = paho_mqtt::MessageBuilder::new()
            .topic(topic)
            .payload(payload)
            .qos(1)
            .finalize();

        self.async_client.publish(msg).wait_for(self.timeout)
    }
}
