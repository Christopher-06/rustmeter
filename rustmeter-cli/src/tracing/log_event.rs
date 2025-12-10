use crate::{time::EmbassyTime, tracing::log_line::LogLine};

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum LogEventType {
    EventEmbassyTaskExecEnd { executor_id: u32, task_id: u32 },
    EventEmbassyTaskReadyBegin { executor_id: u32, task_id: u32 },
    EventEmbassyTaskExecBegin { executor_id: u32, task_id: u32 },
    EventEmbassyTaskEnd { executor_id: u32, task_id: u32 },
    EventEmbassyTaskNew { executor_id: u32, task_id: u32 },
    EventEmbassyExecutorIdle { executor_id: u32 },
    EventEmbassyPollStart { executor_id: u32 },
    EventMonitorStart { function_name: String },
    EventMonitorEnd { function_name: String },
    EventMetric { name: String, value: f64 },
}

impl LogEventType {
    pub fn get_task_id(&self) -> Option<u32> {
        match self {
            LogEventType::EventEmbassyTaskExecEnd { task_id, .. } => Some(*task_id),
            LogEventType::EventEmbassyTaskReadyBegin { task_id, .. } => Some(*task_id),
            LogEventType::EventEmbassyTaskExecBegin { task_id, .. } => Some(*task_id),
            LogEventType::EventEmbassyTaskEnd { task_id, .. } => Some(*task_id),
            LogEventType::EventEmbassyTaskNew { task_id, .. } => Some(*task_id),
            _ => None,
        }
    }

    pub fn get_executor_id(&self) -> Option<u32> {
        match self {
            LogEventType::EventEmbassyTaskExecEnd { executor_id, .. } => Some(*executor_id),
            LogEventType::EventEmbassyTaskReadyBegin { executor_id, .. } => Some(*executor_id),
            LogEventType::EventEmbassyTaskExecBegin { executor_id, .. } => Some(*executor_id),
            LogEventType::EventEmbassyTaskEnd { executor_id, .. } => Some(*executor_id),
            LogEventType::EventEmbassyTaskNew { executor_id, .. } => Some(*executor_id),
            LogEventType::EventEmbassyExecutorIdle { executor_id } => Some(*executor_id),
            LogEventType::EventEmbassyPollStart { executor_id } => Some(*executor_id),
            _ => None,
        }
    }

