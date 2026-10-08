#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
#
# stream-logs.sh — Launch the full TRAceON live streaming pipeline.
#
# Usage:
#   ./scripts/stream-logs.sh [severity]
#
#   severity — DLT_DEBUG | DLT_INFO | DLT_WARN | DLT_ERROR (default) | DLT_FATAL
#
# Environment variables:
#   MQTT_HOST — IP of the MQTT broker host (default: 192.168.88.254)

set -euo pipefail

SEV="${1:-DLT_ERROR}"
MQTT_HOST="${MQTT_HOST:-192.168.88.254}"
REPO_DIR="${HOME}/hackathon/TRAceON"
THREADX_DIR="${HOME}/TRAceON-ThreadX"
TELEMETRY_DIR="${THREADX_DIR}/telemetry-server"

echo "============================================"
echo " TRAceON — Live Stream Launcher"
echo "--------------------------------------------"
echo " Severity : ${SEV}"
echo " MQTT Host: ${MQTT_HOST}"
echo "============================================"

# ---------------------------------------------------------------------------
# Write temp scripts for each tab
# ---------------------------------------------------------------------------

# Tab 1
cat > /tmp/traceon-tab1.sh << EOF
#!/usr/bin/env bash
source \$HOME/.cargo/env
echo "==> Starting sovd_server_comp..."
cd ${REPO_DIR}
cargo run -p sovd_server_comp
EOF

# Tab 2
cat > /tmp/traceon-tab2.sh << EOF
#!/usr/bin/env bash
set -e
THREADX_DIR="${THREADX_DIR}"
TELEMETRY_DIR="${TELEMETRY_DIR}"
if [ ! -d "\${THREADX_DIR}" ]; then
  echo "==> Cloning TRAceON-ThreadX..."
  git clone https://github.com/Eclipse-SDV-Hackathon-Chapter-Four/TRAceON-ThreadX.git "\${THREADX_DIR}"
else
  echo "==> TRAceON-ThreadX already present, skipping clone."
fi
if [ ! -f "\${TELEMETRY_DIR}/.venv/bin/python" ]; then
  echo "==> Setting up venv..."
  cd "\${TELEMETRY_DIR}" && ./setup.sh
else
  echo "==> Venv already set up, skipping."
fi
echo "==> Starting telemetry server (MQTT: ${MQTT_HOST})..."
cd "\${TELEMETRY_DIR}"
TRACEON_MQTT_HOST="${MQTT_HOST}" \
TRACEON_HTTP_PORT=8083 \
TRACEON_LOG_FORWARD_URL=http://127.0.0.1:8080/internal/logs \
./run.sh
EOF

# Tab 3
cat > /tmp/traceon-tab3.sh << EOF
#!/usr/bin/env bash
echo "==> Enabling log forwarding..."
curl -s -X POST localhost:8083/logs/forwarding/start ; echo
echo "==> Forwarding status:"
curl -s localhost:8083/logs/forwarding ; echo
echo ""
echo "============================================"
echo " TRAceON — Live SSE Stream"
echo "--------------------------------------------"
echo " App      : hw-log"
echo " Severity : ${SEV} and above"
echo "============================================"
echo " Press Ctrl+C to stop"
echo ""
curl -N --no-buffer 'http://127.0.0.1:8080/sovd/v1/apps/hw-log/logs/entries/stream?severity=${SEV}'
EOF

chmod +x /tmp/traceon-tab1.sh /tmp/traceon-tab2.sh /tmp/traceon-tab3.sh

# ---------------------------------------------------------------------------
# Helper: open a new Windows Terminal tab
# ---------------------------------------------------------------------------
open_terminal() {
  local title="$1"
  local script="$2"
  if command -v wt.exe >/dev/null 2>&1; then
    wt.exe new-tab --title "${title}" wsl.exe bash "${script}" &
  elif command -v cmd.exe >/dev/null 2>&1; then
    cmd.exe /c start wsl.exe bash "${script}" &
  else
    echo "ERROR: Cannot open new terminal windows automatically."
    exit 1
  fi
}

# ---------------------------------------------------------------------------
# Helper: wait until a port is open
# ---------------------------------------------------------------------------
wait_for_port() {
  local host="$1" port="$2" name="$3" tries=120
  echo "==> Waiting for ${name} on ${host}:${port}..."
  until bash -c "exec 3<>/dev/tcp/${host}/${port}" 2>/dev/null; do
    tries=$((tries - 1))
    [ "${tries}" -le 0 ] && echo "ERROR: ${name} did not start in time." >&2 && exit 1
    sleep 2
  done
  echo "    ${name} is up."
}

# ---------------------------------------------------------------------------
# Launch tabs
# ---------------------------------------------------------------------------
echo "==> Opening Tab 1: SOVD server..."
open_terminal "TRAceON: SOVD Server" /tmp/traceon-tab1.sh
wait_for_port "127.0.0.1" "8080" "sovd_server_comp"

echo "==> Opening Tab 2: Telemetry server..."
open_terminal "TRAceON: Telemetry Server" /tmp/traceon-tab2.sh
wait_for_port "127.0.0.1" "8083" "telemetry-server"

echo "==> Opening Tab 3: Live SSE stream..."
open_terminal "TRAceON: Live Stream" /tmp/traceon-tab3.sh

echo ""
echo "==> All tabs launched."
echo "    UI available at: http://172.26.10.72:8082/"
