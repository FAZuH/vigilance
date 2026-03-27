use std::sync::OnceLock;

static LOG_LEVEL: OnceLock<u8> = OnceLock::new();

pub fn log_level() -> u8 {
    *LOG_LEVEL.get_or_init(|| {
        match std::env::var("VIGILANCE_LOG")
            .map(|v| v.to_ascii_lowercase())
            .as_deref()
        {
            Ok("error") => 1,
            Ok("warn") => 2,
            Ok("info") => 3,
            Ok("debug") => 4,
            _ => 0,
        }
    })
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        if $crate::log::log_level() >= 1 {
            eprintln!("[ERROR] {}", format_args!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        if $crate::log::log_level() >= 2 {
            eprintln!("[WARN] {}", format_args!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        if $crate::log::log_level() >= 3 {
            eprintln!("[INFO] {}", format_args!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::log::log_level() >= 4 {
            eprintln!("[DEBUG] {}", format_args!($($arg)*));
        }
    };
}
