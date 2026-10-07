// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

//! Wiring for the uProtocol-backed log source.
//!
//! Builds a [`UProtocolLogProvider`] that fetches logs from an ECU over the
//! uProtocol `getLogs` RPC. For the self-contained demo this also starts an
//! in-process ECU stand-in (over up-rust's `LocalTransport`) that mirrors the
//! `swc_dummy_log_provider` behaviour, so `GET /logs/entries` returns data
//! through a real RPC round-trip without any external transport.
//!
//! In a deployment, replace `LocalTransport` with the POSIX/Zenoh transport
//! and drop the stand-in: the ECU side is the real `swc_dummy_log_provider`.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use up_rust::communication::{
    InMemoryRpcServer, RequestHandler, RpcServer, ServiceInvocationError, UPayload,
};
use up_rust::local_transport::LocalTransport;
use up_rust::{StaticUriProvider, UAttributes, UPayloadFormat, UTransport};
use uprotocol_log_client_comp::{
    wire::{LogQuery, WireLogConfig, WireLogEntry},
    LogServiceClient, ServiceConfig, UProtocolLogProvider,
};

/// uProtocol authority / entity identifying this SOVD server as the RPC caller.
const LOCAL_AUTHORITY: &str = "sovd-server";
const LOCAL_UE_ID: u32 = 0x7000;
const LOCAL_VERSION: u8 = 1;

/// Builds the uProtocol-backed log provider over a shared in-process transport,
/// starting the demo ECU stand-in on the same transport.
///
/// # Errors
/// Returns an error string if the ECU endpoint or the RPC client cannot be set
/// up.
pub async fn build_demo_provider() -> Result<UProtocolLogProvider, String> {
    let transport: Arc<dyn UTransport> = Arc::new(LocalTransport::default());
    let config = ServiceConfig::default();

    start_dummy_ecu(Arc::clone(&transport), &config).await?;

    let local = StaticUriProvider::new(LOCAL_AUTHORITY, LOCAL_UE_ID, LOCAL_VERSION);
    let client = LogServiceClient::new(transport, &local, config)
        .await
        .map_err(|e| e.to_string())?;

    Ok(UProtocolLogProvider::new(Arc::new(client)))
}

