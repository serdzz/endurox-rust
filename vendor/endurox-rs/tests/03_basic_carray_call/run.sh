#!/usr/bin/env bash

set -euo pipefail

THIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DIR="$THIS_DIR"
CONF_DIR="$THIS_DIR/conf"
BIN_DIR="$THIS_DIR/bin"
PROJECT_DIR="$(cd "$THIS_DIR/../.." && pwd)"

mkdir -p "$BIN_DIR"

if [ -f ~/ndrx_home ]; then
    . ~/ndrx_home
fi

rm -f "$CONF_DIR/app.ini" "$CONF_DIR/settest1"
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

cargo build --manifest-path "$TEST_DIR/Cargo.toml" --target-dir "$PROJECT_DIR/target" --bin rs_it_carray_server --bin rs_it_carray_client
cp "$PROJECT_DIR/target/debug/rs_it_carray_server" "$BIN_DIR/rs_it_carray_server"
chmod +x "$BIN_DIR/rs_it_carray_server"

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

OUT_FILE="$THIS_DIR/test.out"
rm -f "$OUT_FILE"

if ! "$PROJECT_DIR/target/debug/rs_it_carray_client" | tee "$OUT_FILE"; then
    dump_logs
    exit 1
fi

if ! grep -q '0 1 2 3 4 5 6 7 8 9 10' "$OUT_FILE"; then
    echo "TESTERROR: Expected response content not found in client output"
    dump_logs
    exit 1
fi

xadmin psc
xadmin stop -c -y
trap - EXIT

echo "Test OK: 03_basic_carray_call"
