mod rtt_minimal;
pub use rtt_minimal::_SEGGER_RTT;
use rustmeter_beacon_core::protocol::{EventPayload, TypeDefinitionPayload};
mod connector;
mod tracing_rtt;

#[cfg(feature = "defmt")]
mod defmt_logger;

mod cortex_config;
pub use cortex_config::CortexConfig as RustmeterConfig;

use crate::cortex::connector::connector;

#[cfg(any(feature = "rp2040", feature = "rp235xa", feature = "rp235xb"))]
pub const NUM_CORES: usize = 2;
#[cfg(any(feature = "stm32"))]
pub const NUM_CORES: usize = 1;

#[derive(Debug)]
pub enum InitializationError {
    TaskSpawnError(embassy_executor::SpawnError),
}

/// Initialize Rustmeter Beacon tracing and logging system
/// This spawns the printing task that handles all output. 
/// It can run on any executor (Thread or Interrupt) and on any core.
/// 
/// 
/// Example:
/// ```no_run
/// use rustmeter_beacon_target::*;
/// 
/// fn main() -> ! {
///     // Setup
/// 
///    // Initialize Rustmeter Beacon
///    init_rustmeter_beacon(
///        RustmeterConfig::new(get_system_freq!()), &spawner
///    );
/// }
pub fn init_rustmeter_beacon(
    config: RustmeterConfig,
    spawner: &embassy_executor::Spawner,
) -> Result<(), InitializationError> {
    // embassy-executor 0.10: `#[task]` functions return
    // `Result<SpawnToken<_>, SpawnError>`, and `Spawner::spawn()` takes the
    // unwrapped `SpawnToken` and returns `()`.
    let token =
        connector(config).map_err(InitializationError::TaskSpawnError)?;
    spawner.spawn(token);
    Ok(())
}

#[cfg(feature = "stm32")]
#[macro_export]
/// Macro to get system frequency in Hz for STM32 targets using embassy_stm32. Panics if frequency cannot be determined.
macro_rules! get_system_freq {
    () => {
        unsafe {
            let sys_clk = embassy_stm32::rcc::clocks(&Peripherals::steal().RCC)
                .sys
                .to_hertz();

            match sys_clk {
                Some(freq) => freq.0,
                None => panic!("Could not determine system clock frequency"),
            }
        }
    };
}

#[cfg(feature = "rp2040")]
#[macro_export]
/// Macro to get system frequency in Hz. Panics if frequency cannot be determined.
macro_rules! get_system_freq {
    () => {
        // On RP2040, the we use a 1Mhz timer for tracing
        1_000_000
    };
}
