use core::mem::MaybeUninit;

use crate::{tracing::ReadTracingError, varint::VarIntWritable};

/// Internal buffer writer for tracing events using a fixed-size buffer with uninitialized memory for efficiency
pub struct BufferWriter {
    buffer: [MaybeUninit<u8>; 32],
    position: usize,
}

impl BufferWriter {
    pub fn new() -> Self {
        BufferWriter {
            buffer: [MaybeUninit::uninit(); 32],
            position: 0,
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        self.buffer[self.position] = MaybeUninit::new(byte);
        self.position += 1;
    }

    /// Writes a slice of bytes into the buffer. Assumes there is enough space
    pub fn write_bytes(&mut self, data: &[u8]) {
        let len = data.len();
        self.buffer[self.position..self.position + len]
            .copy_from_slice(unsafe { core::mem::transmute::<&[u8], &[MaybeUninit<u8>]>(data) });
        self.position += len;
    }

    /// Writes a little-endian u16 into the buffer
    pub fn write_u16(&mut self, value: u16) {
        let bytes = value.to_le_bytes();
        self.write_bytes(&bytes);
    }

    /// Writes a little-endian u32 into the buffer
    pub fn write_u32(&mut self, value: u32) {
        let bytes = value.to_le_bytes();
        self.write_bytes(&bytes);
    }

    /// Writes a little-endian u64 into the buffer
    pub fn write_u64(&mut self, value: u64) {
        let bytes = value.to_le_bytes();
        self.write_bytes(&bytes);
    }

    /// Writes a generic integer using LEB128 (VarInt) encoding
    #[inline]
    pub fn write_varint<T: VarIntWritable>(&mut self, mut value: T) {
        loop {
            let mut byte = value.low_7_bits();

            value.shr_7();
            if !value.is_zero() {
                // More bytes to come, set continuation bit
                byte |= 0x80;
                self.write_byte(byte);
            } else {
                // Last byte, no continuation bit
                self.write_byte(byte);
                break;
            }
        }
    }

    /// Returns the already written data as a slice
    pub fn as_slice(&self) -> &[u8] {
        &unsafe { core::mem::transmute::<&[MaybeUninit<u8>], &[u8]>(&self.buffer[..self.position]) }
    }

    pub fn len(&self) -> usize {
        self.position
    }
}

/// Simple buffer reader for reading bytes from a slice
pub struct BufferReader<'a> {
    buffer: &'a [u8],
    position: usize,
}

impl<'a> BufferReader<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        BufferReader {
            buffer,
            position: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Reads a single byte from the buffer. Returns None if end of buffer is reached.
    pub fn read_byte(&mut self) -> Result<u8, ReadTracingError> {
        if self.position >= self.buffer.len() {
            return Err(ReadTracingError::InsufficientData);
        }

        let byte = self.buffer[self.position];
        self.position += 1;
        Ok(byte)
    }

    /// Reads a slice of bytes of the given length from the buffer. Returns None if not enough data is available.
    pub fn read_bytes(&mut self, length: usize) -> Result<&[u8], ReadTracingError> {
        if self.position + length > self.buffer.len() {
            return Err(ReadTracingError::InsufficientData);
        }

        let bytes = &self.buffer[self.position..self.position + length];
        self.position += length;
        Ok(bytes)
    }

    /// Reads a LEB128 (VarInt) encoded u64.
    pub fn read_varint(&mut self) -> Result<u64, ReadTracingError> {
        let mut result = 0u64;
        let mut shift = 0;

        loop {
            let byte = self.read_byte()?;

            // Mask out the continuation bit and shift into result
            result |= ((byte & 0x7F) as u64) << shift;

            // If continuation bit is not set, we are done
            if (byte & 0x80) == 0 {
                return Ok(result);
            }

            shift += 7;

            // Protection against overflow / bad data (max 10 bytes for u64)
            if shift >= 70 {
                return Err(ReadTracingError::VarIntOverflow);
            }
        }
    }

    /// Reads a little-endian u16 from the buffer
    pub fn read_u16(&mut self) -> Result<u16, ReadTracingError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
    }

    /// Reads a little-endian u32 from the buffer
    pub fn read_u32(&mut self) -> Result<u32, ReadTracingError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    /// Reads a little-endian u64 from the buffer
    pub fn read_u64(&mut self) -> Result<u64, ReadTracingError> {
        let bytes = self.read_bytes(8)?;
        Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub fn get_position(&self) -> usize {
        self.position
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_writer() {
        let mut writer = BufferWriter::new();
        writer.write_byte(0x12);
        writer.write_bytes(&[0x34, 0x56, 0x78]);

        let written = writer.as_slice();
        assert_eq!(written, &[0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_buffer_reader() {
        let data = [0x9A, 0xBC, 0xDE, 0xF0];
        let mut reader = BufferReader::new(&data);

        assert_eq!(reader.read_byte(), Ok(0x9A));
        assert_eq!(reader.read_bytes(2), Ok(&[0xBC, 0xDE][..]));
        assert_eq!(reader.read_byte(), Ok(0xF0));
        assert_eq!(reader.read_byte(), Err(ReadTracingError::InsufficientData));
    }

    #[test]
    fn test_buffer_write_and_read() {
        // Write data
        let mut writer = BufferWriter::new();
        writer.write_bytes(&[0x01, 0x02, 0x03, 0x04, 0x05]);
        writer.write_varint(1u8);
        writer.write_varint(300u16);
        writer.write_varint(70000u32);
        writer.write_varint(123456789u64);
        writer.write_bytes(&[0x01, 0x02, 0x03, 0x04, 0x05]);

        // Read data
        let data = writer.as_slice();
        let mut reader = BufferReader::new(data);
        assert_eq!(
            reader.read_bytes(5),
            Ok(&[0x01, 0x02, 0x03, 0x04, 0x05][..])
        );
        assert_eq!(reader.read_varint(), Ok(1u64));
        assert_eq!(reader.read_varint(), Ok(300u64));
        assert_eq!(reader.read_varint(), Ok(70000u64));
        assert_eq!(reader.read_varint(), Ok(123456789u64));
        assert_eq!(
            reader.read_bytes(5),
            Ok(&[0x01, 0x02, 0x03, 0x04, 0x05][..])
        );
    }
}
