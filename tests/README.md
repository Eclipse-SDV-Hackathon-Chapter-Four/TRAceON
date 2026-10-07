<!--
SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
SPDX-License-Identifier: Apache-2.0
-->

# TRAceON Logging API — pytest suite

End-to-end tests for the SOVD logging resource served by `sovd_server_comp`.

## Layout

```text
tests/
├── conftest.py                  # fixtures, config, severity helpers
├── test_logging_discovery.py    # GET /logs resource + advertised links
├── test_logging_read.py         # GET /logs/entries + entry shape
├── test_logging_filters.py      # severity/time filters + config CRUD
├── test_logging_errors.py       # 4xx handling (unknown app, bad filters)
├── test_logging_performance.py  # entries query latency
├── test_logging_realtime.py     # SSE live stream via the ingest sink
├── pytest.ini                   # markers + pythonpath
├── requirements.txt
├── Dockerfile
└── docker-compose.yml           # SIL / OpenDUT orchestration
```

## Endpoints under test

All under `/sovd/v1`, app id `diag-app`:

| Method | Path | Purpose |
|---|---|---|
| GET | `/apps/diag-app/logs` | logging resource (entries/config/stream links) |
| GET | `/apps/diag-app/logs/entries` | filtered log entries (`items`) |
| GET | `/apps/diag-app/logs/entries/stream` | SSE live stream |
| GET/PUT/DELETE | `/apps/diag-app/logs/config` | per-context severity config |
| POST | `/internal/logs` | hardware ingest sink (feeds the live stream) |

## Run locally

Start the server (REST-sink mode so the SSE test can post events):

```bash
SOVD_STATIC_DIR=diagnostic_components/sovd_server_comp/static \
  cargo run -p sovd_server_comp
```

Then, in another shell:

```bash
pip install -r tests/requirements.txt
pytest tests/ -v --tb=short --junitxml=results.xml
```

> **Run from inside `tests/`.** Running `pytest` from the repository root makes
> it descend into the `open_source/opensovd-core` submodule, whose own
> `conftest.py` collides with this suite and causes collection errors. The
> `pytest.ini` here scopes collection to this directory, so always `cd tests`
> first (or pass the `tests/` path explicitly).

## Generating a test report

The suite can emit reports in three formats. All of them require a **running
server** (see "Run locally" above) — otherwise every test skips.

Set up the virtual environment once:

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r tests/requirements.txt   # includes pytest-html
```

Then move into the `tests/` directory:

```bash
cd tests
```

There are three test report options below (or the combined command at the end) — they are alternatives, you do not need to run all of them.

**HTML report** (self-contained, open in a browser):

```bash
pytest . -v --html=report.html --self-contained-html
xdg-open report.html        # or just open report.html manually
```

**JUnit XML** (for CI systems):

```bash
pytest . -v --junitxml=results.xml
```

**Plain-text log:**

```bash
pytest . -v | tee report.txt
```

**All three at once:**

```bash
pytest . -v --html=report.html --self-contained-html --junitxml=results.xml | tee report.txt
```

Report files are written to `tests/` (`report.html`, `results.xml`,
`report.txt`). The HTML report lists every test with its status, duration, and
captured output; failures include the assertion traceback inline.


## Markers

```bash
pytest -m smoke         # discovery / reachability only
pytest -m integration   # full API behaviour
pytest -m performance   # latency checks
pytest -m sovd_logging  # the whole logging suite
```

## Configuration (environment variables)

| Variable | Default | Description |
|---|---|---|
| `TRACEON_BASE_URL` | `http://localhost:8080` | Server base URL |
| `TRACEON_APP_ID` | `diag-app` | App id under test |
| `TRACEON_REQUEST_TIMEOUT` | `5` | Per-request timeout (s) |
| `TRACEON_LATENCY_BUDGET` | `2.0` | Max entries-query latency (s) |
| `TRACEON_STREAM_TIMEOUT` | `10` | Max wait for an SSE event (s) |

## Notes

- If the server is unreachable the whole suite **skips** with a clear message
  rather than failing (see the `_require_server` autouse fixture).
- The SSE tests require REST-sink mode (the default). In uProtocol mode there
  is no `/internal/logs` sink, so those tests skip automatically.
- Severity filtering is **threshold-based**: `?severity=warn` returns `warn`,
  `error` and `fatal` entries — everything at least as severe as `warn`.
