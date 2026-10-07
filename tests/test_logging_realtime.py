# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
"""Real-time log streaming tests (Server-Sent Events).

The live stream (``GET /logs/entries/stream``) only emits entries that are
*broadcast* after a subscriber connects. In REST-sink mode those come from the
hardware ingest endpoint (``POST /internal/logs``). The test therefore:

  1. opens the SSE stream in a background thread,
  2. posts a fresh log event to the sink,
  3. asserts the event is delivered over the stream.

Each SSE ``data:`` payload is an ``EventEnvelope``:
``{"timestamp": ..., "payload": { ...LogEntry... }}``.

In uProtocol mode there is no REST sink, so these tests skip gracefully.
"""

import json
import queue
import threading
import time
import uuid
from datetime import datetime, timezone

import pytest
import requests

from conftest import REQUEST_TIMEOUT

sseclient = pytest.importorskip(
    "sseclient", reason="sseclient-py is required for SSE streaming tests"
)

pytestmark = [pytest.mark.integration, pytest.mark.sovd_logging]

# How long to wait for the posted event to arrive over the stream.
STREAM_TIMEOUT = float(__import__("os").environ.get("TRACEON_STREAM_TIMEOUT", "10"))


def _sink_available(session, sink_url):
    """Probe whether the REST ingest sink accepts events (REST-sink mode)."""
    probe = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "severity": "info",
        "msg": f"traceon-test-probe-{uuid.uuid4()}",
        "context": {"type": "RFC5424", "host": "diag-host", "process": "pytest"},
    }
    try:
        response = session.post(sink_url, json=probe, timeout=REQUEST_TIMEOUT)
    except requests.exceptions.RequestException:
        return False
    # 202 Accepted in REST-sink mode; 404 when the sink is not mounted.
    return response.status_code == 202


def test_live_stream_delivers_ingested_event(sovd_session, logs_base, sink_url):
    """An event POSTed to the sink is pushed to an active SSE subscriber."""
    if not _sink_available(sovd_session, sink_url):
        pytest.skip("hardware ingest sink not available (likely uProtocol mode)")

    received = queue.Queue()
    marker = f"traceon-sse-{uuid.uuid4()}"
    stop = threading.Event()

    def _listen():
        try:
            response = requests.get(
                f"{logs_base}/entries/stream",
                stream=True,
                headers={"Accept": "text/event-stream"},
                timeout=STREAM_TIMEOUT + 5,
            )
            client = sseclient.SSEClient(response)
            for event in client.events():
                if stop.is_set():
                    break
                if not event.data:
                    continue
                try:
                    envelope = json.loads(event.data)
                except ValueError:
                    continue
                payload = envelope.get("payload") or {}
                if payload.get("msg") == marker:
                    received.put(envelope)
                    break
        except requests.exceptions.RequestException as exc:  # pragma: no cover
            received.put(exc)

    listener = threading.Thread(target=_listen, daemon=True)
    listener.start()

    # Give the subscriber a moment to connect before broadcasting.
    time.sleep(1.0)

    post = requests.post(
        sink_url,
        json={
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "severity": "error",
            "msg": marker,
            "context": {"type": "RFC5424", "host": "diag-host", "process": "pytest"},
        },
        timeout=REQUEST_TIMEOUT,
    )
    assert post.status_code == 202

    try:
        envelope = received.get(timeout=STREAM_TIMEOUT)
    except queue.Empty:
        pytest.fail("did not receive the ingested event over the SSE stream")
    finally:
        stop.set()

    assert not isinstance(envelope, Exception), f"stream listener failed: {envelope}"
    assert "timestamp" in envelope
    assert envelope["payload"]["msg"] == marker
    assert envelope["payload"]["severity"] == "error"


def test_live_stream_endpoint_sends_event_stream(sovd_session, logs_base):
    """The stream endpoint responds as text/event-stream."""
    try:
        response = requests.get(
            f"{logs_base}/entries/stream",
            stream=True,
            headers={"Accept": "text/event-stream"},
            timeout=REQUEST_TIMEOUT,
        )
    except requests.exceptions.RequestException as exc:
        pytest.skip(f"stream endpoint not reachable: {exc}")

    try:
        assert response.status_code == 200
        assert "text/event-stream" in response.headers.get("content-type", "")
    finally:
        response.close()
