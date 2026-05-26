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

# ── Live start.gg API tests (optional) ───────────────────────────────────────
# Load STARTGG_API_KEY from the root .env if not already set in the environment.
if [ -z "${STARTGG_API_KEY:-}" ] && [ -f "../.env" ]; then
    # shellcheck disable=SC1091
    STARTGG_API_KEY=$(grep -E '^STARTGG_API_KEY=' "../.env" | head -1 | cut -d'=' -f2-)
    export STARTGG_API_KEY
fi

if [ -n "${STARTGG_API_KEY:-}" ]; then
    echo "Running live start.gg API tests..."
    if $VERBOSE; then
        cargo test -p e2e --features live-tests "${PASSTHROUGH[@]+"${PASSTHROUGH[@]}"}" -- --test-threads=1
    else
        tmpfile=$(mktemp)
        if cargo test -p e2e --features live-tests "${PASSTHROUGH[@]+"${PASSTHROUGH[@]}"}" -- --test-threads=1 >"$tmpfile" 2>&1; then
            rm -f "$tmpfile"
            echo "PASS (live)"
        else
            cat "$tmpfile"
            rm -f "$tmpfile"
            exit 1
        fi
    fi
else
    echo "STARTGG_API_KEY not set — skipping live start.gg API tests"
fi
