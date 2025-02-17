use crate::models::PowerReading;
use futures::stream;
use influxdb2::Client;
use miette::{Error, Report, Result};

/// A writer for InfluxDB
#[derive(Debug, Clone)]
pub struct InfluxDBWriter {
    client: Client,
    bucket: String,
}

impl InfluxDBWriter {
    /// Creates a new InfluxDB writer with the given connection parameters
    pub fn new(url: String, org: String, token: String, bucket: String) -> Self {
        let client = Client::new(url, org, token);
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
        self.client
            .write(&self.bucket, stream::iter(vec![point]))
            .await
            .map_err(|e| Report::from_err(e).wrap_err("Failed to write to InfluxDB"))?;

        Ok(())
    }
}
