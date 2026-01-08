

pub struct CortexConfig {
    /// system Frequency in Hz
    pub system_frequency_hz: u32,
}

impl CortexConfig {
    pub fn new(system_frequency_hz: u32) -> Self {
        Self {
            system_frequency_hz,
        }
    }

    pub fn with_system_frequency_hz(mut self, system_frequency_hz: u32) -> Self {
        self.system_frequency_hz = system_frequency_hz;
        self
    }
}