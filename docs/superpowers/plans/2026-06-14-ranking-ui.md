# Ranking UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a complete UI for creating, configuring, and viewing multiple rankings (manual and algorithmic) within a project, including route restructure, algorithm picker, settings page, and algorithmic ranking display.

**Architecture:** The SvelteKit route tree gains a `(hub)` route group that isolates project-hub pages (rankings list, players, import, project settings) from the isolated ranking view — no URL changes, but eliminates the double-tab-bar problem. The ranking layout gains a breadcrumb switcher and a Settings tab. The Ranking tab gains algorithmic display (computed ratings, delta badges, Sync to algorithm) when the ranking has an algorithm set. The backend gains a `player_count` field in the rankings list response and one-time rank_position seeding from computed_rating for algorithmic rankings.

**Tech Stack:** SvelteKit 5 (runes), TypeScript, Rust/Axum, sqlx, shadcn-svelte (Popover, Button, Input, Label, Separator, AlertDialog, Badge), svelte-dnd-action

---

## File Map

**Modify (frontend):**
- `web/src/lib/types.ts` — add algorithm fields to `Ranking`, add `RankingPlayerWithScore`
- `web/src/lib/api.ts` — add `recomputeRanking` method
- `web/src/routes/projects/[id]/+layout.svelte` — strip tab bar, keep project title + guest banner only
- `web/src/routes/projects/[id]/+page.server.ts` → **move** to `(hub)/+page.server.ts`, remove single-ranking redirect
- `web/src/routes/projects/[id]/+page.svelte` → **move** to `(hub)/+page.svelte`, add algorithm label + player count
- `web/src/routes/projects/[id]/(editor)/` → **move** to `(hub)/(editor)/` (all files verbatim)
- `web/src/routes/projects/[id]/settings/` → **move** to `(hub)/settings/` (all files verbatim)
- `web/src/routes/projects/[id]/rankings/[rid]/+layout.server.ts` — also load all project rankings
- `web/src/routes/projects/[id]/rankings/[rid]/+layout.svelte` — add switcher, Settings tab
- `web/src/routes/projects/[id]/rankings/new/+page.svelte` — add algorithm picker
- `web/src/routes/projects/[id]/rankings/new/+page.server.ts` — pass algorithm + algorithm_config
- `web/src/routes/projects/[id]/rankings/[rid]/ranking/+page.server.ts` — call `/ranking` endpoint instead of `/players`
- `web/src/routes/projects/[id]/rankings/[rid]/ranking/+page.svelte` — algorithmic display
- `web/tests/mock-api.js` — add algorithm fields to MOCK_RANKINGS, add new endpoints

**Create (frontend):**
- `web/src/routes/projects/[id]/(hub)/+layout.svelte` — project tab bar
- `web/src/routes/projects/[id]/rankings/[rid]/settings/+page.server.ts`
- `web/src/routes/projects/[id]/rankings/[rid]/settings/+page.svelte`

**Modify (backend):**
- `backend/crates/api/src/routes/rankings.rs` — add `player_count` to `RankingResponse` + `list_rankings` query
- `backend/crates/worker/src/compute.rs` — seed `rank_position` from `computed_rating` on first compute

---

## Task 1: Frontend types and API method

**Files:**
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/lib/api.ts`

- [ ] **Step 1: Extend `Ranking` interface and add `RankingPlayerWithScore`**

In `web/src/lib/types.ts`, replace the current `Ranking` interface:

```typescript
export interface Ranking {
  id: string;
  project_id: string;
  name: string;
  description: string | null;
  published: boolean;
  created_at: string;
  user_role: "owner" | "editor" | "viewer" | null;
  algorithm: string | null;
  algorithm_config: Record<string, unknown>;
  include_external_results: boolean;
  result_sort: string;
}
```

And add this new interface after `RankingPlayer`:

```typescript
export interface RankingPlayerWithScore {
  player_id: string;
  name: string;
  rank_position: number;
  notes: string | null;
  computed_rating: number | null;
  display_data: Record<string, unknown> | null;
}
```

- [ ] **Step 2: Add `recomputeRanking` to `web/src/lib/api.ts`**

Add after the `deleteRanking` method (around line 37):

```typescript
recomputeRanking: (projectId: string, rankingId: string) =>
  req("POST", `/projects/${projectId}/rankings/${rankingId}/recompute`),
```

- [ ] **Step 3: Verify types compile**

```bash
cd web && npx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add web/src/lib/types.ts web/src/lib/api.ts
git commit -m "feat(frontend): extend Ranking type with algorithm fields; add recomputeRanking"
```

---

## Task 2: Backend — add player_count to rankings list response

**Files:**
- Modify: `backend/crates/api/src/routes/rankings.rs`

The `GET /projects/:id/rankings` response needs a `player_count` so ranking cards can show "12 players". Currently the `Ranking` DB model has no such field; we add it to `RankingResponse` with a SQL subquery.

- [ ] **Step 1: Update `RankingResponse` struct**

In `backend/crates/api/src/routes/rankings.rs`, add `player_count` to `RankingResponse` (around line 182):

```rust
#[derive(Serialize)]
struct RankingResponse {
    id: Uuid,
    project_id: Uuid,
    name: String,
    description: Option<String>,
    published: bool,
    algorithm: Option<String>,
    algorithm_config: serde_json::Value,
    include_external_results: bool,
    result_sort: String,
    created_at: DateTime<Utc>,
    user_role: Option<UserRole>,
    player_count: i64,
}
```

- [ ] **Step 2: Update `from_ranking` to accept player_count**

Replace `impl RankingResponse` (around line 197):

```rust
impl RankingResponse {
    fn from_ranking(r: Ranking, role: Option<UserRole>, player_count: i64) -> Self {
        RankingResponse {
            id: r.id,
            project_id: r.project_id,
            name: r.name,
            description: r.description,
            published: r.published,
            algorithm: r.algorithm,
            algorithm_config: r.algorithm_config,
            include_external_results: r.include_external_results,
            result_sort: r.result_sort,
            created_at: r.created_at,
            user_role: role,
            player_count,
        }
    }
}
```

- [ ] **Step 3: Update `list_rankings` to use a query with player_count**

Replace the `list_rankings` handler (around line 240). The current query uses `sqlx::query_as!(Ranking, ...)` which can't include the extra column. Switch to `sqlx::query!` and map manually:

```rust
async fn list_rankings(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    use crate::routes::projects::require_project_read_access;
    let (_, role) = require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;

    let rows = sqlx::query!(
        r#"SELECT r.id, r.project_id, r.name, r.description, r.published,
                  r.algorithm, r.algorithm_config, r.include_external_results,
                  r.result_sort, r.created_at,
                  COUNT(rp.player_id) AS "player_count!"
           FROM rankings r
           LEFT JOIN ranking_players rp ON rp.ranking_id = r.id
           WHERE r.project_id = $1
           GROUP BY r.id
           ORDER BY r.created_at ASC"#,
        project_id,
    )
    .fetch_all(&state.db)
    .await?;

    let resp: Vec<RankingResponse> = rows
        .into_iter()
        .map(|r| {
            let ranking = Ranking {
                id: r.id,
                project_id: r.project_id,
                name: r.name,
                description: r.description,
                published: r.published,
                algorithm: r.algorithm,
                algorithm_config: r.algorithm_config,
                include_external_results: r.include_external_results,
                result_sort: r.result_sort,
                created_at: r.created_at,
            };
            RankingResponse::from_ranking(ranking, role.clone(), r.player_count)
        })
        .collect();
    Ok(Json(resp))
}
```

Also update any other call sites that use `from_ranking` without `player_count` — check for `get_ranking` handler. That endpoint returns a single ranking, which doesn't need player_count (add `player_count: 0` for now if it calls `from_ranking`). 

Actually, `get_ranking` is a separate handler. Check if it uses `from_ranking`:

```bash
grep -n "from_ranking" backend/crates/api/src/routes/rankings.rs
```

If `get_ranking` also calls `from_ranking`, add a separate query there to get the count. If it doesn't (it likely serializes the `Ranking` struct directly), no change needed.

- [ ] **Step 4: Update offline sqlx cache**

```bash
bash backend/prepare-sqlx.sh
```

Expected: completes without error, updates `.sqlx/` files.

- [ ] **Step 5: Run backend tests**

```bash
bash backend/test.sh
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add backend/crates/api/src/routes/rankings.rs backend/.sqlx/
git commit -m "feat(api): add player_count to rankings list response"
```

---

## Task 3: Backend — seed rank_position from computed_rating on first compute

**Files:**
- Modify: `backend/crates/worker/src/compute.rs`

When an algorithmic ranking is computed for the first time, all `rank_position` values for its players are 0 (no editor has ordered them yet). Seed them from `computed_rating` DESC order so the ranking tab has a sensible initial order instead of showing everything at position 0.

- [ ] **Step 1: Write a test for first-compute seeding**

Add to `backend/crates/e2e/src/tests` (or find the appropriate e2e test file — check `backend/crates/e2e/src/`):

```bash
find backend/crates/e2e/src -name "*.rs" | head -10
```

Add this test to the appropriate file (likely `compute.rs` or `ranking.rs` in e2e):

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_algorithmic_ranking_seeds_rank_position_on_first_compute(pool: PgPool) {
    // Set up: project, two players, elo ranking, one included event with one set
    // (minimal setup to trigger a compute that produces computed_ratings)
    // Then run compute::run and verify rank_positions are non-zero and sorted
    // by computed_rating DESC.
    //
    // This is an integration test — search for existing setup helpers in this
    // e2e crate (look for how other tests create projects/players/rankings).
    todo!("implement after reading e2e test helpers");
}
```

