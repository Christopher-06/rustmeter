//! ## Executor Tracing lifecycle
//!
//! ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─
//!       │(1)                                             │
//! │     │
//!   ╔═══▼══╗   (2)     ┌────────────┐  (3)  ┌─────────┐  │
//! │ ║ IDLE ║──────────▶│ SCHEDULING │──────▶│ POLLING │
//!   ╚══════╝           └────────────┘       └─────────┘  │
//! │     ▲              │            ▲            │
//!       │      (5)     │            │  (4)       │       │
//! │     └──────────────┘            └────────────┘
//!   ┌──────────────────────────┐                         │
//! └ ┤ Executor Trace Lifecycle │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─
//!   └──────────────────────────┘
//!
//! 1. The executor is started (no associated trace)
//! 2. A task on this executor is awoken. `_embassy_trace_task_ready_begin` is called
//!    when this occurs, and `_embassy_trace_poll_start` is called when the executor
//!    actually begins running
//! 3. The executor has decided a task to poll. `_embassy_trace_task_exec_begin` is called
//! 4. The executor finishes polling the task. `_embassy_trace_task_exec_end` is called
//! 5. The executor has finished polling tasks. `_embassy_trace_executor_idle` is called
//!
//! (taken from embassy-executor/src/raw/trace.rs)
//!

use std::{collections::HashMap, fmt::Display};

use crossbeam::channel::Sender;

