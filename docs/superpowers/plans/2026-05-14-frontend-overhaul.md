# Frontend Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove Accumulated UF, redesign Stats page as player cards, add a shared set-detail modal, add an H2H side panel with drill-down, and fix account-linking auto-refresh.

**Architecture:** Backend enriches `SetRecord` with tournament/event/round/seed/VOD fields and a new `GET /projects/{id}/head-to-head/{pid_a}/{pid_b}/sets` endpoint. Frontend introduces a shared `SetDetailModal` component (shadcn Dialog) used in both Stats and H2H. Stats page becomes a responsive card grid; H2H page gains a side panel fetched client-side on cell click.

**Tech Stack:** Rust/Axum/sqlx (backend), SvelteKit 2 + Svelte 5 runes, Tailwind CSS, shadcn/svelte (bits-ui Dialog), Vitest + @testing-library/svelte (frontend unit tests), sqlx offline mode (`.sqlx/` cache).

---

## File Map

| File | Action |
|---|---|
| `backend/crates/api/src/routes/tournaments.rs` | Modify: `SetRecord` struct, `SetRow` struct, stats SQL query, sort order, new `H2HSet`/`H2HSetPath` structs, new `get_h2h_sets` handler, router |
| `backend/crates/e2e/tests/full_flow.rs` | Modify: assert enriched fields in stats response; add H2H sets endpoint test |
| `web/src/lib/types.ts` | Modify: extend `SetRecord`; add `H2HSet` |
| `web/src/lib/components/SetDetailModal.svelte` | **Create** |
| `web/src/lib/components/SetDetailModal.test.ts` | **Create** |
| `web/src/routes/projects/[id]/stats/+page.svelte` | Modify: full rewrite |
| `web/src/routes/projects/[id]/stats/stats.test.ts` | Modify: full rewrite |
| `web/src/routes/projects/[id]/h2h/+page.svelte` | Modify: add side panel + set-detail modal |
| `web/src/routes/projects/[id]/h2h/h2h.test.ts` | Modify: add cell-click + panel tests |
| `web/src/routes/projects/[id]/players/+page.svelte` | Modify: add `invalidateAll()` to enhance callbacks |

---

## Task 1: Backend — Enrich `SetRecord` with tournament/event/round/seed/VOD fields

**Files:**
- Modify: `backend/crates/api/src/routes/tournaments.rs`
- Modify: `backend/crates/e2e/tests/full_flow.rs`

- [ ] **Step 1: Add test assertions for enriched fields**

In `full_flow.rs`, find the stats assertions block (after `let armada_stats = ...`) and add:

```rust
// Enriched fields — these will fail until Task 1 is implemented
assert_eq!(armada_stats["wins"][0]["tournament_name"], json!("Test Tournament"));
assert_eq!(armada_stats["wins"][0]["tournament_slug"], json!("tournament/test-2024"));
assert_eq!(armada_stats["wins"][0]["event_name"], json!("Melee Singles"));
assert_eq!(armada_stats["wins"][0]["round_name"], json!("Round 1"));
assert_eq!(armada_stats["wins"][0]["winner_seed"], json!(7));
assert_eq!(armada_stats["wins"][0]["loser_seed"], json!(2));
assert_eq!(armada_stats["wins"][0]["is_dq"], json!(false));
assert_eq!(armada_stats["wins"][0]["startgg_set_id"], json!(4001_i64));
```

- [ ] **Step 2: Update `SetRecord` struct** in `tournaments.rs` (currently ends at line ~57):

```rust
#[derive(Serialize)]
pub struct SetRecord {
    pub opponent_id: Uuid,
    pub opponent_name: String,
    pub upset_factor: i64,
    pub winner_score: Option<i16>,
    pub loser_score: Option<i16>,
    pub tournament_name: String,
    pub tournament_slug: String,
    pub event_name: String,
    pub round_name: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
    pub is_dq: bool,
    pub vod_url: Option<String>,
    pub startgg_set_id: i64,
    pub winner_seed: Option<i32>,
    pub loser_seed: Option<i32>,
}
```

- [ ] **Step 3: Update `SetRow` struct** inside `get_stats` (currently ends around line ~290):

```rust
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
    tournament_slug: String,
    round_name: Option<String>,
    completed_at: Option<DateTime<Utc>>,
    is_dq: bool,
    vod_url: Option<String>,
    startgg_set_id: i64,
}
```

- [ ] **Step 4: Update the stats SQL query** (replace the existing `sqlx::query_as!` block in `get_stats`):

```rust
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
        t.slug                             AS tournament_slug,
        s.round_name,
        s.completed_at,
        s.is_dq,
        s.vod_url,
        s.startgg_set_id
    FROM sets s
    JOIN entrants we ON we.id = s.winner_entrant_id
    JOIN entrants le ON le.id = s.loser_entrant_id
    LEFT JOIN players wp ON wp.id = we.player_id AND wp.project_id = $1
    LEFT JOIN players lp ON lp.id = le.player_id AND lp.project_id = $1
    JOIN project_events pe ON pe.event_id = s.event_id AND pe.project_id = $1
    JOIN events e ON e.id = s.event_id
    JOIN tournaments t ON t.id = e.tournament_id
    WHERE pe.included = true
      AND s.is_dq    = false
      AND (wp.id IS NOT NULL OR lp.id IS NOT NULL)
    "#,
    project_id,
)
.fetch_all(&state.db)
.await?;
```

