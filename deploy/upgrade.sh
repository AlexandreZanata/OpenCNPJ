#!/usr/bin/env bash
# Zero-downtime upgrade of the opencnpj binary with blue/green systemd instances that share
# the listening ports through SO_REUSEPORT (see opencnpj@.service).
#
#   sudo deploy/upgrade.sh /path/to/new/opencnpj
set -euo pipefail

new_bin=${1:?usage: upgrade.sh NEW_BINARY}
health=${HEALTH_URL:-http://127.0.0.1:8080/readyz}

"$new_bin" --version >/dev/null
if systemctl is-active --quiet opencnpj@blue; then old=blue new=green; else old=green new=blue; fi

install -m 0755 "$new_bin" /usr/local/bin/opencnpj.new
mv -f /usr/local/bin/opencnpj.new /usr/local/bin/opencnpj

systemctl start "opencnpj@$new"
for _ in $(seq 1 120); do
    if systemctl is-active --quiet "opencnpj@$new" && curl -fsS -o /dev/null "$health"; then
        break
    fi
    sleep 0.5
done
systemctl is-active --quiet "opencnpj@$new" || { echo "opencnpj@$new failed to start; $old keeps serving" >&2; exit 1; }

# Both instances accept connections now; stopping the old one drains it (SIGTERM).
systemctl stop "opencnpj@$old"
systemctl disable --quiet "opencnpj@$old" || true
systemctl enable --quiet "opencnpj@$new"
echo "serving from opencnpj@$new"
