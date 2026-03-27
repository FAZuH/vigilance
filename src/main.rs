use std::sync::Arc;

use crate::{config::Config, notify::LinuxNotify, watch::WatchService};

pub mod config;
pub mod error;
pub mod log;
pub mod model;
pub mod notify;
pub mod utils;
pub mod watch;

pub use error::Error;
pub use error::Result;

pub const APP_NAME: &'static str = env!("CARGO_PKG_NAME");

fn main() {
    info!("Starting vigilance daemon");
    if let Err(e) = run() {
        error!("{}", e);
        std::process::exit(1);
    }
    info!("vigilance daemon stopped");
}

fn run() -> Result<()> {
    let config = Arc::new(Config::load()?);
    debug!("Config: {:?}", config);
    let notifier = Box::new(LinuxNotify::new());

    let service = WatchService::new(config, notifier);
    service.start();
    Ok(())
}
