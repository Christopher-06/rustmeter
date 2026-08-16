#![allow(dead_code)]
use std::collections::HashMap;

use anyhow::Context;
use rustmeter_beacon_core::{
    code_monitor::FunctionMetadata,
    protocol::{CustomPanicInfo, TypeDefinitionPayload},
};
use time::OffsetDateTime;

use crate::{
    analyze::clocks::{ClockReference, GlobalClockDefinition},
    cargo::elf_file::{FirmwareAddressMap, FirmwareInfo},
    cli::RunArgs,
    tracing::TracingDecodeError,
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct StreamContainer {
    /// Stream ID
    pub stream_id: u32,
    /// Type definitions encountered during tracing this stream
    pub typedefs: Vec<TypeDefinitionPayload>,
    /// Clock references encountered during tracing this stream
    pub clock_refs: Vec<ClockReference>,
    /// Error message if any error occurred during tracing this stream
    pub error: Option<String>,
    /// Panic info if any panic occurred during tracing this stream
    pub panic: Option<CustomPanicInfo>,
}

impl StreamContainer {
    pub fn new(stream_id: u32) -> Self {
        Self {
            stream_id,
            typedefs: Vec::new(),
            clock_refs: Vec::new(),
            error: None,
            panic: None,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TracingSummary {
    start_datetime: OffsetDateTime,
    end_datetime: Option<OffsetDateTime>,
    /// Mapping from stream ID to StreamContainer
    stream_data: HashMap<u32, StreamContainer>,
    /// Mapping from firmware addresses to symbol names (all)
    fw_addr_map: FirmwareAddressMap,
    /// Mapping from code monitor id to fn metadata from firmware section ".rustmeter_fn_metadata"
    fn_metadata: HashMap<u32, FunctionMetadata>,
    /// Chip name used during tracing
    chip: String,
    /// Indicates whether the firmware is a release build
    release: bool,
    /// ClockReference of first Core1 activation, if any
    second_core_startup: Option<ClockReference>,
    /// Global clock definition used during tracing, has to be the same for all streams
    global_clock_def: Option<GlobalClockDefinition>,
    /// Indicates whether the summary has been updated since last write
    #[serde(skip)]
    updated: bool,
}

impl TracingSummary {
    pub fn new(
        start_datetime: OffsetDateTime,
        fw_info: &FirmwareInfo,
        args: &RunArgs,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            start_datetime,
            end_datetime: None,
            stream_data: HashMap::new(),
            updated: true,
            fw_addr_map: fw_info.addr_symbol_map(),
            fn_metadata: extract_fn_metadata(fw_info)?,
            chip: args.chip.clone(),
            release: args.release,
            second_core_startup: None,
            global_clock_def: None,
        })
    }

    /// Get the chip name used during tracing
    pub fn chip(&self) -> &str {
        &self.chip
    }

    /// Check if the firmware is a release build
    pub fn is_release(&self) -> bool {
        self.release
    }

    /// Get the panic info if any panic occurred during tracing
    pub fn panic_info(&self, stream_id: u32) -> Option<&CustomPanicInfo> {
        self.stream_data
            .get(&stream_id)
            .and_then(|container| container.panic.as_ref())
    }

    /// Get the symbol name for a given firmware address, demangled.
    pub fn get_fw_symbol_name(&self, addr: u64) -> Option<String> {
        self.fw_addr_map
            .get(&addr)
            .map(|name| format!("{:#}", rustc_demangle::demangle(name)))
    }

    /// Set the end datetime of the tracing session
    pub fn set_end_datetime(&mut self, end_datetime: OffsetDateTime) {
        self.updated = true;
        self.end_datetime = Some(end_datetime);
    }

    /// Get the duration of the tracing session if end datetime was set
    pub fn get_tracing_duration(&self) -> Option<time::Duration> {
        self.end_datetime
            .and_then(|end| Some(end - self.start_datetime))
    }

    /// Get the entire mapping of code monitor ID to function metadata
    pub fn get_all_fn_metadata(&self) -> &HashMap<u32, FunctionMetadata> {
        &self.fn_metadata
    }

    /// Register a new stream and return its assigned ID
    pub fn register_new_stream(&mut self) -> u32 {
        self.updated = true;

        let new_id = self.stream_data.len() as u32;
        self.stream_data
            .insert(new_id, StreamContainer::new(new_id));
        new_id
    }

    /// Add an error message for a specific stream ID
    pub fn add_stream_error(&mut self, stream_id: u32, error: String) {
        self.updated = true;

        if let Some(container) = self.stream_data.get_mut(&stream_id) {
            container.error = Some(error);
        }
    }

    /// Set the panic info if a panic occurred during tracing. This can only be set once during a tracing session.
    pub fn set_panic_info(
        &mut self,
        stream_id: u32,
        info: CustomPanicInfo,
    ) -> Result<(), TracingDecodeError> {
        self.updated = true;
        if let Some(container) = self.stream_data.get_mut(&stream_id) {
            match &container.panic {
                Some(existing) => Err(TracingDecodeError::Unknown(anyhow::anyhow!(
                    "Panic info has already been set (existing: {existing} vs new: {info})",
                ))),
                None => {
                    container.panic = Some(info);
                    Ok(())
                }
            }
        } else {
            Err(TracingDecodeError::Unknown(anyhow::anyhow!(
                "Stream ID {stream_id} not found when setting panic info"
            )))
        }
    }

    pub fn count_stream_ids(&self) -> usize {
        self.stream_data.len()
    }

    /// Add a type definition payload for a specific stream ID. Automatically handles global clock definition
    pub fn add_typedef(&mut self, stream_id: u32, typedef: TypeDefinitionPayload) {
        self.updated = true;

        // Set global clock definition if applicable or check for consistency
        if let TypeDefinitionPayload::GlobalClockConfiguration { .. } = &typedef {
            let clock_def = GlobalClockDefinition::try_from(&typedef).unwrap();
            match &self.global_clock_def {
                Some(existing) => {
                    // Check for consistency
                    if existing.cpu_clock_hz != clock_def.cpu_clock_hz
                        || existing.tick_divider != clock_def.tick_divider
                    {
                        eprintln!(
                            "Warning: Inconsistent GlobalClockDefinition detected in stream {stream_id}! This can damage the accuracy of timestamp calculations."
                        );
                    }
                }
                None => {
                    // Set global clock definition first time
                    self.global_clock_def = Some(clock_def);
                }
            }
        }

        if let Some(container) = self.stream_data.get_mut(&stream_id) {
            container.typedefs.push(typedef);
        }
    }

    /// Add a clock reference for a specific stream ID.
    /// Sets second_core_startup if the clock reference is for Core1 and not already set.
    pub fn add_clock_reference(&mut self, stream_id: u32, clock_ref: ClockReference) {
        self.updated = true;

        // Set second_core_startup if applicable
        if clock_ref.core_id == 1 && self.second_core_startup.is_none() {
            self.second_core_startup = Some(clock_ref.clone());
        }

        // Add clock reference to stream container
        if let Some(container) = self.stream_data.get_mut(&stream_id) {
            container.clock_refs.push(clock_ref);
        }
    }

    /// Get the ClockReference of the second core activation, if any
    pub fn get_second_core_startup(&self) -> Option<&ClockReference> {
        self.second_core_startup.as_ref()
    }

    /// Get the global clock definition used during tracing, if any
    pub fn get_global_clock_definition(&self) -> Option<&GlobalClockDefinition> {
        self.global_clock_def.as_ref()
    }

    pub fn list_stream_ids(&self) -> impl Iterator<Item = &u32> {
        self.stream_data.keys()
    }

    pub fn get_stream_data(&self, stream_id: u32) -> Option<&StreamContainer> {
        self.stream_data.get(&stream_id)
    }

    pub fn get_all_stream_data(&self) -> impl Iterator<Item = &StreamContainer> {
        self.stream_data.values()
    }

    /// Write the summary to a JSON file at the specified path
    pub fn write_summary(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        if self.updated {
            self.updated = false;

            let json = serde_json::to_string_pretty(self)?;
            std::fs::write(path, json)?;
        }
        Ok(())
    }
}

/// Extract function metadata from the firmware info.
/// This looks for symbols in the ".rustmeter_fn_metadata" section of the firmware and deserializes
/// their string values into FunctionMetadata structs.
/// The resulting mapping is from code monitor ID to FunctionMetadata.
fn extract_fn_metadata(fw_info: &FirmwareInfo) -> anyhow::Result<HashMap<u32, FunctionMetadata>> {
    let symbols = fw_info
        .get_symbols_of_secetion(".rustmeter_fn_metadata")
        .context("Could not get symbols of section .rustmeter_fn_metadata")?;

    // Deserialize metadata and create mapping from monitor ID to metadata
    let mut fn_metadata_map = HashMap::new();
    for (monitor_id, metadata_str) in symbols {
        // zero is nullpointer in legacy systems
        if monitor_id > 0 {
            let metadata: FunctionMetadata = serde_json::from_str(&metadata_str).context(
                format!("Failed to parse function metadata for monitor ID {monitor_id}"),
            )?;
            fn_metadata_map.insert(monitor_id as u32, metadata);
        }
    }

    Ok(fn_metadata_map)
}
