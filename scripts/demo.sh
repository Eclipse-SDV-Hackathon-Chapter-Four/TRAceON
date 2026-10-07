#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
#
# TRAceON end-to-end demo: two log sources behind one SOVD server.
#
#   Source A: app "diag-app"  -> logs fetched from the dummy ECU over
#             uProtocol/Zenoh (process: dummy_diag_app).
#   Source B: app "hw-log"    -> logs pushed into the REST sink, with live
#             SSE streaming (process: sovd_server_comp).
#
# The script starts both processes on loopback, pushes a few hardware events
# into the sink, then exercises the SOVD logs API (entries, config, SSE) for
# both apps. Everything is torn down on exit.
#
# Requirements: a Rust toolchain (cargo) and curl. Run from the repo root:
#   ./scripts/demo.sh

set -euo pipefail

SOVD="http://127.0.0.1:8080/sovd/v1"
SINK="http://127.0.0.1:8080/internal/logs"
ZENOH_ENDPOINT="${APP_ZENOH_LISTEN:-tcp/127.0.0.1:7447}"

PIDS=()
cleanup() {
  echo
  echo "==> Tearing down demo processes"
  for pid in "${PIDS[@]:-}"; do
    [ -n "${pid}" ] && kill "${pid}" 2>/dev/null || true
  done
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

echo "==> Building workspace"
cargo build -p dummy_diag_app -p sovd_server_comp

# ---------------------------------------------------------------------------
# 1. Start the dummy ECU (uProtocol/Zenoh RPC server)
# ---------------------------------------------------------------------------
echo "==> Starting dummy_diag_app (ECU) on ${ZENOH_ENDPOINT}"
APP_ZENOH_LISTEN="${ZENOH_ENDPOINT}" cargo run -q -p dummy_diag_app &
PIDS+=("$!")
wait_for_port "127.0.0.1" "7447" "dummy_diag_app Zenoh listener"

# ---------------------------------------------------------------------------
# 2. Start the SOVD server (connects to the ECU, serves both apps)
# ---------------------------------------------------------------------------
echo "==> Starting sovd_server_comp (SOVD server) on 127.0.0.1:8080"
APP_ZENOH_CONNECT="${ZENOH_ENDPOINT}" cargo run -q -p sovd_server_comp &
PIDS+=("$!")
wait_for_port "127.0.0.1" "8080" "sovd_server_comp HTTP"

sleep 1

# ---------------------------------------------------------------------------
# 3. Push a few hardware events into the REST sink (feeds app "hw-log")
# ---------------------------------------------------------------------------
echo
echo "==> Pushing hardware events into the REST sink (POST ${SINK})"
now_ms() { date -u +%Y-%m-%dT%H:%M:%SZ; }
for sev in info warn error; do
  curl -fsS -X POST "${SINK}" \
    -H 'content-type: application/json' \
    -d "{\"timestamp\":\"$(now_ms)\",\"severity\":\"${sev}\",\"msg\":\"hw ${sev} event\",\"context\":{\"type\":\"RFC5424\",\"host\":\"hw-host\",\"process\":\"hw-daemon\"}}" \
    >/dev/null && echo "    pushed ${sev}"
done

# ---------------------------------------------------------------------------
# 4. Exercise the SOVD logs API for both apps
# ---------------------------------------------------------------------------
echo
echo "==> Source A: diag-app (uProtocol/Zenoh ECU)"
echo "    GET ${SOVD}/apps/diag-app/logs/entries?severity=warn"
curl -fsS "${SOVD}/apps/diag-app/logs/entries?severity=warn&include-schema=true" | sed 's/^/      /'

echo
echo "==> Source B: hw-log (REST sink)"
echo "    GET ${SOVD}/apps/hw-log/logs/entries?severity=warn"
curl -fsS "${SOVD}/apps/hw-log/logs/entries?severity=warn" | sed 's/^/      /'

echo
echo "==> hw-log config round-trip (PUT then DELETE)"
echo "    PUT ${SOVD}/apps/hw-log/logs/config"
curl -fsS -o /dev/null -w "      PUT -> HTTP %{http_code}\n" \
  -X PUT "${SOVD}/apps/hw-log/logs/config" \
  -H 'content-type: application/json' \
  -d '{"items":[{"context":{"type":"RFC5424","host":"hw-host","process":"hw-daemon"},"severity":"warn"}]}'
echo "    DELETE ${SOVD}/apps/hw-log/logs/config"
curl -fsS -o /dev/null -w "      DELETE -> HTTP %{http_code}\n" \
  -X DELETE "${SOVD}/apps/hw-log/logs/config"

# ---------------------------------------------------------------------------
# 5. Live SSE tail on hw-log while a new event is pushed
# ---------------------------------------------------------------------------
echo
echo "==> Live SSE tail on hw-log (GET ${SOVD}/apps/hw-log/logs/entries/stream)"
echo "    (tailing for 3s; a fresh 'error' event is pushed after 1s)"
( curl -fsS -N --max-time 3 "${SOVD}/apps/hw-log/logs/entries/stream" | sed 's/^/      SSE: /' ) &
SSE_PID=$!
sleep 1
curl -fsS -X POST "${SINK}" \
  -H 'content-type: application/json' \
  -d "{\"timestamp\":\"$(now_ms)\",\"severity\":\"error\",\"msg\":\"live hw error\",\"context\":{\"type\":\"RFC5424\",\"host\":\"hw-host\",\"process\":\"hw-daemon\"}}" \
  >/dev/null && echo "    pushed live error event"
wait "${SSE_PID}" 2>/dev/null || true

echo
echo "==> Demo complete"
