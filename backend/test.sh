#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

VERBOSE=false
PASSTHROUGH=()

for arg in "$@"; do
    case "$arg" in
        -v|--verbose) VERBOSE=true ;;
        *) PASSTHROUGH+=("$arg") ;;
    esac
done

CONTAINER="ranking-forge-test"
PORT=15432
export DATABASE_URL="postgres://postgres:postgres@localhost:${PORT}/postgres"
export SQLX_OFFLINE=true

cleanup() {
    docker rm -f "$CONTAINER" 2>/dev/null || true
}
trap cleanup EXIT

docker run -d \
    --name "$CONTAINER" \
    -e POSTGRES_PASSWORD=postgres \
    -p "${PORT}:5432" \
    postgres:18 >/dev/null

echo "Waiting for Postgres..."
until docker exec "$CONTAINER" pg_isready -U postgres -q 2>/dev/null; do
    sleep 0.1
done
sleep 0.5

if $VERBOSE; then
    cargo test --workspace "${PASSTHROUGH[@]+"${PASSTHROUGH[@]}"}"
else
    tmpfile=$(mktemp)
    if cargo test --workspace "${PASSTHROUGH[@]+"${PASSTHROUGH[@]}"}" >"$tmpfile" 2>&1; then
        rm -f "$tmpfile"
        echo "PASS"
    else
        cat "$tmpfile"
        rm -f "$tmpfile"
        exit 1
    fi
fi

# Live start.gg API tests were removed in Task 10 (mirror-backed architecture).
# The live-tests feature and import_live.rs have been deleted.
