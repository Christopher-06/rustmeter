//! ## Task Tracing lifecycle
//!
//! ```text
//! ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─
//!        │(1)                                            │
//! │      │
//!   ╔════▼════╗ (2) ┌─────────┐ (3) ┌─────────┐          │
//! │ ║ SPAWNED ║────▶│ WAITING │────▶│ RUNNING │
//!   ╚═════════╝     └─────────┘     └─────────┘          │
//! │                 ▲         ▲     │    │    │
//!                   │           (4)      │    │(6)       │
//! │                 │(7)      └ ─ ─ ┘    │    │
//!                   │                    │    │          │
//! │             ┌──────┐             (5) │    │  ┌─────┐
//!               │ IDLE │◀────────────────┘    └─▶│ END │ │
//! │             └──────┘                         └─────┘
//!   ┌──────────────────────┐                             │
//! └ ┤ Task Trace Lifecycle │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─
//!   └──────────────────────┘
//! ```
//!
//! 1. A task is spawned, `_embassy_trace_task_new` is called
//! 2. A task is enqueued for the first time, `_embassy_trace_task_ready_begin` is called
//! 3. A task is polled, `_embassy_trace_task_exec_begin` is called
//! 4. WHILE a task is polled, the task is re-awoken, and `_embassy_trace_task_ready_begin` is
//!      called. The task does not IMMEDIATELY move state, until polling is complete and the
//!      RUNNING state is existed. `_embassy_trace_task_exec_end` is called when polling is
//!      complete, marking the transition to WAITING
//! 5. Polling is complete, `_embassy_trace_task_exec_end` is called
//! 6. The task has completed, and `_embassy_trace_task_end` is called
//! 7. A task is awoken, `_embassy_trace_task_ready_begin` is called
//!
//! (taken from embassy-executor/src/raw/trace.rs)
//!
//! We added the Preempted state to indicate that a task was preempted by another executor task with higher priority (Interrupt context).

use std::collections::HashMap;

use crossbeam::channel::Sender;

use crate::{
    elf_file::FirmwareAddressMap,
    perfetto_backend::trace_event::TracingEvent,
    time::EmbassyTime,
    tracing::log_event::{LogEvent, LogEventType},
};

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum TaskTraceState {
    Spawned,
    Waiting,
    Running,
    /// Task was preempted by another executor (task with different executor ID on the same core)
    Preempted {
        by_executor_id: u32,
    },
    Idle,
    Ended,
}

impl TaskTraceState {
    pub fn to_string(&self) -> String {
        match self {
            TaskTraceState::Spawned => "Spawned".to_string(),
            TaskTraceState::Waiting => "Waiting".to_string(),
            TaskTraceState::Running => "Running".to_string(),
            TaskTraceState::Preempted { by_executor_id } => {
                format!("Preempted (by {})", by_executor_id)
            }
            TaskTraceState::Idle => "Idle".to_string(),
            TaskTraceState::Ended => "Ended".to_string(),
        }
    }
}

pub struct TaskTracing {
    task_id: u32,
    executor_id: u32,
    core_id: u8,

    trace_event_sender: Sender<TracingEvent>,

    /// Current state of the task
    state: TaskTraceState,
    /// Timestamp when the current state started
    state_start_time: EmbassyTime,
}

impl TaskTracing {
    pub fn new(
        task_id: u32,
        executor_id: u32,
        core_id: u8,
        trace_event_sender: Sender<TracingEvent>,
        firmware_addr_map: &FirmwareAddressMap,
        created_at: EmbassyTime,
    ) -> Self {
        // try to find task name from global firmware address map
        let task_name = firmware_addr_map.get_symbol_name(task_id as u64);
        let display_name = match task_name.as_ref() {
            Some(name) => name.clone(),
            None => format!("Task 0x{:X}", task_id),
        };

        // Send task metadata
        let _ = trace_event_sender.send(TracingEvent::Metadata {
            name: "thread_name".to_string(),
            cat: None,
            args: HashMap::from([("name".to_string(), display_name)]),
            pid: executor_id,
            tid: Some(task_id),
        });

        // Send Begin trace event for new state SPAWNED
        let _ = trace_event_sender.send(TracingEvent::Begin {
            name: TaskTraceState::Spawned.to_string(),
            cat: None,
            ts: created_at.as_micros(),
            pid: executor_id,
            tid: Some(task_id),
            args: HashMap::new(),
        });

        let instance = TaskTracing {
            task_id,
            executor_id,
            core_id,
            trace_event_sender,
            state: TaskTraceState::Spawned,
            state_start_time: created_at,
        };

        instance
    }