Actually, look at the existing e2e tests for a pattern. Run:

```bash
grep -n "#\[sqlx::test" backend/crates/e2e/src/*.rs | head -10
```

Find an existing test that runs a compute job and adapt it.

- [ ] **Step 2: Add `seed_rank_position_from_scores` function to compute.rs**

In `backend/crates/worker/src/compute.rs`, add after `phase2_algorithm_scores`:

```rust
async fn seed_rank_position_from_scores(pool: &PgPool, ranking_id: Uuid) -> anyhow::Result<()> {
    // Only seed if all rank_positions are still 0 (first compute only)
    let all_zero: bool = sqlx::query_scalar!(
        "SELECT NOT EXISTS (SELECT 1 FROM ranking_players WHERE ranking_id = $1 AND rank_position != 0)",
        ranking_id,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(true);

    if !all_zero {
        return Ok(());
    }

    // Assign rank_position = ROW_NUMBER ordered by computed_rating DESC
    sqlx::query!(
        r#"
        UPDATE ranking_players rp
        SET rank_position = ranked.new_rank::int4
        FROM (
            SELECT player_id,
                   ROW_NUMBER() OVER (ORDER BY computed_rating DESC NULLS LAST) AS new_rank
            FROM ranking_player_scores
            WHERE ranking_id = $1
        ) ranked
        WHERE rp.player_id = ranked.player_id
          AND rp.ranking_id = $1
        "#,
        ranking_id,
    )
    .execute(pool)
    .await?;

    tracing::info!(%ranking_id, "algorithmic ranking: seeded rank_position from computed_rating");
    Ok(())
}
```

- [ ] **Step 3: Call it from `run` after phase2**

In the `run` function, modify the `phase2` call site:

```rust
pub async fn run(pool: &PgPool, ranking_id: Uuid) -> anyhow::Result<()> {
    let ranking = sqlx::query!(
        r#"SELECT algorithm, algorithm_config, include_external_results
           FROM rankings WHERE id = $1"#,
        ranking_id,
    )
    .fetch_optional(pool)
    .await?;

    let Some(ranking) = ranking else {
        anyhow::bail!("ranking {ranking_id} not found");
    };

    phase1_set_results(pool, ranking_id).await?;

    if let Some(ref algo_name) = ranking.algorithm {
        phase2_algorithm_scores(
            pool,
            ranking_id,
            algo_name,
            &ranking.algorithm_config,
        )
        .await?;

        seed_rank_position_from_scores(pool, ranking_id).await?;
    }

    Ok(())
}
```

- [ ] **Step 4: Update offline sqlx cache**

```bash
bash backend/prepare-sqlx.sh
```

- [ ] **Step 5: Run backend tests**

```bash
bash backend/test.sh
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add backend/crates/worker/src/compute.rs backend/.sqlx/
git commit -m "feat(worker): seed rank_position from computed_rating on first algorithmic compute"
```

---

## Task 4: Route restructure — create `(hub)` group

**Files:**
- Modify: `web/src/routes/projects/[id]/+layout.svelte`
- Create: `web/src/routes/projects/[id]/(hub)/+layout.svelte`
- Move: `+page.server.ts` and `+page.svelte` into `(hub)/`
- Move: `(editor)/` into `(hub)/(editor)/`
- Move: `settings/` into `(hub)/settings/`

This is a file-system restructure. SvelteKit route groups (parentheses directories) are invisible to the browser URL — `/projects/[id]/settings` works whether the file is at `settings/` or `(hub)/settings/`. The group just controls which layout is in the render chain.

**Current layout chain for `/projects/[id]`:**
```
+layout.svelte (project title + tab bar)  →  +page.svelte (rankings list)
```

**New layout chain for `/projects/[id]` (rankings list):**
```
+layout.svelte (project title + guest banner only)
  └── (hub)/+layout.svelte (project tab bar)
        └── (hub)/+page.svelte (rankings list)
```

**New layout chain for `/projects/[id]/rankings/[rid]/stats`:**
```
+layout.svelte (thin shell — only project title + guest banner)
  └── rankings/[rid]/+layout.svelte (breadcrumb + ranking tabs)
        └── rankings/[rid]/stats/+page.svelte
```
_(No `(hub)` layout in this chain — the isolated ranking view has no project tab bar.)_

- [ ] **Step 1: Modify `+layout.svelte` to thin shell**

Replace `web/src/routes/projects/[id]/+layout.svelte` with:

```svelte
<script lang="ts">
  import { page } from "$app/state";

  let { children, data } = $props();
</script>

<div class="space-y-4">
  {#if !page.data.user}
    <div class="border-b bg-muted px-4 py-2 text-sm text-muted-foreground">
      You're viewing a shared project · <a
        href="/register"
        class="underline hover:text-foreground">Sign up</a
      > to build your own rankings
    </div>
  {/if}

  <div class="space-y-4 {page.data.wide ? 'mx-auto max-w-5xl px-4' : ''}">
    <div>
      <a
        href={page.data.user ? "/projects" : "/"}
        class="text-sm text-muted-foreground hover:text-foreground"
        >{page.data.user ? "← Projects" : "← Home"}</a
      >
      <h1 class="mt-1 text-2xl font-bold">{data.project.name}</h1>
      {#if data.project.game_name}
        <p class="text-sm text-muted-foreground">{data.project.game_name}</p>
      {/if}
    </div>
  </div>

  {@render children()}
</div>
```

- [ ] **Step 2: Create `(hub)/+layout.svelte`**

Create `web/src/routes/projects/[id]/(hub)/+layout.svelte`:

```svelte
<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { Separator } from "$lib/components/ui/separator";
  import * as Tabs from "$lib/components/ui/tabs";

  let { children, data } = $props();

  const allTabs = [
    { label: "Rankings", href: "", minRole: null },
    { label: "Players", href: "players", minRole: "editor" as const },
    { label: "Import", href: "import", minRole: "editor" as const },
    { label: "Settings", href: "settings", minRole: "owner" as const },
  ];

  const tabs = $derived(
    allTabs.filter((t) => {
      const role = data.project.user_role;
      if (t.minRole === null) return true;
      if (t.minRole === "editor") return role === "editor" || role === "owner";
      if (t.minRole === "owner") return role === "owner";
      return false;
    }),
  );

  function tabHref(slug: string) {
    if (slug === "") return `/projects/${data.project.id}`;
    return `/projects/${data.project.id}/${slug}`;
  }

  const currentTab = $derived(
    tabs.find((t) => {
      if (t.href === "") {
        return page.url.pathname === `/projects/${data.project.id}`;
      }
      return page.url.pathname.startsWith(tabHref(t.href));
    })?.href ?? tabs[0].href,
  );
</script>

<div class="space-y-4 {page.data.wide ? 'mx-auto max-w-5xl px-4' : ''}">
  <Tabs.Root value={currentTab} onValueChange={(v) => v !== undefined && goto(tabHref(v))}>
    <Tabs.List>
      {#each tabs as tab (tab.href)}
        <Tabs.Trigger value={tab.href}>{tab.label}</Tabs.Trigger>
      {/each}
    </Tabs.List>
  </Tabs.Root>

  <Separator />

  {@render children()}
</div>
```

- [ ] **Step 3: Move page files into `(hub)/`**

```bash
git mv "web/src/routes/projects/[id]/+page.server.ts" "web/src/routes/projects/[id]/(hub)/+page.server.ts"
git mv "web/src/routes/projects/[id]/+page.svelte" "web/src/routes/projects/[id]/(hub)/+page.svelte"
```

- [ ] **Step 4: Move `(editor)/` into `(hub)/(editor)/`**

```bash
mkdir -p "web/src/routes/projects/[id]/(hub)"
git mv "web/src/routes/projects/[id]/(editor)" "web/src/routes/projects/[id]/(hub)/(editor)"
```

- [ ] **Step 5: Move `settings/` into `(hub)/settings/`**

```bash
git mv "web/src/routes/projects/[id]/settings" "web/src/routes/projects/[id]/(hub)/settings"
```

- [ ] **Step 6: Verify dev server starts and routes work**

```bash
cd web && npm run dev &
sleep 5
curl -s -o /dev/null -w "%{http_code}" http://localhost:5173/projects/test-id
```

