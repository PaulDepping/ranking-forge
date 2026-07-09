# Design: Backend Build Warning Cleanup

**Date:** 2026-07-09
**Status:** Draft — awaiting review

---

## Overview

`cargo build`/`cargo check` on the `backend/` workspace currently emits warnings from exactly two crates: `api` and `crawler`. This spec catalogs every warning, the investigation done into *why* the dead code exists, and the disposition decided for each. It also documents a correctness gap discovered while investigating the `crawler` warnings — the crawler's DQ-detection logic diverges from, and is weaker than, an existing verified-correct implementation elsewhere in the workspace. That gap is **out of scope for this cleanup** and is written up here as a flagged follow-up investigation, per instruction, with everything found so far.

---

## Part 1 — Warning inventory and decisions

### 1. `api` crate — dead `rsr.upset_factor` SELECT reads

**Where:** `crates/api/src/routes/tournaments.rs`, three near-identical local `sqlx::query_as!` structs:
- `get_stats` (~line 315-343)
- `get_player_stats` (~line 491)
- `get_h2h_sets` (~line 817-843)

Each selects `rsr.upset_factor` into a struct field that is fetched but never read — the response instead recomputes UF on the fly via `set_upset_factor(row.winner_seed, row.loser_seed)`.

**Investigation:** traced the write path. `worker/src/compute.rs` computes `set_upset_factor(ws, ls)` and writes it into `ranking_set_results.upset_factor` on every `compute_ranking` run — i.e. the column is written, but every read site ignores the stored value and recomputes the same function from `winner_seed`/`loser_seed` instead. **The stored column is write-only**: nothing in the codebase ever consumes it. Confirmed via grep that `'upset_factor'` also appears as a `result_sort` string literal (unrelated — a sort-mode name, not a column reference) in `routes/rankings.rs` and `common/jobs.rs`.

**Decision:** ✅ Confirmed by user.
- Remove the 3 dead `rsr.upset_factor` SELECT+struct-field pairs in `tournaments.rs`.
- Stop writing `upset_factor` in `worker/src/compute.rs` — remove the computation and drop it from the `INSERT`/`ON CONFLICT DO UPDATE`.
- Drop the `upset_factor FLOAT` column from `ranking_set_results` by editing `migrations/001_initial.sql` directly (line 332) — user explicitly authorized modifying the existing migration since there is no production database yet, so a new migration is unnecessary churn.
- Run `bash backend/prepare-sqlx.sh` after these changes (all three touch `sqlx::query!`/`query_as!` macros).
- Check `docs/DESIGN.md` for any mention of the `upset_factor` column on `ranking_set_results` and update if present.

**Deferred (not part of this pass):** the `Row`/`H2HSetRow` local-struct pattern is duplicated verbatim between `get_player_tournaments` (~line 658-711) and `get_ranking_player_tournaments` (~line 1015-1063), differing only in their WHERE-clause join. User: "Let's talk about this later on" — no dedup in this pass.

### 2. `api` crate — orphaned `normalize_tournament_handle`

**Where:** `crates/api/src/routes/players.rs:622`, plus its 4 tests (~line 670-696).

**Investigation:** `git log --oneline --all -S "normalize_tournament_handle"` found 3 commits:
- `7805523` — added the function + tests.
- `198cbbd` — added a `/tournament-entrants` GET endpoint that called it to normalize a full start.gg URL passed as a query param.
- `13de862` ("rewrite account linking and tournament entrants to use global mirror") — rewrote `list_tournament_entrants` to take a bare slug as a path param (`/{id}/tournament-entrants/{handle}`) instead of a query param, deleting the only call site (`git show 13de862` shows `- let handle = normalize_tournament_handle(&q.tournament);` removed).

Function has had zero production callers since `13de862`; only its own tests reference it. `strip_startgg_url_prefix` and `normalize_handle` (defined nearby) are unaffected — `normalize_handle` is still called from production code at line 359.

