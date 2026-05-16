#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"

VERBOSE=false
PASSTHROUGH=()

for arg in "$@"; do
    case "$arg" in
        -v|--verbose) VERBOSE=true ;;
        *) PASSTHROUGH+=("$arg") ;;
    esac
done

run_section() {
    local title="$1"
    shift
    if $VERBOSE; then
        echo "=== $title ==="
        "$@"
        echo ""
        return
    fi
    printf "%-36s" "=== $title ==="
    local tmpfile
    tmpfile=$(mktemp)
    if "$@" >"$tmpfile" 2>&1; then
        echo "PASS"
        rm -f "$tmpfile"
    else
        echo "FAIL"
        echo ""
        cat "$tmpfile"
        rm -f "$tmpfile"
        exit 1
    fi
}

run_section "Backend tests" bash "$ROOT/backend/test.sh" "${PASSTHROUGH[@]+"${PASSTHROUGH[@]}"}"

cd "$ROOT/web"
run_section "Frontend unit tests" npm run test:unit
run_section "Frontend e2e tests" npm run test:e2e

if ! $VERBOSE; then
    echo ""
    echo "All tests passed."
fi