- [ ] **Step 5: Update both `SetRecord` construction blocks** in the `for row in sets` loop:

Replace the first block (winner push):
```rust
entry.1.push(SetRecord {
    opponent_id: loser_opp_id,
    opponent_name: row.loser_name.clone(),
    upset_factor: uf,
    winner_score: row.winner_score,
    loser_score: row.loser_score,
    tournament_name: row.tournament_name.clone(),
    tournament_slug: row.tournament_slug.clone(),
    event_name: row.event_name.clone(),
    round_name: row.round_name.clone(),
    completed_at: row.completed_at,
    is_dq: row.is_dq,
    vod_url: row.vod_url.clone(),
    startgg_set_id: row.startgg_set_id,
    winner_seed: row.winner_seed,
    loser_seed: row.loser_seed,
});
```

Replace the second block (loser push):
```rust
entry.2.push(SetRecord {
    opponent_id: winner_opp_id,
    opponent_name: row.winner_name.clone(),
    upset_factor: uf,
    winner_score: row.winner_score,
    loser_score: row.loser_score,
    tournament_name: row.tournament_name.clone(),
    tournament_slug: row.tournament_slug.clone(),
    event_name: row.event_name.clone(),
    round_name: row.round_name.clone(),
    completed_at: row.completed_at,
    is_dq: row.is_dq,
    vod_url: row.vod_url.clone(),
    startgg_set_id: row.startgg_set_id,
    winner_seed: row.winner_seed,
    loser_seed: row.loser_seed,
});
```

---

## Task 2: Backend — Replace stats sort with win rate

**Files:**
- Modify: `backend/crates/api/src/routes/tournaments.rs`

- [ ] **Step 1: Replace the sort block** (lines ~374–378 in `get_stats`):

```rust
resp.sort_by(|a, b| {
    let a_total = a.wins.len() + a.losses.len();
    let b_total = b.wins.len() + b.losses.len();
    let a_rate = if a_total == 0 { -1.0_f64 } else { a.wins.len() as f64 / a_total as f64 };
    let b_rate = if b_total == 0 { -1.0_f64 } else { b.wins.len() as f64 / b_total as f64 };
    b_rate
        .partial_cmp(&a_rate)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then(b.wins.len().cmp(&a.wins.len()))
});
```

---

## Task 3: Backend — Add H2H sets endpoint

**Files:**
- Modify: `backend/crates/api/src/routes/tournaments.rs`
- Modify: `backend/crates/e2e/tests/full_flow.rs`

- [ ] **Step 1: Add test for new endpoint** in `full_flow.rs`, after the existing H2H assertions:

```rust
// ── H2H sets endpoint ─────────────────────────────────────────────────────
let resp = get_req(
    &app,
    &format!("/projects/{project_id}/head-to-head/{mango_id}/{armada_id}/sets"),
    &cookie,
)
.await;
assert_eq!(resp.status(), StatusCode::OK);
let sets_body = read_json(resp).await;
let sets_arr = sets_body.as_array().unwrap();
assert_eq!(sets_arr.len(), 1);
// pid_a = mango_id, mango lost → is_win = false
assert_eq!(sets_arr[0]["is_win"], json!(false));
assert_eq!(sets_arr[0]["tournament_name"], json!("Test Tournament"));
assert_eq!(sets_arr[0]["event_name"], json!("Melee Singles"));
assert_eq!(sets_arr[0]["round_name"], json!("Round 1"));
assert_eq!(sets_arr[0]["opponent_name"], json!("Armada"));
```

- [ ] **Step 2: Add `H2HSetPath` and `H2HSet` structs** in `tournaments.rs` (after the existing `ProjectEventPath` struct):

```rust
#[derive(Deserialize)]
pub struct H2HSetPath {
    pub id: Uuid,
    pub pid_a: Uuid,
    pub pid_b: Uuid,
}

#[derive(Serialize)]
pub struct H2HSet {
    #[serde(flatten)]
    pub set: SetRecord,
    pub is_win: bool,
}
```

- [ ] **Step 3: Add `get_h2h_sets` handler** in `tournaments.rs` (after `get_head_to_head`):

