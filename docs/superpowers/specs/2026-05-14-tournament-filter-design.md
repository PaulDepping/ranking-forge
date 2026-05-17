# Tournament Filter & Bulk Action

**Date:** 2026-05-14

## Context

The tournament page currently shows every imported event in a flat list with no way to narrow it down. Users must scroll through the entire list to manually exclude unwanted events (online tournaments, doubles brackets, weekly/ladder events) from their rankings. This feature adds a collapsible filter panel and a pair of bulk-action buttons to make that workflow fast.

---

## What We're Building

A **collapsible "Filters & Actions" panel** above the tournament list. It contains six filter controls and two bulk-action buttons. All filtering is client-side — no additional API calls. Several new columns are added to `events`, `tournaments`, `sets`, and the new `phases`/`phase_groups` tables — fetched from start.gg during import — so filters can use structured data instead of name-matching.

---

## Filter Behaviour

Filters operate at **two levels**:

**Tournament-level** (hide the whole tournament):
- Venue: online / offline / all
- Date range: from / to (matched against `tournament.start_at`)

**Event-level** (hide individual events; a tournament stays visible as long as ≥ 1 event passes):
- Name search: case-insensitive substring match against the event name OR the parent tournament name. Searching "genesis" shows all events under matching tournaments; searching "doubles" shows only doubles events across all tournaments.
- Entrant range: hide events where `num_entrants < min` or `num_entrants > max`; either bound is optional; events with `null` entrant count pass (shown)
- Event type dropdown (All / Singles / Teams): "Singles" hides events where `event_type != 1`; "Teams" hides events where `event_type != 2`; events with `null` event_type pass in all modes
- Exclude ladder / matchmaking (checkbox): hide events where every entry in `bracket_types` is `'MATCHMAKING'` (i.e. the event is purely matchmaking — a pools→bracket event passes)

The status line above the panel reads: **"Showing N of M tournaments · K of J events"**

---

## Bulk Actions

Two buttons at the bottom of the panel, acting on all currently visible events:

- **Include all visible** — sets `included = true` for every visible event
- **Exclude all visible** — sets `included = false` for every visible event

Each calls the existing `PATCH /projects/{id}/events/{eid}` endpoint per event (reuses existing optimistic-update logic). The event counts in the panel are small enough that sequential calls are fine.

---

## Schema Changes

`backend/migrations/001_initial.sql` — edit in-place (no production DB).

### Extended `events` columns

```sql
slug             TEXT,     -- start.gg event slug, needed for bracket URL
state            TEXT,     -- event state (e.g. ACTIVE, COMPLETED)
is_online        BOOLEAN,  -- event-level online flag (can differ from tournament)
event_type       INTEGER,  -- 1 = individual/singles, 2 = teams/doubles
min_team_size    INTEGER,  -- from teamRosterSize.minPlayers (1 for singles, 2 for doubles, etc.)
max_team_size    INTEGER,  -- from teamRosterSize.maxPlayers
```

### Extended `tournaments` columns

```sql
timezone  TEXT,             -- already fetched but never stored
lat       DOUBLE PRECISION, -- coordinates for potential map features
lng       DOUBLE PRECISION,
state     INTEGER,          -- tournament activity state (active, completed, cancelled)
```

### New `phases` table

Bracket phases within an event (e.g. "Pools", "Top 8 Bracket"):

```sql
CREATE TABLE phases (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id    BIGINT      NOT NULL UNIQUE,  -- start.gg phase ID
    event_id      UUID        NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    name          TEXT,
    bracket_type  TEXT,       -- DOUBLE_ELIMINATION, ROUND_ROBIN, MATCHMAKING, etc.
    phase_order   INTEGER,    -- 1-based display order
    num_seeds     INTEGER,
    group_count   INTEGER,
    state         INTEGER,
    is_exhibition BOOLEAN
);

CREATE INDEX phases_event_id_idx ON phases(event_id);
```

### New `phase_groups` table

Individual brackets/pools within a phase (e.g. "Pool A", "Pool B"):

```sql
CREATE TABLE phase_groups (
    id                 UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id         BIGINT      NOT NULL UNIQUE,  -- start.gg phaseGroup ID
    phase_id           UUID        NOT NULL REFERENCES phases(id) ON DELETE CASCADE,
    display_identifier TEXT,       -- "Pool A", "1", etc.
    bracket_type       TEXT,
    bracket_url        TEXT,       -- direct start.gg bracket URL — used for set links
    num_rounds         INTEGER,
    start_at           TIMESTAMPTZ,
    first_round_time   TIMESTAMPTZ, -- actual start time of the first round
    state              INTEGER
);

CREATE INDEX phase_groups_phase_id_idx ON phase_groups(phase_id);
```

