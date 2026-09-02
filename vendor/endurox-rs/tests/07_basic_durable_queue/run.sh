#!/usr/bin/env bash

set -euo pipefail

THIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONF_DIR="$THIS_DIR/conf"
BIN_DIR="$THIS_DIR/bin"
PROJECT_DIR="$(cd "$THIS_DIR/../.." && pwd)"

mkdir -p "$BIN_DIR"

if [ -f ~/ndrx_home ]; then
    . ~/ndrx_home
fi

rm -f "$CONF_DIR/app.ini" "$CONF_DIR/ndrxconfig.xml" "$CONF_DIR/settest1"
mkdir -p "$THIS_DIR/log"
find "$THIS_DIR/log" -type f -exec rm -f {} +

pushd "$THIS_DIR" >/dev/null
xadmin provision -d \
    -vaddubf=../ubftab/test.fd \
    -vtimeout=60 \
    -vmsgmax=10 \
    -vmsgsizemax=40000
popd >/dev/null

pushd "$CONF_DIR" >/dev/null
. ./settest1
popd >/dev/null

cargo build --manifest-path "$THIS_DIR/Cargo.toml" --target-dir "$PROJECT_DIR/target" --bin rs_dq_client

cleanup() {
    xadmin stop -c -y >/dev/null 2>&1 || true
}

dump_logs() {
    for f in "$NDRX_APPHOME"/log/*; do
        if [ -f "$f" ]; then
            echo "===== $f (tail) ====="
            tail -n 80 "$f" || true
        fi
    done
}

trap cleanup EXIT

xadmin stop -c -y >/dev/null 2>&1 || true

if ! xadmin start -y; then
    dump_logs
    exit 1
fi

sleep 2

export NDRX_CCTAG=RM1TMQ

for scenario in enqueue-dequeue corrid fifo; do
    if ! "$PROJECT_DIR/target/debug/rs_dq_client" "$scenario"; then
        dump_logs
        exit 1
    fi
done

# Transactional scenarios need the client to load XA settings from
# the [@global/RM1TMQ] section of app.ini so tpopen()/tpbegin() succeed.
for scenario in tx-commit tx-abort tx-suspend-resume; do
    if ! "$PROJECT_DIR/target/debug/rs_dq_client" "$scenario"; then
        dump_logs
        exit 1
    fi
done

xadmin psc
xadmin stop -c -y
trap - EXIT

echo "Test OK: basic durable queue"
