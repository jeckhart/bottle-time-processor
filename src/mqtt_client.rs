/// MQTT client implementation with support for filtered message subscriptions
///
/// This module provides the core MQTT client functionality, including message
/// filtering and subscription management.
use crate::error::ResultExt;
use crate::{influxdb::InfluxDBWriter, models::KasaPowerMessage, watchdog::Signal};
use async_trait::async_trait;
use miette::{Report, Result};
use regex::Regex;
use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use std::{
    collections::HashMap,
    fmt::{Debug, Display, Formatter},
    future::Future,
    pin::Pin,
    sync::Arc,
};
use tokio::sync::{Mutex, mpsc::Sender};

/// Trait for message filtering
#[async_trait]
pub trait MessageFilter: Send + Sync + Debug {
    /// Check if the message matches the filter
    fn matches(&self, message: &str) -> bool;
}

/// Simple contains string filter
#[derive(Debug)]
pub struct ContainsFilter(pub String);

impl MessageFilter for ContainsFilter {
    fn matches(&self, message: &str) -> bool {
        message.contains(&self.0)
    }
}

/// Regex-based filter
#[derive(Debug)]
pub struct RegexFilter(Regex);

impl MessageFilter for RegexFilter {
    fn matches(&self, message: &str) -> bool {
        self.0.is_match(message)
    }
}

/// Function-based filter
pub struct FunctionFilter(Box<dyn Fn(&str) -> bool + Send + Sync>);

impl MessageFilter for FunctionFilter {
    fn matches(&self, message: &str) -> bool {
        (self.0)(message)
    }
}

impl std::fmt::Debug for FunctionFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("FunctionFilter")
            .field(&"<function>")
            .finish()
    }
}

/// Callback type for message handlers
pub type MessageCallback =
    Box<dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync>;

/// Structure to hold subscription information
pub struct Subscription {
    /// Optional message filter
    pub filter: Option<Box<dyn MessageFilter>>,
    /// Message callback
    pub callback: MessageCallback,
}

impl Subscription {
    /// Handle a message, returns true if the message was processed
    async fn handle_message(&self, payload: &str) -> Result<()> {
        let should_process = match &self.filter {
            Some(filter) => filter.matches(payload),
            None => true,
        };

        if should_process {
            (self.callback)(payload.to_string()).await?;
        }
        Ok(())
    }
}

impl Debug for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Subscription")
            .field("filter", &self.filter)
            .field("callback", &"<function>")
            .finish()
    }
}

/// An abstraction over the MQTT client functionality allowing dependency injection.
#[async_trait]
pub trait MqttClientInterface: Send + Sync + Debug + Display {
    /// Subscribe to a topic with a given QoS
    async fn subscribe(&self, topic: &str, qos: QoS) -> Result<()>;
}

/// The production implementation wrapping `rumqttc`’s `AsyncClient`.
#[derive(Debug)]
pub struct RealMqttClient {
    client: AsyncClient,
}

impl Display for RealMqttClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "RealMqttClient {{ client: {:?} }}", self.client)
    }
}

#[async_trait]
impl MqttClientInterface for RealMqttClient {
    async fn subscribe(&self, topic: &str, qos: QoS) -> Result<()> {
        // The real client subscribes to the topic and sets up subscription context.
        let sub = self.client.subscribe(topic, qos).await;
        sub.with_subscription_context(topic)?;
        Ok(())
    }
}

/// The MQTT client manager
pub struct MqttClientManager {
    /// The MQTT client
    pub client: Arc<dyn MqttClientInterface>,
    /// The subscriptions of this client
    pub subscriptions: Arc<Mutex<HashMap<String, Vec<Subscription>>>>,
}

impl MqttClientManager {
    /// Create a new MQTT client manager
    pub fn new(mqtt_options: MqttOptions) -> Result<(Self, EventLoop)> {
        let (client, eventloop) = AsyncClient::new(mqtt_options, 10);
        let real_client = RealMqttClient { client };
        Ok((Self::with_client(Arc::new(real_client)), eventloop))
    }

