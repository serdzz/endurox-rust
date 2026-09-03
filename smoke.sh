#!/bin/bash
# One-command smoke test for the endurox-rs migration.
#
#   ./smoke.sh            build everything, start a scratch domain, run all
#                         checks, print PASS/FAIL, leave the domain running
#   ./smoke.sh stop       stop the domain and clean up the scratch APPHOME
#
# Requirements: Enduro/X 8.0.10 installed system-wide (the gnu_epoll deb in
# this repo), Rust, pkg-config, libclang-dev, libpq-dev. PostgreSQL is
# optional: with a reachable DATABASE_URL the oracle_txn_server checks run,
# otherwise they are skipped.
#
# Everything lives in a scratch APPHOME (default /tmp/erx-smoke) so the repo
# stays clean and repeated runs start fresh.

set -u
REPO="$(cd "$(dirname "$0")" && pwd)"
APP="${SMOKE_APPHOME:-/tmp/erx-smoke}"
DATABASE_URL="${DATABASE_URL:-postgres://txnuser:txnpass@127.0.0.1/txndb}"
GW_PORT="${GW_PORT:-8080}"

env_setup() {
    export NDRX_APPHOME="$APP"
    export NDRX_HOME=/usr
    export NDRX_CCONFIG="$APP/conf/ndrxconf.ini"
    export FLDTBLDIR="/usr/share/endurox/ubftab:$APP/ubftab"
    export FIELDTBLS="Exfields.fd,test.fd"
    export LD_LIBRARY_PATH="/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    export DATABASE_URL
    export PATH="$APP/bin:/usr/bin:$PATH"
}

if [ "${1:-}" = "stop" ]; then
    env_setup
    timeout 30 xadmin stop -y >/dev/null 2>&1
    pkill -x ndrxd 2>/dev/null
    pkill -f "rest_gateway" 2>/dev/null
    rm -rf "$APP"
    echo "domain stopped, $APP removed"
    exit 0
fi

PASS=0; FAIL=0; SKIP=0
ok()   { PASS=$((PASS+1)); echo "  PASS  $1"; }
bad()  { FAIL=$((FAIL+1)); echo "  FAIL  $1${2:+ -- $2}"; }
skip() { SKIP=$((SKIP+1)); echo "  skip  $1${2:+ -- $2}"; }

echo "== 1/5 build =="
cd "$REPO"
if cargo build --release -p samplesvr_rust -p ubfsvr_rust -p ubf_test_client \
        -p rest_gateway -p oracle_txn_server; then
    ok "cargo build --release (5 crates)"
else
    bad "cargo build" "fix the build before smoking anything else"
    exit 1
fi

echo "== 2/5 scratch APPHOME: $APP =="
rm -rf "$APP"
mkdir -p "$APP"/{bin,conf,log,tmp,ubftab}
cp "$REPO"/target/release/{samplesvr_rust,ubfsvr_rust,ubf_test_client,rest_gateway,oracle_txn_server} "$APP/bin/"
cp "$REPO"/ubftab/test.fd "$REPO"/ubftab/test.fd.h "$APP/ubftab/"
cp "$REPO"/conf/ndrxconfig.xml "$APP/conf/"
cp "$REPO"/conf/app.ini "$APP/conf/" 2>/dev/null || true
# Repo config targets the docker image (/opt/endurox, Oracle XA, big queues).
# Rewrite for a local system install with modest RLIMIT_MSGQUEUE, no XA.
sed -e 's|NDRX_HOME=/opt/endurox|NDRX_HOME=/usr|' \
    -e 's|NDRX_QPREFIX=/run|NDRX_QPREFIX=/erxsmoke|' \
    -e 's|NDRX_DQMAX=.*|NDRX_DQMAX=5|' \
    -e 's|NDRX_MSGMAX=.*|NDRX_MSGMAX=2|' \
    -e 's|NDRX_MSGSIZEMAX=.*|NDRX_MSGSIZEMAX=4096|' \
    -e '/NDRX_XA_/d' \
    "$REPO/conf/ndrxconf.ini" > "$APP/conf/ndrxconf.ini"
