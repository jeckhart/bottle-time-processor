use crate::common::{FakeInfluxDbClient, FakeMqttClient, get_msg_cb};
use bottle_time_processor::{
    influxdb::InfluxDBWriter,
    mqtt_client::{
        ContainsFilter, MessageCallback, MqttClientManager, Subscription, process_message,
    },
    watchdog::Signal,
};
use influxdb2::models::WriteDataPoint;
use rumqttc::{ConnectReturnCode, ConnectionError, Event, QoS};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, mpsc};

mod common; // Import the common helper module

#[tokio::test]
async fn test_process_message() {
    let fake_client = Arc::new(FakeInfluxDbClient::new());
    // Mock setup
    let mock_writer = InfluxDBWriter::with_client(fake_client.clone(), "test_bucket".to_string());

    // Sample power readings
    let message = r#"{"alias":"dev01","deviceId":"1234","power_total":7,"voltages_mv":[119355,119355,119276,119276,119171,119171,119171],"currents_ma":[23,23,23,23,23,23,22],"powers_mw":[754,757,757,739,739,770,770],"timestamps":[1740947155686,1740947158755,1740947161665,1740947164698,1740947167764,1740947170665,1740947172914],"num_readings":7}"#;
    let (reset_tx, mut reset_rx) = mpsc::channel(16);

    let result = process_message(message, &mock_writer, Some(reset_tx)).await;

    assert!(result.is_ok());

    assert_eq!(fake_client.data_points_written.lock().await.len(), 7);

    // Check the first data point
    let dp1 = &fake_client.data_points_written.lock().await[0];
    let mut v = Vec::new();
    dp1.write_data_point_to(&mut v).unwrap();
    let s = String::from_utf8(v).unwrap();

    assert_eq!(
        s,
        "power_reading,device_id=1234,device_name=dev01 current_ma=23,power_mw=754,voltage_mv=119355 1740947155686000000\n"
    );

    assert_eq!(reset_rx.recv().await.unwrap(), Signal::Reset);
}

#[tokio::test]
async fn test_handle_message() {
    let manager = MqttClientManager {
        client: Arc::new(FakeMqttClient::new()),
        subscriptions: Arc::new(Mutex::new(HashMap::new())),
    };

    let topic = "test".to_string();
    let payload = "test message".to_string();

    let filter = ContainsFilter("test".to_string());

    let callback: MessageCallback = get_msg_cb("test message".to_string());

    let subscription = Subscription {
        filter: Some(Box::new(filter)),
        callback: Box::new(callback),
    };

    {
        let mut subs = manager.subscriptions.lock().await;
        subs.insert(topic.clone(), vec![subscription]);
    }
    manager.handle_message(&topic, &payload).await.unwrap();
}

#[tokio::test]
async fn test_handle_message_none_filter() {
    let manager = MqttClientManager {
        client: Arc::new(FakeMqttClient::new()),
        subscriptions: Arc::new(Mutex::new(HashMap::new())),
    };

    let topic = "test".to_string();
    let payload = "test message".to_string();

    let callback: MessageCallback = get_msg_cb("test message".to_string());

    let subscription = Subscription {
        filter: None,
        callback: Box::new(callback),
    };

    {
        let mut subs = manager.subscriptions.lock().await;
        subs.insert(topic.clone(), vec![subscription]);
    }
    manager.handle_message(&topic, &payload).await.unwrap();
}

#[test]
fn test_format_for_mqtt_manager() {
    let manager = MqttClientManager {
        client: Arc::new(FakeMqttClient::new()),
        subscriptions: Arc::new(Mutex::new(HashMap::new())),
    };

    assert_eq!(
        format!("{:?}", manager),
        format!(
            "MqttClientManager {{ client: {:?}, subscriptions: Mutex {{ data: {{}} }} }}",
            FakeMqttClient::new()
        )
    )
}

#[tokio::test]
async fn test_mqtt_manager_subscribe() {
    let manager = MqttClientManager {
        client: Arc::new(FakeMqttClient::new()),
        subscriptions: Arc::new(Mutex::new(HashMap::new())),
    };

    let topic = "test".to_string();
    let filter = ContainsFilter("test".to_string());
    let callback: MessageCallback = get_msg_cb("test message".to_string());

    manager
        .subscribe(topic.clone(), Some(Box::new(filter)), callback)
        .await
        .unwrap();

    let subs = manager.subscriptions.lock().await;
    assert_eq!(subs.len(), 1);
    assert_eq!(subs.get(&topic).unwrap().len(), 1);
}

#[tokio::test]
async fn test_handle_event_message() {
    let subscriptions = Arc::new(Mutex::new(HashMap::new()));

    let topic = "test".to_string();
    let payload = "test message".to_string();

    let callback: MessageCallback = get_msg_cb("test message".to_string());

    let subscription = Subscription {
        filter: None,
        callback: Box::new(callback),
    };

    {
        let mut subs = subscriptions.lock().await;
        subs.insert(topic.clone(), vec![subscription]);
    }

    let msg = Ok(Event::Incoming(rumqttc::Packet::Publish(
        rumqttc::Publish {
            topic: topic.clone(),
            pkid: 0,
            payload: payload.into(),
            dup: false,
            qos: QoS::AtMostOnce,
            retain: false,
        },
    )));

    let result = MqttClientManager::handle_event_message(msg, subscriptions.clone()).await;
    assert!(result.is_ok());

    let msg = Err(ConnectionError::ConnectionRefused(
        ConnectReturnCode::BadClientId,
    ));
    let result = MqttClientManager::handle_event_message(msg, subscriptions.clone()).await;
    assert!(result.is_err());
}
