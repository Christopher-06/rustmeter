use core::{iter::Chain, sync::atomic::Ordering};

use portable_atomic::AtomicUsize;

pub struct SimpleRingBuffer<const N: usize> {
    buf: [u8; N],
    write: usize,
    read: usize,
}

impl<const N: usize> SimpleRingBuffer<N> {
    pub const fn new() -> Self {
        Self {
            buf: [0; N],
            write: 0,
            read: 0,
        }
    }

    #[inline(always)]
    pub fn free(&self) -> usize {
        if self.write >= self.read {
            (N - 1) - (self.write - self.read)
        } else {
            self.read - self.write - 1
        }
    }

    /// Iterate over all currently stored bytes from start to end in correct order
    pub fn iter(&self) -> Chain<core::slice::Iter<'_, u8>, core::slice::Iter<'_, u8>> {
        if self.write > self.read {
            // Get slice directly (continuous)
            self.buf[self.read..self.write].iter().chain([].iter())
        } else {
            // Create an iterator that chains the two slices
            self.buf[self.read..]
                .iter()
                .chain(self.buf[0..self.write].iter())
                .into_iter()
        }
    }

    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        N - 1
    }

    #[inline(always)]
    pub fn push_slice(&mut self, data: &[u8]) -> Option<usize> {
        let free_space = self.free();
        if data.len() > free_space {
            return None;
        }

        let first_chunk = core::cmp::min(data.len(), N - self.write);
        self.buf[self.write..self.write + first_chunk].copy_from_slice(&data[..first_chunk]);

        if first_chunk < data.len() {
            let second_chunk = data.len() - first_chunk;
            self.buf[0..second_chunk].copy_from_slice(&data[first_chunk..]);
            self.write = second_chunk;
        } else {
            self.write = (self.write + first_chunk) % N;
        }

        Some(free_space - data.len())
    }

    #[inline(always)]
    pub fn push_slice_fast(&mut self, data: &[u8]) -> Option<usize> {
        let len = data.len();
        let free_space = self.free();
        if len > free_space {
            return None;
        }

        unsafe {
            let buf_ptr = self.buf.as_mut_ptr();
            let src_ptr = data.as_ptr();

            if self.write + len <= N {
                let dst = buf_ptr.add(self.write);
                core::ptr::copy_nonoverlapping(src_ptr, dst, len);
                self.write += len;
                if self.write == N {
                    self.write = 0;
                }
            } else {
                let first_chunk = N - self.write;
                let second_chunk = len - first_chunk;

                let dst1 = buf_ptr.add(self.write);
                core::ptr::copy_nonoverlapping(src_ptr, dst1, first_chunk);

                let dst2 = buf_ptr; // Offset 0
                let src2 = src_ptr.add(first_chunk);
                core::ptr::copy_nonoverlapping(src2, dst2, second_chunk);

                self.write = second_chunk;
            }
        }

        Some(free_space - len)
    }

    pub fn drain(&mut self, len: usize) {
        let available = if self.write >= self.read {
            self.write - self.read
        } else {
            N - self.read + self.write
        };

        let to_drain = core::cmp::min(len, available);
        self.read = (self.read + to_drain) % N;
    }

    pub fn pop_slice(&mut self, out: &mut [u8]) -> usize {
        let available = if self.write >= self.read {
            self.write - self.read
        } else {
            N - self.read + self.write
        };

        let to_read = core::cmp::min(out.len(), available);
        if to_read == 0 {
            return 0;
        }

        let first_chunk = core::cmp::min(to_read, N - self.read);
        out[..first_chunk].copy_from_slice(&self.buf[self.read..self.read + first_chunk]);

        if first_chunk < to_read {
            let second_chunk = to_read - first_chunk;
            out[first_chunk..].copy_from_slice(&self.buf[0..second_chunk]);
            self.read = second_chunk;
        } else {
            self.read = (self.read + first_chunk) % N;
        }
        to_read
    }
}

pub struct AtomicRingBuffer<const N: usize> {
    buf: [u8; N],
    write: AtomicUsize,
    read: AtomicUsize,
}

impl<const N: usize> AtomicRingBuffer<N> {
    pub const fn new() -> Self {
        Self {
            buf: [0; N],
            write: AtomicUsize::new(0),
            read: AtomicUsize::new(0),
        }
    }

    pub const fn capacity(&self) -> usize {
        N - 1
    }

