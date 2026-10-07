# TRAceON — Live Stream Setup Guide

End-to-end instructions to receive live logs from the **TRAceON-ThreadX** hardware ECU
and view them in the browser UI.

---

## Architecture

```
ECU (ThreadX RTOS)
  │  MQTT publish  (broker on Mac: 192.168.88.254)
  ▼
TRAceON-ThreadX telemetry-server   (WSL, port 8083)
  │  POST /internal/logs
  ▼
sovd_server_comp                   (WSL, port 8080 — SOVD API)
  │  SSE stream / REST
  ▼
Browser UI                         (WSL, port 8081 — Log Viewer)
```

---

## Prerequisites (one-time)

Open a WSL terminal and run:

```bash
sudo apt update && sudo apt install -y python3 python3-venv python3-pip git
```

> If you don't have sudo access, Python 3 is likely already present.
> Skip this step and continue — the setup script will tell you if anything is missing.

---

## Step 1 — Clone TRAceON-ThreadX

```bash
cd ~
git clone https://github.com/Eclipse-SDV-Hackathon-Chapter-Four/TRAceON-ThreadX.git
cd TRAceON-ThreadX
```

---

## Step 2 — Set up the telemetry server (one-time)

```bash
cd ~/TRAceON-ThreadX/telemetry-server
./setup.sh
```

This creates a `.venv` and installs `fastapi`, `uvicorn`, `paho-mqtt`, and `pydantic`.

---

## Step 3 — Start the SOVD server

Open a new WSL pane/tab and run from the TRAceON workspace root:

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

## Step 4 — Start the telemetry server

Open another WSL pane and run:

```bash
cd ~/TRAceON-ThreadX/telemetry-server
TRACEON_MQTT_HOST=192.168.88.254 \
TRACEON_HTTP_PORT=8083 \
TRACEON_LOG_FORWARD_URL=http://127.0.0.1:8080/internal/logs \
./run.sh
```

| Variable | Example | Description |
|---|---|---|
| `TRACEON_MQTT_HOST` | `192.168.88.254` | Host IP of the machine running the MQTT broker |
| `TRACEON_HTTP_PORT` | `8083` | Port the telemetry server listens on |
| `TRACEON_LOG_FORWARD_URL` | `http://127.0.0.1:8080/internal/logs` | SOVD server log sink |

---

## Step 5 — Enable log forwarding

Open a third WSL pane and run:

```bash
# Start forwarding MQTT events → SOVD sink
curl -s -X POST localhost:8083/logs/forwarding/start ; echo

# Confirm forwarding is active and watch the 'forwarded' counter rise
curl -s localhost:8083/logs/forwarding ; echo
```

---

## Step 6 — Live SSE stream (curl)

To tail the raw SSE stream directly:

```bash
curl -N --no-buffer \
  'http://127.0.0.1:8080/sovd/v1/apps/diag-app/logs/entries/stream?severity=DLT_ERROR'
```

Filter options for `?severity=`:

| Value | Meaning |
|---|---|
| `DLT_FATAL` | Fatal only |
| `DLT_ERROR` | Error and above |
| `DLT_WARN` | Warn and above |
| `DLT_INFO` | Info and above |
| `DLT_DEBUG` | All entries |
