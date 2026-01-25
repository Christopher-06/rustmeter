#![allow(static_mut_refs)]
use core::sync::atomic::Ordering;

use portable_atomic::AtomicU32;

#[repr(C)]
pub struct RttControlBlock {
    pub id: [u8; 16],
    pub max_up_buffers: i32,
    pub max_down_buffers: i32,
    pub up_buffers: [RttChannel; 3],
    pub down_buffers: [RttChannel; 1],
}

#[repr(C)]
pub struct RttChannel {
    pub name: *const u8,
    pub buffer: *mut u8,
    pub size: u32,
    pub write_off: AtomicU32,
    pub read_off: AtomicU32,
    pub flags: u32,
}

// Flags
pub const RTT_MODE_NO_BLOCK_SKIP: u32 = 0;
pub const RTT_MODE_NO_BLOCK_TRIM: u32 = 1;
pub const RTT_MODE_BLOCK_IF_FULL: u32 = 2;

pub const BUFFER_SIZE: usize = 4096;

#[repr(align(4))]
struct AlignedBuffer([u8; BUFFER_SIZE]);
#[repr(align(4))]
struct AlignedBufferSmall([u8; 128]);

static mut CORE0_BUFFER: AlignedBuffer = AlignedBuffer([0u8; BUFFER_SIZE]);
static mut CORE1_BUFFER: AlignedBuffer = AlignedBuffer([0u8; BUFFER_SIZE]);
static mut DEFMT_BUFFER: AlignedBuffer = AlignedBuffer([0u8; BUFFER_SIZE]);
static mut REQ_BUFFER: AlignedBufferSmall = AlignedBufferSmall([0u8; 128]);

const NAME_C0: &[u8] = b"Core0\0";
const NAME_C1: &[u8] = b"Core1\0";
const NAME_C2: &[u8] = b"defmt\0";
const NAME_REQ: &[u8] = b"trace_requests\0";

#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".data")]
// #[unsafe(export_name = "_SEGGER_RTT")]
pub static mut _SEGGER_RTT: RttControlBlock = RttControlBlock {
    // Magischer String (16 Bytes)
    id: *b"SEGGER RTT\0\0\0\0\0\0",
    max_up_buffers: 3,
    max_down_buffers: 1,
    up_buffers: [
        // Channel 0 -> Core 0
        RttChannel {
            name: NAME_C0.as_ptr(),
            buffer: unsafe { CORE0_BUFFER.0.as_mut_ptr() }, // Compile-time pointer
            size: BUFFER_SIZE as u32,
            write_off: AtomicU32::new(0),
            read_off: AtomicU32::new(0),
            flags: RTT_MODE_NO_BLOCK_SKIP,
        },
        // Channel 1 -> Core 1
        RttChannel {
            name: NAME_C1.as_ptr(),
            buffer: unsafe { CORE1_BUFFER.0.as_mut_ptr() },
            size: BUFFER_SIZE as u32,
            write_off: AtomicU32::new(0),
            read_off: AtomicU32::new(0),
            flags: RTT_MODE_NO_BLOCK_SKIP,
        },
        // Channel 2 -> defmt
        RttChannel {
            name: NAME_C2.as_ptr(),
            buffer: unsafe { DEFMT_BUFFER.0.as_mut_ptr() },
            size: BUFFER_SIZE as u32,
            write_off: AtomicU32::new(0),
            read_off: AtomicU32::new(0),
            flags: RTT_MODE_NO_BLOCK_SKIP,
        },
    ],
    down_buffers: [
        // Channel 0 <- Trace Requests
        RttChannel {
            name: NAME_REQ.as_ptr(),
            buffer: unsafe { REQ_BUFFER.0.as_mut_ptr() },
            size: 128,
            write_off: AtomicU32::new(0),
            read_off: AtomicU32::new(0),
            flags: RTT_MODE_NO_BLOCK_TRIM,
        },
    ],
};

/// Writes data to the RTT buffer for the specified core.
#[inline(always)]
pub fn rtt_write_core(core_id: usize, data: &[u8]) -> Option<usize> {
    unsafe {
        let channel = &mut _SEGGER_RTT.up_buffers[core_id];

        let rd = channel.read_off.load(Ordering::Relaxed);
        let wr = channel.write_off.load(Ordering::Relaxed);

        // Calculate available space
        let size = channel.size;
        let mut avail = if rd > wr {
            rd - wr - 1
        } else {
            size - 1 - (wr - rd)
        };

        if avail < data.len() as u32 {
            // Not enough space
            return None;
        }

        let len_to_write = core::cmp::min(data.len() as u32, avail);
        if len_to_write == 0 {
            return None;
        }

        // 4. Write Data
        let buf_ptr = channel.buffer;
        let first_chunk = core::cmp::min(len_to_write, size - wr);

        // First Chunk
        core::ptr::copy_nonoverlapping(
            data.as_ptr(),
            buf_ptr.add(wr as usize),
            first_chunk as usize,
        );

        // Second Chunk (Wrap-around)
        if first_chunk < len_to_write {
            core::ptr::copy_nonoverlapping(
                data.as_ptr().add(first_chunk as usize),
                buf_ptr,
                (len_to_write - first_chunk) as usize,
            );
        }

        //  Update Write Offset
        let mut new_wr = wr + len_to_write;
        if new_wr >= size {
            new_wr -= size;
        }

        // IMPORTANT: Release Ordering!
        // The debugger must not see the new 'write_off' until
        // the data is physically in memory.
        channel.write_off.store(new_wr, Ordering::Release);

        Some(len_to_write as usize)
    }
}

/// Reads data from the RTT buffer. Returns the number of bytes read.
#[inline(always)]
pub fn rtt_read_down_channel(buffer: &mut [u8]) -> usize {
    unsafe {
        let channel = &mut _SEGGER_RTT.down_buffers[0];

        // Load Offsets
        let wr = channel.write_off.load(Ordering::Acquire);
        let rd = channel.read_off.load(Ordering::Relaxed);
        if wr == rd {
            return 0;
        }

        // Calculate available data
        let size = channel.size;
        let count = if wr >= rd { wr - rd } else { (size - rd) + wr };

        // Determine how much to read
        let len_to_read = core::cmp::min(count as usize, buffer.len());
        if len_to_read == 0 {
            return 0;
        }

        let buf_ptr = channel.buffer;
        let rd_idx = rd as usize;

        //  Copy first part
        let first_chunk = core::cmp::min(len_to_read, (size - rd) as usize);
        core::ptr::copy_nonoverlapping(buf_ptr.add(rd_idx), buffer.as_mut_ptr(), first_chunk);

        // Copy second part (wrap-around)
        if first_chunk < len_to_read {
            core::ptr::copy_nonoverlapping(
                buf_ptr,
                buffer.as_mut_ptr().add(first_chunk),
                len_to_read - first_chunk,
            );
        }

        // Update Read Offset
        let mut new_rd = rd + len_to_read as u32;
        if new_rd >= size {
            new_rd -= size;
        }

        // Signal to host
        channel.read_off.store(new_rd, Ordering::Release);
        len_to_read
    }
}
