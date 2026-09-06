use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    #[must_use]
    pub fn parse_name(level: &str) -> Self {
        match level.to_uppercase().as_str() {
            "OFF" => Self::Off,
            "FATAL" | "ERROR" => Self::Error,
            "WARN" => Self::Warn,
            "DEBUG" => Self::Debug,
            "TRACE" | "ALL" => Self::Trace,
            _ => Self::Info,
        }
    }

    #[must_use]
    pub fn as_filter(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

#[must_use]
pub fn level_for(verbosity: u8, log_level: Option<&str>) -> LogLevel {
    let level = log_level.map_or(LogLevel::Info, LogLevel::parse_name);
    match verbosity {
        0 => level,
        1 => level.max(LogLevel::Debug),
        _ => LogLevel::Trace,
    }
}

static INITIALISED: AtomicBool = AtomicBool::new(false);

pub fn init(level: LogLevel) {
    if INITIALISED.swap(true, Ordering::SeqCst) {
        return;
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(level.as_filter()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_target(false)
        .without_time()
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbosity_raises_the_level_and_an_unknown_name_is_info() {
        assert_eq!(level_for(0, None), LogLevel::Info);
        assert_eq!(level_for(0, Some("warn")), LogLevel::Warn);
        assert_eq!(level_for(0, Some("nonsense")), LogLevel::Info);
        assert_eq!(level_for(1, Some("error")), LogLevel::Debug);
        assert_eq!(level_for(2, None), LogLevel::Trace);
    }
}
