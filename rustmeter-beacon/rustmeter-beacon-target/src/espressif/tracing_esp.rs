//! Implement write_tracing_data for ESP32 targets from rustmeter-beacon-core. Uses an embassy Pipe
//! to buffer outgoing tracing data. Needs to be paired with a publisher in the main application to read
//! from the pipe and send it out
use core::{
    cell::{RefCell, UnsafeCell},
    sync::atomic::Ordering,
};

use critical_section::Mutex as CsMutex;
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    pipe::{Pipe, TryWriteError},
};
use rustmeter_beacon_core::{buffer::BufferWriter, protocol::EventPayload, time_delta::TimeDelta};

use crate::{
    NUM_CORES, espressif::local_critical_section::{enter_local_critical_section, exit_local_critical_section}, ringbuffer::{AtomicRingBuffer, SimpleRingBuffer}
};

const BUFFER_SIZE: usize = 4096;

static TRACE_BUFFERS: [PerCoreSync<AtomicRingBuffer<BUFFER_SIZE>>; NUM_CORES] = [
    PerCoreSync::new(AtomicRingBuffer::new()),
    #[cfg(any(feature = "esp32", feature = "esp32s3", feature = "esp32p4"))]
    PerCoreSync::new(AtomicRingBuffer::new()),
];

static NEW_DATA_SIGNAL: embassy_sync::signal::Signal<CriticalSectionRawMutex, ()> =
    embassy_sync::signal::Signal::new();

static DROPPED_EVENTS_COUNTER: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);

pub fn get_tracing_buffers_and_signaller() -> (
    &'static [PerCoreSync<AtomicRingBuffer<BUFFER_SIZE>>; NUM_CORES],
    &'static embassy_sync::signal::Signal<CriticalSectionRawMutex, ()>,
) {
    (&TRACE_BUFFERS, &NEW_DATA_SIGNAL)
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".iram1")]
fn write_tracing_data(data: &[u8]) {
    let core_id = crate::core_id::get_current_core_id() as usize;
    let idx = if core_id > 1 { 0 } else { core_id as usize };

    // Enter local critical section
    let prev_interrupt_state = enter_local_critical_section();

    // Write to Buffer
    let buf = unsafe { &mut *TRACE_BUFFERS[core_id].get() };
    if let Some(free) = buf.push_slice_fast(data) {
        if free < (buf.capacity() * 3 / 4)  {
            NEW_DATA_SIGNAL.signal(());
        }
    } else {
        DROPPED_EVENTS_COUNTER.fetch_add(1, Ordering::Relaxed);
    }

    // Exit local critical section
    exit_local_critical_section(prev_interrupt_state);
}

pub struct PerCoreSync<T>(UnsafeCell<T>);
unsafe impl<T> Sync for PerCoreSync<T> {}

impl<T> PerCoreSync<T> {
    const fn new(val: T) -> Self {
        Self(UnsafeCell::new(val))
    }
    #[inline(always)]
    pub fn get(&self) -> *mut T {
        self.0.get()
    }
}
