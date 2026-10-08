#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
# SPDX-License-Identifier: Apache-2.0
#
# This file was created with the assistance of generative AI.

set -Eeuo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

autosd_host=${AUTOSD_HOST:-}
autosd_user=${AUTOSD_USER:-root}
autosd_ssh_port=${AUTOSD_SSH_PORT:-22}
autosd_http_host=${AUTOSD_HTTP_HOST:-$autosd_host}
sovd_port=${AUTOSD_SOVD_PORT:-8080}
severity=${SOVD_SEVERITY:-DLT_INFO}
autosd_base_image=${AUTOSD_BASE_IMAGE:-ghcr.io/eclipse-autosd/eclipse-autosd-bootc-qemu:latest}
if [[ -n "${AUTOSD_REMOTE_SUDO+x}" ]]; then
    remote_sudo=$AUTOSD_REMOTE_SUDO
elif [[ "$autosd_user" == "root" ]]; then
    remote_sudo=
else
    remote_sudo="sudo "
fi
remote_podman=${AUTOSD_REMOTE_PODMAN:-"${remote_sudo}podman"}

if [[ -z "$autosd_host" ]]; then
    echo "AUTOSD_HOST is required, for example: AUTOSD_HOST=192.168.88.100" >&2
    exit 2
fi

if command -v podman >/dev/null 2>&1; then
    oci_bin=podman
elif command -v docker >/dev/null 2>&1; then
    oci_bin=docker
else
    echo "Neither podman nor docker is installed on the build host." >&2
    exit 2
fi

ssh_target="$autosd_user@$autosd_host"
ssh_args=(-p "$autosd_ssh_port")
server_image=localhost/traceon:dev
dummy_image=localhost/traceon-dummy-diag-app:dev
quadlet_dir=/etc/containers/systemd
env_file=$quadlet_dir/traceon.env
quadlet_generator=/usr/lib/systemd/system-generators/podman-system-generator

run_remote() {
    ssh "${ssh_args[@]}" "$ssh_target" "$@"
}

echo "Building the TraceOn SOVD image with $oci_bin..."
cd "$repo_root"
"$oci_bin" build \
    --build-arg "AUTOSD_BASE_IMAGE=$autosd_base_image" \
    -f deploy/autosd/Containerfile \
    -t "$server_image" .

echo "Building the dummy ECU image with $oci_bin..."
"$oci_bin" build \
    --build-arg "AUTOSD_BASE_IMAGE=$autosd_base_image" \
    -f deploy/autosd/DummyDiagContainerfile \
    -t "$dummy_image" .

echo "Checking the AutoSD target..."
run_remote "test -x \"\$(command -v podman)\" && test -x \"\$(command -v systemctl)\""

echo "Loading images on $ssh_target..."
"$oci_bin" save "$server_image" | ssh "${ssh_args[@]}" "$ssh_target" "$remote_podman load"
"$oci_bin" save "$dummy_image" | ssh "${ssh_args[@]}" "$ssh_target" "$remote_podman load"

echo "Installing AutoSD Quadlets..."
ssh "${ssh_args[@]}" "$ssh_target" "cat > /tmp/dummy-diag-app.container" \
    < deploy/autosd/dummy-diag-app.container
ssh "${ssh_args[@]}" "$ssh_target" "cat > /tmp/traceon-uprotocol.container" \
    < deploy/autosd/traceon-uprotocol.container

run_remote "${remote_sudo}mkdir -p '$quadlet_dir'"
run_remote "${remote_sudo}install -m 0644 /tmp/dummy-diag-app.container '$quadlet_dir/dummy-diag-app.container'"
run_remote "${remote_sudo}install -m 0644 /tmp/traceon-uprotocol.container '$quadlet_dir/traceon-uprotocol.container'"

printf 'SOVD_BASE_URI=http://%s:%s/sovd\n' "$autosd_http_host" "$sovd_port" |
    ssh "${ssh_args[@]}" "$ssh_target" "${remote_sudo}tee '$env_file' >/dev/null"

echo "Starting AutoSD services..."
run_remote "${remote_sudo}systemctl daemon-reload"
# AutoSD's systemd image does not invoke the Quadlet generator on reload.
run_remote "${remote_sudo}rm -f /etc/systemd/system/dummy-diag-app.service /etc/systemd/system/traceon-uprotocol.service"
run_remote "${remote_sudo}$quadlet_generator /etc/systemd/system /etc/systemd/system /etc/systemd/system"
run_remote "${remote_sudo}systemctl daemon-reload"
run_remote "${remote_sudo}systemctl start dummy-diag-app.service"
run_remote "${remote_sudo}systemctl restart traceon-uprotocol.service"

entries_url="http://$autosd_http_host:$sovd_port/sovd/v1/apps/diag-app/logs/entries"
filtered_url="$entries_url?severity=$severity"

echo "Waiting for the AutoSD SOVD endpoint..."
for ((attempt = 1; attempt <= 60; attempt++)); do
    if curl --silent --fail "http://$autosd_http_host:$sovd_port/sovd/v1/apps/diag-app/logs" >/dev/null 2>&1; then
        break
    fi

    if ((attempt == 60)); then
        echo "AutoSD SOVD endpoint did not become ready." >&2
        run_remote "${remote_sudo}systemctl --no-pager --full status dummy-diag-app.service traceon-uprotocol.service || true"
        run_remote "${remote_sudo}journalctl --no-pager -u dummy-diag-app.service -u traceon-uprotocol.service -n 80 || true"
        exit 1
    fi

    sleep 1
done

echo "Testing filtered log retrieval through AutoSD..."
entries=$(curl --silent --show-error --fail "$filtered_url") || {
    echo "SOVD log retrieval failed." >&2
    run_remote "${remote_sudo}journalctl --no-pager -u dummy-diag-app.service -u traceon-uprotocol.service -n 80 || true"
    exit 1
}

if [[ "$entries" != *'"items"'* ]]; then
    echo "Unexpected SOVD response:" >&2
    echo "$entries" >&2
    exit 1
fi

echo "AutoSD uProtocol end-to-end test passed. Response:"
echo "$entries"
echo
echo "Unfiltered request:"
echo "  curl '$entries_url'"
echo "Filtered request:"
echo "  curl '$filtered_url'"
