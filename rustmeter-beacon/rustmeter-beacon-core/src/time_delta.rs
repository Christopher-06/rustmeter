use crate::buffer::BufferWriter;

static mut LAST_TIMESTAMP: u32 = 0;

unsafe extern "Rust" {
    /// Low-level function to get the current tracing time in microseconds. Implemented in the target crate.
    fn get_tracing_time_us() -> u32;
}

pub(crate) struct TimeDelta {
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
    pub fn write(&self, writer: &mut BufferWriter) {
        if self.is_extended() {
            // Cap value at 2^31 - 1
            let capped_delta = if self.delta > (2u32.pow(31) - 1) {
                2u32.pow(31) - 1
            } else {
                self.delta
            };

            // Use extended format (4 bytes)
            let extended_value = capped_delta | 0x8000_0000; // Set highest bit to 1
            writer.write_bytes(&extended_value.to_le_bytes());
        } else {
            // Single format (2 bytes)
            let single_value = (self.delta & 0x7FFF) as u16; // Ensure highest bit is 0
            writer.write_bytes(&single_value.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::buffer::BufferWriter;

    #[test]
    fn test_time_delta_write() {
        // Test single format
        let td_single = TimeDelta { delta: 12345 };
        let mut writer_single = BufferWriter::new();
        td_single.write(&mut writer_single);
        let written_single = writer_single.as_slice();
        assert_eq!(written_single.len(), 2);
        let value_single = u16::from_le_bytes([written_single[0], written_single[1]]);
        assert_eq!(value_single & 0x7FFF, 12345); // Highest bit should be 0

        // Test extended format
        let td_extended = TimeDelta { delta: 40000 };
        let mut writer_extended = BufferWriter::new();
        td_extended.write(&mut writer_extended);
        let written_extended = writer_extended.as_slice();
        assert_eq!(written_extended.len(), 4);
        let value_extended = u32::from_le_bytes([
            written_extended[0],
            written_extended[1],
            written_extended[2],
            written_extended[3],
        ]);
        assert_eq!(value_extended & 0x7FFF_FFFF, 40000); // Highest bit should be 1
    }
}
