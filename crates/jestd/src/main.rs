use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod config;
mod discovery;
mod server;
mod transform;
mod worker;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_target(false)
        .init();

    info!("Starting jestd daemon v{}", env!("CARGO_PKG_VERSION"));

    // Run the RPC server
    server::run().await
}
