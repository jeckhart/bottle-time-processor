use async_trait::async_trait;
use bottle_time_processor::influxdb::InfluxDbClient;
use influxdb2::models::DataPoint;
use std::fmt;
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
