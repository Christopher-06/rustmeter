#![no_std]

use rtt_target::UpChannel;

pub mod core_id;
mod embassy_trace;
mod executor_registry;
pub mod monitors;
mod numeric_registry;

static mut TRACING_CHANNEL: Option<UpChannel> = None;

pub fn set_tracing_channel(channel: UpChannel) {
    unsafe {
        TRACING_CHANNEL = Some(channel);
    }
}

#[unsafe(no_mangle)]
fn write_tracing_data(data: &[u8]) {
    unsafe {
        let channel = core::ptr::addr_of_mut!(TRACING_CHANNEL);
        if let Some(Some(c)) = channel.as_mut() {
            c.write(data);
        } else {
            #[cfg(feature = "defmt")]
            defmt::warn!("Tracing channel not initialized, cannot write tracing data");

            // This will normally not be reached
        }
    }
}

#[unsafe(no_mangle)]
fn get_tracing_time_us() -> u32 {
    embassy_time::Instant::now().as_micros() as u32
}

#[cfg(feature = "defmt")]
mod with_defmt {

    #[macro_export]
    /// Initializes RustMeter with default RTT configuration:
    /// - Channel 0 for defmt (1kB, NoBlockSkip)
    /// - Channel 1 for tracing (4kB, NoBlockSkip)
    macro_rules! rustmeter_init_default {
        () => {{
            // Initialize RTT with default configuration
            let channels = rtt_target::rtt_init! {
                up: {
                    0: {
                        size: 1024,
                        mode: rtt_target::ChannelMode::NoBlockSkip,
                        name: "defmt"
                    }
                    1: {
                        size: 4096,
                        mode: rtt_target::ChannelMode::NoBlockSkip,
                        name: "RustMeter"
                    }
                }
            };

            // Set defmt channel
            let defmt_channel = channels.up.0;
            rtt_target::set_defmt_channel(defmt_channel);

            // Set tracing channel
            let tracing_channel = channels.up.1;
            set_tracing_channel(tracing_channel);
        }};
    }
}

#[cfg(not(feature = "defmt"))]
mod without_defmt {
    #[macro_export]
    /// Initializes RustMeter with default RTT configuration:
    /// - Channel 1 for tracing (4kB, NoBlockSkip)
    macro_rules! rustmeter_init_default {
        () => {{
            // Initialize RTT with default configuration
            let channels = rtt_target::rtt_init! {
                up: {
                    1: {
                        size: 4096,
                        mode: rtt_target::ChannelMode::NoBlockSkip,
                        name: "RustMeter"
                    }
                }
            };

            // Set tracing channel
            let tracing_channel = channels.up.1;
            set_tracing_channel(tracing_channel);
        }};
    }
}
