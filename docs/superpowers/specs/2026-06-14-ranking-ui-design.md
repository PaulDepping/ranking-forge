# Design: Ranking UI — Creation, Configuration, and Navigation

**Date:** 2026-06-14
**Status:** Approved — awaiting implementation plan

---

## Overview

The backend for algorithmic rankings (Elo, Glicko-2), pre-computed set results, and batch event inclusion is fully implemented. The frontend has not been updated to match. This spec covers all missing frontend work:

- SvelteKit route restructure (project hub vs. isolated ranking view)
- New ranking creation form with algorithm picker
- Ranking settings tab (rename, algorithm config, published toggle, delete)
- Algorithmic ranking display on the Ranking tab (computed order with manual override)
- `rank_position` seeding fix (currently overwrites manual order on every import)
- `Ranking` type update in `types.ts`

---

## Route Structure

### Problem

The current SvelteKit layout nests the ranking layout inside the project layout, producing two stacked tab bars when inside a ranking. The project "Rankings" tab also fails to highlight correctly when navigating into a ranking (exact-match URL check).

### Solution: `(hub)` route group

Introduce a `(hub)` route group under `/projects/[id]/`. SvelteKit route groups are file-system-only — they don't affect browser URLs.

```
src/routes/projects/[id]/
  +layout.server.ts       — loads project (unchanged)
  +layout.svelte          — thin shell: project title + guest banner, NO tabs
  (hub)/
    +layout.svelte        — project tab bar: Rankings / Players / Import / Settings
    +page.server.ts       — rankings list (moved from parent; single-ranking redirect removed)
    +page.svelte          — rankings list
    (editor)/
      +layout.server.ts   — editor guard (unchanged)
      import/
      players/
        [player_id]/
    settings/
  rankings/
    new/
    [rid]/
      +layout.server.ts   — loads ranking (unchanged)
      +layout.svelte      — ranking tab bar (see below)
      (editor)/
        players/
      h2h/
      ranking/
      settings/           ← NEW
      stats/
      tournaments/
```

The project layout (`+layout.svelte`) becomes a thin wrapper rendering only the project title, game subtitle, and guest banner — no tab bar. When on a hub page, the `(hub)` layout adds the project tab bar. When on a ranking page, the `(hub)` layout is not in the chain, so no project tabs appear.

**Single-ranking redirect removed.** The rankings list page always renders the list, even when there is only one ranking. The old auto-redirect was the primary cause of the Rankings tab losing its active state.

---

## Project Hub

The rankings list page (`/projects/[id]`) shows:

- Project title, game subtitle (in the thin project shell above)
- Project tab bar: Rankings / Players / Import / Settings
- A ranked list of ranking cards, each showing: name, algorithm label (Manual / Elo / Glicko-2), number of players in the ranking (`ranking_players` count), Public/Private badge, and description
- "+ New ranking" button (editor/owner only)

---

## Ranking Layout

### Breadcrumb and ranking switcher

The ranking layout header shows a compact breadcrumb:

```
Melee Midwest / 2025 Season ▾
```

- "Melee Midwest" is a link back to `/projects/[id]`
- "2025 Season ▾" is a dropdown trigger listing all rankings in the project plus a "+ New ranking" entry
- Switching rankings via the dropdown preserves the current tab segment (e.g. navigating from `2025 Season/stats` to `Elo Experimental` lands on `Elo Experimental/stats`). If the current tab is not available for the target ranking (e.g. switching away from an editor-only tab while viewing as a viewer), fall back to `stats`.

### Tab bar

```
Players | Tournaments | Stats | H2H | Ranking | Settings
```

- **Players** (editor/owner only): manage which project pool players are in this ranking
- **Tournaments**: event inclusion toggles (batch save)
- **Stats**: per-player win/loss lists
- **H2H**: head-to-head set record matrix
- **Ranking**: ordered player list (see below)
- **Settings** (editor/owner only): ranking configuration (see below)

---

## New Ranking Creation Form

Route: `/projects/[id]/rankings/new`

Fields:
1. **Name** (required)
2. **Description** (optional)
3. **Algorithm** — inline radio card picker

### Algorithm picker

Three radio cards: Manual, Elo, Glicko-2. The selected card is highlighted with an accent border. Selecting Elo or Glicko-2 reveals an inline config section inside the card with pre-filled defaults. Most users pick an algorithm and submit without touching the config.

**Manual** — no config. Players are ordered by dragging.

