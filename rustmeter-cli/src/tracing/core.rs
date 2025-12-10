use std::collections::HashMap;

use crossbeam::channel::Sender;

use crate::{
    elf_file::FirmwareAddressMap,
    perfetto_backend::trace_event::TracingEvent,
    tracing::{
        executor::ExecutorTracing,
        log_event::{LogEvent, LogEventType},
    },
};

/// This container represents a single core of the controller we are tracing from. It can hold up many executors or synchronous tasks (e.g. interrupts or main loop)
pub struct CoreTracing {
    firmware_addr_map: FirmwareAddressMap,
    trace_event_sender: Sender<TracingEvent>,

    core_id: u8,
    executors: HashMap<u32, ExecutorTracing>,
}

impl CoreTracing {
    /// Create a new core tracing instance
    pub fn new(
        core_id: u8,
        firmware_addr_map: FirmwareAddressMap,
        trace_event_sender: Sender<TracingEvent>,
    ) -> Self {
        // Send core metadata
        let _ = trace_event_sender.send(TracingEvent::Metadata {
            name: "thread_name".to_string(),
            cat: None,
            args: HashMap::from([
                ("name".to_string(), format!("CORE {core_id}")),
                ("core".to_string(), core_id.to_string()),
            ]),
            pid: 0,
            tid: Some(core_id as u32),
        });

        CoreTracing {
            core_id,
            firmware_addr_map,
            trace_event_sender,
            executors: HashMap::new(),
        }
    }

    pub fn get_core_id(&self) -> u8 {
        self.core_id
    }

    pub fn update(&mut self, log_event: &LogEvent) {
        if let Some(executor_id) = log_event.event_type.get_executor_id() {
            // Check if we have an executor with this ID on this core
            if log_event.core_id == self.core_id {
                // Check that the Message is not TaskReady because those get's sent from an interrupt context and these typically run on the first core only
                if let LogEventType::EventEmbassyTaskReadyBegin { .. } = log_event.event_type {
                } else {
                    // Create new executor tracing if it does not exist
                    let executor_exists = self.executors.contains_key(&executor_id);
                    if !executor_exists {
                        self.executors.insert(
                            executor_id,
                            ExecutorTracing::new(
                                executor_id,
                                self.core_id,
                                log_event.timestamp,
                                self.firmware_addr_map.clone(),
                                self.trace_event_sender.clone(),
                            ),
                        );
                    }
                }
            }
        }

        let previously_running_executor = self
            .executors
            .values()
            .find(|exe| exe.is_currently_running())
            .map(|exe| exe.get_executor_id());

        // Update all executor
        for executor in self.executors.values_mut() {
            executor.update(log_event);
        }

        let currently_running_executor = self
            .executors
            .values()
            .find(|exe| exe.is_currently_running())
            .map(|exe| exe.get_executor_id());

        // Check for executor switches
        match (previously_running_executor, currently_running_executor) {
            (None, Some(exe_id)) => {
                // Executor started running
                let _ = self.trace_event_sender.send(TracingEvent::Begin {
                    name: self.executors.get(&exe_id).unwrap().get_name().to_string(),
                    cat: Some("executor".to_string()),
                    pid: 0,
                    tid: Some(self.core_id as u32),
                    ts: log_event.timestamp.as_micros(),
                    args: HashMap::new(),
                });
            }
            (Some(exe_id), None) => {
                // Executor stopped running
                let _ = self.trace_event_sender.send(TracingEvent::End {
                    name: Some(self.executors.get(&exe_id).unwrap().get_name().to_string()),
                    cat: Some("executor".to_string()),
                    pid: 0,
                    tid: Some(self.core_id as u32),
                    ts: log_event.timestamp.as_micros(),
                    args: HashMap::new(),
                });
            }
            (Some(prev_exe), Some(curr_exe)) if prev_exe != curr_exe => {
                // Executor switch
                // End previous
                let _ = self.trace_event_sender.send(TracingEvent::End {
                    name: Some(
                        self.executors
                            .get(&prev_exe)
                            .unwrap()
                            .get_name()
                            .to_string(),
                    ),
                    cat: Some("executor".to_string()),
                    pid: 0,
                    tid: Some(self.core_id as u32),
                    ts: log_event.timestamp.as_micros(),
                    args: HashMap::new(),
                });
                // Begin current
                let _ = self.trace_event_sender.send(TracingEvent::Begin {
                    name: self
                        .executors
                        .get(&curr_exe)
                        .unwrap()
                        .get_name()
                        .to_string(),
                    cat: Some("executor".to_string()),
                    pid: 0,
                    tid: Some(self.core_id as u32),
                    ts: log_event.timestamp.as_micros(),
                    args: HashMap::new(),
                });
            }
            _ => {} // same executor or both none
        }

        // Handle core-level events
        if log_event.core_id == self.core_id {
            // Check if Function Monitor Start event
            if let LogEventType::EventMonitorStart { function_name } = &log_event.event_type {
                // Send start event
                let _ = self.trace_event_sender.send(TracingEvent::Begin {
                    name: function_name.to_string(),
                    cat: Some("function_monitor".to_string()),
                    pid: 0,
                    tid: Some(self.core_id as u32),
                    ts: log_event.timestamp.as_micros(),
                    args: HashMap::new(),
                });
            }

            // Check if Function Monitor End event
            if let LogEventType::EventMonitorEnd { function_name } = &log_event.event_type {
                // Send end event
                let _ = self.trace_event_sender.send(TracingEvent::End {
                    name: Some(function_name.to_string()),
                    cat: Some("function_monitor".to_string()),
                    pid: 0,
                    tid: Some(self.core_id as u32),
                    ts: log_event.timestamp.as_micros(),
                    args: HashMap::new(),
                });
            }

            // Check if metric event
            if let LogEventType::EventMetric { name, value } = &log_event.event_type {
                // Try to link event to currently running executor
                let current_running_task = self
                    .executors
                    .values()
                    .find_map(|exe| exe.get_currently_running_task());
                let pid = current_running_task.map(|task| task.get_pid());

                // Send counter event
                let tracing_event = TracingEvent::Counter {
                    pid,
                    name: name.to_string(),
                    ts: log_event.timestamp.as_micros(),
                    args: HashMap::from([("value".to_string(), *value)]),
                    cat: None,
                };
                let _ = self.trace_event_sender.send(tracing_event);
            }
        }
    }
}
