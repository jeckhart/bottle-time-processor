use chrono::{DateTime, Local, TimeZone};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kasa_power_into_readings() {
        let message = KasaPowerMessage {
            alias: "Test Device".to_string(),
            device_id: "test-device".to_string(),
            power_total: 0,
            voltages_mv: vec![12000, 12001, 12002],
            currents_ma: vec![100, 101, 102],
            powers_mw: vec![1200, 1201, 1202],
            timestamps: vec![1614556800, 1614556860, 1614556920],
            num_readings: 3,
        };

        let readings = message.into_readings();
        assert_eq!(readings.len(), 3);

        assert_eq!(readings[0].device_name, "Test Device");
        assert_eq!(readings[0].device_id, "test-device");
        assert_eq!(readings[0].voltage_mv, 12000);
        assert_eq!(readings[0].current_ma, 100);
        assert_eq!(readings[0].power_mw, 1200);
        assert_eq!(readings[0].timestamp.timestamp(), 1614556800);

        assert_eq!(readings[1].device_name, "Test Device");
        assert_eq!(readings[1].device_id, "test-device");
        assert_eq!(readings[1].voltage_mv, 12001);
        assert_eq!(readings[1].current_ma, 101);
        assert_eq!(readings[1].power_mw, 1201);
        assert_eq!(readings[1].timestamp.timestamp(), 1614556860);

        assert_eq!(readings[2].device_name, "Test Device");
        assert_eq!(readings[2].device_id, "test-device");
        assert_eq!(readings[2].voltage_mv, 12002);
        assert_eq!(readings[2].current_ma, 102);
        assert_eq!(readings[2].power_mw, 1202);
        assert_eq!(readings[2].timestamp.timestamp(), 1614556920);
    }
}
