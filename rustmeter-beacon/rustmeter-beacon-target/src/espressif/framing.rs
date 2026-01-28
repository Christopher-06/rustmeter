use crate::ringbuffer::AtomicRingBuffer;

pub enum FrameMode {
    Normal = 0xFF,
    Panic = 0xFE,
}

/// Create a data frame with framing heading and checksum.
/// Peeks data from inbuf and writes framed data into framebuf.
/// Returns (total_frame_length, data_length). data_length is the number of bytes peeked from inbuf.
pub fn create_dataframe<const N: usize, const U: usize>(
    mode: FrameMode,
    core_id: u8,
    inbuf: &mut AtomicRingBuffer<N>,
    framebuf: &mut [u8; U],
    seq_id: u8,
) -> (usize, usize) {
    assert!(U >= 4, "Frame buffer too small for framing overhead");

    // try to read some data
    let len = inbuf.peek_slice(&mut framebuf[3..(U - 1)]);
    if len == 0 {
        return (0, 0);
    }

    framebuf[0] = mode as u8;
    framebuf[1] = (core_id << 7) | seq_id; // Core ID + Sequence ID
    framebuf[2] = len as u8; // length byte
    framebuf[len + 3] = calculate_checksum(&framebuf[0..(3 + len)]); // checksum byte

    (3 + len + 1, len)
}

/// Calculate XOR checksum
fn calculate_checksum(data: &[u8]) -> u8 {
    let mut checksum: u8 = 0;
    for &b in data {
        checksum ^= b;
    }
    checksum
}
