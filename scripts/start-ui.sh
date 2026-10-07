#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
#
# start-ui.sh — Start the SOVD server and open the log viewer UI in the browser.
#
# Usage:
#   ./scripts/start-ui.sh

set -euo pipefail

REPO_DIR="${HOME}/hackathon/TRAceON"

# ---------------------------------------------------------------------------
# Write temp script for the server tab
# ---------------------------------------------------------------------------
cat > /tmp/traceon-ui-server.sh << EOF
#!/usr/bin/env bash
source \$HOME/.cargo/env
echo "==> Starting sovd_server_comp..."
cd ${REPO_DIR}
cargo run -p sovd_server_comp
EOF
chmod +x /tmp/traceon-ui-server.sh

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
# Get WSL2 IP
# ---------------------------------------------------------------------------
WSL_IP=$(ip addr show eth0 | grep 'inet ' | awk '{print $2}' | cut -d/ -f1)
UI_URL="http://${WSL_IP}:8082/"

# ---------------------------------------------------------------------------
# Start server in a new terminal tab
# ---------------------------------------------------------------------------
echo "==> Opening server terminal..."
if command -v wt.exe >/dev/null 2>&1; then
  wt.exe new-tab --title "TRAceON: SOVD Server" wsl.exe bash /tmp/traceon-ui-server.sh &
elif command -v cmd.exe >/dev/null 2>&1; then
  cmd.exe /c start wsl.exe bash /tmp/traceon-ui-server.sh &
else
  echo "ERROR: Cannot open a new terminal. Start the server manually:"
  echo "  cd ${REPO_DIR} && cargo run -p sovd_server_comp"
  exit 1
fi

# ---------------------------------------------------------------------------
# Wait for server then open browser
# ---------------------------------------------------------------------------
wait_for_port "127.0.0.1" "8082" "UI server"

echo ""
echo "============================================"
echo " TRAceON — Log Viewer UI"
echo "--------------------------------------------"
echo " URL: ${UI_URL}"
echo "============================================"
echo "==> Opening browser..."

# Open browser from Windows
if command -v cmd.exe >/dev/null 2>&1; then
  cmd.exe /c start "${UI_URL}"
elif command -v explorer.exe >/dev/null 2>&1; then
  explorer.exe "${UI_URL}"
else
  echo "Could not open browser automatically."
  echo "Open manually: ${UI_URL}"
fi

echo "==> Done. UI is running at ${UI_URL}"
