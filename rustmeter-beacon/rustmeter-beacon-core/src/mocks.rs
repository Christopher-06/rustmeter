#[cfg(test)]
pub mod test_mocks {
    use crate::{buffer::BufferReader, protocol::EventPayload, tracing::read_tracing_event};
    use core::cell::RefCell;

    thread_local! {
        static WRITE_HOOK: RefCell<Option<Box<dyn FnMut(&[u8])>>> = RefCell::new(None);
        static RAW_TIME_HOOK: RefCell<Option<Box<dyn FnMut() -> u32>>> = RefCell::new(None);
        static SYS_TIME_HOOK: RefCell<Option<Box<dyn FnMut() -> u64>>> = RefCell::new(None);
    }

    #[unsafe(no_mangle)]
    fn write_tracing_data(data: &[u8]) {
        WRITE_HOOK.with(|hook| {
            if let Some(h) = hook.borrow_mut().as_mut() {
                return h(data);
            }

            // Fallback
            println!(
                "Warning: write_tracing_data called without mock! Data len: {}",
                data.len()
            );
        });
    }

    #[unsafe(no_mangle)]
    fn get_tracing_raw_ticks() -> u32 {
        RAW_TIME_HOOK.with(|hook| {
            if let Some(h) = hook.borrow_mut().as_mut() {
                return h();
            }

            // Fallback
            println!("Warning: get_tracing_raw_ticks called without mock! Returning 0.");
            0
        })
    }

    #[unsafe(no_mangle)]
    fn get_system_time_us() -> u64 {
        SYS_TIME_HOOK.with(|hook| {
            if let Some(h) = hook.borrow_mut().as_mut() {
                return h();
            }

            // Fallback
            println!("Warning: get_system_time_us called without mock! Returning 0.");
            0
        })
    }

    // Method to inject mocks for a test that will be automatically removed after the test
    pub fn with_mocks<W, T, S, F>(write_fn: W, raw_time_fn: T, sys_time_fn: S, test_body: F)
    where
        W: FnMut(&[u8]) + 'static,
        T: FnMut() -> u32 + 'static,
        S: FnMut() -> u64 + 'static,
        F: FnOnce(),
    {
        // Mocks
        WRITE_HOOK.with(|h| *h.borrow_mut() = Some(Box::new(write_fn)));
        RAW_TIME_HOOK.with(|h| *h.borrow_mut() = Some(Box::new(raw_time_fn)));
        SYS_TIME_HOOK.with(|h| *h.borrow_mut() = Some(Box::new(sys_time_fn)));

        // Run the test body
        test_body();

        // Clean up
        WRITE_HOOK.with(|h| *h.borrow_mut() = None);
        RAW_TIME_HOOK.with(|h| *h.borrow_mut() = None);
        SYS_TIME_HOOK.with(|h| *h.borrow_mut() = None);
    }

    /// Simple mock time provider that returns a fixed timestamp (123_456_789)
    pub fn mock_time_provider() -> u32 {
        123_456_789
    }

    /// Mock trace writer that checks the written data to match the expected event payload and
    /// timestamp (123_456_789) from the mock time provider.
    pub fn mock_trace_writer(expected: EventPayload) -> impl Fn(&[u8]) {
        move |data: &[u8]| {
            let mut buffer = BufferReader::new(data);
            let (timestamp, event) =
                read_tracing_event(&mut buffer).expect("Failed to read tracing event");

            assert_eq!(event, expected);
            assert_eq!(timestamp.delta(), 123_456_789);
        }
    }
}
