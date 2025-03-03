use bottle_time_processor::{
    influxdb::InfluxDBWriter,
    mqtt_client::{MqttClientManager, process_message},
    watchdog::Signal,
};
use rumqttc::MqttOptions;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

const PROC_NAME: &str = "bottle_time_processor";

pub async fn setup_mqtt(
    opts: &crate::command_line::Options,
    reset_tx: Option<Sender<Signal>>,
) -> miette::Result<MqttClientManager> {
    let mqttoptions = configure_mqtt_options(opts);
    let (mqtt_client_manager, eventloop) = MqttClientManager::new(mqttoptions)?;

    let influxdb = create_influxdb_writer(opts);

    subscribe_to_topic(&mqtt_client_manager, opts, influxdb, reset_tx).await?;

    mqtt_client_manager.spawn_event_handler(eventloop).await?;

    Ok(mqtt_client_manager)
}

fn configure_mqtt_options(opts: &crate::command_line::Options) -> MqttOptions {
    let mut mqttoptions = MqttOptions::new(PROC_NAME, opts.broker.clone(), opts.port);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    mqttoptions.set_credentials(opts.username.clone(), opts.password.clone());
    mqttoptions
}

fn create_influxdb_writer(opts: &crate::command_line::Options) -> InfluxDBWriter {
    InfluxDBWriter::new(
        opts.influxdb_url.clone(),
        opts.influxdb_org.clone(),
        opts.influxdb_token.clone(),
        opts.influxdb_bucket.clone(),
    )
}

async fn subscribe_to_topic(
    mqtt_client_manager: &MqttClientManager,
    opts: &crate::command_line::Options,
    influxdb: InfluxDBWriter,
    reset_tx: Option<Sender<Signal>>,
) -> miette::Result<()> {
    mqtt_client_manager
        .subscribe(opts.topic.clone(), None, move |message| {
            let influxdb = influxdb.clone();
            let reset_tx_clone = reset_tx.clone();
            Box::pin(
                async move { process_message(message.as_str(), &influxdb, reset_tx_clone).await },
            )
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_line::Options;
    use async_trait::async_trait;
    use bottle_time_processor::mqtt_client::MqttClientInterface;
    use clap::Parser;
    use rumqttc::QoS;
    use std::{
        fmt::{Display, Formatter},
        sync::Arc,
    };
    use tokio::sync::Mutex;

    fn get_default_opts() -> Options {
        Options::try_parse_from(
            vec![
                "bottle-time-processor",
                "--influxdb-url",
                "http://localhost:8086",
                "--influxdb-token",
                "token",
                "--influxdb-org",
                "org",
                "--influxdb-bucket",
                "bucket",
                "--broker",
                "test_broker",
                "--username",
                "user",
                "--password",
                "pass",
                "--topic",
                "username/feeds/topic1",
            ]
            .iter(),
        )
        .unwrap()
    }

    #[derive(Debug)]
    pub struct FakeMqttClient {
        // This field records subscribe calls for later verification.
        pub subscribe_calls: Mutex<Vec<(String, QoS)>>,
    }

    impl FakeMqttClient {
        pub fn new() -> Self {
            Self {
                subscribe_calls: Mutex::new(vec![]),
            }
        }
    }

    impl Display for FakeMqttClient {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "FakeMqttClient")
        }
    }

    #[async_trait]
    impl MqttClientInterface for FakeMqttClient {
        async fn subscribe(&self, topic: &str, qos: QoS) -> miette::Result<()> {
            self.subscribe_calls
                .lock()
                .await
                .push((topic.to_string(), qos));
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_setup_mqtt() {
        let opts = get_default_opts();

        let mqtt_client_manager = setup_mqtt(&opts, None).await.unwrap();

        assert_eq!(
            mqtt_client_manager.to_string(),
            "MqttClientManager { client: RealMqttClient { client: AsyncClient { request_tx: Sender } }, subscriptions: Mutex { data: {\"username/feeds/topic1\": [Subscription { filter: None, callback: \"<function>\" }]} }"
        );
    }

    #[tokio::test]
    async fn test_configure_mqtt_options() {
        let opts = get_default_opts();

        let mqtt_options = configure_mqtt_options(&opts);

        assert_eq!(
            mqtt_options.broker_address(),
            ("test_broker".to_string(), 1883)
        );
    }

    #[tokio::test]
    async fn test_create_influxdb_writer() {
        let opts = get_default_opts();

        let writer = create_influxdb_writer(&opts);

        assert_eq!(
            writer.to_string(),
            "InfluxDBWriter { client: { url: \"http://localhost:8086/\", org: \"org\", }, bucket: \"bucket\" }"
        );
    }

    #[tokio::test]
    async fn test_subscribe_to_topic() {
        let opts = get_default_opts();
        let fake_client = Arc::new(FakeMqttClient::new());
        let mqtt_client_manager = MqttClientManager::with_client(fake_client);
        let influxdb = create_influxdb_writer(&opts);

        subscribe_to_topic(&mqtt_client_manager, &opts, influxdb, None)
            .await
            .unwrap();

        assert_eq!(mqtt_client_manager.subscriptions.lock().await.len(), 1);
    }
}
