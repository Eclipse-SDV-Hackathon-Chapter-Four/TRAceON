# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
"""Error-handling tests.

Verifies the server rejects malformed and unknown requests with the expected
4xx status codes rather than failing opaquely.
"""

import pytest

from conftest import API_PREFIX, REQUEST_TIMEOUT

pytestmark = [pytest.mark.integration, pytest.mark.sovd_logging]


def test_unknown_app_returns_404(sovd_session, server_url):
    """A logging request for an unknown app yields 404 (EntityNotFound)."""
    url = f"{server_url}{API_PREFIX}/apps/does-not-exist/logs/entries"
    response = sovd_session.get(url, timeout=REQUEST_TIMEOUT)

    assert response.status_code == 404


def test_invalid_time_window_returns_400(sovd_session, logs_base):
    """created-after >= created-before is rejected as a bad request."""
    response = sovd_session.get(
        f"{logs_base}/entries",
        params={
            "created-after": "2030-01-01T00:00:00Z",
            "created-before": "2020-01-01T00:00:00Z",
        },
        timeout=REQUEST_TIMEOUT,
    )

    assert response.status_code == 400


def test_invalid_severity_value_rejected(sovd_session, logs_base):
    """An unknown severity token is rejected (query deserialization fails)."""
    response = sovd_session.get(
        f"{logs_base}/entries",
        params={"severity": "not-a-level"},
        timeout=REQUEST_TIMEOUT,
    )

    assert response.status_code in (400, 422)


def test_malformed_timestamp_rejected(sovd_session, logs_base):
    """A non-ISO timestamp in a filter is rejected rather than ignored."""
    response = sovd_session.get(
        f"{logs_base}/entries",
        params={"created-after": "yesterday"},
        timeout=REQUEST_TIMEOUT,
    )

    assert response.status_code in (400, 422)


def test_empty_configuration_rejected(sovd_session, logs_base):
    """PUT /logs/config with an empty items list is a bad request."""
    response = sovd_session.put(
        f"{logs_base}/config",
        json={"items": []},
        timeout=REQUEST_TIMEOUT,
    )

    assert response.status_code == 400