/// In-process stand-in for `swc_dummy_log_provider`, answering `getLogs` with a
/// small set of entries that honour the severity threshold in the query.
async fn start_dummy_ecu(
    transport: Arc<dyn UTransport>,
    config: &ServiceConfig,
) -> Result<(), String> {
    let service_uri = Arc::new(StaticUriProvider::new(
        &config.authority,
        config.service_id,
        config.version_major,
    ));
    let server = InMemoryRpcServer::new(transport, service_uri);

    let handler = Arc::new(DummyLogEcu {
        get_logs_method_id: config.get_logs_method_id,
        get_config_method_id: config.get_config_method_id,
        configure_method_id: config.configure_method_id,
        reset_config_method_id: config.reset_config_method_id,
        // Capture config is mutable at runtime via `configure`/`resetConfig`,
        // so the stand-in holds it behind a mutex and starts from the default.
        config: Mutex::new(seed_config()),
    }) as Arc<dyn RequestHandler>;

    for method_id in [
        config.get_logs_method_id,
        config.get_config_method_id,
        config.configure_method_id,
        config.reset_config_method_id,
    ] {
        server
            .register_endpoint(None, method_id, Arc::clone(&handler))
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// In-process ECU stand-in. Dispatches on the called method id, mirroring a
/// provider that exposes `getLogs`, `getConfig`, `configure` and `resetConfig`.
///
/// Holds the capture configuration behind a `Mutex` so `configure`/`resetConfig`
/// mutate it and both `getConfig` and `getLogs` observe the change — `getLogs`
/// drops entries whose severity is below the configured capture threshold for
/// their host, so reconfiguring is visible in `/logs/entries`.
struct DummyLogEcu {
    get_logs_method_id: u16,
    get_config_method_id: u16,
    configure_method_id: u16,
    reset_config_method_id: u16,
    config: Mutex<Vec<WireLogConfig>>,
}

impl DummyLogEcu {
    fn handle_get_logs(
        &self,
        request_payload: Option<UPayload>,
    ) -> Result<Option<UPayload>, ServiceInvocationError> {
        let query = request_payload
            .and_then(|p| serde_json::from_slice::<LogQuery>(&p.payload()).ok())
            .unwrap_or(LogQuery {
                severity: 4,
                created_after: 0,
                created_before: 0,
            });

        let config = self.config.lock().expect("config mutex poisoned").clone();

        // `created_after`/`created_before` are exclusive bounds in epoch ms;
        // `0` means "unbounded". Severity is a minimum threshold.
        let entries: Vec<WireLogEntry> = seed_entries()
            .into_iter()
            // Capture policy: drop entries below the configured threshold for
            // their host before applying the per-request filters.
            .filter(|e| captured(&config, e))
            .filter(|e| e.severity <= query.severity)
            .filter(|e| query.created_after == 0 || e.timestamp > query.created_after)
            .filter(|e| query.created_before == 0 || e.timestamp < query.created_before)
            .collect();

        let body = serde_json::to_vec(&entries)
            .map_err(|e| ServiceInvocationError::Internal(e.to_string()))?;
        Ok(Some(UPayload::new(
            body,
            UPayloadFormat::UPAYLOAD_FORMAT_RAW,
        )))
    }

    fn handle_get_config(&self) -> Result<Option<UPayload>, ServiceInvocationError> {
        let config = self.config.lock().expect("config mutex poisoned").clone();
        let body = serde_json::to_vec(&config)
            .map_err(|e| ServiceInvocationError::Internal(e.to_string()))?;
        Ok(Some(UPayload::new(
            body,
            UPayloadFormat::UPAYLOAD_FORMAT_RAW,
        )))
    }

    fn handle_configure(
        &self,
        request_payload: Option<UPayload>,
    ) -> Result<Option<UPayload>, ServiceInvocationError> {
        let new_config = request_payload
            .ok_or_else(|| ServiceInvocationError::InvalidArgument("missing config body".into()))
            .and_then(|p| {
                serde_json::from_slice::<Vec<WireLogConfig>>(&p.payload())
                    .map_err(|e| ServiceInvocationError::InvalidArgument(e.to_string()))
            })?;

        *self.config.lock().expect("config mutex poisoned") = new_config;
        Ok(None)
    }

    fn handle_reset_config(&self) -> Result<Option<UPayload>, ServiceInvocationError> {
        *self.config.lock().expect("config mutex poisoned") = seed_config();
        Ok(None)
    }
}

#[async_trait]
impl RequestHandler for DummyLogEcu {
    async fn handle_request(
        &self,
        resource_id: u16,
        _attributes: &UAttributes,
        request_payload: Option<UPayload>,
    ) -> Result<Option<UPayload>, ServiceInvocationError> {
        if resource_id == self.get_logs_method_id {
            self.handle_get_logs(request_payload)
        } else if resource_id == self.get_config_method_id {
            self.handle_get_config()
        } else if resource_id == self.configure_method_id {
            self.handle_configure(request_payload)
        } else if resource_id == self.reset_config_method_id {
            self.handle_reset_config()
        } else {
            Err(ServiceInvocationError::Unimplemented(format!(
                "unknown method id {resource_id:#06x}"
            )))
        }
    }
}

/// Returns `true` if `entry` is captured under the given configuration.
///
/// A rule matches an entry when the rule's `host`/`process` are unset
/// (wildcard) or equal the entry's. The entry is captured if it is at least as
/// severe as the matched rule's threshold (severity encoding is most-severe
/// first, so "at least as severe" means `entry.severity <= threshold`). With no
/// matching rule the entry is captured (nothing restricts it).
fn captured(config: &[WireLogConfig], entry: &WireLogEntry) -> bool {
    let matched: Vec<&WireLogConfig> = config
        .iter()
        .filter(|rule| {
            rule.host.as_ref().map_or(true, |h| Some(h) == entry.host.as_ref())
                && rule
                    .process
                    .as_ref()
                    .map_or(true, |p| Some(p) == entry.process.as_ref())
        })
        .collect();

    if matched.is_empty() {
        return true;
    }
    // Most permissive matching rule wins (largest severity = least severe).
    matched.iter().any(|rule| entry.severity <= rule.severity)
}

/// Capture configuration the stand-in ECU reports via `getConfig`: one rule
/// scoped to `host=ecu1` capturing at `Info` (severity 3) and more severe.
fn seed_config() -> Vec<WireLogConfig> {
    vec![WireLogConfig {
        severity: 3, // Info
        host: Some("ecu1".into()),
        process: None,
    }]
}

fn seed_entries() -> Vec<WireLogEntry> {
    vec![
        WireLogEntry {
            timestamp: 1_700_000_000_000,
            severity: 1, // Error
            msg: "ECU1: brake controller fault detected".into(),
            host: Some("ecu1".into()),
            process: Some("brake_ctl".into()),
        },
        WireLogEntry {
            timestamp: 1_700_000_001_000,
            severity: 2, // Warn
            msg: "ECU1: CAN bus retransmission".into(),
            host: Some("ecu1".into()),
            process: Some("can_mgr".into()),
        },
        WireLogEntry {
            timestamp: 1_700_000_002_000,
            severity: 3, // Info
            msg: "ECU1: diagnostic session opened".into(),
            host: Some("ecu1".into()),
            process: Some("diag".into()),
        },
    ]
}
