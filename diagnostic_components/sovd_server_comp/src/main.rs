// sovd_server_comp — diagnostic SOVD server component
//
// Wires DiagLogProvider into opensovd-server:
//   1. Build a Topology with one Component ("diag-ecu") and one App
//      ("diag-app") that carries the DiagLogProvider.
//   2. Hand the Topology to Server::builder() and start serving.
//
// Exposed endpoints (all under /v1):
//   GET  /v1/apps/diag-app/logs          — log resource URIs
//   GET  /v1/apps/diag-app/logs/entries  — filtered log entries
//   GET  /v1/apps/diag-app/logs/config   — current severity config
//   PUT  /v1/apps/diag-app/logs/config   — update severity config
//   DELETE /v1/apps/diag-app/logs/config — reset severity config

mod log_provider;
mod uprotocol_source;

use opensovd_core::{App, Component, Topology};
use opensovd_server::Server;
use tokio::net::TcpListener;
use tracing::info;

use crate::log_provider::DiagLogProvider;

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
            let provider = uprotocol_source::build_demo_provider()
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            app.with_log_provider(provider)
        } else {
            info!("Log source: in-memory seed data");
            app.with_log_provider(DiagLogProvider::new())
        };

        t.add_app(app);
    }

    info!("Topology ready: component=diag-ecu  app=diag-app");

    // ------------------------------------------------------------------
    // 2. Bind the TCP listener
    // ------------------------------------------------------------------
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    info!("Binding on {}", listener.local_addr()?);

    // ------------------------------------------------------------------
    // 3. Build and start the SOVD server
    // ------------------------------------------------------------------
    Server::builder()
        .listener(listener)
        .topology(topology)
        .build()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
        .serve()
        .await
}
