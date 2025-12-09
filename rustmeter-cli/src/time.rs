use std::time::Duration;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Copy, Serialize)]
pub struct EmbassyTime(Duration);

impl EmbassyTime {
    pub fn from_secs_f64(secs: f64) -> Self {
        Self(Duration::from_secs_f64(secs))
    }

    pub fn as_secs_f64(&self) -> f64 {
        self.0.as_secs_f64()
    }

    pub fn as_micros(&self) -> u128 {
        self.0.as_micros()
    }
}
