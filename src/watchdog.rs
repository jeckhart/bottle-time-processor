use bottle_time_processor::error::ResultExt;
use std::time::Duration;
use tokio::{
    sync::{mpsc, oneshot},
    time::{Instant, sleep},
};

/// Signal for interacting with the watchdog.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum Signal {
    #[default]
    Reset,
    Stop,
}

/// Signal on watchdog expiration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Expired;

/// Watchdog holding the fixed duration.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Watchdog {
    /// The timeout interval.
    duration: Duration,
}

pub async fn setup_watchdog(opts: &crate::command_line::Options) -> miette::Result<Watchdog> {
    let duration = match opts.watchdog_time.as_str() {
        #[cfg(feature = "systemd")]
        "systemd" => {
            let timeout = systemd::daemon::watchdog_enabled(false)
                .map(Duration::from_micros)
                .with_systemd_context();
            tracing::info!("Watchdog enabled by systemd with timeout: {:?}", timeout);
            timeout
        }
        wd_time => wd_time
            .parse()
            .map(Duration::from_secs)
            .with_parse_context(wd_time.to_string()),
    }?;
    let watchdog = Watchdog::new(duration);
    Ok(watchdog)
}

impl Watchdog {
    /// Create a new watchdog with the specified duration.
    pub fn new(duration: Duration) -> Self {
        Self { duration }
    }

    pub async fn watchdog_loop(
        self,
        mut reset_rx: mpsc::Receiver<Signal>,
        expire_tx: oneshot::Sender<Expired>,
    ) -> Result<(), miette::Error> {
        let watchdog = sleep(self.duration);
        tokio::pin!(watchdog);
        let mut active = true;
        loop {
            tokio::select! {
                msg = reset_rx.recv() => {
                    match msg {
                        Some(Signal::Reset) => {
                            active = true;

                            #[cfg(feature = "systemd")]
                            // If we are using systemd, we need to notify systemd that we are still alive
                            systemd::daemon::notify(false, [(systemd::daemon::STATE_WATCHDOG, "1")].iter()).with_systemd_context()?;
                            tracing::trace!("Watchdog signal handler reset");
                            watchdog.as_mut().reset(Instant::now() + self.duration);
                        }
                        Some(Signal::Stop) => {
                            active = false;
                        }
                        None => break,
                    }
                }
                () = watchdog.as_mut(), if active => {
                    let _ = expire_tx.send(Expired);
                    break;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watchdog;
    use futures::future::pending;
    use std::pin::Pin;
    use tokio::{
        sync::oneshot::{
            Receiver, Sender,
            error::{RecvError, TryRecvError},
        },
        time::interval,
    };

    #[tokio::test]
    async fn test_watchdog_premise() {
        // Really testing some behaviors with oneshots more than testing the watchdog itself.
        let mut interval = interval(Duration::from_millis(100));
        let (expire_tx, mut expire_rx) = oneshot::channel();

        tokio::spawn(async move {
            sleep(Duration::from_millis(200)).await;
            expire_tx.send("shut down").unwrap();
        });

        for _ in 0..10 {
            tokio::select! {
                _ = interval.tick() => println!("Another 100ms"),
                msg = &mut expire_rx => {
                    println!("Got message: {}", msg.unwrap());
                    break;
                }
            }
        }
    }

    #[tokio::test]
    async fn test_watchdog_premise_with_option() {
        // Really testing some behaviors with combining the receiver and an option more than testing the watchdog itself.
        let mut interval = interval(Duration::from_millis(100));
        let (expire_tx, expire_rx): (Sender<Expired>, Receiver<Expired>) = oneshot::channel();
        let mut expire_rx = Some(expire_rx);
        let mut expired: Pin<Box<dyn futures::Future<Output = Result<Expired, RecvError>> + Send>> =
            match expire_rx.as_mut() {
                Some(rx) => Box::pin(rx),
                None => Box::pin(pending::<Result<Expired, RecvError>>()),
            };

        tokio::spawn(async move {
            sleep(Duration::from_millis(200)).await;
            expire_tx.send(Expired).unwrap();
        });

        for _ in 0..10 {
            tokio::select! {
                _ = interval.tick() => println!("Another 100ms"),
                _msg = &mut expired => {
                    println!("Got message: ");
                    break;
                }
            }
        }
    }

    #[tokio::test]
    async fn test_watchdog() {
        let mut interval = interval(Duration::from_millis(10));
        let opts = crate::command_line::Options {
            verbose: 0,
            broker: "localhost".to_string(),
            port: 1883,
            username: "username".to_string(),
            password: "password".to_string(),
            topic: "username/feeds/topic1".to_string(),
            influxdb_url: "http://localhost:8086".to_string(),
            influxdb_token: "token".to_string(),
            influxdb_org: "org".to_string(),
            influxdb_bucket: "bucket".to_string(),
            watchdog_time: "1".to_string(),
        };
        let watchdog: Pin<Box<Watchdog>> = Box::pin(watchdog::setup_watchdog(&opts).await.unwrap());

        let (reset_tx, reset_rx) = mpsc::channel(16);
        let (expire_tx, mut expire_rx): (Sender<Expired>, Receiver<Expired>) = oneshot::channel();
        let _expired: Pin<Box<dyn futures::Future<Output = Result<Expired, RecvError>> + Send>> =
            Box::pin(pending::<Result<Expired, RecvError>>());

        tokio::spawn(watchdog.watchdog_loop(reset_rx, expire_tx));

        for _ in 0..20 {
            interval.tick().await;
        }
        assert_eq!(expire_rx.try_recv(), Err(TryRecvError::Empty));
        reset_tx.send(Signal::Reset).await.unwrap();
        for _ in 0..100 {
            interval.tick().await;
        }
        // Ensure we don't expire after 990ms
        assert_eq!(expire_rx.try_recv(), Err(TryRecvError::Empty));
        // Ensure we do expire after 1000ms
        interval.tick().await;
        assert_eq!(expire_rx.try_recv(), Ok(Expired));
    }
}
