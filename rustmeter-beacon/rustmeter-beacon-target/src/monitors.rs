use crate::numeric_registry::NumericRegistry;

pub static VALUE_MONITOR_REGISTRY: NumericRegistry = NumericRegistry::new();

#[macro_export]
macro_rules! get_static_id_by_registry {
    ($registry:expr) => {{
        use rustmeter_beacon::_private::portable_atomic::{AtomicUsize, Ordering};
        static LOCAL_MONITOR_VALUE_ID: AtomicUsize = AtomicUsize::new(0);

        // Get or allocate monitor ID
        match LOCAL_MONITOR_VALUE_ID.load(Ordering::Relaxed) {
            0 => {
                // Allocate new ID
                let id = $registry.allocate_new_id();
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
/// Macro to monitor a numeric value with Rustmeter Beacon.
///
/// ## Parameters
/// - $name: A string literal representing the name of the value to be monitored (max 20 characters).
/// - $val: The numeric value to be monitored. Must be convertible to rustmeter_beacon::protocol::MonitorValue (any primitive).
///
///
/// # Examples
/// ```rust,no_run
/// let temperature: f32 = read_temperature_sensor();
/// monitor_value!("temperature", temperature);
/// ```
macro_rules! monitor_value {
    ($name:literal, $val:expr) => {
        // TODO: Check that val is numeric

        // Limit name length to 20 characters (BufferWriter is only 32 bytes and we need space for TimeDelta and other fields)
        const _: () = {
            core::assert!($name.len() <= 20, "Name of value to be monitored must be 20 characters or less");
        };

        use rustmeter_beacon::monitors::VALUE_MONITOR_REGISTRY;
        use rustmeter_beacon::get_static_id_by_registry;
        use rustmeter_beacon::protocol::MonitorValuePayload;

        let (local_id, registered_newly) = get_static_id_by_registry!(VALUE_MONITOR_REGISTRY);

        // Send TypeDefinition event if newly registered
        if registered_newly {
            let payload = rustmeter_beacon::protocol::TypeDefinitionPayload::ValueMonitor {
                value_id: local_id as u8,
                name: $name,
            };
            rustmeter_beacon::tracing::write_tracing_event(rustmeter_beacon::protocol::EventPayload::TypeDefinition(payload));

            rustmeter_beacon::monitors::defmt_trace_new_monitored_value($name, local_id);
        }

        // Send MonitorValue event
        rustmeter_beacon::tracing::write_tracing_event(rustmeter_beacon::protocol::EventPayload::MonitorValue {
            value_id: local_id as u8,
            value: $val.into(),
        });
    };
}