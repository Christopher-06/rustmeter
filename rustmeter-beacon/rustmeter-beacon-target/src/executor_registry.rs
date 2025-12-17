// List of max. 8 unique executor IDs. Use Array or HashMap for simplicity?
//   ==> Choose Array since max. 8 IDs is small and fixed size (Cache friendly)

use core::sync::atomic::Ordering;

use arbitrary_int::u3;
use portable_atomic::AtomicU32;

pub struct ExecutorRegistry {
    // Number of registered executors
    slots: [AtomicU32; 8],
}

impl ExecutorRegistry {
    pub const fn new() -> Self {
        ExecutorRegistry {
            slots: [const { AtomicU32::new(0) }; 8],
        }
    }

    /// Iterate over registered executor IDs
    pub fn lookup_or_register(&self, executor_id: u32) -> Option<u3> {
        self.slots.iter().enumerate().find_map(|(i, slot)| {
            // 1. Check if executor ID is already registered (can be read without locking)
            let item_id = slot.load(Ordering::Relaxed);
            if item_id == executor_id {
                // Found existing executor ID
                return Some(u3::new(i as u8));
            }

            // 2. Try to register new executor ID
            if item_id == 0 {
                // Store must be blocking to avoid race conditions
                let res =
                    slot.compare_exchange(0, executor_id, Ordering::SeqCst, Ordering::Relaxed);

                match res {
                    Ok(_) => {
                        // Successfully registered new executor ID
                        return Some(u3::new(i as u8));
                    }
                    Err(actual) => {
                        // This Thread failed to register, check if another thread registered the same ID in the meantime or continue to next slot
                        if actual == executor_id {
                            // Another thread registered the same executor ID
                            return Some(u3::new(i as u8));
                        }
                    }
                }
            }

            // No slot available
            None
        })
    }
}
