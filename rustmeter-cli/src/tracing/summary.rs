use std::collections::HashMap;

use rustmeter_beacon_core::protocol::TypeDefinitionPayload;
use time::OffsetDateTime;

use crate::{
    analyze::clocks::{ClockReference, GlobalClockDefinition},
    cargo::elf_file::FirmwareAddressMap,
    cli::RunArgs,
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
}

impl StreamContainer {
    pub fn new(stream_id: u32) -> Self {
        Self {
            stream_id,
            typedefs: Vec::new(),
            clock_refs: Vec::new(),
            error: None,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TracingSummary {
    start_datetime: OffsetDateTime,
    end_datetime: Option<OffsetDateTime>,
    /// Mapping from stream ID to StreamContainer
    stream_data: HashMap<u32, StreamContainer>,
    /// Mapping from firmware addresses to symbol names
    fw_addr_map: FirmwareAddressMap,
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
        fw_addr_map: FirmwareAddressMap,
        args: &RunArgs,
    ) -> Self {
        Self {
            start_datetime,
            end_datetime: None,
            stream_data: HashMap::new(),
            updated: true,
            fw_addr_map,
            chip: args.chip.clone(),
            release: args.release,
            second_core_startup: None,
            global_clock_def: None,
        }
    }

    /// Get the chip name used during tracing
    pub fn chip(&self) -> &str {
        &self.chip
    }

    /// Check if the firmware is a release build
    pub fn is_release(&self) -> bool {
        self.release
    }

    /// Get the symbol name for a given firmware address
    pub fn get_fw_symbol_name(&self, addr: u64) -> Option<String> {
        self.fw_addr_map.get_symbol_name(addr)
    }

    /// Set the end datetime of the tracing session
    pub fn set_end_datetime(&mut self, end_datetime: OffsetDateTime) {
        self.updated = true;
        self.end_datetime = Some(end_datetime);
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
