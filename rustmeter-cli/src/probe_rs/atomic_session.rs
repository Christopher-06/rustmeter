use std::sync::{Arc, Mutex};

use anyhow::Context;
use probe_rs::{
    Session,
    rtt::{Rtt, ScanRegion},
};

#[derive(Clone)]
pub struct AtomicSession {
    session: Arc<Mutex<Session>>,
}

impl AtomicSession {
    pub fn new(session: Session) -> Self {
        Self {
            session: Arc::new(Mutex::new(session)),
        }
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, Session> {
        self.session.lock().unwrap()
    }

    /// Attach RTT to the session's core 0
    pub fn attach_rtt(&self) -> anyhow::Result<Rtt> {
        let mut session = self.lock();
        let mut core = session.core(0)?;

        probe_rs::rtt::Rtt::attach(&mut core).context("Failed to attach RTT to Core")
    }

    /// Attach RTT to the session's core 0 at a specific address
    pub fn attach_rtt_region(&self, addr: u64) -> anyhow::Result<Rtt> {
        let mut session = self.lock();
        let mut core = session.core(0)?;

        let region = ScanRegion::Exact(addr);
        Rtt::attach_region(&mut core, &region).context("No RTT Region in Scan Region")
    }
}

impl From<Session> for AtomicSession {
    fn from(session: Session) -> Self {
        Self::new(session)
    }
}
