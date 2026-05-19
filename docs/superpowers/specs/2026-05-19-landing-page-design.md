# Landing Page Design

## Overview

A public marketing landing page at the root URL (`/`) targeting panelists who are unfamiliar with RankingForge. Tone is scene-native — speaks the language of the Smash community. The app header is also upgraded to a NavigationMenu that renders for all users and adapts to auth state.

## Header

Built with the `NavigationMenu` shadcn-svelte component. Always visible regardless of auth state.

**Structure (left → right):**
- Brand link "RankingForge" → `/` (always)
- "Projects" NavigationMenu.Link (logged-in only; extensible with future links)
- `ml-auto` separator
- ThemeToggle (always)
- Logged out: Button outline "Sign in" → `/login`, Button default "Register" → `/register`
- Logged in: username (muted text), Button ghost "Logout"

## Auth Routing

Both logged-in and logged-out users see the landing page. No redirect. CTAs on the page adapt based on `data.user` (available from the layout's server load).

## Landing Page Sections

### 1. Hero

- **Headline:** "The data behind your power rankings."
- **Subheadline:** "Pull your tournament data from start.gg, curate which events count, and get the stats to back up your ranking decisions."
- **CTAs (logged out):** Button default "Get started" → `/register` + Button outline "Sign in" → `/login`
- **CTAs (logged in):** Button default "Go to your projects" → `/projects`
- Centered layout, generous vertical padding

### 2. Feature Highlights

Four `Card` components in a 2×2 grid (single column on mobile):

| # | Title | Description |
|---|---|---|
| 1 | Import from start.gg | Fetch your tournament history automatically — just provide the player slugs. |
| 2 | Curate your events | Manually exclude tournaments that shouldn't count. You stay in control of what goes into the ranking. |
| 3 | Stats at a glance | Per-player win/loss breakdowns and head-to-head tables, ready to reference when you're building your list. |
| 4 | Collaborate with your panel | Invite other panelists to work on a ranking together — multiple people can contribute to the same project. |

### 3. How It Works

Three numbered steps. `Badge` for step numbers.

| Step | Title | Description |
|---|---|---|
| 1 | Create a project | Add the players you want to rank using their start.gg slugs. Invite your fellow panelists to collaborate. |
| 2 | Import & curate | We fetch the tournament history automatically. Deselect any events that shouldn't count toward the ranking. |
| 3 | Build your ranking | Use the win/loss breakdowns and head-to-head tables to inform your panel's decisions. |

### 4. Footer

`Separator` above. Centered muted text:

> Created by King · [Source on GitHub](https://github.com/PaulDepping/ranking-forge) · [Open source under AGPL v3](https://www.gnu.org/licenses/agpl-3.0.html)

## Components Used

- `NavigationMenu` (needs installation)
- `Button` (default, outline, ghost variants)
- `Card`, `CardHeader`, `CardTitle`, `CardContent`
- `Badge`
- `Separator`
- `ThemeToggle` (existing)

## Files Changed

| File | Change |
|---|---|
| `web/src/routes/+layout.svelte` | Replace header with NavigationMenu |
| `web/src/routes/+page.server.ts` | Delete (layout load already provides `data.user`) |
| `web/src/routes/+page.svelte` | Replace placeholder with landing page |
