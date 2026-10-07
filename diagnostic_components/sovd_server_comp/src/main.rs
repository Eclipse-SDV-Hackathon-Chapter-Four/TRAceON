// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0
/* Portions of this file were generated with AI assistance. */

mod log_provider;
mod uprotocol_source;

use axum::{extract::State, http::StatusCode, routing::{get, post}, Json, Router};
use opensovd_core::{App, Component, Topology};
use opensovd_server::Server;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::log_provider::DiagLogProvider;

async fn serve_ui() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../static/index.html"))
}

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
        t.add_component(Component::new("diag-ecu", "Diagnostic ECU"));

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

    // ------------------------------------------------------------------
    // 2. Spawn UI server on port 8081 (plain Axum, no opensovd fallback)
    // ------------------------------------------------------------------
    let ui_listener = TcpListener::bind("0.0.0.0:8081").await?;
    info!("UI available at http://127.0.0.1:8081/");
    let ui_router = Router::new()
        .route("/", get(serve_ui))
        .layer(CorsLayer::permissive());
    tokio::spawn(async move {
        axum::serve(ui_listener, ui_router).await.ok();
    });

    // ------------------------------------------------------------------
    // 3. SOVD API server on port 8080
    // ------------------------------------------------------------------
    let api_listener = TcpListener::bind("0.0.0.0:8080").await?;
    info!("SOVD API at http://127.0.0.1:8080/sovd/v1/");

    Server::builder()
        .listener(api_listener)
        .base_uri("http://127.0.0.1:8080/sovd")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))?
        .service("/internal", sink)
        .topology(topology)
        .layer(CorsLayer::permissive())
        .build()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
        .serve()
        .await
}
