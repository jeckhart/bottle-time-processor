use miette::{Report, Result};

/// Extension trait for Result types to add application-specific error context
///
/// This trait provides methods to wrap errors with additional context,
/// making error messages more descriptive and helpful for debugging.
/// It allows adding domain-specific context to errors throughout the application.
pub trait ResultExt<T> {
    /// Wraps an error with MQTT context
    #[allow(dead_code)]
    fn with_mqtt_context(self) -> Result<T>;

    /// Wraps an error with MQTT subscription context
    fn with_subscription_context(self, topic: &str) -> Result<T>;

    /// Wraps an error with a serde_json context
    fn with_serde_json_context(self) -> Result<T>;

    /// Wraps an error with a systemd context
    fn with_systemd_context(self) -> Result<T>;

    /// Wraps an error with a parse context
    fn with_parse_context(self, orig: String) -> Result<T>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> ResultExt<T> for std::result::Result<T, E> {
    fn with_mqtt_context(self) -> Result<T> {
        self.map_err(|e| Report::from_err(e).wrap_err("MQTT error"))
    }

    fn with_subscription_context(self, topic: &str) -> Result<T> {
        self.map_err(|e| {
            Report::from_err(e).wrap_err(format!("Error subscribing to topic: {}", topic))
        })
    }

    fn with_serde_json_context(self) -> Result<T> {
        self.map_err(|e| Report::from_err(e).wrap_err("Serde JSON error"))
    }

    fn with_systemd_context(self) -> Result<T> {
        self.map_err(|e| Report::from_err(e).wrap_err("Systemd error"))
    }

    fn with_parse_context(self, orig: String) -> Result<T> {
        self.map_err(|e| Report::from_err(e).wrap_err(format!("Failed parsing value: {}", orig)))
    }
}
