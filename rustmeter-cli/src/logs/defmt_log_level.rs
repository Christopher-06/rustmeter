use std::fmt::{Debug, Display};

#[derive(Clone, PartialEq, Eq, Copy)]
pub enum DefmtLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl DefmtLogLevel {
    pub fn try_from_str(level_str: &str) -> anyhow::Result<DefmtLogLevel> {
        match level_str.trim().to_lowercase().as_str() {
            "trace" => Ok(DefmtLogLevel::Trace),
            "debug" => Ok(DefmtLogLevel::Debug),
            "info" => Ok(DefmtLogLevel::Info),
            "warn" => Ok(DefmtLogLevel::Warn),
            "error" => Ok(DefmtLogLevel::Error),
            _ => Err(anyhow::anyhow!("Unknown log level string: {level_str}")),
        }
    }

    /// Get string representation of the log level
    pub fn to_string(&self) -> String {
        match self {
            DefmtLogLevel::Trace => "TRACE".to_string(),
            DefmtLogLevel::Debug => "DEBUG".to_string(),
            DefmtLogLevel::Info => "INFO".to_string(),
            DefmtLogLevel::Warn => "WARN".to_string(),
            DefmtLogLevel::Error => "ERROR".to_string(),
        }
    }

    /// Get colored string representation of the log level
    pub fn to_string_colored(&self) -> String {
        use colored::Colorize;
        match self {
            DefmtLogLevel::Trace => "TRACE".dimmed().to_string(),
            DefmtLogLevel::Debug => "DEBUG".green().to_string(),
            DefmtLogLevel::Info => "INFO ".blue().to_string(),
            DefmtLogLevel::Warn => "WARN ".yellow().to_string(),
            DefmtLogLevel::Error => "ERROR".red().to_string(),
        }
    }
}

impl Display for DefmtLogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Use the colored string representation for debugging
        write!(f, "{}", self.to_string_colored())
    }
}

impl Debug for DefmtLogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Use the normal string representation for debugging
        write!(f, "{}", self.to_string())
    }
}

impl TryFrom<&str> for DefmtLogLevel {
    type Error = anyhow::Error;

    fn try_from(level_str: &str) -> Result<Self, anyhow::Error> {
        DefmtLogLevel::try_from_str(level_str)
    }
}

impl From<defmt_parser::Level> for DefmtLogLevel {
    fn from(level: defmt_parser::Level) -> Self {
        match level {
            defmt_parser::Level::Trace => DefmtLogLevel::Trace,
            defmt_parser::Level::Debug => DefmtLogLevel::Debug,
            defmt_parser::Level::Info => DefmtLogLevel::Info,
            defmt_parser::Level::Warn => DefmtLogLevel::Warn,
            defmt_parser::Level::Error => DefmtLogLevel::Error,
        }
    }
}
