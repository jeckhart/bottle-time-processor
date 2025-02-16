use chrono::{DateTime, Local, TimeZone};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
/// Represents a power measurement message from a Kasa smart plug device
pub struct KasaPowerMessage {
    /// Human-readable name of the device
    pub alias: String,
    /// Unique identifier of the device
    #[serde(rename = "deviceId")]
    pub device_id: String,
    /// Total power consumption in milliwatts
    pub power_total: i32,
    /// Vector of voltage measurements in millivolts
    pub voltages_mv: Vec<i32>,
    /// Vector of current measurements in milliamps
    pub currents_ma: Vec<i32>,
    /// Vector of power measurements in milliwatts
    pub powers_mw: Vec<i32>,
    /// Vector of Unix timestamps for each reading
    pub timestamps: Vec<i64>,
    /// Number of readings contained in the message
    pub num_readings: usize,
}

/// Represents a single power reading from a device at a specific point in time
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PowerReading {
    /// Human-readable name of the device
    pub device_name: String,
    /// Unique identifier of the device
    pub device_id: String,
    /// Voltage measurement in millivolts
    pub voltage_mv: i32,
    /// Current measurement in milliamps
    pub current_ma: i32,
    /// Power measurement in milliwatts
    pub power_mw: i32,
    /// Timestamp of when the reading was taken
    pub timestamp: DateTime<Local>,
}

impl KasaPowerMessage {
    /// Converts the message into individual power readings
    pub fn into_readings(self) -> Vec<PowerReading> {
        let mut readings = Vec::with_capacity(self.num_readings);

        for i in 0..self.num_readings {
            let timestamp = if self.timestamps[i] < 4000000000 {
                Local.timestamp_opt(self.timestamps[i], 0)
            } else {
                Local.timestamp_millis_opt(self.timestamps[i])
            }
            .single()
            .expect("Invalid timestamp");

            readings.push(PowerReading {
                device_name: self.alias.clone(),
                device_id: self.device_id.clone(),
                voltage_mv: self.voltages_mv[i],
                current_ma: self.currents_ma[i],
                power_mw: self.powers_mw[i],
                timestamp,
            });
        }

        readings
    }
}
