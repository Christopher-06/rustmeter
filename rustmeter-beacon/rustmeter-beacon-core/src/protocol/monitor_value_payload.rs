use crate::buffer::{BufferReader, BufferWriter};

/// Payloads for Monitor Value Events
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorValuePayload {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
}

impl MonitorValuePayload {
    pub fn type_id(&self) -> u8 {
        // cannot be u3 because one byte must be used for length encoding
        match self {
            MonitorValuePayload::U8(x) => x.get_monitor_value_type_id(),
            MonitorValuePayload::U16(x) => x.get_monitor_value_type_id(),
            MonitorValuePayload::U32(x) => x.get_monitor_value_type_id(),
            MonitorValuePayload::U64(x) => x.get_monitor_value_type_id(),
            MonitorValuePayload::I8(x) => x.get_monitor_value_type_id(),
            MonitorValuePayload::I16(x) => x.get_monitor_value_type_id(),
            MonitorValuePayload::I32(x) => x.get_monitor_value_type_id(),
            MonitorValuePayload::I64(x) => x.get_monitor_value_type_id(),
        }
    }

    /// Write the payload data into the provided buffer.
    /// Returns the number of data bytes written into the provided buffer. Assumes the buffer is large enough.
    pub(crate) fn write_bytes(&self, buffer: &mut BufferWriter) {
        match self {
            MonitorValuePayload::U8(v) => {
                buffer.write_byte(*v);
            }
            MonitorValuePayload::U16(v) => {
                buffer.write_bytes(&v.to_le_bytes());
            }
            MonitorValuePayload::U32(v) => {
                buffer.write_bytes(&v.to_le_bytes());
            }
            MonitorValuePayload::U64(v) => {
                buffer.write_bytes(&v.to_le_bytes());
            }
            MonitorValuePayload::I8(v) => {
                buffer.write_byte(*v as u8);
            }
            MonitorValuePayload::I16(v) => {
                buffer.write_bytes(&v.to_le_bytes());
            }
            MonitorValuePayload::I32(v) => {
                buffer.write_bytes(&v.to_le_bytes());
            }
            MonitorValuePayload::I64(v) => {
                buffer.write_bytes(&v.to_le_bytes());
            }
        };
    }

    /// Reads a MonitorValuePayload from the provided buffer based on the given type ID.
    pub(crate) fn from_bytes(
        type_id: u8,
        buffer: &mut BufferReader,
    ) -> Option<MonitorValuePayload> {
        match type_id {
            0 => buffer.read_byte().map(MonitorValuePayload::U8),
            1 => {
                let mut data = [0u8; 2];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Some(MonitorValuePayload::U16(u16::from_le_bytes(data)))
            }
            2 => {
                let mut data = [0u8; 4];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Some(MonitorValuePayload::U32(u32::from_le_bytes(data)))
            }
            3 => {
                let mut data = [0u8; 8];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Some(MonitorValuePayload::U64(u64::from_le_bytes(data)))
            }
            4 => buffer.read_byte().map(|b| MonitorValuePayload::I8(b as i8)),
            5 => {
                let mut data = [0u8; 2];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Some(MonitorValuePayload::I16(i16::from_le_bytes(data)))
            }
            6 => {
                let mut data = [0u8; 4];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Some(MonitorValuePayload::I32(i32::from_le_bytes(data)))
            }
            7 => {
                let mut data = [0u8; 8];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Some(MonitorValuePayload::I64(i64::from_le_bytes(data)))
            }
            _ => None,
        }
    }
}

pub trait MonitorValueType {
    fn to_payload(&self) -> MonitorValuePayload;
    fn get_monitor_value_type_id(&self) -> u8;
}

impl MonitorValueType for u8 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::U8(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        0
    }
}

impl MonitorValueType for u16 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::U16(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        1
    }
}

impl MonitorValueType for u32 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::U32(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        2
    }
}

impl MonitorValueType for u64 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::U64(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        3
    }
}

impl MonitorValueType for i8 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::I8(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        4
    }
}

impl MonitorValueType for i16 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::I16(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        5
    }
}

impl MonitorValueType for i32 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::I32(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        6
    }
}

impl MonitorValueType for i64 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::I64(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        7
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::{BufferReader, BufferWriter};

    #[test]
    fn test_monitor_value_payload_write_and_read() {
        let values: std::vec::Vec<MonitorValuePayload> = vec![
            42u8.to_payload(),
            65535u16.to_payload(),
            4294967295u32.to_payload(),
            18446744073709551615u64.to_payload(),
            (-42i8).to_payload(),
            (-32768i16).to_payload(),
            (-2147483648i32).to_payload(),
            (-9223372036854775808i64).to_payload(),
        ];

        for value in values {
            // Write the value to a buffer
            let mut writer = BufferWriter::new();
            value.write_bytes(&mut writer);
            let data = writer.as_slice();

            // Read the value back from the buffer
            let mut reader = BufferReader::new(&data);
            let type_id = value.type_id();
            let read_value = MonitorValuePayload::from_bytes(type_id, &mut reader).unwrap();

            assert_eq!(value, read_value);
        }
    }
}
