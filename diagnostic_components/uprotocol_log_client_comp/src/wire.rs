// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

//! Wire format for the `getLogs` uProtocol RPC.
//!
//! The toos-api codegen layer is deliberately not used. Instead the request and
//! response are serialized as JSON, so any ECU-side provider (including the C++
//! dummy app, which already uses nlohmann/json) can speak the same contract.
//!
//! Request mirrors `DummyLogProviderService::LogQuery`:
//!   `{ "severity": u8, "created_after": u64, "created_before": u64 }`
//!
//! `severity` uses the service enum encoding (kFatal=0 .. kDebug=4) and acts as
//! a *minimum* severity threshold. `created_after`/`created_before` are
//! millisecond Unix timestamps; `0` means "unbounded".

use serde::{Deserialize, Serialize};

/// Severity as encoded on the wire, matching the ECU service enum
/// (`DummyLogProviderService::LogSeverity`).
pub mod severity {
    pub const FATAL: u8 = 0;
    pub const ERROR: u8 = 1;
    pub const WARN: u8 = 2;
    pub const INFO: u8 = 3;
    pub const DEBUG: u8 = 4;
}

/// Query parameters sent to the ECU `getLogs` method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogQuery {
    /// Minimum severity threshold (service enum encoding).
    pub severity: u8,
    /// Lower time bound, Unix epoch milliseconds. `0` means unbounded.
    #[serde(default)]
    pub created_after: u64,
    /// Upper time bound, Unix epoch milliseconds. `0` means unbounded.
    #[serde(default)]
    pub created_before: u64,
}

/// A single log entry as returned by the ECU.
///
/// This is intentionally a flat, transport-friendly shape. The adapter maps it
/// onto `opensovd_core::LogEntry` with an RFC 5424 context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireLogEntry {
    /// Unix epoch milliseconds.
    pub timestamp: u64,
    /// Severity, service enum encoding.
    pub severity: u8,
    /// Log message text.
    pub msg: String,
    /// Originating host, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Originating process, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
}

/// A single capture-configuration rule as reported by the ECU `getConfig` RPC.
///
/// Flat, transport-friendly shape mirroring [`WireLogEntry`]: it carries an
/// optional RFC 5424 context (`host`/`process`) and the severity threshold the
/// ECU is capturing for that context. The adapter maps it onto
/// `opensovd_core::LogConfiguration` with an RFC 5424 context.
///
/// `severity` uses the service enum encoding (kFatal=0 .. kDebug=4) and is the
/// *capture threshold*: entries at that severity or more severe are retained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireLogConfig {
    /// Capture threshold, service enum encoding.
    pub severity: u8,
    /// Context host this rule applies to, if scoped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Context process this rule applies to, if scoped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
}

/// Response payload for `getLogs`.
///
/// Two shapes are accepted so the adapter is robust against a provider that
/// only returns a plain string (the current C++ dummy returns
/// `"dummy log entry"`):
/// - a structured array of [`WireLogEntry`], or
/// - a bare JSON string, treated as one `Info` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogResponse {
    Entries(Vec<WireLogEntry>),
    PlainText(String),
}

impl LogResponse {
    /// Parses a raw `getLogs` response body.
    ///
    /// Tries the structured array first, then falls back to a bare string.
    pub fn parse(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        if let Ok(entries) = serde_json::from_slice::<Vec<WireLogEntry>>(bytes) {
            return Ok(Self::Entries(entries));
        }
        // A plain JSON string, or raw text that isn't valid JSON at all.
        match serde_json::from_slice::<String>(bytes) {
            Ok(text) => Ok(Self::PlainText(text)),
            Err(_) => Ok(Self::PlainText(String::from_utf8_lossy(bytes).into_owned())),
        }
    }
}
