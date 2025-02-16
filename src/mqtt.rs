use rumqttc::MqttOptions;
use crate::mqtt_client::MqttClientManager;
use std::time::Duration;

pub async fn setup_mqtt(opts: &crate::command_line::Options) -> miette::Result<MqttClientManager> {
    let mut mqttoptions = MqttOptions::new("bottle_time_processor", opts.broker.clone(), opts.port);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    mqttoptions.set_credentials(opts.username.clone(), opts.password.clone());
    
    let (mqtt_client_manager, eventloop) = MqttClientManager::new(mqttoptions)?;

    mqtt_client_manager.subscribe(
        opts.topic.clone(),
        None,
        |message| {
            println!("Received message: {}", message);
            Ok(())
        },
    ).await?;

    mqtt_client_manager.spawn_event_handler(eventloop).await?;
    
    Ok(mqtt_client_manager)
} 