```rust
pub async fn get_h2h_sets(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<H2HSetPath>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, path.id, user.id).await?;

    struct H2HSetRow {
        winner_player_id: Uuid,
        winner_name: String,
        winner_seed: Option<i32>,
        loser_player_id: Uuid,
        loser_name: String,
        loser_seed: Option<i32>,
        winner_score: Option<i16>,
        loser_score: Option<i16>,
        event_name: String,
        tournament_name: String,
        tournament_slug: String,
        round_name: Option<String>,
        completed_at: Option<DateTime<Utc>>,
        is_dq: bool,
        vod_url: Option<String>,
        startgg_set_id: i64,
    }

    let rows = sqlx::query_as!(
        H2HSetRow,
        r#"
        SELECT
            we.player_id                       AS "winner_player_id!: Uuid",
            COALESCE(wp.name, we.display_name) AS "winner_name!",
            we.seed                            AS winner_seed,
            le.player_id                       AS "loser_player_id!: Uuid",
            COALESCE(lp.name, le.display_name) AS "loser_name!",
            le.seed                            AS loser_seed,
            s.winner_score,
            s.loser_score,
            e.name                             AS event_name,
            t.name                             AS tournament_name,
            t.slug                             AS tournament_slug,
            s.round_name,
            s.completed_at,
            s.is_dq,
            s.vod_url,
            s.startgg_set_id
        FROM sets s
        JOIN entrants we ON we.id = s.winner_entrant_id
        JOIN entrants le ON le.id = s.loser_entrant_id
        JOIN players  wp ON wp.id = we.player_id AND wp.project_id = $1
        JOIN players  lp ON lp.id = le.player_id AND lp.project_id = $1
        JOIN events   e  ON e.id  = s.event_id
        JOIN tournaments t ON t.id = e.tournament_id
        JOIN project_events pe ON pe.event_id = s.event_id AND pe.project_id = $1
        WHERE pe.included = true
          AND s.is_dq = false
          AND (
              (we.player_id = $2 AND le.player_id = $3)
           OR (we.player_id = $3 AND le.player_id = $2)
          )
        ORDER BY s.completed_at DESC NULLS LAST
        "#,
        path.id,
        path.pid_a,
        path.pid_b,
    )
    .fetch_all(&state.db)
    .await?;

    let sets: Vec<H2HSet> = rows
        .into_iter()
        .map(|row| {
            let uf = match (row.winner_seed, row.loser_seed) {
                (Some(ws), Some(ls)) => set_upset_factor(ws, ls) as i64,
                _ => 0,
            };
            let is_win = row.winner_player_id == path.pid_a;
            let (opponent_id, opponent_name) = if is_win {
                (row.loser_player_id, row.loser_name)
            } else {
                (row.winner_player_id, row.winner_name)
            };
            H2HSet {
                is_win,
                set: SetRecord {
                    opponent_id,
                    opponent_name,
                    upset_factor: uf,
                    winner_score: row.winner_score,
                    loser_score: row.loser_score,
                    tournament_name: row.tournament_name,
                    tournament_slug: row.tournament_slug,
                    event_name: row.event_name,
                    round_name: row.round_name,
                    completed_at: row.completed_at,
                    is_dq: row.is_dq,
                    vod_url: row.vod_url,
                    startgg_set_id: row.startgg_set_id,
                    winner_seed: row.winner_seed,
                    loser_seed: row.loser_seed,
                },
            }
        })
        .collect();

    Ok(Json(sets))
}
```

- [ ] **Step 4: Register the route** in the `router()` function at the bottom of `tournaments.rs`:

```rust
pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, patch};
    axum::Router::new()
        .route("/tournaments", get(list_tournaments))
        .route("/events/{eid}", patch(patch_event))
        .route("/stats", get(get_stats))
        .route("/head-to-head", get(get_head_to_head))
        .route("/head-to-head/{pid_a}/{pid_b}/sets", get(get_h2h_sets))
}
```

---

## Task 4: Backend — Regenerate sqlx cache and verify all tests pass

**Files:**
- `backend/.sqlx/` (regenerated by script)

- [ ] **Step 1: Run prepare-sqlx.sh**

```bash
cd /path/to/repo && bash backend/prepare-sqlx.sh
```

Expected: ends with `query data written to .sqlx/` and no errors.

- [ ] **Step 2: Run full backend test suite**

```bash
bash backend/test.sh
```

Expected: all tests pass including `full_import_flow`.

- [ ] **Step 3: Commit backend changes**

```bash
git add backend/crates/api/src/routes/tournaments.rs \
        backend/crates/e2e/tests/full_flow.rs \
        backend/.sqlx/
git commit -m "feat(api): enrich SetRecord fields, add H2H sets endpoint, sort stats by win rate"
```

---

## Task 5: Frontend — Update types

**Files:**
- Modify: `web/src/lib/types.ts`

- [ ] **Step 1: Replace `SetRecord` and add `H2HSet`** in `types.ts`:

```typescript
export interface SetRecord {
	opponent_id: string;
	opponent_name: string;
	upset_factor: number;
	winner_score: number | null;
	loser_score: number | null;
	tournament_name: string;
	tournament_slug: string;
	event_name: string;
	round_name: string | null;
	completed_at: string | null;
	is_dq: boolean;
	vod_url: string | null;
	startgg_set_id: number;
	winner_seed: number | null;
	loser_seed: number | null;
}

export interface H2HSet extends SetRecord {
	is_win: boolean;
}
```

- [ ] **Step 2: Run frontend unit tests to confirm no type regressions**

```bash
cd web && npm run test:unit
```

Expected: all existing tests pass (test fixtures use partial data; TypeScript strict mode is not enforced in `.test.ts` files with jsdom setup).

- [ ] **Step 3: Commit**

```bash
git add web/src/lib/types.ts
git commit -m "feat(web): extend SetRecord with enriched fields, add H2HSet type"
```

---

## Task 6: Frontend — Create `SetDetailModal` component

**Files:**
- Create: `web/src/lib/components/SetDetailModal.svelte`
- Create: `web/src/lib/components/SetDetailModal.test.ts`

- [ ] **Step 1: Write the failing test** at `web/src/lib/components/SetDetailModal.test.ts`:

