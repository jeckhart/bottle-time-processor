use crate::{error::ResultExt, models::KasaPowerMessage, mqtt_client::MqttClientManager};
use rumqttc::MqttOptions;
use std::time::Duration;

pub async fn setup_mqtt(opts: &crate::command_line::Options) -> miette::Result<MqttClientManager> {
    let mut mqttoptions = MqttOptions::new("bottle_time_processor", opts.broker.clone(), opts.port);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    mqttoptions.set_credentials(opts.username.clone(), opts.password.clone());

    let (mqtt_client_manager, eventloop) = MqttClientManager::new(mqttoptions)?;

    mqtt_client_manager
        .subscribe(opts.topic.clone(), None, |message| {
            let power_message: KasaPowerMessage =
                serde_json::from_str(message.as_str()).with_serde_json_context()?;
            let readings = power_message.into_readings();
            for reading in readings {
                println!("Reading: {:?}", reading);
            }
            // Process each reading...
            Ok(())
        })
        .await?;

    mqtt_client_manager.spawn_event_handler(eventloop).await?;

    Ok(mqtt_client_manager)
}
