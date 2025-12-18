use crate::buffer::{BufferReader, BufferWriter};

static mut LAST_TIMESTAMP: u32 = 0;

unsafe extern "Rust" {
    /// Low-level function to get the current tracing time in microseconds. Implemented in the target crate.
    fn get_tracing_time_us() -> u32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeDelta {
    delta: u32,
}

impl TimeDelta {
    /// This has to be called inside a critical section
    pub fn from_now() -> Self {
        // estimate time between last timestamp and now
        let now = unsafe { get_tracing_time_us() };
        let delta = now.wrapping_sub(unsafe { LAST_TIMESTAMP });

        // update last timestamp
        unsafe {
            LAST_TIMESTAMP = now;
        }

        TimeDelta { delta }
    }

    /// Returns true if the TimeDelta requires extended format (4 bytes), false if it can be represented in single format (2 bytes).
    pub const fn is_extended(&self) -> bool {
        self.delta >= 2u32.pow(15)
    }

    /// Write the TimeDelta into the provided writer. It will use either 2 or 4 bytes depending on the size:
    /// - If the delta is less than 2^15, it will be written as a 2-byte value with the highest bit set to 0.
    /// - If the delta is 2^15 or more, it will be written as a 4-byte value with the highest bit set to 1. If the delta exceeds 2^31 - 1, it will be capped to that value.
    pub(crate) fn write_bytes(&self, writer: &mut BufferWriter) {
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
