#!/bin/bash

echo "========================================"
echo "Enduro/X REST Gateway - Load Testing with oha"
echo "========================================"
echo ""

BASE_URL="http://localhost:8080"

# Test 1: Health check
echo "1. Health Check (GET /)"
echo "----------------------------------------"
oha -n 50000 -c 100 "$BASE_URL/"
echo ""
echo ""

# Test 2: Status endpoint
echo "2. Status Endpoint (GET /api/status)"
echo "----------------------------------------"
oha -n 50000 -c 100 "$BASE_URL/api/status"
echo ""
echo ""

# Test 3: Hello endpoint
echo "3. Hello Endpoint (POST /api/hello)"
echo "----------------------------------------"
oha -n 50000 -c 100 \
  -m POST \
  -H "Content-Type: application/json" \
  -d '{"name":"LoadTest"}' \
  "$BASE_URL/api/hello"
echo ""
echo ""

# Test 4: Echo endpoint
echo "4. Echo Endpoint (POST /api/echo)"
echo "----------------------------------------"
oha -n 50000 -c 100 \
  -m POST \
  -H "Content-Type: text/plain" \
  -d "Load test data for echo service" \
  "$BASE_URL/api/echo"
echo ""
echo ""

# Test 5: Transaction - SUCCESS
echo "5. Transaction Endpoint - SALE (POST /api/transaction)"
echo "----------------------------------------"
oha -n 50000 -c 100 \
  -m POST \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_type": "sale",
    "transaction_id": "TXN-LOAD",
    "account": "ACC-TEST",
    "amount": 15000,
    "currency": "USD",
    "description": "Load test"
  }' \
  "$BASE_URL/api/transaction"
echo ""
echo ""

# Test 6: Transaction - ERROR
echo "6. Transaction Endpoint - REFUND/Error (POST /api/transaction)"
echo "----------------------------------------"
oha -n 50000 -c 100 \
  -m POST \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_type": "refund",
    "transaction_id": "TXN-ERROR",
    "account": "ACC-TEST",
    "amount": 5000,
    "currency": "USD"
  }' \
  "$BASE_URL/api/transaction"
echo ""
echo ""

# High concurrency test
echo "========================================"
echo "High Concurrency Test"
echo "  Requests: 100000"
echo "  Concurrency: 500"
echo "========================================"
echo ""

echo "Transaction Endpoint - High Load"
echo "----------------------------------------"
oha -n 100000 -c 500 \
  -m POST \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_type": "sale",
    "transaction_id": "TXN-HIGH",
    "account": "ACC-999",
    "amount": 99999,
    "currency": "EUR"
  }' \
  "$BASE_URL/api/transaction"
echo ""

echo "========================================"
echo "Load Testing Complete"
echo "========================================"
