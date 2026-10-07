# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
"""Log reading tests.

Verifies ``GET /logs/entries`` returns a well-formed ``items`` collection and
that each entry carries the required SOVD fields.
"""

import pytest

from conftest import REQUEST_TIMEOUT, SEVERITY_RANK

pytestmark = [pytest.mark.integration, pytest.mark.sovd_logging]


def test_read_log_entries(sovd_session, logs_base):
    """GET /logs/entries returns 200 with an items list."""
    response = sovd_session.get(f"{logs_base}/entries", timeout=REQUEST_TIMEOUT)

    assert response.status_code == 200
    body = response.json()
    assert "items" in body
    assert isinstance(body["items"], list)


def test_log_entry_shape(sovd_session, logs_base):
    """Each entry carries timestamp, context, severity and msg."""
    body = sovd_session.get(f"{logs_base}/entries", timeout=REQUEST_TIMEOUT).json()
    items = body["items"]

    if not items:
        pytest.skip("server returned no log entries to inspect")

    entry = items[0]
    assert "timestamp" in entry
    assert "severity" in entry
    assert "msg" in entry

    # Context is an object with a discriminating "type" field.
    assert "context" in entry
    assert "type" in entry["context"]


def test_entry_severities_are_known(sovd_session, logs_base):
    """Every entry's severity is one of the SOVD severity tokens."""
    body = sovd_session.get(f"{logs_base}/entries", timeout=REQUEST_TIMEOUT).json()

    for entry in body["items"]:
        assert entry["severity"] in SEVERITY_RANK, (
            f"unexpected severity token: {entry['severity']!r}"
        )


def test_entry_timestamps_are_iso8601(sovd_session, logs_base):
    """Timestamps parse as ISO-8601 / RFC 3339 instants."""
    from datetime import datetime

    body = sovd_session.get(f"{logs_base}/entries", timeout=REQUEST_TIMEOUT).json()

    for entry in body["items"]:
        ts = entry["timestamp"].replace("Z", "+00:00")
        # Raises ValueError on a malformed timestamp, failing the test.
        datetime.fromisoformat(ts)
