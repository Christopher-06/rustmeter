#![cfg(not(feature = "std"))]
#![no_std]

pub mod protocol;
pub mod time_delta;
pub mod tracing;

pub fn compressed_task_id(task_id: u32) -> u16 {
    // Step 1: Ignore alignment.
    // We discard the lowest 2 bits (4-byte alignment).
    let shifted = task_id >> 2;

    // Step 2: XOR-Fold for safety.
    // In case we have > 256KB memory or weird layout,
    let folded = (shifted ^ (shifted >> 16)) as u16;

    folded
}
