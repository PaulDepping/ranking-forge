# Code Quality Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Three-layer cleanup of the RankingForge codebase — extract shared utilities, enforce shadcn component policy, replace UX anti-patterns — with no behaviour changes outside Layer 3.

**Architecture:** SvelteKit frontend (`web/`) + Axum backend (`backend/`). Frontend uses Svelte 5 runes, Tailwind CSS v4, shadcn-svelte (bits-ui). Backend uses sqlx with offline query cache in `.sqlx/` (must run `bash backend/prepare-sqlx.sh` after any query move).

**Tech Stack:** Svelte 5, TypeScript, bits-ui, Rust, sqlx, PostgreSQL.

**Test command (after each layer):** `bash test.sh` from repo root.

---

## Layer 1 — Shared utilities & component extraction

---

### Task 1 — Add `winRate()` to `utils.ts` and update `stats/+page.svelte`

- [ ] In `web/src/lib/utils.ts`, add after `toOrdinal`:
  ```ts
  export function winRate(wins: number, losses: number, zeroValue = ''): string {
  	const total = wins + losses;
  	if (total === 0) return zeroValue;
  	return `${Math.round((wins / total) * 100)}%`;
  }
  ```
- [ ] In `web/src/routes/projects/[id]/stats/+page.svelte`:
  - Remove the local `winRate` function (lines 20–24)
  - Add `winRate` to the utils import: `import { formatDate, formatDateTime, winRate } from '$lib/utils'` — wait, `formatDate`/`formatDateTime` are not used in stats; add only `winRate` to the existing utils import (currently not imported)
  - Actually stats doesn't currently import from utils. Add: `import { winRate } from '$lib/utils';`
  - Change the call in the template from `winRate(player.wins.length, player.losses.length)` to `winRate(player.wins.length, player.losses.length, '0%')`
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Task 2 — Update `ranking/+page.svelte` to use shared `winRate()`

- [ ] In `web/src/routes/projects/[id]/ranking/+page.svelte`:
  - Remove the local `winRate(s: PlayerStats | undefined): string` function (lines 92–97)
  - Add `winRate` to imports: `import { winRate } from '$lib/utils';`
  - In the template, replace `{winRate(s)}` with `{winRate(s.wins.length, s.losses.length)}` (the `{#if s}` guard ensures `s` is defined)
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Task 3 — Install `range-calendar` + create `DateRangePicker.svelte`

- [ ] From `web/`, run:
  ```bash
  npx shadcn-svelte@latest add --yes --overwrite range-calendar
  ```
- [ ] Create `web/src/lib/components/DateRangePicker.svelte`:
  ```svelte
  <script lang="ts">
  	import * as Popover from '$lib/components/ui/popover';
  	import { RangeCalendar } from '$lib/components/ui/range-calendar';
  	import { Button } from '$lib/components/ui/button';
  	import type { DateRange } from 'bits-ui';
  	import { getLocalTimeZone } from '@internationalized/date';
  	import { formatDate } from '$lib/utils';

  	let {
  		value,
  		onSelect,
  		placeholder = 'Pick date range',
  	}: {
  		value: DateRange | undefined;
  		onSelect: (range: DateRange | undefined) => void;
  		placeholder?: string;
  	} = $props();

  	let open = $state(false);
  	let pending = $state<DateRange | undefined>(value);
  	$effect(() => { pending = value; });

  	function handleValueChange(range: DateRange | undefined) {
  		pending = range;
  		if (range?.start && range?.end) {
  			onSelect(range);
  			open = false;
  		}
  	}

  	function clearRange() {
  		pending = undefined;
  		onSelect(undefined);
  		open = false;
  	}

  	const triggerLabel = $derived(
  		value?.start && value?.end
  			? `${formatDate(value.start.toDate(getLocalTimeZone()))} – ${formatDate(value.end.toDate(getLocalTimeZone()))}`
  			: placeholder
  	);
  </script>

  <Popover.Root bind:open>
  	<Popover.Trigger>
  		{#snippet child({ props })}
  			<Button {...props} variant="outline" class="justify-start font-normal">
  				{triggerLabel}
  			</Button>
  		{/snippet}
  	</Popover.Trigger>
  	<Popover.Content class="w-auto overflow-hidden p-0" align="start">
  		<div class="space-y-2 p-3">
  			<RangeCalendar
  				value={pending}
  				captionLayout="dropdown"
  				onValueChange={handleValueChange}
  			/>
  			<div class="flex justify-end px-2 pb-1">
  				<Button variant="ghost" size="sm" onclick={clearRange}>Clear</Button>
  			</div>
  		</div>
  	</Popover.Content>
  </Popover.Root>
  ```
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Task 4 — Update `import/+page.svelte` to use `DateRangePicker`

