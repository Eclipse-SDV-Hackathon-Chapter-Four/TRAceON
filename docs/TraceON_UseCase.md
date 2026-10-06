# OpenSOVD Logging and Trace Collection Use Case

## Use Case

### Overview
A diagnostics engineer wants to understand the root cause of a fault reported by an ECU. The engineer uses the SOVD (Service-Oriented Vehicle Diagnostics) interface to request logs from vehicle ECUs. The user may apply filters to retrieve only relevant log information.

This use case supports troubleshooting scenarios where an ECU reports severe or critical errors and detailed log analysis is required.

### Actors
- End User
- Tester / Client
- SOVD Server (running on HPC)
- Vehicle ECUs (ECU1, ECU2, ECU3)

### Log Filtering Options
The client can request logs using filters such as:

- Timestamp
- Context
- Severity

### Flow

1. An end user initiates a diagnostic investigation.
2. A tester/client sends a `GET(log_entries)` request to the SOVD Server.
3. The request may include filtering parameters:
   - Timestamp
   - Context
   - Severity
4. The SOVD Server communicates with relevant ECUs through the vehicle network using the vehicle protocol and REST APIs.
5. Each ECU exposes application logs.
6. The SOVD Server aggregates the requested logs.
7. The filtered log data is returned to the tester/client.
8. The engineer analyzes the logs to determine the root cause of the reported fault.

---

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

---

## Problem Statement

At present, OpenSOVD does not provide native logging support.

This proposal demonstrates a logging and trace retrieval use case that can be contributed back to the OpenSOVD project once a mature implementation is available. The solution should also support:

- Centralized log access through SOVD.
- Log filtering and querying.
- Live synchronization of logs.
- Continuous monitoring of ECU health.
- Root-cause analysis for fault diagnostics.

---

## Expected Benefits

- Faster troubleshooting of vehicle faults.
- Unified access to logs across multiple ECUs.
- Reduced diagnostic effort.
- Improved observability of distributed vehicle software.
- Support for real-time monitoring and alerting.
- Foundation for future trace and telemetry capabilities within OpenSOVD.
