# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
"""Performance smoke tests.

Lightweight latency checks on the entries endpoint. These are not rigorous
benchmarks; they guard against gross regressions (e.g. the in-memory ring
buffer query degrading into something slow).
"""

import time

import pytest

from conftest import REQUEST_TIMEOUT

pytestmark = [pytest.mark.performance, pytest.mark.sovd_logging]

# Max acceptable wall-clock latency for a single entries query, in seconds.
LATENCY_BUDGET = float(__import__("os").environ.get("TRACEON_LATENCY_BUDGET", "2.0"))


def test_entries_query_latency(sovd_session, logs_base):
    """A single GET /logs/entries completes within the latency budget."""
    start = time.perf_counter()
    response = sovd_session.get(f"{logs_base}/entries", timeout=REQUEST_TIMEOUT)
    elapsed = time.perf_counter() - start

    assert response.status_code == 200
    assert elapsed < LATENCY_BUDGET, (
        f"entries query took {elapsed:.3f}s, over budget of {LATENCY_BUDGET}s"
    )


def test_filtered_query_latency(sovd_session, logs_base):
    """A severity-filtered query also stays within the latency budget."""
    start = time.perf_counter()
    response = sovd_session.get(
        f"{logs_base}/entries",
        params={"severity": "error"},
        timeout=REQUEST_TIMEOUT,
    )
    elapsed = time.perf_counter() - start

    assert response.status_code == 200
    assert elapsed < LATENCY_BUDGET


def test_repeated_queries_are_stable(sovd_session, logs_base):
    """Ten sequential queries keep their average latency within budget."""
    durations = []
    for _ in range(10):
        start = time.perf_counter()
        response = sovd_session.get(f"{logs_base}/entries", timeout=REQUEST_TIMEOUT)
        durations.append(time.perf_counter() - start)
        assert response.status_code == 200

    average = sum(durations) / len(durations)
    assert average < LATENCY_BUDGET, f"average latency {average:.3f}s over budget"
