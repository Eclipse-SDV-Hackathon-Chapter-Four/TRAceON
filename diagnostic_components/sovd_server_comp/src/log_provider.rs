// Diagnostic log provider — implements opensovd_core::LogProvider.
//
// Stores log entries and per-context severity configuration in memory,
// protected by a tokio RwLock so the provider is Send + Sync.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use opensovd_core::{
    LogConfiguration, LogContext, LogEntry, LogError, LogFilter, LogProvider, LogSeverity,
};
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

#[derive(Default)]
struct State {
    entries: Vec<LogEntry>,
    config: Vec<LogConfiguration>,
}

// ---------------------------------------------------------------------------
// DiagLogProvider
// ---------------------------------------------------------------------------

/// In-memory SOVD log provider for the diagnostic component.
///
/// Seed entries are added at construction time so the endpoint returns
/// meaningful data immediately.  The configuration table starts with one
/// entry per supported context type at `Info` level.
#[derive(Clone)]
pub struct DiagLogProvider {
    state: Arc<RwLock<State>>,
}

impl DiagLogProvider {
    /// Creates a new provider pre-populated with seed log entries and a
    /// default configuration for each supported context type.
    #[must_use]
    pub fn new() -> Self {
        let provider = Self {
            state: Arc::new(RwLock::new(State::default())),
        };
        provider.seed();
        provider
    }

    fn seed(&self) {
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
                severity: LogSeverity::Info,
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
                msg: "PUT /v1/apps/diag-app/logs/config — severity threshold updated to Warn".into(),
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
                msg: "uProtocol transport not available — falling back to in-memory provider".into(),
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
        ];
    }

    /// Appends a new log entry at runtime (e.g. from internal diagnostics).
    pub async fn push_entry(&self, entry: LogEntry) {
        self.state.write().await.entries.push(entry);
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
    /// Returns all stored entries that match the given filter.
    ///
    /// Filtering rules:
    /// - `severity`: keep entries whose severity is **≤** the filter value
    ///   (i.e. at least as severe — Fatal < Error < Warn < Info < Debug).
    /// - `created_after`: keep entries with `timestamp > created_after`.
    /// - `created_before`: keep entries with `timestamp < created_before`.
    async fn entries(&self, filter: LogFilter) -> Result<Vec<LogEntry>, LogError> {
        let state = self.state.read().await;

        let result = state
            .entries
            .iter()
            .filter(|e| {
                // severity filter from query param
                if let Some(min_sev) = filter.severity {
                    if e.severity > min_sev {
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
                        if e.severity > min_sev {
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
            .collect();

        Ok(result)
    }

    /// Returns the current per-context severity configuration.
    async fn configuration(&self) -> Result<Vec<LogConfiguration>, LogError> {
        Ok(self.state.read().await.config.clone())
    }

    /// Upserts per-context severity configuration (merges by context type).
    ///
    /// Returns `InvalidRequest` if the supplied list is empty.
    async fn configure(
        &self,
        configuration: Vec<LogConfiguration>,
    ) -> Result<(), LogError> {
        if configuration.is_empty() {
            return Err(LogError::InvalidRequest(
                "configuration list must not be empty".into(),
            ));
        }
        let mut state = self.state.write().await;
        for incoming in configuration {
            if let Some(existing) = state.config.iter_mut().find(|c| context_matches(&c.context, &incoming.context)) {
                existing.severity = incoming.severity;
            } else {
                state.config.push(incoming);
            }
        }
        Ok(())
    }

    /// Resets the per-context severity configuration to `Info` for all
    /// known contexts.
    async fn reset_configuration(&self) -> Result<(), LogError> {
        let mut state = self.state.write().await;
        for cfg in &mut state.config {
            cfg.severity = LogSeverity::Info;
        }
        Ok(())
    }
}