    /// Alternative constructor to allow injection of a custom client (for tests).
    pub fn with_client(client: Arc<dyn MqttClientInterface>) -> Self {
        Self {
            client,
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Subscribe to a topic with an optional filter and callback
    pub async fn subscribe(
        &self,
        topic: String,
        filter: Option<Box<dyn MessageFilter>>,
        callback: impl Fn(String) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>
        + Send
        + Sync
        + 'static,
    ) -> Result<()> {
        let mut subs = self.subscriptions.lock().await;

        // Subscribe to MQTT topic if this is the first subscription
        if !subs.contains_key(&topic) {
            self.client.subscribe(&topic, QoS::AtMostOnce).await?;
            subs.insert(topic.clone(), Vec::new());
        }

        let subscription = Subscription {
            filter,
            callback: Box::new(callback),
        };

        subs.get_mut(&topic).unwrap().push(subscription);
        Ok(())
    }

    /// Handle incoming messages
    #[allow(dead_code)]
    pub async fn handle_message(&self, topic: &str, payload: &str) -> Result<()> {
        let subs = self.subscriptions.lock().await;

        if let Some(subscriptions) = subs.get(topic) {
            for subscription in subscriptions {
                if let Err(e) = subscription.handle_message(payload).await {
                    tracing::error!("Error in message callback: {}", e);
                }
            }
        }
        Ok(())
    }

    /// Spawn a task to handle MQTT events
    pub async fn spawn_event_handler(&self, mut eventloop: EventLoop) -> Result<()> {
        let subscriptions = Arc::clone(&self.subscriptions);

        tokio::task::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(notification) => {
                        if let rumqttc::Event::Incoming(rumqttc::Packet::Publish(msg)) =
                            notification
                        {
                            let subs = subscriptions.lock().await;
                            if let Some(topic_subs) = subs.get(&msg.topic) {
                                let payload = String::from_utf8_lossy(&msg.payload);
                                for subscription in topic_subs {
                                    if let Err(e) = subscription.handle_message(&payload).await {
                                        tracing::error!("Error in message callback: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("MQTT connection error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Create a contains filter
    #[allow(dead_code)]
    pub fn contains_filter(contains: &str) -> Box<dyn MessageFilter> {
        Box::new(ContainsFilter(contains.to_string()))
    }

    /// Create a regex filter
    #[allow(dead_code)]
    pub fn regex_filter(pattern: &str) -> Result<Box<dyn MessageFilter>> {
        Regex::new(pattern)
            .map(|r| Box::new(RegexFilter(r)) as Box<dyn MessageFilter>)
            .map_err(|e| Report::from_err(e).wrap_err("Invalid regex pattern"))
    }

    /// Create a function filter
    #[allow(dead_code)]
    pub fn function_filter(
        f: impl Fn(&str) -> bool + Send + Sync + 'static,
    ) -> Box<dyn MessageFilter> {
        Box::new(FunctionFilter(Box::new(f)))
    }
}

impl Debug for MqttClientManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MqttClientManager")
            .field("client", &self.client)
            .field("subscriptions", &self.subscriptions)
            .finish()
    }
}

// impl Display for dyn MqttClientInterface {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "<MqttClientInterface>")
//     }
// }

impl Display for MqttClientManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MqttClientManager {{ client: {}, subscriptions: {:?}",
            self.client, self.subscriptions
        )
    }
}

/// Process a message from the Kasa power monitor
pub async fn process_message(
    message: &str,
    influxdb: &InfluxDBWriter,
    reset_tx: Option<Sender<Signal>>,
) -> miette::Result<()> {
    let power_message: KasaPowerMessage =
        serde_json::from_str(message).with_serde_json_context()?;
    let readings = power_message.into_readings();

    for reading in readings {
        tracing::debug!("Writing reading to InfluxDB: {:?}", reading);
        influxdb
            .write_power_reading(&reading)
            .await
            .expect("Failed to write power reading to InfluxDB");
    }

    tracing::info!("Resetting watchdog timer");

    if let Some(tx) = reset_tx {
        tx.send(Signal::Reset).await.ok();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_filter() {
        let filter = ContainsFilter("test".to_string());
        assert!(filter.matches("this is a test message"));
        assert!(!filter.matches("this is a message"));
    }

    #[test]
    fn test_regex_filter() {
        let filter = RegexFilter(Regex::new(r"test\d+").unwrap());
        assert!(filter.matches("this is test123 message"));
        assert!(filter.matches("test456"));
        assert!(!filter.matches("test message"));
        assert!(!filter.matches("testing"));
    }

    #[test]
    fn test_function_filter() {
        let filter = FunctionFilter(Box::new(|msg| msg.len() > 10));
        assert!(filter.matches("long message"));
        assert!(!filter.matches("short"));
    }

    #[test]
    fn test_filter_creation_methods() {
        // Test contains filter creation
        let contains = MqttClientManager::contains_filter("test");
        assert!(contains.matches("this is a test"));

        // Test regex filter creation
        let regex = MqttClientManager::regex_filter(r"test\d+").unwrap();
        assert!(regex.matches("test123"));

        // Test function filter creation
        let func = MqttClientManager::function_filter(|msg| msg.contains("test"));
        assert!(func.matches("this is a test"));
    }

    #[test]
    fn test_invalid_regex_filter() {
        let result = MqttClientManager::regex_filter("[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_function_filter_format() {
        let func = MqttClientManager::function_filter(|msg| msg.contains("test"));
        assert_eq!(format!("{:?}", func), "FunctionFilter(\"<function>\")");
    }

    fn get_msg_cb(expected_msg: String) -> MessageCallback {
        Box::new(move |msg: String| {
            Box::pin({
                {
                    let value = expected_msg.clone();
                    async move {
                        assert_eq!(msg, value);
                        Ok(())
                    }
                }
            })
        })
    }

    #[test]
    fn test_format_for_subscription() {
        let filter = ContainsFilter("test".to_string());
        let callback: MessageCallback = get_msg_cb("test message".to_string());

        let subscription = Subscription {
            filter: Some(Box::new(filter)),
            callback: Box::new(callback),
        };

        assert_eq!(
            format!("{:?}", subscription),
            "Subscription { filter: Some(ContainsFilter(\"test\")), callback: \"<function>\" }"
        );
    }

    #[tokio::test]
    async fn test_new_mqtt_manager() {
        let (manager, _) =
            MqttClientManager::new(MqttOptions::new("test", "localhost", 1883)).unwrap();
        assert_eq!(manager.subscriptions.lock().await.len(), 0);
    }

    #[ignore]
    /// This test is ignored because it requires a running MQTT broker
    #[tokio::test]
    async fn test_real_mqtt_client_interface_subscribe() {
        let client = RealMqttClient {
            client: AsyncClient::new(MqttOptions::new("test", "localhost", 1883), 10).0,
        };

        let topic = "test";
        let qos = QoS::AtMostOnce;

        let result = client.subscribe(topic, qos).await;
        assert!(result.is_ok());
    }
}
