use crate::models::PowerReading;
use async_trait::async_trait;
use futures::stream;
use influxdb2::{Client, models::DataPoint};
use miette::{Error, Report, Result};
use std::{
    fmt::{Debug, Display, Formatter},
    sync::Arc,
};

/// Define a trait to abstract the write operation for InfluxDB.
#[async_trait]
pub trait InfluxDbClient: Send + Sync + Display {
    /// Writes a single data point to InfluxDB.
    async fn write_data_point(&self, bucket: &str, point: DataPoint) -> Result<(), Error>;
}

struct ClientExt(Client);

impl Display for ClientExt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{ url: {:?}, org: {:?}, }}",
            self.0.base.to_string(),
            self.0.org,
        )
    }
}

/// Implement the trait for the production InfluxDB client.
#[async_trait]
impl InfluxDbClient for ClientExt {
    async fn write_data_point(&self, bucket: &str, point: DataPoint) -> Result<(), Error> {
        self.0
            .write(bucket, stream::iter(vec![point]))
            .await
            .map_err(|e| Report::from_err(e).wrap_err("Failed to write to InfluxDB"))
    }
}

/// A writer for InfluxDB.
///
/// Instead of hardcoding a production client, we hold
/// an Arc to a trait object so that you can swap in a fake/stub.
#[derive(Clone)]
pub struct InfluxDBWriter {
    client: Arc<dyn InfluxDbClient>,
    bucket: String,
}

impl Debug for InfluxDBWriter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "InfluxDBWriter {{ client: {}, bucket: \"{}\" }}",
            self.client, self.bucket
        )
    }
}

impl Display for InfluxDBWriter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "InfluxDBWriter {{ client: {}, bucket: \"{}\" }}",
            self.client, self.bucket
        )
    }
}

impl InfluxDBWriter {
    /// Creates a new InfluxDB writer using a real (production) client.
    pub fn new(url: String, org: String, token: String, bucket: String) -> Self {
        let client = ClientExt(Client::new(url, org, token));
        Self {
            client: Arc::new(client),
            bucket,
        }
    }

    /// Creates a new writer with a provided client.
    ///
    /// This is useful for testing where you provide a fake or stub implementation.
    pub fn with_client(client: Arc<dyn InfluxDbClient>, bucket: String) -> Self {
        Self { client, bucket }
    }

    /// Writes a power reading to InfluxDB
    pub async fn write_power_reading(&self, reading: &PowerReading) -> Result<(), Error> {
        tracing::trace!("Converting power reading to InfluxDB point: {:?}", reading);
        let point = influxdb2::models::DataPoint::builder("power_reading")
            .tag("device_id", reading.device_id.as_str())
            .tag("device_name", reading.device_name.as_str())
            .field("voltage_mv", reading.voltage_mv as f64)
            .field("current_ma", reading.current_ma as f64)
            .field("power_mw", reading.power_mw as f64)
            .timestamp(reading.timestamp.timestamp_nanos_opt().unwrap())
            .build()
            .map_err(|e| Report::from_err(e).wrap_err("Failed to build InfluxDB point"))?;

        tracing::trace!("Writing power reading point to InfluxDB: {:?}", point);
        self.client.write_data_point(&self.bucket, point).await?;

        Ok(())
    }
}
