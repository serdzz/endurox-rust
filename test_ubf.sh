#!/bin/bash

echo "Testing UBF Services"
echo "===================="
echo ""

# Test using ud (Enduro/X UBF dump utility)
echo "1. Testing UBFADD service - creating buffer with fields"
echo ""
echo "Calling UBFADD..."
ud | xadmin lcf UBFADD | ud
echo ""

echo "2. Testing UBFTEST service with name field"
echo ""
echo "Input: T_NAME_FLD=TestUser"
ud -n T_NAME_FLD -v "TestUser" | xadmin lcf UBFTEST | ud
echo ""

echo "3. Testing UBFECHO service"
echo ""
echo "Input: T_NAME_FLD='Echo Test', T_ID_FLD=123"
ud -n T_NAME_FLD -v "Echo Test" -n T_ID_FLD -v 123 | xadmin lcf UBFECHO | ud
echo ""

echo "4. Testing UBFGET service with multiple fields"
echo ""
echo "Input: T_NAME_FLD='John Doe', T_ID_FLD=9999, T_PRICE_FLD=123.45"
ud -n T_NAME_FLD -v "John Doe" -n T_ID_FLD -v 9999 -n T_PRICE_FLD -v 123.45 | xadmin lcf UBFGET | ud
echo ""

echo "===================="
echo "Tests completed"
