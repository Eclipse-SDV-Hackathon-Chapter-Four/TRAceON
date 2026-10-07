# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
"""Logging service discovery tests.

Verifies the logging resource is advertised and that it links to the
``entries``, ``config`` and live-stream sub-resources.
"""

from urllib.parse import urlparse

import pytest

from conftest import API_PREFIX, REQUEST_TIMEOUT

pytestmark = [pytest.mark.smoke, pytest.mark.sovd_logging]


def test_logging_resource_available(sovd_session, logs_base):
    """GET /logs returns 200 and advertises the entries + config URIs."""
    response = sovd_session.get(logs_base, timeout=REQUEST_TIMEOUT)

    assert response.status_code == 200
    body = response.json()

    # Response<LogResources> is flattened, so fields sit at the top level.
    assert "entries" in body
    assert "config" in body


def test_logging_resource_links_are_well_formed(sovd_session, logs_base, app_id):
    """The advertised URIs point at the expected logging sub-resources."""
    body = sovd_session.get(logs_base, timeout=REQUEST_TIMEOUT).json()

    entries_path = urlparse(body["entries"]).path
    config_path = urlparse(body["config"]).path

    assert entries_path.endswith(f"{API_PREFIX}/apps/{app_id}/logs/entries")
    assert config_path.endswith(f"{API_PREFIX}/apps/{app_id}/logs/config")


def test_logging_resource_advertises_live_stream(sovd_session, logs_base, app_id):
    """The OpenSOVD live-log extension link is present and points at /stream."""
    body = sovd_session.get(logs_base, timeout=REQUEST_TIMEOUT).json()

    live = body.get("x-opensovd-live-entries")
    assert live, "expected x-opensovd-live-entries link in the logging resource"
    assert urlparse(live).path.endswith(
        f"{API_PREFIX}/apps/{app_id}/logs/entries/stream"
    )


def test_advertised_entries_uri_is_reachable(sovd_session, logs_base):
    """Following the advertised entries URI yields an items collection."""
    body = sovd_session.get(logs_base, timeout=REQUEST_TIMEOUT).json()

    entries = sovd_session.get(body["entries"], timeout=REQUEST_TIMEOUT)
    assert entries.status_code == 200
    assert "items" in entries.json()
