#!/bin/bash

echo "Testing Enduro/X REST Gateway (Rust)"
echo "====================================="
echo ""

echo "0. Testing Health Check (GET /)"
curl -X GET http://localhost:8080/
echo -e "\n"

echo "1. Testing STATUS endpoint (GET /api/status)"
curl -X GET http://localhost:8080/api/status | jq .
echo -e "\n"

echo "2. Testing HELLO endpoint (POST /api/hello)"
curl -X POST http://localhost:8080/api/hello \
  -H "Content-Type: application/json" \
  -d '{"name":"Alexander"}' | jq .
echo -e "\n"

echo "3. Testing ECHO endpoint (POST /api/echo)"
curl -X POST http://localhost:8080/api/echo \
  -H "Content-Type: text/plain" \
  -d 'Hello from REST Gateway!' | jq .
echo -e "\n"

echo "4. Testing DATAPROC endpoint (POST /api/dataproc)"
curl -X POST http://localhost:8080/api/dataproc \
  -H "Content-Type: application/json" \
  -d '{"data":"test","count":123}' | jq .
echo -e "\n"

echo "====================================="
echo "Tests completed"