- [ ] In `web/src/routes/projects/[id]/import/+page.svelte`:
  - Remove import: `import Calendar from '$lib/components/ui/calendar/calendar.svelte';`
  - Remove from `'@internationalized/date'` import: `type CalendarDate,` and `getLocalTimeZone` (keep the import line only if other symbols remain — in this case remove the whole `@internationalized/date` import)
  - Remove `import { Label } from '$lib/components/ui/label';`
  - Remove `formatDate` from the utils import (only `formatDateTime` is still needed)
  - Add imports:
    ```ts
    import DateRangePicker from '$lib/components/DateRangePicker.svelte';
    import type { DateRange } from 'bits-ui';
    ```
  - Remove state variables: `afterDate`, `beforeDate`, `afterDateOpen`, `beforeDateOpen`
  - Remove derived variables: `afterDateStr`, `beforeDateStr`
  - Add:
    ```ts
    let dateRange = $state<DateRange | undefined>(undefined);
    const afterDateStr = $derived(dateRange?.start?.toString() ?? '');
    const beforeDateStr = $derived(dateRange?.end?.toString() ?? '');
    ```
  - In the template, replace the entire `<div class="grid grid-cols-2 gap-4">` block (lines 115–160) with:
    ```svelte
    <DateRangePicker
    	value={dateRange}
    	onSelect={(r) => { dateRange = r; }}
    	placeholder="All time"
    />
    ```
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Task 5 — Update `tournaments/+page.svelte` to use `DateRangePicker`

- [ ] In `web/src/routes/projects/[id]/tournaments/+page.svelte`:
  - Remove import: `import Calendar from '$lib/components/ui/calendar/calendar.svelte';`
  - Remove `type CalendarDate` and `getLocalTimeZone` from `'@internationalized/date'` import (remove the entire import line since nothing else uses it)
  - Add imports:
    ```ts
    import DateRangePicker from '$lib/components/DateRangePicker.svelte';
    import type { DateRange } from 'bits-ui';
    ```
  - Remove state variables: `dateFrom`, `dateTo`, `dateFromOpen`, `dateToOpen` (lines 31–34)
  - Remove derived variables: `dateFromStr`, `dateToStr` (lines 36–37)
  - Add:
    ```ts
    let dateRange = $state<DateRange | undefined>(undefined);
    const dateFromStr = $derived(dateRange?.start?.toString() ?? '');
    const dateToStr = $derived(dateRange?.end?.toString() ?? '');
    ```
  - In `resetAllFilters()`, replace `dateFrom = undefined; dateTo = undefined;` with `dateRange = undefined;`
  - In the template, replace the `<div class="flex items-center gap-1.5">` block containing the two From/To popovers (lines 300–342) with:
    ```svelte
    <DateRangePicker
    	value={dateRange}
    	onSelect={(r) => { dateRange = r; }}
    	placeholder="All time"
    />
    ```
    (This replaces the surrounding `<div class="flex items-center gap-1.5">`, the two `<span>` labels, and both `<Popover.Root>` blocks)
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Task 6 — Backend: extract `strip_startgg_url_prefix` helper

- [ ] In `backend/crates/api/src/routes/players.rs`, replace the `// ── Handle normalization` section (lines 590–611) with:
  ```rust
  fn strip_startgg_url_prefix(s: &str) -> &str {
      s.trim_start_matches("https://")
       .trim_start_matches("http://")
       .trim_start_matches("www.start.gg/")
       .trim_start_matches("start.gg/")
  }

  fn normalize_handle(input: &str) -> String {
      strip_startgg_url_prefix(input.trim())
          .trim_start_matches("user/")
          .to_string()
  }

  fn normalize_tournament_handle(input: &str) -> String {
      let stripped = strip_startgg_url_prefix(input.trim())
          .trim_start_matches("tournament/");
      stripped.split('/').next().unwrap_or(stripped).to_string()
  }
  ```
- [ ] Run `cd backend && cargo test -p api -- normalize` — all existing normalization tests must pass (no sqlx DB needed for these)

