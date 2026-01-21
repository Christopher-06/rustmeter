use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crossbeam::channel::Sender;
use rustmeter_beacon::protocol::{EventPayload, Request, TypeDefinitionPayload};

use crate::{CoreInfo, tracing::tracing_item::TracingItem};

/// Period between subsequent requests for successfully retrieved data
const REQUEST_PERIOD: Duration = Duration::from_secs(3);
/// Timeout duration before retrying to send a request
const RETRY_TIMEOUT: Duration = Duration::from_millis(1000);
/// Dead Time between start / reset and first request send
const DEAD_TIME: Duration = Duration::from_millis(200);
/// Pause duration between sending subsequent messages
const MESSAGE_PAUSE: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, Default)]
struct RequestState {
    send_time: Option<Instant>,
    recvd_time: Option<Instant>,
}

impl RequestState {
    pub fn new() -> Self {
        Self {
            send_time: None,
            recvd_time: None,
        }
    }

    pub fn with_send_time(mut self, send_time: Instant) -> Self {
        self.send_time = Some(send_time);
        self
    }

    pub fn with_recvd_time(mut self, recvd_time: Instant) -> Self {
        self.recvd_time = Some(recvd_time);
        self
    }

    pub fn should_retry(&self) -> bool {
        // 1. Nothing recvd yet and retry timeout elapsed
        // 2. Something recvd but request period elapsed since last recv
        match self.recvd_time {
            Some(recvd) => recvd.elapsed() >= REQUEST_PERIOD,
            None => self.send_time.is_none_or(|t| t.elapsed() >= RETRY_TIMEOUT),
        }
    }
}

// TODO: What happens when no Core1 exists and not answering? Stale requests? ==> create a priority system with last recvd time, last req time and retry counts

/// Agent container to manage requests for tracing-related data e.g. core clock references when needed
pub struct RequestAgent {
    start_time: Instant,
    last_sent: Instant,
    req_sender: Sender<Request>,

    /// Last core clock reference request states
    last_core_clock_ref: HashMap<CoreInfo, RequestState>,
    /// Last global clock definition request states
    last_clock_def: RequestState,
}

impl RequestAgent {
    pub fn new(req_sender: Sender<Request>) -> Self {
        Self {
            start_time: Instant::now(),
            last_sent: Instant::now(),
            req_sender,
            last_core_clock_ref: HashMap::new(),
            last_clock_def: RequestState::new(),
        }
    }

    fn can_send_message(&self) -> bool {
        self.last_sent.elapsed() >= MESSAGE_PAUSE
    }

    /// Reset all request states and restart requesting
    pub fn reset(&mut self) {
        self.last_core_clock_ref.clear();
        self.last_clock_def = RequestState::new();
        self.start_time = Instant::now();
    }

    /// Send core clock reference request to a specific core
    fn send_core_clock_ref_request(&mut self, core: CoreInfo) -> anyhow::Result<()> {
        let req = Request::GetCoreClockReference { core_id: core.id() };
            self.req_sender.send(req)?;
            self.last_sent = Instant::now();

            // update state
            self.last_core_clock_ref
                .insert(core, RequestState::new().with_send_time(Instant::now()));
        Ok(())
    }

    /// Request core clock reference if needed
    fn request_core_clock_ref(&mut self, core: CoreInfo) -> anyhow::Result<()> {
        let state = self.last_core_clock_ref.get(&core);
        let do_send = state.is_none_or(RequestState::should_retry);

        if do_send && self.can_send_message() {
            self.send_core_clock_ref_request(core)?;
        }

        Ok(())
    }

    /// Request global clock definition if needed
    fn request_clock_definition(&mut self) -> anyhow::Result<()> {
        let do_send = self.last_clock_def.should_retry();

        if do_send && self.can_send_message() {
            // send request
            let req = Request::GetGlobalClockDefinition;
            self.req_sender.send(req)?;
            self.last_sent = Instant::now();

            // update state
            self.last_clock_def = RequestState::new().with_send_time(Instant::now());
        }

        Ok(())
    }

    fn handle_typedef(&mut self, typedef: &TypeDefinitionPayload) -> anyhow::Result<()> {
        match typedef {
            // Update core clock reference state
            TypeDefinitionPayload::CoreClockReference { core_id, .. } => {
                let core = core_id.try_into()?;

                if core == CoreInfo::Core1 {
                    // When Core1 clock ref received, also request Core0 state!
                    let item = self
                        .last_core_clock_ref
                        .entry(CoreInfo::Core0)
                        .or_insert(RequestState::new());
                    *item = item.with_recvd_time(Instant::now());
                }

                // Get or insert item
                let item = self
                    .last_core_clock_ref
                    .entry(core)
                    .or_insert(RequestState::new());
                *item = item.with_recvd_time(Instant::now());
            }
            TypeDefinitionPayload::GlobalClockConfiguration { .. } => {
                self.last_clock_def = self.last_clock_def.with_recvd_time(Instant::now());
            }
            _ => {} // ignore other typedefs
        }
        Ok(())
    }

    pub fn handle_tracing_item(&mut self, item: &TracingItem) -> anyhow::Result<()> {
        match item.payload() {
            EventPayload::TypeDefinition(typedef) => self.handle_typedef(typedef)?,
            _ => {}
        }

        // Poll agent tasks
        if Instant::now().duration_since(self.start_time) >= DEAD_TIME {
            self.request_core_clock_ref(CoreInfo::Core1)?;
            self.request_core_clock_ref(CoreInfo::Core0)?; // core0 will automatically be requested when Core1 Info received
            self.request_clock_definition()?;
        }

        Ok(())
    }
}
