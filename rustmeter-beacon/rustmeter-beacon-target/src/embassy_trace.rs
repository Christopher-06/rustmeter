//! Embassy tracing integration for Rustmeter Beacon
//! Endpoints for Embassy's tracing hooks to send events to Rustmeter Beacon

use rustmeter_beacon_core::{
    compressed_task_id,
    protocol::{EventPayload, TypeDefinitionPayload},
    tracing::write_tracing_event,
};

use crate::{
    core_id::{get_current_core_id, unreachable_core_id},
    executor_registry::ExecutorRegistry,
};

// Registry to map long executor IDs to short IDs
static EXECUTOR_REGISTRY: ExecutorRegistry = ExecutorRegistry::new();

#[unsafe(no_mangle)]
fn _embassy_trace_poll_start(executor_id: u32) {
    let payload = EventPayload::EmbassyExecutorPollStart {
        executor_id: EXECUTOR_REGISTRY.lookup_or_register(executor_id).unwrap(),
    };

    write_tracing_event(payload);
}

#[unsafe(no_mangle)]
fn _embassy_trace_executor_idle(executor_id: u32) {
    let payload = EventPayload::EmbassyExecutorIdle {
        executor_id: EXECUTOR_REGISTRY.lookup_or_register(executor_id).unwrap(),
    };

    write_tracing_event(payload);
}

#[unsafe(no_mangle)]
fn _embassy_trace_task_new(executor_id: u32, task_id: u32) {
    let payload = EventPayload::TypeDefinition(TypeDefinitionPayload::EmbassyTaskCreated {
        task_id: task_id,
        executor_id_long: executor_id,
        executor_id_short: EXECUTOR_REGISTRY.lookup_or_register(executor_id).unwrap(),
    });

    write_tracing_event(payload);
}

#[unsafe(no_mangle)]
fn _embassy_trace_task_end(executor_id: u32, task_id: u32) {
    let payload = EventPayload::TypeDefinition(TypeDefinitionPayload::EmbassyTaskEnded {
        task_id: task_id,
        executor_id_long: executor_id,
        executor_id_short: EXECUTOR_REGISTRY.lookup_or_register(executor_id).unwrap(),
    });

    write_tracing_event(payload);
}

#[unsafe(no_mangle)]
fn _embassy_trace_task_exec_begin(_executor_id: u32, task_id: u32) {
    let core_id = get_current_core_id();

    let payload = match core_id {
        0 => EventPayload::EmbassyTaskExecBeginCore0 {
            task_id: compressed_task_id(task_id),
        },
        1 => EventPayload::EmbassyTaskExecBeginCore1 {
            task_id: compressed_task_id(task_id),
        },
        c => unreachable_core_id(c),
    };

    write_tracing_event(payload);
}

#[unsafe(no_mangle)]
fn _embassy_trace_task_exec_end(executor_id: u32, _task_id: u32) {
    let core_id = get_current_core_id();

    let payload = match core_id {
        0 => EventPayload::EmbassyTaskExecEndCore0 {
            executor_id: EXECUTOR_REGISTRY.lookup_or_register(executor_id).unwrap(),
        },
        1 => EventPayload::EmbassyTaskExecEndCore1 {
            executor_id: EXECUTOR_REGISTRY.lookup_or_register(executor_id).unwrap(),
        },
        c => unreachable_core_id(c),
    };

    write_tracing_event(payload);
}

#[unsafe(no_mangle)]
fn _embassy_trace_task_ready_begin(executor_id: u32, task_id: u32) {
    let payload = EventPayload::EmbassyTaskReady {
        task_id: compressed_task_id(task_id),
        executor_id: EXECUTOR_REGISTRY.lookup_or_register(executor_id).unwrap(),
    };

    write_tracing_event(payload);
}
