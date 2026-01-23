use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crossbeam::channel::{Receiver, Sender};
use rustmeter_beacon::protocol::{EventPayload, Request};
use time::OffsetDateTime;

use crate::{
    CoreInfo,
    cargo::elf_file::FirmwareAddressMap,
    cli::RunArgs,
    tracing::{
        CoreTracingData, TracingDecodeError, buffered_writer::BufferedWriter,
        defmt_buffer::DefmtLogBuffer, defmt_decoder::DefmtDecoder, request_agent::RequestAgent,
        summary::TracingSummary, timeseries_buffer::TimeSeriesItemBuffer,
        trace_data_decoder::TraceDataDecoder, tracing_item::TracingItem,
    },
};

pub struct TracingSink {
    folder: PathBuf,
    current_stream_id: u32,
    summary: TracingSummary,
    tracing_bytes_recver: Receiver<Result<CoreTracingData, TracingDecodeError>>,
    req_agent: RequestAgent,

    timeseries_writer: BufferedWriter<TimeSeriesItemBuffer>,
    defmt_writer: BufferedWriter<DefmtLogBuffer>,
    defmt_decoder: [DefmtDecoder; 2],
    trace_decoder: [TraceDataDecoder; 2],
}

impl TracingSink {
    pub fn new(
        folder: PathBuf,
        elf_path: &PathBuf,
        tracing_bytes_recver: Receiver<Result<CoreTracingData, TracingDecodeError>>,
        req_sender: Sender<Request>,
        args: &RunArgs,
    ) -> anyhow::Result<Self> {
        let start = Instant::now();

        let fw_addr_map = FirmwareAddressMap::new_from_elf_path(elf_path)?;
        let mut summary = TracingSummary::new(OffsetDateTime::now_utc(), fw_addr_map, args);
        let current_stream_id = summary.register_new_stream();

        Ok(Self {
            summary,
            tracing_bytes_recver,
            req_agent: RequestAgent::new(req_sender),
            timeseries_writer: BufferedWriter::new(
                folder.clone(),
                "timeseries".into(),
                current_stream_id,
            )?,
            defmt_writer: BufferedWriter::new(
                folder.clone(),
                "defmt_logs".into(),
                current_stream_id,
            )?,
            current_stream_id,
            folder,
            trace_decoder: [
                TraceDataDecoder::new(CoreInfo::Core0, start),
                TraceDataDecoder::new(CoreInfo::Core1, start),
            ],
            defmt_decoder: [
                DefmtDecoder::new(CoreInfo::Core0, elf_path, start)?,
                DefmtDecoder::new(CoreInfo::Core1, elf_path, start)?,
            ],
        })
    }

    fn handle_defmt_bytes(
        &mut self,
        core: CoreInfo,
        data: &Vec<u8>,
        uc_timeticks: u64,
    ) -> Result<(), TracingDecodeError> {
        // Feed defmt decoder
        let defmt_decoder = match core {
            CoreInfo::Core0 => &mut self.defmt_decoder[0],
            CoreInfo::Core1 => &mut self.defmt_decoder[1],
        };
        defmt_decoder.feed(data, uc_timeticks);

        // Decode available defmt frames
        while let Some(line) = defmt_decoder.decode() {
            self.defmt_writer
                .feed(&line)
                .map_err(TracingDecodeError::Unknown)?;
        }

        Ok(())
    }

    // Handle a single valid tracing item
    fn handle_tracing_item(&mut self, item: TracingItem) -> Result<(), TracingDecodeError> {
        self.req_agent
            .handle_tracing_item(&item)
            .map_err(TracingDecodeError::Unknown)?;

        // Add clock reference if possible
        if let Ok(clock_ref) = (&item).try_into() {
            self.summary
                .add_clock_reference(self.current_stream_id, clock_ref);
        }

        // Handle payload specifically
        let payload = item.payload();
        if let EventPayload::TypeDefinition(typedef) = payload {
            // Handle type definition
            self.summary
                .add_typedef(self.current_stream_id, typedef.clone());

            Ok(())
        } else if let EventPayload::DefmtData { data, .. } = payload {
            // Handle defmt data
            self.handle_defmt_bytes(item.core(), data, item.uc_timeticks())
        } else if let EventPayload::DataLossEvent { dropped_events } = payload {
            // Handle dropped events
            Err(TracingDecodeError::DroppedEvents(*dropped_events))
        } else {
            // Feed to timeseries writer
            self.timeseries_writer
                .feed(&item)
                .map_err(TracingDecodeError::Unknown)
        }
    }

