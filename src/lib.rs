#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_debug_implementations, missing_docs, rust_2018_idioms)]
#![deny(unreachable_pub)]
#![deny(private_bounds)]
#![deny(rustdoc::private_intra_doc_links)]
#![deny(private_interfaces)]

//! bottle-time-processor

/// Test utilities.
#[cfg(any(test, feature = "test_utils"))]
#[cfg_attr(docsrs, doc(cfg(feature = "test_utils")))]
pub mod test_utils;

/// Error handling utilities
pub mod error;

/// Models for MQTT messages
pub mod models;

/// MQTT client implementation
pub mod mqtt_client;

/// InfluxDB client implementation
pub mod influxdb;

/// Watchdog Signal definition for use in tests and main
pub mod watchdog {

    /// Signal for interacting with the watchdog.
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
    #[allow(dead_code)]
    pub enum Signal {
        /// Reset the watchdog.
        #[default]
        Reset,
        /// Stop the watchdog timer.
        Stop,
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
}
