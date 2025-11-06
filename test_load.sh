#!/bin/bash

echo "========================================"
echo "Enduro/X REST Gateway - Load Testing"
echo "========================================"
echo ""

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test configuration
REQUESTS=10000
CONCURRENCY=100
BASE_URL="http://localhost:8080"

echo -e "${BLUE}Test Configuration:${NC}"
echo "  Requests: $REQUESTS"
echo "  Concurrency: $CONCURRENCY"
echo "  Target: $BASE_URL"
echo ""

# Test 1: Health check (GET /)
echo -e "${GREEN}1. Health Check (GET /)${NC}"
ab -n $REQUESTS -c $CONCURRENCY "$BASE_URL/" 2>&1 | grep -E "Requests per second|Time per request|Transfer rate|Failed requests"
echo ""

# Test 2: Status endpoint (GET /api/status)
echo -e "${GREEN}2. Status Endpoint (GET /api/status)${NC}"
ab -n $REQUESTS -c $CONCURRENCY "$BASE_URL/api/status" 2>&1 | grep -E "Requests per second|Time per request|Transfer rate|Failed requests"
echo ""

# Test 3: Echo endpoint (POST /api/echo)
echo -e "${GREEN}3. Echo Endpoint (POST /api/echo)${NC}"
echo "Test data for echo" > /tmp/echo_data.txt
ab -n $REQUESTS -c $CONCURRENCY -p /tmp/echo_data.txt -T "text/plain" "$BASE_URL/api/echo" 2>&1 | grep -E "Requests per second|Time per request|Transfer rate|Failed requests"
rm /tmp/echo_data.txt
echo ""

# Test 4: Hello endpoint (POST /api/hello)
echo -e "${GREEN}4. Hello Endpoint (POST /api/hello)${NC}"
cat > /tmp/hello_data.json << EOF
{"name":"LoadTest"}
EOF
ab -n $REQUESTS -c $CONCURRENCY -p /tmp/hello_data.json -T "application/json" "$BASE_URL/api/hello" 2>&1 | grep -E "Requests per second|Time per request|Transfer rate|Failed requests"
rm /tmp/hello_data.json
echo ""

# Test 5: Transaction endpoint - SUCCESS case (POST /api/transaction)
echo -e "${GREEN}5. Transaction Endpoint - SALE (POST /api/transaction)${NC}"
cat > /tmp/transaction_sale.json << EOF
{
  "transaction_type": "sale",
  "transaction_id": "TXN-LOAD-TEST",
  "account": "ACC-TEST",
  "amount": 10000,
  "currency": "USD",
  "description": "Load test transaction"
}
EOF
ab -n $REQUESTS -c $CONCURRENCY -p /tmp/transaction_sale.json -T "application/json" "$BASE_URL/api/transaction" 2>&1 | grep -E "Requests per second|Time per request|Transfer rate|Failed requests"
rm /tmp/transaction_sale.json
echo ""

# Test 6: Transaction endpoint - ERROR case (POST /api/transaction)
echo -e "${GREEN}6. Transaction Endpoint - REFUND (error) (POST /api/transaction)${NC}"
cat > /tmp/transaction_refund.json << EOF
{
  "transaction_type": "refund",
  "transaction_id": "TXN-ERROR-TEST",
  "account": "ACC-TEST",
  "amount": 5000,
  "currency": "USD"
}
EOF
ab -n $REQUESTS -c $CONCURRENCY -p /tmp/transaction_refund.json -T "application/json" "$BASE_URL/api/transaction" 2>&1 | grep -E "Requests per second|Time per request|Transfer rate|Failed requests"
rm /tmp/transaction_refund.json
echo ""

# Extended test with higher load
echo "========================================"
echo -e "${BLUE}Extended Test - High Load${NC}"
echo "  Requests: 50000"
echo "  Concurrency: 200"
echo "========================================"
echo ""

echo -e "${GREEN}Transaction Endpoint - Extended Load${NC}"
cat > /tmp/transaction_extended.json << EOF
{
  "transaction_type": "sale",
  "transaction_id": "TXN-EXT",
  "account": "ACC-999",
  "amount": 25000,
  "currency": "EUR"
}
EOF
ab -n 50000 -c 200 -p /tmp/transaction_extended.json -T "application/json" "$BASE_URL/api/transaction" 2>&1 | grep -E "Requests per second|Time per request|Transfer rate|Failed requests|Complete requests|Non-2xx"
rm /tmp/transaction_extended.json
echo ""

echo "========================================"
echo "Load Testing Complete"
echo "========================================"
