#!/usr/bin/env bash
set -euo pipefail

# Build WASM bundle
wasm-pack build --target web --release

# Copy static files alongside pkg/
cp www/index.html pkg/
cp www/style.css pkg/
cp www/CNAME pkg/ 2>/dev/null || true

echo ""
echo "  ✅  Deployable output in pkg/"
echo ""
echo "  📦  Local:    npx serve pkg/"
echo "  ⚡  Surge:    npx surge pkg/ galax-nbody.surge.sh"
echo "  ☁️   Vercel:   npx vercel pkg/"
echo "  🦆  Cloudflare: drag pkg/ into https://dash.cloudflare.com → Pages"
echo ""
