// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

//! End-to-end check of the uProtocol log client against an in-process ECU
//! stand-in, exercising both response shapes (structured entries and the
//! plain-string fallback the current C++ dummy returns).

use std::sync::Arc;

use async_trait::async_trait;
use opensovd_core::{LogFilter, LogProvider, LogSeverity};
use up_rust::communication::{
    InMemoryRpcServer, RequestHandler, RpcServer, ServiceInvocationError, UPayload,
};
use up_rust::local_transport::LocalTransport;
use up_rust::{StaticUriProvider, UAttributes, UPayloadFormat, UTransport};

use chrono::{TimeZone, Utc};
use uprotocol_log_client_comp::{
    wire::{LogQuery, WireLogEntry},
    LogServiceClient, ServiceConfig, UProtocolLogProvider,
};

const SERVICE_AUTHORITY: &str = "vehicle";
const SERVICE_ID: u32 = 0x8010;
const SERVICE_VERSION: u8 = 1;
const GET_LOGS_RESOURCE: u16 = 0x0001;

/// ECU stand-in that answers `getLogs` with a fixed structured response.
struct StructuredEcu;

#[async_trait]
impl RequestHandler for StructuredEcu {
    async fn handle_request(
        &self,
        resource_id: u16,
        _attributes: &UAttributes,
        _payload: Option<UPayload>,
    ) -> Result<Option<UPayload>, ServiceInvocationError> {
        assert_eq!(resource_id, GET_LOGS_RESOURCE);
        let entries = vec![WireLogEntry {
            timestamp: 1_700_000_000_000,
            severity: 1, // Error
            msg: "ECU brake controller fault".to_string(),
            host: Some("ecu1".to_string()),
            process: Some("brake_ctl".to_string()),
        }];
        let body = serde_json::to_vec(&entries).unwrap();
        Ok(Some(UPayload::new(
            body,
            UPayloadFormat::UPAYLOAD_FORMAT_RAW,
        )))
    }
}

/// ECU stand-in that mirrors the current C++ dummy: a bare string.
struct PlainTextEcu;

#[async_trait]
impl RequestHandler for PlainTextEcu {
    async fn handle_request(
        &self,
        _resource_id: u16,
        _attributes: &UAttributes,
        _payload: Option<UPayload>,
    ) -> Result<Option<UPayload>, ServiceInvocationError> {
        let body = serde_json::to_vec("dummy log entry").unwrap();
        Ok(Some(UPayload::new(
            body,
            UPayloadFormat::UPAYLOAD_FORMAT_RAW,
        )))
    }
}

/// ECU stand-in that decodes the incoming `LogQuery` and filters its seed by
/// the exclusive time bounds, so the test can assert the client propagated
/// `created_after`/`created_before` over the wire.
struct TimeFilteringEcu;

impl TimeFilteringEcu {
    fn seed() -> Vec<WireLogEntry> {
        vec![
            WireLogEntry {
                timestamp: 1_700_000_000_000,
                severity: 1,
                msg: "entry @20".to_string(),
                host: None,
                process: None,
            },
            WireLogEntry {
                timestamp: 1_700_000_001_000,
                severity: 2,
                msg: "entry @21".to_string(),
                host: None,
                process: None,
            },
            WireLogEntry {
                timestamp: 1_700_000_002_000,
                severity: 3,
                msg: "entry @22".to_string(),
                host: None,
                process: None,
            },
        ]
    }
}

#[async_trait]
impl RequestHandler for TimeFilteringEcu {
    async fn handle_request(
        &self,
        _resource_id: u16,
        _attributes: &UAttributes,
        payload: Option<UPayload>,
    ) -> Result<Option<UPayload>, ServiceInvocationError> {
        let query: LogQuery = payload
            .and_then(|p| serde_json::from_slice(&p.payload()).ok())
            .ok_or_else(|| ServiceInvocationError::InvalidArgument("missing query".to_string()))?;

        let entries: Vec<WireLogEntry> = Self::seed()
            .into_iter()
            .filter(|e| query.created_after == 0 || e.timestamp > query.created_after)
            .filter(|e| query.created_before == 0 || e.timestamp < query.created_before)
            .collect();

        let body = serde_json::to_vec(&entries).unwrap();
        Ok(Some(UPayload::new(
            body,
            UPayloadFormat::UPAYLOAD_FORMAT_RAW,
        )))
    }
}

