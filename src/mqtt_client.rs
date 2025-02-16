use crate::error::ResultExt;
use async_trait::async_trait;
use miette::{Report, Result};
use regex::Regex;
use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
/// MQTT client implementation with support for filtered message subscriptions
///
/// This module provides the core MQTT client functionality, including message
/// filtering and subscription management.
use std::collections::HashMap;
use std::{fmt::Debug, sync::Arc};
use tokio::sync::Mutex;

/// Trait for message filtering
#[async_trait]
pub trait MessageFilter: Send + Sync + Debug {
    /// Check if the message matches the filter
    fn matches(&self, message: &str) -> bool;
}

/// Simple contains string filter
#[derive(Debug)]
pub struct ContainsFilter(String);

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
type MessageCallback = Box<dyn Fn(String) -> Result<()> + Send + Sync>;

/// Structure to hold subscription information
struct Subscription {
    filter: Option<Box<dyn MessageFilter>>,
    callback: MessageCallback,
}

impl Subscription {
    /// Handle a message, returns true if the message was processed
    async fn handle_message(&self, payload: &str) -> Result<()> {
        let should_process = match &self.filter {
            Some(filter) => filter.matches(payload),
            None => true,
        };

        if should_process {
            (self.callback)(payload.to_string())?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Subscription")
            .field("filter", &self.filter)
            .field("callback", &"<function>")
            .finish()
    }
}

/// MQTT Client Manager
pub struct MqttClientManager {
    client: AsyncClient,
    subscriptions: Arc<Mutex<HashMap<String, Vec<Subscription>>>>,
}

impl MqttClientManager {
    /// Create a new MQTT client manager
    pub fn new(mqtt_options: MqttOptions) -> Result<(Self, EventLoop)> {
        let (client, eventloop) = AsyncClient::new(mqtt_options, 10);
        Ok((
            Self {
                client,
                subscriptions: Arc::new(Mutex::new(HashMap::new())),
            },
            eventloop,
        ))
    }

    /// Subscribe to a topic with an optional filter and callback
    pub async fn subscribe(
        &self,
        topic: String,
        filter: Option<Box<dyn MessageFilter>>,
        callback: impl Fn(String) -> Result<()> + Send + Sync + 'static,
    ) -> Result<()> {
        let mut subs = self.subscriptions.lock().await;

        // Subscribe to MQTT topic if this is the first subscription
        if !subs.contains_key(&topic) {
            self.client
                .subscribe(&topic, QoS::AtMostOnce)
                .await
                .with_subscription_context(&topic)?;
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

impl std::fmt::Debug for MqttClientManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MqttClientManager")
            .field("client", &self.client)
            .field("subscriptions", &self.subscriptions)
            .finish()
    }
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
}