**Decision:** ✅ Confirmed by user ("Delete it and its tests").
- Delete `normalize_tournament_handle` and its 4 tests (`normalize_tournament_handle_bare`, `_full_url`, `_with_tournament_prefix`, `_trims_whitespace`) from `players.rs`.

### 3. `crawler` crate — duplicate bin/lib module compilation

**Where:** `crates/crawler/src/main.rs`:
```rust
mod api;
mod api_types;
mod cli;
mod db;
mod scraper;
```
`crates/crawler/src/lib.rs` already declares the same tree as `pub mod`. Because `main.rs` redeclares them with `mod` instead of importing from the lib, Cargo compiles every one of these source files twice — once as part of the `crawler` lib target, once as part of the `crawler` bin target — as two independent crate roots. `main.rs` only actually references `cli::Config` and `scraper::run` directly.

This duplicate compilation is the direct cause of most of the crawler's dead-code warnings appearing (each unread field gets flagged once per compiled crate root — bin and lib — effectively doubling the warning count for the same underlying issue).

**Decision:** fix `main.rs` to `use crawler::{cli, scraper};` instead of the `mod` redeclarations. No behavior change — eliminates duplicate compilation of `api.rs`, `api_types.rs`, `db.rs` under the bin target. *(This was proposed during investigation and not objected to, but should get explicit sign-off in the implementation plan since it was never asked as a direct yes/no.)*

### 4. `crawler` crate — dead fields in `api_types.rs`

All ~19 fields were added in commit `b866a23` ("feat(crawler): add API response types for all 6 queries"), itself part of the `crawler` binary built per `docs/superpowers/specs/2026-06-23-startgg-mirror-crawler-design.md` (2026-06-23/24), which was ported from an external tool (`hci_startgg_dataset`).

Split by design-doc provenance:

**a) Explicitly speced "Keep" fields** — `docs/superpowers/specs/2026-06-23-startgg-mirror-crawler-design.md` lines 245-250 lists `lPlacement`, `wPlacement`, `initialSeedNum`, `isDisqualified`, and `stats { score { value } }` under a deliberate "Keep" decision for `PHASE_GROUP_SETS_QUERY`, as opposed to an adjacent "Remove" list — i.e. these were consciously chosen to be fetched, not incidental. Confirmed via `db.rs`'s `upsert_set` parameter list that `global_sets` has no columns for placement/seed/disqualified-flag today, so none of these are persisted regardless of keep/delete disposition.

- **Superseded finding:** the disposition of `stats.score.value` specifically is no longer just a "keep vs delete" question — see Part 2 below. It should be *used*, not merely retained. `l_placement`/`w_placement`/`initial_seed_num` disposition remains genuinely open. `is_disqualified` (`Entrant.isDisqualified`) should explicitly **not** be used for per-set DQ detection — `docs/startgg/project-notes.md` documents it as "the entrant's overall DQ status, not per-set," distinct from the per-set signal. Whether it has some other legitimate future use is still open.

