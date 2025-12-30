use std::{
    fmt::{Debug, Display},
    time::Duration,
};

use anyhow::Context;

use crate::logs::defmt_log_level::DefmtLogLevel;

#[derive(Clone, PartialEq, Eq)]
pub struct DefmtLine {
    log_level: DefmtLogLevel,
    timestamp_us: Option<u64>,
    message: String,
}

impl DefmtLine {
    pub fn new(log_level: DefmtLogLevel, timestamp_us: Option<u64>, message: String) -> Self {
        DefmtLine {
            log_level,
            timestamp_us,
            message,
        }
    }

    pub fn log_level(&self) -> DefmtLogLevel {
        self.log_level
    }

    pub fn timestamp_us(&self) -> Option<u64> {
        self.timestamp_us
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for DefmtLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.timestamp_us {
            Some(ts) => write!(
                f,
                "{:.6} [{}] {}",
                ts as f64 / 1_000_000.0,
                self.log_level,
                self.message
            ),
            None => write!(f, "[{:?}] {}", self.log_level, self.message),
        }
    }
}

impl Debug for DefmtLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.timestamp_us {
            Some(ts) => write!(f, "{}us [{:?}] {}", ts, self.log_level, self.message),
            None => write!(f, "[{:?}] {}", self.log_level, self.message),
        }
    }
}

// Map defmt log levels to DefmtLogLevel
impl<'a> TryFrom<defmt_decoder::Frame<'a>> for DefmtLine {
    type Error = anyhow::Error;

    fn try_from(value: defmt_decoder::Frame<'a>) -> Result<Self, Self::Error> {
        let level = value
            .level()
            .context("Defmt Log Level cannot be found")?
            .into();
        let message = value.display_message().to_string();

        // Convert timestamp to string and then to u64 microseconds (is there a better way?)
        let timestamp_str = value.display_timestamp().map(|ts| ts.to_string());
        let timestamp = match timestamp_str {
            Some(ts_str) => {
                let ts_float: f64 = ts_str.parse().context("Failed to parse timestamp string")?;
                Some(Duration::from_secs_f64(ts_float).as_micros() as u64)
            }
            None => None,
        };

        Ok(DefmtLine::new(level, timestamp, message))
    }
}
