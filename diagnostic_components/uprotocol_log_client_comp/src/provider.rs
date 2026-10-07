// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

//! `opensovd_core::LogProvider` backed by the ECU `getLogs` uProtocol RPC.
//!
//! This is the integration seam for the SOVD server: swap the in-memory
//! `DiagLogProvider` for `UProtocolLogProvider` and every `GET /logs/entries`
//! request is served by calling the ECU over uProtocol — no toos-api.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use opensovd_core::{
    LogConfiguration, LogContext, LogEntry, LogError, LogFilter, LogProvider, LogResult,
    LogSeverity,
};

use crate::client::LogServiceClient;
use crate::wire::{self, LogQuery, LogResponse, WireLogEntry};

/// SOVD log provider that delegates to an ECU over uProtocol.
pub struct UProtocolLogProvider {
    client: Arc<LogServiceClient>,
}

impl UProtocolLogProvider {
    /// Wraps a ready [`LogServiceClient`].
    #[must_use]
    pub fn new(client: Arc<LogServiceClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl LogProvider for UProtocolLogProvider {
    /// Translates the SOVD filter into a `LogQuery`, calls the ECU, and maps
    /// the response back to SOVD `LogEntry` values.
    async fn entries(&self, filter: LogFilter) -> LogResult<Vec<LogEntry>> {
        let query = LogQuery {
            severity: filter
                .severity
                .map_or(wire::severity::DEBUG, severity_to_wire),
            created_after: datetime_to_millis(filter.created_after),
            created_before: datetime_to_millis(filter.created_before),
        };

        let response = self
            .client
            .get_logs(&query)
            .await
            .map_err(|e| LogError::Internal(e.to_string()))?;

        Ok(match response {
            LogResponse::Entries(entries) => {
                entries.into_iter().map(wire_entry_to_sovd).collect()
            }
            LogResponse::PlainText(text) => vec![plain_text_entry(text)],
        })
    }

    /// Capture configuration is owned by the ECU; without a dedicated RPC the
    /// client cannot read it, so an empty configuration is reported.
    async fn configuration(&self) -> LogResult<Vec<LogConfiguration>> {
        Ok(Vec::new())
    }

    /// Reconfiguring the ECU is not exposed through `getLogs`.
    async fn configure(
        &self,
        _configuration: Vec<LogConfiguration>,
    ) -> LogResult<()> {
        Err(LogError::InvalidRequest(
            "log configuration is not supported by the uProtocol log client".into(),
        ))
    }

    /// Resetting the ECU configuration is not exposed through `getLogs`.
    async fn reset_configuration(&self) -> LogResult<()> {
        Err(LogError::InvalidRequest(
            "log configuration is not supported by the uProtocol log client".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Mapping helpers
// ---------------------------------------------------------------------------

/// Maps a SOVD severity onto the service enum encoding (kFatal=0 .. kDebug=4).
fn severity_to_wire(severity: LogSeverity) -> u8 {
    match severity {
        LogSeverity::Fatal => wire::severity::FATAL,
        LogSeverity::Error => wire::severity::ERROR,
        LogSeverity::Warn => wire::severity::WARN,
        LogSeverity::Info => wire::severity::INFO,
        LogSeverity::Debug => wire::severity::DEBUG,
    }
}

/// Maps the service enum encoding back to a SOVD severity. Unknown values
/// degrade to `Info`.
fn wire_to_severity(value: u8) -> LogSeverity {
    match value {
        wire::severity::FATAL => LogSeverity::Fatal,
        wire::severity::ERROR => LogSeverity::Error,
        wire::severity::WARN => LogSeverity::Warn,
        wire::severity::DEBUG => LogSeverity::Debug,
        _ => LogSeverity::Info,
    }
}

/// Converts an optional UTC timestamp to Unix epoch milliseconds; `None` maps
/// to `0` ("unbounded" on the wire).
fn datetime_to_millis(ts: Option<DateTime<Utc>>) -> u64 {
    ts.map_or(0, |t| u64::try_from(t.timestamp_millis()).unwrap_or(0))
}

/// Converts Unix epoch milliseconds to a UTC timestamp, falling back to the
/// current time if the value is out of range.
fn millis_to_datetime(ms: u64) -> DateTime<Utc> {
    i64::try_from(ms)
        .ok()
        .and_then(|v| Utc.timestamp_millis_opt(v).single())
        .unwrap_or_else(Utc::now)
}

fn wire_entry_to_sovd(entry: WireLogEntry) -> LogEntry {
    LogEntry {
        timestamp: millis_to_datetime(entry.timestamp),
        context: LogContext::Rfc5424 {
            host: entry.host,
            process: entry.process,
            pid: None,
        },
        severity: wire_to_severity(entry.severity),
        msg: entry.msg,
        href: None,
    }
}

/// Wraps a plain-string response (e.g. the current C++ dummy's
/// `"dummy log entry"`) in a single `Info` entry so the SOVD contract still
/// returns structured data.
fn plain_text_entry(text: String) -> LogEntry {
    LogEntry {
        timestamp: Utc::now(),
        context: LogContext::Rfc5424 {
            host: None,
            process: None,
            pid: None,
        },
        severity: LogSeverity::Info,
        msg: text,
        href: None,
    }
}