Expected: 302 or 200 (redirect to login or project page).

Stop dev server. Run e2e tests:

```bash
cd web && npm run test:e2e
```

Expected: all tests pass. (If any test fails because it's navigating to a ranking tab that no longer exists in the double-layout, investigate and fix before proceeding.)

- [ ] **Step 7: Commit**

```bash
git add web/src/routes/projects/
git commit -m "refactor(frontend): isolate project hub into (hub) route group; remove project tab bar from ranking view chain"
```

---

## Task 5: Rankings list page — remove redirect, add algorithm label and player count

**Files:**
- Modify: `web/src/routes/projects/[id]/(hub)/+page.server.ts`
- Modify: `web/src/routes/projects/[id]/(hub)/+page.svelte`
- Modify: `web/tests/mock-api.js`

The rankings list page no longer redirects when there's exactly one ranking (that auto-redirect was what broke the Rankings tab's active state). Cards now show the algorithm label and player count.

- [ ] **Step 1: Remove single-ranking redirect from `+page.server.ts`**

Replace `web/src/routes/projects/[id]/(hub)/+page.server.ts` with:

```typescript
import type { PageServerLoad } from "./$types";
import type { Ranking } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const res = await api.get(`/projects/${params.id}/rankings`);
  const rankings: Ranking[] = res.ok ? await res.json() : [];
  return { rankings };
};
```

- [ ] **Step 2: Update `+page.svelte` to show algorithm label and player count**

Replace `web/src/routes/projects/[id]/(hub)/+page.svelte` with:

```svelte
<script lang="ts">
  import type { PageData } from "./$types";
  import * as Card from "$lib/components/ui/card";
  import { Button } from "$lib/components/ui/button";
  import { Badge } from "$lib/components/ui/badge";
  import type { Ranking } from "$lib/types";

  let { data }: { data: PageData } = $props();
  const isEditor =
    data.project.user_role === "owner" || data.project.user_role === "editor";

  function algorithmLabel(r: Ranking): string {
    if (!r.algorithm) return "Manual";
    if (r.algorithm === "elo") return "Elo";
    if (r.algorithm === "glicko2") return "Glicko-2";
    return r.algorithm;
  }
</script>

<div class="container mx-auto max-w-3xl py-8 px-4">
  <div class="mb-6 flex items-center justify-between">
    <h2 class="text-xl font-semibold">Rankings</h2>
    {#if isEditor}
      <Button href="/projects/{data.project.id}/rankings/new" size="sm"
        >New ranking</Button
      >
    {/if}
  </div>

  {#if data.rankings.length === 0}
    <p class="text-muted-foreground">
      No rankings yet.{#if isEditor}
        Create one to get started.{/if}
    </p>
  {:else}
    <div class="flex flex-col gap-3">
      {#each data.rankings as ranking (ranking.id)}
        <a href="/projects/{data.project.id}/rankings/{ranking.id}/stats">
          <Card.Root class="cursor-pointer transition-colors hover:bg-muted/50">
            <Card.Header>
              <div class="flex items-center justify-between">
                <Card.Title>{ranking.name}</Card.Title>
                {#if ranking.published}
                  <Badge variant="secondary">Public</Badge>
                {:else}
                  <Badge variant="outline">Private</Badge>
                {/if}
              </div>
              <div class="flex items-center gap-2 text-sm text-muted-foreground">
                <span>{algorithmLabel(ranking)}</span>
                {#if (ranking as any).player_count !== undefined}
                  <span>·</span>
                  <span>{(ranking as any).player_count} players</span>
                {/if}
              </div>
              {#if ranking.description}
                <Card.Description>{ranking.description}</Card.Description>
              {/if}
            </Card.Header>
          </Card.Root>
        </a>
      {/each}
    </div>
  {/if}
</div>
```

Note: The card links now go to `/stats` (the default ranking view for all visitors) rather than `/ranking`.

- [ ] **Step 3: Update mock-api.js ranking fixtures to include algorithm fields**

In `web/tests/mock-api.js`, update all ranking objects in `MOCK_RANKINGS` to include the new fields:

```javascript
const MOCK_RANKINGS = {
  'proj-1': [{ 
    id: 'rank-1', project_id: 'proj-1', name: 'Main Ranking', description: null, 
    published: false, created_at: '2026-01-01T00:00:00Z', user_role: 'owner',
    algorithm: null, algorithm_config: {}, include_external_results: false, 
    result_sort: 'upset_factor', player_count: 3
  }],
  'proj-viewer': [{ 
    id: 'rank-viewer', project_id: 'proj-viewer', name: 'Main Ranking', description: null, 
    published: true, created_at: '2026-01-01T00:00:00Z', user_role: 'viewer',
    algorithm: null, algorithm_config: {}, include_external_results: false,
    result_sort: 'upset_factor', player_count: 0
  }],
  'proj-viewer-tournaments': [{ 
    id: 'rank-viewer', project_id: 'proj-viewer-tournaments', name: 'Main Ranking', 
    description: null, published: true, created_at: '2026-01-01T00:00:00Z', user_role: 'viewer',
    algorithm: null, algorithm_config: {}, include_external_results: false,
    result_sort: 'upset_factor', player_count: 0
  }],
  'proj-guest': [{ 
    id: 'rank-guest', project_id: 'proj-guest', name: 'Public Ranking', description: null, 
    published: true, created_at: '2026-01-01T00:00:00Z', user_role: null,
    algorithm: null, algorithm_config: {}, include_external_results: false,
    result_sort: 'upset_factor', player_count: 0
  }],
  'proj-published': [{ 
    id: 'rank-published', project_id: 'proj-published', name: 'Published Ranking', 
    description: null, published: true, created_at: '2026-01-01T00:00:00Z', user_role: 'owner',
    algorithm: null, algorithm_config: {}, include_external_results: false,
    result_sort: 'upset_factor', player_count: 0
  }],
  'proj-tournaments': [{ 
    id: 'rank-tournaments', project_id: 'proj-tournaments', name: 'Main Ranking', 
    description: null, published: false, created_at: '2026-01-01T00:00:00Z', user_role: 'owner',
    algorithm: null, algorithm_config: {}, include_external_results: false,
    result_sort: 'upset_factor', player_count: 0
  }],
  'proj-failed': [],
};
```

Also add a mock entry for `GET /projects/:id/rankings/:rid` (single ranking) in the request handler, if it doesn't already exist. Check with:

```bash
grep -n "rankings/" web/tests/mock-api.js | head -20
```

For each GET to `/projects/:id/rankings/:id`, return the matching ranking object from `MOCK_RANKINGS` with the new fields.

- [ ] **Step 4: Run e2e tests**

```bash
cd web && npm run test:e2e
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/projects/ web/tests/mock-api.js
git commit -m "feat(frontend): rankings list shows algorithm label and player count; remove single-ranking redirect"
```

---

## Task 6: New ranking creation — algorithm picker

**Files:**
- Modify: `web/src/routes/projects/[id]/rankings/new/+page.svelte`
- Modify: `web/src/routes/projects/[id]/rankings/new/+page.server.ts`

The creation form gains a radio card picker for Manual / Elo / Glicko-2. Selecting Elo or Glicko-2 reveals inline config fields with pre-filled defaults.

- [ ] **Step 1: Update `+page.server.ts` to pass algorithm and algorithm_config**

Replace `web/src/routes/projects/[id]/rankings/new/+page.server.ts` with:

```typescript
import { redirect, fail } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";

export const load: PageServerLoad = async ({ parent }) => {
  const { project } = await parent();
  if (project.user_role !== "owner" && project.user_role !== "editor") {
    redirect(303, `/projects/${project.id}`);
  }
  return {};
};

export const actions = {
  default: async ({ request, params, locals }) => {
    const { api } = locals;
    const data = await request.formData();
    const name = (data.get("name") as string)?.trim();
    const description =
      ((data.get("description") as string) || "").trim() || undefined;
    const algorithm = (data.get("algorithm") as string) || null;

    if (!name) return fail(422, { error: "Name is required" });

    // Build algorithm_config from form fields when an algorithm is selected
    let algorithm_config: Record<string, unknown> | undefined;
    if (algorithm === "elo") {
      const k = parseFloat(data.get("elo_k") as string);
      const initial = parseFloat(data.get("elo_initial") as string);
      algorithm_config = {
        k_factor: isNaN(k) ? 32 : k,
        initial_rating: isNaN(initial) ? 1500 : initial,
      };
    } else if (algorithm === "glicko2") {
      const tau = parseFloat(data.get("g2_tau") as string);
      const rd = parseFloat(data.get("g2_rd") as string);
      const sigma = parseFloat(data.get("g2_sigma") as string);
      algorithm_config = {
        tau: isNaN(tau) ? 0.5 : tau,
        initial_rd: isNaN(rd) ? 350 : rd,
        initial_volatility: isNaN(sigma) ? 0.06 : sigma,
      };
    }

    const res = await api.post(`/projects/${params.id}/rankings`, {
      name,
      description,
      algorithm: algorithm || undefined,
      algorithm_config,
    });
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      return fail(res.status, {
        error:
          (body as { message?: string }).message ?? "Failed to create ranking",
      });
    }
    const ranking = await res.json();
    redirect(303, `/projects/${params.id}/rankings/${ranking.id}/players`);
  },
} satisfies Actions;
```