```typescript
import { render, screen } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import SetDetailModal from './SetDetailModal.svelte';
import type { SetRecord } from '$lib/types';

const baseSet: SetRecord = {
	opponent_id: 'p2',
	opponent_name: 'Bob',
	upset_factor: 2,
	winner_score: 3,
	loser_score: 1,
	tournament_name: 'Genesis 9',
	tournament_slug: 'tournament/genesis-9',
	event_name: 'Melee Singles',
	round_name: 'Winners Finals',
	completed_at: '2024-01-20T18:00:00Z',
	is_dq: false,
	vod_url: null,
	startgg_set_id: 12345,
	winner_seed: 1,
	loser_seed: 12,
};

describe('SetDetailModal', () => {
	it('renders nothing when set is null', () => {
		render(SetDetailModal, {
			props: { set: null, isWin: false, currentPlayerName: 'Alice', onClose: () => {} }
		});
		expect(screen.queryByText('Genesis 9')).not.toBeInTheDocument();
	});

	it('shows player names in title', () => {
		render(SetDetailModal, {
			props: { set: baseSet, isWin: true, currentPlayerName: 'Alice', onClose: () => {} }
		});
		expect(screen.getByText('Alice vs Bob')).toBeInTheDocument();
	});

	it('shows Win with score from winner perspective', () => {
		render(SetDetailModal, {
			props: { set: baseSet, isWin: true, currentPlayerName: 'Alice', onClose: () => {} }
		});
		expect(screen.getByText(/Win · 3–1/)).toBeInTheDocument();
	});

	it('shows Loss with score from loser perspective', () => {
		render(SetDetailModal, {
			props: { set: baseSet, isWin: false, currentPlayerName: 'Alice', onClose: () => {} }
		});
		expect(screen.getByText(/Loss · 1–3/)).toBeInTheDocument();
	});

	it('shows tournament, event and round', () => {
		render(SetDetailModal, {
			props: { set: baseSet, isWin: true, currentPlayerName: 'Alice', onClose: () => {} }
		});
		expect(screen.getByText('Genesis 9')).toBeInTheDocument();
		expect(screen.getByText('Melee Singles')).toBeInTheDocument();
		expect(screen.getByText('Winners Finals')).toBeInTheDocument();
	});

	it('shows upset factor as integer', () => {
		render(SetDetailModal, {
			props: { set: baseSet, isWin: true, currentPlayerName: 'Alice', onClose: () => {} }
		});
		expect(screen.getByText('2')).toBeInTheDocument();
	});

	it('shows start.gg link when tournament_slug is present', () => {
		render(SetDetailModal, {
			props: { set: baseSet, isWin: true, currentPlayerName: 'Alice', onClose: () => {} }
		});
		const link = screen.getByRole('link', { name: /View on start\.gg/ });
		expect(link).toHaveAttribute(
			'href',
			'https://www.start.gg/tournament/tournament/genesis-9'
		);
	});

	it('hides VOD link when vod_url is null', () => {
		render(SetDetailModal, {
			props: { set: baseSet, isWin: true, currentPlayerName: 'Alice', onClose: () => {} }
		});
		expect(screen.queryByRole('link', { name: /Watch VOD/ })).not.toBeInTheDocument();
	});

	it('shows VOD link when vod_url is present', () => {
		render(SetDetailModal, {
			props: {
				set: { ...baseSet, vod_url: 'https://youtube.com/watch?v=abc' },
				isWin: true,
				currentPlayerName: 'Alice',
				onClose: () => {}
			}
		});
		const link = screen.getByRole('link', { name: /Watch VOD/ });
		expect(link).toHaveAttribute('href', 'https://youtube.com/watch?v=abc');
	});
});
```

- [ ] **Step 2: Run to confirm failure**

```bash
cd web && npm run test:unit -- SetDetailModal
```

Expected: FAIL — `Cannot find module './SetDetailModal.svelte'`

- [ ] **Step 3: Create `SetDetailModal.svelte`**

