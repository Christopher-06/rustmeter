use std::{
    collections::{HashMap, VecDeque},
    u32,
};

use arbitrary_int::u3;
use crossbeam::channel::Sender;

use crate::{perfetto_backend::trace_event::TracingEvent, tracing::executor::ExecutorTracing};

macro_rules! begin_state {
    ($self:ident, $name:expr, $timestamp:expr) => {{
        let _ = $self.trace_event_tx.send(TracingEvent::Begin {
            name: $name,
            cat: None,
            pid: u32::MAX - 1,
            ts: $timestamp.as_micros(),
            tid: Some(core_id_to_tid!($self)),
            args: HashMap::new(),
        });
    }};
}

macro_rules! end_state {
    ($self:ident, $name:expr, $timestamp:expr) => {{
        let _ = $self.trace_event_tx.send(TracingEvent::End {
            name: $name,
            cat: None,
            pid: u32::MAX - 1,
            ts: $timestamp.as_micros(),
            tid: Some(core_id_to_tid!($self)),
            args: HashMap::new(),
        });
    }};
}

macro_rules! core_id_to_tid {
    ($self:ident) => {
        $self.core_id as u32 + 1 // TID 0 is reserved, so core 0 -> TID 1
    };
}

pub struct CoreTracing {
    core_id: u8,
    executors: HashMap<u3, ExecutorTracing>,
    trace_event_tx: Sender<TracingEvent>,
    monitor_stack: VecDeque<(String, std::time::Duration)>,

    preempted_monitors: VecDeque<String>,
}

impl CoreTracing {
    pub fn new(core_id: u8, trace_event_tx: Sender<TracingEvent>) -> Self {
        Self {
            core_id,
            executors: HashMap::new(),
            trace_event_tx,
            monitor_stack: VecDeque::new(),
            preempted_monitors: VecDeque::new(),
        }
    }

    pub fn monitor_start(&mut self, name: String, timestamp: std::time::Duration) {
        // Check if any executor is running which we can associate the monitor with
        let running_executor = self.executors.values_mut().find(|e| e.is_running());

        if let Some(executor) = running_executor {
            // Associate monitor with running executor
            executor.on_monitor_start(name, timestamp);
        } else {
            // No executor running, just log monitor start
            self.monitor_stack.push_front((name.clone(), timestamp));
        }
    }

    /// Try to forward monitor end to running executor else just log monitor end here
    pub fn monitor_end(&mut self, timestamp: std::time::Duration) {
        let running_executor = self.executors.values_mut().find(|e| e.is_running());
        if let Some(executor) = running_executor {
            // Found running executor, forward event
            executor.on_monitor_end(timestamp);
        } else {
            // Else just log monitor end here?
            if let Some((name, start_timestamp)) = self.monitor_stack.pop_front() {
                let tid = core_id_to_tid!(self);
                let _ = self.trace_event_tx.send(TracingEvent::Complete {
                    name,
                    cat: Some("code_monitor".into()),
                    pid: u32::MAX - 1,
                    tid,
                    ts: start_timestamp.as_micros(),
                    dur: (timestamp - start_timestamp).as_micros() as u64 - 30,
                    args: HashMap::new(),
                });
            }
        }
    }

    pub fn on_task_new_spawned(
        &mut self,
        executor_id: u3,
        task_id: u16,
        timestamp: std::time::Duration,
    ) -> anyhow::Result<()> {
        if let Some(executor) = self.executors.get_mut(&executor_id) {
            // Found executor, forward event
            executor.on_task_new_spawned(task_id, timestamp)
        } else {
            // Ignore event, no executor found
            // Task created without executor context is not valid to create a new executor tracing (core info missing)

            Ok(())
        }
    }

    pub fn on_desynchronize(&mut self, timestamp: std::time::Duration) {
        // feed desync to all executors
        for executor in self.executors.values_mut() {
            executor.on_desynchronize(timestamp);
        }

        end_state!(self, None, timestamp);
        begin_state!(self, "Desynchronization".into(), timestamp);

        // write all open code monitors as completed till now
        let tid = core_id_to_tid!(self);
        for (name, start_timestamp) in self.monitor_stack.drain(..) {
            let _ = self.trace_event_tx.send(TracingEvent::Complete {
                name,
                cat: Some("code_monitor".into()),
                pid: u32::MAX - 1,
                tid,
                ts: start_timestamp.as_micros(),
                dur: (timestamp - start_timestamp).as_micros() as u64,
                args: HashMap::new(),
            });
        }
    }

    /// Handle a task ready event. Tries to find the executor tracing otherwise ignores the event.
    pub fn on_task_ready(
        &mut self,
        executor_id: u3,
        task_id: u16,
        timestamp: std::time::Duration,
    ) -> anyhow::Result<()> {
        if let Some(executor) = self.executors.get_mut(&executor_id) {
            // Found executor, forward event
            executor.on_task_ready(task_id, timestamp)
        } else {
            // No executor found, ignore event

            // task ready without executor context is not valid to create a new executor tracing (core info missing)

            Ok(())
        }
    }

