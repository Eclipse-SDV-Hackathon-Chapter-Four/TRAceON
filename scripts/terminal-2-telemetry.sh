#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
#
# terminal-2-telemetry.sh — Clone TRAceON-ThreadX (if needed), set up venv, start telemetry server.
# Run this in Terminal 2 after terminal-1-sovd-server.sh is up.
#
# Environment variables (override defaults):
#   MQTT_HOST — IP of the machine running the MQTT broker (default: 192.168.88.254)

set -euo pipefail

MQTT_HOST="${MQTT_HOST:-192.168.88.254}"
THREADX_DIR="${HOME}/TRAceON-ThreadX"
TELEMETRY_DIR="${THREADX_DIR}/telemetry-server"

# ---------------------------------------------------------------------------
# 1. Check SOVD server is up
# ---------------------------------------------------------------------------
if ! bash -c "exec 3<>/dev/tcp/127.0.0.1/8080" 2>/dev/null; then
  echo "ERROR: sovd_server_comp is not running on port 8080."
  echo "       Start Terminal 1 first:"
  echo "         ./scripts/terminal-1-sovd-server.sh"
  exit 1
fi
echo "==> SOVD server is up on port 8080."

# ---------------------------------------------------------------------------
# 2. Clone TRAceON-ThreadX if not already present
# ---------------------------------------------------------------------------
if [ ! -d "${THREADX_DIR}" ]; then
  echo "==> Cloning TRAceON-ThreadX into ${THREADX_DIR}..."
  git clone https://github.com/Eclipse-SDV-Hackathon-Chapter-Four/TRAceON-ThreadX.git "${THREADX_DIR}"
else
  echo "==> TRAceON-ThreadX already present at ${THREADX_DIR}, skipping clone."
fi

# ---------------------------------------------------------------------------
# 3. Set up Python venv if not already done
# ---------------------------------------------------------------------------
if [ ! -f "${TELEMETRY_DIR}/.venv/bin/python" ]; then
  echo "==> Setting up telemetry-server venv..."
  ( cd "${TELEMETRY_DIR}" && ./setup.sh )
else
  echo "==> Telemetry-server venv already set up, skipping."
fi

# ---------------------------------------------------------------------------
# 4. Start telemetry server
# ---------------------------------------------------------------------------
echo "==> Starting telemetry server (MQTT broker: ${MQTT_HOST}, port 8083)..."
echo "    Forwarding logs to http://127.0.0.1:8080/internal/logs"
cd "${TELEMETRY_DIR}"
TRACEON_MQTT_HOST="${MQTT_HOST}" \
TRACEON_HTTP_PORT=8083 \
TRACEON_LOG_FORWARD_URL=http://127.0.0.1:8080/internal/logs \
./run.sh
