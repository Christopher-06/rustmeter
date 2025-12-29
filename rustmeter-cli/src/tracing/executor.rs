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

use std::{collections::HashMap, time::Duration};

use anyhow::bail;
use arbitrary_int::u3;
use crossbeam::channel::Sender;

use crate::{
    perfetto_backend::trace_event::{TracingArgsMap, TracingEvent},
    tracing::task::{TaskState, TaskTracing},
};

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum PreemptedPrevState {
    Scheduling,
    Polling { task_id: u16 },
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
        by_executor_id: u3,
        prev_state: PreemptedPrevState,
    },
    Polling {
        task_id: u16,
    },
    StreamDesynchronized,
}

impl ExecutorState {
    pub fn to_string(&self) -> String {
        match self {
            ExecutorState::Idle => "Idle".to_string(),
            ExecutorState::Scheduling => "Scheduling".to_string(),
            ExecutorState::Preempted { by_executor_id, .. } => {
                format!("Preempted (by Executor {})", by_executor_id)
            }
            ExecutorState::Polling { task_id } => format!("Polling Task {}", task_id),
            ExecutorState::StreamDesynchronized => "Stream Desynchronized".to_string(),
        }
    }
}

pub struct ExecutorTracing {
    /// Short unique executor ID
    executor_id: u3,

    /// Current
    current_state: ExecutorState,
    state_start: Duration,

    /// Tracked tasks by their short unique task ID
    tasks: HashMap<u16, TaskTracing>,

    trace_event_tx: Sender<TracingEvent>,
}

impl ExecutorTracing {
    /// Create a new ExecutorTracing in Polling state with given task
    pub fn new_polling(
        executor_id: u3,
        state_start: Duration,
        task_id: u16,
        trace_event_tx: Sender<TracingEvent>,
    ) -> Self {
        // send end event for previous state (when created and data loss happened)
        let _ = trace_event_tx.send(TracingEvent::End {
            name: None,
            cat: None,
            pid: executor_id.into(),
            tid: Some(0),
            ts: state_start.as_micros(),
            args: TracingArgsMap::new(),
        });

        Self {
            executor_id,
            current_state: ExecutorState::Polling { task_id },
            state_start,
            tasks: HashMap::from([(
                task_id,
                TaskTracing::new_exec_begin(
                    executor_id,
                    task_id,
                    state_start,
                    trace_event_tx.clone(),
                ),
            )]),
            trace_event_tx,
        }
    }

    /// Check if executor is currently running (Polling or Scheduling) on the core
    pub fn is_running(&self) -> bool {
        matches!(self.current_state, ExecutorState::Polling { .. })
            || matches!(self.current_state, ExecutorState::Scheduling)
    }

    /// Check if executor is preempted by given executor ID
    pub fn is_preempted_by(&self, executor_id: u3) -> bool {
        match self.current_state {
            ExecutorState::Preempted { by_executor_id, .. } if by_executor_id == executor_id => {
                true
            }
            _ => false,
        }
    }

    pub fn executor_id(&self) -> u3 {
        self.executor_id
    }

    fn transition_to(&mut self, new_state: ExecutorState, timestamp: Duration) {
        if self.current_state != new_state {
            // send trace event (end and begin)
            let _ = self.trace_event_tx.send(TracingEvent::End {
                name: None,
                cat: None,
                pid: self.executor_id.into(),
                tid: Some(0),
                ts: timestamp.as_micros(),
                args: TracingArgsMap::new(),
            });
            let _ = self.trace_event_tx.send(TracingEvent::Begin {
                name: new_state.to_string(),
                cat: None,
                pid: self.executor_id.into(),
                tid: Some(0),
                ts: timestamp.as_micros(),
                args: TracingArgsMap::new(),
            });

            // switch state
            self.current_state = new_state;
            self.state_start = timestamp;
        }
    }

    pub fn on_desynchronize(&mut self, timestamp: Duration) {
        self.transition_to(ExecutorState::StreamDesynchronized, timestamp);

        for task in self.tasks.values_mut() {
            task.on_desynchronize(timestamp);
        }
    }

    /// Called when a new task is created in Spawned state (ignores if already exists)
    pub fn on_task_new_spawned(&mut self, task_id: u16, timestamp: Duration) -> anyhow::Result<()> {
        if !self.tasks.contains_key(&task_id) {
            let task_tracing = TaskTracing::new_spawned(
                self.executor_id,
                task_id,
                timestamp,
                self.trace_event_tx.clone(),
            );
            self.tasks.insert(task_id, task_tracing);
        }
        Ok(())
    }

    /// Called when a task is ready to run (creates if not exists)
    pub fn on_task_ready(&mut self, task_id: u16, timestamp: Duration) -> anyhow::Result<()> {
        if let Some(task_tracing) = self.tasks.get_mut(&task_id) {
            // Update existing task
            task_tracing.on_ready(timestamp)
        } else {
            // Create new task in Ready state
            let task_tracing = TaskTracing::new_ready(
                self.executor_id,
                task_id,
                timestamp,
                self.trace_event_tx.clone(),
            );
            self.tasks.insert(task_id, task_tracing);
            Ok(())
        }
    }

    /// Called when a task begins execution (creates if not exists)
    pub fn on_task_exec_begin(&mut self, task_id: u16, timestamp: Duration) -> anyhow::Result<()> {
        // Check that no else task is running
        let running_tasks = self
            .tasks
            .values()
            .any(|t| matches!(t.state(), TaskState::Running));
        if running_tasks {
            bail!(
                "Executor {} cannot start executing task {} while another task is running",
                self.executor_id,
                task_id
            );
        }

        // Update or create task
        if let Some(task_tracing) = self.tasks.get_mut(&task_id) {
            // Update existing task
            task_tracing.on_exec_begin(timestamp)?;
        } else {
            // Create new task in Running state
            let task_tracing = TaskTracing::new_exec_begin(
                self.executor_id,
                task_id,
                timestamp,
                self.trace_event_tx.clone(),
            );
            self.tasks.insert(task_id, task_tracing);
        }

        // Transition to Polling state
        self.transition_to(ExecutorState::Polling { task_id }, timestamp);

        Ok(())
    }

