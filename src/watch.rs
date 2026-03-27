use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::config::Config;
use crate::debug;
use crate::error;
use crate::info;
use crate::model::Battery;
use crate::model::BatteryData;
use crate::model::Disk;
use crate::model::Memory;
use crate::model::MemoryData;
use crate::notify::Notifiable;
use crate::notify::Notification;
use crate::notify::Urgency;

pub struct WatchService {
    config: Arc<Config>,
    source: WatchSource,
    notifier: Box<dyn Notifiable>,
    running: Arc<AtomicBool>,
}

impl WatchService {
    pub fn new(config: Arc<Config>, notifier: Box<dyn Notifiable>) -> Self {
        let running = Arc::new(AtomicBool::new(false));
        Self {
            source: WatchSource::new(config.clone(), running.clone()),
            config,
            notifier,
            running,
        }
    }

    pub fn start(&self) {
        info!("WatchService::start() - starting event loop");
        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);
        let rx = self.source.listen();
        let conf = self.config.clone();
        debug!("Entering main event loop");
        for event in rx {
            if !running.load(Ordering::SeqCst) {
                info!("WatchService stopped");
                return;
            }
            match event {
                Ok(event) => {
                    debug!("Received event: {:?}", event);
                    let notif = match event {
                        WatchEvents::Battery(event) => {
                            debug!("Battery event: {}%", event.percentage);
                            let conf = &conf.battery;

                            if event.percentage > conf.low_threshold {
                                continue;
                            }

                            Notification {
                                summary: format!(
                                    "Battery {}",
                                    if event.percentage <= conf.critical_threshold {
                                        "Critical"
                                    } else {
                                        "Low"
                                    }
                                ),
                                body: Some(format!("{}% remaining", event.percentage)),
                                urgency: Some(if event.percentage <= conf.critical_threshold {
                                    Urgency::Critical
                                } else {
                                    Urgency::Low
                                }),
                                ..Default::default()
                            }
                        }
                        WatchEvents::Disk(event) => {
                            debug!(
                                "Disk event: {} at {}",
                                event.name,
                                event.mount_point.to_string_lossy()
                            );
                            let conf = &conf.disk;

                            if !conf.watch_disks.contains(&event.name) {
                                continue;
                            }

                            let perc_usage =
                                (1.0 - (event.available_space / event.total_space) as f64) * 100.0;

                            if perc_usage < conf.critical_threshold.into() {
                                continue;
                            };

                            Notification {
                                summary: "Disk Almost Full".to_string(),
                                body: Some(format!(
                                    "{} at {} is {:.0}% full",
                                    event.name,
                                    event.mount_point.to_string_lossy(),
                                    perc_usage
                                )),
                                urgency: Some(Urgency::Critical),
                                ..Default::default()
                            }
                        }
                        WatchEvents::Memory(e) => {
                            debug!(
                                "Memory event: {}/{} bytes used",
                                e.used_memory, e.total_memory
                            );
                            let conf = &conf.memory;

                            let mem_perc = (e.used_memory / e.total_memory) as f64 * 100.0;
                            if mem_perc < conf.critical_threshold.into() {
                                continue;
                            };

                            Notification {
                                summary: "Memory Critical".to_string(),
                                body: Some(format!("{:.0}% used", mem_perc)),
                                urgency: Some(Urgency::Critical),
                                ..Default::default()
                            }
                        }
                    };
                    let _ = self.notifier.notify(notif);
                }
                Err(e) => {
                    error!("Watch event error: {e}");
                }
            }
        }
    }

    pub fn stop(&self) {
        debug!("WatchService::stop()");
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Drop for WatchService {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct WatchSource {
    config: Arc<Config>,
    running: Arc<AtomicBool>,
    battery: Arc<Battery>,
    disk: Arc<Disk>,
    memory: Arc<Memory>,
}

impl WatchSource {
    pub fn new(config: Arc<Config>, running: Arc<AtomicBool>) -> Self {
        Self {
            config,
            battery: Arc::new(Battery::new(Some(running.clone()))),
            running,
            disk: Arc::new(Disk::new()),
            memory: Arc::new(Memory::new()),
        }
    }

    pub fn listen(&self) -> mpsc::Receiver<Result<WatchEvents, WatchError>> {
        debug!("WatchSource::listen() - creating channel");
        let (tx, rx) = mpsc::channel();

        let config = self.config.clone();
        let tx_c = tx.clone();
        let running = self.running.clone();
        let memory = self.memory.clone();
        debug!("Spawning memory monitoring thread");
        thread::spawn(move || {
            info!(
                "Memory monitor thread started (interval: {}s)",
                config.memory.poll_interval_secs
            );
            while config.memory.enabled && running.load(Ordering::SeqCst) {
                let memory = memory.get();
                tx_c.send(Ok(WatchEvents::Memory(memory))).unwrap();
                thread::sleep(Duration::from_secs(config.memory.poll_interval_secs))
            }
            info!("Memory monitor thread stopped");
        });

        let config = self.config.clone();
        let disk = self.disk.clone();
        let tx_c = tx.clone();
        let running = self.running.clone();
        debug!("Spawning disk monitoring thread");
        thread::spawn(move || {
            info!(
                "Disk monitor thread started (interval: {}s)",
                config.disk.poll_interval_secs
            );
            while config.disk.enabled && running.load(Ordering::SeqCst) {
                for disk in &disk.get() {
                    let event = DiskEvent {
                        name: disk.name().to_string_lossy().to_string(),
                        mount_point: disk.mount_point().to_path_buf(),
                        total_space: disk.total_space(),
                        available_space: disk.available_space(),
                    };
                    tx_c.send(Ok(WatchEvents::Disk(event))).unwrap();
                }
                thread::sleep(Duration::from_secs(config.disk.poll_interval_secs))
            }
            info!("Disk monitor thread stopped");
        });

        let config = self.config.clone();
        let battery = self.battery.clone();
        let tx_c = tx.clone();
        let running = self.running.clone();
        debug!("Spawning battery monitoring thread");
        thread::spawn(move || {
            if config.battery.enabled && running.load(Ordering::SeqCst) {
                info!("Battery monitor thread started");
                let callback = move |battery: BatteryData| {
                    tx_c.send(Ok(WatchEvents::Battery(battery))).unwrap();
                    if !config.battery.enabled {
                        running.store(false, Ordering::SeqCst);
                    }
                };
                let _ = battery.publish_to(callback);
                info!("Battery monitor thread stopped");
            }
        });

        rx
    }
}

#[derive(Debug)]
pub enum WatchEvents {
    Battery(BatteryData),
    Disk(DiskEvent),
    Memory(MemoryData),
}

#[derive(Debug)]
pub struct DiskEvent {
    name: String,
    mount_point: PathBuf,
    total_space: u64,
    available_space: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error(transparent)]
    BatteryError(battery::Error),
}