---

### Task 7 — Backend: extract `PlayerResponse::from_player`

- [ ] In `backend/crates/api/src/routes/players.rs`, add an `impl` block immediately after the `PlayerResponse` struct definition (after line 59):
  ```rust
  impl PlayerResponse {
      fn from_player(p: Player, accounts: Vec<AccountResponse>) -> Self {
          PlayerResponse {
              id: p.id,
              project_id: p.project_id,
              name: p.name,
              created_at: p.created_at,
              accounts,
          }
      }
  }
  ```
- [ ] In `add_player` handler, replace the manual `PlayerResponse { ... }` construction (lines 167–174) with:
  ```rust
  Ok((StatusCode::CREATED, Json(PlayerResponse::from_player(player, vec![]))))
  ```
- [ ] In `rename_player` handler, replace the manual `PlayerResponse { ... }` construction (lines 580–586) with:
  ```rust
  Ok(Json(PlayerResponse::from_player(player, vec![])))
  ```
- [ ] In `list_players`, replace the `PlayerResponse { ... }` construction inside the `.map()` closure (lines 124–130) with:
  ```rust
  PlayerResponse::from_player(p, accounts)
  ```
- [ ] Run `cd backend && cargo build -p api` — expect compile success (no tests needed; no queries changed)

---

### Task 8 — Backend: extract `create_player_with_account` helper + run sqlx prepare

- [ ] In `backend/crates/api/src/routes/players.rs`, add the helper function before the `// ── Bulk add players` section:
  ```rust
  async fn create_player_with_account(
      pool: &sqlx::PgPool,
      project_id: Uuid,
      name: &str,
      user_id: i64,
      handle: &str,
      display_name: Option<&str>,
  ) -> crate::error::Result<Uuid> {
      let player = sqlx::query!(
          "INSERT INTO players (project_id, name, rank_position)
           VALUES (
               $1, $2,
               (SELECT COALESCE(MAX(rank_position), 0) + 1 FROM players WHERE project_id = $1)
           )
           RETURNING id",
          project_id,
          name,
      )
      .fetch_one(pool)
      .await?;

      sqlx::query!(
          "INSERT INTO startgg_accounts (player_id, startgg_user_id, handle, display_name)
           VALUES ($1, $2, $3, $4)",
          player.id,
          user_id,
          handle,
          display_name,
      )
      .execute(pool)
      .await?;

      Ok(player.id)
  }
  ```
- [ ] In `bulk_add_players`, replace the "Insert player" + "Insert startgg account" block (lines 327–351) with:
  ```rust
  create_player_with_account(&state.db, id, &name, user_id, &handle, None).await?;
  results.push(BulkAddResult { name, handle, status: "created" });
  ```
- [ ] In `add_players_by_handles`, replace the "Insert player" + "Insert startgg account" block (lines 425–454) with:
  ```rust
  create_player_with_account(&state.db, id, &gamer_tag, user_id, &handle, Some(&gamer_tag)).await?;
  results.push(ByHandlesResult { handle, name: Some(gamer_tag), status: "created".to_string() });
  ```
- [ ] Run `bash backend/prepare-sqlx.sh` from repo root to regenerate the `.sqlx/` offline cache (moving queries to the helper changes their call-site hash)
- [ ] Run `bash backend/test.sh` — all backend tests must pass

---

### Layer 1 commit

- [ ] `git add web/src/lib/utils.ts web/src/lib/components/DateRangePicker.svelte web/src/lib/components/ui/range-calendar/ web/src/routes/projects/[id]/stats/+page.svelte web/src/routes/projects/[id]/ranking/+page.svelte web/src/routes/projects/[id]/import/+page.svelte web/src/routes/projects/[id]/tournaments/+page.svelte backend/crates/api/src/routes/players.rs backend/.sqlx/`
- [ ] Commit: `refactor: layer 1 — shared winRate, DateRangePicker, backend helpers`

---

## Layer 2 — shadcn policy fixes

---

### Task 9 — Create `AccountBadge.svelte`

