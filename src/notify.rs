use std::process::Command;

use std::{path::PathBuf, time::Duration};

pub use crate::watch::WatchEvents;

pub trait Notifiable {
    fn notify(&self, notif: Notification) -> Result<(), NotifyError>;
}

#[derive(Clone, Debug, Default)]
pub struct Notification {
    pub summary: String,
    pub body: Option<String>,
    pub urgency: Option<Urgency>,
    pub expire_time: Option<Duration>,
    pub app_name: Option<String>,
    pub icon: Option<PathBuf>,
    // app_icon: Option<PathBuf>,
    // category: Option<Vec<String>>,
    // transient: Option<bool>,
    // wait: Option<Duration>,
}

#[derive(Clone, Debug)]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}

#[derive(Debug, thiserror::Error)]
pub enum NotifyError {
    #[error("Notification command not found: {0}")]
    NotFound(String),
    #[error("Failed to send notification: {0}")]
    Failed(String),
    #[error("Invalid notification data: {0}")]
    InvalidNotification(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

pub struct LinuxNotify { }

impl LinuxNotify {
    pub fn new() -> Self {
        Self { }
    }
    fn get_args(notif: Notification) -> Vec<String> {
        let mut ret = Vec::new();

        if let Some(urgency) = notif.urgency {
            ret.push("-u".to_string());
            let u = match urgency {
                Urgency::Low => "low",
                Urgency::Normal => "normal",
                Urgency::Critical => "critical",
            };
            ret.push(u.to_string())
        }
        if let Some(expire_time) = notif.expire_time {
            ret.push("-t".to_string());
            ret.push(expire_time.as_millis().to_string())
        }
        if let Some(app_name) = notif.app_name {
            ret.push("-a".to_string());
            ret.push(app_name)
        }
        if let Some(icon) = notif.icon {
            ret.push("-i".to_string());
            ret.push(icon.to_string_lossy().into())
        }
        ret.push(notif.summary);
        if let Some(body) = notif.body {
            ret.push(body)
        }

        ret
    }
}

impl Notifiable for LinuxNotify {
    fn notify(&self, notif: Notification) -> Result<(), NotifyError> {
        let args = Self::get_args(notif);
        // TODO: Handle res

        match Command::new("notify-send").args(args).status() {
            Ok(status) => {
                if status.success() {
                    Ok(())
                } else {
                    Err(NotifyError::Failed(format!("notify-send returned: {status}")))
                }
            },
            Err(e) => {
                Err(NotifyError::Failed(format!("Error executing notify-send: {e}")))
            },
        }
    }
}
