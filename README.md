# TRAceON

> SOVD logs resource for OpenSOVD, fetched from the ECU over uProtocol/Zenoh.

TRAceON implements the SOVD `logs` resource for [OpenSOVD](https://github.com/eclipse-opensovd),
letting a diagnostics engineer pull filtered logs from vehicle ECUs through a single SOVD
interface to speed up root-cause analysis.

![TRAceON architecture overview](docs/TRAceON.png)

## Problem Statement

OpenSOVD does not yet provide a native logging resource. When an ECU reports a fault there is
no unified way to collect and inspect logs across the vehicle network.

TRAceON fills that gap end-to-end with two complementary ingestion paths, with the goal of
contributing the implementation back to OpenSOVD once it is mature.

## System Architecture

TRAceON supports two log ingestion paths that can be selected at runtime.

### Path 1 — ThreadX / MQTT hardware ingest (default mode)

A hardware ECU running **ThreadX RTOS** publishes log events over **MQTT**. The telemetry
server bridges those events into the SOVD server via a REST sink. The server buffers entries
in memory and broadcasts them to any active SSE stream under app ID `hw-log`.

```text
+------------------+  MQTT publish  +----------------------+  POST /internal/logs
|  ECU (ThreadX)   | -------------> |  telemetry-server    | -------------------->+
|  (ThreadX RTOS)  |                |  (Python, port 8083) |                      |
+------------------+                +----------------------+                      |
                                                                                   v
+------------+  GET /sovd/v1/apps/hw-log/logs/entries      +--------------------+
| Tester /   | <-----------------------------------------> |  sovd_server_comp  |
| Client     |  GET /sovd/v1/apps/hw-log/logs/entries      |  app: hw-log       |
|  (UI :8082 |       /stream  (SSE live stream)            |  DiagLogProvider   |
|   /curl)   | <-----------------------------------------> |  (in-memory ring   |
+------------+                                             |   + broadcast)     |
                                                           |  :8080 API         |
                                                           |  :8082 UI          |
                                                           +--------------------+
```

### Path 2 — uProtocol / Zenoh RPC

A standalone ECU stand-in (`dummy_diag_app`) exposes a `getLogs` / `getConfig` RPC service
over **uProtocol** using a **Zenoh** TCP transport. The SOVD server connects to it and
delegates every `GET /entries` request to the ECU via RPC under app ID `diag-app`.

```text
+------------------+  uProtocol getLogs RPC (Zenoh TCP :7447)  +--------------------+
|  dummy_diag_app  | <----------------------------------------> |  sovd_server_comp  |
|  (ECU stand-in)  |                                            |  app: diag-app     |
+------------------+                                            |  UProtocolLog-     |
                                                                |  Provider          |
+------------+  GET /sovd/v1/apps/diag-app/logs/entries        |                    |
| Tester /   | <----------------------------------------->    |  :8080 API         |
| Client     |  GET /sovd/v1/apps/diag-app/logs/entries        |  :8082 UI          |
|  (UI/curl) |       /stream  (SSE live stream)                +--------------------+
+------------+
```

See [docs/TraceON_UseCase.md](docs/TraceON_UseCase.md) for the full use-case description.

## Repository Structure

```text
TRAceON/
├── diagnostic_components/
│   ├── sovd_server_comp/             # SOVD server — REST API, log sink, SSE stream, UI
│   ├── uprotocol_log_client_comp/    # uProtocol RPC client library (getLogs/getConfig/…)
│   └── dummy_diag_app/               # Standalone ECU stand-in — serves getLogs over Zenoh
├── docs/
│   ├── TRAceON.png
│   └── TraceON_UseCase.md
├── scripts/
│   ├── start-ui.sh                   # Start server + open browser (auto-detects WSL2 IP)
│   ├── stream-logs.sh                # Open all 3 terminals in Windows Terminal tabs
│   ├── terminal-1-sovd-server.sh     # Tab 1: build & run sovd_server_comp
│   ├── terminal-2-telemetry.sh       # Tab 2: start MQTT telemetry receptor
│   └── terminal-3-stream.sh          # Tab 3: enable forwarding + tail SSE stream
├── deploy/autosd/                     # AutoSD images, Quadlets, and deployment
├── LIVE-STREAM-SETUP.md              # End-to-end hardware stream setup guide
├── UI-GUIDE.md                       # Log viewer UI reference
└── open_source/
    ├── eclipse-autosd/               # Eclipse AutoSD source (git submodule)
    └── opensovd-core/                # OpenSOVD core (git submodule)
```

## What Is Implemented

- **SOVD logs REST API** — `GET /entries`, `GET /config`, `PUT /config`, `DELETE /config`
  under `/sovd/v1/apps/{app-id}/logs/`.
- **Live SSE stream** — `GET /entries/stream` pushes new entries in real time via
  Server-Sent Events; backed by a `tokio::broadcast` channel fed by the hardware ingest sink.
- **Hardware log ingest sink** — `POST /internal/logs` accepts JSON log events from an
  MQTT receptor (ThreadX ECU → MQTT → receptor → REST sink → SOVD); entries are buffered
  in a ring buffer (max 1024) and immediately broadcast to active SSE subscribers.
- **Severity and time filters** — `?severity=`, `?created-after=`, `?created-before=`
  query parameters on `GET /entries`.
- **Per-context severity config** — `PUT /config` stores a threshold per context type
  (RFC 5424 or AUTOSAR DLT); `entries()` and `stream()` both apply it server-side.
- **uProtocol/Zenoh transport** — `getLogs`, `getConfig`, `configure`, `resetConfig` RPCs
  carried as `UPAYLOAD_FORMAT_JSON` over a Zenoh TCP peer connection.
- **Two-process uProtocol demo** — `sovd_server_comp` (HPC) + `dummy_diag_app` (ECU)
  run as separate OS processes connected by Zenoh.
- **Live log viewer UI** — served on port `8082`; context-aware filtering, per-context
  severity config panel, auto-refresh, SSE live mode.

## AutoSD deployment

The AutoSD source is pinned as the `open_source/eclipse-autosd` submodule. The
TRAceON SOVD server and dummy ECU can be deployed into a running AutoSD image
with the Quadlets and automated test described in
[deploy/autosd/README.md](deploy/autosd/README.md).

## Roadmap

Two log sources (`diag-app` over uProtocol/Zenoh and `hw-log` over the REST
sink) are already wired behind one SOVD server. Planned next steps:

- **Server-side `?context-type=` filter** on `GET /entries` (today the context
  filter is applied client-side in the UI).
- **Real DLT / journald ingestion** on the ECU side (the ECU currently serves
  canned entries).
- **Dynamic multi-ECU topology** — register ECUs/apps at runtime instead of the
  current static wiring.
- **ECU health monitoring and alerting.**

## Getting Started

### Prerequisites

- [Rust toolchain](https://rustup.rs/)
- Git with submodule support

### Clone

```bash
git clone --recurse-submodules <repository-url>
cd TRAceON
```

If you already cloned without submodules:

```bash
git submodule update --init --recursive
```

### Build

```bash
# Build all three crates from the workspace root
cargo build
```

### Run — ThreadX/MQTT hardware ingest mode (default)

The server binds two ports: the SOVD API on `8080` and the log viewer UI on `8082`.
Seed entries are pre-loaded so the UI shows data immediately.

```bash
# API:   http://<host>:8080/sovd/v1/apps/hw-log/logs/entries
# UI:    http://<host>:8082/
# Sink:  POST http://<host>:8080/internal/logs
cargo run -p sovd_server_comp
```

> **WSL2 note:** the UI's `const BASE` and `const APP` in
> `diagnostic_components/sovd_server_comp/static/index.html` must point to the WSL2 IP
> (e.g. `http://172.26.10.72:8080/sovd` and app `hw-log`). Find your IP with
> `ip addr show eth0 | grep 'inet '`. Re-run `cargo build` after editing the file.

Send a hardware log event to the sink:

```bash
curl -X POST http://localhost:8080/internal/logs \
  -H 'Content-Type: application/json' \
  -d '{"timestamp":"2026-01-01T00:00:00Z","severity":"error","msg":"brake fault","context":{"type":"AUTOSAR_DLT","application_id":"BRK","context_id":"FAULT"}}'
```

### Quick Start (WSL2 + Windows Terminal)

```bash
# Start server and open browser automatically
bash scripts/start-ui.sh

# Or open all 3 terminals (server + telemetry + SSE stream) in separate tabs
bash scripts/stream-logs.sh
```

See [LIVE-STREAM-SETUP.md](LIVE-STREAM-SETUP.md) for the full end-to-end hardware stream
setup and [UI-GUIDE.md](UI-GUIDE.md) for the log viewer UI reference.

### Run — uProtocol/Zenoh two-process mode

uProtocol is **enabled by default**. Just run both processes — no extra env vars needed.

**Terminal 1 — ECU stand-in (listens on Zenoh TCP `127.0.0.1:7447`):**

```bash
cargo run -p dummy_diag_app
```

**Terminal 2 — SOVD server (connects to ECU over Zenoh):**

```bash
cargo run -p sovd_server_comp
```

To run **without** uProtocol (REST sink `hw-log` only, no `dummy_diag_app` needed):

```bash
TRACEON_UPROTOCOL=off cargo run -p sovd_server_comp
```

### Environment Variables

| Variable | Component | Default | Description |
|---|---|---|---|
| `TRACEON_UPROTOCOL` | `sovd_server_comp` | `on` | uProtocol `diag-app` is enabled by default; set to `off` to serve only the REST sink `hw-log` (no `dummy_diag_app` needed) |
| `APP_ZENOH_CONNECT` | `sovd_server_comp` | `tcp/127.0.0.1:7447` | Zenoh TCP endpoint of the ECU to connect to |
| `SOVD_LISTEN_ADDR` | `sovd_server_comp` | `0.0.0.0:8080` | Address the SOVD API server binds to |
| `SOVD_BASE_URI` | `sovd_server_comp` | `http://127.0.0.1:8080/sovd` | Base URI advertised in SOVD responses |
| `APP_ZENOH_LISTEN` | `dummy_diag_app` | `tcp/127.0.0.1:7447` | Zenoh TCP endpoint the ECU listens on |
| `MQTT_HOST` | `terminal-2-telemetry.sh` | `192.168.88.254` | IP of the machine running the MQTT broker |
| `RUST_LOG` | both | `info` | Tracing log level (`trace`, `debug`, `info`, `warn`, `error`) |

> The log viewer UI is compiled into the `sovd_server_comp` binary (served on port
> `8082`), so there is no runtime static-directory variable.

### API Base Path

All SOVD endpoints are versioned under `/sovd/v1`:

```
GET    /sovd/v1/apps/{app-id}/logs
GET    /sovd/v1/apps/{app-id}/logs/entries[?severity=warn&created-after=<iso8601>&created-before=<iso8601>]
GET    /sovd/v1/apps/{app-id}/logs/entries/stream   (SSE live stream)
GET    /sovd/v1/apps/{app-id}/logs/config
PUT    /sovd/v1/apps/{app-id}/logs/config
DELETE /sovd/v1/apps/{app-id}/logs/config

POST   /internal/logs   (hardware event ingest — ThreadX/MQTT receptor bridge)
```

App IDs: `hw-log` (REST sink, default) · `diag-app` (uProtocol, requires `dummy_diag_app`)

The live log viewer UI is served at `http://<host>:8082/`.

## Scripts

All scripts are in the `scripts/` directory. Make them executable once with `chmod +x scripts/*.sh`.

| Script | Description |
|---|---|
| `demo.sh` | One-command end-to-end demo — starts both processes on loopback, injects events into the sink, and exercises the logs API (entries, config, SSE) for both apps, with automatic teardown |
| `demo-threadx.sh` | Full ThreadX pipeline demo (board → MQTT → telemetry → sink) with an interactive menu to read each app, tail the SSE stream, and inject a board log |
| `start-ui.sh` | Starts `sovd_server_comp` in a new terminal tab and automatically opens the log viewer UI in the browser |
| `stream-logs.sh [severity]` | Full live streaming pipeline — opens 3 terminal tabs: SOVD server, telemetry server (MQTT→forward), and live SSE stream |
| `terminal-1-sovd-server.sh` | Starts the SOVD server on port 8080 (UI on 8082) |
| `terminal-2-telemetry.sh` | Clones TRAceON-ThreadX (if needed), sets up Python venv, starts the telemetry server on port 8083 |
| `terminal-3-stream.sh [severity]` | Enables log forwarding and tails the live SSE stream |
| `test_autosd_uprotocol.sh` | Builds and deploys the SOVD server + dummy ECU into a running AutoSD image (podman/docker) and validates the uProtocol flow — see [deploy/autosd/README.md](deploy/autosd/README.md) |

**Quick start — UI only:**
```bash
./scripts/start-ui.sh
```

**Quick start — full live stream from ThreadX ECU:**
```bash
./scripts/stream-logs.sh DLT_ERROR
```

See [LIVE-STREAM-SETUP.md](LIVE-STREAM-SETUP.md) and [UI-GUIDE.md](UI-GUIDE.md) for detailed instructions.

## Contributing

This project is intended to be contributed back to OpenSOVD. Contributions are welcome —
please open an issue or pull request to discuss changes.

## License

Apache-2.0 — see [LICENSE](LICENSE).

## Team

| Name | Role |
|---|---|
| Helge Gudmundsen | Developer |
| Isabella Lanes Rocha | Developer |
| Kavyasree Sankaranarayanan Nair | Developer |
| Marufa Binte Mostafa | Developer |
| Matheus Abrahao | Developer |
| Priyankkumar Bidya | Developer |
| Himank Meattle | Team Support |
