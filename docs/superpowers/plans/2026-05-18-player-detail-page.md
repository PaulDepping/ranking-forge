# Player Detail Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a per-player detail page at `/projects/[id]/players/[player_id]` showing wins, losses, and full tournament attendance history, linked from every place a player name appears in the UI.

**Architecture:** Two new backend endpoints (`GET /projects/{id}/stats/{player_id}` and `GET /projects/{id}/players/{player_id}/tournaments`) feed a new SvelteKit page. Player names throughout the UI (stats cards, players list, H2H matrix, SetDetailModal) become links to this page.

**Tech Stack:** Rust/Axum (backend handlers, sqlx queries), SvelteKit + TypeScript (frontend route), shadcn-svelte components (Table, ScrollArea, Card), Vitest + Playwright (tests).

---

## File Map

| Action | Path | Responsibility |
|---|---|---|
| Modify | `backend/crates/api/src/routes/tournaments.rs` | Add `TournamentAttendance` struct, `get_player_stats`, `get_player_tournaments` handlers |
| Modify | `backend/crates/api/src/routes/projects.rs` | Register `/{id}/stats/{player_id}` route |
| Modify | `backend/crates/api/src/routes/players.rs` | Register `/{pid}/tournaments` route |
| Modify | `backend/openapi.yaml` | Add `TournamentAttendance` schema + two new endpoints |
| Modify | `backend/crates/api/tests/api.rs` | Add integration tests for both endpoints |
| Modify | `web/src/lib/types.ts` | Add `TournamentAttendance` interface |
| Create | `web/src/routes/projects/[id]/players/[player_id]/+page.server.ts` | Load player stats + tournament history |
| Create | `web/src/routes/projects/[id]/players/[player_id]/+page.svelte` | Player detail page |
| Modify | `web/src/lib/components/SetDetailModal.svelte` | Add optional `projectId` + `opponentPlayerId` props for opponent link |
| Modify | `web/src/routes/projects/[id]/stats/+page.svelte` | Player name → link; pass opponent link props to modal |
| Modify | `web/src/lib/components/PlayerCard.svelte` | Add `projectId` prop, player name → link |
| Modify | `web/src/routes/projects/[id]/players/+page.svelte` | Pass `projectId` to PlayerCard |
| Modify | `web/src/routes/projects/[id]/h2h/+page.svelte` | Row/col labels → links; pass opponent link props to modal |
| Modify | `web/tests/mock-api.js` | Add mock handlers for both new endpoints |

---

## Task 1: Backend — `get_player_stats` endpoint (TDD)

**Files:**
- Modify: `backend/crates/api/tests/api.rs`
- Modify: `backend/crates/api/src/routes/tournaments.rs`
- Modify: `backend/crates/api/src/routes/projects.rs`

- [ ] **Step 1: Write the failing test**

