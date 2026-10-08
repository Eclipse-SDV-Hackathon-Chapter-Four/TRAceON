#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
#
# terminal-1-sovd-server.sh — Start the SOVD server.
# Run this in Terminal 1 first, then run terminal-2-telemetry.sh and terminal-3-stream.sh.

set -euo pipefail

REPO_DIR="${HOME}/hackathon/TRAceON"

if [ ! -d "${REPO_DIR}" ]; then
  echo "ERROR: Repo not found at ${REPO_DIR}"
  exit 1
fi

echo "==> Starting sovd_server_comp on port 8080 and UI on port 8082..."
cd "${REPO_DIR}"
cargo run -p sovd_server_comp
