use anyhow::Result;
use serde_json::json;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::broadcast,
};

use crate::models::{EventsEnvelope, LogEntry, LogLevel, now_rfc3339};

use super::{AppState, MAX_LOG_LINES};

impl AppState {
    pub fn subscribe(&self) -> broadcast::Receiver<EventsEnvelope> {
        self.events.subscribe()
    }

    pub(crate) async fn append_log(&self, entry: LogEntry) -> Result<()> {
        {
            let mut inner = self.inner.write().await;
            if inner.logs.len() >= MAX_LOG_LINES {
                inner.logs.pop_front();
            }
            inner.logs.push_back(entry.clone());
        }
        self.emit(
            "log_appended",
            json!({ "instanceId": entry.instance_id, "level": entry.level, "source": entry.source }),
        )
        .await;
        Ok(())
    }

    pub(crate) fn spawn_log_task<T>(
        &self,
        stream: T,
        instance_id: String,
        source: String,
        level: LogLevel,
    ) where
        T: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        let app = self.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let level = classify_log_level(&level, &line);
                let _ = app
                    .append_log(LogEntry {
                        timestamp: now_rfc3339(),
                        level,
                        source: source.clone(),
                        message: line,
                        instance_id: instance_id.clone(),
                    })
                    .await;
            }
        });
    }

    pub(crate) async fn emit(&self, event_type: &str, payload: serde_json::Value) {
        let _ = self.events.send(EventsEnvelope {
            event_type: event_type.to_string(),
            timestamp: now_rfc3339(),
            payload,
        });
    }
}

fn classify_log_level(default_level: &LogLevel, line: &str) -> LogLevel {
    if !matches!(default_level, LogLevel::Error) {
        return default_level.clone();
    }

    let lower = line.to_ascii_lowercase();
    if lower.contains("error")
        || lower.contains("failed")
        || lower.contains("panic")
        || lower.contains("exception")
        || lower.contains("traceback")
    {
        LogLevel::Error
    } else {
        LogLevel::Warn
    }
}

#[cfg(test)]
mod tests {
    use super::classify_log_level;
    use crate::models::LogLevel;

    #[test]
    fn stderr_without_failure_keywords_becomes_warn() {
        assert_eq!(
            classify_log_level(&LogLevel::Error, "DeprecationWarning: util._extend"),
            LogLevel::Warn
        );
    }

    #[test]
    fn stderr_with_failure_keywords_stays_error() {
        assert_eq!(
            classify_log_level(&LogLevel::Error, "Unhandled error: boom"),
            LogLevel::Error
        );
    }
}
