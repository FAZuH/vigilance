use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::APP_NAME;
use crate::config::Config;
use crate::debug;
use crate::error;
use crate::info;
use crate::model::Battery;
use crate::model::BatteryEvent;
use crate::model::BatteryState;
use crate::model::Disk;
use crate::model::Memory;
use crate::model::MemoryData;
use crate::notify::Notifiable;
use crate::notify::Notification;
use crate::notify::Urgency;
use crate::utils::format_bytes;
use crate::utils::top_ps_by_mem;

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
                        WatchEvents::Battery(e) => match Self::handle_battery(e, conf.clone()) {
                            Some(n) => n,
                            None => continue,
                        },
                        WatchEvents::Disk(e) => match Self::handle_disk(e, conf.clone()) {
                            Some(n) => n,
                            None => continue,
                        },
                        WatchEvents::Memory(e) => match Self::handle_memory(e, conf.clone()) {
                            Some(n) => n,
                            None => continue,
                        },
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

    fn handle_battery(e: BatteryEvent, conf: Arc<Config>) -> Option<Notification> {
        let conf = &conf.battery;
        let ret = match e {
            BatteryEvent::PercentageUpdate(perc) => {
                debug!("Battery: {}%", perc);
                if perc > conf.low_threshold {
                    return None;
                }
                let critical = perc <= conf.critical_threshold;
                let summary = format!("Battery {}", if critical { "Critical" } else { "Low" });
                let body = format!(
                    "{}% remaining — {}",
                    perc,
                    if critical {
                        "Connect charger immediately"
                    } else {
                        "Connect charger soon"
                    }
                );
                let urgency = if critical {
                    Urgency::Critical
                } else {
                    Urgency::Low
                };
                Notification {
                    summary,
                    body: Some(body),
                    urgency: Some(urgency),
                    app_name: Some(APP_NAME.to_string()),
                    ..Default::default()
                }
            }
            BatteryEvent::StateUpdate(state) => {
                debug!("Battery state: {}", state);
                let urgency = match state {
                    BatteryState::Empty => Urgency::Critical,
                    _ => Urgency::Normal,
                };
                Notification {
                    summary: format!("Battery {}", state),
                    body: Some(Self::battery_to_body(state).to_string()),
                    urgency: Some(urgency),
                    app_name: Some(APP_NAME.to_string()),
                    ..Default::default()
                }
            }
        };
        Some(ret)
    }

    fn battery_to_body(state: BatteryState) -> &'static str {
        match state {
            BatteryState::Unknown => "Unknown",
            BatteryState::Charging => "Power connected. Battery is charging",
            BatteryState::Discharging => "Power disconnected. Running on battery power",
            BatteryState::Empty => "Battery is completely drained",
            BatteryState::Full => "Battery is fully charged",
            BatteryState::PendingCharge => "Pending Charge",
            BatteryState::PendingDischarge => "Pending Discharge",
        }
    }

    fn handle_memory(e: MemoryData, conf: Arc<Config>) -> Option<Notification> {
        let conf = &conf.memory;
        let mem_perc = (e.used_memory as f64 / e.total_memory as f64) * 100.0;
        debug!(
            "Memory: {}/{} bytes ({:.0}%)",
            e.used_memory, e.total_memory, mem_perc
        );

        if mem_perc < conf.warning_threshold as f64 {
            return None;
        }

        let critical = mem_perc >= conf.critical_threshold as f64;
        let summary = if critical {
            "Memory Critical"
        } else {
            "Memory Warning"
        };
        let urgency = if critical {
            Urgency::Critical
        } else {
            Urgency::Normal
        };

        let mut body = format!(
            "{} of {} used ({:.0}%)",
            format_bytes(e.used_memory),
            format_bytes(e.total_memory),
            mem_perc
        );

        if critical {
            body.push_str("\nConsider closing unused applications\nTop memory consumers:");
            for c in top_ps_by_mem(5) {
                body.push_str(&format!("\n  {}: {}", c.name, format_bytes(c.usage)));
            }
        }

        Some(Notification {
            summary: summary.to_string(),
            body: Some(body),
            urgency: Some(urgency),
            app_name: Some(APP_NAME.to_string()),
            ..Default::default()
        })
    }

    fn handle_disk(e: DiskEvent, conf: Arc<Config>) -> Option<Notification> {
        debug!(
            "Disk event: {} at {}",
            e.name,
            e.mount_point.to_string_lossy()
        );
        let conf = &conf.disk;

        let mount_point = e.mount_point.to_string_lossy().to_string();
        if !conf.watch_mounts.contains(&mount_point) {
            return None;
        }

        let perc_usage =
            ((e.total_space - e.available_space) as f64 / e.total_space as f64) * 100.0;

        if perc_usage < conf.critical_threshold.into() {
            return None;
        };

        let body = format!(
            "{} ({}):\n\n{} free of {}\n({:.0}% used)",
            e.name,
            mount_point,
            format_bytes(e.available_space),
            format_bytes(e.total_space),
            perc_usage
        );

        Some(Notification {
            summary: "Disk Almost Full".to_string(),
            body: Some(body),
            urgency: Some(Urgency::Critical),
            app_name: Some(APP_NAME.to_string()),
            ..Default::default()
        })
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
                let callback = move |event: BatteryEvent| {
                    debug!("Received battery event: {event:?}");
                    tx_c.send(Ok(WatchEvents::Battery(event))).unwrap();
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
    Battery(BatteryEvent),
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
