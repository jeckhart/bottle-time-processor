use miette::{Result, Report};

/// Extension trait for Result types to add application-specific error context
///
/// This trait provides methods to wrap errors with additional context,
/// making error messages more descriptive and helpful for debugging.
/// It allows adding domain-specific context to errors throughout the application.
pub trait ResultExt<T> {
    /// Wraps an error with MQTT context
    fn with_mqtt_context(self) -> Result<T>;
    
    /// Wraps an error with MQTT subscription context
    fn with_subscription_context(self, topic: &str) -> Result<T>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> ResultExt<T> for std::result::Result<T, E> {
    fn with_mqtt_context(self) -> Result<T> {
        self.map_err(|e| Report::from_err(e).wrap_err("MQTT error"))
    }

    fn with_subscription_context(self, topic: &str) -> Result<T> {
        self.map_err(|e| Report::from_err(e).wrap_err(format!("Error subscribing to topic: {}", topic)))
    }
}