**b) "Ported unchanged" fields** — `total` (`PageInfo`), `event_type` (`EventNode`), `prefix` (`PlayerNode`), various `id` fields (`PhaseGroupsPage.page_info` in the sense of the id fields on `FullPhaseGroupNode`, `PhaseGroupInfo`, `PhaseInfo`, `SlimPhaseGroupNode`, `SlimSetNode`'s placement fields via `SlimEntrantNode`, `GamesPhaseGroupNode`, `StandingNode`), `is_final` (`StandingNode`) — sourced from `TOURNAMENT_QUERY` and `PHASE_GROUPS_QUERY`, which the design doc notes were "ported unchanged" from the old HCI scraper with no field-level review.

**Decision:** deferred. User: "Let's talk about this later" (twice, across two rounds of questions). No action in this pass.

---

## Part 2 — Follow-up investigation: DQ-detection correctness gap

**Status: flagged for investigation immediately after this cleanup lands. Not to be fixed as part of this pass.** Everything below is what has been found so far, to save re-investigation time when this is picked up.

### The two implementations

**`common::startgg::queries::SetNode::is_dq()`** (`crates/common/src/startgg/queries.rs:366-378`):
```rust
pub fn is_dq(&self) -> bool {
    self.slots.iter().any(|slot| {
        slot.standing.as_ref()
            .and_then(|s| s.stats.as_ref())
            .and_then(|s| s.score.as_ref())
            .and_then(|s| s.value)
            .map(|v| v < 0.0)
            .unwrap_or(false)
    })
}
```
One signal: any slot's `standing.stats.score.value < 0.0`. Documented in `docs/startgg/project-notes.md` under "DQ detection via score value": *"`standing.stats.score.value` is a `f64`. Any negative value (`< 0.0`) indicates a disqualification, not a real score... This is distinct from `Entrant.isDisqualified` (which marks the entrant's overall DQ status, not per-set)."* Has dedicated unit tests (`common/src/startgg/queries/tests.rs`: `set_node_is_dq_false_for_normal_scores`, `set_node_is_dq_true_when_any_score_negative`, `set_node_is_dq_false_when_no_scores`) and integration-level tests in `common/src/startgg/mod.rs` (`event_sets_detects_dq_from_negative_score`).

**`crawler::scraper::is_dq()`** (`crates/crawler/src/scraper.rs:35-42`):
```rust
pub fn is_dq(state: Option<i64>, display_score: Option<&str>) -> bool {
    if state == Some(7) {
        return true;
    }
    display_score.map(|s| s.to_uppercase().contains("DQ")).unwrap_or(false)
}
```
Two signals, OR'd: `state == 7`, or `displayScore` (raw text) containing the substring "DQ" case-insensitively. Stated flatly in `docs/superpowers/specs/2026-06-23-startgg-mirror-crawler-design.md` line 293: *"A set is marked `is_dq = true` when `state = 7` (start.gg's DQ state code) or when `displayScore` contains 'DQ'."* — asserted as fact, with no citation and no cross-reference to `project-notes.md` or `common::startgg`'s existing approach. See below: this logic turns out not to actually be ported from the source tool it's attributed to.

### Is `state == 7` actually documented anywhere?

No — checked four independent sources, none corroborate it. Note on methodology: start.gg's schema declares exactly one state-related enum, `ActivityState` (`CREATED, ACTIVE, COMPLETED, READY, INVALID, CALLED, QUEUED`), but per `project-notes.md`'s own documented "ActivityState type inconsistency" quirk, that enum is only the *wire type* for `Event.state`/`Phase.state` (serialized as strings). `Tournament.state`, `PhaseGroup.state`, and `Set.state` are declared as plain `Int` in the schema — not typed as `ActivityState` at all, and GraphQL introspection exposes no further semantics for a bare `Int` field. So `ActivityState`'s documentation was never going to be the right place to look for what `7` means on `Set.state`; checking it only rules out one wrong hypothesis (that the int is a disguised `ActivityState` ordinal), not the real question.

