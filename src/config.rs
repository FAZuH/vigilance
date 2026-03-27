use std::collections::HashSet;
use std::fs;
use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;

use crate::debug;
use crate::info;
use crate::utils;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub battery: BatteryConfig,
    pub memory: MemoryConfig,
    pub disk: DiskConfig,
    // pub wifi: WifiConfig,
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let conf_dir = utils::conf_dir();
        debug!("Config directory: {:?}", conf_dir);
        if !conf_dir.exists() {
            fs::create_dir_all(&conf_dir)?;
            info!("Created config directory at {conf_dir:?}");
        }
        let conf_path = conf_dir.join("config.yaml");
        if !conf_path.exists() {
            let config = Config::default();
            let file = fs::File::create(&conf_path)?;
            serde_yml::to_writer(&file, &config)?;
            info!("Default config written to {:?}", conf_path);
            Ok(config)
        } else {
            let file = fs::File::open(&conf_path)?;
            let config: Config = serde_yml::from_reader(&file)?;
            info!("Configuration loaded successfully");
            Ok(config)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BatteryConfig {
    pub enabled: bool,
    pub low_threshold: u8,
    pub critical_threshold: u8,
    pub on_warning: Vec<String>,
    pub on_critical: Vec<String>,
    pub on_charging: Vec<String>,
    pub on_discharging: Vec<String>,
    pub on_full: Vec<String>,
}

impl Default for BatteryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            low_threshold: 20,
            critical_threshold: 5,
            on_warning: Vec::new(),
            on_critical: Vec::new(),
            on_charging: Vec::new(),
            on_discharging: Vec::new(),
            on_full: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    pub enabled: bool,
    pub poll_interval_secs: u64,
    pub warning_threshold: u8,
    pub critical_threshold: u8,
    pub on_warning: Vec<String>,
    pub on_critical: Vec<String>,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval_secs: 10,
            warning_threshold: 90,
            critical_threshold: 95,
            on_warning: Vec::new(),
            on_critical: Vec::new(),
        }
    }
}

impl MemoryConfig {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval_secs)
    }
}

fn default_mounts() -> HashSet<String> {
    HashSet::from_iter(["/".to_string()])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DiskConfig {
    pub enabled: bool,
    #[serde(default = "default_mounts")]
    pub watch_mounts: HashSet<String>,
    pub poll_interval_secs: u64,
    pub critical_threshold: u8,
    pub on_critical: Vec<String>,
}

impl Default for DiskConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            watch_mounts: default_mounts(),
            poll_interval_secs: 10,
            critical_threshold: 95,
            on_critical: Vec::new(),
        }
    }
}

impl DiskConfig {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval_secs)
    }
}

// fn default_true() -> bool {
//     true
// }
//
// #[derive(Debug, Clone, Serialize, Deserialize)]
// #[serde(default)]
// pub struct WifiConfig {
//     #[serde(default = "default_true")]
//     pub enabled: bool,
//     pub watch_interfaces: HashSet<String>,
//     pub on_connect: Vec<String>,
//     pub on_disconnect: Vec<String>,
// }
//
// impl Default for WifiConfig {
//     fn default() -> Self {
//         Self {
//             enabled: true,
//             watch_interfaces: HashSet::new(),
//             on_connect: Vec::new(),
//             on_disconnect: Vec::new(),
//         }
//     }
// }

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_yml::Error),
}
