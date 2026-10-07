# TRAceON

Centralized logging and trace collection for vehicle ECUs over SOVD (Service-Oriented Vehicle Diagnostics).

TRAceON demonstrates a logging and trace-retrieval use case for [OpenSOVD](https://github.com/eclipse-opensovd), letting a diagnostics engineer pull filtered logs from multiple ECUs through a single SOVD interface to speed up root-cause analysis.

![TRAceON architecture overview](docs/TRAceON.png)

## Problem Statement

At present, OpenSOVD does not provide native logging support. When an ECU reports a severe or critical fault, there is no unified way to collect and inspect the relevant logs across the vehicle.

TRAceON solves this by implementing the logging use case end to end, with the goal of contributing it back to OpenSOVD once the implementation is mature. The solution aims to provide:

- Centralized log access through SOVD.
- Log filtering and querying.
- Live synchronization of logs.
- Continuous monitoring of ECU health.
- Root-cause analysis for fault diagnostics.

## Use Case

A diagnostics engineer wants to understand the root cause of a fault reported by an ECU (for example, ECU1). The engineer uses the SOVD interface to request logs from vehicle ECUs and can apply filters to retrieve only the relevant information.

This supports troubleshooting scenarios where an ECU reports severe or critical errors and detailed log analysis is required.

### Actors

- End User
- Tester / Client
- SOVD Server (running on the HPC)
- Vehicle ECUs (ECU1, ECU2, ECU3)

### Log Filtering Options

The client can request logs using filters such as:

- Timestamp
- Context
- Severity

### Flow

1. An end user initiates a diagnostic investigation.
2. A tester/client sends a `GET(log_entries)` request to the SOVD Server.
3. The request may include filtering parameters (timestamp, context, severity).
4. The SOVD Server communicates with the relevant ECUs through the vehicle network using the vehicle protocol and REST APIs.
5. Each ECU exposes its application logs.
6. The SOVD Server aggregates the requested logs.
7. The filtered log data is returned to the tester/client.
8. The engineer analyzes the logs to determine the root cause of the reported fault.

## System Architecture

```text
+------------+       GET(log_entries)        +----------------+
| Tester /   | ---------------------------> |  SOVD Server   |
| Client     |                              |    (HPC)       |
+------------+ <--------------------------- +----------------+
                    Filtered Logs

                          |
                          | Vehicle Protocol / REST API
                          |
        -------------------------------------------------
        |                       |                       |
        v                       v                       v

    +---------+            +---------+            +---------+
    |  ECU1   |            |  ECU2   |            |  ECU3   |
    +---------+            +---------+            +---------+
    | App1    |            | App1    |            | App1    |
    | App2    |            | App2    |            | App2    |
    | AppN    |            | AppN    |            | AppN    |
    | Logs    |            | Logs    |            | Logs    |
    +---------+            +---------+            +---------+
```

See [docs/TraceON_UseCase.md](docs/TraceON_UseCase.md) for the full use-case description.

## Expected Benefits

- Faster troubleshooting of vehicle faults.
- Unified access to logs across multiple ECUs.
- Reduced diagnostic effort.
- Improved observability of distributed vehicle software.
- Support for real-time monitoring and alerting.
- Foundation for future trace and telemetry capabilities within OpenSOVD.

## Repository Structure

```text
TRAceON/
├── diagnostic_components/
│   └── sovd_server_comp/      # SOVD server component (Rust)
├── docs/                      # Documentation and diagrams
│   ├── TRAceON.png
│   └── TraceON_UseCase.md
└── open_source/
    └── opensovd-core/         # OpenSOVD core (submodule)
```

## Getting Started

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable)
- Git (with submodule support)

### Clone

```bash
git clone --recurse-submodules <repository-url>
cd TRAceON
```

If you already cloned without submodules:

```bash
git submodule update --init --recursive
```

### Build

```bash
cargo build
```

## Contributing

This project is intended to be contributed back to the OpenSOVD project. Contributions are welcome. Please open an issue or pull request to discuss changes.

## License

See the repository for license details.

## Team member

### Helge Gudmundsen - Developer
### Isabella Lanes Rocha - Developer
### Kavyasree Sankaranarayanan Nair - Developer
### Marufa Binte Mostafa - Developer
### Matheus Abrahao - Developer
### Priyankkumar Bidya - Developer
### Himank Meattle - Team Support