1. **Local schema (`crates/common/src/startgg/schema.graphql`).** `Set.state: Int` has no docstring at all, and (per the above) isn't tied to the `ActivityState` enum's declared members in the first place.
2. **`ActivityState`'s own doc page** (`developer.start.gg/reference/activitystate.doc.html`, redirects to `smashgg-schema.netlify.app/reference/activitystate.doc.html`) — lists only the 7 enum names, no numeric ordinals, no DQ mention. As above, not actually applicable to `Set.state`'s wire format, so this alone doesn't resolve the question.
3. **`Set`'s own doc page** (`smashgg-schema.netlify.app/reference/set.doc`) — the field is documented as bare `state: Int`, with zero elaboration on what values it takes. This is the right artifact to check for `Set.state` specifically, and it has nothing. Also checked `SetFilters` (which accepts `state: [Int]` for filtering) — same story, no value list. Broader web search for any start.gg/smash.gg community reverse-engineering of numeric set-state codes (community wrapper libraries, forums, gists) turned up nothing either.
4. **The tool the crawler design spec claims this was ported from.** This is the significant finding: `docs/superpowers/specs/2026-06-23-startgg-mirror-crawler-design.md` line 293 presents `state = 7` as "start.gg's DQ state code" in a section describing logic that "transfers without meaningful change" from `hci_startgg_dataset` (a separate local project at `/home/pd/uni/hci_startgg_dataset`, referenced in both this spec and the predecessor `2026-06-13-ranking-algorithms-global-mirror-design.md` as the "battle-tested" source being ported). Read that source directly: **it has no `is_dq` function, no `state == 7` check, and no `displayScore` text-matching anywhere.** `hci_startgg_dataset/src/scraper.rs` and `db.rs` only ever store `set_state` and `display_score` as opaque raw columns (`db.rs:427`) — appropriate for a research dataset that preserves raw fields for downstream analysis — and separately persist `Entrant.isDisqualified` verbatim per entrant (`models.rs:38`, `db.rs:158`). There is no DQ-inference logic in the source tool at all.

So `crawler::scraper::is_dq()` isn't a carried-over piece of "battle-tested" logic — it's new logic written during the RankingForge port, and the design spec's framing of `7` as a known start.gg DQ state code is unsourced.

### Empirical check against real tournament data (SAPF 2)

