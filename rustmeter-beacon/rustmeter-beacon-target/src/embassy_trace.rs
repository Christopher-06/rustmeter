//! Embassy tracing integration for Rustmeter Beacon
//! Endpoints for Embassy's tracing hooks to send events to Rustmeter Beacon

use arbitrary_int::traits::Integer;
use rustmeter_beacon_core::{
    compressed_task_id,
    protocol::{
        EventPayload, TypeDefinitionPayload,
        raw_writers::{
            write_embassy_executor_idle, write_embassy_executor_poll_start,
            write_embassy_task_exec_begin, write_embassy_task_exec_end, write_embassy_task_ready,
        },
    },
    tracing::write_tracing_event,
};

use crate::{
    core_id::{get_current_core_id, unreachable_core_id},
    executor_registry::ExecutorRegistry,
    timing::get_tracing_time_us,
};

// Registry to map long executor IDs to short IDs
static EXECUTOR_REGISTRY: ExecutorRegistry = ExecutorRegistry::new();

#[unsafe(no_mangle)]
fn _embassy_trace_poll_start(executor_id: u32) {
    let executor_id = EXECUTOR_REGISTRY.lookup_or_register(executor_id).unwrap();

    write_embassy_executor_poll_start(executor_id);
}

#[unsafe(no_mangle)]
fn _embassy_trace_executor_idle(executor_id: u32) {
    let executor_id = EXECUTOR_REGISTRY.lookup_or_register(executor_id).unwrap();

    write_embassy_executor_idle(executor_id);
}

#[unsafe(no_mangle)]
fn _embassy_trace_task_new(executor_id: u32, task_id: u32) {
    critical_section::with(|_| {
        let payload = EventPayload::TypeDefinition(TypeDefinitionPayload::EmbassyTaskCreated {
            task_id: task_id,
            executor_id_long: executor_id,
            executor_id_short: EXECUTOR_REGISTRY.lookup_or_register(executor_id).unwrap(),
        });

        write_tracing_event(payload);
    });
}

#[unsafe(no_mangle)]
fn _embassy_trace_task_end(executor_id: u32, task_id: u32) {
    critical_section::with(|_| {
        let payload = EventPayload::TypeDefinition(TypeDefinitionPayload::EmbassyTaskEnded {
            task_id: task_id,
            executor_id_long: executor_id,
            executor_id_short: EXECUTOR_REGISTRY.lookup_or_register(executor_id).unwrap(),
        });

        write_tracing_event(payload);
    });
}

#[unsafe(no_mangle)]
fn _embassy_trace_task_exec_begin(executor_id: u32, task_id: u32) {
    let core_id = get_current_core_id();
    let executor_id = EXECUTOR_REGISTRY.lookup_or_register(executor_id).unwrap();

    write_embassy_task_exec_begin(core_id, compressed_task_id(task_id), executor_id);
}

#[unsafe(no_mangle)]
fn _embassy_trace_task_exec_end(executor_id: u32, _task_id: u32) {
    let core_id = get_current_core_id();
    let executor_id = EXECUTOR_REGISTRY.lookup_or_register(executor_id).unwrap();

    write_embassy_task_exec_end(core_id, executor_id);
}

#[unsafe(no_mangle)]
#[cfg_attr(
    any(
        feature = "esp32",
        feature = "esp32c2",
        feature = "esp32c3",
        feature = "esp32c6",
        feature = "esp32h2",
        feature = "esp32s2",
        feature = "esp32s3"
    ),
    esp_hal::ram
)]
fn _embassy_trace_task_ready_begin(executor_id: u32, task_id: u32) {
    use crate::timing::get_tracing_raw_ticks;
    
    let executor_id = EXECUTOR_REGISTRY.lookup_or_register(executor_id).unwrap();
    let start_time = get_tracing_raw_ticks();

    write_embassy_task_ready(compressed_task_id(task_id), executor_id);

    let end_time = get_tracing_raw_ticks();
    // defmt::info!(
    //     "Task ready begin: task_id={}, executor_id={}, time_taken={}us",
    //     task_id,
    //     executor_id.as_u8(),
    //     (end_time - start_time)
    // );
}
