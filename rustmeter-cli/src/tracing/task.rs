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
//!    called. The task does not IMMEDIATELY move state, until polling is complete and the
//!    RUNNING state is existed. `_embassy_trace_task_exec_end` is called when polling is
//!    complete, marking the transition to WAITING
//! 5. Polling is complete, `_embassy_trace_task_exec_end` is called
//! 6. The task has completed, and `_embassy_trace_task_end` is called
//! 7. A task is awoken, `_embassy_trace_task_ready_begin` is called
//!
//! (taken from embassy-executor/src/raw/trace.rs)

use std::{
    collections::{HashMap, VecDeque},
    time::Duration,
};

use anyhow::bail;
use arbitrary_int::u3;
use crossbeam::channel::Sender;

use crate::perfetto_backend::trace_event::{TracingArgsMap, TracingEvent};

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum TaskState {
    Spawned,
    Ready,
    Running,
    Preempted { by_executor_id: u3 },
    Idle,
    Ended,
    StreamDesynchronized,
}

impl TaskState {
    pub fn to_string(&self) -> String {
        match self {
            TaskState::Spawned => "Spawned".to_string(),
            TaskState::Ready => "Ready".to_string(),
            TaskState::Running => "Running".to_string(),
            TaskState::Preempted { by_executor_id } => format!("Preempted (by {})", by_executor_id),
            TaskState::Idle => "Idle".to_string(),
            TaskState::Ended => "Ended".to_string(),
            TaskState::StreamDesynchronized => "StreamDesynchronized".to_string(),
        }
    }
}

pub struct TaskTracing {
    executor_id: u3,
    task_id: u16,
    state: TaskState,
    state_start: Duration,
    reawoken_while_running: bool,
    trace_event_tx: Sender<TracingEvent>,

    current_monitors: VecDeque<(String, Duration)>,
    preempted_monitors: VecDeque<String>,
}

impl TaskTracing {
    /// Create a new TaskTracing in Spawned state
    pub fn new_spawned(
        executor_id: u3,
        task_id: u16,
        state_start: Duration,
        trace_event_tx: Sender<TracingEvent>,
    ) -> Self {
        // send end event for previous state (when created and data loss happened)
        let _ = trace_event_tx.send(TracingEvent::End {
            name: None,
            cat: None,
            pid: executor_id.into(),
            tid: Some(task_id as u32),
            ts: state_start.as_micros(),
            args: TracingArgsMap::new(),
        });

        Self {
            executor_id,
            task_id,
            state: TaskState::Spawned,
            state_start,
            reawoken_while_running: false,
            trace_event_tx,
            current_monitors: VecDeque::new(),
            preempted_monitors: VecDeque::new(),
        }
    }

    /// Create a new TaskTracing in Ready state
    pub fn new_ready(
        executor_id: u3,
        task_id: u16,
        state_start: Duration,
        trace_event_tx: Sender<TracingEvent>,
    ) -> Self {
        // send end event for previous state (when created and data loss happened)
        let _ = trace_event_tx.send(TracingEvent::End {
            name: None,
            cat: None,
            pid: executor_id.into(),
            tid: Some(task_id as u32),
            ts: state_start.as_micros(),
            args: TracingArgsMap::new(),
        });

        Self {
            executor_id,
            task_id,
            state: TaskState::Ready,
            state_start,
            reawoken_while_running: false,
            trace_event_tx,
            current_monitors: VecDeque::new(),
            preempted_monitors: VecDeque::new(),
        }
    }

    /// Create a new TaskTracing in Running state
    pub fn new_exec_begin(
        executor_id: u3,
        task_id: u16,
        state_start: Duration,
        trace_event_tx: Sender<TracingEvent>,
    ) -> Self {
        // send end event for previous state (when created and data loss happened)
        let _ = trace_event_tx.send(TracingEvent::End {
            name: None,
            cat: None,
            pid: executor_id.into(),
            tid: Some(task_id as u32),
            ts: state_start.as_micros(),
            args: TracingArgsMap::new(),
        });

        Self {
            executor_id,
            task_id,
            state: TaskState::Running,
            state_start,
            reawoken_while_running: false,
            trace_event_tx,
            current_monitors: VecDeque::new(),
            preempted_monitors: VecDeque::new(),
        }
    }

    pub fn state(&self) -> &TaskState {
        &self.state
    }

    fn transition_to(&mut self, new_state: TaskState, timestamp: Duration) {
        if self.state != new_state {
            // send trace event (end and begin)
            let _ = self.trace_event_tx.send(TracingEvent::End {
                name: None,
                cat: None,
                pid: self.executor_id.into(),
                tid: Some(self.task_id as u32),
                ts: timestamp.as_micros(),
                args: TracingArgsMap::new(),
            });
            let _ = self.trace_event_tx.send(TracingEvent::Begin {
                name: new_state.to_string(),
                cat: None,
                pid: self.executor_id.into(),
                tid: Some(self.task_id as u32),
                ts: timestamp.as_micros(),
                args: TracingArgsMap::new(),
            });

            // switch state
            self.state = new_state;
            self.state_start = timestamp;
        }
    }

    pub fn on_desynchronize(&mut self, timestamp: Duration) {
        self.transition_to(TaskState::StreamDesynchronized, timestamp);

        // Send all code monitors as completed to now
        for (name, start_timestamp) in self.current_monitors.drain(..) {
            let _ = self.trace_event_tx.send(TracingEvent::Complete {
                name,
                cat: Some("code_monitor".into()),
                pid: self.executor_id.into(),
                tid: self.task_id as u32,
                ts: start_timestamp.as_micros(),
                dur: (timestamp - start_timestamp).as_micros() as u64,
                args: HashMap::new(),
            });
        }
    }