Since nothing in docs settled it, queried the live start.gg API directly (key from the repo's `.env`, used via shell env expansion, never printed) against `start.gg/tournament/sapf-2`, event 1364056 ("Melee Singles", 245 entrants).

1. Fetched all entrants and filtered for `isDisqualified: true` — found 7: `Velani`, `Schwanzus`, `Isdsar`, `solariiii`, `Halo-Halo`, `Kid+`, `der_Aufraeumer`.
2. Fetched every set involving those 7 entrant IDs via `sets(filters: { entrantIds: [...] })`, requesting `id state displayScore winnerId slots { entrant { id name } standing { stats { score { value } } } }`.

Result — every real DQ found (loser slot from a disqualified entrant) looked like this:

| set id | state | displayScore | DQ'd entrant's `score.value` |
|---|---|---|---|
| 101322251 | **3** | `"DQ"` | **-1** |
| 101322367 | **3** | `"DQ"` | **-1** |
| 101322479 | **3** | `"DQ"` | **-1** |
| 101322363 | **3** | `"DQ"` | **-1** |
| 101322475 | **3** | `"DQ"` | **-1** |
| 101322244 | **3** | `"DQ"` | **-1** |
| 101322359 | **3** | `"DQ"` | **-1** |

Every one: `state == 3`, never `7`. To rule out coincidence, also pulled 70 more sets from the same event unfiltered (a mix of normal completed sets and more of the same DQ'd entrants' other sets) — **every single one, DQ or not, has `state == 3`.** `state` never took any other value in this sample. `3` also matches the *exact example value* `docs/startgg/project-notes.md`'s own "ActivityState type inconsistency" table already gives for `Set.state` (`| Set.state | Integer | 3 |`) — i.e. `3` reads as generic "COMPLETED," not DQ-specific.

Conclusions from real data:
- **`state == 7` is empirically wrong**, not just unsourced. It doesn't merely lack documentation — it doesn't match reality for this tournament. As written, `crawler::scraper::is_dq()`'s `state == Some(7)` branch is dead code that will never fire for genuine start.gg DQs (at least for Melee singles brackets of this shape); the crawler's DQ detection is, in practice, entirely carried by the `displayScore` substring fallback.
- **The `displayScore` substring check happened to work on all 7 real examples** — every real DQ's `displayScore` was the exact literal string `"DQ"`, not embedded in participant-tag text, so `.to_uppercase().contains("DQ")` matched correctly here. The false-positive risk from a tag containing "DQ" (discussed above) remains a real but *unobserved* risk in this sample — not disproven, just not hit by this particular data.
- **The negative-score-value check (`common::startgg`'s approach) matched all 7 real DQs exactly** (`score.value == -1` for the DQ'd entrant, `0` or `null` for the opponent, in every case) — 100% agreement with `Entrant.isDisqualified` ground truth in this sample, with no reliance on an unverified magic number.

This raises the priority of the earlier "clean, low-risk fix" recommendation (wire up the already-fetched `stats.score.value` in `crawler::scraper::is_dq()`) from "would be an improvement" to "the `state` branch of the current logic is currently non-functional against real data and should not be trusted as-is."

### Is this gap live / consequential?

**Yes.** Traced the full data flow:
- `worker/src/compute.rs` (lines 53-64, 138-147) computes upset factors by querying `global_sets` and explicitly filtering `WHERE gs.is_dq = false`. This is the **only** production path that computes rankings from mirrored data.
- `global_sets.is_dq` is populated exclusively by `crawler::db::upsert_set`, fed by `crawler::scraper::is_dq()`.
- `common::startgg::queries::SetNode::is_dq()` — the documented-correct implementation — is **never called from production code**. Grepped `.is_dq()` and `.scores()` usage workspace-wide: every call site is inside `common/src/startgg/mod.rs`'s own `#[cfg(test)]` module. Likewise `StartggClient::event_sets()` (the method that returns `SetNode`s) has no production caller.

So the situation is not "two systems both feed rankings and one is better" — it's: **the correct, tested implementation is currently orphaned, and the sole live path feeding real upset-factor computation uses the weaker one.**

### Why `event_sets`/`SetNode` became orphaned

Traced via the design-spec history:
- `docs/superpowers/specs/2026-05-14-tournament-dedup-design.md` (pre-global-mirror) describes the original architecture: `worker::import.rs` called `import_event` → `import_entrants` + `import_sets` per project, live against start.gg, via `StartggClient::event_sets()`. This was the production DQ-detection path at the time and is why `SetNode::is_dq()` was built to the documented-correct spec with matching tests.
- `docs/superpowers/specs/2026-06-23-startgg-mirror-crawler-design.md` introduced the `crawler` binary as an independent global mirror, with its own GraphQL query construction and response types built from a different source (`hci_startgg_dataset`), not reusing `common::startgg`.
- Commit `13de862` ("rewrite account linking and tournament entrants to use global mirror") then rewrote `worker::import.rs` to stop calling start.gg directly at all — `crates/worker/src/import.rs` now only reads already-mirrored `global_*` tables and links relevant events into a project's rankings (confirmed by reading the current file in full: zero references to `SetNode`, `score`, or `StartggClient` set-level calls).
- Net effect: the pivot to the global-mirror architecture silently orphaned the old, correct DQ/score implementation without carrying its logic into the new crawler.

### Where each implementation is right, wrong, or complementary

- **Crawler already fetches the reliable signal and ignores it.** Both `PHASE_GROUP_SETS_QUERY` and `PHASE_GROUP_SETS_QUERY_SLIM` (`crates/crawler/src/api.rs`) already request `standing { stats { score { value } } }` per slot — the exact field `common::startgg` uses. It's deserialized into `SlotStanding`/`SlotStats`/`ScoreValue` (`crates/crawler/src/api_types.rs`) — these are 3 of the dead-code-warned fields from Part 1 item 4a. Nothing in `scraper.rs` or `db.rs` reads `.value` for DQ purposes or for scores. Wiring this up would simultaneously fix the correctness gap *and* resolve those specific dead-code warnings by giving them a real reader, rather than needing `#[allow(dead_code)]` or deletion.
- **`state == 7` is an unsourced assumption**, not wrong on its face (state-code-based DQ detection is plausible and other tools may use it) but backed by nothing: not start.gg's schema or public docs, not this repo's own `project-notes.md`, and — checked directly — not even the `hci_startgg_dataset` tool the crawler design spec cites as its source (that tool stores `state` as an opaque raw column and never interprets it). Risk: silent false negatives if the real DQ state code differs from 7, is game-specific, or has changed — nothing here would catch that failure since it's untested with real DQ fixtures.
- **`displayScore` substring match on "DQ" is fragile in both directions.** `extract_scores`'s own doc comment notes `displayScore` "may be 'EntrantTag score - EntrantTag score'" — i.e. it can contain player tag text. A gamer tag literally containing "dq" (e.g. "MrDQ") would false-positive `is_dq` on an otherwise normal completed set, incorrectly excluding a real result from upset-factor computation. Conversely, if start.gg ever renders a DQ'd set's `displayScore` without the literal substring "DQ" (format/locale variance is undocumented), that's a silent false negative.
- **`common::startgg`'s negative-score check isn't necessarily a strict superset.** It only checks score value; it does not consult `state` at all. If start.gg represents some DQ scenarios (e.g., disqualified before any games are scored) via `state` alone with a `null`/non-negative score, `common::startgg`'s check would miss it where crawler's `state == 7` branch would not — this can't be ruled out without real fixture data, and is untested on either side for that scenario.
- **`Entrant.isDisqualified` is correctly unused by both** for per-set DQ — `project-notes.md` explicitly documents it as entrant-level, not per-set, so neither implementation's omission of it here is a bug.
- **Crawler's score-extraction (`extract_scores`) has the same class of fragility as its DQ text-matching**, and for the same root cause: it parses `displayScore` text (`splitn(" - ")`, then trailing whitespace-delimited token parsed as `i16`) instead of reading the already-fetched numeric `stats.score.value` field that `common::startgg::SetNode::scores()` uses directly. Any non-numeric trailing text per side (e.g., unusual formatting) would silently fail to extract scores at all (`extract_scores` returns `None`, and `upsert_set` gets `(None, None)`), whereas structural extraction from `score.value` would work unless the field itself is legitimately absent.

### Where merging looks clean vs. where it doesn't

- **Clean, low-risk fix:** teach `crawler::scraper::is_dq()` and score extraction to primarily read `slot.standing.stats.score.value` (already fetched, already deserialized, just unread) the same way `common::startgg::SetNode::is_dq()`/`.scores()` do, keeping `state == 7` and/or the `displayScore` "DQ" substring as a fallback OR only when score data is absent — mirroring the layered approach crawler already uses for its two existing signals. This requires no architectural changes to either crate.
- **Not clean:** wholesale reuse/merge of `common::startgg::queries::SetNode` into the crawler, or vice versa. The two crates have structurally different GraphQL layers — `common::startgg` has a single `event_sets` query/pagination model with no complexity-budget handling, while `crawler` has a full/slim/games three-query fallback system (`PHASE_GROUP_SETS_QUERY` / `_SLIM` / `PHASE_GROUP_GAMES_QUERY`) purpose-built for start.gg's complexity limits, with its own `FullSetNode`/`SlimSetNode`/`GamesSetNode` types and a different `SetsPage<T>` generic pagination wrapper. Unifying the type layers is a much larger, separate refactor with its own risk profile — not something to bundle into a DQ-detection fix.
- **Open question requiring real data, not just code reading:** whether `state`-based detection genuinely catches DQ cases the negative-score check misses (or vice versa), and what start.gg actually emits for `displayScore` on a DQ'd set across different game titles/bracket types. This needs either real API fixtures/samples or start.gg documentation beyond what's cached locally, not something resolvable by further code reading alone.

---

## Self-review

- No placeholders remain; every decision is either ✅ confirmed or explicitly marked deferred/flagged.
- Internal consistency checked: Part 1 §4a's disposition of `stats.score.value` explicitly cross-references Part 2 rather than duplicating the analysis.
- Scope check: Part 2 is written as findings-only, no proposed code changes, per instruction that it's a follow-up investigation, not part of this cleanup.
- Ambiguity check: the one item needing explicit sign-off in the plan (the `main.rs` module-import fix) is called out as such rather than presented as already-decided.
