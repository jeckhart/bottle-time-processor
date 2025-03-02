use crate::watchdog::Signal;
use bottle_time_processor::{
    error::ResultExt, influxdb::InfluxDBWriter, models::KasaPowerMessage,
    mqtt_client::MqttClientManager,
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

async fn process_message(
    message: &str,
    influxdb: &InfluxDBWriter,
    reset_tx: Option<Sender<Signal>>,
) -> miette::Result<()> {
    let power_message: KasaPowerMessage =
        serde_json::from_str(message).with_serde_json_context()?;
    let readings = power_message.into_readings();

    for reading in readings {
        tracing::debug!("Writing reading to InfluxDB: {:?}", reading);
        influxdb
            .write_power_reading(&reading)
            .await
            .expect("Failed to write power reading to InfluxDB");
    }

    tracing::info!("Resetting watchdog timer");

    if let Some(tx) = reset_tx {
        tx.send(Signal::Reset).await.ok();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_line::Options;
    use clap::Parser;

    fn get_default_opts() -> Options {
        Options::try_parse_from(
            vec![
                "bottle-time-processor",
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
            ]
            .iter(),
        )
        .unwrap()
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
}