- [ ] Create `web/src/lib/components/AccountBadge.svelte`:
  ```svelte
  <script lang="ts">
  	import { enhance } from '$app/forms';
  	import { invalidateAll } from '$app/navigation';
  	import { Badge } from '$lib/components/ui/badge';
  	import { Button } from '$lib/components/ui/button';

  	let { playerId, accountId, displayName, handle }: {
  		playerId: string;
  		accountId: string;
  		displayName: string | null;
  		handle: string;
  	} = $props();
  </script>

  <form
  	method="POST"
  	action="?/unlinkAccount"
  	use:enhance={() => {
  		return async ({ result, update }) => {
  			if (result.type === 'success') {
  				await invalidateAll();
  			} else {
  				await update();
  			}
  		};
  	}}
  	class="inline-flex"
  >
  	<input type="hidden" name="pid" value={playerId} />
  	<input type="hidden" name="aid" value={accountId} />
  	<Badge variant="secondary" class="gap-1 pr-1">
  		{displayName ?? handle}
  		<Button
  			type="submit"
  			variant="ghost"
  			size="icon"
  			class="ml-0.5 h-4 w-4 rounded-full p-0"
  			title="Remove"
  		>×</Button>
  	</Badge>
  </form>
  ```
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Task 10 — Create `PlayerCard.svelte`

- [ ] Create `web/src/lib/components/PlayerCard.svelte`:
  ```svelte
  <script lang="ts">
  	import { enhance } from '$app/forms';
  	import { invalidateAll } from '$app/navigation';
  	import { Button } from '$lib/components/ui/button';
  	import { Input } from '$lib/components/ui/input';
  	import AccountBadge from '$lib/components/AccountBadge.svelte';
  	import type { Player } from '$lib/types';

  	let { player, isEditing, form, onEdit, onCancelEdit, onOpenLinkDialog }: {
  		player: Player;
  		isEditing: boolean;
  		form: { renameError?: string; renamePid?: string } | null;
  		onEdit: () => void;
  		onCancelEdit: () => void;
  		onOpenLinkDialog: () => void;
  	} = $props();

  	let editingName = $state('');
  	$effect(() => {
  		if (isEditing) editingName = player.name;
  	});
  </script>

  <div class="rounded-md border border-border p-3">
  	{#if isEditing}
  		<form
  			method="POST"
  			action="?/renamePlayer"
  			use:enhance={() => {
  				return async ({ result, update }) => {
  					if (result.type === 'success') {
  						onCancelEdit();
  						await invalidateAll();
  					} else {
  						await update();
  					}
  				};
  			}}
  		>
  			<input type="hidden" name="pid" value={player.id} />
  			<div class="flex items-center gap-2">
  				<Input name="name" bind:value={editingName} class="flex-1" />
  				<Button type="submit" size="sm">Save</Button>
  				<Button type="button" variant="ghost" size="sm" onclick={onCancelEdit}>Cancel</Button>
  			</div>
  			{#if form?.renameError && form.renamePid === player.id}
  				<p class="mt-1 text-sm text-destructive">{form.renameError}</p>
  			{/if}
  		</form>
  	{:else}
  		<div class="flex items-start justify-between">
  			<div class="space-y-1">
  				<p class="font-medium">{player.name}</p>
  				<div class="flex flex-wrap gap-1">
  					{#each player.accounts as account (account.id)}
  						<AccountBadge
  							playerId={player.id}
  							accountId={account.id}
  							displayName={account.display_name}
  							handle={account.handle}
  						/>
  					{/each}
  					<Button
  						type="button"
  						variant="link"
  						size="sm"
  						class="h-auto p-0 text-xs"
  						onclick={onOpenLinkDialog}
  					>+ add account</Button>
  				</div>
  			</div>
  			<div class="flex gap-1">
  				<Button type="button" variant="ghost" size="sm" onclick={onEdit}>Edit</Button>
  				<form method="POST" action="?/deletePlayer" use:enhance>
  					<input type="hidden" name="pid" value={player.id} />
  					<Button
  						type="submit"
  						variant="ghost"
  						size="sm"
  						class="text-destructive hover:text-destructive"
  						onclick={(e: MouseEvent) => {
  							if (!confirm(`Remove ${player.name}?`)) e.preventDefault();
  						}}
  					>Remove</Button>
  				</form>
  			</div>
  		</div>
  	{/if}
  </div>
  ```
  Note: The `window.confirm` on Remove stays here; it is replaced by AlertDialog in Task 17.
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Task 11 — Simplify `players/+page.svelte` using `PlayerCard`