```svelte
<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import type { SetRecord } from '$lib/types';

	interface Props {
		set: SetRecord | null;
		isWin: boolean;
		currentPlayerName: string;
		onClose: () => void;
	}

	let { set, isWin, currentPlayerName, onClose }: Props = $props();

	function formatDate(s: string | null): string {
		if (!s) return 'Unknown';
		return new Date(s).toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric',
			year: 'numeric'
		});
	}

	function score(): string {
		if (!set || set.winner_score === null || set.loser_score === null) return '';
		return isWin
			? `${set.winner_score}–${set.loser_score}`
			: `${set.loser_score}–${set.winner_score}`;
	}
</script>

<Dialog.Root open={set !== null} onOpenChange={(open) => { if (!open) onClose(); }}>
	<Dialog.Content class="max-w-sm">
		{#if set}
			<Dialog.Header>
				<Dialog.Title>{currentPlayerName} vs {set.opponent_name}</Dialog.Title>
				<Dialog.Description
					class={isWin
						? 'text-green-600 dark:text-green-400'
						: 'text-red-600 dark:text-red-400'}
				>
					{isWin ? 'Win' : 'Loss'}{score() ? ` · ${score()}` : ''}
				</Dialog.Description>
			</Dialog.Header>
			<div class="grid grid-cols-2 gap-x-4 gap-y-3 py-2 text-sm">
				<div>
					<p class="text-xs uppercase tracking-wide text-muted-foreground">Tournament</p>
					<p>{set.tournament_name}</p>
				</div>
				<div>
					<p class="text-xs uppercase tracking-wide text-muted-foreground">Event</p>
					<p>{set.event_name}</p>
				</div>
				<div>
					<p class="text-xs uppercase tracking-wide text-muted-foreground">Round</p>
					<p>{set.round_name ?? 'Unknown'}</p>
				</div>
				<div>
					<p class="text-xs uppercase tracking-wide text-muted-foreground">Date</p>
					<p>{formatDate(set.completed_at)}</p>
				</div>
				<div>
					<p class="text-xs uppercase tracking-wide text-muted-foreground">Seeds</p>
					<p>
						<span class="text-green-600 dark:text-green-400">{set.winner_seed ?? '?'}</span>
						<span class="text-muted-foreground"> vs </span>
						<span class="text-red-600 dark:text-red-400">{set.loser_seed ?? '?'}</span>
					</p>
				</div>
				<div>
					<p class="text-xs uppercase tracking-wide text-muted-foreground">Upset Factor</p>
					<p>{set.upset_factor}</p>
				</div>
			</div>
			{#if set.tournament_slug || set.vod_url}
				<div class="flex gap-4 border-t pt-3 text-sm">
					{#if set.tournament_slug}
						<a
							href="https://www.start.gg/tournament/{set.tournament_slug}"
							target="_blank"
							rel="noopener noreferrer"
							class="text-primary hover:underline"
						>↗ View on start.gg</a>
					{/if}
					{#if set.vod_url}
						<a
							href={set.vod_url}
							target="_blank"
							rel="noopener noreferrer"
							class="text-primary hover:underline"
						>▶ Watch VOD</a>
					{/if}
				</div>
			{/if}
		{/if}
	</Dialog.Content>
</Dialog.Root>
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cd web && npm run test:unit -- SetDetailModal
```

Expected: all 9 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add web/src/lib/components/SetDetailModal.svelte \
        web/src/lib/components/SetDetailModal.test.ts
git commit -m "feat(web): add SetDetailModal shared component"
```

---

## Task 7: Frontend — Redesign Stats page

**Files:**
- Modify: `web/src/routes/projects/[id]/stats/+page.svelte`
- Modify: `web/src/routes/projects/[id]/stats/stats.test.ts`

- [ ] **Step 1: Rewrite `stats.test.ts`** (completely replaces the existing file):

```typescript
import { render, screen, fireEvent } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import Page from './+page.svelte';
import type { SetRecord } from '$lib/types';

const user = { id: 'u1', username: 'testuser', created_at: '2026-01-01T00:00:00Z' };
const project = { id: 'proj-1', name: 'Test Project', game_id: null, game_name: null, created_at: '2026-01-01T00:00:00Z' };

function makeSet(opponentName: string, uf: number): SetRecord {
	return {
		opponent_id: 'opp',
		opponent_name: opponentName,
		upset_factor: uf,
		winner_score: null,
		loser_score: null,
		tournament_name: 'Test Tournament',
		tournament_slug: 'tournament/test',
		event_name: 'Melee Singles',
		round_name: 'Round 1',
		completed_at: null,
		is_dq: false,
		vod_url: null,
		startgg_set_id: 1,
		winner_seed: null,
		loser_seed: null,
	};
}

const stats = [
	{
		player_id: 'p1',
		name: 'Alice',
		wins: [makeSet('Bob', 2), makeSet('Charlie', 1)],
		losses: [makeSet('Charlie', 0)],
	},
	{
		player_id: 'p2',
		name: 'Bob',
		wins: [makeSet('Charlie', 1)],
		losses: [makeSet('Alice', 2)],
	},
	{
		player_id: 'p3',
		name: 'Charlie',
		wins: [],
		losses: [makeSet('Alice', 1)],
	},
];

