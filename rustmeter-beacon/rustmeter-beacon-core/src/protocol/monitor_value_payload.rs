/// Payloads for Monitor Value Events
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
    pub fn data_bytes(&self, buffer: &mut [u8]) -> usize {
        match self {
            MonitorValuePayload::U8(v) => {
                buffer[0] = *v;
                1
            }
            MonitorValuePayload::U16(v) => {
                buffer[0..2].copy_from_slice(&v.to_le_bytes());
                2
            }
            MonitorValuePayload::U32(v) => {
                buffer[0..4].copy_from_slice(&v.to_le_bytes());
                4
            }
            MonitorValuePayload::U64(v) => {
                buffer[0..8].copy_from_slice(&v.to_le_bytes());
                8
            }
            MonitorValuePayload::I8(v) => {
                buffer[0] = *v as u8;
                1
            }
            MonitorValuePayload::I16(v) => {
                buffer[0..2].copy_from_slice(&v.to_le_bytes());
                2
            }
            MonitorValuePayload::I32(v) => {
                buffer[0..4].copy_from_slice(&v.to_le_bytes());
                4
            }
            MonitorValuePayload::I64(v) => {
                buffer[0..8].copy_from_slice(&v.to_le_bytes());
                8
            }
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