    /// Returns the number of used bytes in the buffer
    pub fn len(&self) -> usize {
        let current_write = self.write.load(Ordering::Acquire);
        let current_read = self.read.load(Ordering::Relaxed);

        if current_write >= current_read {
            current_write - current_read
        } else {
            N - current_read + current_write
        }
    }

    #[inline(always)]
    pub fn push_slice_fast(&mut self, data: &[u8]) -> Option<usize> {
        let len = data.len();
        let current_read = self.read.load(Ordering::Acquire);
        let current_write = self.write.load(Ordering::Relaxed);

        // Calculate free space
        let free = if current_write >= current_read {
            (N - 1) - (current_write - current_read)
        } else {
            current_read - current_write - 1
        };

        if len > free {
            return None;
        }

        unsafe {
            let src = data.as_ptr();
            let dst_base = self.buf.as_mut_ptr();

            // 1. Copy data
            if current_write + len <= N {
                core::ptr::copy_nonoverlapping(src, dst_base.add(current_write), len);
            } else {
                let first_chunk = N - current_write;
                let second_chunk = len - first_chunk;
                core::ptr::copy_nonoverlapping(src, dst_base.add(current_write), first_chunk);
                core::ptr::copy_nonoverlapping(src.add(first_chunk), dst_base, second_chunk);
            }
        }

        // IMPORTANT: Memory Barrier
        // Release ensures that the consumer sees the NEW write index only after all data has been written.
        let new_write = (current_write + len) % N;
        self.write.store(new_write, Ordering::Release);

        // Return remaining free space
        Some(free - len)
    }

    /// Peek into the buffer without removing data. Important Memory Barriers must be ensured by the caller.
    pub fn peek_slice(&mut self, out: &mut [u8]) -> usize {
        let current_write = self.write.load(Ordering::Acquire);
        let current_read = self.read.load(Ordering::Relaxed);

        // Calculate available data
        let available = if current_write >= current_read {
            current_write - current_read
        } else {
            N - current_read + current_write
        };

        // Handle read length
        let to_read = core::cmp::min(out.len(), available);
        if to_read == 0 {
            return 0;
        }

        // Copy data
        unsafe {
            let dst = out.as_mut_ptr();
            let src_base = self.buf.as_ptr();

            if current_read + to_read <= N {
                core::ptr::copy_nonoverlapping(src_base.add(current_read), dst, to_read);
            } else {
                let first_chunk = N - current_read;
                let second_chunk = to_read - first_chunk;
                core::ptr::copy_nonoverlapping(src_base.add(current_read), dst, first_chunk);
                core::ptr::copy_nonoverlapping(src_base, dst.add(first_chunk), second_chunk);
            }
        }

        to_read
    }

    /// Drain data from the buffer. Important Memory Barriers must be ensured by the caller.
    pub fn drain(&mut self, len: usize) {
        let current_write = self.write.load(Ordering::Acquire);
        let current_read = self.read.load(Ordering::Relaxed);

        // Calculate available data
        let available = if current_write >= current_read {
            current_write - current_read
        } else {
            N - current_read + current_write
        };

        // Handle drain length
        let to_drain = core::cmp::min(len, available);
        let new_read = (current_read + to_drain) % N;
        self.read.store(new_read, Ordering::Release);
    }

    pub fn is_empty(&self) -> bool {
        self.write.load(Ordering::Relaxed) == self.read.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::SimpleRingBuffer;

    #[test]
    fn test_ring_buffer_push_pop() {
        let mut rb: SimpleRingBuffer<8> = SimpleRingBuffer::new();

        assert_eq!(rb.free(), 7);

        assert!(rb.push_slice(&[1, 2, 3]));
        assert_eq!(rb.free(), 4);

        let mut out = [0u8; 5];
        let read_bytes = rb.pop_slice(&mut out);
        assert_eq!(read_bytes, 3);
        assert_eq!(&out[..read_bytes], &[1, 2, 3]);
        assert_eq!(rb.free(), 7);

        assert!(rb.push_slice(&[4, 5, 6, 7, 8, 9]));
        assert_eq!(rb.free(), 1);

        let read_bytes = rb.pop_slice(&mut out);
        assert_eq!(read_bytes, 5);
        assert_eq!(&out[..read_bytes], &[4, 5, 6, 7, 8]);
        assert_eq!(rb.free(), 6);
    }
}