    /// Handle a task execution begin event. Creates a new polling executor tracing if not found. Checks for preemption of other executors.
    pub fn on_task_exec_begin(
        &mut self,
        executor_id: u3,
        task_id: u16,
        timestamp: std::time::Duration,
    ) -> anyhow::Result<()> {
        if let Some(executor) = self.executors.get_mut(&executor_id) {
            // Found executor, forward event
            executor.on_task_exec_begin(task_id, timestamp)
        } else {
            // Create new polling executor
            let executor = ExecutorTracing::new_polling(
                executor_id,
                timestamp,
                task_id,
                self.trace_event_tx.clone(),
            );
            self.executors.insert(executor_id, executor);
            Ok(())
        }
    }

    pub fn on_task_exec_end(
        &mut self,
        executor_id: u3,
        timestamp: std::time::Duration,
    ) -> anyhow::Result<()> {
        if let Some(executor) = self.executors.get_mut(&executor_id) {
            // Found executor, forward event
            executor.on_task_exec_end(timestamp)?;
        }

        Ok(())
    }

    pub fn on_task_end(
        &mut self,
        executor_id: u3,
        task_id: u16,
        timestamp: std::time::Duration,
    ) -> anyhow::Result<()> {
        if let Some(executor) = self.executors.get_mut(&executor_id) {
            // Found executor, forward event
            executor.on_task_end(timestamp, task_id)?;
        }
        Ok(())
    }

    pub fn on_executor_poll_start(
        &mut self,
        executor_id: u3,
        timestamp: std::time::Duration,
    ) -> anyhow::Result<()> {
        if !self.executors.contains_key(&executor_id) {
            return Ok(()); // Ignore event, no executor found (probably another core)
        }

        // Check if any other executor is currently running on this core, if so, preempt it
        let running_executor = self
            .executors
            .values_mut()
            .find(|e| e.is_running() && e.executor_id() != executor_id);
        if let Some(running_executor) = running_executor {
            // Preempt other running executor
            running_executor.on_preempted(timestamp, executor_id)?;

            // write all open code monitors as completed till now
            let tid = core_id_to_tid!(self);
            for (name, start_timestamp) in self.monitor_stack.drain(..) {
                self.preempted_monitors.push_front(name.clone());
                let _ = self.trace_event_tx.send(TracingEvent::Complete {
                    name,
                    cat: Some("code_monitor".into()),
                    pid: u32::MAX - 1,
                    tid,
                    ts: start_timestamp.as_micros(),
                    dur: (timestamp - start_timestamp).as_micros() as u64,
                    args: HashMap::new(),
                });
            }

            end_state!(
                self,
                Some(format!("Executor {}", running_executor.executor_id())),
                timestamp
            );
        }

        if let Some(executor) = self.executors.get_mut(&executor_id) {
            // Forward event
            begin_state!(self, format!("Executor {}", executor_id), timestamp);
            executor.on_poll_start(timestamp)
        } else {
            // Ignore event, no executor found
            // Poll start without executor context is not valid to create a new executor tracing (core info missing)

            Ok(())
        }
    }

    pub fn on_executor_idle(
        &mut self,
        executor_id: u3,
        timestamp: std::time::Duration,
    ) -> anyhow::Result<()> {
        if !self.executors.contains_key(&executor_id) {
            return Ok(()); // Ignore event, no executor found (probably another core)
        }

        end_state!(self, Some(format!("Executor {}", executor_id)), timestamp);

        // Check if any got preempted by this executor and resume it
        let preempted_executor = self
            .executors
            .values_mut()
            .find(|e| e.is_preempted_by(executor_id));
        if let Some(executor) = preempted_executor {
            executor.on_resume(timestamp)?;

            begin_state!(
                self,
                format!("Executor {}", executor.executor_id()),
                timestamp
            );

            // Restore preempted code monitors
            for name in self.preempted_monitors.drain(..) {
                self.monitor_stack.push_front((name, timestamp));
            }
        }

        if let Some(executor) = self.executors.get_mut(&executor_id) {
            // Found executor, forward event
            executor.on_idle(timestamp)
        } else {
            // Ignore event, no executor found
            // Idle without executor context is not valid to create a new executor tracing (core info missing)
            Ok(())
        }
    }

    pub fn on_drop(&mut self, timestamp: std::time::Duration) {
        // feed drop to all executors
        for executor in self.executors.values_mut() {
            executor.on_drop(timestamp);
        }

        // Send all code monitors as completed
        for (name, start_timestamp) in self.monitor_stack.drain(..) {
            let _ = self.trace_event_tx.send(TracingEvent::Complete {
                name,
                cat: Some("code_monitor".into()),
                pid: u32::MAX - 1,
                tid: core_id_to_tid!(self),
                ts: start_timestamp.as_micros(),
                dur: (timestamp - start_timestamp).as_micros() as u64,
                args: HashMap::new(),
            });
        }

        end_state!(self, None, timestamp);
    }
}