    pub fn try_from_name_and_param(
        name: &str,
        params_map: &std::collections::HashMap<&str, &str>,
    ) -> anyhow::Result<LogEventType> {
        match name {
            "EVENT_EMBASSY_TASK_EXEC_END" => Ok(LogEventType::EventEmbassyTaskExecEnd {
                executor_id: params_map
                    .get("executor_id")
                    .ok_or(anyhow::anyhow!("Missing parameter 'executor_id'"))?
                    .parse()?,
                task_id: params_map
                    .get("task_id")
                    .ok_or(anyhow::anyhow!("Missing parameter 'task_id'"))?
                    .parse()?,
            }),
            "EVENT_EMBASSY_TASK_READY_BEGIN" => Ok(LogEventType::EventEmbassyTaskReadyBegin {
                executor_id: params_map
                    .get("executor_id")
                    .ok_or(anyhow::anyhow!("Missing parameter 'executor_id'"))?
                    .parse()?,
                task_id: params_map
                    .get("task_id")
                    .ok_or(anyhow::anyhow!("Missing parameter 'task_id'"))?
                    .parse()?,
            }),
            "EVENT_EMBASSY_TASK_EXEC_BEGIN" => Ok(LogEventType::EventEmbassyTaskExecBegin {
                executor_id: params_map
                    .get("executor_id")
                    .ok_or(anyhow::anyhow!("Missing parameter 'executor_id'"))?
                    .parse()?,
                task_id: params_map
                    .get("task_id")
                    .ok_or(anyhow::anyhow!("Missing parameter 'task_id'"))?
                    .parse()?,
            }),
            "EVENT_EMBASSY_TASK_END" => Ok(LogEventType::EventEmbassyTaskEnd {
                executor_id: params_map
                    .get("executor_id")
                    .ok_or(anyhow::anyhow!("Missing parameter 'executor_id'"))?
                    .parse()?,
                task_id: params_map
                    .get("task_id")
                    .ok_or(anyhow::anyhow!("Missing parameter 'task_id'"))?
                    .parse()?,
            }),
            "EVENT_EMBASSY_TASK_NEW" => Ok(LogEventType::EventEmbassyTaskNew {
                executor_id: params_map
                    .get("executor_id")
                    .ok_or(anyhow::anyhow!("Missing parameter 'executor_id'"))?
                    .parse()?,
                task_id: params_map
                    .get("task_id")
                    .ok_or(anyhow::anyhow!("Missing parameter 'task_id'"))?
                    .parse()?,
            }),
            "EVENT_EMBASSY_EXECUTOR_IDLE" => Ok(LogEventType::EventEmbassyExecutorIdle {
                executor_id: params_map
                    .get("executor_id")
                    .ok_or(anyhow::anyhow!("Missing parameter 'executor_id'"))?
                    .parse()?,
            }),
            "EVENT_EMBASSY_POLL_START" => Ok(LogEventType::EventEmbassyPollStart {
                executor_id: params_map
                    .get("executor_id")
                    .ok_or(anyhow::anyhow!("Missing parameter 'executor_id'"))?
                    .parse()?,
            }),
            "EVENT_MONITOR_START" => Ok(LogEventType::EventMonitorStart {
                function_name: params_map
                    .get("function_name")
                    .ok_or(anyhow::anyhow!("Missing parameter 'function_name'"))?
                    .to_string(),
            }),
            "EVENT_MONITOR_END" => Ok(LogEventType::EventMonitorEnd {
                function_name: params_map
                    .get("function_name")
                    .ok_or(anyhow::anyhow!("Missing parameter 'function_name'"))?
                    .to_string(),
            }),
            "EVENT_METRIC" => Ok(LogEventType::EventMetric {
                name: params_map
                    .get("name")
                    .ok_or(anyhow::anyhow!("Missing parameter 'name'"))?
                    .to_string(),
                value: params_map
                    .get("value")
                    .ok_or(anyhow::anyhow!("Missing parameter 'value'"))?
                    .parse()?,
            }),
            _ => Err(anyhow::anyhow!("Unknown LogEvent type: {name}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogEvent {
    pub timestamp: EmbassyTime,
    pub core_id: u8,
    pub event_type: LogEventType,
}
impl LogEvent {
    pub fn new(timestamp: EmbassyTime, core_id: u8, event_type: LogEventType) -> Self {
        LogEvent {
            timestamp,
            core_id,
            event_type,
        }
    }

    /// Parse a LogEvent from a LogLine
    pub fn from_log_line(log_line: &LogLine) -> anyhow::Result<LogEvent> {
        // Trim and check prefix
        let message = log_line.message.trim();
        if !message.starts_with("@") {
            return Err(anyhow::anyhow!(
                "LogEvent message does not start with '@': {message}"
            ));
        }

        // Find event type name and parameters
        let opening_bracket = message.find('(').ok_or(anyhow::anyhow!(
            "Invalid LogEvent message format (found no opening bracket): {message}"
        ))?;
        let closing_bracket = message.find(')').ok_or(anyhow::anyhow!(
            "Invalid LogEvent message format (found no closing bracket): {message}"
        ))?;
        let event_type_name = &message[1..opening_bracket];
        let params_str = &message[opening_bracket + 1..closing_bracket];

        // Parse parameters into a map
        let mut params_map = std::collections::HashMap::new();
        for param in params_str.split(',') {
            let parts: Vec<&str> = param.splitn(2, '=').collect();
            if parts.len() == 2 {
                params_map.insert(parts[0].trim(), parts[1].trim());
            }
        }

        // Get parameters
        let core_id = params_map
            .get("core_id")
            .ok_or(anyhow::anyhow!("Missing parameter 'core_id'"))?
            .parse()?;
        let event_type = LogEventType::try_from_name_and_param(event_type_name, &params_map)?;

        Ok(LogEvent::new(log_line.timestamp, core_id, event_type))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_log_event_from_log_line() {
        let log_line = LogLine::from_str("1.812321 [DEBUG] @EVENT_EMBASSY_TASK_EXEC_BEGIN(executor_id=1073610704, core_id=0, task_id=1073425160)").unwrap();
        let log_event = LogEvent::from_log_line(&log_line).unwrap();

        assert_eq!(log_event.core_id, 0);
        match log_event.event_type {
            LogEventType::EventEmbassyTaskExecBegin {
                executor_id,
                task_id,
            } => {
                assert_eq!(executor_id, 1073610704);
                assert_eq!(task_id, 1073425160);
            }
            e => panic!("Unexpected LogEventType: {e:?}"),
        }
    }
}
