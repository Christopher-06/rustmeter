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

    /// Parse a log line from a string: e.q. "0.438284 [DEBUG ] pop - New prio level: 0 (esp_rtos esp-rtos-0.2.0/src/run_queue.rs:292)"
    pub fn try_from_str(line: &str) -> anyhow::Result<DefmtLine> {
        // Find open / close brackets for log level
        let open_bracket = line.find('[').ok_or(anyhow::anyhow!(
            "Invalid log line format (found no opening bracket): {line}"
        ))?;
        let close_bracket = line.find(']').ok_or(anyhow::anyhow!(
            "Invalid log line format (found no closing bracket): {line}"
        ))?;
        if open_bracket > close_bracket.saturating_sub(3) {
            return Err(anyhow::anyhow!(
                "Invalid log line format (malformed log level brackets): {line}"
            ));
        }

        // Extract parts
        let timestamp_str = &line[0..open_bracket].trim();
        let level_str = &line[open_bracket + 1..close_bracket].trim();
        let message = line[close_bracket + 1..].trim().to_string();

        // Parse
        let timestamp = match timestamp_str.parse::<f64>() {
            Ok(val) => Some(Duration::from_secs_f64(val).as_micros() as u64),
            Err(_) => None,
        };
        let level = DefmtLogLevel::try_from_str(level_str)
            .context("Failed to parse log level of log line")?;

        Ok(DefmtLine::new(level, timestamp, message))
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
