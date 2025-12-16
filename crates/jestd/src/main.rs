use anyhow::Result;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod config;
mod discovery;
mod git;
mod metrics;
mod server;
mod transform;
mod watch;
mod worker;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging with environment filter
    // Can be controlled via RUST_LOG env var (e.g., RUST_LOG=jestd=debug,info)
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("jestd=debug,info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_file(false)
                .with_line_number(false),
        )
        .init();

    info!("Starting jestd daemon v{}", env!("CARGO_PKG_VERSION"));

    // Initialize metrics
    metrics::init();

    // Run the RPC server
    server::run().await
}