    /// Try to decode a single tracing item from given core, returns true if more data could be available
    fn decode_single_tracing(&mut self, core: CoreInfo) -> Result<bool, TracingDecodeError> {
        let trace_decoder = match core {
            CoreInfo::Core0 => &mut self.trace_decoder[0],
            CoreInfo::Core1 => &mut self.trace_decoder[1],
        };

        match trace_decoder.decode_single()? {
            Some(item) => {
                self.handle_tracing_item(item)?;
                Ok(true)
            }
            None => Ok(false), // No more data
        }
    }

    fn handle_new_bytes(&mut self, data: CoreTracingData) -> Result<(), TracingDecodeError> {
        // Feed appropriate trace decoder
        let core = data.core_info();
        {
            let trace_decoder = match core {
                CoreInfo::Core0 => &mut self.trace_decoder[0],
                CoreInfo::Core1 => &mut self.trace_decoder[1],
            };
            trace_decoder.feed(data.data());
        }

        // Try to decode all available tracing items
        while self.decode_single_tracing(core)? {}

        Ok(())
    }

    fn handle_error(&mut self, error: TracingDecodeError) -> anyhow::Result<()> {
        // Error occured, log it in summary and reset decoder state with new stream id
        self.summary
            .add_stream_error(self.current_stream_id, error.to_string());
        self.current_stream_id = self.summary.register_new_stream();

        self.req_agent.reset();

        // Create new writers for the new stream id
        self.timeseries_writer = BufferedWriter::new(
            self.folder.clone(),
            "timeseries".into(),
            self.current_stream_id,
        )?;
        self.defmt_writer = BufferedWriter::new(
            self.folder.clone(),
            "defmt_logs".into(),
            self.current_stream_id,
        )?;

        // Renew decoders
        for decoder in &mut self.trace_decoder {
            decoder.renew();
        }
        for decoder in &mut self.defmt_decoder {
            decoder.renew();
        }

        println!("TracingSink: Renewed decoders after error.");
        Ok(())
    }

    /// Continuously sink tracing bytes until stop is requested. Automatically handles channel closure
    /// or TracingDecodeErrors. Errors while handling errors are returned to the caller.
    pub fn sink_bytes(&mut self, stop: Arc<AtomicBool>) -> anyhow::Result<()> {
        while !stop.load(Ordering::Relaxed) {
            // Read next tracing data
            let trace_data = match self
                .tracing_bytes_recver
                .recv_timeout(Duration::from_millis(100))
            {
                Ok(data) => data,
                Err(crossbeam::channel::RecvTimeoutError::Timeout) => {
                    // continue loop
                    continue;
                }
                Err(crossbeam::channel::RecvTimeoutError::Disconnected) => {
                    // Channel closed, exit
                    return Ok(());
                }
            };

            // Handle result
            match trace_data {
                Ok(data) => {
                    if let Err(e) = self.handle_new_bytes(data) {
                        println!("Warning: Tracing decode error: {e}");
                        self.handle_error(e)?; // error while handling new bytes
                    }
                }
                Err(e) => {
                    self.handle_error(e)?; // decode error
                }
            }
        }

        Ok(())
    }

    /// Finalize the tracing sink, writes summary and flushes writers. Need to be called in main to allow after analyzing the result but also
    /// called in Drop implementation to ensure data is flushed on panic or early return. In main an error can be handled so no analyzing get started
    /// with incomplete data.
    pub fn finalize(mut self) -> anyhow::Result<()> {
        self.summary.set_end_datetime(OffsetDateTime::now_utc());
        self.finalize_internal()
    }

    /// Internal finalize function to be used also in Drop implementation. Does nothing when called multiple times without updates in between.
    fn finalize_internal(&mut self) -> anyhow::Result<()> {
        self.timeseries_writer.flush()?;
        self.defmt_writer.flush()?;
        self.summary
            .write_summary(&self.folder.join("summary.json"))?;
        Ok(())
    }
}

/// Finalize tracing sink on drop to ensure data is flushed
impl Drop for TracingSink {
    fn drop(&mut self) {
        if let Err(e) = self.finalize_internal() {
            println!("[ERROR] Finalizing TracingSink: {e}");
        }
    }
}
