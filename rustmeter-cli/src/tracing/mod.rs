use std::fmt::Display;

use rustmeter_beacon_core::tracing::ReadTracingError;

mod buffered_writer;
mod defmt_buffer;
mod defmt_decoder;
mod request_agent;
mod timeseries_buffer;
mod trace_data_decoder;
pub mod tracing_item;

pub mod sink;
pub mod summary;

#[derive(Debug, Clone)]
pub enum CoreTracingData {
    Core0(Box<[u8]>),
    Core1(Box<[u8]>),
}

impl CoreTracingData {
    pub fn core_info(&self) -> crate::CoreInfo {
        match self {
            CoreTracingData::Core0(_) => crate::CoreInfo::Core0,
            CoreTracingData::Core1(_) => crate::CoreInfo::Core1,
        }
    }

    pub fn data(&self) -> &[u8] {
        match self {
            CoreTracingData::Core0(data) => data,
            CoreTracingData::Core1(data) => data,
        }
    }
}

#[derive(Debug)]
pub enum TracingDecodeError {
    InvalidData(ReadTracingError),
    DroppedEvents(u32),
    ChecksumMismatch,
    SerialPortError(std::io::Error),
    InvalidFrameID(u8),
    RttFailure(probe_rs::rtt::Error),
    ProbeRsError(probe_rs::Error),
    Unknown(anyhow::Error),
}

impl Display for TracingDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let err: anyhow::Error = self.into();
        write!(f, "{err}")
    }
}

impl From<&TracingDecodeError> for anyhow::Error {
    fn from(val: &TracingDecodeError) -> Self {
        match val {
            TracingDecodeError::InvalidData(e) => anyhow::anyhow!("Invalid tracing data: {e:?}"),
            TracingDecodeError::DroppedEvents(n) => {
                anyhow::anyhow!("Dropped {n} tracing events")
            }
            TracingDecodeError::ChecksumMismatch => anyhow::anyhow!("Checksum mismatch"),
            TracingDecodeError::SerialPortError(e) => {
                anyhow::anyhow!("Serial port error: {e}")
            }
            TracingDecodeError::InvalidFrameID(id) => {
                anyhow::anyhow!("Invalid frame ID: {id}")
            }
            TracingDecodeError::RttFailure(e) => {
                anyhow::anyhow!("RTT failure: {e}")
            }
            TracingDecodeError::ProbeRsError(e) => {
                anyhow::anyhow!("ProbeRs error: {e}")
            }
            TracingDecodeError::Unknown(e) => anyhow::anyhow!("Unknown error: {e}"),
        }
    }
}

impl From<ReadTracingError> for TracingDecodeError {
    fn from(err: ReadTracingError) -> Self {
        TracingDecodeError::InvalidData(err)
    }
}

impl From<probe_rs::rtt::Error> for TracingDecodeError {
    fn from(err: probe_rs::rtt::Error) -> Self {
        TracingDecodeError::RttFailure(err)
    }
}

impl From<probe_rs::Error> for TracingDecodeError {
    fn from(err: probe_rs::Error) -> Self {
        TracingDecodeError::ProbeRsError(err)
    }
}
