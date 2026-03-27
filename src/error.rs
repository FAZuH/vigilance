use crate::{config::ConfigError, model::ModelError, notify::NotifyError, watch::WatchError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error(transparent)]
    Watch(#[from] WatchError),
    #[error(transparent)]
    Notify(#[from] NotifyError),
}

pub type Result<T> = std::result::Result<T, Error>;
