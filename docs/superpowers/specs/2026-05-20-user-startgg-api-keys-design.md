# Design: User-Provided start.gg API Keys

**Date:** 2026-05-20
**Status:** Approved

## Problem

RankingForge currently uses a single server-wide `STARTGG_API_KEY` env var shared across all imports and the games-search endpoint. As the user base grows, all imports share one rate-limit quota (80 req/60s), creating a scaling bottleneck.

## Solution

Each user stores their own start.gg API key. The project owner's key is used for all imports on that project. All start.gg API calls (games search + imports) use a per-user key. The server-wide `STARTGG_API_KEY` env var is removed entirely.

## Decisions

- **Whose key?** Project owner's key — editors don't need their own keys, reducing onboarding friction.
- **Key required?** Yes — the import endpoint returns 422 and the import UI is blocked if the owner has no key configured.
- **Key validation on save?** Yes — a lightweight `search_games` call validates the key before storing it.
- **Storage approach?** `startgg_api_key TEXT` column on `users` (nullable). Worker looks up the owner's key at job-claim time; key is not duplicated into job records.
- **Games search?** Switches from server-wide key to the authenticated user's key. Becomes auth-required.

## Database

Modify `backend/migrations/001_initial.sql` to add a `startgg_api_key` column to `users`:

```sql
CREATE TABLE users (
    ...
    startgg_api_key TEXT  -- nullable; user's personal start.gg developer token
);
```

The `User` model in `common/src/models/mod.rs` gains `startgg_api_key: Option<String>`. This field is **never** included in API responses — treated like `password_hash`.

## Backend: API

### New endpoints

**`PUT /account/startgg-key`** (auth required)
- Request: `{ "api_key": "..." }`
- Validates the key by calling `StartggClient::new(api_key).search_games("smash")`.
- On success: stores key in `users.startgg_api_key`, returns 200.
- On invalid key: returns 422 `{ "error": "Invalid start.gg API key" }`.

**`DELETE /account/startgg-key`** (auth required)
- Sets `users.startgg_api_key = NULL`.
- Returns 204.

### Existing endpoint changes

**`GET /account/me`** (or profile endpoint)
- Add `has_startgg_key: bool` to response (derived from whether `startgg_api_key IS NOT NULL`). Never return the key value.
- If no unauthenticated GET endpoint for the current user exists, add `GET /account/me`.

**`GET /projects/:id`** (project detail)
- Add `owner_has_startgg_key: bool` to the response. This allows editors (who are not the owner) to see whether the owner has a key configured, so the import panel can show the appropriate callout without requiring the editor to attempt an import first.

**`GET /games`** (games search)
- Becomes auth-required (`AuthUser` extractor).
- Creates a one-off `StartggClient` from `AuthUser`'s `startgg_api_key`.
- Returns 422 `{ "error": "Configure a start.gg API key in account settings before searching" }` if no key is set.

**`POST /projects/:id/import`**
- Before enqueuing, queries the project owner's `startgg_api_key`.
- Returns 422 `{ "error": "Project owner has not configured a start.gg API key" }` if NULL.
- No change to `ImportParams` — key is not embedded in the job record.

### AppState

`AppState` loses the `startgg: StartggClient` field entirely. The `STARTGG_API_KEY` env var is removed from the API's `Config`.

## Backend: Worker

### Config

`STARTGG_API_KEY` is removed from `worker/src/config.rs`. The env var is no longer required.

### Job execution

After claiming a job, before spawning the import task, the worker runs:

```sql
SELECT u.startgg_api_key
FROM ranking_projects rp
JOIN users u ON u.id = rp.owner_id
WHERE rp.id = $1
```

- If `startgg_api_key` is NULL: mark job failed with error `"Project owner has no start.gg API key configured"`.
- Otherwise: construct `StartggClient::new(api_key)` and pass it into `import::run`.

The `import::run` signature is unchanged — it already receives a `&StartggClient`. No changes to import logic.

## Frontend

### Account settings page

A "start.gg API Key" section is added to the user's account settings. Two states:

**No key configured:**
- A `Card` with a brief explanation, a link to `https://start.gg/admin/profile/developer` ("Get your API key →"), a masked `Input` (type="password"), and a "Save" `Button`.
- On save: calls `PUT /account/startgg-key`. Shows inline error on 422.

**Key configured:**
- Shows "API key configured ✓" with a `Badge` and a "Remove" `Button`.
- Remove calls `DELETE /account/startgg-key`.
- The key value is never shown — only presence is communicated.

### Import panel inline callout

When `owner_has_startgg_key` is `false` and the logged-in user is the project owner:
- Replace import controls with a `Card` callout: "A start.gg API key is required to run imports. [Configure in account settings →]" with a link to `https://start.gg/admin/profile/developer`.

When `owner_has_startgg_key` is `false` and the logged-in user is an editor (not the owner):
- Show: "The project owner needs to configure a start.gg API key before imports can run."

When the key is present: no change to existing import controls.

## openapi.yaml changes

- Add `PUT /account/startgg-key`
- Add `DELETE /account/startgg-key`
- Add or update `GET /account/me` response to include `has_startgg_key: boolean`
- Update `GET /projects/{project_id}` response to include `owner_has_startgg_key: boolean`
- Update `GET /games` to require authentication
- Update `POST /projects/{project_id}/import` to document 422 when owner has no key

## What is NOT changing

- Import logic in `worker/src/import.rs` — no changes beyond receiving a different `StartggClient`
- Jobs table schema — no `api_key` field added to job records
- `ImportParams` — unchanged
- Rate-limit retry and complexity-halving logic in `StartggClient` — unchanged
