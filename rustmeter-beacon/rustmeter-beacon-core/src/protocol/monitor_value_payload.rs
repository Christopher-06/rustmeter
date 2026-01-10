use arbitrary_int::{traits::Integer, u3};

use crate::{
    buffer::{BufferReader, BufferWriter},
    varint::ZigZag,
};

/// Payloads for Monitor Value Events
#[derive(Debug, Clone, PartialEq)]
pub enum MonitorValuePayload {
    Unsigned(u64),
    Signed(i64),
    Float32(f32),
    Float64(f64),
}

impl MonitorValuePayload {
    pub const fn type_id(&self) -> u3 {
        // cannot be u3 because one byte must be used for length encoding
        match self {
            MonitorValuePayload::Unsigned(_) => u3::new(0),
            MonitorValuePayload::Signed(_) => u3::new(1),
            MonitorValuePayload::Float32(_) => u3::new(2),
            MonitorValuePayload::Float64(_) => u3::new(3),
        }
    }

    /// Write the payload data into the provided buffer.
    /// Returns the number of data bytes written into the provided buffer. Assumes the buffer is large enough.
    #[inline(always)]
    pub(crate) fn write_bytes(&self, buffer: &mut BufferWriter) {
        match self {
            MonitorValuePayload::Unsigned(v) => {
                buffer.write_varint(*v);
            }
            MonitorValuePayload::Signed(v) => {
                let enc = v.zigzag_encode();
                buffer.write_varint(enc);
            }
            MonitorValuePayload::Float32(v) => {
                buffer.write_bytes(&v.to_le_bytes());
            }
            MonitorValuePayload::Float64(v) => {
                buffer.write_bytes(&v.to_le_bytes());
            }
        };
    }

    /// Reads a MonitorValuePayload from the provided buffer based on the given type ID.
    pub(crate) fn from_bytes(
        type_id: u3,
        buffer: &mut BufferReader,
    ) -> Option<MonitorValuePayload> {
        match type_id.as_u8() {
            0 => {
                let value = buffer.read_varint()?;
                Some(MonitorValuePayload::Unsigned(value))
            }
            1 => {
                let enc = buffer.read_varint()?;
                let value = i64::zigzag_decode(enc);
                Some(MonitorValuePayload::Signed(value))
            }
            2 => {
                let bytes = buffer.read_bytes(4)?;
                let value = f32::from_le_bytes(bytes.try_into().ok()?);
                Some(MonitorValuePayload::Float32(value))
            }
            3 => {
                let bytes = buffer.read_bytes(8)?;
                let value = f64::from_le_bytes(bytes.try_into().ok()?);
                Some(MonitorValuePayload::Float64(value))
            }
            _ => None,
        }
    }

    pub fn as_f64(&self) -> f64 {
        match self {
            MonitorValuePayload::Unsigned(v) => *v as f64,
            MonitorValuePayload::Signed(v) => *v as f64,
            MonitorValuePayload::Float32(v) => *v as f64,
            MonitorValuePayload::Float64(v) => *v,
        }
    }
}

// Conversions from primitive types to MonitorValuePayload
macro_rules! impl_from_primitive {
    ($($t:ty => $variant:ident),*) => {
        $(
            impl From<$t> for MonitorValuePayload {
                #[inline(always)]
                fn from(value: $t) -> Self {
                    MonitorValuePayload::$variant(value as _)
                }
            }
        )*
    };
}
impl_from_primitive!(
    u8 => Unsigned,
    u16 => Unsigned,
    u32 => Unsigned,
    u64 => Unsigned,
    i8 => Signed,
    i16 => Signed,
    i32 => Signed,
    i64 => Signed,
    f32 => Float32,
    f64 => Float64
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::{BufferReader, BufferWriter};

    #[test]
    fn test_monitor_value_payload_write_and_read() {
        let values: std::vec::Vec<MonitorValuePayload> = vec![
            42u8.into(),
            65535u16.into(),
            4294967295u32.into(),
            18446744073709551615u64.into(),
            (-42i8).into(),
            (-32768i16).into(),
            (-2147483648i32).into(),
            (-9223372036854775808i64).into(),
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
