use std::collections::HashMap;

use crossbeam::channel::{Receiver, Sender};
use rustmeter_beacon::{
    compressed_task_id,
    protocol::{EventPayload, TypeDefinitionPayload},
};

use crate::{
    elf_file::FirmwareAddressMap,
    logs::defmt_line::DefmtLine,
    perfetto_backend::trace_event::{InstantScope, TracingArgsMap, TracingEvent},
    tracing::{core::CoreTracing, trace_data_decoder::TracingItem},
};

pub struct TracingInstance {
    latest_timestamp: std::time::Duration,
    core_0: CoreTracing,
    core_1: CoreTracing,

    trace_event_tx: Sender<TracingEvent>,
    trace_event_rx: Receiver<TracingEvent>,

    fw_addr_map: FirmwareAddressMap,

    monitor_value_names: HashMap<u32, String>,
    monitor_code_names: HashMap<u32, String>,
}

impl TracingInstance {
    pub fn new(fw_addr_map: FirmwareAddressMap) -> Self {
        let (trace_event_tx, trace_event_rx) = crossbeam::channel::unbounded();

        // write initial metadata for core overview
        let _ = trace_event_tx.send(TracingEvent::Metadata {
            name: "process_name".to_string(),
            cat: Some("core_overview".to_string()),
            pid: u32::MAX - 1,
            tid: None,
            args: TracingArgsMap::from([("name".to_string(), "Core Overview".to_string())]),
        });
        let _ = trace_event_tx.send(TracingEvent::Metadata {
            name: "thread_name".to_string(),
            cat: Some("core_overview".to_string()),
            pid: u32::MAX - 1,
            tid: Some(1),
            args: TracingArgsMap::from([("name".to_string(), "Core 0".to_string())]),
        });
        let _ = trace_event_tx.send(TracingEvent::Metadata {
            name: "thread_name".to_string(),
            cat: Some("core_overview".to_string()),
            pid: u32::MAX - 1,
            tid: Some(2),
            args: TracingArgsMap::from([("name".to_string(), "Core 1".to_string())]),
        });

        let _ = trace_event_tx.send(TracingEvent::Metadata {
            name: "process_name".to_string(),
            cat: Some("core_overview".to_string()),
            pid: u32::MAX,
            tid: None,
            args: TracingArgsMap::from([("name".to_string(), "Metriken".to_string())]),
        });

        Self {
            latest_timestamp: std::time::Duration::from_secs(0),
            core_0: CoreTracing::new(0, trace_event_tx.clone()),
            core_1: CoreTracing::new(1, trace_event_tx.clone()),
            trace_event_tx,
            trace_event_rx,
            fw_addr_map,
            monitor_value_names: HashMap::new(),
            monitor_code_names: HashMap::new(),
        }
    }

    pub fn get_trace_event_receiver(&self) -> Receiver<TracingEvent> {
        self.trace_event_rx.clone()
    }