- [ ] **Step 2: Update `+page.svelte` with algorithm picker**

Replace `web/src/routes/projects/[id]/rankings/new/+page.svelte` with:

```svelte
<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { Separator } from "$lib/components/ui/separator";
  import type { ActionData } from "./$types";

  let { form }: { form: ActionData } = $props();

  let algorithm = $state<"" | "elo" | "glicko2">("");
</script>

<div class="container mx-auto max-w-md py-8 px-4">
  <h1 class="mb-6 text-2xl font-bold">New ranking</h1>
  <form method="POST" class="flex flex-col gap-4">
    <div class="flex flex-col gap-1.5">
      <Label for="name">Name</Label>
      <Input id="name" name="name" required placeholder="2025 Season" />
    </div>
    <div class="flex flex-col gap-1.5">
      <Label for="description"
        >Description <span class="text-muted-foreground">(optional)</span
        ></Label
      >
      <Input
        id="description"
        name="description"
        placeholder="Brief description"
      />
    </div>

    <div class="flex flex-col gap-2">
      <Label>Algorithm</Label>

      <!-- Manual card -->
      <label
        class="flex cursor-pointer items-start gap-3 rounded-md border p-3 transition-colors
          {algorithm === '' ? 'border-primary bg-muted/40' : 'border-border hover:bg-muted/20'}"
      >
        <input
          type="radio"
          name="algorithm"
          value=""
          bind:group={algorithm}
          class="mt-0.5 accent-primary"
        />
        <div>
          <div class="text-sm font-semibold">Manual</div>
          <div class="text-xs text-muted-foreground">
            You set the order by dragging players
          </div>
        </div>
      </label>

      <!-- Elo card -->
      <label
        class="flex cursor-pointer items-start gap-3 rounded-md border p-3 transition-colors
          {algorithm === 'elo' ? 'border-primary bg-muted/40' : 'border-border hover:bg-muted/20'}"
      >
        <input
          type="radio"
          name="algorithm"
          value="elo"
          bind:group={algorithm}
          class="mt-0.5 accent-primary"
        />
        <div class="w-full">
          <div class="flex items-baseline gap-2">
            <span class="text-sm font-semibold">Elo</span>
            <a
              href="https://en.wikipedia.org/wiki/Elo_rating_system"
              target="_blank"
              rel="noopener noreferrer"
              class="text-xs text-primary hover:underline"
              onclick={(e) => e.stopPropagation()}
            >Wikipedia ↗</a>
          </div>
          <div class="text-xs text-muted-foreground">
            Classic rating system — players gain or lose points based on results relative to opponent strength
          </div>
          {#if algorithm === "elo"}
            <div class="mt-3 flex flex-col gap-3 border-t pt-3">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <div class="text-xs font-semibold">K-factor</div>
                  <div class="text-xs text-muted-foreground">
                    Points at stake per set. 32 is standard; lower (16) = slow changes, higher (64) = fast.
                  </div>
                </div>
                <Input
                  type="number"
                  name="elo_k"
                  value="32"
                  min="1"
                  max="256"
                  class="w-20 text-right"
                />
              </div>
              <div class="flex items-center justify-between gap-4">
                <div>
                  <div class="text-xs font-semibold">Initial rating</div>
                  <div class="text-xs text-muted-foreground">
                    Starting rating for all players. 1500 is the universal convention.
                  </div>
                </div>
                <Input
                  type="number"
                  name="elo_initial"
                  value="1500"
                  min="1"
                  class="w-20 text-right"
                />
              </div>
            </div>
          {/if}
        </div>
      </label>

      <!-- Glicko-2 card -->
      <label
        class="flex cursor-pointer items-start gap-3 rounded-md border p-3 transition-colors
          {algorithm === 'glicko2' ? 'border-primary bg-muted/40' : 'border-border hover:bg-muted/20'}"
      >
        <input
          type="radio"
          name="algorithm"
          value="glicko2"
          bind:group={algorithm}
          class="mt-0.5 accent-primary"
        />
        <div class="w-full">
          <div class="flex items-baseline gap-2">
            <span class="text-sm font-semibold">Glicko-2</span>
            <a
              href="https://en.wikipedia.org/wiki/Glicko_rating_system"
              target="_blank"
              rel="noopener noreferrer"
              class="text-xs text-primary hover:underline"
              onclick={(e) => e.stopPropagation()}
            >Wikipedia ↗</a>
          </div>
          <div class="text-xs text-muted-foreground">
            Extends Elo with a rating deviation (RD) — confidence interval on each player's true strength
          </div>
          {#if algorithm === "glicko2"}
            <div class="mt-3 flex flex-col gap-3 border-t pt-3">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <div class="text-xs font-semibold">τ (tau)</div>
                  <div class="text-xs text-muted-foreground">
                    Controls volatility change rate. Glickman recommends 0.3–1.2; lower = more stable.
                  </div>
                </div>
                <Input
                  type="number"
                  name="g2_tau"
                  value="0.5"
                  min="0.1"
                  max="2"
                  step="0.1"
                  class="w-20 text-right"
                />
              </div>
              <div class="flex items-center justify-between gap-4">
                <div>
                  <div class="text-xs font-semibold">Initial RD</div>
                  <div class="text-xs text-muted-foreground">
                    Rating deviation for a new player. High RD = uncertain rating; shrinks as they play.
                  </div>
                </div>
                <Input
                  type="number"
                  name="g2_rd"
                  value="350"
                  min="50"
                  max="700"
                  class="w-20 text-right"
                />
              </div>
              <div class="flex items-center justify-between gap-4">
                <div>
                  <div class="text-xs font-semibold">Initial volatility (σ)</div>
                  <div class="text-xs text-muted-foreground">
                    Expected rating fluctuation for a new player. Glickman recommends 0.06; rarely changed.
                  </div>
                </div>
                <Input
                  type="number"
                  name="g2_sigma"
                  value="0.06"
                  min="0.01"
                  max="1"
                  step="0.01"
                  class="w-20 text-right"
                />
              </div>
            </div>
          {/if}
        </div>
      </label>
    </div>

    {#if form?.error}
      <p class="text-sm text-destructive">{form.error}</p>
    {/if}
    <Button type="submit">Create ranking</Button>
  </form>
</div>
```

- [ ] **Step 3: Run e2e tests and type check**

```bash
cd web && npx tsc --noEmit && npm run test:e2e
```

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add web/src/routes/projects/
git commit -m "feat(frontend): add algorithm picker to new ranking creation form"
```

---

## Task 7: Ranking layout — breadcrumb ranking switcher and Settings tab

**Files:**
- Modify: `web/src/routes/projects/[id]/rankings/[rid]/+layout.server.ts`
- Modify: `web/src/routes/projects/[id]/rankings/[rid]/+layout.svelte`

The ranking layout gains a ranking-switcher dropdown in the breadcrumb and a Settings tab. The switcher preserves the current tab when navigating between rankings.

- [ ] **Step 1: Update layout.server.ts to load all rankings**

Replace `web/src/routes/projects/[id]/rankings/[rid]/+layout.server.ts` with:

```typescript
import { error } from "@sveltejs/kit";
import type { LayoutServerLoad } from "./$types";
import type { Ranking } from "$lib/types";

export const load: LayoutServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const [rankingRes, rankingsRes] = await Promise.all([
    api.get(`/projects/${params.id}/rankings/${params.rid}`),
    api.get(`/projects/${params.id}/rankings`),
  ]);
  if (!rankingRes.ok) {
    error(rankingRes.status === 404 ? 404 : rankingRes.status, {
      message: rankingRes.status === 404 ? "not_found" : "error",
    });
  }
  const ranking: Ranking = await rankingRes.json();
  const rankings: Ranking[] = rankingsRes.ok ? await rankingsRes.json() : [];
  return { ranking, rankings };
};
```

- [ ] **Step 2: Update layout.svelte**

Replace `web/src/routes/projects/[id]/rankings/[rid]/+layout.svelte` with:

```svelte
<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import * as Tabs from "$lib/components/ui/tabs";
  import { Separator } from "$lib/components/ui/separator";
  import * as Popover from "$lib/components/ui/popover";
  import { Button } from "$lib/components/ui/button";

  let { children, data } = $props();

  const allTabs = [
    { label: "Players", href: "players", minRole: "editor" as const },
    { label: "Tournaments", href: "tournaments", minRole: null },
    { label: "Stats", href: "stats", minRole: null },
    { label: "H2H", href: "h2h", minRole: null },
    { label: "Ranking", href: "ranking", minRole: null },
    { label: "Settings", href: "settings", minRole: "editor" as const },
  ];

  const tabs = $derived(
    allTabs.filter((t) => {
      const role = data.project.user_role;
      if (t.minRole === null) return true;
      if (t.minRole === "editor") return role === "editor" || role === "owner";
      return false;
    }),
  );

  function tabHref(slug: string) {
    return `/projects/${data.project.id}/rankings/${data.ranking.id}/${slug}`;
  }

  const currentTab = $derived(
    tabs.find((t) => page.url.pathname.startsWith(tabHref(t.href)))?.href ??
      tabs[0]?.href,
  );

  let switcherOpen = $state(false);

  function switchRanking(rid: string) {
    switcherOpen = false;
    const tab = currentTab ?? "stats";
    goto(`/projects/${data.project.id}/rankings/${rid}/${tab}`);
  }
