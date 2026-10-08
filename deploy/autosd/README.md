<!--
SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
SPDX-License-Identifier: Apache-2.0

This file was created with the assistance of generative AI.
-->

# TraceOn on AutoSD

This guide runs the TraceOn SOVD server and the dummy diagnostic application
inside an AutoSD QEMU image and verifies log retrieval through the uProtocol
path.

The resulting communication path is:

```text
SOVD client
  -> TraceOn SOVD server in AutoSD
  -> Zenoh/uProtocol over localhost:7447
  -> dummy diagnostic application in AutoSD
  -> log response
```

## Prerequisites

The following must be available:

- WSL 2 with KVM access (`ls -l /dev/kvm`);
- Docker Desktop with WSL integration enabled for `Ubuntu-22.04`;
- Jumpstarter installed and activated with `source ~/.local/jumpstarter/set`;
- the AutoSD QEMU image downloaded locally;
- the TraceOn repository initialized with its submodules.

Initialize the submodules from the TraceOn repository root:

```bash
git submodule update --init --recursive
```

The expected AutoSD image is:

```text
.cache/autosd/eclipse-autosd-bootc-qemu-x86_64.qcow2
```

The development image uses the credentials `root` / `password`.

## Terminal 1: start Jumpstarter

Run this from the TraceOn repository root and keep the terminal open:

```bash
cd ~/git/opensovd-repos/TRAceON
source ~/.local/jumpstarter/set

jmp run \
  --exporter-config deploy/autosd/qemu.yml \
  --tls-grpc-listener 127.0.0.1:19090 \
  --tls-grpc-insecure
```

The QEMU configuration forwards:

```text
127.0.0.1:2222 -> AutoSD SSH port 22
127.0.0.1:8080 -> AutoSD port 8080
```

## Terminal 2: flash and boot AutoSD

Open a second terminal and keep the Jumpstarter shell open:

```bash
cd ~/git/opensovd-repos/TRAceON
source ~/.local/jumpstarter/set

jmp shell \
  --tls-grpc 127.0.0.1:19090 \
  --tls-grpc-insecure
```

Inside the Jumpstarter shell, flash the image while the VM is stopped:

```bash
j qemu power off
j qemu flasher flash /home/mmo4t6/git/opensovd-repos/TRAceON/.cache/autosd/eclipse-autosd-bootc-qemu-x86_64.qcow2
j qemu power on
j qemu console pipe
```

Wait for the console to show:

```text
esdv-main-compute login:
```

Leave the exporter and VM running. The console viewer can remain open while
the other commands are executed.

## Terminal 3: verify SSH access

Because the VM may have a new host key after flashing, remove the old key:

```bash
ssh-keygen -f "$HOME/.ssh/known_hosts" -R "[127.0.0.1]:2222"
```

Connect to AutoSD:

```bash
ssh -p 2222 root@127.0.0.1
```

Use the password `password`, then verify the target:

```bash
hostname
systemctl is-system-running
podman --version
```

## Deploy TraceOn and the dummy app

From Terminal 3, or another WSL terminal, run:

```bash
cd ~/git/opensovd-repos/TRAceON

AUTOSD_HOST=127.0.0.1 \
AUTOSD_SSH_PORT=2222 \
AUTOSD_HTTP_HOST=127.0.0.1 \
AUTOSD_SOVD_PORT=8080 \
./scripts/test_autosd_uprotocol.sh
```

The script:

1. builds the TraceOn and dummy diagnostic OCI images;
2. loads both images into rootful Podman on AutoSD;
3. installs the Quadlet files;
4. starts the dummy app and TraceOn services;
5. requests filtered logs through the SOVD endpoint.

The script supports Docker on the WSL host and Podman on the AutoSD target.
The AutoSD target runs as `root`, so the script does not require `sudo` or
`scp` inside the target image.

## Short demo after deployment

Once the deployment script reports success, do not rerun the build script.
Keep the Jumpstarter exporter, VM, and services running. Use a new terminal for
the demonstration.

First show that the applications are running inside AutoSD:

```bash
ssh -p 2222 root@127.0.0.1 \
  'hostname; podman ps --format "table {{.Names}}\t{{.Image}}\t{{.Status}}"; systemctl is-active dummy-diag-app.service traceon-uprotocol.service'
```

Then issue the SOVD requests from the host:

```bash
curl 'http://127.0.0.1:8080/sovd/v1/apps/diag-app/logs/entries'

curl 'http://127.0.0.1:8080/sovd/v1/apps/diag-app/logs/entries?severity=DLT_INFO'

curl -N --no-buffer \
  'http://127.0.0.1:8080/sovd/v1/apps/diag-app/logs/entries/stream?severity=DLT_INFO'
```

The host port is forwarded by QEMU to the TraceOn server listening inside the
AutoSD VM. The filtered response includes entries at or above `DLT_INFO`.

## Troubleshooting

If `docker` is unavailable in WSL, enable Docker Desktop integration for the
exact distro shown by `wsl -l -v` under **Settings -> Resources -> WSL
Integration**, then run `wsl --shutdown` and restart WSL.

If the SSH host-key warning appears after reflashing:

```bash
ssh-keygen -f "$HOME/.ssh/known_hosts" -R "[127.0.0.1]:2222"
```

If the VM falls back to PXE, flash while it is powered off, then power it on:

```bash
j qemu power off
j qemu flasher flash /home/mmo4t6/git/opensovd-repos/TRAceON/.cache/autosd/eclipse-autosd-bootc-qemu-x86_64.qcow2
j qemu power on
```

If a Quadlet service is missing, check that the files exist on AutoSD:

```bash
ssh -p 2222 root@127.0.0.1 'ls -la /etc/containers/systemd'
```

The deployment script explicitly invokes the Podman Quadlet generator because
the AutoSD development image does not invoke it automatically during
`systemctl daemon-reload`.
