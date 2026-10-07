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

/// Result alias matching the `LogProvider` trait's associated result type.
/// `opensovd_core`'s own `log::Result` is not publicly re-exported.
type LogResult<T> = std::result::Result<T, LogError>;

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
        state.entries = vec![
            LogEntry {
                timestamp: now,
                context: LogContext::Rfc5424 {
                    host: Some("diag-host".into()),
                    process: Some("sovd_server_comp".into()),
                    pid: Some(1),
                },
                severity: LogSeverity::Info,
                msg: "sovd_server_comp started successfully".into(),
                href: None,
            },
            LogEntry {
                timestamp: now,
                context: LogContext::AutosarDlt {
                    session: Some("DIAG".into()),
                    session_id: Some("0x01".into()),
                    application_id: Some("SOVD".into()),
                    context_id: Some("MAIN".into()),
                    message_id: Some("0x0001".into()),
                },
                severity: LogSeverity::Debug,
                msg: "DLT transport initialised".into(),
                href: None,
            },
            LogEntry {
                timestamp: now,
                context: LogContext::Rfc5424 {
                    host: Some("diag-host".into()),
                    process: Some("sovd_server_comp".into()),
                    pid: Some(1),
                },
                severity: LogSeverity::Warn,
                msg: "Retrying connection to ECU (attempt 1)".into(),
                href: None,
            },
            LogEntry {
                timestamp: now,
                context: LogContext::Rfc5424 {
                    host: Some("diag-host".into()),
                    process: Some("sovd_server_comp".into()),
                    pid: Some(1),
                },
                severity: LogSeverity::Error,
                msg: "ECU response timeout after 5 s".into(),
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
    async fn entries(&self, filter: LogFilter) -> LogResult<Vec<LogEntry>> {
        let state = self.state.read().await;

        let result = state
            .entries
            .iter()
            .filter(|e| {
                // severity filter: include entries at or more severe than the threshold
                if let Some(min_sev) = filter.severity {
                    if e.severity > min_sev {
                        return false;
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
    async fn configuration(&self) -> LogResult<Vec<LogConfiguration>> {
        Ok(self.state.read().await.config.clone())
    }

    /// Replaces the per-context severity configuration.
    ///
    /// Returns `InvalidRequest` if the supplied list is empty.
    async fn configure(
        &self,
        configuration: Vec<LogConfiguration>,
    ) -> LogResult<()> {
        if configuration.is_empty() {
            return Err(LogError::InvalidRequest(
                "configuration list must not be empty".into(),
            ));
        }
        self.state.write().await.config = configuration;
        Ok(())
    }

    /// Resets the per-context severity configuration to `Info` for all
    /// known contexts.
    async fn reset_configuration(&self) -> LogResult<()> {
        let mut state = self.state.write().await;
        for cfg in &mut state.config {
            cfg.severity = LogSeverity::Info;
        }
        Ok(())
    }
}