    /// Called when a task ends execution
    pub fn on_task_exec_end(&mut self, timestamp: Duration) -> anyhow::Result<()> {
        // Get running task of Polling state
        if let ExecutorState::Polling { task_id } = self.current_state {
            if let Some(task_tracing) = self.tasks.get_mut(&task_id) {
                // Update existing task
                task_tracing.on_exec_end(timestamp)?;
            } else {
                // In Polling but no such task tracked? ==> this will normally not happen
                bail!(
                    "Executor {} has no tracked task {} to end execution for",
                    self.executor_id,
                    task_id
                );
            }
        } else {
            // Not in Polling state
            bail!(
                "Executor {} cannot end task execution while not in Polling state (current state: {:?})",
                self.executor_id,
                self.current_state
            );
        }

        self.transition_to(ExecutorState::Scheduling, timestamp);

        Ok(())
    }

    /// Called when executor is preempted
    pub fn on_preempted(&mut self, timestamp: Duration, by_executor_id: u3) -> anyhow::Result<()> {
        let prev_state = match self.current_state {
            ExecutorState::Scheduling => PreemptedPrevState::Scheduling,
            ExecutorState::Polling { task_id } => PreemptedPrevState::Polling { task_id },
            _ => {
                bail!(
                    "Cannot preempt executor {} from state {:?}",
                    self.executor_id,
                    self.current_state
                )
            }
        };

        // Preempt running task (if any)
        if let Some(task) = self
            .tasks
            .values_mut()
            .find(|t| matches!(t.state(), TaskState::Running))
        {
            // When executor is preempted, the running task is also preempted. But
            // there could be no running task (e.g., if preempted while scheduling). This
            // is not an error.
            task.on_preempted(timestamp, by_executor_id)?;
        }

        self.transition_to(
            ExecutorState::Preempted {
                by_executor_id,
                prev_state,
            },
            timestamp,
        );
        Ok(())
    }

    /// Called when executor resumes from preemption
    pub fn on_resume(&mut self, timestamp: Duration) -> anyhow::Result<()> {
        match self.current_state {
            ExecutorState::Preempted {
                by_executor_id: _,
                prev_state,
            } => {
                // Resume preempted task
                if let Some(task) = self
                    .tasks
                    .values_mut()
                    .find(|t| matches!(t.state(), TaskState::Preempted { .. }))
                {
                    task.on_resumed(timestamp)?;
                }

                self.transition_to(prev_state.into(), timestamp);
                Ok(())
            }
            _ => {
                bail!(
                    "Cannot resume executor {} from state {:?}",
                    self.executor_id,
                    self.current_state
                )
            }
        }
    }

    /// Called when a task ends
    pub fn on_task_end(&mut self, timestamp: Duration, task_id: u16) -> anyhow::Result<()> {
        // Check if we have such a task
        if !self.tasks.contains_key(&task_id) {
            // Task ended, but we have no record of it ==> ignore and not an error
            return Ok(());
        }

        // Get running task of Polling state
        if let ExecutorState::Polling { task_id } = self.current_state {
            if let Some(task_tracing) = self.tasks.get_mut(&task_id) {
                // Update existing task
                task_tracing.on_end(timestamp)?;
            } else {
                // In Polling but no such task tracked? ==> this will normally not happen
                bail!(
                    "Executor {} has no tracked task {} to end for",
                    self.executor_id,
                    task_id
                );
            }
        } else {
            // Not in Polling state
            bail!(
                "Executor {} cannot end task while not in Polling state (current state: {:?})",
                self.executor_id,
                self.current_state
            );
        }

        Ok(())
    }

    /// Called when polling starts
    pub fn on_poll_start(&mut self, timestamp: Duration) -> anyhow::Result<()> {
        self.transition_to(ExecutorState::Scheduling, timestamp);
        Ok(())
    }

    /// Called when executor becomes idle
    pub fn on_idle(&mut self, timestamp: Duration) -> anyhow::Result<()> {
        self.transition_to(ExecutorState::Idle, timestamp);
        Ok(())
    }

    /// Broadcast monitor start to current polling task
    pub fn on_monitor_start(&mut self, name: String, timestamp: std::time::Duration) {
        if let ExecutorState::Polling { task_id } = self.current_state {
            if let Some(task_tracing) = self.tasks.get_mut(&task_id) {
                task_tracing.on_monitor_start(name, timestamp);
            }
        }
    }

    /// Broadcast monitor end to current polling task
    pub fn on_monitor_end(&mut self, timestamp: std::time::Duration) {
        if let ExecutorState::Polling { task_id } = self.current_state {
            if let Some(task_tracing) = self.tasks.get_mut(&task_id) {
                task_tracing.on_monitor_end(timestamp);
            }
        }
    }

    pub fn on_drop(&mut self, timestamp: Duration) {
        // feed drop to all tasks
        for task in self.tasks.values_mut() {
            task.on_drop(timestamp);
        }

        // close current executor state
        let _ = self.trace_event_tx.send(TracingEvent::End {
            name: None,
            cat: None,
            pid: self.executor_id.into(),
            tid: Some(0),
            ts: timestamp.as_micros(),
            args: TracingArgsMap::new(),
        });
    }
}
