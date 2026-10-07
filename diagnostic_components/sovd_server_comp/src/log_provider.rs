// Diagnostic log provider — implements opensovd_core::LogProvider.
//
// Reads log entries from the realtime hardware and stores the per-context
// configuration locally, protected by a tokio RwLock.

use std::{collections::VecDeque, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream;
use opensovd_core::{
    LogConfiguration, LogContext, LogEntry, LogError, LogFilter, LogProvider, LogSeverity,
    LogStream,
};
use serde_json::Value;
use tokio::sync::{broadcast, RwLock};

type Result<T> = std::result::Result<T, LogError>;
const MAX_BUFFERED_ENTRIES: usize = 1024;

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

#[derive(Default)]
struct State {
    entries: VecDeque<LogEntry>,
    config: Vec<LogConfiguration>,
}

// ---------------------------------------------------------------------------
// DiagLogProvider
// ---------------------------------------------------------------------------

/// SOVD log provider fed by the TraceOn-specific REST log sink.
#[derive(Clone)]
pub struct DiagLogProvider {
    state: Arc<RwLock<State>>,
    events: broadcast::Sender<LogEntry>,
}

impl DiagLogProvider {
    /// Creates an empty provider with default per-context configuration.
    #[must_use]
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(256);
        let provider = Self {
            state: Arc::new(RwLock::new(State::default())),
            events,
        };
        provider.initialize_configuration();
        provider
    }

    fn initialize_configuration(&self) {
        // Use try_write — we are still in synchronous construction so the lock
        // is uncontested.
        let Ok(mut state) = self.state.try_write() else {
            return;
        };

        // Default per-context severity configuration
        state.config = vec![
            LogConfiguration {
                context: LogContext::Rfc5424 {
                    host: Some("diag-host".into()),
                    process: Some("sovd_server_comp".into()),
                    pid: None,
                },
                severity: LogSeverity::Info,
            },
            LogConfiguration {
                context: LogContext::AutosarDlt {
                    session: Some("DIAG".into()),
                    session_id: None,
                    application_id: Some("SOVD".into()),
                    context_id: Some("MAIN".into()),
                    message_id: None,
                },
                severity: LogSeverity::DltInfo,
            },
        ];

        // Seed log entries covering all severity levels
        let now = Utc::now();
        let t = |secs_ago: i64| now - chrono::Duration::seconds(secs_ago);
        state.entries = vec![
            // ── Application lifecycle ──────────────────────────────
            LogEntry {
                timestamp: t(60),
                context: LogContext::Rfc5424 {
                    host: Some("diag-host".into()),
                    process: Some("diag-app".into()),
                    pid: Some(42),
                },
                severity: LogSeverity::Info,
                msg: "diag-app initialised — SOVD topology registered".into(),
                href: None,
            },
            LogEntry {
                timestamp: t(58),
                context: LogContext::Rfc5424 {
                    host: Some("diag-host".into()),
                    process: Some("diag-app".into()),
                    pid: Some(42),
                },
                severity: LogSeverity::Info,
                msg: "SOVD server listening on 127.0.0.1:8080".into(),
                href: None,
            },
            // ── SOVD session events ────────────────────────────────
            LogEntry {
                timestamp: t(50),
                context: LogContext::AutosarDlt {
                    session: Some("DIAG".into()),
                    session_id: Some("0x01".into()),
                    application_id: Some("SOVD".into()),
                    context_id: Some("SESS".into()),
                    message_id: Some("0x0010".into()),
                },
                severity: LogSeverity::Info,
                msg: "SOVD diagnostic session opened by client 192.168.1.10".into(),
                href: None,
            },
            LogEntry {
                timestamp: t(45),
                context: LogContext::AutosarDlt {
                    session: Some("DIAG".into()),
                    session_id: Some("0x01".into()),
                    application_id: Some("SOVD".into()),
                    context_id: Some("SESS".into()),
                    message_id: Some("0x0011".into()),
                },
                severity: LogSeverity::Debug,
                msg: "Security access granted — level 0x01".into(),
                href: None,
            },
            // ── Diagnostic requests ────────────────────────────────
            LogEntry {
                timestamp: t(40),
                context: LogContext::Rfc5424 {
                    host: Some("diag-host".into()),
                    process: Some("diag-app".into()),
                    pid: Some(42),
                },
                severity: LogSeverity::Info,
                msg: "GET /v1/apps/diag-app/logs/entries — 200 OK (4 entries)".into(),
                href: None,
            },
            LogEntry {
                timestamp: t(35),
                context: LogContext::Rfc5424 {
                    host: Some("diag-host".into()),
                    process: Some("diag-app".into()),
                    pid: Some(42),
                },
                severity: LogSeverity::Info,
                msg: "PUT /v1/apps/diag-app/logs/config — severity threshold updated to Warn"
                    .into(),
                href: None,
            },
            // ── Application warnings ───────────────────────────────
            LogEntry {
                timestamp: t(30),
                context: LogContext::Rfc5424 {
                    host: Some("diag-host".into()),
                    process: Some("diag-app".into()),
                    pid: Some(42),
                },
                severity: LogSeverity::Warn,
                msg: "uProtocol transport not available — falling back to in-memory provider"
                    .into(),
                href: None,
            },
            LogEntry {
                timestamp: t(25),
                context: LogContext::AutosarDlt {
                    session: Some("DIAG".into()),
                    session_id: Some("0x01".into()),
                    application_id: Some("SOVD".into()),
                    context_id: Some("DATA".into()),
                    message_id: Some("0x0020".into()),
                },
                severity: LogSeverity::Warn,
                msg: "Data provider response time exceeded 200 ms threshold".into(),
                href: None,
            },
            // ── Application errors ─────────────────────────────────
            LogEntry {
                timestamp: t(20),
                context: LogContext::Rfc5424 {
                    host: Some("diag-host".into()),
                    process: Some("diag-app".into()),
                    pid: Some(42),
                },
                severity: LogSeverity::Error,
                msg: "ECU response timeout after 5 s — request aborted".into(),
                href: None,
            },
            LogEntry {
                timestamp: t(15),
                context: LogContext::AutosarDlt {
                    session: Some("DIAG".into()),
                    session_id: Some("0x01".into()),
                    application_id: Some("SOVD".into()),
                    context_id: Some("FAULT".into()),
                    message_id: Some("0x0030".into()),
                },
                severity: LogSeverity::Error,
                msg: "DTC P0300 — random misfire detected, freeze frame captured".into(),
                href: None,
            },
            // ── Fatal ──────────────────────────────────────────────
            LogEntry {
                timestamp: t(5),
                context: LogContext::Rfc5424 {
                    host: Some("diag-host".into()),
                    process: Some("diag-app".into()),
                    pid: Some(42),
                },
                severity: LogSeverity::Fatal,
                msg: "Unrecoverable fault in brake controller — safe state activated".into(),
                href: None,
            },
            // ── Recovery ──────────────────────────────────────────
            LogEntry {
                timestamp: now,
                context: LogContext::Rfc5424 {
                    host: Some("diag-host".into()),
                    process: Some("diag-app".into()),
                    pid: Some(42),
                },
                severity: LogSeverity::Info,
                msg: "System recovery complete — diagnostic session resumed".into(),
                href: None,
            },
        ]
        .into_iter()
        .collect();
    }

    /// Stores one hardware event received through the REST sink.
    pub async fn ingest_hardware_event(&self, value: Value) -> Result<()> {
        let entry = parse_hardware_entry(&value)?;
        let mut state = self.state.write().await;
        if state.entries.len() == MAX_BUFFERED_ENTRIES {
            state.entries.pop_front();
        }
        state.entries.push_back(entry.clone());
        let _ = self.events.send(entry);
        Ok(())
    }
}

