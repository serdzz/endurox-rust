#!/bin/bash
# Parallel benchmark for CREATE_TXN endpoint
# Tests throughput with multiple concurrent connections

set -e

BASE_URL="http://localhost:8080"
CONCURRENCY=${1:-10}  # Default 10 concurrent connections
REQUESTS=${2:-100}     # Default 100 total requests

echo "================================================"
echo "Parallel CREATE_TXN Benchmark"
echo "================================================"
echo "Concurrency: $CONCURRENCY"
echo "Total requests: $REQUESTS"
echo "Base URL: $BASE_URL"
echo ""

# Check if server is available
if ! curl -s -f "$BASE_URL/" > /dev/null; then
    echo "ERROR: Server at $BASE_URL is not responding"
    exit 1
fi

echo "✓ Server is available"
echo ""

# Create temporary file for POST data template
DATA_FILE=$(mktemp)
cat > "$DATA_FILE" <<'EOF'
{
    "transaction_type": "sale",
    "transaction_id": "BENCH-{{ID}}-{{TIME}}",
    "account": "ACC-{{ID}}",
    "amount": {{AMOUNT}},
    "currency": "USD",
    "description": "Benchmark transaction {{ID}}"
}
EOF

echo "Starting benchmark..."
echo ""

# Function to make a single request
make_request() {
    local id=$1
    local timestamp=$(date +%s%N)
    local amount=$((10000 + id))
    
    # Replace placeholders
    local data=$(sed "s/{{ID}}/$id/g; s/{{TIME}}/$timestamp/g; s/{{AMOUNT}}/$amount/g" "$DATA_FILE")
    
    # Make request and capture result
    local start_time=$(date +%s.%N)
    local response=$(curl -s -X POST "$BASE_URL/api/oracle/create" \
        -H "Content-Type: application/json" \
        -d "$data")
    local end_time=$(date +%s.%N)
    
    local duration=$(echo "$end_time - $start_time" | bc)
    
    # Check if successful
    if echo "$response" | grep -q "SUCCESS"; then
        echo "SUCCESS:$duration"
    else
        echo "FAILED:$duration"
    fi
}

export -f make_request
export BASE_URL
export DATA_FILE

# Run requests in parallel using GNU parallel or xargs
START_TIME=$(date +%s.%N)

if command -v parallel &> /dev/null; then
    # Use GNU parallel if available (faster)
    seq 1 $REQUESTS | parallel -j $CONCURRENCY "make_request {}"
else
    # Fallback to background processes
    for i in $(seq 1 $REQUESTS); do
        make_request $i &
        
        # Control concurrency manually
        if (( i % CONCURRENCY == 0 )); then
            wait
        fi
    done
    wait
fi > /tmp/benchmark_results.txt

END_TIME=$(date +%s.%N)

# Process results
TOTAL_TIME=$(echo "$END_TIME - $START_TIME" | bc)
SUCCESS_COUNT=$(grep -c "^SUCCESS:" /tmp/benchmark_results.txt || echo 0)
FAILED_COUNT=$(grep -c "^FAILED:" /tmp/benchmark_results.txt || echo 0)

# Calculate average response time for successful requests
if [ $SUCCESS_COUNT -gt 0 ]; then
    AVG_TIME=$(grep "^SUCCESS:" /tmp/benchmark_results.txt | cut -d: -f2 | \
        awk '{sum+=$1; count++} END {printf "%.3f", sum/count}')
else
    AVG_TIME="N/A"
fi

THROUGHPUT=$(echo "scale=2; $REQUESTS / $TOTAL_TIME" | bc)

# Cleanup
rm -f "$DATA_FILE" /tmp/benchmark_results.txt

echo ""
echo "================================================"
echo "Results"
echo "================================================"
echo "Total time: ${TOTAL_TIME}s"
echo "Total requests: $REQUESTS"
echo "Successful: $SUCCESS_COUNT"
echo "Failed: $FAILED_COUNT"
echo "Avg response time: ${AVG_TIME}s (successful only)"
echo "Throughput: ${THROUGHPUT} req/sec"
echo ""

# Calculate percentage
SUCCESS_PCT=$(echo "scale=1; 100 * $SUCCESS_COUNT / $REQUESTS" | bc)
echo "Success rate: ${SUCCESS_PCT}%"
echo "================================================"
