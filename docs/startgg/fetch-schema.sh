#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"

# Load STARTGG_API_KEY from backend/.env if not already set in environment
if [ -z "${STARTGG_API_KEY:-}" ]; then
  ENV_FILE="$REPO_ROOT/backend/.env"
  if [ -f "$ENV_FILE" ]; then
    export $(grep -E '^STARTGG_API_KEY=' "$ENV_FILE" | xargs)
  fi
fi

if [ -z "${STARTGG_API_KEY:-}" ]; then
  echo "Error: STARTGG_API_KEY is not set and was not found in backend/.env" >&2
  echo "Set it in the environment or add STARTGG_API_KEY=<your-key> to backend/.env" >&2
  exit 1
fi

echo "Fetching start.gg GraphQL schema..."
npx --yes get-graphql-schema \
  https://api.start.gg/gql/alpha \
  --header "Authorization=Bearer ${STARTGG_API_KEY}" \
  > "$SCRIPT_DIR/schema.graphql"

echo "Done. Schema written to docs/startgg/schema.graphql"
