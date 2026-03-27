use crate::config::ConfigError;
use crate::model::ModelError;
use crate::notify::NotifyError;
use crate::watch::WatchError;

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
