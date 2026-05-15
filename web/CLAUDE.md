# web/CLAUDE.md

Frontend-specific guidance for the SvelteKit app in this directory.

## Stack

- SvelteKit + TypeScript (Svelte 5 with runes)
- Tailwind CSS v4
- shadcn-svelte (component library, built on bits-ui)
- mode-watcher (dark/light mode)
- Vitest (unit tests) + Playwright (e2e tests)

## Component policy

**Always prefer shadcn-svelte over raw HTML.** Every interactive primitive should come from the library, not be hand-rolled with manual Tailwind classes.

- Never write a raw `<input>`, `<select>`, `<button>`, `<table>`, `<dialog>`, or `<label>` when a shadcn equivalent exists.
- Never copy Tailwind class strings that replicate what a shadcn component already provides (e.g. `flex h-9 w-full rounded-md border border-input...` instead of `<Input>`).
- For navigation tabs, use `Tabs` + `TabsList` + `TabsTrigger` (bind `value` to the current route segment, use `onValueChange` + `goto()` for navigation — no `TabsContent` needed since SvelteKit renders page content).
- For dropdowns and autocomplete, use the `Command` + `Popover` pattern rather than a hand-rolled `<ul>/<li>` list.
- For popovers, use `Popover` + `PopoverTrigger` + `PopoverContent` — never manage click-outside detection manually.

**Adding components:** `npx shadcn-svelte@latest add <name>` — do not write component files by hand. Installed components live in `src/lib/components/ui/`.

### Currently installed components

| Component | Import path |
|---|---|
| Alert | `$lib/components/ui/alert` |
| Badge | `$lib/components/ui/badge` |
| Button | `$lib/components/ui/button` |
| Card | `$lib/components/ui/card` |
| Dialog | `$lib/components/ui/dialog` |
| Input | `$lib/components/ui/input` |
| Label | `$lib/components/ui/label` |
| Select | `$lib/components/ui/select` |
| Separator | `$lib/components/ui/separator` |
| Table | `$lib/components/ui/table` |

Update this list when adding new components.

## API client

`src/lib/api.ts` is the fetch wrapper. All API calls go through it — never use raw `fetch` in page files. It sets `credentials: 'include'` and prefixes `PUBLIC_API_URL` (client-side) or `INTERNAL_API_URL` (server-side).

## Dark mode

`ModeWatcher` (from mode-watcher) is mounted once in `+layout.svelte`. Use Tailwind's `dark:` variant for any colors not covered by CSS variables. Toggle is in `ThemeToggle.svelte`.

## Testing

```bash
# Unit tests (Vitest)
npm run test:unit

# e2e tests (Playwright — auto-starts mock API on :9999 and dev server on :5174)
npm run test:e2e
```

The Playwright mock API lives in `tests/` and intercepts all `PUBLIC_API_URL` calls. e2e tests cannot log in as a real user (no cookie injection) — auth-protected flows are tested at the unit level.