### Extended `sets` columns

```sql
phase_group_id  UUID     REFERENCES phase_groups(id),
has_placeholder BOOLEAN, -- true if one slot is a bye/placeholder (not a real match)
state           INTEGER, -- set state code from start.gg
identifier      TEXT,    -- bracket position label, e.g. "A3", "W1", "GF"
```

`phase_group_id`: during import the start.gg integer phaseGroup ID is resolved to the internal UUID with a `SELECT id FROM phase_groups WHERE startgg_id = $1` lookup — the same pattern used when linking sets to events.

`has_placeholder`: sets where this is `true` are bye matches and must be excluded from stats and H2H queries alongside `is_dq = false` (add `AND has_placeholder = false` to the existing WHERE clauses in `get_stats` and `get_head_to_head`).

This gives every set a path to its bracket URL via `phase_groups.bracket_url`, enabling direct start.gg links from the stats and H2H detail views — no URL reconstruction needed.

> **Scope note:** the phases schema is added now to get it right while there is no production DB. The filter feature uses `phases.bracket_type` to power the "Exclude ladder / matchmaking" filter. Set linking (surfacing `bracket_url` in the UI) is a follow-on feature.

---

## Backend Changes

### 1. GraphQL query — `common/src/startgg/operations.rs`

Extend the tournaments query to add tournament-level fields and the richer event fragment:

```graphql
# on TournamentNode — add:
timezone lat lng state

# event fragment:
events(filter: { videogameId: [$gameId] }) {
    id name numEntrants startAt
    slug state isOnline type
    teamRosterSize { minPlayers maxPlayers }
    phases {
        id name bracketType phaseOrder numSeeds groupCount state isExhibition
        phaseGroups(query: { perPage: 100 }) {
            nodes {
                id displayIdentifier bracketType bracketUrl
                numRounds startAt firstRoundTime state
            }
        }
    }
}
```

Extend the set fragment (in `event_sets`) to include:

```graphql
hasPlaceholder state identifier
phaseGroup { id }
```

### 2. Structs — `common/src/startgg/queries.rs`

Add new structs mirroring the schema:

```rust
pub struct TeamRosterSize {
    pub min_players: Option<i32>,
    pub max_players: Option<i32>,
}

pub struct PhaseGroupNode {
    pub id: i64,
    pub display_identifier: Option<String>,
    pub bracket_type: Option<String>,
    pub bracket_url: Option<String>,
    pub num_rounds: Option<i32>,
    pub start_at: Option<i64>,
    pub first_round_time: Option<i64>,
    pub state: Option<i32>,
}

pub struct PhaseNode {
    pub id: i64,
    pub name: Option<String>,
    pub bracket_type: Option<String>,
    pub phase_order: Option<i32>,
    pub num_seeds: Option<i32>,
    pub group_count: Option<i32>,
    pub state: Option<i32>,
    pub is_exhibition: Option<bool>,
    pub phase_groups: Option<PhaseGroupPage>,
}

// extend EventNode:
pub struct EventNode {
    // existing: id, name, num_entrants, start_at
    pub slug: Option<String>,
    pub state: Option<String>,
    pub is_online: Option<bool>,
    pub r#type: Option<i32>,
    pub team_roster_size: Option<TeamRosterSize>,
    pub phases: Option<Vec<PhaseNode>>,
}

// extend TournamentNode:
// add: timezone (already present), lat, lng, state

// extend SetNode:
pub struct SetNode {
    // existing fields …
    pub has_placeholder: Option<bool>,
    pub state: Option<i32>,
    pub identifier: Option<String>,
    pub phase_group: Option<SetPhaseGroup>,
}
pub struct SetPhaseGroup { pub id: i64 }
```

### 3. Import — `worker/src/import.rs`

- **Tournament upsert**: add `timezone`, `lat`, `lng`, `state` to the INSERT/ON CONFLICT.
- **`import_event`**: extend INSERT to include `slug`, `state`, `is_online`, `event_type`, `min_team_size`, `max_team_size`. After upserting the event, upsert each phase into `phases` and each phase group into `phase_groups` (ON CONFLICT on `startgg_id`).
- **`import_set`**: add `has_placeholder`, `state`, `identifier`, and `phase_group_id` (resolved from `set.phase_group.id` via `SELECT id FROM phase_groups WHERE startgg_id = $1`).
- **Stats/H2H queries** (`get_stats`, `get_head_to_head`, `get_h2h_sets`): add `AND s.has_placeholder = false` alongside the existing `AND s.is_dq = false`.

