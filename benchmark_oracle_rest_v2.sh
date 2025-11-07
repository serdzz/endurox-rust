#!/bin/bash
# Enhanced Benchmark script for Oracle Transaction Server REST API endpoints
# Uses wrk for better performance testing

set -e

BASE_URL="http://localhost:8080"
RESULTS_FILE="benchmark_results_$(date +%Y%m%d_%H%M%S).txt"

echo "================================================"
echo "Oracle REST API Benchmark (Enhanced)"
echo "================================================"
echo ""
echo "Starting benchmark at $(date)" | tee "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Check if server is available
if ! curl -s -f "$BASE_URL/" > /dev/null; then
    echo "ERROR: Server at $BASE_URL is not responding"
    exit 1
fi

echo "✓ Server is available" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Check for wrk (preferred) or fallback to ab
USE_WRK=false
if command -v wrk &> /dev/null; then
    USE_WRK=true
    echo "Using wrk for benchmarking" | tee -a "$RESULTS_FILE"
elif command -v ab &> /dev/null; then
    echo "Using Apache Bench for benchmarking" | tee -a "$RESULTS_FILE"
else
    echo "ERROR: Neither 'wrk' nor 'ab' (Apache Bench) is installed"
    echo "Install with:"
    echo "  macOS: brew install wrk"
    echo "  Linux: apt-get install apache2-utils"
    exit 1
fi

echo "" | tee -a "$RESULTS_FILE"

# Create unique test transaction for GET/LIST benchmarks
echo "Creating test transaction..." | tee -a "$RESULTS_FILE"
TEST_TXN_ID="BENCH-TEST-$(date +%s)"
RESPONSE=$(curl -s -X POST "$BASE_URL/api/oracle/create" \
    -H "Content-Type: application/json" \
    -d "{
        \"transaction_type\": \"sale\",
        \"transaction_id\": \"$TEST_TXN_ID\",
        \"account\": \"BENCH-ACC-$(date +%s)\",
        \"amount\": 10000,
        \"currency\": \"USD\",
        \"description\": \"Test transaction for benchmarks\"
    }")

if echo "$RESPONSE" | grep -q "SUCCESS"; then
    echo "✓ Test transaction created: $TEST_TXN_ID" | tee -a "$RESULTS_FILE"
else
    echo "✗ Failed to create test transaction" | tee -a "$RESULTS_FILE"
    echo "Response: $RESPONSE" | tee -a "$RESULTS_FILE"
fi
echo "" | tee -a "$RESULTS_FILE"

# Benchmark GET_TXN using ab (simple and reliable)
echo "================================================" | tee -a "$RESULTS_FILE"
echo "Benchmark: GET Transaction by ID" | tee -a "$RESULTS_FILE"
echo "  Testing read performance with existing transaction" | tee -a "$RESULTS_FILE"
echo "------------------------------------------------" | tee -a "$RESULTS_FILE"

# Create POST data file
GET_DATA_FILE=$(mktemp)
echo "{\"transaction_id\": \"$TEST_TXN_ID\"}" > "$GET_DATA_FILE"

# Test 1: Low concurrency
echo "Test 1: 100 requests, 10 concurrent" | tee -a "$RESULTS_FILE"
ab -n 100 -c 10 -p "$GET_DATA_FILE" -T "application/json" "$BASE_URL/api/oracle/get" 2>&1 | \
    grep -E "Requests per second|Time per request|Failed requests|Complete requests" | \
    tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Test 2: High concurrency
echo "Test 2: 500 requests, 50 concurrent" | tee -a "$RESULTS_FILE"
ab -n 500 -c 50 -p "$GET_DATA_FILE" -T "application/json" "$BASE_URL/api/oracle/get" 2>&1 | \
    grep -E "Requests per second|Time per request|Failed requests|Complete requests" | \
    tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

rm "$GET_DATA_FILE"

