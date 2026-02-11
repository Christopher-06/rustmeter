use crate::protocol::raw_writers::{write_code_monitor_end, write_code_monitor_start};

/// Simple guard for code monitors. When dropped, it writes the end event for the monitor.
pub struct CodeMonitorGuard {
    active: bool,
}

impl CodeMonitorGuard {
    pub fn new() -> Self {
        Self { active: false }
    }

    /// Manually end the monitor before the guard is dropped. 
    /// Deactivates the guard and writes the end event.
    pub fn end(&mut self) {
        if self.active {
            write_code_monitor_end();
            self.active = false;
        }
    }

    pub fn activate(&mut self, monitor_idx: u16, state_idx: u16) {
        if self.active {
            // If already active, end the previous monitor first
            // TODO: This is only a temporary solution until we have proper nested monitors support
            self.end();
        }

        write_code_monitor_start(monitor_idx, state_idx);
        self.active = true;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }
}

impl Drop for CodeMonitorGuard {
    fn drop(&mut self) {
        self.end();
    }
}

#[cfg(feature = "std")]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct FunctionMetadata {
    pub disambiguator: u64,
    pub fn_name: String,
    pub is_async: bool,
    pub state_names: std::collections::HashMap<u16, String>, // state_index -> state_name
    pub state_transition_names:
        std::collections::HashMap<u16, std::collections::HashMap<u16, String>>, // (from_state, to_state) -> transition_name (in async fns this is before and after an .await and the string would be the name of the .await function)
}
