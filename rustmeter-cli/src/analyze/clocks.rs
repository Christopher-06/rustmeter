use rustmeter_beacon::protocol::TypeDefinitionPayload;

pub struct GlobalClockDefinition {
    pub tick_divider: u16,
    pub cpu_clock_hz: u32,
}

impl GlobalClockDefinition {
    /// Get any GlobalClockDefinition from an iterator over TypeDefinitionPayloads
    pub fn from_typedef_iter<'a>(
        iter: impl Iterator<Item = &'a TypeDefinitionPayload>,
    ) -> anyhow::Result<Self> {
        iter.filter_map(|typedef| GlobalClockDefinition::try_from(typedef).ok())
            .next()
            .ok_or_else(|| anyhow::anyhow!("No GlobalClockConfiguration found in typedefs"))
    }
}

pub struct ClockReference {
    pub core_id: u8,
    pub systimer_us: u64,
    pub cpu_ticks: u32,
}

impl ClockReference {
    /// Get all ClockReference entries for an optional specific core from an iterator over TypeDefinitionPayloads
    pub fn all_from_typedef_iter<'a>(
        iter: impl Iterator<Item = &'a TypeDefinitionPayload>,
        core_id: Option<u8>,
    ) -> Vec<Self> {
        iter.filter_map(|typedef| {
            let cref = ClockReference::try_from(typedef).ok()?;
            if core_id.map_or(true, |id| cref.core_id == id) {
                Some(cref)
            } else {
                None
            }
        })
        .collect()
    }
}

impl TryFrom<&TypeDefinitionPayload> for ClockReference {
    type Error = anyhow::Error;

    fn try_from(value: &TypeDefinitionPayload) -> Result<Self, Self::Error> {
        match value {
            TypeDefinitionPayload::CoreClockReference {
                core_id,
                systimer_us,
                cpu_ticks,
            } => Ok(ClockReference {
                core_id: *core_id,
                systimer_us: *systimer_us,
                cpu_ticks: *cpu_ticks,
            }),
            _ => Err(anyhow::anyhow!(
                "Invalid TypeDefinitionPayload for ClockReference"
            )),
        }
    }
}

impl TryFrom<&TypeDefinitionPayload> for GlobalClockDefinition {
    type Error = anyhow::Error;

    fn try_from(value: &TypeDefinitionPayload) -> Result<Self, Self::Error> {
        match value {
            TypeDefinitionPayload::GlobalClockConfiguration {
                tick_divider,
                system_frequency_hz,
            } => Ok(GlobalClockDefinition {
                tick_divider: *tick_divider,
                cpu_clock_hz: *system_frequency_hz,
            }),
            _ => Err(anyhow::anyhow!(
                "Invalid TypeDefinitionPayload for GlobalClockDefinition"
            )),
        }
    }
}
