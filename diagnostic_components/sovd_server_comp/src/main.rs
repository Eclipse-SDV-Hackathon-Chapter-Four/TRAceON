// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0
/* Portions of this file were generated with AI assistance. */

// sovd_server_comp — diagnostic SOVD server component
//
// Wires DiagLogProvider into opensovd-server:
//   1. Build a Topology with one Component ("diag-ecu") and one App
//      ("diag-app") that carries the DiagLogProvider.
//   2. Hand the Topology to Server::builder() and start serving.
//
// Exposed endpoints (all under /sovd/v1):
//   GET  /sovd/v1/apps/diag-app/logs          — log resource URIs
//   GET  /sovd/v1/apps/diag-app/logs/entries  — filtered log entries
//   GET  /sovd/v1/apps/diag-app/logs/config   — current severity config
//   PUT  /sovd/v1/apps/diag-app/logs/config   — update severity config
//   DELETE /sovd/v1/apps/diag-app/logs/config — reset severity config

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
    // 1. Build the topology
    // ------------------------------------------------------------------
    let topology = Topology::new();
    let log_provider = DiagLogProvider::new();
    let sink = Router::new()
        .route("/logs", post(ingest_log))
        .with_state(log_provider.clone());

    {
        let mut t = topology.write().await;

        // Physical component (ECU)
        t.add_component(Component::new("diag-ecu", "Diagnostic ECU"));

        // Software app hosted on the ECU, carrying the log provider.
        //
        // Source selection (env TRACEON_LOG_SOURCE):
        //   "uprotocol" -> fetch logs from the ECU over the uProtocol getLogs RPC
        //   anything else (default) -> in-memory seed data
        let app = App::new("diag-app", "Diagnostic Application").with_component_id("diag-ecu");

        let log_source = std::env::var("TRACEON_LOG_SOURCE").unwrap_or_default();
        let app = if log_source == "uprotocol" {
            info!("Log source: uProtocol getLogs RPC");
            let provider = uprotocol_source::build_provider()
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            app.with_log_provider(provider)
        } else {
            info!("Log source: REST log sink");
            app.with_log_provider(log_provider)
        };

        t.add_app(app);
    }

    info!("Topology ready: component=diag-ecu  app=diag-app");
    info!("Log sink available at POST /internal/logs");

    // ------------------------------------------------------------------
    // 2. Bind the TCP listener
    // ------------------------------------------------------------------
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
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
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
        .serve()
        .await
}
