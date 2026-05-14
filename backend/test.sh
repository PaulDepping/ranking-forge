#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

CONTAINER="ranking-forge-test"
PORT=15432
export DATABASE_URL="postgres://postgres:postgres@localhost:${PORT}/postgres"

cleanup() {
    docker rm -f "$CONTAINER" 2>/dev/null || true
}
trap cleanup EXIT

docker run -d \
    --name "$CONTAINER" \
    -e POSTGRES_PASSWORD=postgres \
    -p "${PORT}:5432" \
    postgres:18

echo "Waiting for Postgres..."
until docker exec "$CONTAINER" pg_isready -U postgres -q 2>/dev/null; do
    sleep 0.1
done
sleep 0.5

cargo test --workspace "$@"
