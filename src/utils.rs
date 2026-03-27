use std::env;
use std::path::{Path, PathBuf};

use crate::debug;

pub fn conf_dir() -> PathBuf {
    // #[cfg(any(target_os = "linux", target_os = "macos"))]
    let home = env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| env::var("HOME").unwrap());
    debug!("Config directory: {}/vigilance", home);

    // #[cfg(target_os = "windows")]
    // let home = env::var("APPDATA").unwrap();

    Path::new(&home).join("vigilance")
}
