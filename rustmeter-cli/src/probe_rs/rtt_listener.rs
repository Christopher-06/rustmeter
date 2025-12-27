use std::time::Duration;

use anyhow::Context;
use crossbeam::channel::{Receiver, Sender};
use probe_rs::rtt::Rtt;

use crate::{flash_and_monitor::ChipMonitoringTool, probe_rs::atomic_session::AtomicSession};

/// This struct aggressively reads RTT data from the target to ensure that the RTT Channels do not overflow.
/// It spawns a thread that continuously reads from all up channels and sends the data to defmt_bytes or tracing_bytes
pub struct RttListener {
    defmt_bytes_recver: Receiver<Box<[u8]>>,
    tracing_bytes_recver: Receiver<Box<[u8]>>,
    error_recver: Receiver<anyhow::Error>,
}

impl RttListener {
    pub fn new(session: AtomicSession) -> anyhow::Result<Self> {
        let rtt = session.attach_rtt()?;

        let (defmt_bytes_sender, defmt_bytes_recver) = crossbeam::channel::unbounded();
        let (tracing_bytes_sender, tracing_bytes_recver) = crossbeam::channel::unbounded();
        let (error_sender, error_recver) = crossbeam::channel::unbounded();

        std::thread::spawn(move || {
            rtt_reader_thread(
                rtt,
                session,
                defmt_bytes_sender,
                tracing_bytes_sender,
                error_sender,
            )
        });

        Ok(Self {
            defmt_bytes_recver,
            tracing_bytes_recver,
            error_recver,
        })
    }
}

impl ChipMonitoringTool for RttListener {
    fn get_defmt_bytes_recver(&self) -> Receiver<Box<[u8]>> {
        self.defmt_bytes_recver.clone()
    }

    fn get_tracing_bytes_recver(&self) -> Receiver<Box<[u8]>> {
        self.tracing_bytes_recver.clone()
    }

    fn get_error_recver(&self) -> Receiver<anyhow::Error> {
        self.error_recver.clone()
    }
}

/// The RTT reader thread that continuously reads from the RTT up channels till the receivers are closed
fn rtt_reader_thread(
    mut rtt: Rtt,
    session: AtomicSession,
    defmt_bytes_recver: Sender<Box<[u8]>>,
    tracing_bytes_recver: Sender<Box<[u8]>>,
    error_recver: Sender<anyhow::Error>,
) {
    loop {
        // Read defmt channel
        let defmt_result = read_rtt_channel(&mut rtt, &session, 0);
        if route_reading_result(defmt_result, &defmt_bytes_recver, &error_recver) {
            break;
        }

        // Read tracing channel
        let tracing_result = read_rtt_channel(&mut rtt, &session, 1);
        if route_reading_result(tracing_result, &tracing_bytes_recver, &error_recver) {
            break;
        }

        // Avoid busy-waiting
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Route the reading result to the appropriate channel (data or error) and returning if the receiver is closed
fn route_reading_result(
    result: anyhow::Result<Box<[u8]>>,
    bytes_recver: &Sender<Box<[u8]>>,
    error_recver: &Sender<anyhow::Error>,
) -> bool {
    match result {
        Ok(bytes) => bytes_recver.send(bytes).is_err(),
        Err(e) => error_recver.send(e).is_err(),
    }
}

/// Read data from a specific RTT up channel
fn read_rtt_channel(
    rtt: &mut Rtt,
    session: &AtomicSession,
    channel_index: usize,
) -> anyhow::Result<Box<[u8]>> {
    // Get the channel
    let channel = rtt
        .up_channel(channel_index)
        .context(format!("Failed to get RTT up channel {}", channel_index))?;

    // Get the core
    let mut session_lock = session.lock();
    let mut core = session_lock.core(0)?;

    // Read data from the channel
    let mut buffer = vec![0u8; 1024];
    let bytes_read = channel.read(&mut core, &mut buffer).context(format!(
        "Failed to read from RTT up channel {}",
        channel_index
    ))?;

    buffer.truncate(bytes_read);
    Ok(buffer.into_boxed_slice())
}
