use std::{fmt::Display, time::Duration};

use rustmeter_beacon::protocol::EventPayload;

use crate::CoreInfo;

#[derive(Debug, Clone)]
pub struct TracingItem {
    core: CoreInfo,
    // Absolute time in ticks of the microcontroller
    uc_timeticks: u64,
    // Absolute time in host pc timestamp when the event was received
    pc_timestamp: Duration,
    payload: EventPayload,
}

impl TracingItem {
    pub fn new(
        core: CoreInfo,
        uc_timeticks: u64,
        pc_timestamp: Duration,
        payload: EventPayload,
    ) -> Self {
        Self {
            core,
            uc_timeticks,
            pc_timestamp,
            payload,
        }
    }

    pub fn uc_timeticks(&self) -> u64 {
        self.uc_timeticks
    }

    pub fn pc_timestamp(&self) -> &Duration {
        &self.pc_timestamp
    }

    pub fn core(&self) -> CoreInfo {
        self.core
    }

    pub fn payload(&self) -> &EventPayload {
        &self.payload
    }
}

impl Display for TracingItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[Tracing - {:?}] {:.6}s - {:?}",
            self.core,
            self.pc_timestamp.as_secs_f64(),
            self.payload
        )
    }
}
