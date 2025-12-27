#[cfg(feature = "defmt")]
pub mod esp_defmt_pipe;
pub mod espressif_config;
pub mod tracing_esp;

mod printing;
pub use printing::*;

// TODO: Add Serial-JTAG support for better performance on supported chips