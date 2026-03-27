use std::fmt;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use dbus::Message;
use dbus::arg;
use dbus::blocking::Connection;

use crate::debug;
use crate::info;
use crate::warn;

pub struct BatteryModel {
    running: Arc<AtomicBool>,
}
#[derive(Debug)]
pub enum BatteryEvent {
    PercentageUpdate(u8),
    StateUpdate(BatteryState),
}
#[derive(Debug)]
pub enum BatteryState {
    Unknown,
    Charging,
    Discharging,
    Empty,
    Full,
    PendingCharge,
    PendingDischarge,
}
impl fmt::Display for BatteryState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            BatteryState::Unknown => "Unknown",
            BatteryState::Charging => "Charging",
            BatteryState::Discharging => "Unplugged",
            BatteryState::Empty => "Empty",
            BatteryState::Full => "Full",
            BatteryState::PendingCharge => "Pending Charge",
            BatteryState::PendingDischarge => "Pending Discharge",
        };
        write!(f, "{}", s)
    }
}
impl BatteryState {
    pub fn from_upower_variant(variant: u8) -> Result<Self, ModelError> {
        let ret = match variant {
            0 => Self::Unknown,
            1 => Self::Charging,
            2 => Self::Discharging,
            3 => Self::Empty,
            4 => Self::Full,
            5 => Self::PendingCharge,
            6 => Self::PendingDischarge,
            _ => {
                warn!("Invalid battery variant: {}", variant);
                return Err(ModelError::InvalidBatteryVariant(variant));
            }
        };
        debug!("Battery state resolved");
        Ok(ret)
    }
}
pub struct DiskModel;
pub struct MemoryModel;
#[derive(Debug)]
pub struct MemoryData {
    pub total_memory: u64,
    pub used_memory: u64,
    pub total_swap: u64,
    pub used_swap: u64,
}

impl DiskModel {
    pub fn new() -> Self {
        Self {}
    }
    pub fn get(&self) -> sysinfo::Disks {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        debug!("Found {} disk(s)", disks.len());
        disks
    }
}

impl MemoryModel {
    pub fn new() -> Self {
        Self {}
    }
    pub fn get(&self) -> MemoryData {
        let kind = sysinfo::MemoryRefreshKind::everything();
        let refreshes = sysinfo::RefreshKind::nothing().with_memory(kind);

        let sys = sysinfo::System::new_with_specifics(refreshes);
        let data = MemoryData {
            total_memory: sys.total_memory(),
            used_memory: sys.used_memory(),
            total_swap: sys.total_swap(),
            used_swap: sys.used_swap(),
        };
        debug!(
            "Memory: {}/{} bytes used, Swap: {}/{} bytes used",
            data.used_memory, data.total_memory, data.used_swap, data.total_swap
        );
        data
    }
}

impl BatteryModel {
    pub fn new(running: Option<Arc<AtomicBool>>) -> Self {
        let running = match running {
            Some(running) => running,
            None => Arc::new(AtomicBool::new(false)),
        };
        Self { running }
    }

    pub fn publish_to<F>(&self, callback: F) -> Result<(), ModelError>
    where
        F: Fn(BatteryEvent) + 'static + Send,
    {
        self.running.store(true, Ordering::SeqCst);
        let c = Connection::new_system()?;
        info!("Connected to system D-Bus");
        let proxy = c.with_proxy(
            "org.freedesktop.UPower",
            "/org/freedesktop/UPower/devices/battery_BAT0",
            Duration::from_secs(10),
        );

        let _id = proxy.match_signal(move |p: PropertiesChanged, _: &Connection, _: &Message| {
            let perc = p
                .changed
                .get("Percentage")
                .and_then(|v| arg::cast::<f64>(&v.0).copied())
                .map(|v| v as u8);
            let state = p
                .changed
                .get("State")
                .and_then(|v| arg::cast::<u32>(&v.0).copied())
                .map(|v| v as u8);
            debug!(
                "Received battery D-Bus signal: perc={:?} state={:?}",
                perc, state
            );

            let event = match (state, perc) {
                (Some(s), _) => {
                    BatteryEvent::StateUpdate(BatteryState::from_upower_variant(s).unwrap())
                }
                (_, Some(p)) => BatteryEvent::PercentageUpdate(p),
                _ => return true,
            };

            callback(event);
            true
        });

        info!("Battery monitoring loop started");
        while self.running.load(Ordering::SeqCst) {
            c.process(Duration::from_secs(10)).unwrap();
        }
        info!("Battery monitoring stopped");
        Ok(())
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug)]
pub struct PropertiesChanged {
    pub interface: String,
    pub changed: arg::PropMap,
}

impl arg::AppendAll for PropertiesChanged {
    fn append(&self, i: &mut arg::IterAppend) {
        arg::RefArg::append(&self.interface, i);
        arg::RefArg::append(&self.changed, i);
    }
}

impl arg::ReadAll for PropertiesChanged {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(PropertiesChanged {
            interface: i.read()?,
            changed: i.read()?,
        })
    }
}

impl dbus::message::SignalArgs for PropertiesChanged {
    const NAME: &'static str = "PropertiesChanged";
    const INTERFACE: &'static str = "org.freedesktop.DBus.Properties";
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error(transparent)]
    Dbus(#[from] dbus::Error),

    #[error("Invalid variant: {0}")]
    InvalidBatteryVariant(u8),
}