describe('Stats page', () => {
	it('renders player names', () => {
		render(Page, { data: { user, project, stats } });
		expect(screen.getByText('Alice')).toBeInTheDocument();
		expect(screen.getByText('Bob')).toBeInTheDocument();
		expect(screen.getByText('Charlie')).toBeInTheDocument();
	});

	it('shows W/L/% summary in each card header', () => {
		render(Page, { data: { user, project, stats } });
		// Alice: 2W 1L = 67%, Bob: 1W 1L = 50%, Charlie: 0W 1L = 0%
		expect(screen.getByText('W 2 · L 1 · 67%')).toBeInTheDocument();
		expect(screen.getByText('W 1 · L 1 · 50%')).toBeInTheDocument();
		expect(screen.getByText('W 0 · L 1 · 0%')).toBeInTheDocument();
	});

	it('shows win opponent names with integer UF', () => {
		render(Page, { data: { user, project, stats } });
		expect(screen.getByText(/Bob · UF 2/)).toBeInTheDocument();
		expect(screen.getByText(/Charlie · UF 1/)).toBeInTheDocument();
	});

	it('does not render any decimal UF values', () => {
		const statsWithDecimalUF = [
			{ player_id: 'p1', name: 'Alice', wins: [makeSet('Bob', 2)], losses: [] },
		];
		render(Page, { data: { user, project, stats: statsWithDecimalUF } });
		expect(screen.queryByText(/UF 2\.0/)).not.toBeInTheDocument();
		expect(screen.queryByText(/UF 2\.5/)).not.toBeInTheDocument();
	});

	it('does not show Agg. UF or accumulated upset factor', () => {
		render(Page, { data: { user, project, stats } });
		expect(screen.queryByText(/Agg\./i)).not.toBeInTheDocument();
		expect(screen.queryByText(/accumulated/i)).not.toBeInTheDocument();
	});

	it('shows empty state when stats is empty', () => {
		render(Page, { data: { user, project, stats: [] } });
		expect(
			screen.getByText('No stats yet. Import tournaments and include some events first.')
		).toBeInTheDocument();
		expect(screen.queryByRole('table')).not.toBeInTheDocument();
	});

	it('opens set detail modal when a win row is clicked', async () => {
		render(Page, { data: { user, project, stats } });
		const bobRow = screen.getByRole('button', { name: /Bob · UF 2/ });
		await fireEvent.click(bobRow);
		expect(screen.getByText('Alice vs Bob')).toBeInTheDocument();
		expect(screen.getByText(/Win/)).toBeInTheDocument();
	});

	it('opens set detail modal when a loss row is clicked', async () => {
		render(Page, { data: { user, project, stats } });
		// Alice's losses list contains "Charlie · UF 0"
		const lossRow = screen.getByRole('button', { name: /Charlie · UF 0/ });
		await fireEvent.click(lossRow);
		expect(screen.getByText('Alice vs Charlie')).toBeInTheDocument();
		expect(screen.getByText(/Loss/)).toBeInTheDocument();
	});
});
```

- [ ] **Step 2: Run to confirm failures**

```bash
cd web && npm run test:unit -- stats
```

Expected: multiple FAILs — tests reference the new card UI which doesn't exist yet.

- [ ] **Step 3: Rewrite `stats/+page.svelte`**:

```svelte
<script lang="ts">
	import type { SetRecord } from '$lib/types';
	import SetDetailModal from '$lib/components/SetDetailModal.svelte';

	let { data } = $props();

	let selectedSet = $state<SetRecord | null>(null);
	let selectedIsWin = $state(false);
	let selectedPlayerName = $state('');

	function openModal(set: SetRecord, isWin: boolean, playerName: string) {
		selectedSet = set;
		selectedIsWin = isWin;
		selectedPlayerName = playerName;
	}

	function winRate(wins: number, losses: number): string {
		const total = wins + losses;
		if (total === 0) return '0%';
		return `${Math.round((wins / total) * 100)}%`;
	}
</script>

