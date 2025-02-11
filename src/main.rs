//! bottle-time-processor

use bottle_time_processor::dummy_task;
use std::time::Duration;
use tokio_graceful_shutdown::{SubsystemBuilder, Toplevel};

mod command_line;

/// Main entry point.
#[tokio::main]
async fn main() -> miette::Result<()> {
    // Query command line options and initialize logging
    let _opts = command_line::parse();

    // Initialize and run subsystems
    Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("dummy_task", dummy_task));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_millis(1000))
    .await
    .map_err(Into::into)
}