</script>

<div class="space-y-4 {page.data.wide ? 'mx-auto max-w-5xl px-4' : ''}">
  <div class="px-4">
    <p class="text-sm text-muted-foreground">
      <a href="/projects/{data.project.id}" class="hover:text-foreground"
        >{data.project.name}</a
      >
      {" / "}
      <Popover.Root bind:open={switcherOpen}>
        <Popover.Trigger>
          <button class="font-medium text-foreground hover:underline">
            {data.ranking.name} ▾
          </button>
        </Popover.Trigger>
        <Popover.Content class="w-56 p-1" align="start">
          {#each data.rankings as r (r.id)}
            <button
              class="w-full rounded px-3 py-1.5 text-left text-sm transition-colors
                {r.id === data.ranking.id
                ? 'font-semibold text-primary'
                : 'text-foreground hover:bg-muted'}"
              onclick={() => switchRanking(r.id)}
            >
              {r.name}
            </button>
          {/each}
          {#if data.project.user_role === "editor" || data.project.user_role === "owner"}
            <Separator class="my-1" />
            <a
              href="/projects/{data.project.id}/rankings/new"
              class="block rounded px-3 py-1.5 text-sm text-primary hover:bg-muted"
              onclick={() => (switcherOpen = false)}
            >
              + New ranking
            </a>
          {/if}
        </Popover.Content>
      </Popover.Root>
    </p>
  </div>

  <Tabs.Root value={currentTab} onValueChange={(v) => v !== undefined && goto(tabHref(v))}>
    <div class="px-4">
      <Tabs.List>
        {#each tabs as tab (tab.href)}
          <Tabs.Trigger value={tab.href}>{tab.label}</Tabs.Trigger>
        {/each}
      </Tabs.List>
    </div>
  </Tabs.Root>

  <Separator />

  {@render children()}
</div>
```

- [ ] **Step 3: Add mock for single-ranking GET to mock-api.js**

In `web/tests/mock-api.js`, ensure the handler for `GET /projects/:id/rankings/:rid` returns the full ranking object with algorithm fields. Check the existing handler:

```bash
grep -n "rankings/" web/tests/mock-api.js
```

If the single-ranking GET isn't mocked, add it to the request handler switch/if chain. The response should be the matching entry from `MOCK_RANKINGS[projectId]` filtered by ranking ID:

```javascript
// In the request handler, add a case for GET /projects/:id/rankings/:rid
if (method === 'GET' && pathParts[1] === 'projects' && pathParts[3] === 'rankings' && pathParts[4] && pathParts[4] !== 'new') {
  const projectId = pathParts[2];
  const rankingId = pathParts[4];
  const rankings = MOCK_RANKINGS[projectId] || [];
  const ranking = rankings.find(r => r.id === rankingId);
  if (ranking) {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(ranking));
  } else {
    res.writeHead(404); res.end();
  }
  return;
}
```

(Adapt to match the existing handler structure in mock-api.js.)

- [ ] **Step 4: Run type check and e2e tests**

```bash
cd web && npx tsc --noEmit && npm run test:e2e
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/projects/ web/tests/mock-api.js
git commit -m "feat(frontend): add ranking switcher dropdown and Settings tab to ranking layout"
```

---

## Task 8: Ranking settings page

**Files:**
- Create: `web/src/routes/projects/[id]/rankings/[rid]/settings/+page.server.ts`
- Create: `web/src/routes/projects/[id]/rankings/[rid]/settings/+page.svelte`

The settings page covers: general (name/description), publishing toggle, algorithm config (read-only type label + editable params for algorithmic rankings), recompute button, and danger zone (delete with confirmation).

- [ ] **Step 1: Create `+page.server.ts`**

Create `web/src/routes/projects/[id]/rankings/[rid]/settings/+page.server.ts`:

```typescript
import { error, fail, redirect } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";

export const load: PageServerLoad = async ({ parent }) => {
  const { project } = await parent();
  const role = project.user_role;
  if (role !== "editor" && role !== "owner") {
    error(403, { message: "forbidden" });
  }
  return {};
};

export const actions = {
  save: async ({ request, params, locals }) => {
    const { api } = locals;
    const data = await request.formData();
    const name = ((data.get("name") as string) ?? "").trim();
    const description =
      ((data.get("description") as string) || "").trim() || undefined;
    const publishedRaw = data.get("published");
    const published =
      publishedRaw !== null ? publishedRaw === "true" : undefined;

    if (!name) return fail(422, { saveError: "Name is required" });

    const res = await locals.api.patch(
      `/projects/${params.id}/rankings/${params.rid}`,
      { name, description, published },
    );
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      return fail(res.status, {
        saveError: (body as { message?: string }).message ?? "Save failed",
      });
    }
    return { saved: true };
  },

  saveAlgorithmConfig: async ({ request, params, locals }) => {
    const { api } = locals;
    const data = await request.formData();
    const algorithm = data.get("algorithm") as string;

    let algorithm_config: Record<string, unknown>;
    if (algorithm === "elo") {
      const k = parseFloat(data.get("elo_k") as string);
      const initial = parseFloat(data.get("elo_initial") as string);
      algorithm_config = {
        k_factor: isNaN(k) ? 32 : k,
        initial_rating: isNaN(initial) ? 1500 : initial,
      };
    } else if (algorithm === "glicko2") {
      const tau = parseFloat(data.get("g2_tau") as string);
      const rd = parseFloat(data.get("g2_rd") as string);
      const sigma = parseFloat(data.get("g2_sigma") as string);
      algorithm_config = {
        tau: isNaN(tau) ? 0.5 : tau,
        initial_rd: isNaN(rd) ? 350 : rd,
        initial_volatility: isNaN(sigma) ? 0.06 : sigma,
      };
    } else {
      return fail(422, { algoError: "Invalid algorithm" });
    }

    const patchRes = await api.patch(
      `/projects/${params.id}/rankings/${params.rid}`,
      { algorithm_config },
    );
    if (!patchRes.ok) {
      const body = await patchRes.json().catch(() => ({}));
      return fail(patchRes.status, {
        algoError:
          (body as { message?: string }).message ?? "Failed to save config",
      });
    }

    const recomputeRes = await api.post(
      `/projects/${params.id}/rankings/${params.rid}/recompute`,
    );
    if (!recomputeRes.ok) {
      return fail(recomputeRes.status, {
        algoError: "Config saved but failed to enqueue recompute",
      });
    }

    return { algoSaved: true };
  },

  delete: async ({ params, locals }) => {
    const { api } = locals;
    const res = await api.delete(
      `/projects/${params.id}/rankings/${params.rid}`,
    );
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      return fail(res.status, {
        deleteError:
          (body as { message?: string }).message ?? "Delete failed",
      });
    }
    redirect(303, `/projects/${params.id}`);
  },
} satisfies Actions;
```

- [ ] **Step 2: Create `+page.svelte`**

Create `web/src/routes/projects/[id]/rankings/[rid]/settings/+page.svelte`:

```svelte
<script lang="ts">
  import { untrack } from "svelte";
  import { enhance } from "$app/forms";
  import { makeApi } from "$lib/api";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { Separator } from "$lib/components/ui/separator";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";

  let { data, form } = $props();

  let name = $state(untrack(() => data.ranking.name));
  let description = $state(untrack(() => data.ranking.description ?? ""));
  let published = $state(untrack(() => data.ranking.published));
  $effect(() => {
    name = data.ranking.name;
    description = data.ranking.description ?? "";
    published = data.ranking.published;
  });

  let deleteDialogOpen = $state(false);
  let deleteFormEl = $state<HTMLFormElement | null>(null);

  let recomputeStatus = $state<"idle" | "sending" | "sent" | "error">("idle");

  async function triggerRecompute() {
    recomputeStatus = "sending";
    const api = makeApi(fetch);
    const res = await api.recomputeRanking(data.project.id, data.ranking.id);
    if (res.ok) {
      recomputeStatus = "sent";
      setTimeout(() => (recomputeStatus = "idle"), 3000);
    } else {
      recomputeStatus = "error";
      setTimeout(() => (recomputeStatus = "idle"), 3000);
    }
  }

  // Elo config state
  const eloConfig = $derived(data.ranking.algorithm_config as { k_factor?: number; initial_rating?: number });
  let eloK = $state(untrack(() => String(eloConfig.k_factor ?? 32)));
  let eloInitial = $state(untrack(() => String(eloConfig.initial_rating ?? 1500)));

  // Glicko-2 config state
  const g2Config = $derived(data.ranking.algorithm_config as { tau?: number; initial_rd?: number; initial_volatility?: number });
  let g2Tau = $state(untrack(() => String(g2Config.tau ?? 0.5)));
  let g2Rd = $state(untrack(() => String(g2Config.initial_rd ?? 350)));
  let g2Sigma = $state(untrack(() => String(g2Config.initial_volatility ?? 0.06)));

  function algorithmLabel(a: string | null): string {
    if (!a) return "Manual";
    if (a === "elo") return "Elo";
    if (a === "glicko2") return "Glicko-2";
    return a;
  }

  function algorithmWikiUrl(a: string | null): string | null {
    if (a === "elo") return "https://en.wikipedia.org/wiki/Elo_rating_system";
    if (a === "glicko2") return "https://en.wikipedia.org/wiki/Glicko_rating_system";
    return null;
  }