# One copy of each server is plenty for a smoke run.
sed -i -e 's|<min>5</min>|<min>1</min>|' -e 's|<max>10</max>|<max>1</max>|' \
    "$APP/conf/ndrxconfig.xml"
env_setup
ok "scratch APPHOME prepared"

echo "== 3/5 domain =="
xadmin stop -y >/dev/null 2>&1 &
sleep 5; kill %1 2>/dev/null   # in case a previous run is up; stop may hang
# xadmin start can hang waiting for progress replies on hosts with a small
# RLIMIT_MSGQUEUE. Re-issue the start until everything advertises: a repeated
# `xadmin start -y` only boots what is not up yet.
for attempt in 1 2 3 4; do
    timeout 60 xadmin start -y >> "$APP/log/start.out" 2>&1
    for _ in $(seq 1 10); do
        xadmin psc 2>/dev/null | grep -q "UBFECHO.*AVAIL" && break 2
        sleep 2
    done
done
if xadmin psc 2>/dev/null | grep -q "UBFECHO.*AVAIL"; then
    ok "ndrxd up, ubfsvr_rust advertised"
else
    bad "xadmin start" "see $APP/log/start.out and $APP/log/"
fi
if xadmin psc 2>/dev/null | grep -q "CREATE_TXN.*AVAIL"; then
    TXN_UP=1
    ok "oracle_txn_server advertised (CREATE_TXN)"
else
    skip "oracle_txn_server" "no database? check DATABASE_URL and $APP/log/"
fi

echo "== 4/5 clients =="
if "$APP/bin/ubf_test_client" > "$APP/log/ubf_test_client.out" 2>&1; then
    ok "ubf_test_client (UBFADD/UBFTEST/UBFECHO/UBFGET)"
else
    bad "ubf_test_client" "see $APP/log/ubf_test_client.out"
fi
if cargo run --release --example nested_structs_example -p ubf_test_client \
        > "$APP/log/nested.out" 2>&1 && grep -q "round-trip OK" "$APP/log/nested.out"; then
    ok "nested_structs_example (embedded UBF round-trip)"
else
    bad "nested_structs_example" "see $APP/log/nested.out"
fi

echo "== 5/5 REST gateway =="
pkill -x rest_gateway 2>/dev/null   # any instance, incl. stale ones on :8080
sleep 1
# Few workers: each worker owns an AtmiCtx with its own reply queue, and small
# RLIMIT_MSGQUEUE hosts cannot afford 16 of them.
REST_WORKERS="${REST_WORKERS:-2}" "$APP/bin/rest_gateway" > "$APP/log/gw.log" 2>&1 &
for _ in $(seq 1 15); do
    curl -sf "http://127.0.0.1:$GW_PORT/" >/dev/null 2>&1 && break
    sleep 2
done
if curl -sf "http://127.0.0.1:$GW_PORT/api/status" | grep -q "OK"; then
    ok "GET /api/status"
else
    bad "GET /api/status" "see $APP/log/gw.log"
fi
if curl -sf -X POST "http://127.0.0.1:$GW_PORT/api/hello" \
        -H 'Content-Type: application/json' \
        -d '{"name":"Smoke"}' | grep -q "Hello"; then
    ok "POST /api/hello"
else
    bad "POST /api/hello"
fi
if [ "${TXN_UP:-0}" = "1" ]; then
    R=$(curl -sf -X POST "http://127.0.0.1:$GW_PORT/api/oracle/create" \
        -H 'Content-Type: application/json' \
        -d "{\"transaction_type\":\"sale\",\"transaction_id\":\"SMOKE-$$-$(date +%s)\",\"account\":\"SMOKE-1\",\"amount\":100,\"currency\":\"EUR\"}")
    if echo "$R" | grep -q SUCCESS; then
        ok "POST /api/oracle/create -> XA transaction -> DB"
    else
        bad "POST /api/oracle/create" "$R"
    fi
else
    skip "XA transaction check" "oracle_txn_server not up"
fi

echo
echo "======================================"
echo " smoke: $PASS passed, $FAIL failed, $SKIP skipped"
echo " domain still running; ./smoke.sh stop to tear down"
echo " logs: $APP/log/"
echo "======================================"
[ "$FAIL" -eq 0 ]
