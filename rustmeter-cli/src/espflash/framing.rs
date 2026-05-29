use crate::espflash::SerialFrameError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameMode {
    Normal = 0xFF,
    Panic = 0xFE,
}

/// Represents a serial frame received from the device.
/// First byte: Frame mode (0xFF = normal, 0xFE = panic)
/// Second byte: Core ID (bit 7) + Sequence ID (bits 0-6)
/// Third byte: Data length (N)
/// Next N bytes: Data payload
/// Last byte: Checksum (xor of all previous bytes)
#[derive(Debug)]
pub struct SerialFrame<'a>(&'a [u8]);

impl SerialFrame<'_> {
    /// Get frame mode. Returns None if frame is incomplete.
    pub fn mode(&self) -> Result<Option<FrameMode>, SerialFrameError> {
        match self.0.get(0) {
            Some(0xFF) => Ok(Some(FrameMode::Normal)),
            Some(0xFE) => Ok(Some(FrameMode::Panic)),
            Some(other) => Err(SerialFrameError::UnknownFrameMode(*other)), // this should never happen by design
            None => Ok(None),
        }
    }

    /// Get core ID. Returns None if frame is incomplete.
    pub fn core_id(&self) -> Option<u8> {
        self.0.get(1).map(|byte| (byte & 0x80) >> 7)
    }

    /// Get sequence ID. Returns None if frame is incomplete.
    pub fn seq_id(&self) -> Option<u8> {
        self.0.get(1).map(|byte| byte & 0x7F)
    }

    /// Get data payload length. Returns None if frame is incomplete.
    pub fn data_length(&self) -> Option<usize> {
        self.0.get(2).map(|byte| *byte as usize)
    }

    /// Get data payload slice. Returns None if frame is incomplete.
    pub fn data(&self) -> Option<&[u8]> {
        let length = self.data_length()?;
        if self.0.len() < 3 + length {
            return None;
        }
        Some(&self.0[3..(3 + length)])
    }

    /// Get total frame length. Returns None if frame is incomplete.
    pub fn len(&self) -> Option<usize> {
        let length = self.data_length()?;
        Some(3 + length + 1)
    }

    /// Get checksum byte. Returns None if frame is incomplete.
    pub fn checksum(&self) -> Option<u8> {
        let length = self.data_length()?;
        self.0.get(3 + length).copied()
    }

    /// Verify checksum of the frame. Returns None if frame is incomplete.
    pub fn verify_checksum(&self) -> Option<bool> {
        let length = self.data_length()?;
        let checksum = self.checksum()?;

        // xor checksum
        let calculated_checksum = calculate_checksum(&self.0[0..(3 + length)]);
        Some(calculated_checksum == checksum)
    }

    /// Convert data payload to boxed slice. Returns None if frame is incomplete.
    pub fn data_as_boxed(&self) -> Option<Box<[u8]>> {
        Some(self.data()?.to_vec().into_boxed_slice())
    }

    /// Check if the frame is complete.
    pub fn is_complete(&self) -> bool {
        self.data_length().is_some() && self.checksum().is_some()
    }
}

impl<'a> From<&'a [u8]> for SerialFrame<'a> {
    fn from(slice: &'a [u8]) -> Self {
        SerialFrame(slice)
    }
}

impl Default for SerialFrame<'_> {
    fn default() -> Self {
        SerialFrame(&[])
    }
}

pub struct FrameStream<'a>(&'a [u8]);

impl<'a> FrameStream<'a> {
    pub fn from_bytes(slice: &'a [u8]) -> Self {
        FrameStream(slice)
    }

    /// Find the first frame start in the stream for the given mode or any.
    /// Returns the position and the frame, or None if no start is found.
    pub fn get_first_frame_start(
        &self,
        mode: Option<FrameMode>,
    ) -> Option<(usize, SerialFrame<'a>)> {
        let pos = match mode {
            Some(mode) => self.0.iter().position(|b| *b == mode as u8), // find specific mode
            None => self
                .0
                .iter()
                .position(|b| *b == FrameMode::Normal as u8 || *b == FrameMode::Panic as u8),
        }?;

        Some((pos, SerialFrame::from(&self.0[pos..])))
    }
}

impl<'a> From<&'a [u8]> for FrameStream<'a> {
    fn from(slice: &'a [u8]) -> Self {
        FrameStream(slice)
    }
}

/// Calculate checksum for given data slice (xor of all bytes).
pub fn calculate_checksum(data: &[u8]) -> u8 {
    let mut checksum: u8 = 0;
    for &b in data {
        checksum ^= b;
    }

    checksum
}
