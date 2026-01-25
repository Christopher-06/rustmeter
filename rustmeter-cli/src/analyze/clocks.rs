use std::time::Duration;

use rustmeter_beacon_core::protocol::{EventPayload, TypeDefinitionPayload};

use crate::tracing::tracing_item::TracingItem;

/// Definition of the global clock configuration used for tracing timestamp calculations. This
/// will not change during a tracing session. Else it will be treated as an error!
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GlobalClockDefinition {
    pub tick_divider: u16,
    pub cpu_clock_hz: u32,
}

impl TryFrom<&TypeDefinitionPayload> for GlobalClockDefinition {
    type Error = anyhow::Error;

    fn try_from(value: &TypeDefinitionPayload) -> Result<Self, Self::Error> {
        match value {
            TypeDefinitionPayload::GlobalClockConfiguration {
                system_frequency_hz,
                tick_divider,
            } => Ok(GlobalClockDefinition {
                cpu_clock_hz: *system_frequency_hz,
                tick_divider: *tick_divider,
            }),
            _ => Err(anyhow::anyhow!("Not a GlobalClockConfiguration typedef")),
        }
    }
}

/// Reference clock event point from the microcontroller, used for tracing timestamp calculations
/// based on the clock configuration. This freezes the current microcontroller clock state at a given
/// point in time to regressively calculate timestamps of other events in all cores / system timer.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ClockReference {
    pub core_id: u8,
    /// Current systimer in microseconds
    pub systimer_us: u64,
    /// Current CPU ticks register value (divided by tick divider)
    pub cpu_ticks: u32,
    /// Cumsum of timedelta ticks till this clock reference
    pub uc_timeticks: u64,
    /// PC timestamp when this clock reference was recved
    pub pc_timestamp: Duration,
}

impl ClockReference {
    pub fn new(
        core_id: u8,
        systimer_us: u64,
        cpu_ticks: u32,
        uc_timeticks: u64,
        pc_timestamp: Duration,
    ) -> Self {
        Self {
            core_id,
            systimer_us,
            cpu_ticks,
            uc_timeticks,
            pc_timestamp,
        }
    }

    /// Try to create ClockReference from any TypeDefinitionPayload
    pub fn try_from_typedef(
        value: &TypeDefinitionPayload,
        pc_timestamp: Duration,
        uc_timeticks: u64,
    ) -> anyhow::Result<Self> {
        match value {
            TypeDefinitionPayload::CoreClockReference {
                core_id,
                systimer_us,
                cpu_ticks,
            } => Ok(ClockReference::new(*core_id, *systimer_us, *cpu_ticks, uc_timeticks, pc_timestamp)),
            _ => Err(anyhow::anyhow!("Not a CoreClockReference typedef")),
        }
    }
}

impl TryFrom<&TracingItem> for ClockReference {
    type Error = anyhow::Error;

    fn try_from(value: &TracingItem) -> Result<Self, Self::Error> {
        match value.payload() {
            EventPayload::TypeDefinition(typedef) => ClockReference::try_from_typedef(
                typedef,
                *value.pc_timestamp(),
                value.uc_timeticks(),
            ),
            _ => Err(anyhow::anyhow!(
                "TracingItem does not contain a TypeDefinition payload"
            )),
        }
    }
}