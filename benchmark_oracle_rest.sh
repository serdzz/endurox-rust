#!/bin/bash
# Benchmark script for Oracle Transaction Server REST API endpoints

set -e

BASE_URL="http://localhost:8080"
RESULTS_FILE="benchmark_results.txt"

echo "================================================"
echo "Oracle REST API Benchmark"
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

# Function to run benchmark
run_benchmark() {
    local name=$1
    local method=$2
    local endpoint=$3
    local data=$4
    local requests=${5:-100}
    local concurrency=${6:-10}
    
    echo "================================================" | tee -a "$RESULTS_FILE"
    echo "Benchmark: $name" | tee -a "$RESULTS_FILE"
    echo "  Requests: $requests, Concurrency: $concurrency" | tee -a "$RESULTS_FILE"
    echo "------------------------------------------------" | tee -a "$RESULTS_FILE"
    
    if [ "$method" = "GET" ]; then
        ab -n "$requests" -c "$concurrency" -T "application/json" "$BASE_URL$endpoint" 2>&1 | \
            grep -E "Requests per second|Time per request|Transfer rate|Failed requests|Non-2xx" | \
            tee -a "$RESULTS_FILE"
    else
        # Create temp file with data
        local temp_file=$(mktemp)
        echo "$data" > "$temp_file"
        
        ab -n "$requests" -c "$concurrency" -p "$temp_file" -T "application/json" "$BASE_URL$endpoint" 2>&1 | \
            grep -E "Requests per second|Time per request|Transfer rate|Failed requests|Non-2xx" | \
            tee -a "$RESULTS_FILE"
        
        rm "$temp_file"
    fi
    
    echo "" | tee -a "$RESULTS_FILE"
}

# Check if ab (Apache Bench) is available
if ! command -v ab &> /dev/null; then
    echo "ERROR: 'ab' (Apache Bench) is not installed"
    echo "Install it with: brew install httpd (macOS) or apt-get install apache2-utils (Linux)"
    exit 1
fi

echo "Starting benchmarks..." | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Benchmark 1: CREATE_TXN - Low concurrency
run_benchmark \
    "CREATE Transaction (Low concurrency)" \
    "POST" \
    "/api/oracle/create" \
    '{
        "transaction_type": "sale",
        "transaction_id": "BENCH-'$(date +%s%N)'",
        "account": "BENCH-ACC",
        "amount": 10000,
        "currency": "USD",
        "description": "Benchmark transaction"
    }' \
    50 \
    5

# Benchmark 2: CREATE_TXN - Medium concurrency
run_benchmark \
    "CREATE Transaction (Medium concurrency)" \
    "POST" \
    "/api/oracle/create" \
    '{
        "transaction_type": "sale",
        "transaction_id": "BENCH-'$(date +%s%N)'",
        "account": "BENCH-ACC",
        "amount": 10000,
        "currency": "USD",
        "description": "Benchmark transaction"
    }' \
    100 \
    10

# Benchmark 3: CREATE_TXN - High concurrency
run_benchmark \
    "CREATE Transaction (High concurrency)" \
    "POST" \
    "/api/oracle/create" \
    '{
        "transaction_type": "sale",
        "transaction_id": "BENCH-'$(date +%s%N)'",
        "account": "BENCH-ACC",
        "amount": 10000,
        "currency": "USD",
        "description": "Benchmark transaction"
    }' \
    200 \
    20

# Create some test transactions for GET benchmarks
echo "Creating test transactions for GET benchmarks..." | tee -a "$RESULTS_FILE"
TEST_TXN_ID="BENCH-TEST-$(date +%s)"
curl -s -X POST "$BASE_URL/api/oracle/create" \
    -H "Content-Type: application/json" \
    -d "{
        \"transaction_type\": \"sale\",
        \"transaction_id\": \"$TEST_TXN_ID\",
        \"account\": \"BENCH-ACC\",
        \"amount\": 10000,
        \"currency\": \"USD\",
        \"description\": \"Test transaction for GET benchmark\"
    }" > /dev/null

echo "✓ Test transaction created: $TEST_TXN_ID" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Benchmark 4: GET_TXN - Low concurrency
run_benchmark \
    "GET Transaction (Low concurrency)" \
    "POST" \
    "/api/oracle/get" \
    "{\"transaction_id\": \"$TEST_TXN_ID\"}" \
    100 \
    10

# Benchmark 5: GET_TXN - High concurrency
run_benchmark \
    "GET Transaction (High concurrency)" \
    "POST" \
    "/api/oracle/get" \
    "{\"transaction_id\": \"$TEST_TXN_ID\"}" \
    500 \
    50

# Benchmark 6: LIST_TXN - Low concurrency
run_benchmark \
    "LIST Transactions (Low concurrency)" \
    "GET" \
    "/api/oracle/list" \
    "" \
    50 \
    5

# Benchmark 7: LIST_TXN - High concurrency
run_benchmark \
    "LIST Transactions (High concurrency)" \
    "GET" \
    "/api/oracle/list" \
    "" \
    200 \
    20

# Summary
echo "================================================" | tee -a "$RESULTS_FILE"
echo "Benchmark Summary" | tee -a "$RESULTS_FILE"
echo "================================================" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Count total transactions created
TOTAL_CREATED=$(( 50 + 100 + 200 ))
echo "Total transactions created: $TOTAL_CREATED" | tee -a "$RESULTS_FILE"

# Check database
echo "" | tee -a "$RESULTS_FILE"
echo "Checking database..." | tee -a "$RESULTS_FILE"
DB_COUNT=$(docker-compose exec -T oracledb sqlplus -S ctp/ctp@//localhost:1521/XE <<< "SELECT COUNT(*) FROM transactions;" 2>/dev/null | grep -E "^[0-9]+$" | head -1)
echo "Total records in database: $DB_COUNT" | tee -a "$RESULTS_FILE"

echo "" | tee -a "$RESULTS_FILE"
echo "Benchmark completed at $(date)" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"
echo "Results saved to: $RESULTS_FILE"
echo ""
echo "================================================"
echo "To view detailed results:"
echo "  cat $RESULTS_FILE"
echo "================================================"