impl Default for DiagLogProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns true if a config context matches a log entry context by type.
fn context_matches(cfg: &LogContext, entry: &LogContext) -> bool {
    matches!(
        (cfg, entry),
        (LogContext::Rfc5424 { .. }, LogContext::Rfc5424 { .. })
            | (LogContext::AutosarDlt { .. }, LogContext::AutosarDlt { .. })
    ) || matches!(
        (cfg, entry),
        (LogContext::Custom { context_type: a, .. }, LogContext::Custom { context_type: b, .. })
        if a == b
    )
}

// ---------------------------------------------------------------------------
// LogProvider implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl LogProvider for DiagLogProvider {
    /// Returns buffered hardware entries matching the requested filter.
    ///
    /// Filtering rules:
    /// - `severity`: keep entries at or above the requested severity.
    /// - `created_after`: keep entries with `timestamp > created_after`.
    /// - `created_before`: keep entries with `timestamp < created_before`.
    async fn entries(&self, filter: LogFilter) -> Result<Vec<LogEntry>> {
        validate_filter(&filter)?;

        let state = self.state.read().await;
        Ok(state
            .entries
            .iter()
            .filter(|e| {
                // severity filter from query param
                if let Some(min_sev) = filter.severity {
                    if !severity_at_least(e.severity, min_sev) {
                        return false;
                    }
                }
                // context-based severity filter from stored config
                if !state.config.is_empty() {
                    let ctx_sev = state.config.iter().find_map(|cfg| {
                        if context_matches(&cfg.context, &e.context) {
                            Some(cfg.severity)
                        } else {
                            None
                        }
                    });
                    if let Some(min_sev) = ctx_sev {
                        if !severity_at_least(e.severity, min_sev) {
                            return false;
                        }
                    }
                }
                if let Some(after) = filter.created_after {
                    if e.timestamp <= after {
                        return false;
                    }
                }
                if let Some(before) = filter.created_before {
                    if e.timestamp >= before {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect())
    }

    async fn stream(&self, filter: LogFilter) -> Result<LogStream> {
        validate_filter(&filter)?;
        let receiver = self.events.subscribe();
        let stream = stream::unfold(receiver, move |mut receiver| {
            let filter = filter.clone();
            async move {
                loop {
                    match receiver.recv().await {
                        Ok(entry) if matches_filter(&entry, &filter) => {
                            return Some((Ok(entry), receiver));
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(count)) => {
                            return Some((
                                Err(LogError::Internal(format!(
                                    "live log stream lagged by {count} entries"
                                ))),
                                receiver,
                            ));
                        }
                        Err(broadcast::error::RecvError::Closed) => return None,
                    }
                }
            }
        });
        Ok(Box::pin(stream))
    }

    /// Returns the current per-context severity configuration.
    async fn configuration(&self) -> Result<Vec<LogConfiguration>> {
        Ok(self.state.read().await.config.clone())
    }

    /// Upserts per-context severity configuration (merges by context type).
    ///
    /// Returns `InvalidRequest` if the supplied list is empty.
    async fn configure(&self, configuration: Vec<LogConfiguration>) -> Result<()> {
        if configuration.is_empty() {
            return Err(LogError::InvalidRequest(
                "configuration list must not be empty".into(),
            ));
        }
        let mut state = self.state.write().await;
        for incoming in configuration {
            if let Some(existing) = state
                .config
                .iter_mut()
                .find(|c| context_matches(&c.context, &incoming.context))
            {
                existing.severity = incoming.severity;
            } else {
                state.config.push(incoming);
            }
        }
        Ok(())
    }

    /// Resets the per-context severity configuration to `Info` for all
    /// known contexts.
    async fn reset_configuration(&self) -> Result<()> {
        let mut state = self.state.write().await;
        for cfg in &mut state.config {
            cfg.severity = match &cfg.context {
                LogContext::AutosarDlt { .. } => LogSeverity::DltInfo,
                _ => LogSeverity::Info,
            };
        }
        Ok(())
    }
}

fn validate_filter(filter: &LogFilter) -> Result<()> {
    if let (Some(after), Some(before)) = (filter.created_after, filter.created_before) {
        if after >= before {
            return Err(LogError::InvalidRequest(
                "created-after must be earlier than created-before".into(),
            ));
        }
    }
    Ok(())
}

fn matches_filter(entry: &LogEntry, filter: &LogFilter) -> bool {
    filter
        .severity
        .is_none_or(|severity| severity_at_least(entry.severity, severity))
        && filter
            .created_after
            .is_none_or(|after| entry.timestamp > after)
        && filter
            .created_before
            .is_none_or(|before| entry.timestamp < before)
}

/// Returns whether `value` is at least as severe as `threshold` according to
/// the SOVD severity ordering.
fn severity_at_least(value: LogSeverity, threshold: LogSeverity) -> bool {
    severity_rank(value) <= severity_rank(threshold)
}

fn severity_rank(severity: LogSeverity) -> u8 {
    match severity {
        LogSeverity::Fatal | LogSeverity::DltFatal => 0,
        LogSeverity::Error | LogSeverity::DltError => 1,
        LogSeverity::Warn | LogSeverity::DltWarn => 2,
        LogSeverity::Info | LogSeverity::DltInfo => 3,
        LogSeverity::Debug | LogSeverity::DltDebug => 4,
    }
}

fn parse_hardware_entry(value: &Value) -> Result<LogEntry> {
    let timestamp = required_string(value, "timestamp")?
        .parse::<DateTime<Utc>>()
        .map_err(|error| LogError::Internal(format!("invalid log timestamp: {error}")))?;
    let severity = parse_severity(required_string(value, "severity")?)?;
    let msg = required_string(value, "msg")?.to_owned();
    let context = parse_context(value.get("context"));
    let severity = normalize_context_severity(&context, severity);
    let href = value
        .get("href")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);

    Ok(LogEntry {
        timestamp,
        context,
        severity,
        msg,
        href,
    })
}

fn normalize_context_severity(context: &LogContext, severity: LogSeverity) -> LogSeverity {
    if !matches!(context, LogContext::AutosarDlt { .. }) {
        return severity;
    }

    match severity {
        LogSeverity::Fatal | LogSeverity::DltFatal => LogSeverity::DltFatal,
        LogSeverity::Error | LogSeverity::DltError => LogSeverity::DltError,
        LogSeverity::Warn | LogSeverity::DltWarn => LogSeverity::DltWarn,
        LogSeverity::Info | LogSeverity::DltInfo => LogSeverity::DltInfo,
        LogSeverity::Debug | LogSeverity::DltDebug => LogSeverity::DltDebug,
    }
}

fn required_string<'a>(value: &'a serde_json::Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            LogError::Internal(format!(
                "hardware log entry is missing string field '{field}'"
            ))
        })
}

