use rustmeter_beacon_core::get_current_core_id;

#[unsafe(no_mangle)]
fn _embassy_trace_poll_start(executor_id: u32) {
    let core_id = get_current_core_id();
    defmt::info!(
        "@EVENT_EMBASSY_POLL_START(executor_id={}, core_id={})",
        executor_id,
        core_id
    );
}

#[unsafe(no_mangle)]
fn _embassy_trace_executor_idle(executor_id: u32) {
    let core_id = get_current_core_id();
    defmt::info!(
        "@EVENT_EMBASSY_EXECUTOR_IDLE(executor_id={}, core_id={})",
        executor_id,
        core_id
    );
}

#[unsafe(no_mangle)]
fn _embassy_trace_task_new(executor_id: u32, task_id: u32) {
    let core_id = get_current_core_id();
    defmt::info!(
        "@EVENT_EMBASSY_TASK_NEW(executor_id={}, core_id={}, task_id={})",
        executor_id,
        core_id,
        task_id
    );
}

#[unsafe(no_mangle)]
fn _embassy_trace_task_end(executor_id: u32, task_id: u32) {
    let core_id = get_current_core_id();
    defmt::info!(
        "@EVENT_EMBASSY_TASK_END(executor_id={}, core_id={}, task_id={})",
        executor_id,
        core_id,
        task_id
    );
}

#[unsafe(no_mangle)]
fn _embassy_trace_task_exec_begin(executor_id: u32, task_id: u32) {
    let core_id = get_current_core_id();
    defmt::info!(
        "@EVENT_EMBASSY_TASK_EXEC_BEGIN(executor_id={}, core_id={}, task_id={})",
        executor_id,
        core_id,
        task_id
    );
}

#[unsafe(no_mangle)]
fn _embassy_trace_task_exec_end(excutor_id: u32, task_id: u32) {
    let core_id = get_current_core_id();
    defmt::info!(
        "@EVENT_EMBASSY_TASK_EXEC_END(executor_id={}, core_id={}, task_id={})",
        excutor_id,
        core_id,
        task_id
    );
}

#[unsafe(no_mangle)]
fn _embassy_trace_task_ready_begin(executor_id: u32, task_id: u32) {
    let core_id = get_current_core_id();
    defmt::info!(
        "@EVENT_EMBASSY_TASK_READY_BEGIN(executor_id={}, core_id={}, task_id={})",
        executor_id,
        core_id,
        task_id
    );
}
