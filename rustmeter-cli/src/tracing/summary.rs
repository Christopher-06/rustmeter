use std::collections::HashMap;

use rustmeter_beacon::protocol::TypeDefinitionPayload;
use time::OffsetDateTime;

use crate::{cargo::elf_file::FirmwareAddressMap, cli::RunArgs};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TracingSummary {
    start_datetime: OffsetDateTime,
    end_datetime: Option<OffsetDateTime>,
    /// Mapping from stream ID to error message. None if no error occurred.
    stream_errors: HashMap<u32, Option<String>>,
    /// Container to hold type definitions encountered during tracing
    typedefs: HashMap<u32, Vec<TypeDefinitionPayload>>,
    /// Mapping from firmware addresses to symbol names
    fw_addr_map: FirmwareAddressMap,
    /// Chip name used during tracing
    chip: String,
    /// Indicates whether the firmware is a release build
    release: bool,

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
            stream_errors: HashMap::new(),
            typedefs: HashMap::new(),
            updated: true,
            fw_addr_map,
            chip: args.chip.clone(),
            release: args.release,
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

        let new_id = self.stream_errors.len() as u32;
        self.stream_errors.insert(new_id, None);
        self.typedefs.insert(new_id, Vec::new());
        new_id
    }

    /// Add an error message for a specific stream ID
    pub fn add_stream_error(&mut self, stream_id: u32, error: String) {
        self.updated = true;
        self.stream_errors.insert(stream_id, Some(error));
    }

    /// Add a type definition payload for a specific stream ID
    pub fn add_typedef(&mut self, stream_id: u32, typedef: TypeDefinitionPayload) {
        self.updated = true;
        self.typedefs
            .entry(stream_id)
            .or_insert_with(Vec::new)
            .push(typedef);
    }

    pub fn list_stream_ids(&self) -> impl Iterator<Item = &u32> {
        self.stream_errors.keys()
    }

    /// Get an iterator over type definitions for a specific stream ID
    pub fn iter_typedefs(&self, stream_id: u32) -> Option<impl Iterator<Item = &TypeDefinitionPayload> + Clone> {
        self.typedefs.get(&stream_id).map(|vec| vec.iter())
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
