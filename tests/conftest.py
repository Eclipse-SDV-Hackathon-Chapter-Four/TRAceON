# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
"""Shared pytest fixtures for the TRAceON SOVD logging API test suite.

The suite exercises the logging resource exposed by ``sovd_server_comp``:

    GET    /sovd/v1/apps/{app}/logs
    GET    /sovd/v1/apps/{app}/logs/entries[?severity=&created-after=&created-before=]
    GET    /sovd/v1/apps/{app}/logs/entries/stream   (SSE live stream)
    GET    /sovd/v1/apps/{app}/logs/config
    PUT    /sovd/v1/apps/{app}/logs/config
    DELETE /sovd/v1/apps/{app}/logs/config
    POST   /internal/logs                             (hardware ingest sink)

Responses use the OpenSOVD envelope with ``#[serde(flatten)]``, so collection
payloads surface their fields at the top level:

    GET /logs           -> {"entries": <uri>, "config": <uri>, "x-opensovd-live-entries": <uri>}
    GET /logs/entries   -> {"items": [ {timestamp, context:{type,...}, severity, msg, href?} ]}
    GET /logs/config    -> {"contexts": [ {context:{type,...}, severity} ]}

Severity values are lowercase for RFC 5424 (``info``, ``error``, ...) and
``DLT_*`` for AUTOSAR DLT contexts.
"""

import os

import pytest
import requests

# --------------------------------------------------------------------------- #
# Configuration
# --------------------------------------------------------------------------- #

# Base server URL. Override with TRACEON_BASE_URL for SIL / OpenDUT / CI.
BASE_URL = os.environ.get("TRACEON_BASE_URL", "http://localhost:8080").rstrip("/")

# App id registered by sovd_server_comp (see README / main.rs).
APP_ID = os.environ.get("TRACEON_APP_ID", "diag-app")

# Versioned SOVD mount path.
API_PREFIX = "/sovd/v1"

# Default per-request timeout in seconds.
REQUEST_TIMEOUT = float(os.environ.get("TRACEON_REQUEST_TIMEOUT", "5"))


# --------------------------------------------------------------------------- #
# Fixtures
# --------------------------------------------------------------------------- #


@pytest.fixture(scope="session")
def server_url():
    """Base URL of the SOVD server under test."""
    return BASE_URL


@pytest.fixture(scope="session")
def app_id():
    """App id whose logging resource is under test."""
    return APP_ID


@pytest.fixture(scope="session")
def logs_base(server_url, app_id):
    """Fully-qualified base URL of the app's logging resource."""
    return f"{server_url}{API_PREFIX}/apps/{app_id}/logs"


@pytest.fixture(scope="session")
def sink_url(server_url):
    """URL of the hardware ingest sink (POST JSON log events)."""
    return f"{server_url}/internal/logs"


@pytest.fixture(scope="session")
def sovd_session():
    """A requests session with JSON Accept headers, reused across the suite."""
    session = requests.Session()
    session.headers.update({"Accept": "application/json"})
    yield session
    session.close()


@pytest.fixture(scope="session", autouse=True)
def _require_server(sovd_session, logs_base):
    """Skip the whole suite with a clear message if the server is unreachable."""
    try:
        response = sovd_session.get(logs_base, timeout=REQUEST_TIMEOUT)
    except requests.exceptions.RequestException as exc:
        pytest.skip(
            f"SOVD server not reachable at {logs_base}: {exc}. "
            "Start it with `cargo run -p sovd_server_comp` or set TRACEON_BASE_URL."
        )
    if response.status_code != 200:
        pytest.skip(
            f"Logging resource not available at {logs_base} "
            f"(status {response.status_code})."
        )


# --------------------------------------------------------------------------- #
# Helpers (importable by test modules)
# --------------------------------------------------------------------------- #

# SOVD severity ordering: lower rank == more severe. Mirrors severity_rank in
# log_provider.rs. A query ``?severity=X`` keeps entries at least as severe as X.
SEVERITY_RANK = {
    "fatal": 0,
    "error": 1,
    "warn": 2,
    "info": 3,
    "debug": 4,
    "DLT_FATAL": 0,
    "DLT_ERROR": 1,
    "DLT_WARN": 2,
    "DLT_INFO": 3,
    "DLT_DEBUG": 4,
}
