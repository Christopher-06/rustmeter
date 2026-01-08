use arbitrary_int::traits::Integer;

use crate::{
    buffer::{BufferReader, BufferWriter},
    protocol::{EventPayload, TypeDefinitionPayload},
    tracing::write_tracing_event,
};

#[cfg(not(feature = "multicore"))]
static LAST_TIMESTAMP: [portable_atomic::AtomicU32; 1] = [portable_atomic::AtomicU32::new(0)];
#[cfg(feature = "multicore")]
static LAST_TIMESTAMP: [portable_atomic::AtomicU32; 2] = [
    portable_atomic::AtomicU32::new(0),
    portable_atomic::AtomicU32::new(0),
];

pub static CORE_CLOCK_REFERENCED: [portable_atomic::AtomicBool; 2] = [
    portable_atomic::AtomicBool::new(false),
    portable_atomic::AtomicBool::new(false),
];

#[inline(always)]
fn do_core_clock_referencing(core_id: usize) {
    // Send a CoreClockReference event to establish the baseline timestamp for this core. This must be done inside a
    // critical section to avoid interrupts interfering with the timestamp measurement. (Core-local would be sufficient,
    // but critical section is easier to implement cross-platform.)
    // Normally TimeDelta is already inside a critical section when called from tracing event writing, so this should be safe to do
    // without critical section here again. But we do it anyway to be sure.
    critical_section::with(|_| {
        CORE_CLOCK_REFERENCED[core_id].store(true, portable_atomic::Ordering::Relaxed);

        #[cfg(not(feature = "std"))]
        unsafe { preinit_clock_reference() };
        
        let cpu_ticks = unsafe { get_tracing_raw_ticks() };
        let systimer_us = unsafe { get_system_time_us() };

        write_tracing_event(EventPayload::TypeDefinition(
            TypeDefinitionPayload::CoreClockReference {
                core_id: core_id as u8,
                systimer_us,
                cpu_ticks,
            },
        ));
    });
}

#[cfg(not(feature = "std"))]
unsafe extern "Rust" {
    /// Low-level function to get the current tracing time in microseconds. Implemented in the target crate.
    /// In tests this already should return the timedelta directly.
    // pub fn get_tracing_time_us() -> u32;

    /// Low-level function to preinitialize the clock reference for the target core. Implemented in the target crate.
    /// Runs in critical section.
    pub fn preinit_clock_reference();

    /// Low-level function to get the current tracing raw ticks. Implemented in the target crate.
    pub fn get_tracing_raw_ticks() -> u32;

    pub fn get_system_time_us() -> u64;
}

#[cfg(all(feature = "std"))]
#[unsafe(no_mangle)]
unsafe fn get_tracing_raw_ticks() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    now.as_micros() as u32
}

#[cfg(all(feature = "std"))]
#[unsafe(no_mangle)]
unsafe fn get_system_time_us() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    now.as_micros() as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeDelta {
    delta: u32,
}

impl TimeDelta {
    /// This has to be called inside a critical section
    #[inline(always)]
    pub fn from_now() -> Self {
        let core_id = unsafe { crate::get_current_core_id() as usize };
        if !CORE_CLOCK_REFERENCED[core_id].load(portable_atomic::Ordering::Relaxed) {
            do_core_clock_referencing(core_id);
        }

        // estimate time between last timestamp and now
        let now = unsafe { get_tracing_raw_ticks() };
        let last = LAST_TIMESTAMP[core_id].swap(now, portable_atomic::Ordering::Relaxed);

        if now < last {
            // Handle wrap-around
            let last_till_end = arbitrary_int::u26::MAX.as_u32() - last;
            return TimeDelta {
                delta: last_till_end + now,
            };
        } else {
            TimeDelta { delta: now - last }
        }
    }

    // #[cfg(test)]
    // pub fn from_now() -> Self {
    //     let now = unsafe { get_tracing_time_us() };
    //     TimeDelta { delta: now }
    // }

    /// Returns true if the TimeDelta requires extended format (4 bytes), false if it can be represented in single format (2 bytes).
    #[inline(always)]
    pub const fn is_extended(&self) -> bool {
        self.delta >= 2u32.pow(15)
    }

    /// Write the TimeDelta into the provided writer. It will use either 2 or 4 bytes depending on the size:
    /// - If the delta is less than 2^15, it will be written as a 2-byte value with the highest bit set to 0.
    /// - If the delta is 2^15 or more, it will be written as a 4-byte value with the highest bit set to 1. If the delta exceeds 2^31 - 1, it will be capped to that value.
    pub fn write_bytes(&self, writer: &mut BufferWriter) {
        if self.is_extended() {
            // Cap value at 2^31 - 1
            let capped_delta = if self.delta > (2u32.pow(31) - 1) {
                2u32.pow(31) - 1
            } else {
                self.delta
            };

            // Use extended format (4 bytes)
            let extended_value = capped_delta | 0x8000_0000; // Set highest bit to 1
            writer.write_bytes(&extended_value.to_be_bytes());
        } else {
            // Single format (2 bytes)
            let single_value = (self.delta & 0x7FFF) as u16; // Ensure highest bit is 0
            writer.write_bytes(&single_value.to_be_bytes());
        }
    }

