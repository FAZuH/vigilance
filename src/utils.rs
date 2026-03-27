use std::env;
use std::path::Path;
use std::path::PathBuf;

use crate::debug;

pub fn conf_dir() -> PathBuf {
    // #[cfg(any(target_os = "linux", target_os = "macos"))]
    let home = env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| env::var("HOME").unwrap());
    debug!("Config directory: {}/vigilance", home);

    // #[cfg(target_os = "windows")]
    // let home = env::var("APPDATA").unwrap();

    Path::new(&home).join("vigilance")
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
