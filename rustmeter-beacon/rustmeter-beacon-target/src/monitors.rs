use crate::numeric_registry::NumericRegistry;

pub static VALUE_MONITOR_REGISTRY: NumericRegistry = NumericRegistry::new();
pub static CODE_MONITOR_REGISTRY: NumericRegistry = NumericRegistry::new();

#[macro_export]
macro_rules! get_static_id_by_registry {
    ($registry:expr) => {{
        use rustmeter_beacon::_private::portable_atomic::{AtomicUsize, Ordering};
        static LOCAL_MONITOR_VALUE_ID: AtomicUsize = AtomicUsize::new(0);

        // Get or allocate monitor ID
        match LOCAL_MONITOR_VALUE_ID.load(Ordering::Relaxed) {
            0 => {
                // Allocate new ID
                let id = VALUE_MONITOR_REGISTRY.allocate_new_id();
                let res = LOCAL_MONITOR_VALUE_ID.compare_exchange(
                    0,
                    id,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );

                match res {
                    Ok(_) => {
                        // Successfully stored this ID as LOCAL_MONITOR_VALUE_ID
                        (id, true)
                    }
                    Err(actual) => {
                        // Another thread stored an ID in the meantime
                        (actual, false)
                    }
                }
            }
            id => (id, false),
        }
    }};
}

#[allow(unused_variables)]
pub fn defmt_trace_new_monitored_value(name: &str, local_id: usize) {
    #[cfg(feature = "defmt")]
    defmt::trace!(
        "Registered new monitored value: {} with id {}",
        name,
        local_id
    );
}

#[macro_export]
macro_rules! monitor_value {
    ($name:literal, $val:expr) => {
        // TODO: Check that val is numeric

        // Limit name length to 20 characters (BufferWriter is only 32 bytes and we need space for TimeDelta and other fields)
        const _: () = {
            core::assert!($name.len() <= 20, "Name of value to be monitored must be 20 characters or less");
        };

        use crate::monitors::{CODE_MONITOR_REGISTRY, VALUE_MONITOR_REGISTRY};
        use rustmeter_beacon::protocol::MonitorValueType;

        let (local_id, registered_newly) = get_static_id_by_registry!(VALUE_MONITOR_REGISTRY);

        // Send TypeDefinition event if newly registered
        if registered_newly {
            let payload = rustmeter_beacon::protocol::TypeDefinitionPayload::ValueMonitor {
                value_id: local_id as u8,
                type_id: $val.get_monitor_value_type_id(),
                name: $name,
            };
            rustmeter_beacon::tracing::write_tracing_event(rustmeter_beacon::protocol::EventPayload::TypeDefinition(payload));

            rustmeter_beacon::monitors::defmt_trace_new_monitored_value($name, local_id);
        }

        // Send MonitorValue event
        let payload = $val.to_payload();
        rustmeter_beacon::tracing::write_tracing_event(rustmeter_beacon::protocol::EventPayload::MonitorValue {
            value_id: local_id as u8,
            value: payload,
        });
    };
}

/// A guard that runs a function when dropped. Used in monitors to catch scope exits via return and other control flow statements.
pub struct DropGuard<F: FnOnce()> {
    drop_fn: Option<F>,
}

impl<F: FnOnce()> DropGuard<F> {
    pub fn new(drop_fn: F) -> Self {
        Self {
            drop_fn: Some(drop_fn),
        }
    }
}

impl<F: FnOnce()> Drop for DropGuard<F> {
    fn drop(&mut self) {
        if let Some(f) = self.drop_fn.take() {
            f();
        }
    }
}

#[allow(unused_variables)]
pub fn defmt_trace_new_scope(name: &str, local_id: usize) {
    #[cfg(feature = "defmt")]
    defmt::trace!(
        "Registered new scope monitor: {} with id {}",
        name,
        local_id
    );
}

#[macro_export]
macro_rules! monitor_scoped {
    ($name:literal, $body:block) => {{
        // Limit name length to 20 characters (BufferWriter is only 32 bytes and we need space for TimeDelta and other fields)
        const _: () = {
            core::assert!($name.len() <= 20, "Scope name must be 20 characters or less");
        };

        use rustmeter_beacon::monitors::{CODE_MONITOR_REGISTRY, VALUE_MONITOR_REGISTRY};
        use rustmeter_beacon::core_id::get_current_core_id;
        use rustmeter_beacon::get_static_id_by_registry;
        use rustmeter_beacon::tracing::write_tracing_event;

        let (local_id, registered_newly) = get_static_id_by_registry!(CODE_MONITOR_REGISTRY);
        let core_id = get_current_core_id();

        // Send TypeDefinition event if newly registered
        if registered_newly {
            let payload = rustmeter_beacon::protocol::TypeDefinitionPayload::ScopeMonitor {
                monitor_id: local_id as u8,
                name: $name,
            };
            write_tracing_event(rustmeter_beacon::protocol::EventPayload::TypeDefinition(payload));

            rustmeter_beacon::monitors::defmt_trace_new_scope($name, local_id);
        }

        // Create guard to signal end of scope
        let _guard = rustmeter_beacon::monitors::DropGuard::new(|| {
            let payload = match core_id {
                0 => rustmeter_beacon::protocol::EventPayload::MonitorEndCore0 {},
                1 => rustmeter_beacon::protocol::EventPayload::MonitorEndCore1 {},
                _ => rustmeter_beacon::core_id::unreachable_core_id(core_id),
            };

            write_tracing_event(payload);
        });

        // Send MonitorStart event (after guard-created to lower tracing impact on measured scope)
        let payload = match core_id {
            0 => rustmeter_beacon::protocol::EventPayload::MonitorStartCore0 {monitor_id: local_id as u8},
            1 => rustmeter_beacon::protocol::EventPayload::MonitorStartCore1 {monitor_id: local_id as u8},
            _ => rustmeter_beacon::core_id::unreachable_core_id(core_id),
        };
        write_tracing_event(payload);

        { $body }
    }};
}

// Call from proc-macro when a new function monitor is registered
#[allow(unused_variables)]
pub fn defmt_trace_new_function_monitor(name: &str, local_id: usize) {
    #[cfg(feature = "defmt")]
    defmt::trace!(
        "Registered new function monitor: {} with id {}",
        name,
        local_id
    );
}
