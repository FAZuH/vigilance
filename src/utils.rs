use std::env;
use std::path::Path;
use std::path::PathBuf;

use sysinfo::ProcessRefreshKind;
use sysinfo::RefreshKind;
use sysinfo::System;

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

pub fn top_ps_by_mem(limit: usize) -> Vec<MemoryConsumer> {
    let refresh =
        RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing().with_memory());
    let mut ps: Vec<_> = System::new_with_specifics(refresh)
        .processes()
        .iter()
        .filter(|(_, p)| p.thread_kind().is_none())
        .map(|(_, p)| MemoryConsumer {
            name: p.name().to_string_lossy().into(),
            usage: p.memory(),
        })
        .collect();
    ps.sort_by_key(|m| std::cmp::Reverse(m.usage));
    ps.truncate(limit);
    ps
}

#[derive(Clone, Debug)]
pub struct MemoryConsumer {
    pub name: String,
    pub usage: u64,
}