    pub fn get_pid(&self) -> u32 {
        self.executor_id
    }

    /// Set a new state for the task, sending statistics as needed
    fn set_new_state(&mut self, new_state: TaskTraceState, timestamp: EmbassyTime) {
        if self.state != new_state {
            // Send End trace event for state change
            let _ = self.trace_event_sender.send(TracingEvent::End {
                name: None,
                cat: None,
                pid: self.get_pid(),
                tid: Some(self.task_id),
                ts: timestamp.as_micros(),
                args: HashMap::new(),
            });

            // Send Begin trace event for new state
            let _ = self.trace_event_sender.send(TracingEvent::Begin {
                name: new_state.to_string(),
                cat: None,
                ts: timestamp.as_micros(),
                pid: self.get_pid(),
                tid: Some(self.task_id),
                args: HashMap::new(),
            });

            // update state
            self.state = new_state;
            self.state_start_time = timestamp;
        }
    }

    /// Update the task state based on a new trace item
    pub fn update(&mut self, log_event: &LogEvent) {
        // Check if we get preempted
        if self.state == TaskTraceState::Running {
            // check if another executor on the same core_id is beginning to poll (that would preempt us because only one executor can run on a core at a time)
            if let LogEventType::EventEmbassyPollStart { executor_id, .. } = log_event.event_type {
                if log_event.core_id == self.core_id && executor_id != self.executor_id {
                    // preempted by another executor
                    self.set_new_state(
                        TaskTraceState::Preempted {
                            by_executor_id: executor_id,
                        },
                        log_event.timestamp,
                    );
                    return;
                }
            }
        }

        // // Check if we are resuming from preemption (other executor is now idle)
        if let TaskTraceState::Preempted { by_executor_id } = self.state {
            // check if the other executor goes to idle
            if let LogEventType::EventEmbassyExecutorIdle { executor_id, .. } = log_event.event_type
            {
                if executor_id == by_executor_id {
                    // resume our task to running
                    self.set_new_state(TaskTraceState::Running, log_event.timestamp);
                    return;
                }
            }
        }

        // Check that this trace item is for this executor
        if log_event
            .event_type
            .get_executor_id()
            .is_some_and(|exe_id| exe_id == self.executor_id)
        {
            // Check that this trace item is for this task
            if log_event
                .event_type
                .get_task_id()
                .is_some_and(|tid| tid == self.task_id)
            {
                // State machine transitions
                match self.state {
                    TaskTraceState::Spawned => {
                        if let LogEventType::EventEmbassyTaskReadyBegin { .. } =
                            log_event.event_type
                        {
                            self.set_new_state(TaskTraceState::Waiting, log_event.timestamp);
                        }
                    }
                    TaskTraceState::Waiting => {
                        if let LogEventType::EventEmbassyTaskExecBegin { .. } = log_event.event_type
                        {
                            self.set_new_state(TaskTraceState::Running, log_event.timestamp);
                        }
                    }
                    TaskTraceState::Running => {
                        match log_event.event_type {
                            LogEventType::EventEmbassyTaskExecEnd { .. } => {
                                self.set_new_state(TaskTraceState::Idle, log_event.timestamp);
                            }
                            LogEventType::EventEmbassyTaskReadyBegin { .. } => {
                                // Normally this would transition after TaskExecEnd, but we can handle it here in this way too (maybe?)
                                // This means the task was re-awoken while running
                                self.set_new_state(TaskTraceState::Waiting, log_event.timestamp);
                            }
                            LogEventType::EventEmbassyTaskEnd { .. } => {
                                self.set_new_state(TaskTraceState::Ended, log_event.timestamp);
                            }
                            _ => {}
                        }
                    }
                    TaskTraceState::Idle => {
                        if let LogEventType::EventEmbassyTaskReadyBegin { .. } =
                            log_event.event_type
                        {
                            self.set_new_state(TaskTraceState::Waiting, log_event.timestamp);
                        }
                    }
                    TaskTraceState::Ended => {
                        // No transitions out of ended for tasks
                    }
                    TaskTraceState::Preempted { .. } => {} // nothing here because of other task-id
                }
            }
        }
    }
}