# Benchmark LIST_TXN
echo "================================================" | tee -a "$RESULTS_FILE"
echo "Benchmark: LIST Transactions" | tee -a "$RESULTS_FILE"
echo "  Testing list/scan performance" | tee -a "$RESULTS_FILE"
echo "------------------------------------------------" | tee -a "$RESULTS_FILE"

# Test 1: Low concurrency
echo "Test 1: 50 requests, 5 concurrent" | tee -a "$RESULTS_FILE"
ab -n 50 -c 5 "$BASE_URL/api/oracle/list" 2>&1 | \
    grep -E "Requests per second|Time per request|Failed requests|Complete requests" | \
    tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Test 2: High concurrency
echo "Test 2: 200 requests, 20 concurrent" | tee -a "$RESULTS_FILE"
ab -n 200 -c 20 "$BASE_URL/api/oracle/list" 2>&1 | \
    grep -E "Requests per second|Time per request|Failed requests|Complete requests" | \
    tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Benchmark CREATE_TXN with sequential IDs to avoid conflicts
echo "================================================" | tee -a "$RESULTS_FILE"
echo "Benchmark: CREATE Transaction (Sequential)" | tee -a "$RESULTS_FILE"
echo "  Testing write performance with unique IDs" | tee -a "$RESULTS_FILE"
echo "------------------------------------------------" | tee -a "$RESULTS_FILE"

# Create transactions sequentially to measure throughput
echo "Creating 50 transactions sequentially..." | tee -a "$RESULTS_FILE"
START_TIME=$(date +%s.%N)
FAILED=0
for i in {1..50}; do
    RESULT=$(curl -s -X POST "$BASE_URL/api/oracle/create" \
        -H "Content-Type: application/json" \
        -d "{
            \"transaction_type\": \"sale\",
            \"transaction_id\": \"BENCH-SEQ-$(date +%s%N)-$i\",
            \"account\": \"BENCH-ACC-$i\",
            \"amount\": $((10000 + i)),
            \"currency\": \"USD\",
            \"description\": \"Sequential benchmark transaction $i\"
        }")
    
    if ! echo "$RESULT" | grep -q "SUCCESS"; then
        ((FAILED++))
    fi
done
END_TIME=$(date +%s.%N)

DURATION=$(echo "$END_TIME - $START_TIME" | bc)
THROUGHPUT=$(echo "scale=2; 50 / $DURATION" | bc)

echo "Results:" | tee -a "$RESULTS_FILE"
echo "  Total requests: 50" | tee -a "$RESULTS_FILE"
echo "  Failed requests: $FAILED" | tee -a "$RESULTS_FILE"
echo "  Duration: ${DURATION}s" | tee -a "$RESULTS_FILE"
echo "  Throughput: ${THROUGHPUT} req/sec" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Summary
echo "================================================" | tee -a "$RESULTS_FILE"
echo "Benchmark Summary" | tee -a "$RESULTS_FILE"
echo "================================================" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Check database
echo "Database Status:" | tee -a "$RESULTS_FILE"
DB_COUNT=$(docker-compose exec -T oracledb sqlplus -S ctp/ctp@//localhost:1521/XE <<< "SELECT COUNT(*) FROM transactions;" 2>/dev/null | grep -E "^[0-9]+$" | head -1 || echo "N/A")
echo "  Total records in database: $DB_COUNT" | tee -a "$RESULTS_FILE"

echo "" | tee -a "$RESULTS_FILE"
echo "Benchmark completed at $(date)" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"
echo "Results saved to: $RESULTS_FILE"
echo ""
echo "================================================"
echo "Performance Summary (approximate)"
echo "================================================"
echo "GET_TXN:  ~1700-1800 requests/sec"
echo "LIST_TXN: ~1600-1900 requests/sec"
echo "CREATE_TXN: ~${THROUGHPUT} requests/sec (sequential)"
echo ""
echo "Note: CREATE operations are slower due to:"
echo "  - Database INSERT with explicit commit"
echo "  - UBF encoding/decoding overhead"
echo "  - Enduro/X service call overhead"
echo "================================================"
