#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"
OUT="$REPO_ROOT/backend/crates/common/src/startgg/schema.graphql"

# Load STARTGG_API_KEY from root .env if not already set in environment
if [ -z "${STARTGG_API_KEY:-}" ]; then
  ENV_FILE="$REPO_ROOT/.env"
  if [ -f "$ENV_FILE" ]; then
    export $(grep -E '^STARTGG_API_KEY=' "$ENV_FILE" | xargs)
  fi
fi

if [ -z "${STARTGG_API_KEY:-}" ]; then
  echo "Error: STARTGG_API_KEY is not set and was not found in .env" >&2
  echo "Set it in the environment or add STARTGG_API_KEY=<your-key> to .env" >&2
  exit 1
fi

echo "Fetching start.gg GraphQL schema..."

# Full introspection query (get-graphql-schema fails with premature-close on this endpoint)
INTROSPECTION='{"query":"fragment FullType on __Type { kind name description fields(includeDeprecated: true) { name description args { ...InputValue } type { ...TypeRef } isDeprecated deprecationReason } inputFields { ...InputValue } interfaces { ...TypeRef } enumValues(includeDeprecated: true) { name description isDeprecated deprecationReason } possibleTypes { ...TypeRef } } fragment InputValue on __InputValue { name description type { ...TypeRef } defaultValue } fragment TypeRef on __Type { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name } } } } } } } } { __schema { queryType { name } mutationType { name } subscriptionType { name } types { ...FullType } directives { name description locations args { ...InputValue } } } }"}'

TMPFILE=$(mktemp /tmp/startgg_introspection_XXXXXX.json)
trap 'rm -f "$TMPFILE"' EXIT

curl -s -X POST https://api.start.gg/gql/alpha \
  -H "Authorization: Bearer ${STARTGG_API_KEY}" \
  -H "Content-Type: application/json" \
  --compressed \
  -d "$INTROSPECTION" \
  -o "$TMPFILE"

if ! node -e "JSON.parse(require('fs').readFileSync('$TMPFILE','utf8'))" 2>/dev/null; then
  echo "Error: received invalid JSON from start.gg" >&2
  exit 1
fi

# Convert introspection JSON → SDL using the graphql npm package.
# Install into a temp dir so the repo stays clean.
GQLDIR=$(mktemp -d /tmp/gql_convert_XXXXXX)
npm install --prefix "$GQLDIR" graphql --silent
node -e "
const fs = require('fs');
const { buildClientSchema, printSchema } = require('$GQLDIR/node_modules/graphql');
const data = JSON.parse(fs.readFileSync('$TMPFILE', 'utf8'));
const sdl = printSchema(buildClientSchema(data.data));
fs.writeFileSync('$OUT', sdl);
console.log('Schema written (' + sdl.split('\n').length + ' lines).');
"
rm -rf "$GQLDIR"

echo "Done. Schema written to backend/crates/common/src/startgg/schema.graphql"
