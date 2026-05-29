#[cfg(feature = "defmt")]
mod defmt_logger;
mod espressif_config;
pub use espressif_config::{Config as RustmeterConfig, *};
mod framing;
mod local_critical_section;
mod printing;
mod tracing_esp;
mod panic_handler;

#[cfg(any(feature = "esp32", feature = "esp32s3", feature = "esp32p4"))]
pub const NUM_CORES: usize = 2;
#[cfg(any(
    feature = "esp32s2",
    feature = "esp32c3",
    feature = "esp32c6",
    feature = "esp32h2"
))]
pub const NUM_CORES: usize = 1;

#[derive(Debug)]
pub enum InitializationError {
    UartConfigError(esp_hal::uart::ConfigError),
    TaskSpawnError(embassy_executor::SpawnError),
}

/// Initialize Rustmeter Beacon tracing and logging system
/// This spawns the printing task that handles all output
/// It can run on any executor (Thread or Interrupt) and on any core.
///
///
/// Example:
/// ```no_run
/// use rustmeter_beacon_target::*;
///
/// fn main() -> ! {
///     // Setup e.q.
///     let config = esp_hal::Config::default()
///                         .with_cpu_clock(CpuClock::max()); // any clock config
///     // ...
///
///    // Initialize Rustmeter Beacon
///    init_rustmeter_beacon(
///        RustmeterConfig::new(config.cpu_clock().frequency()),
///        &spawner,
///    );
///
///    // ... rest of your main function
/// }
pub fn init_rustmeter_beacon<P: ConfigPrinterBuild>(
    config: RustmeterConfig<P>,
    spawner: &embassy_executor::Spawner,
) -> Result<(), InitializationError> {
    // Build Printer
    let printer_route = config
        .build_printer_route()
        .map_err(InitializationError::UartConfigError)?;

    // Spawn printing task
    let token = printing::connector(printer_route, config.cpu_freq)
        .map_err(InitializationError::TaskSpawnError)?;
    spawner.spawn(token);

    Ok(())
}
