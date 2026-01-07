
mod rtt_minimal;
pub use rtt_minimal::_SEGGER_RTT;
mod tracing_rtt;

#[cfg(feature = "defmt")]
mod defmt_logger;

#[cfg(any(feature = "rp2040", feature = "rp235xa", feature = "rp235xb"))]
pub const NUM_CORES: usize = 2;
#[cfg(any(feature = "stm32"))]
pub const NUM_CORES: usize = 1;

#[derive(Debug)]
pub enum InitializationError {
    TaskSpawnError(embassy_executor::SpawnError),
}

/// Initialize Rustmeter Beacon tracing and logging system
/// This spawns the printing task that handles all output
pub fn init_rustmeter_beacon(
    _spawner: &embassy_executor::Spawner,
) -> Result<(), InitializationError> {
    Ok(())
}

// pub fn init_rustmeter_beacon<P: ConfigPrinterBuild>(
//     config: RustmeterConfig<P>,
//     spawner: &embassy_executor::Spawner,
// ) -> Result<(), InitializationError> {
//     Ok(())
// }
