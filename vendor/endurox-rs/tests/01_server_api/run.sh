#!/usr/bin/env bash

set -euo pipefail

THIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DIR="$THIS_DIR"
CONF_DIR="$THIS_DIR/conf"
BIN_DIR="$THIS_DIR/bin"
PROJECT_DIR="$(cd "$THIS_DIR/../.." && pwd)"
SCENARIO="${1:-tpcall}"
CRATE_FEATURE="${2:-}"
DISPATCH_MODE="${3:-multi}"

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

if [ "$DISPATCH_MODE" = "single" ]; then
    export NDRX_CONFIG="$CONF_DIR/ndrxconfig-single.xml"
fi

# Worker tpsvrthrdone hooks run only at shutdown, so they record themselves here
# and are checked once the domain has stopped.
THRDONE_MARKER="$THIS_DIR/log/thrdone.marker"
export NDRX_RS_THRDONE_MARKER="$THRDONE_MARKER"
rm -f "$THRDONE_MARKER"

if [ -n "$CRATE_FEATURE" ]; then
    cargo build --manifest-path "$TEST_DIR/Cargo.toml" --target-dir "$PROJECT_DIR/target" --features "$CRATE_FEATURE" --bin rs_it_server --bin rs_it_client
else
    cargo build --manifest-path "$TEST_DIR/Cargo.toml" --target-dir "$PROJECT_DIR/target" --bin rs_it_server --bin rs_it_client
fi
cp "$PROJECT_DIR/target/debug/rs_it_server" "$BIN_DIR/rs_it_server"
chmod +x "$BIN_DIR/rs_it_server"

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

if ! "$PROJECT_DIR/target/debug/rs_it_client" "$SCENARIO"; then
    dump_logs
    exit 1
fi

xadmin psc
xadmin stop -c -y
trap - EXIT

# Only the threaded scenario configures dispatch threads worth checking.
if [ "$SCENARIO" = "dispatch-threads" ] && [ "$DISPATCH_MODE" != "single" ]; then
    lines=$(wc -l < "$THRDONE_MARKER" 2>/dev/null || echo 0)
    if [ "$lines" -lt 2 ]; then
        echo "expected >=2 tpsvrthrdone hook invocations, got $lines" >&2
        cat "$THRDONE_MARKER" 2>/dev/null >&2 || true
        exit 1
    fi
    if grep -q "srvid=-1" "$THRDONE_MARKER"; then
        echo "tpsvrthrdone hook ran without a usable worker context" >&2
        cat "$THRDONE_MARKER" >&2
        exit 1
    fi
    echo "tpsvrthrdone verified: $lines worker(s)"
fi

echo "Test OK: $SCENARIO"
