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
/// This spawns the printing task that handles all output
pub fn init_rustmeter_beacon(
    config: RustmeterConfig,
    spawner: &embassy_executor::Spawner,
) -> Result<(), InitializationError> {
    spawner.spawn(connector(config).map_err(InitializationError::TaskSpawnError)?);

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