    /// Called when task is ready to run
    pub fn on_ready(&mut self, timestamp: Duration) -> anyhow::Result<()> {
        match self.state {
            TaskState::Spawned | TaskState::Idle => {
                self.transition_to(TaskState::Ready, timestamp);
                self.reawoken_while_running = false;
                Ok(())
            }
            TaskState::Running | TaskState::Preempted { by_executor_id: _ } => {
                // Mark that the task was reawoken while running
                self.reawoken_while_running = true;
                Ok(())
            }
            _ => {
                // Cannot transition to Ready from other states
                bail!(
                    "Task {} cannot transition to Ready from state {:?}",
                    self.task_id,
                    self.state
                );
            }
        }
    }

    /// Called when task starts execution
    pub fn on_exec_begin(&mut self, timestamp: Duration) -> anyhow::Result<()> {
        match self.state {
            TaskState::Ready => {
                self.transition_to(TaskState::Running, timestamp);
                Ok(())
            }
            _ => {
                // Only Ready tasks can start execution
                bail!(
                    "Task {} is not in Ready state, cannot begin execution (current state: {:?})",
                    self.task_id,
                    self.state
                );
            }
        }
    }

    /// Called when task execution ends
    pub fn on_exec_end(&mut self, timestamp: Duration) -> anyhow::Result<()> {
        match self.state {
            TaskState::Running => {
                if self.reawoken_while_running {
                    self.transition_to(TaskState::Ready, timestamp);
                } else {
                    self.transition_to(TaskState::Idle, timestamp);
                }

                self.reawoken_while_running = false;
                Ok(())
            }
            _ => {
                // Only Running tasks can end
                bail!(
                    "Task {} is not in Running state, cannot end execution (current state: {:?})",
                    self.task_id,
                    self.state
                );
            }
        }
    }

    /// Called when task ends
    pub fn on_end(&mut self, timestamp: Duration) -> anyhow::Result<()> {
        match self.state {
            TaskState::Running => {
                self.transition_to(TaskState::Ended, timestamp);
                Ok(())
            }
            _ => {
                // Only Running tasks can end
                bail!(
                    "Task {} is not in Running state, cannot end (current state: {:?})",
                    self.task_id,
                    self.state
                );
            }
        }
    }

    /// Called when task is preempted
    pub fn on_preempted(&mut self, timestamp: Duration, by_executor_id: u3) -> anyhow::Result<()> {
        match self.state {
            TaskState::Running => {
                self.transition_to(TaskState::Preempted { by_executor_id }, timestamp);

                // Send all code monitors as completed till now and store them as preempted
                for (name, start_timestamp) in self.current_monitors.drain(..) {
                    self.preempted_monitors.push_front(name.clone());
                    let _ = self.trace_event_tx.send(TracingEvent::Complete {
                        name,
                        cat: Some("code_monitor".into()),
                        pid: self.executor_id.into(),
                        tid: self.task_id as u32,
                        ts: start_timestamp.as_micros(),
                        dur: (timestamp - start_timestamp).as_micros() as u64,
                        args: HashMap::new(),
                    });
                }

                Ok(())
            }
            _ => {
                // Only running tasks can be preempted
                bail!(
                    "Task {} is not in Running state, cannot be preempted (current state: {:?})",
                    self.task_id,
                    self.state
                );
            }
        }
    }

    /// Called when task is resumed from preemption
    pub fn on_resumed(&mut self, timestamp: Duration) -> anyhow::Result<()> {
        match self.state {
            TaskState::Preempted { by_executor_id: _ } => {
                self.transition_to(TaskState::Running, timestamp);

                // Restore preempted code monitors
                for name in self.preempted_monitors.drain(..) {
                    self.current_monitors.push_back((name, timestamp));
                }

                Ok(())
            }
            _ => {
                bail!(
                    "Task {} is not in Preempted state, cannot resume (current state: {:?})",
                    self.task_id,
                    self.state
                );
            }
        }
    }

    /// Push a new monitor onto the monitor stack
    pub fn on_monitor_start(&mut self, name: String, timestamp: std::time::Duration) {
        self.current_monitors.push_back((name.clone(), timestamp));
    }

    /// Top of the monitor stack is ended
    pub fn on_monitor_end(&mut self, timestamp: std::time::Duration) {
        if let Some((name, start_timestamp)) = self.current_monitors.pop_back() {
            let _ = self.trace_event_tx.send(TracingEvent::Complete {
                name,
                cat: Some("code_monitor".into()),
                pid: self.executor_id.into(),
                tid: self.task_id as u32,
                ts: start_timestamp.as_micros(),
                dur: (timestamp - start_timestamp).as_micros() as u64,
                args: HashMap::new(),
            });
        }
    }

    pub fn on_drop(&mut self, timestamp: Duration) {
        // send end event for current state
        let _ = self.trace_event_tx.send(TracingEvent::End {
            name: None,
            cat: None,
            pid: self.executor_id.into(),
            tid: Some(self.task_id as u32),
            ts: timestamp.as_micros(),
            args: TracingArgsMap::new(),
        });

        // Send all code monitors as completed
        for (name, start_timestamp) in self.current_monitors.drain(..) {
            let _ = self.trace_event_tx.send(TracingEvent::Complete {
                name,
                cat: Some("code_monitor".into()),
                pid: self.executor_id.into(),
                tid: self.task_id as u32,
                ts: start_timestamp.as_micros(),
                dur: (timestamp - start_timestamp).as_micros() as u64,
                args: HashMap::new(),
            });
        }
    }
}
