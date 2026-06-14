# SvelteKit Route Map

## Access control layers

Two guards enforce access control across all routes:

1. **`src/hooks.server.ts`** — runs on every server-side request. Calls `GET /auth/me`
   and attaches the result to `event.locals.user`. For all routes except those listed
   as Public below, an unauthenticated response redirects to `/login`.

2. **`(editor)` group layout** (`src/routes/projects/[id]/(editor)/+layout.server.ts`) —
   checks that the current user has **editor** or **owner** role on the project. Viewers are redirected.
   Redirects non-members to `/login?redirect=<path>`.

"Owner/member" means the route is accessible to the project owner and all project
members (collaborators). "Published: guest" means unauthenticated users can also
access it when the ranking's `published` flag is `true`.

## Routes

| Path | Access | Purpose |
|---|---|---|
| `/` | Public | Landing page |
| `/login` | Public (redirects if authed) | Login form |
| `/register` | Public (redirects if authed) | Registration form |
| `/logout` | Authenticated | Clears session cookie and redirects to `/login` |
| `/account` | Authenticated | Manage username, password, start.gg API key, delete account |
| `/invite/[token]` | Public | Accept a collaboration invite link |
| `/projects` | Authenticated | List projects owned by or shared with the current user |
| `/projects/new` | Authenticated | Create a new project |
| `/projects/[id]` | Owner/member (published: guest via any ranking) | Lists rankings; redirects to single ranking if only one |
| `/projects/[id]/rankings/new` | Owner/editor | Create a new ranking in this project |
| `/projects/[id]/settings` | Owner only | Project name, game, member management, invite links |
| `/projects/[id]/(editor)/import` | Owner/editor | Trigger a start.gg import; view current job status |
| `/projects/[id]/(editor)/players` | Owner/editor | Add, remove, and link players in the project player pool |
| `/projects/[id]/rankings/[rid]/players/[player_id]` | Owner/member (published: guest) | Per-ranking player stats: wins, losses, and tournament history scoped to the ranking's included events |
| `/projects/[id]/rankings/[rid]` | Owner/member (published: guest) | Ranking overview; redirects editors to /players, viewers/guests to /ranking |
| `/projects/[id]/rankings/[rid]/ranking` | Owner/member (published: guest) | Players ordered by computed_rating (algorithmic) or rank_position (manual); calls `GET /projects/:id/rankings/:rid/ranking` |
| `/projects/[id]/rankings/[rid]/stats` | Owner/member (published: guest) | Per-player win/loss lists sorted by upset factor |
| `/projects/[id]/rankings/[rid]/h2h` | Owner/member (published: guest) | Head-to-head set record matrix |
| `/projects/[id]/rankings/[rid]/tournaments` | Owner/member (published: guest) | Tournament list with per-ranking include/exclude toggles (save-button, bulk `PUT /events`); delete tournament |
| `/projects/[id]/rankings/[rid]/(editor)/players` | Owner/editor | Manage which project players are in this ranking; set rank position and notes |
| `/projects/[id]/rankings/[rid]/(editor)/recompute` | Owner/editor | Manually trigger a ranking recompute via `POST /projects/:id/rankings/:rid/recompute` |