use crate::{
    elf_file::FirmwareAddressMap,
    perfetto_backend::trace_event::TracingEvent,
    time::EmbassyTime,
    tracing::{
        log_event::{LogEvent, LogEventType},
        task::TaskTracing,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum PreemptedPrevState {
    Scheduling,
    Polling { task_id: u32 },
}

impl From<PreemptedPrevState> for ExecutorState {
    fn from(val: PreemptedPrevState) -> Self {
        match val {
            PreemptedPrevState::Scheduling => ExecutorState::Scheduling,
            PreemptedPrevState::Polling { task_id } => ExecutorState::Polling { task_id },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum ExecutorState {
    Idle,
    Scheduling,
    /// Executor was preempted by another higher priority executor on the same core
    Preempted {
        by_executor_id: u32,
        prev_state: PreemptedPrevState,
    },
    Polling {
        task_id: u32,
    },
}

impl Display for ExecutorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutorState::Idle => write!(f, "Idle"),
            ExecutorState::Scheduling => write!(f, "Scheduling"),
            ExecutorState::Preempted { by_executor_id, .. } => {
                write!(f, "Preempted (by {by_executor_id})")
            }
            ExecutorState::Polling { .. } => write!(f, "Polling"),
        }
    }
}

pub struct ExecutorTracing {
    executor_id: u32,
    core_id: u8,
    display_name: String,

    firmware_addr_map: FirmwareAddressMap,
    trace_event_sender: Sender<TracingEvent>,

    /// Current state of the executor
    state: ExecutorState,
    /// Timestamp when the current state started
    state_start_time: EmbassyTime,

    tasks: HashMap<u32, TaskTracing>,
}

impl ExecutorTracing {
    pub fn new(
        executor_id: u32,
        core_id: u8,
        created_at: EmbassyTime,
        firmware_addr_map: FirmwareAddressMap,
        trace_event_sender: Sender<TracingEvent>,
    ) -> Self {
        // try to find task name from global firmware address map
        let executor_name = firmware_addr_map.get_symbol_name(executor_id as u64);
        let display_name = match &executor_name {
            Some(name) => name.clone(),
            None => format!("Executor 0x{executor_id:X}"),
        };

        // Send executor metadata
        let _ = trace_event_sender.send(TracingEvent::Metadata {
            name: "process_name".to_string(),
            cat: None,
            args: HashMap::from([
                (
                    "name".to_string(),
                    format!("[CORE {core_id}] {display_name}"),
                ),
                ("core".to_string(), core_id.to_string()),
            ]),
            pid: executor_id,
            tid: None,
        });
        // Send thread name metadata (to visualize executors as threads in perfetto)
        let _ = trace_event_sender.send(TracingEvent::Metadata {
            name: "thread_name".to_string(),
            cat: None,
            args: HashMap::from([
                ("name".to_string(), "Executor".to_string()),
                ("core".to_string(), core_id.to_string()),
            ]),
            pid: executor_id,
            tid: None,
        });

        // Send Begin trace event for executor creation
        let _ = trace_event_sender.send(TracingEvent::Begin {
            name: "Created".to_string(),
            cat: None,
            ts: created_at.as_micros(),
            pid: executor_id,
            tid: None,
            args: HashMap::new(),
        });

        Self {
            executor_id,
            core_id,
            display_name,
            state: ExecutorState::Idle,
            state_start_time: created_at,
            firmware_addr_map,
            trace_event_sender,
            tasks: HashMap::new(),
        }
    }

    /// Get the display name of the executor
    pub fn get_name(&self) -> &str {
        &self.display_name
    }

    /// Get the executor ID
    pub fn get_executor_id(&self) -> u32 {
        self.executor_id
    }

    /// Set a new state for the executor, sending statistics as needed
    fn set_new_state(&mut self, new_state: ExecutorState, timestamp: EmbassyTime) {
        if self.state != new_state {
            // Send End trace event for previous state
            let _ = self.trace_event_sender.send(TracingEvent::End {
                name: None,
                cat: None,
                ts: timestamp.as_micros(),
                pid: self.executor_id,
                tid: None,
                args: HashMap::new(),
            });
            // Send Begin trace event for new state
            let _ = self.trace_event_sender.send(TracingEvent::Begin {
                name: new_state.to_string(),
                cat: None,
                ts: timestamp.as_micros(),
                pid: self.executor_id,
                tid: None,
                args: HashMap::new(),
            });

            // update state
            self.state = new_state;
            self.state_start_time = timestamp;
        }
    }

    /// Run State Machine transition based on trace item
    pub fn update(&mut self, log_event: &LogEvent) {
        // Check if the log event contains a task for this executor that we do not yet track
        if let Some(executor_id) = log_event.event_type.get_executor_id() {
            // Check executor ID
            if executor_id == self.executor_id {
                if let Some(task_id) = log_event.event_type.get_task_id() {
                    if !self.tasks.contains_key(&task_id) {
                        // If the task does not exist, create it (probably a TaskNew event)
                        let new_task = TaskTracing::new(
                            task_id,
                            self.executor_id,
                            self.core_id,
                            self.trace_event_sender.clone(),
                            &self.firmware_addr_map,
                            log_event.timestamp,
                        );
                        self.tasks.insert(task_id, new_task);
                    }
                }
            }
        }

        // Update tasks
        for task in self.tasks.values_mut() {
            task.update(log_event);
        }

        // Check preemption state
        match self.state {
            ExecutorState::Polling { .. } | ExecutorState::Scheduling => {
                // Check if we are beeing preempted
                if let LogEventType::EventEmbassyPollStart { executor_id } = log_event.event_type {
                    if executor_id != self.executor_id && log_event.core_id == self.core_id {
                        // preempt
                        let prev_state = match self.state {
                            ExecutorState::Scheduling => PreemptedPrevState::Scheduling,
                            ExecutorState::Polling { task_id } => {
                                PreemptedPrevState::Polling { task_id }
                            }
                            _ => unreachable!(),
                        };

                        self.set_new_state(
                            ExecutorState::Preempted {
                                by_executor_id: executor_id,
                                prev_state,
                            },
                            log_event.timestamp,
                        );
                    }
                }
            }
            ExecutorState::Preempted {
                by_executor_id,
                prev_state,
            } => {
                // Check if we can resume (the higher prio executor goes back to idle)
                if let LogEventType::EventEmbassyExecutorIdle { executor_id } = log_event.event_type
                {
                    if executor_id == by_executor_id {
                        // resume
                        self.set_new_state(prev_state.into(), log_event.timestamp);
                    }
                }
            }
            _ => {}
        }

        // Check that the trace item is for this executor
        if log_event
            .event_type
            .get_executor_id()
            .is_some_and(|exe_id| exe_id == self.executor_id)
        {
            // Executor State machine transitions

            match self.state {
                ExecutorState::Idle => {
                    if let LogEventType::EventEmbassyPollStart { .. } = log_event.event_type {
                        self.set_new_state(ExecutorState::Scheduling, log_event.timestamp);
                    }
                }
                ExecutorState::Scheduling => {
                    if let LogEventType::EventEmbassyTaskExecBegin { task_id, .. } =
                        log_event.event_type
                    {
                        self.set_new_state(ExecutorState::Polling { task_id }, log_event.timestamp);
                    }

                    if let LogEventType::EventEmbassyExecutorIdle { .. } = log_event.event_type {
                        self.set_new_state(ExecutorState::Idle, log_event.timestamp);
                    }
                }
                ExecutorState::Polling { .. } => {
                    if let LogEventType::EventEmbassyTaskExecEnd { .. } = log_event.event_type {
                        self.set_new_state(ExecutorState::Scheduling, log_event.timestamp);
                    }
                }
                _ => {}
            }
        }
    }

    /// Check if this executor is currently spending cpu time (scheduling or polling)
    pub fn is_currently_running(&self) -> bool {
        matches!(self.state, ExecutorState::Polling { .. })
            || matches!(self.state, ExecutorState::Scheduling)
    }

    /// Check if there is a currently running task and return it
    pub fn get_currently_running_task(&self) -> Option<&TaskTracing> {
        if let ExecutorState::Polling { task_id } = self.state {
            return self.tasks.get(&task_id);
        }

        None
    }
}