Append to `backend/crates/api/tests/api.rs`:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn player_stats_returns_single_player_data(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice_ps", "password123").await;
    let pid_str = create_project(&app, &cookie).await;
    let pid = Uuid::parse_str(&pid_str).unwrap();

    let alice_id = Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Alice").await).unwrap();
    let bob_id = Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Bob").await).unwrap();
    let charlie_id = Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Charlie").await).unwrap();

    let (_, event_id) = seed_tournament_event(&pool, pid, 7001, 8001).await;
    let alice_e = seed_entrant(&pool, event_id, Some(alice_id), 301, Some(1)).await;
    let bob_e = seed_entrant(&pool, event_id, Some(bob_id), 302, Some(2)).await;
    let charlie_e = seed_entrant(&pool, event_id, Some(charlie_id), 303, Some(3)).await;

    // Alice beats Bob and Charlie
    seed_set(&pool, event_id, alice_e, bob_e, 401).await;
    seed_set(&pool, event_id, alice_e, charlie_e, 402).await;
    // Bob beats Charlie (should not appear in Alice's stats)
    seed_set(&pool, event_id, bob_e, charlie_e, 403).await;

    let resp = get_req(&app, &format!("/projects/{pid}/stats/{alice_id}"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = read_json(resp).await;
    assert_eq!(body["player_id"], alice_id.to_string());
    assert_eq!(body["name"], "Alice");
    assert_eq!(body["wins"].as_array().unwrap().len(), 2);
    assert_eq!(body["losses"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn player_stats_returns_404_for_unknown_player(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice_ps2", "password123").await;
    let pid = create_project(&app, &cookie).await;
    let fake_id = Uuid::new_v4();

    let resp = get_req(&app, &format!("/projects/{pid}/stats/{fake_id}"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

```bash
cd backend && cargo test -p api -- player_stats 2>&1 | tail -20
```

Expected: compilation error (`get_player_stats` not defined yet).

- [ ] **Step 3: Add `get_player_stats` handler to `tournaments.rs`**

In `backend/crates/api/src/routes/tournaments.rs`, add after the `get_stats` function:

```rust
pub async fn get_player_stats(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path((project_id, player_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse> {
    require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;

    let name: Option<String> = sqlx::query_scalar!(
        "SELECT name FROM players WHERE id = $1 AND project_id = $2",
        player_id,
        project_id,
    )
    .fetch_optional(&state.db)
    .await?;
    let name = name.ok_or(AppError::NotFound)?;

    struct SetRow {
        winner_player_id: Option<Uuid>,
        winner_name: String,
        winner_seed: Option<i32>,
        winner_entrant_id: Uuid,
        loser_player_id: Option<Uuid>,
        loser_name: String,
        loser_seed: Option<i32>,
        loser_entrant_id: Uuid,
        winner_score: Option<i16>,
        loser_score: Option<i16>,
        event_name: String,
        tournament_name: String,
        tournament_handle: String,
        round_name: Option<String>,
        completed_at: Option<DateTime<Utc>>,
        is_dq: bool,
        vod_url: Option<String>,
        startgg_set_id: i64,
        phase_name: Option<String>,
        pool_identifier: Option<String>,
        winner_placement: Option<i32>,
        loser_placement: Option<i32>,
        num_entrants: Option<i32>,
        online: bool,
        city: Option<String>,
        addr_state: Option<String>,
        country_code: Option<String>,
        event_handle: Option<String>,
    }

    let sets = sqlx::query_as!(
        SetRow,
        r#"
        SELECT
            we.player_id                       AS "winner_player_id?: Uuid",
            COALESCE(wp.name, we.display_name) AS "winner_name!",
            we.seed                            AS winner_seed,
            we.id                              AS winner_entrant_id,
            le.player_id                       AS "loser_player_id?: Uuid",
            COALESCE(lp.name, le.display_name) AS "loser_name!",
            le.seed                            AS loser_seed,
            le.id                              AS loser_entrant_id,
            s.winner_score,
            s.loser_score,
            e.name                             AS event_name,
            t.name                             AS tournament_name,
            t.handle                           AS tournament_handle,
            s.round_name,
            s.completed_at,
            s.is_dq,
            s.vod_url,
            s.startgg_set_id,
            ph.name                            AS "phase_name?: String",
            pg.display_identifier              AS "pool_identifier?: String",
            we.final_placement                 AS winner_placement,
            le.final_placement                 AS loser_placement,
            e.num_entrants,
            t.online,
            t.city,
            t.addr_state,
            t.country_code,
            e.handle                           AS "event_handle?: String"
        FROM sets s
        JOIN entrants we ON we.id = s.winner_entrant_id
        JOIN entrants le ON le.id = s.loser_entrant_id
        LEFT JOIN players wp ON wp.id = we.player_id AND wp.project_id = $1
        LEFT JOIN players lp ON lp.id = le.player_id AND lp.project_id = $1
        JOIN project_events pe ON pe.event_id = s.event_id AND pe.project_id = $1
        JOIN events e ON e.id = s.event_id
        JOIN tournaments t ON t.id = e.tournament_id
        LEFT JOIN phase_groups pg ON pg.id = s.phase_group_id
        LEFT JOIN phases ph ON ph.id = pg.phase_id
        WHERE pe.included = true
          AND s.is_dq    = false
          AND s.has_placeholder = false
          AND (wp.id IS NOT NULL OR lp.id IS NOT NULL)
          AND (we.player_id = $2 OR le.player_id = $2)
        "#,
        project_id,
        player_id,
    )
    .fetch_all(&state.db)
    .await?;

    let mut wins: Vec<SetRecord> = Vec::new();
    let mut losses: Vec<SetRecord> = Vec::new();

    for row in sets {
        let uf = match (row.winner_seed, row.loser_seed) {
            (Some(ws), Some(ls)) => set_upset_factor(ws, ls) as i64,
            _ => 0,
        };
        let location = compute_location(
            row.online,
            row.city.as_deref(),
            row.addr_state.as_deref(),
            row.country_code.as_deref(),
        );
        let loser_opp_id = row.loser_player_id.unwrap_or(row.loser_entrant_id);
        let winner_opp_id = row.winner_player_id.unwrap_or(row.winner_entrant_id);

        if row.winner_player_id == Some(player_id) {
            wins.push(SetRecord {
                opponent_id: loser_opp_id,
                opponent_name: row.loser_name,
                upset_factor: uf,
                winner_score: row.winner_score,
                loser_score: row.loser_score,
                tournament_name: row.tournament_name.clone(),
                tournament_handle: row.tournament_handle.clone(),
                event_name: row.event_name.clone(),
                round_name: row.round_name.clone(),
                completed_at: row.completed_at,
                is_dq: row.is_dq,
                vod_url: row.vod_url.clone(),
                startgg_set_id: row.startgg_set_id,
                winner_seed: row.winner_seed,
                loser_seed: row.loser_seed,
                phase_name: row.phase_name.clone(),
                pool_identifier: row.pool_identifier.clone(),
                winner_placement: row.winner_placement,
                loser_placement: row.loser_placement,
                location: location.clone(),
                num_entrants: row.num_entrants,
                event_handle: row.event_handle.clone(),
            });
        }
        if row.loser_player_id == Some(player_id) {
            losses.push(SetRecord {
                opponent_id: winner_opp_id,
                opponent_name: row.winner_name,
                upset_factor: uf,
                winner_score: row.winner_score,
                loser_score: row.loser_score,
                tournament_name: row.tournament_name,
                tournament_handle: row.tournament_handle,
                event_name: row.event_name,
                round_name: row.round_name,
                completed_at: row.completed_at,
                is_dq: row.is_dq,
                vod_url: row.vod_url,
                startgg_set_id: row.startgg_set_id,
                winner_seed: row.winner_seed,
                loser_seed: row.loser_seed,
                phase_name: row.phase_name,
                pool_identifier: row.pool_identifier,
                winner_placement: row.winner_placement,
                loser_placement: row.loser_placement,
                location,
                num_entrants: row.num_entrants,
                event_handle: row.event_handle,
            });
        }
    }

    wins.sort_by(|a, b| b.upset_factor.cmp(&a.upset_factor));
    losses.sort_by(|a, b| b.upset_factor.cmp(&a.upset_factor));

    Ok(Json(PlayerStatsResponse {
        player_id,
        name,
        wins,
        losses,
    }))
}
```

- [ ] **Step 4: Register the route in `projects.rs`**

In `backend/crates/api/src/routes/projects.rs`, add the new route after the existing stats route:

```rust
.route("/{id}/stats", get(t::get_stats))
.route("/{id}/stats/{player_id}", get(t::get_player_stats))  // add this line
.route("/{id}/head-to-head", get(t::get_head_to_head))
```

- [ ] **Step 5: Run the tests to confirm they pass**

```bash
cd backend && cargo test -p api -- player_stats 2>&1 | tail -20
```

Expected: both `player_stats_returns_single_player_data` and `player_stats_returns_404_for_unknown_player` pass.

- [ ] **Step 6: Commit**

```bash
git add backend/crates/api/src/routes/tournaments.rs \
        backend/crates/api/src/routes/projects.rs \
        backend/crates/api/tests/api.rs
git commit -m "feat: add GET /projects/{id}/stats/{player_id} endpoint"
```

---

## Task 2: Backend — `get_player_tournaments` endpoint (TDD)

**Files:**
- Modify: `backend/crates/api/tests/api.rs`
- Modify: `backend/crates/api/src/routes/tournaments.rs`
- Modify: `backend/crates/api/src/routes/players.rs`

- [ ] **Step 1: Write the failing test**

Append to `backend/crates/api/tests/api.rs`:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn player_tournaments_returns_attendance_history(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice_pt", "password123").await;
    let pid_str = create_project(&app, &cookie).await;
    let pid = Uuid::parse_str(&pid_str).unwrap();

    let alice_id = Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Alice").await).unwrap();

    // Tournament 1: included event, Alice placed 2nd
    let t1_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO tournaments (startgg_id, name, handle, online, city, addr_state)
         VALUES (9101, 'Genesis 9', 'tournament/genesis-9', false, 'San Jose', 'CA')
         RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let e1_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO events (tournament_id, startgg_id, name, num_entrants, handle)
         VALUES ($1, 8101, 'Melee Singles', 486, 'melee-singles')
         RETURNING id", t1_id
    ).fetch_one(&pool).await.unwrap();

    sqlx::query!(
        "INSERT INTO project_events (project_id, event_id, included) VALUES ($1, $2, true)",
        pid, e1_id
    ).execute(&pool).await.unwrap();

    sqlx::query!(
        "INSERT INTO entrants (event_id, player_id, startgg_entrant_id, display_name, final_placement)
         VALUES ($1, $2, 5001, 'Alice', 2)",
        e1_id, alice_id
    ).execute(&pool).await.unwrap();

    // Tournament 2: NOT linked to project, Alice attended — should still appear
    let t2_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO tournaments (startgg_id, name, handle, online)
         VALUES (9102, 'CEO 2024', 'tournament/ceo-2024', false)
         RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let e2_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO events (tournament_id, startgg_id, name, handle)
         VALUES ($1, 8102, 'Melee Singles', 'melee-singles-2')
         RETURNING id", t2_id
    ).fetch_one(&pool).await.unwrap();

    sqlx::query!(
        "INSERT INTO entrants (event_id, player_id, startgg_entrant_id, display_name)
         VALUES ($1, $2, 5002, 'Alice')",
        e2_id, alice_id
    ).execute(&pool).await.unwrap();

    let resp = get_req(
        &app,
        &format!("/projects/{pid}/players/{alice_id}/tournaments"),
        &cookie,
    ).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = read_json(resp).await;
    let entries = body.as_array().unwrap();
    assert_eq!(entries.len(), 2, "should include both in-project and out-of-project events");

    let genesis = entries.iter().find(|e| e["tournament_name"] == "Genesis 9").unwrap();
    assert_eq!(genesis["placement"], 2);
    assert_eq!(genesis["num_entrants"], 486);
    assert_eq!(genesis["location"], "San Jose, CA");
    assert_eq!(genesis["event_name"], "Melee Singles");
}

#[sqlx::test(migrations = "../../migrations")]
async fn player_tournaments_returns_404_for_unknown_player(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice_pt2", "password123").await;
    let pid = create_project(&app, &cookie).await;
    let fake_id = Uuid::new_v4();

    let resp = get_req(
        &app,
        &format!("/projects/{pid}/players/{fake_id}/tournaments"),
        &cookie,
    ).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Run to confirm failure**

```bash
cd backend && cargo test -p api -- player_tournaments 2>&1 | tail -20
```

Expected: compilation error — `get_player_tournaments` not defined yet.

- [ ] **Step 3: Add `TournamentAttendance` struct and `get_player_tournaments` handler to `tournaments.rs`**

In `backend/crates/api/src/routes/tournaments.rs`, add after the `get_stats` function (and after `get_player_stats`):

```rust
#[derive(Serialize)]
pub struct TournamentAttendance {
    pub tournament_name: String,
    pub tournament_slug: String,
    pub event_name: String,
    pub placement: Option<i32>,
    pub num_entrants: Option<i32>,
    pub start_at: Option<DateTime<Utc>>,
    pub location: Option<String>,
}

pub async fn get_player_tournaments(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path((project_id, player_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse> {
    require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;

    let exists: Option<bool> = sqlx::query_scalar!(
        "SELECT true FROM players WHERE id = $1 AND project_id = $2",
        player_id,
        project_id,
    )
    .fetch_optional(&state.db)
    .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    struct Row {
        tournament_name: String,
        tournament_slug: String,
        event_name: String,
        placement: Option<i32>,
        num_entrants: Option<i32>,
        start_at: Option<DateTime<Utc>>,
        online: bool,
        city: Option<String>,
        addr_state: Option<String>,
        country_code: Option<String>,
    }

    let rows = sqlx::query_as!(
        Row,
        r#"
        SELECT
            t.name              AS tournament_name,
            t.handle            AS tournament_slug,
            e.name              AS event_name,
            ent.final_placement AS "placement?: i32",
            e.num_entrants      AS "num_entrants?: i32",
            t.start_at,
            t.online,
            t.city,
            t.addr_state,
            t.country_code
        FROM entrants ent
        JOIN players pl ON pl.id = ent.player_id AND pl.project_id = $1
        JOIN events e ON e.id = ent.event_id
        JOIN tournaments t ON t.id = e.tournament_id
        WHERE ent.player_id = $2
        ORDER BY t.start_at DESC NULLS LAST
        "#,
        project_id,
        player_id,
    )
    .fetch_all(&state.db)
    .await?;

    let resp: Vec<TournamentAttendance> = rows
        .into_iter()
        .map(|r| TournamentAttendance {
            tournament_name: r.tournament_name,
            tournament_slug: r.tournament_slug,
            event_name: r.event_name,
            placement: r.placement,
            num_entrants: r.num_entrants,
            start_at: r.start_at,
            location: compute_location(
                r.online,
                r.city.as_deref(),
                r.addr_state.as_deref(),
                r.country_code.as_deref(),
            ),
        })
        .collect();

    Ok(Json(resp))
}
```

- [ ] **Step 4: Register the route in the players router in `players.rs`**

In `backend/crates/api/src/routes/players.rs`, add the import and route. First add the import at the top of the file:

```rust
use crate::routes::tournaments::get_player_tournaments;
```

Then in the `router()` function, add the new route:

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_players).post(add_player))
        .route("/bulk", post(bulk_add_players))
        .route("/by-handles", post(add_players_by_handles))
        .route("/{pid}", delete(delete_player).patch(rename_player))
        .route("/{pid}/accounts", post(link_account))
        .route("/{pid}/accounts/{aid}", delete(unlink_account))
        .route("/{pid}/tournaments", get(get_player_tournaments))  // add this line
}
```

- [ ] **Step 5: Run the tests to confirm they pass**

```bash
cd backend && cargo test -p api -- player_tournaments 2>&1 | tail -20
```

Expected: both tournament tests pass.

- [ ] **Step 6: Update the sqlx offline cache**

```bash
cd backend && bash prepare-sqlx.sh
```

This runs migrations + `cargo sqlx prepare` against a fresh container. Commit the resulting changes to `.sqlx/`.

- [ ] **Step 7: Commit**

```bash
git add backend/crates/api/src/routes/tournaments.rs \
        backend/crates/api/src/routes/players.rs \
        backend/crates/api/tests/api.rs \
        backend/.sqlx/
git commit -m "feat: add GET /projects/{id}/players/{player_id}/tournaments endpoint"
```

---

## Task 3: OpenAPI spec update

**Files:**
- Modify: `backend/openapi.yaml`

- [ ] **Step 1: Add `TournamentAttendance` schema**

In `backend/openapi.yaml`, add `TournamentAttendance` to the `components/schemas` section (after the `PlayerStats` schema):

```yaml
    TournamentAttendance:
      type: object
      required: [tournament_name, tournament_slug, event_name]
      properties:
        tournament_name:
          type: string
        tournament_slug:
          type: string
        event_name:
          type: string
        placement:
          type: integer
          nullable: true
        num_entrants:
          type: integer
          nullable: true
        start_at:
          type: string
          format: date-time
          nullable: true
        location:
          type: string
          nullable: true
```

- [ ] **Step 2: Add the two new endpoint definitions**

In `backend/openapi.yaml`, add after the existing `/projects/{project_id}/stats` block:

```yaml
  /projects/{project_id}/stats/{player_id}:
    parameters:
      - name: project_id
        in: path
        required: true
        schema:
          type: string
          format: uuid
      - name: player_id
        in: path
        required: true
        schema:
          type: string
          format: uuid

    get:
      summary: Get stats for a single player
      description: >
        Same semantics as GET /projects/{project_id}/stats but scoped to one player.
        Returns a single PlayerStats object rather than an array.
      tags: [Stats]
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/PlayerStats'
        '401':
          $ref: '#/components/responses/Unauthorized'
        '404':
          $ref: '#/components/responses/NotFound'
```

And add after the `/projects/{project_id}/players/{player_id}/accounts` block:

```yaml
  /projects/{project_id}/players/{player_id}/tournaments:
    parameters:
      - name: project_id
        in: path
        required: true
        schema:
          type: string
          format: uuid
      - name: player_id
        in: path
        required: true
        schema:
          type: string
          format: uuid

    get:
      summary: Get all tournament events a player has attended
      description: >
        Returns every event the player entered (via linked start.gg accounts),
        not restricted to included events or events with tracked opponents.
        Sorted by tournament start date descending.
      tags: [Players]
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/TournamentAttendance'
        '401':
          $ref: '#/components/responses/Unauthorized'
        '404':
          $ref: '#/components/responses/NotFound'
```

- [ ] **Step 3: Commit**

```bash
git add backend/openapi.yaml
git commit -m "docs: add player stats and tournament attendance endpoints to OpenAPI spec"
```

---

## Task 4: Frontend — `TournamentAttendance` type + player page route

**Files:**
- Modify: `web/src/lib/types.ts`
- Create: `web/src/routes/projects/[id]/players/[player_id]/+page.server.ts`
- Create: `web/src/routes/projects/[id]/players/[player_id]/+page.svelte`

- [ ] **Step 1: Add `TournamentAttendance` to `types.ts`**

In `web/src/lib/types.ts`, append:

```typescript
export interface TournamentAttendance {
  tournament_name: string;
  tournament_slug: string;
  event_name: string;
  placement: number | null;
  num_entrants: number | null;
  start_at: string | null;
  location: string | null;
}
```

- [ ] **Step 2: Create the page server load**

Create `web/src/routes/projects/[id]/players/[player_id]/+page.server.ts`:

```typescript
import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { PlayerStats, TournamentAttendance } from '$lib/types';
import { INTERNAL_API_URL } from '$env/static/private';

export const load: PageServerLoad = async ({ fetch, params, cookies }) => {
  const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
  const [statsRes, tournamentsRes] = await Promise.all([
    api.get(`/projects/${params.id}/stats/${params.player_id}`),
    api.get(`/projects/${params.id}/players/${params.player_id}/tournaments`),
  ]);
  if (!statsRes.ok) {
    throw error(statsRes.status === 404 ? 404 : 500, 'Player not found');
  }
  const stats: PlayerStats = await statsRes.json();
  const tournaments: TournamentAttendance[] = tournamentsRes.ok
    ? await tournamentsRes.json()
    : [];
  return { stats, tournaments };
};
```

- [ ] **Step 3: Create the player detail page**

Create `web/src/routes/projects/[id]/players/[player_id]/+page.svelte`:

```svelte
<script lang="ts">
  import type { SetRecord } from '$lib/types';
  import SetDetailModal from '$lib/components/SetDetailModal.svelte';
  import * as Card from '$lib/components/ui/card';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import { Button } from '$lib/components/ui/button';
  import * as Table from '$lib/components/ui/table';
  import * as Empty from '$lib/components/ui/empty';
  import { winRate, toOrdinal, formatDate } from '$lib/utils';

  let { data } = $props();

  let selectedSet = $state<SetRecord | null>(null);
  let selectedIsWin = $state(false);

  function openModal(set: SetRecord, isWin: boolean) {
    selectedSet = set;
    selectedIsWin = isWin;
  }
</script>

<div class="space-y-6">
  <div>
    <Button
      variant="link"
      class="h-auto p-0 text-sm text-muted-foreground"
      onclick={() => history.back()}
    >← Back</Button>
    <h2 class="mt-1 text-xl font-bold">{data.stats.name}</h2>
    <p class="mt-1 text-sm text-muted-foreground">
      {data.stats.wins.length} W · {data.stats.losses.length} L ·
      {winRate(data.stats.wins.length, data.stats.losses.length, '0%')} win rate ·
      {data.tournaments.length} tournament{data.tournaments.length === 1 ? '' : 's'}
    </p>
  </div>

  <div class="grid gap-3 sm:grid-cols-2">
    <Card.Root class="py-0">
      <Card.Content class="p-3">
        <p class="mb-1 text-xs font-semibold uppercase tracking-wide text-green-600 dark:text-green-400">
          Wins ({data.stats.wins.length})
        </p>
        {#if data.stats.wins.length === 0}
          <p class="text-xs text-muted-foreground">No wins yet.</p>
        {:else}
          <ScrollArea class="h-48 rounded border border-border bg-muted/20">
            {#each data.stats.wins as set, i (i)}
              <Button
                variant="ghost"
                class="h-auto w-full border-b border-border px-2 py-1 text-xs last:border-0 justify-start whitespace-normal"
                onclick={() => openModal(set, true)}
              >
                {set.opponent_name} · UF {set.upset_factor} · {set.tournament_name}
              </Button>
            {/each}
          </ScrollArea>
        {/if}
      </Card.Content>
    </Card.Root>

    <Card.Root class="py-0">
      <Card.Content class="p-3">
        <p class="mb-1 text-xs font-semibold uppercase tracking-wide text-red-600 dark:text-red-400">
          Losses ({data.stats.losses.length})
        </p>
        {#if data.stats.losses.length === 0}
          <p class="text-xs text-muted-foreground">No losses yet.</p>
        {:else}
          <ScrollArea class="h-48 rounded border border-border bg-muted/20">
            {#each data.stats.losses as set, i (i)}
              <Button
                variant="ghost"
                class="h-auto w-full border-b border-border px-2 py-1 text-xs last:border-0 justify-start whitespace-normal"
                onclick={() => openModal(set, false)}
              >
                {set.opponent_name} · UF {set.upset_factor} · {set.tournament_name}
              </Button>
            {/each}
          </ScrollArea>
        {/if}
      </Card.Content>
    </Card.Root>
  </div>

  <div>
    <h3 class="mb-3 text-sm font-semibold">
      Tournament history ({data.tournaments.length})
    </h3>
    {#if data.tournaments.length === 0}
      <Empty.Root>
        <Empty.Header>
          <Empty.Title>No tournament history</Empty.Title>
          <Empty.Description>
            Import tournaments to populate this player's attendance.
          </Empty.Description>
        </Empty.Header>
      </Empty.Root>
    {:else}
      <Table.Root>
        <Table.Header>
          <Table.Row>
            <Table.Head>Tournament · Event</Table.Head>
            <Table.Head class="text-right">Placement</Table.Head>
            <Table.Head class="text-right">Entrants</Table.Head>
            <Table.Head class="text-right">Date</Table.Head>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#each data.tournaments as t, i (i)}
            <Table.Row>
              <Table.Cell>
                <span class="font-medium">{t.tournament_name}</span>
                <span class="text-muted-foreground"> · {t.event_name}</span>
              </Table.Cell>
              <Table.Cell class="text-right tabular-nums">
                {#if t.placement !== null}
                  <span
                    class={t.placement <= 3
                      ? 'font-semibold text-green-600 dark:text-green-400'
                      : ''}
                  >{toOrdinal(t.placement)}</span>
                {:else}
                  <span class="text-muted-foreground">—</span>
                {/if}
              </Table.Cell>
              <Table.Cell class="text-right tabular-nums text-muted-foreground">
                {t.num_entrants ?? '—'}
              </Table.Cell>
              <Table.Cell class="text-right text-muted-foreground">
                {formatDate(t.start_at)}
              </Table.Cell>
            </Table.Row>
          {/each}
        </Table.Body>
      </Table.Root>
    {/if}
  </div>
</div>

<SetDetailModal
  set={selectedSet}
  isWin={selectedIsWin}
  currentPlayerName={data.stats.name}
  onClose={() => (selectedSet = null)}
/>
```

- [ ] **Step 4: Run frontend unit tests to confirm nothing broke**

```bash
cd web && npm run test:unit
```

Expected: all pass (no new unit tests needed for this step — the page is tested via e2e in Task 6).

- [ ] **Step 5: Commit**

```bash
git add web/src/lib/types.ts \
        web/src/routes/projects/[id]/players/[player_id]/
git commit -m "feat: add player detail page route"
```

---

## Task 5: Frontend — wire navigation entry points + SetDetailModal opponent link

**Files:**
- Modify: `web/src/lib/components/SetDetailModal.svelte`
- Modify: `web/src/routes/projects/[id]/stats/+page.svelte`
- Modify: `web/src/lib/components/PlayerCard.svelte`
- Modify: `web/src/routes/projects/[id]/players/+page.svelte`
- Modify: `web/src/routes/projects/[id]/h2h/+page.svelte`

- [ ] **Step 1: Add optional opponent link props to `SetDetailModal.svelte`**

In `web/src/lib/components/SetDetailModal.svelte`, update the Props interface and title:

```svelte
<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import type { SetRecord } from '$lib/types';
  import { formatDate, toOrdinal } from '$lib/utils';

  interface Props {
    set: SetRecord | null;
    isWin: boolean;
    currentPlayerName: string;
    onClose: () => void;
    projectId?: string;
    opponentPlayerId?: string;
  }

  let { set, isWin, currentPlayerName, onClose, projectId, opponentPlayerId }: Props = $props();

  let open = $derived(set !== null);

  function phaseLabel(set: SetRecord): string | null {
    if (!set.phase_name) return null;
    return set.pool_identifier
      ? `${set.phase_name} · ${set.pool_identifier}`
      : set.phase_name;
  }
</script>
```

Then update the dialog title to link the opponent name when `opponentPlayerId` is provided. Replace:

```svelte
<Dialog.Title>{currentPlayerName} vs {set.opponent_name}</Dialog.Title>
```

with:

```svelte
<Dialog.Title>
  {currentPlayerName} vs
  {#if projectId && opponentPlayerId}
    <a
      href="/projects/{projectId}/players/{opponentPlayerId}"
      class="hover:underline"
      onclick={() => onClose()}
    >{set.opponent_name}</a>
  {:else}
    {set.opponent_name}
  {/if}
</Dialog.Title>
```

- [ ] **Step 2: Update the stats page — player name links + modal opponent props**

In `web/src/routes/projects/[id]/stats/+page.svelte`, make the following changes:

1. Build a set of tracked player IDs from the stats data.
2. Wrap each player name in a link.
3. Pass `projectId` and `opponentPlayerId` to `SetDetailModal`.

Full updated script block:

```svelte
<script lang="ts">
  import type { SetRecord } from '$lib/types';
  import SetDetailModal from '$lib/components/SetDetailModal.svelte';
  import * as Card from '$lib/components/ui/card';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import * as Empty from '$lib/components/ui/empty';
  import { Button } from '$lib/components/ui/button';
  import { winRate } from '$lib/utils';

  let { data } = $props();

  let selectedSet = $state<SetRecord | null>(null);
  let selectedIsWin = $state(false);
  let selectedPlayerName = $state('');

  const trackedPlayerIds = $derived(new Set(data.stats.map((p) => p.player_id)));

  function openModal(set: SetRecord, isWin: boolean, playerName: string) {
    selectedSet = set;
    selectedIsWin = isWin;
    selectedPlayerName = playerName;
  }
</script>
```

In the template, replace the player name `<span>` with a link:

```svelte
<!-- Replace: -->
<span class="font-semibold">{player.name}</span>
<!-- With: -->
<a
  href="/projects/{data.project.id}/players/{player.player_id}"
  class="font-semibold hover:underline"
>{player.name}</a>
```

Update the `SetDetailModal` call at the bottom to pass opponent link props:

```svelte
<SetDetailModal
  set={selectedSet}
  isWin={selectedIsWin}
  currentPlayerName={selectedPlayerName}
  onClose={() => (selectedSet = null)}
  projectId={data.project.id}
  opponentPlayerId={selectedSet && trackedPlayerIds.has(selectedSet.opponent_id)
    ? selectedSet.opponent_id
    : undefined}
/>
```

- [ ] **Step 3: Update `PlayerCard.svelte` — add `projectId` prop + name link**

In `web/src/lib/components/PlayerCard.svelte`, add `projectId` to the Props interface and wrap the player name:

```svelte
let { player, isEditing, form, onEdit, onCancelEdit, onOpenLinkDialog, projectId }: {
  player: Player;
  isEditing: boolean;
  form: { renameError?: string; renamePid?: string } | null;
  onEdit: () => void;
  onCancelEdit: () => void;
  onOpenLinkDialog: () => void;
  projectId: string;
} = $props();
```

In the template, replace:

```svelte
<p class="font-medium">{player.name}</p>
```

with:

```svelte
<a
  href="/projects/{projectId}/players/{player.id}"
  class="font-medium hover:underline"
>{player.name}</a>
```

- [ ] **Step 4: Pass `projectId` to `PlayerCard` in the players page**

In `web/src/routes/projects/[id]/players/+page.svelte`, update the `PlayerCard` usage:

```svelte
{#each data.players as player (player.id)}
  <PlayerCard
    {player}
    projectId={data.project.id}
    isEditing={editingPid === player.id}
    {form}
    onEdit={() => startEdit(player.id)}
    onCancelEdit={cancelEdit}
    onOpenLinkDialog={() => openLinkDialog(player.id, player.name)}
  />
{/each}
```

- [ ] **Step 5: Update the H2H page — row/col labels + modal opponent props**

In `web/src/routes/projects/[id]/h2h/+page.svelte`:

1. Wrap row and column player name labels in links.
2. Pass `projectId` and `opponentPlayerId` to `SetDetailModal`.

Replace the row label `<span>`:

```svelte
<!-- Replace: -->
<span {...props} class="block max-w-[8rem] truncate">{row.name}</span>
<!-- With: -->
<a
  {...props}
  href="/projects/{data.project.id}/players/{row.id}"
  class="block max-w-[8rem] truncate hover:underline"
>{row.name}</a>
```

Replace the column label `<span>`:

```svelte
<!-- Replace: -->
<span {...props} class="block max-w-[5rem] truncate">{col.name}</span>
<!-- With: -->
<a
  {...props}
  href="/projects/{data.project.id}/players/{col.id}"
  class="block max-w-[5rem] truncate hover:underline"
>{col.name}</a>
```

Update the `SetDetailModal` call (near the bottom of the file):

```svelte
<SetDetailModal
  set={selectedSet}
  isWin={selectedIsWin}
  currentPlayerName={selectedPair?.rowPlayer.name ?? ''}
  onClose={() => (selectedSet = null)}
  projectId={data.project.id}
  opponentPlayerId={selectedPair?.colPlayer.id}
/>
```

- [ ] **Step 6: Run frontend unit tests**

```bash
cd web && npm run test:unit
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add web/src/lib/components/SetDetailModal.svelte \
        web/src/routes/projects/[id]/stats/+page.svelte \
        web/src/lib/components/PlayerCard.svelte \
        web/src/routes/projects/[id]/players/+page.svelte \
        web/src/routes/projects/[id]/h2h/+page.svelte
git commit -m "feat: link player names throughout UI to player detail page"
```

---

## Task 6: Mock API handlers + Playwright e2e tests

**Files:**
- Modify: `web/tests/mock-api.js`
- Modify: `web/tests/projects.test.ts` (or create a new `player.test.ts`)

- [ ] **Step 1: Add mock API data to `mock-api.js`**

In `web/tests/mock-api.js`, add mock data constants near the top (after the existing `MOCK_STATS` definition):

```javascript
const MOCK_PLAYER_STATS = {
  player_id: 'player-1',
  name: 'Alice',
  wins: [{ ...MOCK_SET_BASE, opponent_id: 'player-2', opponent_name: 'Bob', upset_factor: 3 }],
  losses: [{ ...MOCK_SET_BASE, opponent_id: 'player-3', opponent_name: 'Charlie', upset_factor: 1 }],
};

const MOCK_PLAYER_TOURNAMENTS = [
  {
    tournament_name: 'Genesis 9',
    tournament_slug: 'tournament/genesis-9',
    event_name: 'Melee Singles',
    placement: 1,
    num_entrants: 486,
    start_at: '2024-01-12T00:00:00Z',
    location: 'San Jose, CA',
  },
  {
    tournament_name: 'CEO 2024',
    tournament_slug: 'tournament/ceo-2024',
    event_name: 'Melee Singles',
    placement: 5,
    num_entrants: 312,
    start_at: '2024-06-14T00:00:00Z',
    location: 'Kissimmee, FL',
  },
];
```

Then add handlers in the request router (before the final 404 fallback):

```javascript
const playerStatsMatch = path.match(/^\/projects\/([^/]+)\/stats\/([^/]+)$/);
if (playerStatsMatch && req.method === 'GET') {
  respond(res, 200, MOCK_PLAYER_STATS);
  return;
}

const playerTournamentsMatch = path.match(/^\/projects\/([^/]+)\/players\/([^/]+)\/tournaments$/);
if (playerTournamentsMatch && req.method === 'GET') {
  respond(res, 200, MOCK_PLAYER_TOURNAMENTS);
  return;
}
```

> **Note:** The `playerTournamentsMatch` regex must be placed **before** `playerPatchMatch` (which matches `\/projects\/([^/]+)\/players\/([^/]+)$`) to avoid the more general pattern consuming it first. Verify the ordering after making this change.

- [ ] **Step 2: Write the Playwright e2e test**

Create `web/tests/player.test.ts`:

```typescript
import { test, expect } from '@playwright/test';

test('player detail page shows stats and tournament history', async ({ page }) => {
  await page.goto('/projects/proj-1/players/player-1');

  // Player name and summary
  await expect(page.getByText('Alice')).toBeVisible();
  await expect(page.getByText(/1 W/)).toBeVisible();
  await expect(page.getByText(/1 L/)).toBeVisible();

  // Wins section
  await expect(page.getByText(/Wins \(1\)/i)).toBeVisible();
  await expect(page.getByText(/Bob/)).toBeVisible();

  // Losses section
  await expect(page.getByText(/Losses \(1\)/i)).toBeVisible();
  await expect(page.getByText(/Charlie/)).toBeVisible();

  // Tournament history table
  await expect(page.getByText('Tournament history (2)')).toBeVisible();
  await expect(page.getByText('Genesis 9')).toBeVisible();
  await expect(page.getByText('1st')).toBeVisible();
  await expect(page.getByText('CEO 2024')).toBeVisible();
});

test('stats page player name links to detail page', async ({ page }) => {
  await page.goto('/projects/proj-1/stats');
  await page.getByRole('link', { name: 'Alice' }).first().click();
  await expect(page).toHaveURL(/\/projects\/proj-1\/players\/player-1/);
});
```

- [ ] **Step 3: Run all frontend tests**

```bash
cd web && npm run test:e2e
```

Expected: all e2e tests pass including the two new player tests.

- [ ] **Step 4: Run the full test suite**

```bash
bash test.sh
```

Expected: PASS for all sections.

- [ ] **Step 5: Commit**

```bash
git add web/tests/mock-api.js web/tests/player.test.ts
git commit -m "test: add mock API handlers and e2e tests for player detail page"
```