async fn start_ecu(transport: Arc<dyn UTransport>, handler: Arc<dyn RequestHandler>) {
    let service_uri = Arc::new(StaticUriProvider::new(
        SERVICE_AUTHORITY,
        SERVICE_ID,
        SERVICE_VERSION,
    ));
    let server = InMemoryRpcServer::new(transport, service_uri);
    server
        .register_endpoint(None, GET_LOGS_RESOURCE, handler)
        .await
        .expect("register getLogs endpoint");
}

async fn build_provider(transport: Arc<dyn UTransport>) -> UProtocolLogProvider {
    // SOVD-side local uEntity (the RPC response sink).
    let local = StaticUriProvider::new("sovd-server", 0x7000, 1);
    let client = LogServiceClient::new(transport, &local, ServiceConfig::default())
        .await
        .expect("build log service client");
    UProtocolLogProvider::new(Arc::new(client))
}

#[tokio::test]
async fn entries_maps_structured_response() {
    let transport: Arc<dyn UTransport> = Arc::new(LocalTransport::default());
    start_ecu(Arc::clone(&transport), Arc::new(StructuredEcu)).await;

    let provider = build_provider(transport).await;
    let entries = provider
        .entries(LogFilter {
            severity: Some(LogSeverity::Error),
            ..Default::default()
        })
        .await
        .expect("getLogs should succeed");

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].severity, LogSeverity::Error);
    assert_eq!(entries[0].msg, "ECU brake controller fault");
}

#[tokio::test]
async fn entries_propagates_time_window() {
    let transport: Arc<dyn UTransport> = Arc::new(LocalTransport::default());
    start_ecu(Arc::clone(&transport), Arc::new(TimeFilteringEcu)).await;

    let provider = build_provider(transport).await;

    // Exclusive window (22:13:20Z, 22:13:22Z) should drop both endpoints and
    // return only the entry at 22:13:21Z.
    let after = Utc.timestamp_millis_opt(1_700_000_000_000).single().unwrap();
    let before = Utc.timestamp_millis_opt(1_700_000_002_000).single().unwrap();

    let entries = provider
        .entries(LogFilter {
            created_after: Some(after),
            created_before: Some(before),
            ..Default::default()
        })
        .await
        .expect("getLogs should succeed");

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].msg, "entry @21");
}

#[tokio::test]
async fn entries_time_window_can_exclude_everything() {
    let transport: Arc<dyn UTransport> = Arc::new(LocalTransport::default());
    start_ecu(Arc::clone(&transport), Arc::new(TimeFilteringEcu)).await;

    let provider = build_provider(transport).await;

    // A lower bound past every seed entry yields an empty result.
    let after = Utc.timestamp_millis_opt(1_900_000_000_000).single().unwrap();

    let entries = provider
        .entries(LogFilter {
            created_after: Some(after),
            ..Default::default()
        })
        .await
        .expect("getLogs should succeed");

    assert!(entries.is_empty());
}

#[tokio::test]
async fn entries_wraps_plain_text_response() {
    let transport: Arc<dyn UTransport> = Arc::new(LocalTransport::default());
    start_ecu(Arc::clone(&transport), Arc::new(PlainTextEcu)).await;

    let provider = build_provider(transport).await;
    let entries = provider
        .entries(LogFilter::default())
        .await
        .expect("getLogs should succeed");

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].severity, LogSeverity::Info);
    assert_eq!(entries[0].msg, "dummy log entry");
}
