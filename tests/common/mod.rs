use async_trait::async_trait;
use bottle_time_processor::{
    influxdb::InfluxDbClient,
    mqtt_client::{MessageCallback, MqttClientInterface},
};
use influxdb2::models::DataPoint;
use rumqttc::QoS;
use std::{
    fmt,
    fmt::{Display, Formatter},
};
use tokio::sync::Mutex;

/// A fake InfluxDB client for testing.
pub struct FakeInfluxDbClient {
    pub data_points_written: Mutex<Vec<DataPoint>>,
}

/// Implement the trait for the fake InfluxDB client.
impl FakeInfluxDbClient {
    #[allow(dead_code)]
    /// Create a new instance of the fake InfluxDB client.
    pub fn new() -> Self {
        Self {
            data_points_written: Mutex::new(Vec::new()),
        }
    }
}

impl fmt::Display for FakeInfluxDbClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FakeInfluxDbClient")
    }
}

// If you’re implementing a trait for testing, do so here with #[async_trait]:
#[async_trait]
impl InfluxDbClient for FakeInfluxDbClient {
    async fn write_data_point(&self, _bucket: &str, point: DataPoint) -> Result<(), miette::Error> {
        let mut data_points = self.data_points_written.lock().await;
        data_points.push(point);
        Ok(())
    }
}

#[derive(Debug)]
pub struct FakeMqttClient {
    // This field records subscribe calls for later verification.
    pub subscribe_calls: Mutex<Vec<(String, QoS)>>,
}

impl FakeMqttClient {
    #[allow(dead_code)]
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

#[allow(dead_code)]
/// Get a message callback for testing.
pub fn get_msg_cb(expected_msg: String) -> MessageCallback {
    Box::new(move |msg: String| {
        Box::pin({
            {
                let value = expected_msg.clone();
                async move {
                    assert_eq!(msg, value);
                    Ok(())
                }
            }
        })
    })
}