- [ ] In `web/src/routes/projects/[id]/players/+page.svelte`:
  - Remove imports: `Badge`, raw button usage
  - Add import: `import PlayerCard from '$lib/components/PlayerCard.svelte';`
  - Remove `editingName` state variable (now internal to PlayerCard)
  - Keep: `editingPid`, `openLinkDialog`, `startEdit`, `cancelEdit`
  - Replace the `{#if editingPid === player.id} ... {:else} ... {/if}` block (~lines 54–139) with:
    ```svelte
    <PlayerCard
    	{player}
    	isEditing={editingPid === player.id}
    	{form}
    	onEdit={() => startEdit(player.id, player.name)}
    	onCancelEdit={cancelEdit}
    	onOpenLinkDialog={() => openLinkDialog(player.id, player.name)}
    />
    ```
  - Remove the `startEdit` function's setting of `editingName` (since PlayerCard manages it internally). Keep `startEdit` to set `editingPid`:
    ```ts
    function startEdit(pid: string, _name: string) {
    	editingPid = pid;
    }
    ```
    Or simplify directly: `onEdit={() => { editingPid = player.id; }}`
  - Remove unused imports: `Badge` (if no longer used in this file)
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Task 12 — Fix raw `<button>` elements in `h2h/+page.svelte`

- [ ] In `web/src/routes/projects/[id]/h2h/+page.svelte`, replace the matrix cell raw `<button>` (line 124):
  ```svelte
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
  ```
  with:
  ```svelte
  <Button
  	variant="ghost"
  	class="h-auto rounded px-1 py-0
  		{isSelected(row.id, col.id)
  			? 'ring-2 ring-primary bg-primary/10 hover:bg-primary/10'
  			: rec.wins > rec.losses
  				? 'bg-green-50 dark:bg-green-950/30 text-green-700 dark:text-green-400'
  				: rec.wins < rec.losses
  					? 'bg-red-50 dark:bg-red-950/30 text-red-700 dark:text-red-400'
  					: ''}"
  	onclick={() => selectCell(row, col)}
  >
  	{rec.wins}–{rec.losses}
  </Button>
  ```
- [ ] Replace the set-row raw `<button>` (line 181):
  ```svelte
  <button
  	class="w-full flex items-center gap-2 rounded px-2 py-1.5 text-xs hover:bg-muted/50 border-b border-border last:border-0"
  	onclick={() => { selectedSet = set; selectedIsWin = set.is_win; }}
  >
  ```
  with:
  ```svelte
  <Button
  	variant="ghost"
  	class="h-auto w-full flex items-center gap-2 rounded px-2 py-1.5 text-xs border-b border-border last:border-0 justify-start"
  	onclick={() => { selectedSet = set; selectedIsWin = set.is_win; }}
  >
  ```
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Task 13 — Fix raw `<button>` elements in `stats/+page.svelte`

- [ ] In `web/src/routes/projects/[id]/stats/+page.svelte`, replace the wins raw `<button>` (line 55):
  ```svelte
  <button
  	class="w-full border-b border-border px-2 py-1 text-left text-xs last:border-0 hover:bg-muted/50"
  	onclick={() => openModal(set, true, player.name)}
  >
  	<span>{set.opponent_name} · UF {set.upset_factor}</span>
  </button>
  ```
  with:
  ```svelte
  <Button
  	variant="ghost"
  	class="h-auto w-full border-b border-border px-2 py-1 text-xs last:border-0 justify-start"
  	onclick={() => openModal(set, true, player.name)}
  >
  	{set.opponent_name} · UF {set.upset_factor}
  </Button>
  ```
- [ ] Replace the losses raw `<button>` (line 69) with the same pattern:
  ```svelte
  <Button
  	variant="ghost"
  	class="h-auto w-full border-b border-border px-2 py-1 text-xs last:border-0 justify-start"
  	onclick={() => openModal(set, false, player.name)}
  >
  	{set.opponent_name} · UF {set.upset_factor}
  </Button>
  ```
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Task 14 — Fix raw `<button>` and `<label>` in `tournaments/+page.svelte`

