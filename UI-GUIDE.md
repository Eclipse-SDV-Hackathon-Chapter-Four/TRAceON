# TRAceON — Log Viewer UI Guide

Instructions to configure and run the **TRAceON SOVD Log Viewer** (`index.html`).

---

## Overview

The UI is a single-page log viewer served at port **8081** by `sovd_server_comp`.
It talks to the SOVD API on port **8080** to fetch log entries and manage configuration.

```
Browser  ──────────────────────►  http://<WSL2-IP>:8081/       (UI)
Browser  ──────────────────────►  http://<WSL2-IP>:8080/sovd/v1/  (SOVD API)
```

---

## Step 1 — Configure the API URL

The UI has a hardcoded API base URL near the top of
`diagnostic_components/sovd_server_comp/static/index.html`:

```js
const API = 'http://172.26.10.72:8080/sovd/v1/apps/diag-app/logs';
```

This must point to your WSL2 IP. To find it:

```bash
ip addr show eth0 | grep 'inet '
# Example: inet 172.26.10.72/20 ...
```

Update the `const API` line with your IP if it has changed, then rebuild:

```bash
cd ~/hackathon/TRAceON
cargo build -p sovd_server_comp
```

> The HTML is embedded at compile time via `include_str!` — a rebuild is required
> after every change to `index.html`.

---

## Step 2 — Run the server

```bash
cd ~/hackathon/TRAceON
cargo run -p sovd_server_comp
```

Expected output:

```
INFO UI available at http://127.0.0.1:8081/
INFO SOVD API at http://127.0.0.1:8080/sovd/v1/
INFO Log source: REST log sink
```

---

## Step 3 — Open the UI

In your **Windows browser**, navigate to:

```
http://<WSL2-IP>:8081/
```

Example: `http://172.26.10.72:8081/`

> Use the WSL2 IP, not `127.0.0.1` — WSL2 has its own network namespace and
> `127.0.0.1` from Windows does not reach it. The IP changes on every reboot;
> re-run the `ip addr` command above to get the current one.

---

## UI Features

### Log Table (left panel)

| Column | Description |
|---|---|
| Sev | Severity badge — FATAL / ERROR / WARN / INFO / DEBUG |
| Timestamp | Local time of the log entry |
| Context | RFC5424 (host · process) or AUTOSAR DLT (app-id / ctx-id) |
| Message | Log message text |

- **Severity filter** — dropdown to show only entries at or above a level
- **Search** — filters rows by message text (client-side, instant)
- **Auto checkbox** — toggles auto-refresh every 3 seconds
- **↻ Refresh** — manual fetch
- **Click any row** — opens a JSON detail drawer at the bottom of the page

### Log Configuration (right panel)

Controls the server-side severity threshold per context type.

| Control | Description |
|---|---|
| Context Type | Select `RFC 5424` or `AUTOSAR DLT` |
| Min Severity | Minimum severity the server will return for that context |
| Apply | Sends `PUT /sovd/v1/apps/diag-app/logs/config` |
| Reset | Sends `DELETE /sovd/v1/apps/diag-app/logs/config` — restores defaults |

### Connection indicator (top-right)

| Colour | Meaning |
|---|---|
| Green dot | API reachable — last fetch succeeded |
| Red dot | API unreachable — check server is running and IP is correct |

---

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `TRACEON_LOG_SOURCE` | _(unset)_ | Omit for REST sink mode; set to `uprotocol` for Zenoh RPC mode |
| `RUST_LOG` | `info` | Server log level (`trace` / `debug` / `info` / `warn` / `error`) |

---

## Changing the API target at runtime (without rebuild)

If you want to point the UI at a different host without rebuilding, open the browser
DevTools console and override the constant:

```js
// Paste in browser DevTools console — takes effect on next fetch
window.__API = 'http://192.168.1.50:8080/sovd/v1/apps/diag-app/logs';
```

Then edit `index.html` to read `window.__API ?? 'http://...'` and rebuild for a
permanent change.
