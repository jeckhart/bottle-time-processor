#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_debug_implementations, missing_docs, rust_2018_idioms)]
#![deny(unreachable_pub)]
#![deny(private_bounds)]
#![deny(rustdoc::private_intra_doc_links)]
#![deny(private_interfaces)]

//! bottle-time-processor

use tokio_graceful_shutdown::SubsystemHandle;

/// Test utilities.
#[cfg(any(test, feature = "test_utils"))]
#[cfg_attr(docsrs, doc(cfg(feature = "test_utils")))]
pub mod test_utils;

/// Add two integers together.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Multiplies two integers together.
pub fn mult(a: i32, b: i32) -> i32 {
    a * b
}

/// A dummy task that will run until a shutdown is requested.
pub async fn dummy_task(subsys: SubsystemHandle) -> miette::Result<()> {
    tracing::info!("dummy_task started.");
    subsys.on_shutdown_requested().await;
    tracing::info!("dummy_task stopped.");

    Ok(())
}

/// Error handling utilities
pub mod error;

/// Models for MQTT messages
pub mod models;

/// MQTT client implementation
pub mod mqtt_client;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mult() {
        assert_eq!(mult(3, 2), 6);
    }
}