- [ ] In `web/src/routes/projects/[id]/tournaments/+page.svelte`:
  - Add `import { Label } from '$lib/components/ui/label';` to the script imports
  - In the `{#snippet bracketRow(bt)}` block, replace the raw `<button>` (line 220):
    ```svelte
    <button
    	type="button"
    	onclick={() => setBracketState(bt, s)}
    	class="h-5 w-5 mx-auto rounded border text-xs font-bold flex items-center justify-center
    		{bracketFilter[bt] === s
    			? s === 'required'
    				? 'border-green-500 bg-green-950 text-green-400'
    				: s === 'excluded'
    					? 'border-red-500 bg-red-950 text-red-400'
    					: 'border-indigo-500 bg-indigo-950 text-indigo-400'
    			: 'border-border bg-muted/30 text-transparent hover:text-muted-foreground'}"
    >
    	{s === 'neutral' ? '–' : s === 'required' ? '✓' : '✕'}
    </button>
    ```
    with:
    ```svelte
    <Button
    	type="button"
    	size="icon"
    	variant="ghost"
    	class="h-5 w-5 mx-auto rounded border text-xs font-bold flex items-center justify-center
    		{bracketFilter[bt] === s
    			? s === 'required'
    				? 'border-green-500 bg-green-950 text-green-400 hover:bg-green-950'
    				: s === 'excluded'
    					? 'border-red-500 bg-red-950 text-red-400 hover:bg-red-950'
    					: 'border-indigo-500 bg-indigo-950 text-indigo-400 hover:bg-indigo-950'
    			: 'border-border bg-muted/30 text-transparent hover:text-muted-foreground'}"
    	onclick={() => setBracketState(bt, s)}
    >
    	{s === 'neutral' ? '–' : s === 'required' ? '✓' : '✕'}
    </Button>
    ```
  - Replace the raw `<label>` (line 507):
    ```svelte
    <label class="flex cursor-pointer items-center justify-between px-4 py-2 hover:bg-accent/50">
    ```
    with:
    ```svelte
    <Label class="flex cursor-pointer items-center justify-between px-4 py-2 hover:bg-accent/50">
    ```
    and its closing `</label>` with `</Label>`
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Layer 2 commit

- [ ] `git add web/src/lib/components/AccountBadge.svelte web/src/lib/components/PlayerCard.svelte web/src/routes/projects/[id]/players/+page.svelte web/src/routes/projects/[id]/h2h/+page.svelte web/src/routes/projects/[id]/stats/+page.svelte web/src/routes/projects/[id]/tournaments/+page.svelte`
- [ ] Commit: `refactor: layer 2 — AccountBadge, PlayerCard, shadcn button/label replacements`

---

## Layer 3 — UX improvements

---

### Task 15 — Install `alert-dialog`

- [ ] From `web/`, run:
  ```bash
  npx shadcn-svelte@latest add --yes --overwrite alert-dialog
  ```
- [ ] Verify `web/src/lib/components/ui/alert-dialog/` was created
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Task 16 — Fix `location.href` navigation in `+layout.svelte`

- [ ] In `web/src/routes/+layout.svelte`, add `import { goto } from '$app/navigation';` to the script imports
- [ ] Replace `location.href = '/login';` with `goto('/login');`
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Task 17 — Replace `window.confirm` in `PlayerCard.svelte` with AlertDialog

- [ ] In `web/src/lib/components/PlayerCard.svelte`, add to script:
  ```ts
  import * as AlertDialog from '$lib/components/ui/alert-dialog';

  let deleteDialogOpen = $state(false);
  let deleteFormEl = $state<HTMLFormElement | null>(null);
  ```
- [ ] On the delete `<form>`, add `bind:this={deleteFormEl}`
- [ ] Change the Remove `Button` from `type="submit"` to `type="button"` and replace its `onclick` with:
  ```ts
  onclick={() => { deleteDialogOpen = true; }}
  ```
  (remove the `e.preventDefault()` confirm logic)
- [ ] Add the AlertDialog after the closing `</div>` of the view-mode block (inside the `{:else}` branch, before the `{/if}`):
  ```svelte
  <AlertDialog.Root bind:open={deleteDialogOpen}>
  	<AlertDialog.Content>
  		<AlertDialog.Header>
  			<AlertDialog.Title>Remove {player.name}?</AlertDialog.Title>
  			<AlertDialog.Description>
  				This will permanently remove the player and all their stats from this project.
  			</AlertDialog.Description>
  		</AlertDialog.Header>
  		<AlertDialog.Footer>
  			<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
  			<AlertDialog.Action
  				class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
  				onclick={() => deleteFormEl?.requestSubmit()}
  			>Remove</AlertDialog.Action>
  		</AlertDialog.Footer>
  	</AlertDialog.Content>
  </AlertDialog.Root>
  ```
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Task 18 — Replace `window.confirm` in `settings/+page.svelte` with AlertDialog

