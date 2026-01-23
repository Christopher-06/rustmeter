use anyhow::Result;
use defmt_decoder::{DecodeError, Table};
use defmt_parser::Level;
use std::{path::PathBuf, sync::OnceLock, time::Instant};

use crate::CoreInfo;

static TABLE: OnceLock<anyhow::Result<Table>> = OnceLock::new();

pub struct DefmtLine {
    pub core_origin: CoreInfo,
    pub message: String,
    pub defmt_timestamp_s: Option<f64>,
    pub pc_timestamp_us: u64,
    pub uc_timeticks: Option<u64>,
    pub level: Option<Level>,
}

pub struct DefmtDecoder {
    core: CoreInfo,
    decoder: Box<dyn defmt_decoder::StreamDecoder>,
    decoding_start: Instant,
    last_uc_timeticks: Option<u64>,
}

impl DefmtDecoder {
    pub fn new(core: CoreInfo, elf_path: &PathBuf, decoding_start: Instant) -> Result<Self> {
        // Create static table instance
        let _ = TABLE.get_or_init(|| read_defmt_table(elf_path));
        let table = TABLE.get().unwrap().as_ref();

        // Handle errors
        if let Err(e) = table {
            return Err(anyhow::anyhow!(
                "Failed to initialize defmt decoder for {core:?}: {e}"
            ));
        }

        Ok(Self {
            core,
            decoder: table.unwrap().new_stream_decoder(),
            decoding_start,
            last_uc_timeticks: None,
        })
    }

    /// Renew the decoder state
    pub fn renew(&mut self) {
        let table = TABLE.get().unwrap().as_ref().unwrap(); // unwrap okay since initialized in new()
        self.decoder = table.new_stream_decoder();
    }

    /// Feed defmt bytes into the decoder
    pub fn feed(&mut self, data: &Vec<u8>, uc_timeticks: u64) {
        self.decoder.received(data);
        self.last_uc_timeticks = Some(uc_timeticks);
    }

    /// Try to decode a defmt frame, returns None if no complete frame is available or an error occurs
    pub fn decode(&mut self) -> Option<DefmtLine> {
        match self.decoder.decode() {
            Ok(frame) => {
                // Print frame
                let str = format!("{}", frame.display(true));
                println!("{str}");

                // Extract defmt timestamp from str
                let defmt_timestamp_s = str
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<f64>().ok());

                // Return defmt line
                Some(DefmtLine {
                    core_origin: self.core,
                    message: frame.display_message().to_string(),
                    defmt_timestamp_s,
                    pc_timestamp_us: self.decoding_start.elapsed().as_micros() as u64,
                    uc_timeticks: self.last_uc_timeticks,
                    level: frame.level(),
                })
            }
            Err(DecodeError::UnexpectedEof) => None,
            Err(e) => {
                println!("Defmt decoding error on {:?}: {}", self.core, e);
                None
            }
        }
    }
}

fn read_defmt_table(elf_path: &PathBuf) -> anyhow::Result<defmt_decoder::Table> {
    // read elf file
    let bytes = std::fs::read(elf_path)
        .map_err(|e| anyhow::anyhow!("Failed to read elf file {elf_path:?}: {e}"))?;

    // parse defmt table
    let table = Table::parse(&bytes)
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse defmt table from elf file {elf_path:?}: {e}"
            )
        })?
        .ok_or_else(|| anyhow::anyhow!("No .defmt data found in elf file {elf_path:?}"))?;

    // Check if all indices have location info
    let locs = table.get_locations(&bytes)?;
    let all_locs = table.indices().all(|idx| locs.contains_key(&(idx as u64)));
    if !all_locs {
        println!("(BUG) location info is incomplete; it will be omitted from the output");
    }

    Ok(table)
}
