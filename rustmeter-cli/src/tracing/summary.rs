use std::collections::HashMap;

use rustmeter_beacon::protocol::TypeDefinitionPayload;
use time::OffsetDateTime;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TracingSummary {
    start_datetime: OffsetDateTime,
    end_datetime: Option<OffsetDateTime>,
    /// Mapping from stream ID to error message. None if no error occurred.
    stream_errors: HashMap<u32, Option<String>>,
    /// Container to hold type definitions encountered during tracing
    typedefs: HashMap<u32, Vec<TypeDefinitionPayload>>,

    /// Indicates whether the summary has been updated since last write
    #[serde(skip)]
    updated: bool,
}

impl TracingSummary {
    pub fn new(start_datetime: OffsetDateTime) -> Self {
        Self {
            start_datetime,
            end_datetime: None,
            stream_errors: HashMap::new(),
            typedefs: HashMap::new(),
            updated: true,
        }
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
