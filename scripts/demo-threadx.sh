#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
#
# TRAceON two-source demo: two real log sources behind ONE SOVD server, driven
# interactively.
#
#   Source A — app "diag-app":  logs fetched from the dummy ECU over
#              uProtocol/Zenoh (process: dummy_diag_app).
#   Source B — app "hw-log":    logs from the full Eclipse ThreadX telemetry
#              pipeline, forwarded into our SOVD sink, with live SSE:
#
#     board (send-log.sh / mock-log-publisher.sh over MQTT)
#        │ TRAceON/logs  (ISO 17978-3 LogEntry, DLT_* severities)
#        ▼
#     Mosquitto broker (Docker :1883)
#        │ subscribe (Eclipse Paho)
#        ▼
#     telemetry-server (Python/FastAPI :8083)  --forwarding ON-->
#        │ POST ISO LogEntry
#        ▼
#     sovd_server_comp (:8080)  POST /internal/logs  ->  DiagLogProvider
#        │
#        ▼
#     SOVD API:  GET /sovd/v1/apps/{diag-app,hw-log}/logs/entries[/stream]
#
# After everything is up, the script drops into an interactive menu so you can
# switch, at runtime, between reading diag-app, reading hw-log, tailing the live
# SSE stream, and injecting a fresh board log.
#
# Everything runs on loopback; no board, no WiFi. The board is simulated with
# the ThreadX project's own MQTT publishers (mosquitto_pub). If mosquitto_pub is
# not on PATH, the demo also checks ~/.local/bin.
#
# Run from the repo root:
#   ./scripts/demo-threadx.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
THREADX_DIR="${REPO_ROOT}/TRAceON-ThreadX"
TELEMETRY_DIR="${THREADX_DIR}/telemetry-server"
VENV_PY="${TELEMETRY_DIR}/.venv/bin/python"

SOVD="http://127.0.0.1:8080/sovd/v1"
SINK_URL="http://127.0.0.1:8080/internal/logs"
TELEMETRY="http://127.0.0.1:8083"
BROKER_NAME="traceon-broker"
ZENOH_ENDPOINT="${APP_ZENOH_LISTEN:-tcp/127.0.0.1:7447}"

# Make a user-local mosquitto_pub (apt-get download + extract) visible if present.
export PATH="$HOME/.local/bin:${PATH}"

PIDS=()
STARTED_BROKER=0