fn parse_severity(value: &str) -> Result<LogSeverity> {
    match value.to_ascii_lowercase().as_str() {
        "fatal" | "critical" => Ok(LogSeverity::Fatal),
        "error" | "err" => Ok(LogSeverity::Error),
        "warn" | "warning" => Ok(LogSeverity::Warn),
        "info" | "notice" => Ok(LogSeverity::Info),
        "debug" | "trace" => Ok(LogSeverity::Debug),
        "dlt_fatal" => Ok(LogSeverity::DltFatal),
        "dlt_error" => Ok(LogSeverity::DltError),
        "dlt_warn" | "dlt_warning" => Ok(LogSeverity::DltWarn),
        "dlt_info" => Ok(LogSeverity::DltInfo),
        "dlt_debug" | "dlt_verbose" => Ok(LogSeverity::DltDebug),
        _ => Err(LogError::Internal(format!(
            "unknown hardware log severity '{value}'"
        ))),
    }
}

fn parse_context(value: Option<&serde_json::Value>) -> LogContext {
    if let Some(context_type) = value.and_then(serde_json::Value::as_str) {
        return LogContext::Custom {
            context_type: context_type.into(),
            attributes: serde_json::Map::new(),
        };
    }

    let Some(value) = value.and_then(serde_json::Value::as_object) else {
        return LogContext::Custom {
            context_type: "hardware".into(),
            attributes: serde_json::Map::new(),
        };
    };

    let context_type = value
        .get("type")
        .or_else(|| value.get("context_type"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("hardware");

    match context_type {
        "RFC5424" => LogContext::Rfc5424 {
            host: string_field(value, "host"),
            process: string_field(value, "process"),
            pid: value.get("pid").and_then(serde_json::Value::as_i64),
        },
        "AUTOSAR_DLT" => LogContext::AutosarDlt {
            session: string_field(value, "session"),
            session_id: string_field(value, "session_id"),
            application_id: string_field(value, "application_id"),
            context_id: string_field(value, "context_id"),
            message_id: string_field(value, "message_id"),
        },
        _ => LogContext::Custom {
            context_type: context_type.into(),
            attributes: value.clone().into_iter().collect(),
        },
    }
}

fn string_field(value: &serde_json::Map<String, serde_json::Value>, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}
