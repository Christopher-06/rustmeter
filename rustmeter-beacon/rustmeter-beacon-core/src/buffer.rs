use core::mem::MaybeUninit;

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

    /// Reads a single byte from the buffer. Returns None if end of buffer is reached.
    pub fn read_byte(&mut self) -> Option<u8> {
        if self.position >= self.buffer.len() {
            return None;
        }

        let byte = self.buffer[self.position];
        self.position += 1;
        Some(byte)
    }

    /// Reads a slice of bytes of the given length from the buffer. Returns None if not enough data is available.
    pub fn read_bytes(&mut self, length: usize) -> Option<&[u8]> {
        if self.position + length > self.buffer.len() {
            return None;
        }

        let bytes = &self.buffer[self.position..self.position + length];
        self.position += length;
        Some(bytes)
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

        assert_eq!(reader.read_byte(), Some(0x9A));
        assert_eq!(reader.read_bytes(2), Some(&[0xBC, 0xDE][..]));
        assert_eq!(reader.read_byte(), Some(0xF0));
        assert_eq!(reader.read_byte(), None);
    }
}
