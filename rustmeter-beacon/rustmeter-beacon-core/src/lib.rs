#![no_std]

mod core_id;
mod monitor_scoped;
pub use crate::core_id::*;

#[macro_export]
/// Logs an event metric with a name and value via defmt.
macro_rules! event_metric {
    ($name:literal, $val:expr) => {
        // TODO: Check that val is numeric
        // TODO: Check that name is a string literal without any special characters

        defmt::info!(
            "@EVENT_METRIC(name={=istr},value={},core_id={})",
            defmt::intern!($name),
            $val,
            rustmeter_beacon::get_current_core_id()
        );
    };
}
