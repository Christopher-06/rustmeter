use std::fmt::Display;

use anyhow::Context;

use crate::{perfetto_backend::trace_event::CName, time::EmbassyTime};

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn from_str(level_str: &str) -> anyhow::Result<LogLevel> {
        match level_str.trim().to_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(anyhow::anyhow!("Unknown log level string: {}", level_str)),
        }
    }

    pub fn get_cname(&self) -> CName {
        match self {
            LogLevel::Trace => CName::Good,
            LogLevel::Debug => CName::Good,
            LogLevel::Info => CName::Good,
            LogLevel::Warn => CName::Terrible,
            LogLevel::Error => CName::Terrible,
        }
    }
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let level_str = match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        };
        write!(f, "{}", level_str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub timestamp: EmbassyTime,
    pub level: LogLevel,
    pub message: String,
}

impl LogLine {
    pub fn new(timestamp: EmbassyTime, level: LogLevel, message: String) -> Self {
        LogLine {
            timestamp,
            level,
            message,
        }
    }

    /// Parse a log line from a string: e.q. "0.438284 [DEBUG ] pop - New prio level: 0 (esp_rtos esp-rtos-0.2.0/src/run_queue.rs:292)"
    pub fn from_str(line: &str) -> anyhow::Result<LogLine> {
        // Find open / close brackets for log level
        let open_bracket = line.find('[').ok_or(anyhow::anyhow!(
            "Invalid log line format (found no opening bracket): {}",
            line
        ))?;
        let close_bracket = line.find(']').ok_or(anyhow::anyhow!(
            "Invalid log line format (found no closing bracket): {}",
            line
        ))?;
        if open_bracket > close_bracket.saturating_sub(3) {
            return Err(anyhow::anyhow!(
                "Invalid log line format (malformed log level brackets): {}",
                line
            ));
        }

        // Extract parts
        let timestamp_str = &line[0..open_bracket].trim();
        let level_str = &line[open_bracket + 1..close_bracket].trim();
        let message = line[close_bracket + 1..].trim().to_string();

        // Parse
        let timestamp = EmbassyTime::from_secs_f64(
            timestamp_str
                .parse::<f64>()
                .context("Failed to parse timestamp of log line")?,
        );
        let level =
            LogLevel::from_str(level_str).context("Failed to parse log level of log line")?;
        Ok(LogLine::new(timestamp, level, message))
    }
}

impl Display for LogLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.6} [{}] {}",
            self.timestamp.as_secs_f64(),
            self.level,
            self.message
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_line_parsing() {
        let log_str = "0.438284 [DEBUG ] pop - New prio level: 0 (esp_rtos esp-rtos-0.2.0/src/run_queue.rs:292)";
        let log_line = LogLine::from_str(log_str).expect("Failed to parse log line");

        assert_eq!(log_line.timestamp.as_secs_f64(), 0.438284);
        assert_eq!(log_line.level, LogLevel::Debug);
        assert_eq!(
            log_line.message,
            "pop - New prio level: 0 (esp_rtos esp-rtos-0.2.0/src/run_queue.rs:292)"
        );
    }
}