### 4. API response — `api/src/routes/tournaments.rs`

Add to `ProjectEventResponse`:
```rust
pub event_type: Option<i32>,
pub bracket_types: Vec<String>,  // aggregated from phases, ordered by phase_order
```

Update `list_tournaments` SQL to LEFT JOIN phases and aggregate bracket types:

```sql
SELECT …,
       e.event_type,
       ARRAY(
           SELECT p.bracket_type
           FROM phases p
           WHERE p.event_id = e.id
             AND p.bracket_type IS NOT NULL
           ORDER BY p.phase_order
       ) AS bracket_types
FROM …
```

Update `patch_event` similarly so the returned `ProjectEventResponse` includes these fields.

### 5. OpenAPI — `backend/openapi.yaml`

Add `event_type` (`integer`, nullable) and `bracket_types` (`array` of `string`) to the event response schema.

### 6. sqlx offline cache

Run `bash backend/prepare-sqlx.sh` after all schema and query changes.

---

## Frontend Changes

### `web/src/lib/types.ts`

Add to `TournamentEvent`:
```ts
event_type: number | null
bracket_types: string[]   // ordered by phase_order; empty if phases not yet imported
```

### `web/src/routes/projects/[id]/tournaments/+page.svelte`

**New reactive filter state:**
```svelte
let filterOpen    = $state(false)
let search        = $state('')
let venueFilter   = $state<'all' | 'online' | 'offline'>('all')
let minEntrants   = $state<number | null>(null)
let maxEntrants   = $state<number | null>(null)
let dateFrom      = $state('')
let dateTo        = $state('')
let eventType     = $state<'all' | 'singles' | 'teams'>('all')
let excludeLadder = $state(false)
```

**Derived visibility:**
```svelte
const visibleTournaments = $derived(
  tournaments
    .filter(t => tournamentVisible(t))
    .map(t => ({ ...t, events: t.events.filter(e => eventVisible(e, t)) }))
    .filter(t => t.events.length > 0)
)
```

**Bulk-action handlers** iterate `visibleTournaments`, collect all event IDs, and call the existing toggle helper for each.

**UI layout** (collapsible panel, always-visible count line):
```
Showing N of M tournaments · K of J events      [Filters & Actions ▲]
┌──────────────────────────────────────────────────────────────────┐
│ [Search tournament or event name…]        [Venue: All ▼]        │
│ Entrants: [min] – [max]   From: [date]  To: [date]              │
│ Event type: [All ▼]   □ Exclude ladder / matchmaking            │
│ ─────────────────────────────────────────────────────────────   │
│ Bulk actions apply to K visible events                           │
│                    [✓ Include all visible] [✕ Exclude all visible]│
└──────────────────────────────────────────────────────────────────┘
```

---

## Testing

1. Run `bash backend/test.sh` — all existing tests pass.
2. Re-import a project; verify in the DB that `events` has `slug`, `event_type`, `min_team_size`/`max_team_size`; `tournaments` has `timezone`, `lat`, `lng`, `state`; `phases` and `phase_groups` are populated; `sets` have `has_placeholder`, `state`, `identifier`, and `phase_group_id`.
2a. Confirm that bye sets (`has_placeholder = true`) do not appear in the stats or H2H views.
3. On the tournament page:
   - Type "doubles" — only doubles events visible; other events hidden within their tournaments.
   - Type a tournament name — all events under matching tournaments visible.
   - Set Event type to "Singles" — team/doubles events disappear; set to "Teams" — singles events disappear.
   - Toggle "Exclude ladder / matchmaking" — events where all phases are MATCHMAKING disappear; a pools→bracket event (ROUND_ROBIN + DOUBLE_ELIMINATION) stays visible.
   - Set Venue to "Online only" — only online tournaments shown.
   - Set min entrants to 64 — small events hidden; set max entrants to 100 — large events hidden; use both together as a range.
   - "Exclude all visible" → verify `included = false` persisted (reload page).
   - "Include all visible" → verify `included = true` persisted.
4. Run `cd web && npm run test:e2e` — existing e2e tests pass (mock data may need `event_type`/`bracket_type` fields added as nulls).
