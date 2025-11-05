#!/bin/bash

# Source environment
. /app/setenv.sh

# Preload libnstd.so to provide symbols for libubf.so
export LD_PRELOAD=/opt/endurox/lib/libnstd.so

# Run derive macro example
echo "=== Running UbfStruct Derive Macro Example ==="
/app/bin/derive_macro_example