    fn handle_typedef(
        &mut self,
        typedef: &TypeDefinitionPayload,
        timestamp: std::time::Duration,
    ) -> anyhow::Result<()> {
        match typedef {
            TypeDefinitionPayload::EmbassyTaskCreated {
                executor_id_short,
                task_id,
                executor_id_long,
            } => {
                // write metadata about the executor
                let _ = self.trace_event_tx.send(TracingEvent::Metadata {
                    name: "process_name".to_string(),
                    cat: Some("embassy_executor".to_string()),
                    pid: (*executor_id_short).into(),
                    tid: Some(0),
                    args: {
                        let mut args = HashMap::new();
                        args.insert(
                            "executor_id_long".to_string(),
                            executor_id_long.clone().to_string(),
                        );
                        args.insert(
                            "name".into(),
                            self.fw_addr_map
                                .get_symbol_name(*executor_id_long as u64)
                                .unwrap_or(format!("Executor 0x{:X}", executor_id_long)),
                        );
                        TracingArgsMap::from(args)
                    },
                });

                // write metadata about the task
                let _ = self.trace_event_tx.send(TracingEvent::Metadata {
                    name: "thread_name".to_string(),
                    cat: Some("embassy_task".to_string()),
                    pid: (*executor_id_short).into(),
                    tid: Some(compressed_task_id(*task_id) as u32),
                    args: {
                        let mut args = HashMap::new();
                        args.insert("task_id_long".to_string(), task_id.clone().to_string());
                        args.insert(
                            "name".to_string(),
                            self.fw_addr_map
                                .get_symbol_name(*task_id as u64)
                                .unwrap_or(format!("Task 0x{:X}", task_id)),
                        );
                        TracingArgsMap::from(args)
                    },
                });

                // Feed both cores
                self.core_0.on_task_new_spawned(
                    *executor_id_short,
                    compressed_task_id(*task_id),
                    timestamp,
                )?;
                self.core_1.on_task_new_spawned(
                    *executor_id_short,
                    compressed_task_id(*task_id),
                    timestamp,
                )?;
                Ok(())
            }
            TypeDefinitionPayload::EmbassyTaskEnded {
                task_id,
                executor_id_short,
                executor_id_long,
            } => {
                // write metadata about the executor
                let _ = self.trace_event_tx.send(TracingEvent::Metadata {
                    name: self
                        .fw_addr_map
                        .get_symbol_name(*executor_id_long as u64)
                        .unwrap_or(format!("Executor 0x{:X}", executor_id_long)),
                    cat: Some("embassy_executor".to_string()),
                    pid: (*executor_id_short).into(),
                    tid: None,
                    args: {
                        let mut args = HashMap::new();
                        args.insert(
                            "executor_id_long".to_string(),
                            executor_id_long.clone().to_string(),
                        );
                        TracingArgsMap::from(args)
                    },
                });

                // write metadata about the task
                let _ = self.trace_event_tx.send(TracingEvent::Metadata {
                    name: self
                        .fw_addr_map
                        .get_symbol_name(*task_id as u64)
                        .unwrap_or(format!("Task 0x{:X}", task_id)),
                    cat: Some("embassy_task".to_string()),
                    pid: (*executor_id_short).into(),
                    tid: Some(compressed_task_id(*task_id) as u32),
                    args: {
                        let mut args = HashMap::new();
                        args.insert("task_id".to_string(), task_id.clone().to_string());
                        TracingArgsMap::from(args)
                    },
                });

                // Feed both cores
                self.core_0.on_task_end(
                    *executor_id_short,
                    compressed_task_id(*task_id),
                    timestamp,
                )?;
                self.core_1.on_task_end(
                    *executor_id_short,
                    compressed_task_id(*task_id),
                    timestamp,
                )?;
                Ok(())
            }
            TypeDefinitionPayload::ValueMonitor { value_id, name, .. } => {
                self.monitor_value_names
                    .insert(*value_id as u32, name.clone());
                Ok(())
            }
            TypeDefinitionPayload::FunctionMonitor {
                monitor_id,
                fn_address,
            } => {
                // Try to get function name from firmware address map
                let fn_name = self
                    .fw_addr_map
                    .get_symbol_name(*fn_address as u64)
                    .unwrap_or(format!("Function 0x{:X}", fn_address));
                self.monitor_code_names.insert(*monitor_id as u32, fn_name);
                Ok(())
            }
            TypeDefinitionPayload::ScopeMonitor { monitor_id, name } => {
                self.monitor_code_names
                    .insert(*monitor_id as u32, name.clone());
                Ok(())
            }
        }
    }

