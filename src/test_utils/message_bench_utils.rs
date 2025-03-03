use crate::{influxdb::InfluxDbClient, mqtt_client::MqttClientInterface};
use async_trait::async_trait;
use influxdb2::models::DataPoint;
use rumqttc::QoS;
use std::{
    fmt,
    fmt::{Display, Formatter},
};
use tokio::sync::Mutex;

/// A fake InfluxDB client for testing.
#[derive(Debug)]
pub struct FakeInfluxDbClient {
    data_points_written: Option<Mutex<Vec<DataPoint>>>,
}

/// Implement the trait for the fake InfluxDB client.
impl FakeInfluxDbClient {
    #[allow(dead_code)]
    /// Create a new instance of the fake InfluxDB client.
    pub fn new(track_points: bool) -> Self {
        Self {
            data_points_written: if track_points {
                Some(Mutex::new(Vec::new()))
            } else {
                None
            },
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
        match &self.data_points_written {
            Some(x) => {
                let mut data_points = x.lock().await;
                data_points.push(point);
            }
            None => {}
        }
        Ok(())
    }
}

#[derive(Debug)]
struct FakeMqttClient {
    // This field records subscribe calls for later verification.
    subscribe_calls: Option<Mutex<Vec<(String, QoS)>>>,
}

impl FakeMqttClient {
    #[allow(dead_code)]
    fn new(track_points: bool) -> Self {
        Self {
            subscribe_calls: if track_points {
                Some(Mutex::new(vec![]))
            } else {
                None
            },
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
        match &self.subscribe_calls {
            Some(x) => {
                x.lock().await.push((topic.to_string(), qos));
            }
            None => {}
        }
        Ok(())
    }
}
