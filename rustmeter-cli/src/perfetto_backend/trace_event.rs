use anyhow::Context;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub enum InstantScope {
    #[serde(rename = "t")]
    Thread,
    #[serde(rename = "p")]
    Process,
    #[serde(rename = "g")]
    Global,
}

#[derive(Debug, Serialize)]
pub enum CName {
    #[serde(rename = "good")]
    Good,
    #[serde(rename = "terrible")]
    Terrible,
}

pub type TracingArgsMap<T> = std::collections::HashMap<String, T>;

#[derive(Debug, Serialize)]
// rename the enum variants to match the Perfetto trace event types
// ==> {ph = "X", "B", "E", "i", "C", "M", ...other types} in one dictionary (tagged enum)
#[serde(tag = "ph")]
#[allow(dead_code)]
pub enum TracingEvent {
    #[serde(rename = "X")]
    Complete {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cat: Option<String>,
        pid: u32,
        tid: u32,
        ts: u128,
        dur: u64,
        #[serde(skip_serializing_if = "TracingArgsMap::is_empty")]
        args: TracingArgsMap<String>,
    },
    #[serde(rename = "B")]
    Begin {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cat: Option<String>,
        ts: u128,
        pid: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        tid: Option<u32>,
        #[serde(skip_serializing_if = "TracingArgsMap::is_empty")]
        args: TracingArgsMap<String>,
    },
    #[serde(rename = "E")]
    End {
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cat: Option<String>,
        pid: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        tid: Option<u32>,
        ts: u128,
        #[serde(skip_serializing_if = "TracingArgsMap::is_empty")]
        args: TracingArgsMap<String>,
    },
    #[serde(rename = "i")]
    Instant {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cat: Option<String>,
        ts: u128,
        #[serde(skip_serializing_if = "Option::is_none")]
        pid: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tid: Option<u32>,
        #[serde(rename = "s")]
        scope: InstantScope,
        #[serde(skip_serializing_if = "TracingArgsMap::is_empty")]
        args: TracingArgsMap<String>,
        cname: CName,
    },
    #[serde(rename = "C")]
    Counter {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cat: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pid: Option<u32>,
        ts: u128,
        #[serde(skip_serializing_if = "TracingArgsMap::is_empty")]
        args: TracingArgsMap<f64>,
    },
    #[serde(rename = "M")]
    Metadata {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cat: Option<String>,
        pid: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        tid: Option<u32>,
        #[serde(skip_serializing_if = "TracingArgsMap::is_empty")]
        args: TracingArgsMap<String>,
    },
}

impl TracingEvent {
    /// Convert the tracing event to a JSON string for Perfetto
    pub fn to_json(&self) -> anyhow::Result<String> {
        serde_json::to_string(self).context("Failed to serialize TracingEvent to JSON")
    }
}
