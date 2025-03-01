//! bottle-time-processor

use crate::watchdog::{Expired, Watchdog};
use bottle_time_processor::dummy_task;
use futures::future::pending;
use miette::Error;
use std::{pin::Pin, time::Duration};
use tokio::sync::{
    mpsc, oneshot,
    oneshot::{Receiver, error::RecvError},
};
use tokio_graceful_shutdown::{SubsystemBuilder, SubsystemHandle, Toplevel};

mod command_line;

mod mqtt;
mod watchdog;

pub async fn watchdog_task(
    subsys: SubsystemHandle,
    watchdog_expired_rx: Option<&mut Receiver<Expired>>,
) -> miette::Result<(), Error> {
    let duration = Duration::from_millis(20);

    let sleep = tokio::time::sleep(duration);
    let expired: Pin<Box<dyn futures::Future<Output = Result<Expired, RecvError>> + Send>> =
        match watchdog_expired_rx {
            Some(rx) => Box::pin(rx),
            None => Box::pin(pending::<Result<Expired, RecvError>>()),
        };

    tokio::pin!(sleep);
    tokio::pin!(expired);

    tracing::info!("watchdog_task started.");
    loop {
        tokio::select! {
            msg = &mut expired => {
                match msg {
                    Ok(_result) => {
                        tracing::error!("Watchdog expired!");
                    },
                    Err(e) => {
                        tracing::error!("Watchdog expired with error: {:?}", e);
                    },
                }

                // If we're not using systemd, then we need to request a shutdown
                #[cfg(not(feature = "systemd"))]
                subsys.request_shutdown();

                break;
            },
            _ = subsys.on_shutdown_requested() => {
                tracing::error!("Shutdown requested!");
                break;
            },
            () = sleep.as_mut() => {
                continue;
            }
        };
    }
    tracing::info!("watchdog_task stopped.");

    Ok(())
}

/// Main entry point.
#[tokio::main]
async fn main() -> miette::Result<()> {
    // Query command line options and initialize logging
    let opts = command_line::parse::<Vec<String>>(None);

    // Setup the watchdog timer
    let watchdog: Pin<Box<Watchdog>> = Box::pin(watchdog::setup_watchdog(&opts).await?);

    // let (reset_tx, expire_rx) = watchdog.run();
    let (reset_tx, reset_rx) = mpsc::channel(16);
    let (expire_tx, expire_rx) = oneshot::channel();

    // Initialize and run subsystems
    Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("dummy_task", dummy_task));
        s.start(SubsystemBuilder::new(
            "watchdog_loop",
            |_subsys| async move { watchdog.watchdog_loop(reset_rx, expire_tx).await },
        ));
        s.start(SubsystemBuilder::new(
            "watchdog_task",
            |subsys| async move { watchdog_task(subsys, Some(expire_rx).as_mut()).await },
        ));
        s.start(SubsystemBuilder::new("mqtt", |_subsys| async move {
            mqtt::setup_mqtt(&opts, Some(reset_tx)).await.map(|_| ())
        }));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_millis(1000))
    .await
    .map_err(Into::into)
}
