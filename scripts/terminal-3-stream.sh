#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
#
# terminal-3-stream.sh — Enable forwarding and tail the live SSE stream.
# Run this in Terminal 3 after terminal-1 and terminal-2 are both running.
#
# Usage:
#   ./scripts/terminal-3-stream.sh [severity]
#
#   severity — DLT_DEBUG | DLT_INFO | DLT_WARN | DLT_ERROR (default) | DLT_FATAL

set -euo pipefail

SEV="${1:-DLT_ERROR}"
SOVD_APP="hw-log"
STREAM_URL="http://127.0.0.1:8080/sovd/v1/apps/${SOVD_APP}/logs/entries/stream?severity=${SEV}"

# ---------------------------------------------------------------------------
# 1. Check SOVD server is up
# ---------------------------------------------------------------------------
if ! bash -c "exec 3<>/dev/tcp/127.0.0.1/8080" 2>/dev/null; then
  echo "ERROR: sovd_server_comp is not running on port 8080."
  echo "       Run: ./scripts/terminal-1-sovd-server.sh"
  exit 1
fi

# ---------------------------------------------------------------------------
# 2. Check telemetry server is up
# ---------------------------------------------------------------------------
if ! bash -c "exec 3<>/dev/tcp/127.0.0.1/8083" 2>/dev/null; then
  echo "ERROR: telemetry-server is not running on port 8083."
  echo "       Run: ./scripts/terminal-2-telemetry.sh"
  exit 1
fi

# ---------------------------------------------------------------------------
# 3. Enable forwarding
# ---------------------------------------------------------------------------
echo "==> Enabling log forwarding..."
curl -s -X POST localhost:8083/logs/forwarding/start ; echo
echo "==> Forwarding status:"
curl -s localhost:8083/logs/forwarding ; echo

# ---------------------------------------------------------------------------
# 4. Tail the live SSE stream
# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo " TRAceON — Live SSE Stream"
echo "--------------------------------------------"
echo " App      : ${SOVD_APP}"
echo " Severity : ${SEV} and above"
echo " URL      : ${STREAM_URL}"
echo "============================================"
echo " Press Ctrl+C to stop"
echo ""

curl -N --no-buffer "${STREAM_URL}"
