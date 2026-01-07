
#[cfg(test)]
pub mod test_mocks {
    use core::cell::RefCell;

    // 1. Thread-Local Storage für die Hooks
    // Hier speichern wir Closures, die das eigentliche Verhalten pro Test definieren.
    thread_local! {
        static WRITE_HOOK: RefCell<Option<Box<dyn FnMut(&[u8])>>> = RefCell::new(None);
        static TIME_HOOK: RefCell<Option<Box<dyn FnMut() -> u32>>> = RefCell::new(None);
    }

    // 2. Die EINE globale Definition der no_mangle Funktionen für Tests
    #[unsafe(no_mangle)]
    fn write_tracing_data(data: &[u8]) {
        WRITE_HOOK.with(|hook| {
            if let Some(h) = hook.borrow_mut().as_mut() {
                h(data);
            } else {
                // Fallback oder Panic, falls kein Mock gesetzt ist
                println!(
                    "Warning: write_tracing_data called without mock! Data len: {}",
                    data.len()
                );
            }
        });
    }

    #[unsafe(no_mangle)]
    fn get_tracing_time_us() -> u32 {
        TIME_HOOK.with(|hook| {
            if let Some(h) = hook.borrow_mut().as_mut() {
                h()
            } else {
                0 // Default Zeit
            }
        })
    }

    // 3. Helper-Funktion für deine Tests
    // Diese Funktion setzt die Mocks für den aktuellen Scope (den Test)
    pub fn with_mocks<W, T, F>(mut write_fn: W, mut time_fn: T, test_body: F)
    where
        W: FnMut(&[u8]) + 'static,
        T: FnMut() -> u32 + 'static,
        F: FnOnce(),
    {
        // Mocks setzen
        WRITE_HOOK.with(|h| *h.borrow_mut() = Some(Box::new(write_fn)));
        TIME_HOOK.with(|h| *h.borrow_mut() = Some(Box::new(time_fn)));

        // Test ausführen
        test_body();

        // Aufräumen (wichtig, damit state nicht in andere Tests leakt)
        WRITE_HOOK.with(|h| *h.borrow_mut() = None);
        TIME_HOOK.with(|h| *h.borrow_mut() = None);
    }
}
