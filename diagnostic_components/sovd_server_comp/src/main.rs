// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0
/* Portions of this file were generated with AI assistance. */

// sovd_server_comp — diagnostic SOVD server component
//
// Wires two log providers into opensovd-server, so a single SOVD server
// aggregates two independent log sources ("multiple ECUs over one API"):
//   1. Build a Topology with:
//        - Component "diag-ecu" + App "diag-app"  → UProtocolLogProvider
//          (logs fetched from the standalone dummy ECU over uProtocol/Zenoh).
//        - Component "hw-ecu"   + App "hw-log"    → DiagLogProvider
//          (logs pushed into the REST sink, with live SSE streaming).
//   2. Hand the Topology to Server::builder() and start serving.
//
// Exposed endpoints (all under /sovd/v1), for both {app} ∈ {diag-app, hw-log}:
//   GET  /sovd/v1/apps/{app}/logs                 — log resource URIs
//   GET  /sovd/v1/apps/{app}/logs/entries         — filtered log entries
//   GET  /sovd/v1/apps/{app}/logs/entries/stream  — live SSE log stream
//   GET  /sovd/v1/apps/{app}/logs/config          — current severity config
//   PUT  /sovd/v1/apps/{app}/logs/config          — update severity config
//   DELETE /sovd/v1/apps/{app}/logs/config        — reset severity config
//
// The uProtocol source is registered by default. Set TRACEON_UPROTOCOL=off to
// skip it (e.g. when the dummy ECU process is not running), in which case only
// the REST-sink app is served.

mod log_provider;
mod uprotocol_source;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use opensovd_core::{App, Component, Topology};
use opensovd_server::Server;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::info;

use crate::log_provider::DiagLogProvider;

async fn ingest_log(
    State(provider): State<DiagLogProvider>,
    Json(payload): Json<Value>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    provider
        .ingest_hardware_event(payload)
        .await
        .map(|()| StatusCode::ACCEPTED)
        .map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": error.to_string() })),
            )
        })
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // ------------------------------------------------------------------
    // 1. Build the topology with two independent log sources
    // ------------------------------------------------------------------
    let topology = Topology::new();

    // Source B: REST sink + live SSE stream, exposed as app "hw-log".
    let sink_provider = DiagLogProvider::new();
    let sink = Router::new()
        .route("/logs", post(ingest_log))
        .with_state(sink_provider.clone());

    {
        let mut t = topology.write().await;

        // ---- App "diag-app" on component "diag-ecu": uProtocol/Zenoh ECU ----
        //
        // Registered by default; set TRACEON_UPROTOCOL=off to skip it (useful
        // when the dummy ECU process is not running).
        let uprotocol_enabled =
            std::env::var("TRACEON_UPROTOCOL").unwrap_or_default() != "off";
        if uprotocol_enabled {
            info!("Source A (diag-app): uProtocol getLogs RPC over Zenoh");
            let provider = uprotocol_source::build_provider()
                .await
                .map_err(std::io::Error::other)?;
            t.add_component(Component::new("diag-ecu", "Diagnostic ECU"));
            t.add_app(
                App::new("diag-app", "Diagnostic Application")
                    .with_component_id("diag-ecu")
                    .with_log_provider(provider),
            );
        } else {
            info!("Source A (diag-app): disabled (TRACEON_UPROTOCOL=off)");
        }

        // ---- App "hw-log" on component "hw-ecu": REST sink + SSE stream ----
        info!("Source B (hw-log): REST log sink with live SSE streaming");
        t.add_component(Component::new("hw-ecu", "Hardware Log ECU"));
        t.add_app(
            App::new("hw-log", "Hardware Log Application")
                .with_component_id("hw-ecu")
                .with_log_provider(sink_provider),
        );
    }

    info!("Topology ready: apps=[diag-app (uProtocol), hw-log (REST sink)]");
    info!("Log sink available at POST /internal/logs");

    // ------------------------------------------------------------------
    // 2. Bind the TCP listener
    // ------------------------------------------------------------------
    // Bind to loopback: this is a self-contained demo, not a production server.
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    info!("Binding on {}", listener.local_addr()?);

    // ------------------------------------------------------------------
    // 3. Build and start the SOVD server
    // ------------------------------------------------------------------
    let static_dir = std::env::var("SOVD_STATIC_DIR").unwrap_or_else(|_| "static".to_string());

    Server::builder()
        .listener(listener)
        .base_uri("http://127.0.0.1:8080/sovd")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))?
        .service("/internal", sink)
        .topology(topology)
        .service("/ui", ServeDir::new(&static_dir))
        .layer(CorsLayer::permissive())
        .build()
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .serve()
        .await
}