cleanup() {
  echo
  echo "==> Tearing down demo"
  curl -fsS -X POST "${TELEMETRY}/logs/forwarding/stop" >/dev/null 2>&1 || true
  for pid in "${PIDS[@]:-}"; do
    [ -n "${pid}" ] && kill "${pid}" 2>/dev/null || true
  done
  if [ "${STARTED_BROKER}" -eq 1 ]; then
    docker rm -f "${BROKER_NAME}" >/dev/null 2>&1 || true
  fi
  wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

wait_for_port() {
  local host="$1" port="$2" name="$3" tries=100
  echo "==> Waiting for ${name} (${host}:${port})"
  until bash -c "exec 3<>/dev/tcp/${host}/${port}" 2>/dev/null; do
    tries=$((tries - 1))
    if [ "${tries}" -le 0 ]; then
      echo "ERROR: ${name} did not come up on ${host}:${port}" >&2
      exit 1
    fi
    sleep 0.3
  done
}

# ---------------------------------------------------------------------------
# 0. Preconditions
# ---------------------------------------------------------------------------
command -v docker >/dev/null || { echo "ERROR: docker is required" >&2; exit 1; }
command -v cargo  >/dev/null || { echo "ERROR: cargo (Rust) is required; run 'source \$HOME/.cargo/env'" >&2; exit 1; }
command -v curl   >/dev/null || { echo "ERROR: curl is required" >&2; exit 1; }
HAVE_MOSQUITTO_PUB=1
command -v mosquitto_pub >/dev/null || {
  HAVE_MOSQUITTO_PUB=0
  echo "WARN: mosquitto_pub not found on PATH (nor ~/.local/bin); board injection disabled." >&2
}

if [ ! -x "${VENV_PY}" ]; then
  echo "==> Creating telemetry-server venv (one-time)"
  ( cd "${TELEMETRY_DIR}" && ./setup.sh )
fi

# ---------------------------------------------------------------------------
# 1. Broker: Eclipse Mosquitto in Docker
# ---------------------------------------------------------------------------
if docker ps --format '{{.Names}}' | grep -q "^${BROKER_NAME}$"; then
  echo "==> Reusing running broker '${BROKER_NAME}'"
else
  echo "==> Starting Mosquitto broker '${BROKER_NAME}' (Docker, :1883)"
  docker rm -f "${BROKER_NAME}" >/dev/null 2>&1 || true
  docker run -d --name "${BROKER_NAME}" -p 1883:1883 \
    -v "${THREADX_DIR}/scripts/mosquitto-docker.conf:/mosquitto/config/mosquitto.conf" \
    eclipse-mosquitto:2 >/dev/null
  STARTED_BROKER=1
fi
wait_for_port "127.0.0.1" "1883" "Mosquitto broker"

# ---------------------------------------------------------------------------
# 2. Source A — dummy ECU (uProtocol/Zenoh RPC server)
# ---------------------------------------------------------------------------
echo "==> Building workspace"
( cd "${REPO_ROOT}" && cargo build -q -p dummy_diag_app -p sovd_server_comp )

echo "==> Starting dummy_diag_app (ECU) on ${ZENOH_ENDPOINT}"
( cd "${REPO_ROOT}" && APP_ZENOH_LISTEN="${ZENOH_ENDPOINT}" cargo run -q -p dummy_diag_app ) &
PIDS+=("$!")
wait_for_port "127.0.0.1" "7447" "dummy_diag_app Zenoh listener"

# ---------------------------------------------------------------------------
# 3. The SOVD server: BOTH apps (diag-app via uProtocol, hw-log via sink/SSE)
# ---------------------------------------------------------------------------
echo "==> Starting sovd_server_comp (:8080) with BOTH sources"
( cd "${REPO_ROOT}" && APP_ZENOH_CONNECT="${ZENOH_ENDPOINT}" cargo run -q -p sovd_server_comp ) &
PIDS+=("$!")
wait_for_port "127.0.0.1" "8080" "sovd_server_comp HTTP"

# ---------------------------------------------------------------------------
# 4. Source B — ThreadX telemetry server, forwarding into our sink
# ---------------------------------------------------------------------------
echo "==> Starting telemetry-server (Python, :8083) with forward URL -> ${SINK_URL}"
(
  cd "${TELEMETRY_DIR}"
  TRACEON_MQTT_HOST=localhost \
  TRACEON_HTTP_PORT=8083 \
  TRACEON_LOG_FORWARD_URL="${SINK_URL}" \
  ./run.sh
) &
PIDS+=("$!")
wait_for_port "127.0.0.1" "8083" "telemetry-server HTTP"

echo "==> Enabling log forwarding (telemetry-server -> our sink)"
curl -fsS -X POST "${TELEMETRY}/logs/forwarding/start" | sed 's/^/      /'; echo

# Seed hw-log with an initial burst so the first read is not empty.
# NOTE: mock-log-publisher.sh runs forever (no COUNT support), so we seed with a
# fixed set of one-shot send-log.sh calls instead.
seed_hw_log() {
  local sl="${THREADX_DIR}/scripts/send-log.sh"
  BROKER=localhost "${sl}" DLT_INFO  SensorTask "Sensor sampling started"            >/dev/null 2>&1 || true
  BROKER=localhost "${sl}" DLT_WARN  SensorTask "Humidity reading out of range"       >/dev/null 2>&1 || true
  BROKER=localhost "${sl}" DLT_ERROR SensorTask "Acceleration improbable: 50000 mg"   >/dev/null 2>&1 || true
  BROKER=localhost "${sl}" DLT_INFO  MQTT       "Published telemetry batch"           >/dev/null 2>&1 || true
  BROKER=localhost "${sl}" DLT_WARN  MQTT       "Reconnect attempt 2"                 >/dev/null 2>&1 || true
}
if [ "${HAVE_MOSQUITTO_PUB}" -eq 1 ]; then
  echo "==> Seeding hw-log: publishing 5 board logs over MQTT"
  seed_hw_log
  sleep 1
fi

# ---------------------------------------------------------------------------
# Interactive helpers
# ---------------------------------------------------------------------------
read_app() {  # $1 = app id, $2 = severity
  local app="$1" sev="${2:-debug}"
  echo "---- GET ${SOVD}/apps/${app}/logs/entries?severity=${sev} ----"
  curl -fsS "${SOVD}/apps/${app}/logs/entries?severity=${sev}" \
    | "${VENV_PY}" -c "import sys,json
d=json.load(sys.stdin)
items=d.get('items',[])
print(f'({len(items)} entries)')
for e in items[-12:]:
    ctx=e.get('context',{})
    cid=ctx.get('context_id') or ctx.get('process') or ctx.get('type')
    print(f\"  {e.get('severity',''):<10} [{cid}] {e.get('msg','')}\")" 2>/dev/null \
    || echo "  (request failed — is ${app} available?)"
}

# Board logs published live during an SSE tail — one per second.
# Format: "severity|context|message".
LIVE_LOGS=(
  "DLT_INFO|SensorTask|live: sampling cycle started"
  "DLT_WARN|SensorTask|live: humidity reading out of range"
  "DLT_ERROR|SensorTask|live: brake temperature critical"
  "DLT_INFO|MQTT|live: telemetry batch published"
  "DLT_WARN|MQTT|live: broker reconnect attempt"
)

tail_sse() {  # $1 = app id, $2 = seconds
  local app="$1" secs="${2:-7}"
  echo "---- SSE tail ${SOVD}/apps/${app}/logs/entries/stream (${secs}s) ----"
  if [ "${HAVE_MOSQUITTO_PUB}" -eq 1 ] && [ "${app}" = "hw-log" ]; then
    # Open the live SSE tail first.
    ( curl -fsS -N --max-time "${secs}" "${SOVD}/apps/${app}/logs/entries/stream" | sed 's/^/  SSE: /' ) &
    local sse_pid=$!
    # Then publish one board log per second over MQTT; each should surface on
    # the stream above within a moment.
    sleep 1
    local entry sev ctx msg
    for entry in "${LIVE_LOGS[@]}"; do
      sev="${entry%%|*}"; ctx="${entry#*|}"; ctx="${ctx%%|*}"; msg="${entry##*|}"
      echo "  >> publish ${sev} [${ctx}] ${msg}"
      BROKER=localhost "${THREADX_DIR}/scripts/send-log.sh" "${sev}" "${ctx}" "${msg}" >/dev/null 2>&1 || true
      sleep 1
    done
    wait "${sse_pid}" 2>/dev/null || true
  else
    curl -fsS -N --max-time "${secs}" "${SOVD}/apps/${app}/logs/entries/stream" | sed 's/^/  SSE: /' || true
  fi
}

# --- Config endpoints (hw-log) ---------------------------------------------
# The DiagLogProvider matches config rules to entries by CONTEXT TYPE, so an
# AUTOSAR_DLT rule governs every ThreadX board log. The threshold keeps entries
# at that severity or more severe (e.g. "warn" keeps warn/error/fatal, drops
# info/debug).

config_get() {
  echo "---- GET ${SOVD}/apps/hw-log/logs/config ----"
  curl -fsS "${SOVD}/apps/hw-log/logs/config" \
    | "${VENV_PY}" -c "import sys,json
d=json.load(sys.stdin)
rows=d.get('contexts',[])
print(f'({len(rows)} context rules)')
for c in rows:
    ctx=c.get('context',{})
    cid=ctx.get('type')
    print(f\"  {c.get('severity',''):<10} context={cid}\")" 2>/dev/null \
    || echo "  (request failed)"
}

config_put() {  # $1 = severity (warn|error)
  local sev="$1"
  echo "---- PUT ${SOVD}/apps/hw-log/logs/config  (RFC5424 + AUTOSAR_DLT -> ${sev}) ----"
  echo "     (affects hw-log only; diag-app keeps its own config)"
  # Set the threshold for BOTH context rules so the filter applies to every
  # hw-log entry (ISO 17978-3 §7.21.4: body is an array of LogConfiguration,
  # one per context — matches the example in §7.21.5).
  curl -fsS -o /dev/null -w "  PUT -> HTTP %{http_code}\n" \
    -X PUT "${SOVD}/apps/hw-log/logs/config" \
    -H 'content-type: application/json' \
    -d "{\"items\":[{\"context\":{\"type\":\"RFC5424\"},\"severity\":\"${sev}\"},{\"context\":{\"type\":\"AUTOSAR_DLT\"},\"severity\":\"${sev}\"}]}" \
    || echo "  (request failed)"
  config_get
}

config_delete() {
  echo "---- DELETE ${SOVD}/apps/hw-log/logs/config  (reset) ----"
  curl -fsS -o /dev/null -w "  DELETE -> HTTP %{http_code}\n" \
    -X DELETE "${SOVD}/apps/hw-log/logs/config" \
    || echo "  (request failed)"
  config_get
}

# ---------------------------------------------------------------------------
# Interactive menu — switch sources at runtime
# ---------------------------------------------------------------------------
cat <<EOF

================================================================
TRAceON demo is UP. Two sources behind one SOVD server:
  • diag-app  → uProtocol/Zenoh ECU (dummy_diag_app)
  • hw-log    → Eclipse ThreadX pipeline (MQTT → forward → SSE)

Choose what to do (type a key, Enter to repeat last, q to quit):
  1) Read   diag-app entries        (uProtocol source)
  2) Read   hw-log entries          (ThreadX source)
  3) Tail   hw-log LIVE SSE (~7s)   (publishes board logs, one per second)

  Config affects the hw-log entity ONLY (per-entity, per ISO 17978-3 §7.21).
  diag-app has its own independent config and is NOT changed by these:
  w) PUT    hw-log config severity=warn   (both contexts; keep warn/error/fatal)
  e) PUT    hw-log config severity=error  (both contexts; keep error/fatal)
  g) GET    hw-log config                 (show current rules)
  d) DELETE hw-log config                 (reset to default)

  q) Quit (tears everything down)
================================================================
EOF

last=""
while true; do
  printf "demo> "
  if ! read -r choice; then break; fi
  [ -z "${choice}" ] && choice="${last}"
  last="${choice}"
  case "${choice}" in
    1) read_app diag-app warn ;;
    2) read_app hw-log debug ;;
    3) tail_sse hw-log 7 ;;
    w|W) config_put warn ;;
    e|E) config_put error ;;
    g|G) config_get ;;
    d|D) config_delete ;;
    q|Q) break ;;
    *) echo "  unknown option: ${choice}" ;;
  esac
done

echo "==> Bye"
