# SvelteKit Route Map

## Access control layers

Two guards enforce access control across all routes:

1. **`src/hooks.server.ts`** — runs on every server-side request. Calls `GET /auth/me`
   and attaches the result to `event.locals.user`. For all routes except those listed
   as Public below, an unauthenticated response redirects to `/login`.

2. **`(editor)` group layout** (`src/routes/projects/[id]/(editor)/+layout.server.ts`) —
   checks that the current user is a member of the project. Redirects non-members to
   `/login?redirect=<path>`.

"Owner/member" means the route is accessible to the project owner and all project
members (collaborators). "Published: guest" means unauthenticated users can also
access it when the project's `published` flag is `true`.

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
| `/projects/[id]` | Owner/member (published: guest) | Project root — redirects to `/stats` |
| `/projects/[id]/stats` | Owner/member (published: guest) | Per-player win/loss lists sorted by upset factor |
| `/projects/[id]/h2h` | Owner/member (published: guest) | Head-to-head set record matrix |
| `/projects/[id]/ranking` | Owner/member (published: guest) | Players ordered by aggregate upset factor |
| `/projects/[id]/tournaments` | Owner/member (published: guest) | Tournament list with include/exclude toggles |
| `/projects/[id]/settings` | Owner/member | Project name, game, published flag, member management |
| `/projects/[id]/(editor)/import` | Owner/member | Trigger a start.gg import; view current job status |
| `/projects/[id]/(editor)/players` | Owner/member | Add, remove, and link players |
| `/projects/[id]/(editor)/players/[player_id]` | Owner/member | Edit one player's display name and start.gg accounts |
