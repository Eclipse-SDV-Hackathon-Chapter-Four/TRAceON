// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0
/* Portions of this file were generated with AI assistance. */

//! Wiring for the uProtocol-backed log source.
//!
//! Builds a [`UProtocolLogProvider`] that fetches logs from the standalone
//! dummy diag app (`dummy_diag_app` binary) over a real uProtocol **Zenoh** transport.
//! The SOVD server and the ECU run as separate processes; this side is the
//! Zenoh peer that *connects* to the ECU's listen endpoint.
//!
//! In a real deployment the ECU is the actual `swc_dummy_log_provider`; only
//! the Zenoh endpoint/authority configuration changes, not this code.

use std::sync::Arc;

use serde_json::json;
use up_rust::{LocalUriProvider, StaticUriProvider, UTransport};
use up_transport_zenoh::{zenoh_config, UPTransportZenoh};
use uprotocol_log_client_comp::{LogServiceClient, ServiceConfig, UProtocolLogProvider};

/// uProtocol authority / entity identifying this SOVD server as the RPC caller.
const LOCAL_AUTHORITY: &str = "sovd-server";
const LOCAL_UE_ID: u32 = 0x7000;
const LOCAL_VERSION: u8 = 1;

/// Default Zenoh TCP endpoint of the ECU that this server connects to.
const DEFAULT_ECU_ENDPOINT: &str = "tcp/127.0.0.1:7447";

/// Builds the uProtocol-backed log provider over a real Zenoh transport,
/// connecting to the standalone dummy ECU.
///
/// The app endpoint can be overridden with `APP_ZENOH_CONNECT`
/// (default `tcp/127.0.0.1:7447`).
///
/// # Errors
/// Returns an error string if the Zenoh transport or the RPC client cannot be
/// set up.
pub async fn build_provider() -> Result<UProtocolLogProvider, String> {
    let connect =
        std::env::var("APP_ZENOH_CONNECT").unwrap_or_else(|_| DEFAULT_ECU_ENDPOINT.to_string());
    let config = ServiceConfig::default();

    let local = StaticUriProvider::new(LOCAL_AUTHORITY, LOCAL_UE_ID, LOCAL_VERSION);
    let transport: Arc<dyn UTransport> = UPTransportZenoh::builder(local.get_authority())
        .map_err(|e| format!("invalid authority name: {e}"))?
        .with_config(peer_config_connect(&connect))
        .build()
        .await
        .map(Arc::new)
        .map_err(|e| format!("failed to build Zenoh transport: {e}"))?;

    let client = LogServiceClient::new(transport, &local, config)
        .await
        .map_err(|e| e.to_string())?;

    Ok(UProtocolLogProvider::new(Arc::new(client)))
}

/// Zenoh peer configuration that connects to the ECU at `endpoint` and disables
/// multicast scouting (loopback-friendly for a self-contained two-process demo).
fn peer_config_connect(endpoint: &str) -> zenoh_config::Config {
    let mut cfg = zenoh_config::Config::default();
    cfg.insert_json5("mode", &json!("peer").to_string())
        .expect("set zenoh mode");
    cfg.insert_json5("connect/endpoints", &json!([endpoint]).to_string())
        .expect("set zenoh connect endpoints");
    cfg.insert_json5("scouting/multicast/enabled", &json!(false).to_string())
        .expect("disable multicast scouting");
    cfg
}
