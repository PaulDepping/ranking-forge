# Tournament Import Error Handling — Design Spec

**Date:** 2026-05-22
**Scope:** "Add players from tournament" tab (`TournamentTab.svelte` → `GET /projects/{id}/tournament-entrants`)

---

## Problem

The "Add players from tournament" tab has two silent failure modes:

1. **Tournament not found.** When the slug doesn't match any tournament on start.gg, both `tournament_participants` and `tournament_events_with_entrants` receive `null` from the API and silently collapse it to an empty `Vec`. The user sees 0 results with no explanation.

2. **Events not published.** A tournament may exist on start.gg (registration open, participants visible) but some or all events haven't been published yet (`state = "CREATED"`). Those events return 0 entrants. The user sees empty event tabs with no explanation of why.

The background import job (`/import` page) is out of scope for this change.

---

## Confirmed API Behaviour

From the introspected `schema.graphql` and `docs/startgg/project-notes.md`:

- `tournament(slug: String): Tournament` — nullable return type. An unknown slug returns `null` for `data.tournament`.
- `Event.state: ActivityState` — arrives on the wire as a **string**. Values: `"CREATED"` (not yet published), `"ACTIVE"` (running), `"COMPLETED"` (finished). Matches the existing `Option<String>` typing on `EventNode.state`.
- `TOURNAMENT_ALL_EVENTS_QUERY` currently requests only `{ id name }` — `state` must be added.

---

## Design

### Data Flow

```
StartggClient (common)
  tournament_participants      → Result<Option<Vec<...>>, StartggError>
                                 None  = tournament null on start.gg (not found)
                                 Some  = tournament found (list may be empty)

  tournament_events_with_entrants → Result<Vec<...>, StartggError>   (unchanged)
                                 each TournamentEventWithEntrants carries state: Option<String>

API handler  list_tournament_entrants  (api/src/routes/players.rs)
  → participants == None  →  422 "Tournament not found on start.gg"
  → threads event state through to TournamentEventResp

Frontend  TournamentTab.svelte
  → fetchError already displayed; now receives a real message for not-found
  → per-event: if entrants == 0 && state == "CREATED" → info note
  → "All" tab unaffected — participants show regardless of event state
```

### Backend Changes

#### 2a — `TOURNAMENT_ALL_EVENTS_QUERY` + `TournamentAllEventNode`
**Files:** `common/src/startgg/operations.rs`, `common/src/startgg/queries.rs`

Add `state` to the GraphQL query:
```graphql
query($slug: String!) {
    tournament(slug: $slug) {
        events {
            id name state
        }
    }
}
```

Add `state: Option<String>` to `TournamentAllEventNode`. Thread it through `TournamentEventWithEntrants` (public output type in `queries.rs`).

#### 2b — `tournament_participants` return type
**File:** `common/src/startgg/operations.rs`

Change signature:
```rust
// before
pub async fn tournament_participants(&self, ...) -> Result<Vec<TournamentParticipant>, StartggError>

// after
pub async fn tournament_participants(&self, ...) -> Result<Option<Vec<TournamentParticipant>>, StartggError>
```

Logic change inside the pagination loop: on page 1, if `data.tournament` is `None`, return `Ok(None)`. On subsequent pages an unexpected null simply breaks the loop. On success return `Ok(Some(result))`.

`tournament_events_with_entrants` keeps its existing return type — a null tournament there would already be caught by the participants check above.

#### 2c — `list_tournament_entrants` handler
**File:** `api/src/routes/players.rs`

```rust
let participants = match startgg.tournament_participants(&handle).await? {
    Some(p) => p,
    None => return Err(AppError::UnprocessableEntity(
        "Tournament not found on start.gg".into()
    )),
};
```

Add `state: Option<String>` to `TournamentEventResp` and populate it from `TournamentEventWithEntrants.state`.

### Frontend Changes

#### 3a — `lib/types.ts`

Add `state: string | null` to the `TournamentEventWithEntrants` type (or equivalent frontend type).

#### 3b — `TournamentTab.svelte` — per-event empty state

When an event tab is active and `visibleEntrants.length === 0`, show a message above (or instead of) the scroll area:

- `state === "CREATED"` → "This event's brackets haven't been published yet"
- otherwise → "No entrants found for this event"

The current event is derived from `tournamentData.events.find(e => String(e.id) === activeTab)`.

The "All" participants tab is unchanged.

#### 3c — Error display

No change required. The existing `{#if fetchError}` block in `TournamentTab.svelte` already renders the API error message. It will now receive `"Tournament not found on start.gg"` instead of the generic `"upstream API error"`.

---

## Out of Scope

- Background import job error messages (`/import` page, worker)
- The "By handle" and "By name" tabs in AddPlayersDialog
- Any changes to the `link_account` handler or other player routes
