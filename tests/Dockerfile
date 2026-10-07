# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
FROM python:3.13-slim

WORKDIR /tests
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt
COPY . .

# Point the suite at the opensovd service name inside the compose network.
ENV TRACEON_BASE_URL=http://opensovd:8080

CMD ["pytest", ".", "-v", "--tb=short", "--junitxml=results.xml"]
