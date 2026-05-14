#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"

echo "=== Backend tests ==="
bash "$ROOT/backend/test.sh" "$@"

echo ""
echo "=== Frontend unit tests ==="
cd "$ROOT/web"
npm run test:unit

echo ""
echo "=== Frontend e2e tests ==="
npm run test:e2e
