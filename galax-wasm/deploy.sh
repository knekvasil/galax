#!/usr/bin/env bash
set -euo pipefail

# Build WASM bundle
wasm-pack build --target web --release

# Copy static files alongside pkg/
cp www/index.html pkg/
cp www/style.css pkg/

echo "Deployable output in pkg/"
echo "Serve with: npx serve pkg/"
