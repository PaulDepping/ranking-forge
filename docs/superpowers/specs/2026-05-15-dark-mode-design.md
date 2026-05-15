# Dark Mode Design

**Date:** 2026-05-15
**Status:** Approved

## Overview

Add dark mode support to all pages using the `mode-watcher` package (the canonical shadcn-svelte approach). The `.dark` CSS class and all dark-mode CSS variables are already defined in `app.css` — only the activation mechanism and toggle UI are missing.

## Infrastructure

Install `mode-watcher` as a production dependency. Add `<ModeWatcher />` to the root layout (`src/routes/+layout.svelte`). On mount, `mode-watcher` reads the user's saved preference from `localStorage`; if none exists, it falls back to the OS `prefers-color-scheme` value. It then toggles the `dark` class on `<html>` reactively, activating the existing dark-mode CSS variables across the entire app.

No changes to `app.css` are required.

## Toggle Component

Create `src/lib/components/ThemeToggle.svelte`. It renders a shadcn `Button` (variant `ghost`, size `icon`) containing a Sun or Moon Lucide icon that reflects the current mode. Clicking it calls `toggleMode()` from `mode-watcher`, which flips the mode and persists the choice to `localStorage`.

## Placement

Add `<ThemeToggle />` to the nav header in `src/routes/+layout.svelte`, between the username display and the Logout button.

## Files Changed

| File | Change |
|------|--------|
| `web/package.json` | Add `mode-watcher` dependency |
| `web/src/routes/+layout.svelte` | Import and render `<ModeWatcher />` and `<ThemeToggle />` |
| `web/src/lib/components/ThemeToggle.svelte` | New component — sun/moon toggle button |

## Out of Scope

- Per-page theming or custom colour overrides
- Dropdown with explicit Light / Dark / System options
- Server-side theme detection via cookies
