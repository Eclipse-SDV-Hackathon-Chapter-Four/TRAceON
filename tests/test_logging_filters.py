# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
"""Filtering and configuration tests.

Covers the query filters on ``GET /logs/entries`` (``severity``,
``created-after``, ``created-before``) and the per-context severity
configuration CRUD on ``GET/PUT/DELETE /logs/config``.

Severity filtering semantics (see log_provider.rs ``severity_at_least``):
a request for ``?severity=warn`` keeps entries that are *at least* as severe
as ``warn`` — that is warn, error and fatal — not only exact ``warn`` matches.
"""

import pytest

from conftest import REQUEST_TIMEOUT, SEVERITY_RANK

pytestmark = [pytest.mark.integration, pytest.mark.sovd_logging]


# RFC 5424 severity tokens accepted as query values.
@pytest.mark.parametrize("level", ["debug", "info", "warn", "error", "fatal"])
def test_filter_by_severity_threshold(sovd_session, logs_base, level):
    """?severity=X returns only entries at least as severe as X."""
    response = sovd_session.get(
        f"{logs_base}/entries",
        params={"severity": level},
        timeout=REQUEST_TIMEOUT,
    )

    assert response.status_code == 200

    threshold = SEVERITY_RANK[level]
    for entry in response.json()["items"]:
        rank = SEVERITY_RANK[entry["severity"]]
        assert rank <= threshold, (
            f"entry severity {entry['severity']!r} (rank {rank}) is less severe "
            f"than requested threshold {level!r} (rank {threshold})"
        )


def test_severity_filter_narrows_results(sovd_session, logs_base):
    """A stricter threshold never returns more entries than a looser one."""
    all_entries = sovd_session.get(
        f"{logs_base}/entries", timeout=REQUEST_TIMEOUT
    ).json()["items"]
    errors_only = sovd_session.get(
        f"{logs_base}/entries",
        params={"severity": "error"},
        timeout=REQUEST_TIMEOUT,
    ).json()["items"]

    assert len(errors_only) <= len(all_entries)


def test_filter_created_after(sovd_session, logs_base):
    """?created-after keeps only entries strictly newer than the bound."""
    from datetime import datetime, timezone

    items = sovd_session.get(
        f"{logs_base}/entries", timeout=REQUEST_TIMEOUT
    ).json()["items"]
    if len(items) < 2:
        pytest.skip("not enough entries to exercise created-after")

    # Use the oldest entry's timestamp as the lower bound (exclusive).
    timestamps = sorted(e["timestamp"].replace("Z", "+00:00") for e in items)
    bound = timestamps[0]

    response = sovd_session.get(
        f"{logs_base}/entries",
        params={"created-after": bound},
        timeout=REQUEST_TIMEOUT,
    )
    assert response.status_code == 200

    bound_dt = datetime.fromisoformat(bound).astimezone(timezone.utc)
    for entry in response.json()["items"]:
        ts = datetime.fromisoformat(
            entry["timestamp"].replace("Z", "+00:00")
        ).astimezone(timezone.utc)
        assert ts > bound_dt


def test_filter_created_before(sovd_session, logs_base):
    """?created-before keeps only entries strictly older than the bound."""
    from datetime import datetime, timezone

    items = sovd_session.get(
        f"{logs_base}/entries", timeout=REQUEST_TIMEOUT
    ).json()["items"]
    if len(items) < 2:
        pytest.skip("not enough entries to exercise created-before")

    timestamps = sorted(e["timestamp"].replace("Z", "+00:00") for e in items)
    bound = timestamps[-1]

    response = sovd_session.get(
        f"{logs_base}/entries",
        params={"created-before": bound},
        timeout=REQUEST_TIMEOUT,
    )
    assert response.status_code == 200

    bound_dt = datetime.fromisoformat(bound).astimezone(timezone.utc)
    for entry in response.json()["items"]:
        ts = datetime.fromisoformat(
            entry["timestamp"].replace("Z", "+00:00")
        ).astimezone(timezone.utc)
        assert ts < bound_dt


# --------------------------------------------------------------------------- #
# Configuration CRUD
# --------------------------------------------------------------------------- #


def test_get_configuration(sovd_session, logs_base):
    """GET /logs/config returns a contexts list of {context, severity}."""
    response = sovd_session.get(f"{logs_base}/config", timeout=REQUEST_TIMEOUT)

    assert response.status_code == 200
    body = response.json()
    assert "contexts" in body
    assert isinstance(body["contexts"], list)

    for cfg in body["contexts"]:
        assert "context" in cfg
        assert "type" in cfg["context"]
        assert cfg["severity"] in SEVERITY_RANK


def test_update_configuration_roundtrip(sovd_session, logs_base):
    """PUT /logs/config upserts a context threshold; GET reflects it."""
    put = sovd_session.put(
        f"{logs_base}/config",
        json={
            "items": [
                {
                    "context": {"type": "RFC5424", "host": "diag-host"},
                    "severity": "warn",
                }
            ]
        },
        timeout=REQUEST_TIMEOUT,
    )
    assert put.status_code == 204

    contexts = sovd_session.get(
        f"{logs_base}/config", timeout=REQUEST_TIMEOUT
    ).json()["contexts"]

    rfc = [c for c in contexts if c["context"]["type"] == "RFC5424"]
    assert rfc, "expected an RFC5424 context in the configuration"
    assert rfc[0]["severity"] == "warn"


def test_reset_configuration(sovd_session, logs_base):
    """DELETE /logs/config resets per-context thresholds to the Info baseline."""
    # Raise the threshold first so the reset is observable.
    sovd_session.put(
        f"{logs_base}/config",
        json={
            "items": [
                {
                    "context": {"type": "RFC5424", "host": "diag-host"},
                    "severity": "error",
                }
            ]
        },
        timeout=REQUEST_TIMEOUT,
    )

    delete = sovd_session.delete(f"{logs_base}/config", timeout=REQUEST_TIMEOUT)
    assert delete.status_code == 204

    contexts = sovd_session.get(
        f"{logs_base}/config", timeout=REQUEST_TIMEOUT
    ).json()["contexts"]

    for cfg in contexts:
        if cfg["context"]["type"] == "RFC5424":
            assert cfg["severity"] == "info"
        elif cfg["context"]["type"] == "AUTOSAR_DLT":
            assert cfg["severity"] == "DLT_INFO"
