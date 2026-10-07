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

use std::sync::Arc;

use async_trait::async_trait;
use up_rust::communication::{
    InMemoryRpcServer, RequestHandler, RpcServer, ServiceInvocationError, UPayload,
};
use up_rust::local_transport::LocalTransport;
use up_rust::{StaticUriProvider, UAttributes, UPayloadFormat, UTransport};
use uprotocol_log_client_comp::{
    wire::{LogQuery, WireLogEntry},
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
    server
        .register_endpoint(
            None,
            config.get_logs_method_id,
            Arc::new(DummyLogEcu) as Arc<dyn RequestHandler>,
        )
        .await
        .map_err(|e| e.to_string())
}

struct DummyLogEcu;

#[async_trait]
impl RequestHandler for DummyLogEcu {
    async fn handle_request(
        &self,
        _resource_id: u16,
        _attributes: &UAttributes,
        request_payload: Option<UPayload>,
    ) -> Result<Option<UPayload>, ServiceInvocationError> {
        let query = request_payload
            .and_then(|p| serde_json::from_slice::<LogQuery>(&p.payload()).ok())
            .unwrap_or(LogQuery {
                severity: 4,
                created_after: 0,
                created_before: 0,
            });

        // `created_after`/`created_before` are exclusive bounds in epoch ms;
        // `0` means "unbounded". Severity is a minimum threshold.
        let entries: Vec<WireLogEntry> = seed_entries()
            .into_iter()
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
