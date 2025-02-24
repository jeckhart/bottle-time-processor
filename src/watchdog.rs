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
        "systemd" => systemd::daemon::watchdog_enabled(false)
            .map(Duration::from_micros)
            .with_systemd_context(),
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
