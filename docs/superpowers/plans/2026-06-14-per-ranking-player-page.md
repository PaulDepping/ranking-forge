# Per-Ranking Player Detail Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the broken project-level player detail page with a ranking-scoped one at `/projects/[id]/rankings/[rid]/players/[player_id]`.

**Architecture:** Add one backend endpoint (`GET /projects/{id}/rankings/{rid}/players/{pid}/tournaments`) that filters tournament history to included events only; create a new SvelteKit page in the rankings subtree; update all six link sites; delete the old broken page.

**Tech Stack:** Rust/Axum (backend), SvelteKit 5 / TypeScript / Tailwind / shadcn-svelte (frontend), sqlx (query macros), PostgreSQL.

---

### Task 1: Backend — add `get_ranking_player_tournaments` handler

**Files:**
- Modify: `backend/crates/api/src/routes/tournaments.rs`

- [ ] **Step 1: Write the failing e2e test**

Add this test to the bottom of `backend/crates/e2e/tests/full_flow.rs`:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_ranking_player_tournaments(pool: PgPool) {
    let app = make_app(pool.clone(), "http://unused");
    let cookie = register(&app, "alice", "pass1234").await;

    let resp = post_json(&app, "/projects", &cookie, json!({"name": "Test"})).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let ranking_id = create_ranking(&app, &cookie, &project_id, "Season 1").await;

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "Mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let player_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &player_id).await;

    // Player in ranking → 200 with empty list (no import run)
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/players/{player_id}/tournaments"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(read_json(resp).await, json!([]));

    // Player exists in project but not in this ranking → 404
    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "Armada"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let other_id = read_json(resp).await["id"].as_str().unwrap().to_string();
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/players/{other_id}/tournaments"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Run the test to confirm it fails with 404 (route not yet registered)**

```bash
cd backend && DATABASE_URL=postgres://postgres:password@localhost:5432/rankingforge cargo test -p e2e -- test_ranking_player_tournaments --nocapture 2>&1 | tail -20
```

Expected: test fails — the endpoint returns 404.

- [ ] **Step 3: Add the handler in `tournaments.rs`**

Add after `get_player_tournaments` (around line 720, before the `// ── Router` section):

```rust
pub async fn get_ranking_player_tournaments(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingPlayerStatPath>,
) -> Result<impl IntoResponse> {
    require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;

    let exists: Option<Uuid> = sqlx::query_scalar!(
        "SELECT player_id FROM ranking_players WHERE ranking_id = $1 AND player_id = $2",
        path.rid,
        path.player_id,
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
        JOIN events e ON e.id = ent.event_id
        JOIN ranking_events re ON re.event_id = e.id
                               AND re.ranking_id = $2
                               AND re.included = true
        JOIN tournaments t ON t.id = e.tournament_id
        WHERE ent.player_id = $3
          AND ent.player_id IN (
              SELECT pl.id FROM players pl WHERE pl.project_id = $1
          )
        ORDER BY t.start_at DESC NULLS LAST
        "#,
        path.id,
        path.rid,
        path.player_id,
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

- [ ] **Step 4: Register the route in `tournaments::router()`**

In the `router()` function at the bottom of `tournaments.rs`, add the new route:

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tournaments", get(list_tournaments))
        .route("/events", axum::routing::put(put_events))
        .route("/stats", get(get_stats))
        .route("/stats/{player_id}", get(get_player_stats))
        .route("/players/{player_id}/tournaments", get(get_ranking_player_tournaments))
        .route("/head-to-head", get(get_head_to_head))
        .route("/head-to-head/{pid_a}/{pid_b}/sets", get(get_h2h_sets))
}
```

- [ ] **Step 5: Update the sqlx offline cache**

```bash
cd backend && bash prepare-sqlx.sh
```

Expected: exits 0, updates files under `.sqlx/`.

- [ ] **Step 6: Run the test to confirm it passes**

```bash
cd backend && DATABASE_URL=postgres://postgres:password@localhost:5432/rankingforge cargo test -p e2e -- test_ranking_player_tournaments --nocapture 2>&1 | tail -20
```

Expected: `test test_ranking_player_tournaments ... ok`

- [ ] **Step 7: Commit**

```bash
git add backend/crates/api/src/routes/tournaments.rs backend/crates/e2e/tests/full_flow.rs backend/.sqlx
git commit -m "feat(api): add ranking-scoped player tournament history endpoint"
```

---

### Task 2: Frontend — update `SetDetailModal` to accept `rankingId`

**Files:**
- Modify: `web/src/lib/components/SetDetailModal.svelte`

- [ ] **Step 1: Add `rankingId` prop and update the opponent link**

Replace the `interface Props` block and the opponent link. The current Props block is:

```typescript
interface Props {
  set: SetRecord | null;
  isWin: boolean;
  currentPlayerName: string;
  onClose: () => void;
  projectId?: string;
  opponentPlayerId?: string;
}

let {
  set,
  isWin,
  currentPlayerName,
  onClose,
  projectId,
  opponentPlayerId,
}: Props = $props();
```

Replace with:

```typescript
interface Props {
  set: SetRecord | null;
  isWin: boolean;
  currentPlayerName: string;
  onClose: () => void;
  projectId?: string;
  rankingId?: string;
  opponentPlayerId?: string;
}

let {
  set,
  isWin,
  currentPlayerName,
  onClose,
  projectId,
  rankingId,
  opponentPlayerId,
}: Props = $props();
```

Then update the opponent link (currently around line 45-49):

```svelte
{currentPlayerName} vs {#if projectId && rankingId && opponentPlayerId}<a
    href="/projects/{projectId}/rankings/{rankingId}/players/{opponentPlayerId}"
    class="hover:underline"
    onclick={() => onClose()}>{set.opponent_name}</a
  >{:else}{set.opponent_name}{/if}
```

- [ ] **Step 2: Type-check**

```bash
cd web && npm run check 2>&1 | tail -20
```

Expected: no errors or warnings.

- [ ] **Step 3: Commit**

```bash
git add web/src/lib/components/SetDetailModal.svelte
git commit -m "feat(frontend): add rankingId prop to SetDetailModal for ranking-scoped opponent links"
```

---

### Task 3: Frontend — create the new ranking-scoped player detail page

**Files:**
- Create: `web/src/routes/projects/[id]/rankings/[rid]/players/[player_id]/+page.server.ts`
- Create: `web/src/routes/projects/[id]/rankings/[rid]/players/[player_id]/+page.svelte`

- [ ] **Step 1: Create the directory**

```bash
mkdir -p "web/src/routes/projects/[id]/rankings/[rid]/players/[player_id]"
```

- [ ] **Step 2: Create `+page.server.ts`**

```typescript
import { error } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";
import type { PlayerStats, RankingPlayer, TournamentAttendance } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals }) => {
  const { api } = locals;

  const [statsRes, tournamentsRes, rankingPlayersRes] = await Promise.all([
    api.get(`/projects/${params.id}/rankings/${params.rid}/stats/${params.player_id}`),
    api.get(
      `/projects/${params.id}/rankings/${params.rid}/players/${params.player_id}/tournaments`,
    ),
    api.get(`/projects/${params.id}/rankings/${params.rid}/players`),
  ]);

  if (!statsRes.ok) {
    if (statsRes.status === 404) {
      error(404, { message: "not_found" });
    }
    error(statsRes.status, { message: "error" });
  }

  const stats: PlayerStats = await statsRes.json();

  if (!tournamentsRes.ok) {
    error(tournamentsRes.status, "Failed to load tournament history");
  }

  const tournaments: TournamentAttendance[] = await tournamentsRes.json();

  const rankingPlayers: RankingPlayer[] = rankingPlayersRes.ok
    ? await rankingPlayersRes.json()
    : [];
  const trackedPlayerIds = new Set(rankingPlayers.map((p) => p.player_id));

  return {
    stats,
    tournaments,
    trackedPlayerIds,
    projectId: params.id,
    rankingId: params.rid,
  };
};
```

- [ ] **Step 3: Create `+page.svelte`**

```svelte
<script lang="ts">
  import type { SetRecord } from "$lib/types";
  import SetDetailModal from "$lib/components/SetDetailModal.svelte";
  import * as Card from "$lib/components/ui/card";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import * as Empty from "$lib/components/ui/empty";
  import { Button } from "$lib/components/ui/button";
  import * as Table from "$lib/components/ui/table";
  import { winRate, toOrdinal, formatDate } from "$lib/utils";
  import { previousPage } from "$lib/stores/navigation";

  let { data } = $props();

  let selectedSet = $state<SetRecord | null>(null);
  let selectedIsWin = $state(false);

  const backHref = $derived(
    $previousPage ??
      `/projects/${data.projectId}/rankings/${data.rankingId}/stats`,
  );

  function openModal(set: SetRecord, isWin: boolean) {
    selectedSet = set;
    selectedIsWin = isWin;
  }

  const wins = $derived(data.stats.wins);
  const losses = $derived(data.stats.losses);
  const winRateStr = $derived(winRate(wins.length, losses.length, "0%"));
  const tournamentCount = $derived(data.tournaments.length);
</script>

<div class="space-y-6">
  <!-- Back button -->
  <Button variant="link" class="px-0" href={backHref}>← Back</Button>

  <!-- Header -->
  <div>
    <h2 class="text-2xl font-bold">{data.stats.name}</h2>
    <p class="text-sm text-muted-foreground">
      {wins.length} W · {losses.length} L · {winRateStr} win rate · {tournamentCount}
      tournaments
    </p>
  </div>

  <!-- Wins / Losses side by side -->
  <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
    <!-- Wins card -->
    <Card.Root>
      <Card.Header class="pb-2">
        <Card.Title class="text-base text-green-600 dark:text-green-400">
          Wins ({wins.length})
        </Card.Title>
      </Card.Header>
      <Card.Content class="pt-0">
        {#if wins.length === 0}
          <p class="text-sm text-muted-foreground">No wins yet.</p>
        {:else}
          <ScrollArea class="h-48 rounded border border-border bg-muted/20">
            {#each wins as set, i (i)}
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

    <!-- Losses card -->
    <Card.Root>
      <Card.Header class="pb-2">
        <Card.Title class="text-base text-red-600 dark:text-red-400">
          Losses ({losses.length})
        </Card.Title>
      </Card.Header>
      <Card.Content class="pt-0">
        {#if losses.length === 0}
          <p class="text-sm text-muted-foreground">No losses yet.</p>
        {:else}
          <ScrollArea class="h-48 rounded border border-border bg-muted/20">
            {#each losses as set, i (i)}
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

  <!-- Tournament history -->
  <div>
    <h3 class="mb-2 text-base font-semibold">
      Tournament history ({data.tournaments.length})
    </h3>
    {#if data.tournaments.length === 0}
      <Empty.Root>
        <Empty.Header>
          <Empty.Title>No tournament history</Empty.Title>
          <Empty.Description
            >No included tournaments found for this player in this ranking.</Empty.Description
          >
        </Empty.Header>
      </Empty.Root>
    {:else}
      <Table.Root>
        <Table.Header>
          <Table.Row>
            <Table.Head>Tournament · Event</Table.Head>
            <Table.Head>Placement</Table.Head>
            <Table.Head>Entrants</Table.Head>
            <Table.Head>Date</Table.Head>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#each data.tournaments as t, i (i)}
            <Table.Row>
              <Table.Cell class="font-medium">
                {t.tournament_name} · {t.event_name}
              </Table.Cell>
              <Table.Cell
                class={t.placement !== null && t.placement <= 3
                  ? "text-green-600 dark:text-green-400"
                  : ""}
              >
                {t.placement !== null ? toOrdinal(t.placement) : "—"}
              </Table.Cell>
              <Table.Cell>{t.num_entrants ?? "—"}</Table.Cell>
              <Table.Cell>{formatDate(t.start_at)}</Table.Cell>
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
  projectId={data.projectId}
  rankingId={data.rankingId}
  opponentPlayerId={selectedSet &&
  selectedSet.opponent_id !== null &&
  data.trackedPlayerIds.has(selectedSet.opponent_id)
    ? selectedSet.opponent_id
    : undefined}
/>
```

- [ ] **Step 4: Type-check**

```bash
cd web && npm run check 2>&1 | tail -20
```

Expected: no errors or warnings.

- [ ] **Step 5: Commit**

```bash
git add "web/src/routes/projects/[id]/rankings/[rid]/players/[player_id]"
git commit -m "feat(frontend): add ranking-scoped player detail page"
```

---

### Task 4: Frontend — update player name links in stats, ranking, and h2h pages

**Files:**
- Modify: `web/src/routes/projects/[id]/rankings/[rid]/stats/+page.svelte`
- Modify: `web/src/routes/projects/[id]/rankings/[rid]/ranking/+page.svelte`
- Modify: `web/src/routes/projects/[id]/rankings/[rid]/h2h/+page.svelte`

In all three files, `data.ranking.id` is available from the ranking layout's server data.

- [ ] **Step 1: Update `stats/+page.svelte`**

Add `rankingId` to the SetDetailModal call and update the player name link. Find the player name anchor (around line 51) and update the `href`:

```svelte
href="/projects/{data.project.id}/rankings/{data.ranking.id}/players/{player.player_id}"
```

Also add `rankingId={data.ranking.id}` to the `<SetDetailModal>` call (around line 111):

```svelte
<SetDetailModal
  set={selectedSet}
  isWin={selectedIsWin}
  currentPlayerName={selectedPlayerName}
  onClose={() => (selectedSet = null)}
  projectId={data.project.id}
  rankingId={data.ranking.id}
  opponentPlayerId={selectedSet &&
  selectedSet.opponent_id !== null &&
  trackedPlayerIds.has(selectedSet.opponent_id)
    ? selectedSet.opponent_id
    : undefined}
/>
```

- [ ] **Step 2: Update `ranking/+page.svelte`**

There are two `href` instances in this file (lines ~259 and ~298), both currently:

```svelte
href="/projects/{data.project.id}/players/{item.id}"
```

Change both to:

```svelte
href="/projects/{data.project.id}/rankings/{data.ranking.id}/players/{item.id}"
```

- [ ] **Step 3: Update `h2h/+page.svelte`**

There are two `href` instances (lines ~108 and ~131), currently:

```svelte
href="/projects/{data.project.id}/players/{col.id}"
```

and

```svelte
href="/projects/{data.project.id}/players/{row.id}"
```

Change both to:

```svelte
href="/projects/{data.project.id}/rankings/{data.ranking.id}/players/{col.id}"
```

and

```svelte
href="/projects/{data.project.id}/rankings/{data.ranking.id}/players/{row.id}"
```

- [ ] **Step 4: Type-check**

```bash
cd web && npm run check 2>&1 | tail -20
```

Expected: no errors or warnings.

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/projects/\[id\]/rankings/\[rid\]/stats/+page.svelte \
        web/src/routes/projects/\[id\]/rankings/\[rid\]/ranking/+page.svelte \
        web/src/routes/projects/\[id\]/rankings/\[rid\]/h2h/+page.svelte
git commit -m "fix(frontend): update player name links to ranking-scoped URLs"
```

---

### Task 5: Frontend — remove player link from `PlayerCard`

**Files:**
- Modify: `web/src/lib/components/PlayerCard.svelte`

- [ ] **Step 1: Replace the player name anchor with plain text**

Find the anchor in the non-editing branch (around line 68-70):

```svelte
<a
  href="/projects/{projectId}/players/{player.id}"
  class="font-medium hover:underline">{player.name}</a
>
```

Replace with:

```svelte
<span class="font-medium">{player.name}</span>
```

- [ ] **Step 2: Type-check**

```bash
cd web && npm run check 2>&1 | tail -20
```

Expected: no errors or warnings.

- [ ] **Step 3: Commit**

```bash
git add web/src/lib/components/PlayerCard.svelte
git commit -m "fix(frontend): remove broken player detail link from PlayerCard"
```

---

### Task 6: Frontend — delete the old player detail page

**Files:**
- Delete: `web/src/routes/projects/[id]/(hub)/(editor)/players/[player_id]/+page.svelte`
- Delete: `web/src/routes/projects/[id]/(hub)/(editor)/players/[player_id]/+page.server.ts`

- [ ] **Step 1: Delete both files**

```bash
rm "web/src/routes/projects/[id]/(hub)/(editor)/players/[player_id]/+page.svelte"
rm "web/src/routes/projects/[id]/(hub)/(editor)/players/[player_id]/+page.server.ts"
```

- [ ] **Step 2: Verify directory is now empty (optional cleanup)**

```bash
ls "web/src/routes/projects/[id]/(hub)/(editor)/players/[player_id]/"
```

If the directory is empty, remove it:

```bash
rmdir "web/src/routes/projects/[id]/(hub)/(editor)/players/[player_id]/"
```

- [ ] **Step 3: Type-check**

```bash
cd web && npm run check 2>&1 | tail -20
```

Expected: no errors or warnings.

- [ ] **Step 4: Commit**

```bash
git add -u "web/src/routes/projects/[id]/(hub)/(editor)/players/[player_id]/"
git commit -m "chore(frontend): delete broken project-level player detail page"
```

---

### Task 7: Update documentation

**Files:**
- Modify: `docs/routes.md`
- Modify: `backend/openapi.yaml`

- [ ] **Step 1: Update `docs/routes.md`**

Replace the old player detail row:

```
| `/projects/[id]/(editor)/players/[player_id]` | Owner/editor | Edit one player's display name and start.gg accounts |
```

With the new row:

```
| `/projects/[id]/rankings/[rid]/players/[player_id]` | Owner/member (published: guest) | Per-ranking player stats: wins, losses, and tournament history scoped to the ranking's included events |
```

- [ ] **Step 2: Update `backend/openapi.yaml`**

Find the section for `GET /projects/{id}/rankings/{rid}/stats/{player_id}` and add the new endpoint immediately after it, following the same pattern:

```yaml
/projects/{id}/rankings/{rid}/players/{pid}/tournaments:
  get:
    summary: Get a player's tournament history for a ranking
    description: Returns tournaments where the player competed in events included in the specified ranking.
    tags: [tournaments]
    parameters:
      - name: id
        in: path
        required: true
        schema:
          type: string
          format: uuid
      - name: rid
        in: path
        required: true
        schema:
          type: string
          format: uuid
      - name: pid
        in: path
        required: true
        schema:
          type: string
          format: uuid
    responses:
      '200':
        description: Tournament attendance list
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
git add docs/routes.md backend/openapi.yaml
git commit -m "docs: update route map and openapi spec for ranking-scoped player page"
```

---

### Final verification

- [ ] **Run the full test suite**

```bash
bash test.sh
```

Expected: all sections PASS.
