// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

//! Standalone dummy diagnostic app binary (ECU stand-in).
//!
//! Runs the dummy log service (`getLogs`/`getConfig`/`configure`/`resetConfig`)
//! over a real uProtocol Zenoh transport, so it talks to the SOVD server
//! (`sovd_server_comp`) across process boundaries.
//!
//! The ECU acts as the Zenoh peer that *listens* on a TCP endpoint; the SOVD
//! server connects to it. Multicast scouting is disabled so the demo does not
//! depend on multicast being available on the loopback interface.
//!
//! Environment:
//!   `APP_ZENOH_LISTEN`  TCP endpoint to listen on (default `tcp/127.0.0.1:7447`)
//!   `RUST_LOG`          log level (default `info`)

use std::sync::Arc;

use dummy_diag_app::register_endpoints;
use serde_json::json;
use up_rust::{LocalUriProvider, StaticUriProvider, UTransport};
use up_transport_zenoh::{zenoh_config, UPTransportZenoh};
use uprotocol_log_client_comp::ServiceConfig;

/// Default TCP endpoint the ECU listens on.
const DEFAULT_LISTEN: &str = "tcp/127.0.0.1:7447";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let listen = std::env::var("APP_ZENOH_LISTEN").unwrap_or_else(|_| DEFAULT_LISTEN.to_string());
    let config = ServiceConfig::default();

    // Build the Zenoh transport for the ECU service entity.
    let uri_provider = StaticUriProvider::new(&config.authority, config.service_id, config.version_major);
    let transport: Arc<dyn UTransport> = UPTransportZenoh::builder(uri_provider.get_authority())
        .expect("invalid authority name")
        .with_config(peer_config_listen(&listen))
        .build()
        .await
        .map(Arc::new)?;

    // Register the log service endpoints; keep the server alive for the process
    // lifetime.
    let _server = register_endpoints(transport, &config)
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    tracing::info!(
        authority = %config.authority,
        service_id = format_args!("{:#06x}", config.service_id),
        %listen,
        "Dummy log ECU ready — serving getLogs/getConfig/configure/resetConfig over Zenoh"
    );

    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down dummy log ECU");
    Ok(())
}

/// Zenoh peer configuration that listens on `endpoint` and disables multicast
/// scouting (loopback-friendly for a self-contained two-process demo).
fn peer_config_listen(endpoint: &str) -> zenoh_config::Config {
    let mut cfg = zenoh_config::Config::default();
    cfg.insert_json5("mode", &json!("peer").to_string())
        .expect("set zenoh mode");
    cfg.insert_json5("listen/endpoints", &json!([endpoint]).to_string())
        .expect("set zenoh listen endpoints");
    cfg.insert_json5("scouting/multicast/enabled", &json!(false).to_string())
        .expect("disable multicast scouting");
    cfg
}
