<!--
SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
SPDX-License-Identifier: Apache-2.0
-->

# TRAceON Documentation

Documentation for the TRAceON demos — the SOVD `logs` resource on Eclipse
OpenSOVD, with two log sources (uProtocol/Zenoh and the Eclipse ThreadX
pipeline) served through one SOVD server.

## Contents

- **[DEMO.md](DEMO.md)** — the main reference:
  - Requirements (common + ThreadX pipeline)
  - Components and architecture diagram
  - Deployment / how to run both demos
  - Interactive menu reference
  - Sequence diagrams (live SSE, uProtocol read, config)
  - State diagrams (demo lifecycle, log-entry lifecycle)
  - ISO 17978-3 conformance notes
- **[TraceON_UseCase.md](TraceON_UseCase.md)** — the original use case: actors,
  flow, problem statement, and expected benefits.
- **TRAceON.png** — the original high-level architecture diagram.

## Demo scripts

| Script | Description |
|---|---|
| [`../scripts/demo.sh`](../scripts/demo.sh) | Lightweight smoke test (Rust + curl only). |
| [`../scripts/demo-threadx.sh`](../scripts/demo-threadx.sh) | Full interactive demo with the Eclipse ThreadX pipeline. |

## Quick start

```bash
source "$HOME/.cargo/env"

# lightweight:
./scripts/demo.sh

# full ThreadX pipeline (needs Docker + Python + mosquitto_pub):
./scripts/demo-threadx.sh
```

See [DEMO.md §2](DEMO.md#2-requirements) for the full requirements and the
user-local `mosquitto_pub` install if you don't have root.

> The diagrams in DEMO.md use Mermaid, which renders on GitHub and in most
> Markdown viewers.
