use std::collections::HashMap;

use crossbeam::channel::{Receiver, Sender};

use crate::{
    elf_file::FirmwareAddressMap,
    perfetto_backend::trace_event::{InstantScope, TracingEvent},
    tracing::{core::CoreTracing, log_event::LogEvent, log_line::LogLine},
};

/// This container holds the state for the entire tracing system (represents something like the controller)
pub struct TracingInstance {
    firmware_addr_map: FirmwareAddressMap,

    trace_event_receiver: Receiver<TracingEvent>,
    trace_event_sender: Sender<TracingEvent>,

    cores: Vec<CoreTracing>,
}

impl TracingInstance {
    /// Create a new tracing instance
    pub fn new(firmware_addr_map: FirmwareAddressMap) -> Self {
        let (trace_event_sender, trace_event_receiver) = crossbeam::channel::unbounded();

        // send core overview metadata
        let _ = trace_event_sender.send(TracingEvent::Metadata {
            name: "process_name".to_string(),
            cat: None,
            args: HashMap::from([("name".to_string(), "CORE OVERVIEW".to_string())]),
            pid: 0,
            tid: None,
        });

        TracingInstance {
            firmware_addr_map,
            trace_event_receiver,
            trace_event_sender,
            cores: Vec::new(),
        }
    }

    pub fn get_trace_event_receiver(&self) -> Receiver<TracingEvent> {
        self.trace_event_receiver.clone()
    }

    /// Update the tracing instance (and everything underlying) with a new log event
    pub fn update(&mut self, log_event: &LogEvent) {
        // Check if we have a core for this event's core id
        let core_exists = self
            .cores
            .iter()
            .any(|core| core.get_core_id() == log_event.core_id);

        // Create core if it does not exist
        if !core_exists {
            self.cores.push(CoreTracing::new(
                log_event.core_id,
                self.firmware_addr_map.clone(),
                self.trace_event_sender.clone(),
            ));
        }

        // Update all cores
        for core in &mut self.cores {
            core.update(log_event);
            // TODO: Only update the core that matches the log event's core id???
        }
    }

    /// Adds a raw log line to the tracing instance (seperate plane)
    pub fn add_log_line(&mut self, log_line: &LogLine) {
        // Define event
        let event = TracingEvent::Instant {
            name: log_line.message.to_string(),
            cat: Some(log_line.level.to_string()),
            ts: log_line.timestamp.as_micros(),
            pid: None,
            tid: None,
            scope: InstantScope::Global,
            args: HashMap::from([("level".to_string(), log_line.level.to_string())]),
            cname: log_line.level.get_cname(),
        };

        // Send event
        let _ = self.trace_event_sender.send(event);
    }
}
