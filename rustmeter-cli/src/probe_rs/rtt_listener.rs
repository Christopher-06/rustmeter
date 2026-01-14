use std::time::Duration;

use crossbeam::channel::{Receiver, Sender};
use probe_rs::rtt::Rtt;
use rustmeter_beacon::{buffer::BufferWriter, protocol::Request};

use crate::{
    commands::flash_and_monitor::ChipMonitoringTool,
    probe_rs::atomic_session::AtomicSession,
    tracing::{CoreTracingData, TracingDecodeError},
};

/// This struct aggressively reads RTT data from the target to ensure that the RTT Channels do not overflow.
/// It spawns a thread that continuously reads from all up channels and sends the data to defmt_bytes or tracing_bytes
pub struct RttListener {
    tracing_bytes_recver: Receiver<Result<CoreTracingData, TracingDecodeError>>,
    req_sender: Sender<Request>,
}

impl RttListener {
    pub fn new(session: AtomicSession, rtt_address: Option<u64>) -> anyhow::Result<Self> {
        // Attach to RTT
        let rtt = match rtt_address {
            Some(addr) => {
                match session.attach_rtt_region(addr) {
                    Ok(rtt) => rtt,
                    Err(_) => {
                        // fallback to normal attach
                        println!(
                            "Warning: Could not attach to RTT at address 0x{:X}, falling back to normal RTT attach",
                            addr
                        );
                        session.attach_rtt()?
                    }
                }
            }
            None => session.attach_rtt()?, // scan whole memory for RTT (slow)
        };

        let (tracing_bytes_sender, tracing_bytes_recver) = crossbeam::channel::unbounded();
        let (req_sender, req_recver) = crossbeam::channel::unbounded();

        std::thread::spawn(move || {
            rtt_reader_thread(rtt, session, tracing_bytes_sender, req_recver)
        });

        Ok(Self {
            tracing_bytes_recver,
            req_sender,
        })
    }
}

impl ChipMonitoringTool for RttListener {
    fn get_tracing_bytes_recver(&self) -> Receiver<Result<CoreTracingData, TracingDecodeError>> {
        self.tracing_bytes_recver.clone()
    }

    fn get_request_sender(&self) -> Sender<Request> {
        self.req_sender.clone()
    }
}

/// The RTT reader thread that continuously reads from the RTT up channels till the receivers are closed
fn rtt_reader_thread(
    mut rtt: Rtt,
    session: AtomicSession,
    tracing_bytes_sender: Sender<Result<CoreTracingData, TracingDecodeError>>,
    req_recver: Receiver<Request>,
) {
    // Check if core1 exists
    let core1_exists = rtt.up_channels().len() > 1;

    let mut buffer = vec![0u8; 4096];
    loop {
        // Read tracing channel core0
        let mut tracing_size_core0 = 0;
        let tracing_result = read_rtt_channel(&mut rtt, &mut buffer, &session, 0).map(|n| {
            tracing_size_core0 = n;
            CoreTracingData::Core0(buffer[..n].to_vec().into_boxed_slice())
        });
        let mut ch_err = tracing_bytes_sender.send(tracing_result).is_err();

        // Read tracing channel core1
        let mut tracing_size_core1 = 0;
        if core1_exists {
            let tracing_result = read_rtt_channel(&mut rtt, &mut buffer, &session, 1).map(|n| {
                tracing_size_core1 = n;
                CoreTracingData::Core1(buffer[..n].to_vec().into_boxed_slice())
            });
            ch_err = tracing_bytes_sender.send(tracing_result).is_err() || ch_err;
        }

        if ch_err {
            // Receiver has been closed, exit thread
            break;
        }

        // Handle requests
        if let Ok(req) = req_recver.try_recv() {
            if let Err(e) = send_request(req, &mut rtt, &session) {
                // Currently just log the error
                eprintln!("Error sending RTT request: {:?}", e);
            }
        }

        // Wait a bit if no data was read to avoid busy-waiting,
        // else do not sleep to ensure low latency and reread as soon as possible
        if tracing_size_core0 + tracing_size_core1 == 0 {
            // No data read, avoid busy-waiting
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn send_request(req: Request, rtt: &mut Rtt, session: &AtomicSession) -> anyhow::Result<()> {
    // Serialize
    let mut writer = BufferWriter::new();
    req.write_bytes(&mut writer);

    // Get first downchannel
    let channel = rtt
        .down_channel(0)
        .ok_or(probe_rs::rtt::Error::MissingChannel(0))?;

    // send data
    let mut session_lock = session.lock();
    let mut core = session_lock.core(0)?;
    channel.write(&mut core, writer.as_slice())?;

    Ok(())
}

/// Read data from a specific RTT up channel
fn read_rtt_channel(
    rtt: &mut Rtt,
    buffer: &mut [u8],
    session: &AtomicSession,
    channel_index: usize,
) -> Result<usize, TracingDecodeError> {
    // Get the channel
    let channel = rtt
        .up_channel(channel_index)
        .ok_or(TracingDecodeError::RttFailure(
            probe_rs::rtt::Error::MissingChannel(channel_index),
        ))?;

    // Get the core
    let mut session_lock = session.lock();
    let mut core = session_lock.core(0)?;

    // Read data from the channel
    let n = channel.read(&mut core, buffer)?;

    Ok(n)
}