- [ ] In `web/src/routes/projects/[id]/settings/+page.svelte`, add to script:
  ```ts
  import * as AlertDialog from '$lib/components/ui/alert-dialog';

  let deleteDialogOpen = $state(false);
  let deleteFormEl = $state<HTMLFormElement | null>(null);
  ```
- [ ] On the delete `<form>` (line 54), add `bind:this={deleteFormEl}` and remove `use:enhance` (the delete navigates away, so no progressive enhancement is needed — but keep it if it was there; checking: `use:enhance` is present on the form so keep it)
- [ ] Change the Delete project `Button` to `type="button"` and replace its `onclick` with:
  ```ts
  onclick={() => { deleteDialogOpen = true; }}
  ```
- [ ] Add the AlertDialog after the closing `</div>` of the danger zone section (outside the main `<div class="max-w-lg space-y-8">`):
  ```svelte
  <AlertDialog.Root bind:open={deleteDialogOpen}>
  	<AlertDialog.Content>
  		<AlertDialog.Header>
  			<AlertDialog.Title>Delete this project?</AlertDialog.Title>
  			<AlertDialog.Description>
  				Permanently removes all players, tournaments, and stats. This cannot be undone.
  			</AlertDialog.Description>
  		</AlertDialog.Header>
  		<AlertDialog.Footer>
  			<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
  			<AlertDialog.Action
  				class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
  				onclick={() => deleteFormEl?.requestSubmit()}
  			>Delete project</AlertDialog.Action>
  		</AlertDialog.Footer>
  	</AlertDialog.Content>
  </AlertDialog.Root>
  ```
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Task 19 — Replace `window.confirm` in `import/+page.svelte` with AlertDialog

- [ ] In `web/src/routes/projects/[id]/import/+page.svelte`, add to script:
  ```ts
  import * as AlertDialog from '$lib/components/ui/alert-dialog';

  let importDialogOpen = $state(false);
  let importFormEl = $state<HTMLFormElement | null>(null);
  ```
- [ ] On the main `<form>` (line 99), add `bind:this={importFormEl}` and remove the `({ cancel })` confirm logic from `use:enhance`, so it becomes:
  ```svelte
  use:enhance={() => {
  	return ({ result }) => {
  		if (result.type === 'success' && result.data?.job) {
  			job = result.data.job as Job;
  		}
  	};
  }}
  ```
- [ ] Change the submit `Button` to `type="button"` with:
  ```svelte
  <Button
  	type="button"
  	onclick={() => {
  		if (isActiveJob) {
  			importDialogOpen = true;
  		} else {
  			importFormEl?.requestSubmit();
  		}
  	}}
  >
  	{job ? 'Re-import' : 'Start import'}
  </Button>
  ```
- [ ] Add the AlertDialog after the closing `</form>` tag:
  ```svelte
  <AlertDialog.Root bind:open={importDialogOpen}>
  	<AlertDialog.Content>
  		<AlertDialog.Header>
  			<AlertDialog.Title>Import already running</AlertDialog.Title>
  			<AlertDialog.Description>
  				An import is currently in progress. Start a new one anyway?
  			</AlertDialog.Description>
  		</AlertDialog.Header>
  		<AlertDialog.Footer>
  			<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
  			<AlertDialog.Action onclick={() => importFormEl?.requestSubmit()}>
  				Start import
  			</AlertDialog.Action>
  		</AlertDialog.Footer>
  	</AlertDialog.Content>
  </AlertDialog.Root>
  ```
- [ ] Run `cd web && npm run test:unit` — expect pass

---

### Layer 3 commit

- [ ] `git add web/src/lib/components/ui/alert-dialog/ web/src/routes/+layout.svelte web/src/lib/components/PlayerCard.svelte web/src/routes/projects/[id]/settings/+page.svelte web/src/routes/projects/[id]/import/+page.svelte`
- [ ] Commit: `refactor: layer 3 — AlertDialog confirms, goto() navigation`

---

## Final verification

- [ ] Run `bash test.sh` from repo root — all sections (backend, frontend unit, frontend e2e) must pass
- [ ] Manually smoke-test in browser: date range picker in import and tournaments, player remove dialog, project delete dialog, import-while-running dialog, logout navigation