<div class="space-y-4">
	<h2 class="text-lg font-semibold">Stats</h2>

	{#if data.stats.length === 0}
		<p class="text-sm text-muted-foreground">No stats yet. Import tournaments and include some events first.</p>
	{:else}
		<div class="grid gap-3" style="grid-template-columns: repeat(auto-fill, minmax(320px, 1fr))">
			{#each data.stats as player (player.player_id)}
				<div class="rounded-md border border-border p-3">
					<div class="mb-2 flex items-baseline justify-between">
						<span class="font-semibold">{player.name}</span>
						<span class="text-xs text-muted-foreground">
							W {player.wins.length} · L {player.losses.length} · {winRate(player.wins.length, player.losses.length)}
						</span>
					</div>
					<div class="flex gap-2">
						<div class="flex-1">
							<p class="mb-1 text-xs font-semibold uppercase tracking-wide text-green-600 dark:text-green-400">
								WINS ({player.wins.length})
							</p>
							<div class="h-24 overflow-y-auto rounded border border-border bg-muted/20">
								{#each player.wins as set, i (i)}
									<button
										class="w-full border-b border-border px-2 py-1 text-left text-xs last:border-0 hover:bg-muted/50"
										onclick={() => openModal(set, true, player.name)}
									>
										<span>{set.opponent_name} · UF {set.upset_factor}</span>
									</button>
								{/each}
							</div>
						</div>
						<div class="flex-1">
							<p class="mb-1 text-xs font-semibold uppercase tracking-wide text-red-600 dark:text-red-400">
								LOSSES ({player.losses.length})
							</p>
							<div class="h-24 overflow-y-auto rounded border border-border bg-muted/20">
								{#each player.losses as set, i (i)}
									<button
										class="w-full border-b border-border px-2 py-1 text-left text-xs last:border-0 hover:bg-muted/50"
										onclick={() => openModal(set, false, player.name)}
									>
										<span>{set.opponent_name} · UF {set.upset_factor}</span>
									</button>
								{/each}
							</div>
						</div>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<SetDetailModal
	set={selectedSet}
	isWin={selectedIsWin}
	currentPlayerName={selectedPlayerName}
	onClose={() => (selectedSet = null)}
/>
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cd web && npm run test:unit -- stats
```

Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/projects/\[id\]/stats/+page.svelte \
        web/src/routes/projects/\[id\]/stats/stats.test.ts
git commit -m "feat(web): redesign Stats page as player card grid, remove Agg. UF"
```

---

## Task 8: Frontend — Add H2H side panel with set-detail drill-down

**Files:**
- Modify: `web/src/routes/projects/[id]/h2h/+page.svelte`
- Modify: `web/src/routes/projects/[id]/h2h/h2h.test.ts`

- [ ] **Step 1: Update imports and add mock + new tests in `h2h.test.ts`**

At the very top of the file, update the existing import line and add the mock:

```typescript
// Change existing first import line to add fireEvent:
import { render, screen, fireEvent } from '@testing-library/svelte';

// Add immediately after all imports — vi.mock must be at module scope:
vi.mock('$env/static/public', () => ({ PUBLIC_API_URL: 'http://localhost:8080' }));
// Note: vi is globally available (vitest.config.ts has globals: true)
```

Then add these two tests inside the existing `describe('H2H page', () => { ... })` block:

it('renders non-diagonal cells as clickable buttons', () => {
    render(Page, { data: { user, project, players, h2h } });
    // Alice vs Bob cell shows "3–1" as a button
    expect(screen.getByRole('button', { name: '3–1' })).toBeInTheDocument();
});

it('does not show side panel before any cell is clicked', () => {
    render(Page, { data: { user, project, players, h2h } });
    expect(screen.queryByText(/wins ·/i)).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run to confirm new tests fail**

```bash
cd web && npm run test:unit -- h2h
```

Expected: "renders non-diagonal cells as clickable buttons" FAILS (cells are currently `<td>` spans, not buttons).

- [ ] **Step 3: Rewrite `h2h/+page.svelte`**:

```svelte
<script lang="ts">
	import { PUBLIC_API_URL } from '$env/static/public';
	import { makeApi } from '$lib/api';
	import type { HeadToHeadEntry, H2HSet } from '$lib/types';
	import SetDetailModal from '$lib/components/SetDetailModal.svelte';

	let { data } = $props();

	interface SelectedPair {
		rowPlayer: { id: string; name: string };
		colPlayer: { id: string; name: string };
		sets: H2HSet[];
		wins: number;
		losses: number;
	}

	let selectedPair = $state<SelectedPair | null>(null);
	let loading = $state(false);
	let selectedSet = $state<H2HSet | null>(null);
	let selectedIsWin = $state(false);

	async function selectCell(
		rowPlayer: { id: string; name: string },
		colPlayer: { id: string; name: string }
	) {
		if (
			selectedPair?.rowPlayer.id === rowPlayer.id &&
			selectedPair?.colPlayer.id === colPlayer.id
		) {
			selectedPair = null;
			return;
		}
		loading = true;
		selectedPair = null;
		const api = makeApi(fetch, PUBLIC_API_URL);
		const res = await api.get(
			`/projects/${data.project.id}/head-to-head/${rowPlayer.id}/${colPlayer.id}/sets`
		);
		const sets: H2HSet[] = res.ok ? await res.json() : [];
		selectedPair = {
			rowPlayer,
			colPlayer,
			sets,
			wins: sets.filter((s) => s.is_win).length,
			losses: sets.filter((s) => !s.is_win).length,
		};
		loading = false;
	}

	function isSelected(rowId: string, colId: string): boolean {
		return selectedPair?.rowPlayer.id === rowId && selectedPair?.colPlayer.id === colId;
	}

	function getRecord(rowId: string, colId: string): HeadToHeadEntry | undefined {
		return data.h2h.find((e) => e.player_id === rowId && e.opponent_id === colId);
	}
</script>

<div class="space-y-4">
	<h2 class="text-lg font-semibold">Head-to-head</h2>

	{#if data.players.length < 2 || data.h2h.length === 0}
		<p class="text-sm text-muted-foreground">No head-to-head data yet. Import tournaments first.</p>
	{:else}
		<div class="flex gap-4 items-start">
			<!-- Matrix -->
			<div class="overflow-x-auto">
				<table class="border-collapse text-sm">
					<thead>
						<tr>
							<th class="w-32 pb-2 pr-3 text-left font-normal text-muted-foreground"></th>
							{#each data.players as col (col.id)}
								<th class="px-2 pb-2 text-center font-medium" style="min-width:5rem">
									<span class="block max-w-[5rem] truncate" title={col.name}>{col.name}</span>
								</th>
							{/each}
						</tr>
					</thead>
					<tbody>
						{#each data.players as row (row.id)}
							<tr>
								<td class="max-w-[8rem] truncate py-1 pr-3 font-medium" title={row.name}>{row.name}</td>
								{#each data.players as col (col.id)}
									{#if row.id === col.id}
										<td class="px-2 py-1 text-center text-muted-foreground">—</td>
									{:else}
										{@const rec = getRecord(row.id, col.id)}
										<td class="px-2 py-1 text-center tabular-nums">
											{#if rec}
												<button
													class="rounded px-1
														{isSelected(row.id, col.id)
															? 'ring-2 ring-primary bg-primary/10'
															: rec.wins > rec.losses
																? 'bg-green-50 dark:bg-green-950/30 text-green-700 dark:text-green-400'
																: rec.wins < rec.losses
																	? 'bg-red-50 dark:bg-red-950/30 text-red-700 dark:text-red-400'
																	: ''}"
													onclick={() => selectCell(row, col)}
												>
													{rec.wins}–{rec.losses}
												</button>
											{:else}
												<span class="text-muted-foreground">—</span>
											{/if}
										</td>
									{/if}
								{/each}
							</tr>
						{/each}
					</tbody>
				</table>
				<p class="mt-1 text-xs text-muted-foreground">Row player's record vs. column player</p>
			</div>

			<!-- Side panel -->
			{#if loading}
				<div class="flex items-center justify-center rounded-md border border-border p-6 text-sm text-muted-foreground min-w-[200px]">
					Loading…
				</div>
			{:else if selectedPair}
				<div class="rounded-md border border-border p-3 min-w-[220px] flex-1 max-w-xs">
					<div class="mb-3 flex items-start justify-between gap-2 border-b border-border pb-2">
						<div>
							<p class="font-semibold text-sm">{selectedPair.rowPlayer.name} vs {selectedPair.colPlayer.name}</p>
							<p class="text-xs text-muted-foreground">{selectedPair.wins} wins · {selectedPair.losses} losses</p>
						</div>
						<button
							class="text-muted-foreground hover:text-foreground text-lg leading-none"
							onclick={() => (selectedPair = null)}
							aria-label="Close panel"
						>×</button>
					</div>
					{#if selectedPair.sets.length === 0}
						<p class="text-xs text-muted-foreground">No sets found.</p>
					{:else}
						<div class="space-y-px">
							{#each selectedPair.sets as set, i (i)}
								<button
									class="w-full flex items-center gap-2 rounded px-2 py-1.5 text-xs hover:bg-muted/50 border-b border-border last:border-0"
									onclick={() => { selectedSet = set; selectedIsWin = set.is_win; }}
								>
									<span class={set.is_win ? 'font-bold text-green-600 dark:text-green-400 min-w-[12px]' : 'font-bold text-red-600 dark:text-red-400 min-w-[12px]'}>
										{set.is_win ? 'W' : 'L'}
									</span>
									{#if set.winner_score !== null && set.loser_score !== null}
										<span class="tabular-nums">
											{set.is_win ? `${set.winner_score}–${set.loser_score}` : `${set.loser_score}–${set.winner_score}`}
										</span>
									{/if}
									<span class="text-muted-foreground truncate flex-1 text-left">{set.tournament_name}</span>
									{#if set.round_name}
										<span class="text-muted-foreground shrink-0">{set.round_name}</span>
									{/if}
								</button>
							{/each}
						</div>
					{/if}
					<p class="mt-2 text-xs text-muted-foreground">Click a row for full details</p>
				</div>
			{/if}
		</div>
	{/if}
</div>

<SetDetailModal
	set={selectedSet}
	isWin={selectedIsWin}
	currentPlayerName={selectedPair?.rowPlayer.name ?? ''}
	onClose={() => (selectedSet = null)}
/>
```

- [ ] **Step 4: Run tests to confirm they all pass**

```bash
cd web && npm run test:unit -- h2h
```

Expected: all tests PASS (including pre-existing ones and new ones).

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/projects/\[id\]/h2h/+page.svelte \
        web/src/routes/projects/\[id\]/h2h/h2h.test.ts
git commit -m "feat(web): add H2H side panel with set-detail drill-down"
```

---

## Task 9: Frontend — Fix account linking auto-refresh

**Files:**
- Modify: `web/src/routes/projects/[id]/players/+page.svelte`

- [ ] **Step 1: Add `invalidateAll` import** at the top of the `<script>` block in `players/+page.svelte`:

```svelte
import { invalidateAll } from '$app/navigation';
```

- [ ] **Step 2: Update the `linkAccount` enhance callback** (line ~93):

```svelte
use:enhance={() => {
    return async ({ result, update }) => {
        if (result.type === 'success') {
            linkDialogOpen = false;
            await invalidateAll();
        } else {
            await update();
        }
    };
}}
```

- [ ] **Step 3: Update the `unlinkAccount` enhance** (line ~48 — currently `use:enhance` with no callback):

```svelte
<form method="POST" action="?/unlinkAccount" use:enhance={() => {
    return async ({ result, update }) => {
        if (result.type === 'success') {
            await invalidateAll();
        } else {
            await update();
        }
    };
}} class="inline-flex">
```

- [ ] **Step 4: Run full frontend test suite**

```bash
cd web && npm run test:unit
```

Expected: all tests PASS.

- [ ] **Step 5: Run e2e tests**

```bash
cd web && npm run test:e2e
```

Expected: all Playwright tests PASS.

- [ ] **Step 6: Commit**

```bash
git add web/src/routes/projects/\[id\]/players/+page.svelte
git commit -m "fix(web): invalidate page data after linking/unlinking start.gg account"
```

---

## Final Verification

- [ ] Run full backend test suite: `bash backend/test.sh` — all pass
- [ ] Run full frontend test suite: `cd web && npm run test:unit` — all pass
- [ ] Run frontend e2e: `cd web && npm run test:e2e` — all pass
- [ ] Start dev stack and open Stats page — player cards render in a grid, no Agg. UF, UF shown as integer, clicking a set row opens the detail modal with all fields
- [ ] Open H2H page — clicking a matrix cell shows the side panel; clicking a row in the panel opens the set detail modal
- [ ] Link a start.gg account on the Players page — the account badge appears immediately without a manual refresh
