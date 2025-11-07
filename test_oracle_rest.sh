#!/bin/bash
# Test script for Oracle Transaction Server REST API endpoints
# This script tests the integration between REST gateway and oracle_txn_server

set -e

echo "================================================"
echo "Testing Oracle Transaction Server REST API"
echo "================================================"
echo ""

BASE_URL="http://localhost:8080"

# Test 1: Create transaction
echo "1. Creating transaction TXN100..."
curl -s -X POST $BASE_URL/api/oracle/create \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_type": "sale",
    "transaction_id": "TXN100",
    "account": "ACC1000",
    "amount": 99900,
    "currency": "USD",
    "description": "Test transaction from script"
  }' | jq '.'
echo ""

# Test 2: Get transaction
echo "2. Getting transaction TXN100..."
curl -s -X POST $BASE_URL/api/oracle/get \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_id": "TXN100"
  }' | jq '.'
echo ""

# Test 3: Create another transaction
echo "3. Creating transaction TXN101..."
curl -s -X POST $BASE_URL/api/oracle/create \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_type": "sale",
    "transaction_id": "TXN101",
    "account": "ACC2000",
    "amount": 50000,
    "currency": "EUR",
    "description": "Another test transaction"
  }' | jq '.'
echo ""

# Test 4: List all transactions
echo "4. Listing all transactions..."
curl -s -X GET $BASE_URL/api/oracle/list | jq '.'
echo ""

# Test 5: Try to get non-existent transaction
echo "5. Getting non-existent transaction..."
curl -s -X POST $BASE_URL/api/oracle/get \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_id": "TXN999"
  }' | jq '.'
echo ""

# Test 6: Try invalid transaction type
echo "6. Testing invalid transaction type..."
curl -s -X POST $BASE_URL/api/oracle/create \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_type": "invalid",
    "transaction_id": "TXN102",
    "account": "ACC3000",
    "amount": 1000,
    "currency": "USD",
    "description": "Should fail"
  }' | jq '.'
echo ""

echo "================================================"
echo "Tests completed!"
echo "================================================"