    fn handle_event_payloads(
        &mut self,
        payload: &EventPayload,
        timestamp: std::time::Duration,
    ) -> anyhow::Result<()> {
        match payload {
            EventPayload::EmbassyTaskReady {
                task_id,
                executor_id,
            } => {
                self.core_0
                    .on_task_ready(*executor_id, *task_id, timestamp)?;
                self.core_1
                    .on_task_ready(*executor_id, *task_id, timestamp)?;
                Ok(())
            }
            EventPayload::EmbassyExecutorPollStart { executor_id } => {
                self.core_0
                    .on_executor_poll_start(*executor_id, timestamp)?;
                self.core_1
                    .on_executor_poll_start(*executor_id, timestamp)?;
                Ok(())
            }
            EventPayload::EmbassyExecutorIdle { executor_id } => {
                self.core_0.on_executor_idle(*executor_id, timestamp)?;
                self.core_1.on_executor_idle(*executor_id, timestamp)?;
                Ok(())
            }
            EventPayload::EmbassyTaskExecBeginCore0 {
                task_id,
                executor_id,
            } => self
                .core_0
                .on_task_exec_begin(*executor_id, *task_id, timestamp),
            EventPayload::EmbassyTaskExecBeginCore1 {
                task_id,
                executor_id,
            } => self
                .core_1
                .on_task_exec_begin(*executor_id, *task_id, timestamp),
            EventPayload::EmbassyTaskExecEndCore0 { executor_id } => {
                self.core_0.on_task_exec_end(*executor_id, timestamp)
            }
            EventPayload::EmbassyTaskExecEndCore1 { executor_id } => {
                self.core_1.on_task_exec_end(*executor_id, timestamp)
            }
            EventPayload::TypeDefinition(typedef) => self.handle_typedef(typedef, timestamp),
            EventPayload::DataLossEvent { .. } => Ok(()),
            EventPayload::MonitorStartCore0 { monitor_id } => {
                if let Some(name) = self.monitor_code_names.get(&(*monitor_id as u32)) {
                    self.core_0.monitor_start(name.to_string(), timestamp);
                }
                Ok(())
            }
            EventPayload::MonitorStartCore1 { monitor_id } => {
                if let Some(name) = self.monitor_code_names.get(&(*monitor_id as u32)) {
                    self.core_1.monitor_start(name.to_string(), timestamp);
                }
                Ok(())
            }
            EventPayload::MonitorEndCore0 => {
                self.core_0.monitor_end(timestamp);
                Ok(())
            }
            EventPayload::MonitorEndCore1 => {
                self.core_1.monitor_end(timestamp);
                Ok(())
            }
            EventPayload::MonitorValue { value, value_id } => {
                if let Some(name) = self.monitor_value_names.get(&(*value_id as u32)) {
                    // write trace event for monitor value
                    let _ = self.trace_event_tx.send(TracingEvent::Counter {
                        pid: Some(u32::MAX),
                        name: name.clone(),
                        ts: timestamp.as_micros(),
                        args: HashMap::from([("value".to_string(), value.as_f64())]),
                        cat: None,
                    });
                }
                Ok(())
            }
        }
    }

    pub fn feed(&mut self, item: TracingItem, panic_by_resync: bool) {
        self.latest_timestamp = item.timestamp();

        // Handle data loss events separately to resynchronize
        if let EventPayload::DataLossEvent { dropped_events } = item.payload() {
            println!(
                "Data loss: dropped {} events. Try to resynchronize the trace",
                dropped_events
            );

            self.on_desynchronize(item.timestamp(), panic_by_resync);
            return;
        }

        // Feed events and check for state transition errors
        if let Err(e) = self.handle_event_payloads(item.payload(), item.timestamp()) {
            println!(
                "Error handling tracing item at {:?}: {:?}",
                item.timestamp(),
                e
            );
            self.on_desynchronize(item.timestamp(), panic_by_resync);
        }
    }

    pub fn add_defmt_log(&mut self, log_line: &DefmtLine) {
        // Define event
        if let Some(timestamp) = log_line.timestamp_us() {
            let event = TracingEvent::Instant {
                name: log_line.message().into(),
                cat: Some(log_line.log_level().to_string()),
                ts: timestamp as u128,
                pid: None,
                tid: None,
                scope: InstantScope::Global,
                args: HashMap::from([("level".to_string(), log_line.log_level().to_string())]),
                cname: log_line.log_level().get_cname(),
            };

            // Send event
            let _ = self.trace_event_tx.send(event);
        }
    }

    fn on_desynchronize(&mut self, timestamp: std::time::Duration, panic_by_resync: bool) {
        self.core_0.on_desynchronize(timestamp);
        self.core_1.on_desynchronize(timestamp);

        if panic_by_resync {
            panic!("Data loss detected in tracing data - resynchronization required");
        }

        // Clear Core Tracings to resynchronize
        self.core_0 = CoreTracing::new(0, self.trace_event_tx.clone());
        self.core_1 = CoreTracing::new(1, self.trace_event_tx.clone());
    }
}

impl Drop for TracingInstance {
    fn drop(&mut self) {
        // Notify cores about drop event
        self.core_0.on_drop(self.latest_timestamp);
        self.core_1.on_drop(self.latest_timestamp);
    }
}
