use core::sync::atomic::Ordering;

/// A registry for allocating unique numeric IDs by incrementing a counter. It uses u32 internally to avoid overhead because of 32-bit alignment.
/// Starting by 1 because 0 is reserved for "unregistered".
pub struct NumericRegistry {
    next_id: portable_atomic::AtomicUsize,
}

impl NumericRegistry {
    pub const fn new() -> Self {
        NumericRegistry {
            next_id: portable_atomic::AtomicUsize::new(1),
        }
    }

    /// Allocates and returns a new unique numeric ID
    pub fn allocate_new_id(&self) -> usize {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }
}