**Elo** ([Wikipedia](https://en.wikipedia.org/wiki/Elo_rating_system)):
- **K-factor** (default: 32) — points at stake per set. Lower values (16) make ratings change slowly; higher values (64) make them react quickly. 32 is standard for most competitive scenes.
- **Initial rating** (default: 1500) — starting rating for all players before any sets are counted.

**Glicko-2** ([Wikipedia](https://en.wikipedia.org/wiki/Glicko_rating_system)):
- **τ / tau** (default: 0.5) — controls how quickly a player's volatility can change. Glickman recommends 0.3–1.2; smaller values produce a more stable system.
- **Initial RD** (default: 350) — rating deviation for a new player — the ± on their rating. A player at 1500 ± 350 is very uncertain; ± 50 is well-established. RD shrinks automatically as they play more sets.
- **Initial volatility σ** (default: 0.06) — expected degree of rating fluctuation for a new player. Glickman recommends 0.06. Rarely needs adjustment.

On submit, the form POSTs to the existing `POST /projects/:id/rankings` endpoint with `algorithm` and `algorithm_config` included. Redirects to the new ranking's `/players` tab.

---

## Ranking Settings Tab

Route: `/projects/[id]/rankings/[rid]/settings` (editor/owner only)

### General

Name and description fields with a single Save button. Calls `PATCH /projects/:id/rankings/:rid`.

### Publishing

A toggle for the `published` flag. Label: "Public — anyone with the link can view stats, H2H, and ranking." Calls `PATCH` on change.

### Algorithm

Shows the algorithm type as a read-only label ("Manual", "Elo", "Glicko-2") with a note: _"Set at creation. Create a new ranking to use a different algorithm."_ A Wikipedia link appears next to the algorithm name.

For algorithmic rankings, the config parameters (K-factor, τ, etc.) are editable with the same descriptions as the creation form. A **"Save & recompute"** button makes two sequential API calls: `PATCH /projects/:id/rankings/:rid` to save the updated `algorithm_config`, then `POST /projects/:id/rankings/:rid/recompute` to enqueue a compute job. The Algorithm section is shown for Manual rankings too (displaying "Manual" as read-only) but without a config or recompute button.

### Recompute

A secondary-styled **"Recompute now"** button for manually triggering recalculation. Useful as a recovery action if a compute job failed. Calls `POST /projects/:id/rankings/:rid/recompute`. This section is not shown on the Ranking tab — Settings is the only place to trigger a manual recompute.

### Danger zone

Delete ranking with a confirmation dialog. Calls `DELETE /projects/:id/rankings/:rid`. Redirects to the project hub on success.

---

## Ranking Tab

Route: `/projects/[id]/rankings/[rid]/ranking`

### Manual rankings (unchanged)

Drag-to-reorder list. Editors drag players to set `rank_position`. Save button calls `PUT /projects/:id/rankings/:rid/ranking`. Win/loss record shown per row.

### Algorithmic rankings

`rank_position` is the authoritative published order for algorithmic rankings too — the algorithm's `computed_rating` informs but does not override the final editor-confirmed order.

The Ranking tab for an algorithmic ranking shows:

- An ordered list (by `rank_position`) with drag handles visible to editors — dragging works at any time, same as a manual ranking
- Each row shows: rank number, player name, computed rating (Elo: plain integer e.g. `1543`; Glicko-2: `1487 ± 45`), and a delta badge when the player's current `rank_position` differs from where `computed_rating` would place them (e.g. `↑2`, `↓1`)
- A **"Sync to algorithm"** button that calls `PUT /projects/:id/rankings/:rid/ranking` with players ordered by `computed_rating` — resetting `rank_position` to match the computed order
- A Save button (same as manual rankings) that persists any drag changes via `PUT /projects/:id/rankings/:rid/ranking`

This mirrors how real power rankings work: the algorithm suggests, the TO confirms and adjusts.

### rank_position seeding

`rank_position` is only auto-set when all players in a ranking still have the default value (0), meaning no ordering has been established yet:

- **Manual ranking, first import:** seeded by win rate (existing behaviour, now correctly treated as one-time)
- **Manual ranking, subsequent imports:** rank_position is not touched — manual order is preserved
- **Algorithmic ranking, first compute:** seeded by `computed_rating` order
- **Algorithmic ranking, subsequent computes:** rank_position is not touched — editor controls order via "Sync to algorithm"

This fixes a latent bug in `seed_ranking_by_winrate`, which currently runs unconditionally on every import and overwrites any manual ordering.

---

## Frontend Type Updates

Add the following fields to the `Ranking` interface in `web/src/lib/types.ts`:

```typescript
algorithm: string | null;
algorithm_config: Record<string, unknown>;
include_external_results: boolean;
result_sort: string;
```

The `GET /projects/:id/rankings` and `GET /projects/:id/rankings/:rid` responses already include these fields from the backend.

---

## Files Affected

**Create:**
- `web/src/routes/projects/[id]/(hub)/+layout.svelte`
- `web/src/routes/projects/[id]/(hub)/+page.server.ts`
- `web/src/routes/projects/[id]/(hub)/+page.svelte`
- `web/src/routes/projects/[id]/rankings/[rid]/settings/+page.server.ts`
- `web/src/routes/projects/[id]/rankings/[rid]/settings/+page.svelte`

**Move into `(hub)/`:**
- `(editor)/import/` → `(hub)/(editor)/import/`
- `(editor)/players/` → `(hub)/(editor)/players/`
- `settings/` → `(hub)/settings/`
- `(editor)/+layout.server.ts` → `(hub)/(editor)/+layout.server.ts`

**Modify:**
- `web/src/routes/projects/[id]/+layout.svelte` — remove tab bar, keep title/banner only
- `web/src/routes/projects/[id]/rankings/[rid]/+layout.svelte` — add breadcrumb with ranking switcher; add Settings tab; fix active tab detection to use `startsWith`
- `web/src/routes/projects/[id]/rankings/[rid]/ranking/+page.svelte` — algorithmic ranking display with computed_rating badge, delta indicator, "Sync to algorithm" button
- `web/src/routes/projects/[id]/rankings/new/+page.svelte` — add algorithm picker
- `web/src/routes/projects/[id]/rankings/new/+page.server.ts` — pass algorithm/algorithm_config to API
- `web/src/lib/types.ts` — extend `Ranking` interface
- `web/src/lib/api.ts` — add `patchRanking`, `deleteRanking`, `recomputeRanking` client methods if missing
- `backend/crates/worker/src/import.rs` — make `seed_ranking_by_winrate` one-time (skip if any rank_position > 0)
- `backend/crates/worker/src/compute.rs` — seed rank_position from computed_rating on first compute for algorithmic rankings