</script>

<div class="container mx-auto max-w-lg space-y-8 px-4 py-8">

  <!-- General -->
  <div class="space-y-4">
    <h2 class="text-lg font-semibold">General</h2>
    <form method="POST" action="?/save" use:enhance class="space-y-3">
      <div class="flex flex-col gap-1.5">
        <Label for="name">Name</Label>
        <Input id="name" name="name" bind:value={name} required />
      </div>
      <div class="flex flex-col gap-1.5">
        <Label for="description"
          >Description <span class="text-muted-foreground text-sm">(optional)</span></Label
        >
        <Input id="description" name="description" bind:value={description} />
      </div>
      {#if form?.saveError}
        <p class="text-sm text-destructive">{form.saveError}</p>
      {/if}
      {#if form?.saved}
        <p class="text-sm text-green-600">Saved.</p>
      {/if}
      <Button type="submit">Save</Button>
    </form>
  </div>

  <Separator />

  <!-- Publishing -->
  <div class="space-y-3">
    <h2 class="text-lg font-semibold">Publishing</h2>
    <div class="flex items-center justify-between rounded-md border p-3">
      <div>
        <p class="text-sm font-medium">Public</p>
        <p class="text-xs text-muted-foreground">
          Anyone with the link can view stats, H2H, and ranking
        </p>
      </div>
      <form method="POST" action="?/save" use:enhance>
        <input type="hidden" name="name" value={data.ranking.name} />
        <input
          type="hidden"
          name="published"
          value={published ? "false" : "true"}
        />
        <Button type="submit" variant={published ? "default" : "outline"} size="sm">
          {published ? "Public" : "Private"}
        </Button>
      </form>
    </div>
  </div>

  <Separator />

  <!-- Algorithm -->
  <div class="space-y-3">
    <h2 class="text-lg font-semibold">Algorithm</h2>
    <div class="flex items-center gap-2">
      <span class="text-sm font-semibold">{algorithmLabel(data.ranking.algorithm)}</span>
      {#if algorithmWikiUrl(data.ranking.algorithm)}
        <a
          href={algorithmWikiUrl(data.ranking.algorithm)!}
          target="_blank"
          rel="noopener noreferrer"
          class="text-xs text-primary hover:underline"
        >Wikipedia ↗</a>
      {/if}
    </div>
    <p class="text-xs text-muted-foreground">
      Set at creation. Create a new ranking to use a different algorithm.
    </p>

    {#if data.ranking.algorithm === "elo"}
      <form method="POST" action="?/saveAlgorithmConfig" use:enhance class="space-y-3">
        <input type="hidden" name="algorithm" value="elo" />
        <div class="flex items-center justify-between gap-4">
          <div>
            <div class="text-xs font-semibold">K-factor</div>
            <div class="text-xs text-muted-foreground">
              Points at stake per set. 32 is standard; lower (16) = slow changes, higher (64) = fast.
            </div>
          </div>
          <Input
            type="number"
            name="elo_k"
            bind:value={eloK}
            min="1"
            max="256"
            class="w-20 text-right"
          />
        </div>
        <div class="flex items-center justify-between gap-4">
          <div>
            <div class="text-xs font-semibold">Initial rating</div>
            <div class="text-xs text-muted-foreground">
              Starting rating for all players. 1500 is the universal convention.
            </div>
          </div>
          <Input
            type="number"
            name="elo_initial"
            bind:value={eloInitial}
            min="1"
            class="w-20 text-right"
          />
        </div>
        {#if form?.algoError}
          <p class="text-sm text-destructive">{form.algoError}</p>
        {/if}
        {#if form?.algoSaved}
          <p class="text-sm text-green-600">Saved. Recompute queued.</p>
        {/if}
        <Button type="submit">Save &amp; recompute</Button>
      </form>

    {:else if data.ranking.algorithm === "glicko2"}
      <form method="POST" action="?/saveAlgorithmConfig" use:enhance class="space-y-3">
        <input type="hidden" name="algorithm" value="glicko2" />
        <div class="flex items-center justify-between gap-4">
          <div>
            <div class="text-xs font-semibold">τ (tau)</div>
            <div class="text-xs text-muted-foreground">
              Controls volatility change rate. Glickman recommends 0.3–1.2; lower = more stable.
            </div>
          </div>
          <Input
            type="number"
            name="g2_tau"
            bind:value={g2Tau}
            min="0.1"
            max="2"
            step="0.1"
            class="w-20 text-right"
          />
        </div>
        <div class="flex items-center justify-between gap-4">
          <div>
            <div class="text-xs font-semibold">Initial RD</div>
            <div class="text-xs text-muted-foreground">
              Rating deviation for a new player. Shrinks as they play more sets.
            </div>
          </div>
          <Input
            type="number"
            name="g2_rd"
            bind:value={g2Rd}
            min="50"
            max="700"
            class="w-20 text-right"
          />
        </div>
        <div class="flex items-center justify-between gap-4">
          <div>
            <div class="text-xs font-semibold">Initial volatility (σ)</div>
            <div class="text-xs text-muted-foreground">
              Expected rating fluctuation for a new player. Glickman recommends 0.06.
            </div>
          </div>
          <Input
            type="number"
            name="g2_sigma"
            bind:value={g2Sigma}
            min="0.01"
            max="1"
            step="0.01"
            class="w-20 text-right"
          />
        </div>
        {#if form?.algoError}
          <p class="text-sm text-destructive">{form.algoError}</p>
        {/if}
        {#if form?.algoSaved}
          <p class="text-sm text-green-600">Saved. Recompute queued.</p>
        {/if}
        <Button type="submit">Save &amp; recompute</Button>
      </form>
    {/if}
  </div>

  <Separator />

  <!-- Recompute (algorithmic rankings only) -->
  {#if data.ranking.algorithm}
    <div class="space-y-2">
      <h2 class="text-lg font-semibold">Recompute</h2>
      <p class="text-sm text-muted-foreground">
        Manually trigger a recalculation. This happens automatically after imports and event inclusion changes.
      </p>
      <Button
        variant="outline"
        onclick={triggerRecompute}
        disabled={recomputeStatus === "sending"}
      >
        {#if recomputeStatus === "sending"}
          Sending…
        {:else if recomputeStatus === "sent"}
          Queued ✓
        {:else if recomputeStatus === "error"}
          Failed
        {:else}
          Recompute now
        {/if}
      </Button>
    </div>

    <Separator />
  {/if}

  <!-- Danger zone -->
  <div class="space-y-3">
    <h2 class="text-lg font-semibold text-destructive">Danger zone</h2>
    <div class="flex items-center justify-between rounded-md border border-destructive/40 p-4">
      <div>
        <p class="font-medium">Delete this ranking</p>
        <p class="text-sm text-muted-foreground">
          Removes all players, event inclusion, and computed stats.
        </p>
      </div>
      <form
        method="POST"
        action="?/delete"
        use:enhance
        bind:this={deleteFormEl}
        class="ml-4"
      >
        <Button
          type="button"
          variant="destructive"
          size="sm"
          onclick={() => (deleteDialogOpen = true)}
        >
          Delete
        </Button>
      </form>
    </div>
    {#if form?.deleteError}
      <p class="text-sm text-destructive">{form.deleteError}</p>
    {/if}
  </div>
</div>

<AlertDialog.Root bind:open={deleteDialogOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Delete "{data.ranking.name}"?</AlertDialog.Title>
      <AlertDialog.Description>
        Removes all players, event inclusion, and computed stats for this ranking. This cannot be undone.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
        onclick={() => {
          deleteDialogOpen = false;
          deleteFormEl?.requestSubmit();
        }}
      >
        Delete ranking
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
```

- [ ] **Step 3: Add recompute mock endpoint to mock-api.js**

In `web/tests/mock-api.js`, add a handler for `POST /projects/:id/rankings/:rid/recompute`:

```javascript
// In the request handler:
if (method === 'POST' && pathParts[3] === 'rankings' && pathParts[5] === 'recompute') {
  res.writeHead(202); res.end();
  return;
}
```

Also add `POST /projects/:id/rankings/:rid` (PATCH for the settings save action) and `DELETE /projects/:id/rankings/:rid`.

- [ ] **Step 4: Type check and e2e tests**

```bash
cd web && npx tsc --noEmit && npm run test:e2e
```

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/projects/ web/tests/mock-api.js
git commit -m "feat(frontend): add ranking settings page (name, publishing, algorithm config, delete)"
```

---

## Task 9: Ranking tab — algorithmic display with "Sync to algorithm"

**Files:**
- Modify: `web/src/routes/projects/[id]/rankings/[rid]/ranking/+page.server.ts`
- Modify: `web/src/routes/projects/[id]/rankings/[rid]/ranking/+page.svelte`
- Modify: `web/tests/mock-api.js`

For algorithmic rankings, the Ranking tab shows `computed_rating` and `display_data` alongside the existing drag-to-reorder. A "Sync to algorithm" button resets `rank_position` to match the computed order. A delta badge shows when a player's rank_position diverges from their computed order.

The backend endpoint `GET /projects/:id/rankings/:rid/ranking` returns `ComputedRankingPlayerResponse { player_id, name, rank_position, notes, computed_rating, display_data }`. We switch the page to always use this endpoint so we get `computed_rating` for free on algorithmic rankings.

- [ ] **Step 1: Update `+page.server.ts` to call `/ranking` endpoint**

Replace `web/src/routes/projects/[id]/rankings/[rid]/ranking/+page.server.ts` with:

```typescript
import type { PageServerLoad } from "./$types";
import type { RankingPlayerWithScore, PlayerStats } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const [playersRes, statsRes] = await Promise.all([
    api.get(`/projects/${params.id}/rankings/${params.rid}/ranking`),
    api.get(`/projects/${params.id}/rankings/${params.rid}/stats`),
  ]);
  const players: RankingPlayerWithScore[] = playersRes.ok
    ? await playersRes.json()
    : [];
  const stats: PlayerStats[] = statsRes.ok ? await statsRes.json() : [];
  return { players, stats };
};
```

- [ ] **Step 2: Update `+page.svelte` to handle algorithmic display**

Replace `web/src/routes/projects/[id]/rankings/[rid]/ranking/+page.svelte` with:

```svelte
<script lang="ts">
  import { untrack } from "svelte";
  import { dragHandleZone, dragHandle } from "svelte-dnd-action";
  import type { DndEvent } from "svelte-dnd-action";
  import { makeApi } from "$lib/api";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import * as Empty from "$lib/components/ui/empty";
  import type { RankingPlayerWithScore, PlayerStats } from "$lib/types";
  import { winRate } from "$lib/utils";

  let { data } = $props();

  type RankItem = {
    id: string;
    name: string;
    computed_rating: number | null;
    display_data: Record<string, unknown> | null;
  };

  // Sort by rank_position ascending for the initial drag order
  const sortedPlayers = $derived(
    [...(data.players as RankingPlayerWithScore[])].sort(
      (a, b) => a.rank_position - b.rank_position,
    ),
  );

  // Computed order: players sorted by computed_rating DESC (for delta badge)
  const computedOrder = $derived(
    data.ranking.algorithm
      ? [...(data.players as RankingPlayerWithScore[])]
          .filter((p) => p.computed_rating !== null)
          .sort((a, b) => (b.computed_rating ?? 0) - (a.computed_rating ?? 0))
          .map((p) => p.player_id)
      : [],
  );

  let items = $state<RankItem[]>(
    untrack(() =>
      sortedPlayers.map((p) => ({
        id: p.player_id,
        name: p.name,
        computed_rating: p.computed_rating ?? null,
        display_data: (p.display_data as Record<string, unknown>) ?? null,
      })),
    ),
  );
  let savedIds = $state<string[]>(
    untrack(() => sortedPlayers.map((p) => p.player_id)),
  );

  const statsMap = $derived<Record<string, PlayerStats>>(
    Object.fromEntries(
      (data.stats as PlayerStats[]).map((s) => [s.player_id, s]),
    ),
  );

  const hasChanges = $derived(
    items.length !== savedIds.length ||
      items.some((item, i) => item.id !== savedIds[i]),
  );

  const canEdit = $derived(
    data.project.user_role === "editor" || data.project.user_role === "owner",
  );

  const isAlgorithmic = $derived(!!data.ranking.algorithm);

  let saveStatus = $state<"idle" | "saving" | "saved">("idle");

  function handleConsider(e: CustomEvent<DndEvent<RankItem>>) {
    items = e.detail.items;
  }

  function handleFinalize(e: CustomEvent<DndEvent<RankItem>>) {
    items = e.detail.items;
  }

  let editingId = $state<string | null>(null);
  let editingValue = $state("");
  let editInput = $state<HTMLInputElement | null>(null);

  $effect(() => {
    if (editingId && editInput) {
      editInput.focus();
      editInput.select();
    }
  });

  function startEdit(id: string, rank: number) {
    editingId = id;
    editingValue = String(rank);
  }

  function commitEdit() {
    if (!editingId) return;
    const n = parseInt(editingValue, 10);
    if (!isNaN(n)) {
      const clamped = Math.max(1, Math.min(n, items.length));
      const idx = items.findIndex((i) => i.id === editingId);
      if (idx !== -1) {
        const copy = [...items];
        const [item] = copy.splice(idx, 1);
        copy.splice(clamped - 1, 0, item);
        items = copy;
      }
    }
    editingId = null;
  }

  function onRankKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") commitEdit();
    if (e.key === "Escape") editingId = null;
  }

  async function save() {
    saveStatus = "saving";
    const api = makeApi(fetch);
    const res = await api.putRanking(
      data.project.id,
      data.ranking.id,
      items.map((i) => i.id),
    );
    if (res.ok) {
      savedIds = items.map((i) => i.id);
      saveStatus = "saved";
      setTimeout(() => {
        saveStatus = "idle";
      }, 2000);
    } else {
      saveStatus = "idle";
    }
  }

  async function syncToAlgorithm() {
    // Reorder items by computed_rating DESC (same order as computedOrder)
    const ratingMap = new Map(
      items.map((item) => [item.id, item.computed_rating ?? -Infinity]),
    );
    items = [...items].sort(
      (a, b) => (ratingMap.get(b.id) ?? 0) - (ratingMap.get(a.id) ?? 0),
    );
    await save();
  }

  // Delta: how many positions does a player diverge from computed order?
  function computedDelta(
    playerId: string,
    currentIndex: number,
  ): number | null {
    if (!isAlgorithmic || computedOrder.length === 0) return null;
    const computedIndex = computedOrder.indexOf(playerId);
    if (computedIndex === -1) return null;
    return computedIndex - currentIndex; // positive = player should be higher
  }

  function wlRecord(s: PlayerStats | undefined): string {
    if (!s) return "";
    return `${s.wins.length}W · ${s.losses.length}L`;
  }

  function isMoved(id: string, currentIndex: number): boolean {
    return hasChanges && savedIds[currentIndex] !== id;
  }

  function formatRating(item: RankItem): string {
    if (item.computed_rating === null) return "";
    if (item.display_data?.rd !== undefined) {
      return `${Math.round(item.computed_rating)} ± ${Math.round(item.display_data.rd as number)}`;
    }
    return String(Math.round(item.computed_rating));
  }
</script>

{#if data.players.length === 0}
  <Empty.Root>
    <Empty.Header>
      <Empty.Title>No players</Empty.Title>
      <Empty.Description
        >Add players to start building your ranking.</Empty.Description
      >
    </Empty.Header>
  </Empty.Root>
{:else}
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <h2 class="text-lg font-semibold">Ranking</h2>
      {#if canEdit}
        <div class="flex items-center gap-3">
          {#if isAlgorithmic}
            <Button
              variant="outline"
              size="sm"
              onclick={syncToAlgorithm}
              disabled={saveStatus === "saving"}
            >
              Sync to algorithm
            </Button>
          {/if}
          {#if hasChanges && saveStatus !== "saved"}
            <span class="text-sm text-muted-foreground">Unsaved changes</span>
          {/if}
          <Button
            onclick={save}
            disabled={!hasChanges || saveStatus === "saving"}
            size="sm"
            variant={saveStatus === "saved" ? "outline" : "default"}
          >
            {saveStatus === "saving"
              ? "Saving…"
              : saveStatus === "saved"
                ? "Saved ✓"
                : "Save"}
          </Button>
        </div>
      {/if}
    </div>

    {#if canEdit}
      <div
        class="flex max-w-xl flex-col gap-1"
        use:dragHandleZone={{ items, flipDurationMs: 0 }}
        onconsider={handleConsider}
        onfinalize={handleFinalize}
      >
        {#each items as item, i (item.id)}
          {@const s = statsMap[item.id]}
          {@const moved = isMoved(item.id, i)}
          {@const delta = computedDelta(item.id, i)}
          <div
            class="flex items-center gap-3 rounded-md border px-3 py-2.5 text-sm transition-colors
              {moved ? 'border-primary/40 bg-primary/5' : 'bg-card'}"
          >
            <span
              use:dragHandle
              class="cursor-grab select-none text-base text-muted-foreground active:cursor-grabbing"
            >
              ⠿
            </span>

            {#if editingId === item.id}
              <Input
                bind:ref={editInput}
                type="number"
                class="h-7 w-12 px-1 text-center text-xs [appearance:textfield]"
                bind:value={editingValue}
                onblur={commitEdit}
                onkeydown={onRankKeydown}
              />
            {:else}
              <Button
                variant="ghost"
                size="sm"
                class="h-7 w-8 rounded p-0 text-xs font-normal text-muted-foreground"
                onclick={() => startEdit(item.id, i + 1)}
              >
                {i + 1}
              </Button>
            {/if}

            <a
              href="/projects/{data.project.id}/players/{item.id}"
              class="flex-1 font-semibold hover:underline">{item.name}</a
            >

            {#if isAlgorithmic && item.computed_rating !== null}
              <span class="text-xs font-semibold text-primary tabular-nums">
                {formatRating(item)}
              </span>
              {#if delta !== null && delta !== 0}
                <span
                  class="min-w-[28px] text-right text-xs tabular-nums
                    {delta > 0 ? 'text-green-600' : 'text-red-500'}"
                >
                  {delta > 0 ? `↑${delta}` : `↓${Math.abs(delta)}`}
                </span>
              {/if}
            {:else if s}
              <span class="text-xs text-muted-foreground">{wlRecord(s)}</span>
              <span class="min-w-[36px] text-right text-xs font-semibold"
                >{winRate(s.wins.length, s.losses.length)}</span
              >
            {/if}
          </div>
        {/each}
      </div>
    {:else}
      <div class="flex max-w-xl flex-col gap-1">
        {#each items as item, i (item.id)}
          {@const s = statsMap[item.id]}
          {@const delta = computedDelta(item.id, i)}
          <div
            class="flex items-center gap-3 rounded-md border bg-card px-3 py-2.5 text-sm"
          >
            <span class="w-8 text-center text-xs text-muted-foreground"
              >{i + 1}</span
            >
            <a
              href="/projects/{data.project.id}/players/{item.id}"
              class="flex-1 font-semibold hover:underline">{item.name}</a
            >
            {#if isAlgorithmic && item.computed_rating !== null}
              <span class="text-xs font-semibold text-primary tabular-nums">
                {formatRating(item)}
              </span>
              {#if delta !== null && delta !== 0}
                <span
                  class="min-w-[28px] text-right text-xs tabular-nums
                    {delta > 0 ? 'text-green-600' : 'text-red-500'}"
                >
                  {delta > 0 ? `↑${delta}` : `↓${Math.abs(delta)}`}
                </span>
              {/if}
            {:else if s}
              <span class="text-xs text-muted-foreground">{wlRecord(s)}</span>
              <span class="min-w-[36px] text-right text-xs font-semibold"
                >{winRate(s.wins.length, s.losses.length)}</span
              >
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    {#if canEdit}
      <p class="text-xs text-muted-foreground">
        {#if isAlgorithmic}
          Drag ⠿ to reorder · "Sync to algorithm" resets order to computed ratings · Save to persist
        {:else}
          Click the rank number to edit · Drag ⠿ to reorder · Click Save to persist
        {/if}
      </p>
    {/if}
  </div>
{/if}
```

- [ ] **Step 3: Update mock-api.js `/ranking` endpoint**

In `web/tests/mock-api.js`, the mock handler for `GET /projects/:id/rankings/:rid/ranking` should return `MOCK_RANKING_PLAYERS` with algorithm fields added:

```javascript
const MOCK_RANKING_PLAYERS = [
  { player_id: 'player-1', name: 'Alice', rank_position: 1, notes: null, computed_rating: null, display_data: null },
  { player_id: 'player-2', name: 'Bob', rank_position: 2, notes: null, computed_rating: null, display_data: null },
  { player_id: 'player-3', name: 'Charlie', rank_position: 3, notes: null, computed_rating: null, display_data: null },
];
```

Check that the mock handler matches `GET /projects/:id/rankings/:rid/ranking` (the 5th path segment being "ranking") and doesn't accidentally match other patterns.

- [ ] **Step 4: Type check and tests**

```bash
cd web && npx tsc --noEmit && npm run test:e2e
```

Expected: all pass.

- [ ] **Step 5: Format and commit**

```bash
cd web && npm run format
git add web/src/routes/projects/ web/tests/mock-api.js
git commit -m "feat(frontend): algorithmic ranking display with computed ratings, delta badges, and Sync to algorithm"
```

---

## Task 10: Final integration check

- [ ] **Step 1: Run the full test suite**

```bash
bash test.sh
```

Expected: all backend and frontend tests pass.

- [ ] **Step 2: Format all code**

```bash
cd backend && cargo fmt --all
cd web && npm run format
git add -A
git diff --cached --stat
```

Check that only formatting changes remain (if any).

- [ ] **Step 3: Commit any formatting fixes**

```bash
git commit -m "chore: format after ranking UI implementation" || echo "nothing to commit"
```

---

## Self-Review

### Spec coverage check

| Spec section | Covered by task |
|---|---|
| Route restructure — `(hub)` group | Task 4 |
| Single-ranking redirect removed | Task 5 |
| Rankings tab active state fix | Task 4 (by removing project tab bar from ranking view chain) |
| Rankings list: algorithm label + player count | Task 5 |
| "+ New ranking" button | Task 5 (already exists, preserved) |
| Ranking breadcrumb switcher | Task 7 |
| Settings tab in ranking layout | Task 7 |
| New ranking: algorithm picker | Task 6 |
| New ranking: Elo config | Task 6 |
| New ranking: Glicko-2 config | Task 6 |
| Wikipedia links in algorithm picker | Task 6 |
| Ranking settings: General | Task 8 |
| Ranking settings: Publishing toggle | Task 8 |
| Ranking settings: Algorithm type (read-only) | Task 8 |
| Ranking settings: Algorithm config (editable) | Task 8 |
| Ranking settings: Save & recompute | Task 8 |
| Ranking settings: Recompute now button | Task 8 |
| Ranking settings: Danger zone + delete | Task 8 |
| Ranking tab: drag UI (manual, unchanged) | Task 9 (preserved) |
| Ranking tab: computed_rating badge | Task 9 |
| Ranking tab: Glicko-2 `rating ± RD` format | Task 9 |
| Ranking tab: delta badge (↑2/↓1) | Task 9 |
| Ranking tab: "Sync to algorithm" button | Task 9 |
| Backend: seed rank_position on first compute | Task 3 |
| Frontend types: algorithm fields on Ranking | Task 1 |
| Frontend types: RankingPlayerWithScore | Task 1 |
| API client: recomputeRanking | Task 1 |
| Backend player_count in rankings list | Task 2 |

### Potential issues

1. **`(hub)` move and `$types` imports:** After moving files, SvelteKit regenerates `$types` files. If any moved file imports `PageData` or `LayoutData` from `"./$types"`, the generated types will be correct because SvelteKit generates them per-file. No manual updates needed.

2. **The ranking layout already uses `startsWith`:** The current `+layout.svelte` for rankings already detects active tabs with `startsWith`. No fix needed there — but the Settings tab is absent, which is why it needs to be added.

3. **Player count cast:** The `+page.svelte` for the rankings list accesses `(ranking as any).player_count`. Once the `Ranking` interface is updated to include `player_count`, remove the `as any` cast and add `player_count: number` to the interface. (The backend sends it as `i64` which is JSON `number`.)

4. **`PATCH /projects/:id/rankings/:rid` mock:** The settings page save action sends a PATCH. The mock-api.js needs a handler for PATCH to rankings. Check if it exists; if not, add:

```javascript
if (method === 'PATCH' && pathParts[3] === 'rankings' && pathParts[4] && pathParts[5] === undefined) {
  res.writeHead(200, { 'Content-Type': 'application/json' });
  // Return the updated ranking (mock just returns the existing one)
  const rankings = MOCK_RANKINGS[pathParts[2]] || [];
  res.end(JSON.stringify(rankings.find(r => r.id === pathParts[4]) || rankings[0]));
  return;
}
```

5. **`DELETE /projects/:id/rankings/:rid` mock:** Add if missing:

```javascript
if (method === 'DELETE' && pathParts[3] === 'rankings' && pathParts[4]) {
  res.writeHead(204); res.end();
  return;
}
```