    #[inline(always)]
    pub fn write_bytes_mut(&self, writer: &mut [u8]) -> usize {
        if self.is_extended() {
            // Cap value at 2^31 - 1
            let capped_delta = if self.delta > (2u32.pow(31) - 1) {
                2u32.pow(31) - 1
            } else {
                self.delta
            };

            // Use extended format (4 bytes)
            let extended_value = capped_delta | 0x8000_0000; // Set highest bit to 1
            let bytes = extended_value.to_be_bytes();
            writer[..4].copy_from_slice(&bytes);
            4
        } else {
            // Single format (2 bytes)
            let single_value = self.delta as u16;
            let bytes = single_value.to_be_bytes();
            writer[..2].copy_from_slice(&bytes);
            2
        }
    }

    /// Reads a TimeDelta from the provided reader. Returns None if reading fails.
    /// It automatically detects whether the format is single (2 bytes) or extended (4 bytes) based on the highest bit.
    pub fn read_bytes(reader: &mut BufferReader) -> Option<Self> {
        // Read first 2 bytes to determine format
        let first_byte = reader.read_byte()?;
        let second_byte = reader.read_byte()?;

        if (first_byte & 0x80) == 0 {
            // Single format
            let delta = u16::from_be_bytes([first_byte, second_byte]) as u32;
            Some(TimeDelta { delta })
        } else {
            // Extended format, read additional 2 bytes
            let next_two_bytes = reader.read_bytes(2)?;
            let extended_value = u32::from_be_bytes([
                first_byte,
                second_byte,
                next_two_bytes[0],
                next_two_bytes[1],
            ]);
            let delta = extended_value & 0x7FFF_FFFF; // Clear highest bit
            Some(TimeDelta { delta })
        }
    }

    pub fn get_delta_us(&self) -> u32 {
        self.delta
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::buffer::BufferWriter;

    #[test]
    fn test_time_delta_read_and_write_exponents() {
        // Simply test all exponents from 0 to 32
        for exponent in 0..=32 {
            let delta = (2u64.pow(exponent) - 1) as u32; // u64 because 2^32 doesn't fit in u32
            let time_delta = TimeDelta { delta };

            // Write to buffer
            let mut writer = BufferWriter::new();
            time_delta.write_bytes(&mut writer);
            let written_bytes = writer.as_slice();

            if exponent <= 15 {
                // Single format (2 bytes)
                assert_eq!(
                    written_bytes.len(),
                    2,
                    "Expected 2 bytes for delta {}",
                    delta
                );
            } else {
                // Extended format (4 bytes)
                assert_eq!(
                    written_bytes.len(),
                    4,
                    "Expected 4 bytes for delta {}",
                    delta
                );
            }

            // Read from buffer
            let mut reader = BufferReader::new(written_bytes);
            let read_time_delta =
                TimeDelta::read_bytes(&mut reader).expect("Failed to read TimeDelta");

            // 2^31 - 1 capping check
            let expected_delta = delta.min(2u32.pow(31) - 1);

            assert_eq!(
                expected_delta, read_time_delta.delta,
                "Mismatch for delta {}",
                delta
            );
        }
    }

    #[test]
    fn test_time_delta_read_and_write_specials() {
        let deltas = [
            (0u32, 2),
            (1u32, 2),
            (2u32.pow(15) - 1, 2),
            (2u32.pow(15), 4),
            (2u32.pow(15) + 1, 4),
            (2u32.pow(16), 4),
            (2u32.pow(31) - 1, 4),
            (2u32.pow(31), 4),
        ];

        for (delta, byte_size) in &deltas {
            let time_delta = TimeDelta { delta: *delta };

            // Write to buffer
            let mut writer = BufferWriter::new();
            time_delta.write_bytes(&mut writer);
            let written_bytes = writer.as_slice();

            assert_eq!(
                written_bytes.len(),
                *byte_size,
                "Expected {} bytes for delta {}",
                byte_size,
                delta
            );

            // Read from buffer
            let mut reader = BufferReader::new(written_bytes);
            let read_time_delta =
                TimeDelta::read_bytes(&mut reader).expect("Failed to read TimeDelta");

            // 2^31 - 1 capping check
            let expected_delta = (*delta).min(2u32.pow(31) - 1);

            assert_eq!(
                expected_delta, read_time_delta.delta,
                "Mismatch for delta {}",
                delta
            );
        }
    }
}
