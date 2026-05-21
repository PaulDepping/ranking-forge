# API Client Separation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate the server-side and client-side API helpers so the session cookie is forwarded once in `hooks.server.ts` instead of in every route, fix cookie set/delete symmetry, and ensure all `.svelte` files go through the unified client helper.

**Architecture:** Create `src/lib/server/api.ts` (server-only, reads `INTERNAL_API_URL`, forwards session cookie) and simplify `src/lib/api.ts` (client-only, reads `PUBLIC_API_URL` internally, relies on `credentials: "include"`). The hook builds `locals.api` once per request; every `+page.server.ts` uses `locals.api` with no cookie plumbing. Login/register read `session_id` from the API response body instead of parsing `Set-Cookie` header with a regex.

**Tech Stack:** SvelteKit 2, Svelte 5, TypeScript, Vitest, Axum (Rust)

---

### Task 1: Add `$env/dynamic/private` alias to vitest config

**Files:**
- Create: `web/src/__mocks__/env.private.ts`
- Modify: `web/vitest.config.ts`

- [ ] **Step 1: Create the private env mock**

```typescript
// web/src/__mocks__/env.private.ts
export const env = {
  INTERNAL_API_URL: "http://localhost:8080",
  COOKIE_DOMAIN: "",
};
```

- [ ] **Step 2: Add alias to vitest config**

Replace the `resolve.alias` block in `web/vitest.config.ts`:

```typescript
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { resolve } from 'path';

export default defineConfig({
	plugins: [svelte()],
	resolve: {
		alias: {
			$lib: resolve('./src/lib'),
			'$env/dynamic/public': resolve('./src/__mocks__/env.ts'),
			'$env/dynamic/private': resolve('./src/__mocks__/env.private.ts'),
		},
		conditions: ['browser']
	},
	test: {
		include: ['src/**/*.{test,spec}.{js,ts}'],
		globals: true,
		environment: 'jsdom',
		setupFiles: ['src/setupTests.ts']
	}
});
```

- [ ] **Step 3: Verify existing tests still pass**

Run from `web/`:
```bash
npm run test:unit
```
Expected: all existing tests pass (same as before).

- [ ] **Step 4: Commit**

```bash
git add web/src/__mocks__/env.private.ts web/vitest.config.ts
git commit -m "test: add \$env/dynamic/private mock alias for vitest"
```

---

### Task 2: Create `src/lib/server/api.ts` (TDD)

**Files:**
- Create: `web/src/lib/server/api.ts`
- Create: `web/src/lib/server/api.test.ts`

- [ ] **Step 1: Write the failing tests**

```typescript
// web/src/lib/server/api.test.ts
import { describe, it, expect, vi } from "vitest";
import { makeServerApi } from "./api";

describe("makeServerApi", () => {
  it("forwards session cookie when sessionId is provided", async () => {
    const mockFetch = vi.fn().mockResolvedValue(new Response("{}", { status: 200 }));
    const api = makeServerApi(mockFetch, "test-session-id");
    await api.get("/projects");
    expect(mockFetch).toHaveBeenCalledWith(
      "http://localhost:8080/projects",
      expect.objectContaining({
        headers: expect.objectContaining({ Cookie: "session_id=test-session-id" }),
      }),
    );
  });

  it("omits Cookie header when sessionId is undefined", async () => {
    const mockFetch = vi.fn().mockResolvedValue(new Response("{}", { status: 200 }));
    const api = makeServerApi(mockFetch, undefined);
    await api.get("/test");
    const headers = (mockFetch.mock.calls[0][1] as RequestInit).headers as Record<string, string>;
    expect(headers.Cookie).toBeUndefined();
  });

  it("sends POST with JSON body and Content-Type header", async () => {
    const mockFetch = vi.fn().mockResolvedValue(new Response("{}", { status: 200 }));
    const api = makeServerApi(mockFetch, "sid");
    await api.post("/projects", { name: "Test" });
    expect(mockFetch).toHaveBeenCalledWith(
      "http://localhost:8080/projects",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ name: "Test" }),
        headers: expect.objectContaining({ "Content-Type": "application/json" }),
      }),
    );
  });

  it("sends DELETE with no body", async () => {
    const mockFetch = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
    const api = makeServerApi(mockFetch, "sid");
    await api.delete("/projects/1");
    expect(mockFetch).toHaveBeenCalledWith(
      "http://localhost:8080/projects/1",
      expect.objectContaining({ method: "DELETE", body: undefined }),
    );
  });

  it("returns the raw fetch response", async () => {
    const mockResponse = new Response(JSON.stringify({ id: "1" }), { status: 200 });
    const mockFetch = vi.fn().mockResolvedValue(mockResponse);
    const api = makeServerApi(mockFetch, "sid");
    const result = await api.get("/projects/1");
    expect(result).toBe(mockResponse);
  });
});
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cd web && npm run test:unit -- --reporter=verbose src/lib/server/api.test.ts
```
Expected: `Error: Cannot find module './api'` or similar — confirms tests fail before implementation.

- [ ] **Step 3: Implement `src/lib/server/api.ts`**

```typescript
// web/src/lib/server/api.ts
import { env } from "$env/dynamic/private";

export function makeServerApi(fetchFn: typeof fetch, sessionId: string | undefined) {
  async function req(method: string, path: string, body?: unknown): Promise<Response> {
    const headers: Record<string, string> = {};
    if (sessionId) headers["Cookie"] = `session_id=${sessionId}`;
    if (body !== undefined) headers["Content-Type"] = "application/json";
    return fetchFn(env.INTERNAL_API_URL + path, {
      method,
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
  }

  return {
    get: (path: string) => req("GET", path),
    post: (path: string, body?: unknown) => req("POST", path, body),
    patch: (path: string, body: unknown) => req("PATCH", path, body),
    put: (path: string, body: unknown) => req("PUT", path, body),
    delete: (path: string) => req("DELETE", path),
  };
}

export type ServerApi = ReturnType<typeof makeServerApi>;
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cd web && npm run test:unit -- --reporter=verbose src/lib/server/api.test.ts
```
Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/lib/server/api.ts web/src/lib/server/api.test.ts
git commit -m "feat: add server-only API helper with session cookie forwarding"
```

---

### Task 3: Update `src/lib/api.ts` and its tests (TDD)

**Files:**
- Modify: `web/src/lib/api.ts`
- Modify: `web/src/lib/api.test.ts`

The client API no longer takes `baseUrl` or `sessionId` — it reads `PUBLIC_API_URL` from `$env/dynamic/public` internally.

- [ ] **Step 1: Update the tests to the new signature**

Replace the entire contents of `web/src/lib/api.test.ts`:

```typescript
import { describe, it, expect, vi } from "vitest";
import { makeApi } from "./api";

// PUBLIC_API_URL is mocked as "http://localhost:8080" via src/__mocks__/env.ts

describe("makeApi", () => {
  it("sends GET with credentials:include to PUBLIC_API_URL", async () => {
    const mockFetch = vi.fn().mockResolvedValue(new Response("{}"));
    const api = makeApi(mockFetch);

    await api.get("/projects");

    expect(mockFetch).toHaveBeenCalledWith(
      "http://localhost:8080/projects",
      expect.objectContaining({ method: "GET", credentials: "include" }),
    );
  });

  it("sends POST with JSON body and Content-Type header", async () => {
    const mockFetch = vi.fn().mockResolvedValue(new Response("{}"));
    const api = makeApi(mockFetch);

    await api.post("/projects", { name: "Test" });

    expect(mockFetch).toHaveBeenCalledWith("http://localhost:8080/projects", {
      method: "POST",
      credentials: "include",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "Test" }),
    });
  });

  it("sends POST without body or Content-Type when no body given", async () => {
    const mockFetch = vi.fn().mockResolvedValue(new Response("{}"));
    const api = makeApi(mockFetch);

    await api.post("/auth/logout");

    expect(mockFetch).toHaveBeenCalledWith(
      "http://localhost:8080/auth/logout",
      {
        method: "POST",
        credentials: "include",
        headers: {},
        body: undefined,
      },
    );
  });

  it("sends PATCH with JSON body", async () => {
    const mockFetch = vi.fn().mockResolvedValue(new Response("{}"));
    const api = makeApi(mockFetch);

    await api.patch("/projects/1/events/2", { included: false });

    expect(mockFetch).toHaveBeenCalledWith(
      "http://localhost:8080/projects/1/events/2",
      {
        method: "PATCH",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ included: false }),
      },
    );
  });

  it("sends DELETE with no body or Content-Type", async () => {
    const mockFetch = vi
      .fn()
      .mockResolvedValue(new Response(null, { status: 200 }));
    const api = makeApi(mockFetch);

    await api.delete("/projects/1");

    expect(mockFetch).toHaveBeenCalledWith("http://localhost:8080/projects/1", {
      method: "DELETE",
      credentials: "include",
      headers: {},
      body: undefined,
    });
  });

  it("returns the raw fetch response", async () => {
    const mockResponse = new Response(JSON.stringify({ id: "1" }), {
      status: 200,
    });
    const mockFetch = vi.fn().mockResolvedValue(mockResponse);
    const api = makeApi(mockFetch);

    const result = await api.get("/projects/1");

    expect(result).toBe(mockResponse);
  });
});
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cd web && npm run test:unit -- --reporter=verbose src/lib/api.test.ts
```
Expected: failures because `makeApi` still takes `(fetchFn, baseUrl, sessionId?)`.

- [ ] **Step 3: Update `src/lib/api.ts`**

Replace the entire file:

```typescript
// web/src/lib/api.ts
import { env } from "$env/dynamic/public";

export function makeApi(fetchFn: typeof fetch) {
  async function req(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<Response> {
    const headers: Record<string, string> = {};
    if (body !== undefined) headers["Content-Type"] = "application/json";
    return fetchFn(env.PUBLIC_API_URL + path, {
      method,
      credentials: "include",
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
  }

  return {
    get: (path: string) => req("GET", path),
    post: (path: string, body?: unknown) => req("POST", path, body),
    patch: (path: string, body: unknown) => req("PATCH", path, body),
    put: (path: string, body: unknown) => req("PUT", path, body),
    delete: (path: string) => req("DELETE", path),
    putRanking: (projectId: string, playerIds: string[]) =>
      req("PUT", `/projects/${projectId}/ranking`, { player_ids: playerIds }),
  };
}
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cd web && npm run test:unit -- --reporter=verbose src/lib/api.test.ts
```
Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/lib/api.ts web/src/lib/api.test.ts
git commit -m "refactor: client makeApi reads PUBLIC_API_URL internally, removes baseUrl/sessionId params"
```

---

### Task 4: Wire up `app.d.ts` and `hooks.server.ts`

**Files:**
- Modify: `web/src/app.d.ts`
- Modify: `web/src/hooks.server.ts`

- [ ] **Step 1: Add `api` to `App.Locals` in `app.d.ts`**

Replace entire file:

```typescript
// web/src/app.d.ts
// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
  namespace App {
    // interface Error {}
    interface Locals {
      user: {
        id: string;
        email: string;
        display_name: string;
        has_startgg_key: boolean;
        created_at: string;
      } | null;
      api: import("$lib/server/api").ServerApi;
    }
    // interface PageData {}
    // interface PageState {}
    // interface Platform {}
  }
}

export {};
```

- [ ] **Step 2: Update `hooks.server.ts` to build `locals.api`**

Replace entire file:

```typescript
// web/src/hooks.server.ts
import type { Handle } from "@sveltejs/kit";
import { redirect } from "@sveltejs/kit";
import { makeServerApi } from "$lib/server/api";

export const handle: Handle = async ({ event, resolve }) => {
  const { pathname } = event.url;

  const sessionId = event.cookies.get("session_id");
  event.locals.api = makeServerApi(event.fetch, sessionId);

  const res = await event.locals.api.get("/auth/me");
  if (res.ok) {
    event.locals.user = await res.json();
  } else {
    event.locals.user = null;
    const isPublic =
      pathname === "/" ||
      ["/login", "/register"].includes(pathname) ||
      /^\/projects\/[^/]/.test(pathname) ||
      /^\/invite\//.test(pathname);
    if (!isPublic) {
      redirect(303, "/login");
    }
  }

  return resolve(event);
};
```

- [ ] **Step 3: Run unit tests to ensure no TypeScript errors were introduced**

```bash
cd web && npm run test:unit
```
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add web/src/app.d.ts web/src/hooks.server.ts
git commit -m "feat: build locals.api once in hooks using makeServerApi"
```

---

### Task 5: Update all `+page.server.ts` files to use `locals.api`

**Files:** (all modified)
- `web/src/routes/projects/+page.server.ts`
- `web/src/routes/projects/new/+page.server.ts`
- `web/src/routes/projects/[id]/+layout.server.ts`
- `web/src/routes/projects/[id]/(editor)/import/+page.server.ts`
- `web/src/routes/projects/[id]/(editor)/players/+page.server.ts`
- `web/src/routes/projects/[id]/(editor)/players/[player_id]/+page.server.ts`
- `web/src/routes/projects/[id]/h2h/+page.server.ts`
- `web/src/routes/projects/[id]/ranking/+page.server.ts`
- `web/src/routes/projects/[id]/settings/+page.server.ts`
- `web/src/routes/projects/[id]/stats/+page.server.ts`
- `web/src/routes/projects/[id]/tournaments/+page.server.ts`
- `web/src/routes/invite/[token]/+page.server.ts`
- `web/src/routes/account/+page.server.ts`

The pattern for every file is the same:
- Remove `import { makeApi } from "$lib/api"`
- Remove `import { env } from "$env/dynamic/private"` (account/login/logout/register still need it for `COOKIE_DOMAIN` — handle those in Task 7)
- Remove `cookies` from the function parameter destructure (in load functions/actions that only used it for session forwarding)
- Remove `fetch` from load functions that only used it to pass to `makeApi`
- Replace `const api = makeApi(fetch, env.INTERNAL_API_URL, cookies.get("session_id"))` + `api.method(...)` with `locals.api.method(...)`

- [ ] **Step 1: Update `routes/projects/+page.server.ts`**

```typescript
import type { PageServerLoad } from "./$types";
import type { Project } from "$lib/types";

export const load: PageServerLoad = async ({ locals }) => {
  const res = await locals.api.get("/projects");
  if (!res.ok) return { projects: [] as Project[] };
  const projects: Project[] = await res.json();
  return { projects };
};
```

- [ ] **Step 2: Update `routes/projects/new/+page.server.ts`**

```typescript
import { fail, redirect } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";

export const load: PageServerLoad = ({ locals }) => {
  return { hasStartggKey: locals.user?.has_startgg_key ?? false };
};

export const actions: Actions = {
  default: async ({ request, locals }) => {
    const data = await request.formData();
    const name = (data.get("name") as string)?.trim();
    const game_id_raw = data.get("game_id") as string | null;
    const game_name = (data.get("game_name") as string | null) || null;

    if (!name) return fail(422, { error: "Project name is required" });

    const body: Record<string, unknown> = { name };
    if (game_id_raw) body.game_id = parseInt(game_id_raw, 10);
    if (game_name) body.game_name = game_name;

    const res = await locals.api.post("/projects", body);

    if (!res.ok) {
      const err = await res
        .json()
        .catch(() => ({ message: "Failed to create project" }));
      return fail(res.status, { error: err.message });
    }

    const project = await res.json();
    redirect(303, `/projects/${project.id}/players`);
  },
};
```

- [ ] **Step 3: Update `routes/projects/[id]/+layout.server.ts`**

```typescript
import { error } from "@sveltejs/kit";
import type { LayoutServerLoad } from "./$types";
import type { Project } from "$lib/types";

export const load: LayoutServerLoad = async ({ locals, params }) => {
  const res = await locals.api.get(`/projects/${params.id}`);
  if (!res.ok) {
    if (res.status === 404) {
      if (!locals.user) {
        error(404, { message: "private_project" });
      }
      error(404, { message: "not_found" });
    }
    error(res.status, { message: "error" });
  }
  const project: Project = await res.json();
  return { project };
};
```

- [ ] **Step 4: Update `routes/projects/[id]/(editor)/import/+page.server.ts`**

```typescript
import { fail } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";
import type { Job } from "$lib/types";

export const load: PageServerLoad = async ({ locals, params }) => {
  const res = await locals.api.get(`/projects/${params.id}/import`);
  const job: Job | null = res.ok ? await res.json() : null;
  return { job };
};

export const actions: Actions = {
  default: async ({ locals, params, request }) => {
    const data = await request.formData();
    const afterDate = data.get("after_date") as string | null;
    const beforeDate = data.get("before_date") as string | null;
    const body: Record<string, string> = {};
    if (afterDate) body.after_date = afterDate;
    if (beforeDate) body.before_date = beforeDate;
    const res = await locals.api.post(
      `/projects/${params.id}/import`,
      Object.keys(body).length ? body : undefined,
    );
    if (!res.ok) {
      const err = await res
        .json()
        .catch(() => ({ message: "Failed to start import" }));
      return fail(res.status, { error: err.message });
    }
    const job: Job = await res.json();
    return { job };
  },
};
```

- [ ] **Step 5: Update `routes/projects/[id]/(editor)/players/+page.server.ts`**

```typescript
import { fail } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";
import type { Player } from "$lib/types";

export const load: PageServerLoad = async ({ locals, params }) => {
  const res = await locals.api.get(`/projects/${params.id}/players`);
  const players: Player[] = res.ok ? await res.json() : [];
  return { players };
};

export const actions: Actions = {
  addPlayer: async ({ locals, request, params }) => {
    const data = await request.formData();
    const name = (data.get("name") as string)?.trim();
    if (!name) return fail(422, { addError: "Player name is required" });

    const res = await locals.api.post(`/projects/${params.id}/players`, { name });
    if (!res.ok) {
      const err = await res
        .json()
        .catch(() => ({ message: "Failed to add player" }));
      return fail(res.status, { addError: err.message });
    }
  },

  deletePlayer: async ({ locals, request, params }) => {
    const data = await request.formData();
    const pid = data.get("pid") as string;
    const res = await locals.api.delete(`/projects/${params.id}/players/${pid}`);
    if (!res.ok)
      return fail(res.status, { deleteError: "Failed to delete player" });
  },

  renamePlayer: async ({ locals, request, params }) => {
    const data = await request.formData();
    const pid = data.get("pid") as string;
    const name = (data.get("name") as string)?.trim();
    if (!name)
      return fail(422, { renameError: "Name is required", renamePid: pid });

    const res = await locals.api.patch(`/projects/${params.id}/players/${pid}`, {
      name,
    });
    if (!res.ok) {
      const err = await res
        .json()
        .catch(() => ({ message: "Failed to rename player" }));
      return fail(res.status, { renameError: err.message, renamePid: pid });
    }
  },

  linkAccount: async ({ locals, request, params }) => {
    const data = await request.formData();
    const pid = data.get("pid") as string;
    const handle = (data.get("handle") as string)?.trim();
    if (!handle)
      return fail(422, { linkError: "Handle is required", linkPid: pid });

    const res = await locals.api.post(
      `/projects/${params.id}/players/${pid}/accounts`,
      { handle },
    );
    if (!res.ok) {
      const err = await res
        .json()
        .catch(() => ({ message: "Failed to link account" }));
      return fail(res.status, { linkError: err.message, linkPid: pid });
    }
  },

  unlinkAccount: async ({ locals, request, params }) => {
    const data = await request.formData();
    const pid = data.get("pid") as string;
    const aid = data.get("aid") as string;
    const res = await locals.api.delete(
      `/projects/${params.id}/players/${pid}/accounts/${aid}`,
    );
    if (!res.ok)
      return fail(res.status, { deleteError: "Failed to unlink account" });
  },
};
```

- [ ] **Step 6: Update `routes/projects/[id]/(editor)/players/[player_id]/+page.server.ts`**

```typescript
import { error } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";
import type { Player, PlayerStats, TournamentAttendance } from "$lib/types";

export const load: PageServerLoad = async ({ locals, params }) => {
  const [statsRes, tournamentsRes, playersRes] = await Promise.all([
    locals.api.get(`/projects/${params.id}/stats/${params.player_id}`),
    locals.api.get(`/projects/${params.id}/players/${params.player_id}/tournaments`),
    locals.api.get(`/projects/${params.id}/players`),
  ]);

  if (!statsRes.ok) {
    if (statsRes.status === 404) {
      error(404, { message: "not_found" });
    }
    error(statsRes.status, { message: "error" });
  }

  const stats: PlayerStats = await statsRes.json();

  if (!tournamentsRes.ok) {
    error(tournamentsRes.status, "Failed to load tournament history");
  }

  const tournaments: TournamentAttendance[] = await tournamentsRes.json();

  const players: Player[] = playersRes.ok ? await playersRes.json() : [];
  const trackedPlayerIds = new Set(players.map((p) => p.id));

  return { stats, tournaments, trackedPlayerIds, projectId: params.id };
};
```

- [ ] **Step 7: Update `routes/projects/[id]/h2h/+page.server.ts`**

```typescript
import type { PageServerLoad } from "./$types";
import type { HeadToHeadEntry, Player } from "$lib/types";

export const load: PageServerLoad = async ({ locals, params }) => {
  const [h2hRes, playersRes] = await Promise.all([
    locals.api.get(`/projects/${params.id}/head-to-head`),
    locals.api.get(`/projects/${params.id}/players`),
  ]);
  const h2h: HeadToHeadEntry[] = h2hRes.ok ? await h2hRes.json() : [];
  const players: Player[] = playersRes.ok ? await playersRes.json() : [];
  return { h2h, players };
};
```

- [ ] **Step 8: Update `routes/projects/[id]/ranking/+page.server.ts`**

```typescript
import type { PageServerLoad } from "./$types";
import type { Player, PlayerStats } from "$lib/types";

export const load: PageServerLoad = async ({ locals, params }) => {
  const [playersRes, statsRes] = await Promise.all([
    locals.api.get(`/projects/${params.id}/players`),
    locals.api.get(`/projects/${params.id}/stats`),
  ]);
  const players: Player[] = playersRes.ok ? await playersRes.json() : [];
  const stats: PlayerStats[] = statsRes.ok ? await statsRes.json() : [];
  return { players, stats };
};
```

- [ ] **Step 9: Update `routes/projects/[id]/settings/+page.server.ts`**

```typescript
import { fail, redirect, error } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";
import type { ProjectMember, InviteLink } from "$lib/types";

export const load: PageServerLoad = async ({ locals, params, parent }) => {
  const { project } = await parent();
  if (project.user_role !== "owner") {
    error(403, { message: "forbidden" });
  }

  const [membersRes, linksRes] = await Promise.all([
    locals.api.get(`/projects/${params.id}/members`),
    locals.api.get(`/projects/${params.id}/invite-links`),
  ]);

  const members: ProjectMember[] = membersRes.ok ? await membersRes.json() : [];
  const inviteLinks: InviteLink[] = linksRes.ok ? await linksRes.json() : [];

  return { members, inviteLinks };
};

export const actions: Actions = {
  rename: async ({ locals, params, request }) => {
    const data = await request.formData();
    const name = ((data.get("name") as string) ?? "").trim();
    if (!name) return fail(400, { renameError: "Name is required" });
    if ([...name].length > 100)
      return fail(400, { renameError: "Name must be at most 100 characters" });
    const res = await locals.api.patch(`/projects/${params.id}`, { name });
    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: "Rename failed" }));
      return fail(res.status, { renameError: body.message });
    }
    return { project: await res.json() };
  },

  publish: async ({ locals, params, request }) => {
    const data = await request.formData();
    const published = data.get("published") === "true";
    const res = await locals.api.patch(`/projects/${params.id}`, { published });
    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Failed to update" }));
      return fail(res.status, { publishError: body.message });
    }
    return { project: await res.json() };
  },

  addMember: async ({ locals, params, request }) => {
    const data = await request.formData();
    const email = ((data.get("email") as string) ?? "").trim();
    const role = data.get("role") as string;
    if (!email) return fail(400, { memberError: "Email is required" });
    if (!["editor", "viewer"].includes(role))
      return fail(400, { memberError: "Invalid role" });
    const res = await locals.api.post(`/projects/${params.id}/members`, {
      email,
      role,
    });
    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Failed to add member" }));
      return fail(res.status, { memberError: body.message });
    }
    return {};
  },

  removeMember: async ({ locals, params, request }) => {
    const data = await request.formData();
    const userId = data.get("user_id") as string;
    const res = await locals.api.delete(`/projects/${params.id}/members/${userId}`);
    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Failed to remove member" }));
      return fail(res.status, { memberError: body.message });
    }
    return {};
  },

  changeMemberRole: async ({ locals, params, request }) => {
    const data = await request.formData();
    const userId = data.get("user_id") as string;
    const role = data.get("role") as string;
    const res = await locals.api.patch(`/projects/${params.id}/members/${userId}`, {
      role,
    });
    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Failed to update role" }));
      return fail(res.status, { memberError: body.message });
    }
    return {};
  },

  transferOwnership: async ({ locals, params, request }) => {
    const data = await request.formData();
    const userId = data.get("user_id") as string;
    const res = await locals.api.post(
      `/projects/${params.id}/members/transfer-ownership`,
      { user_id: userId },
    );
    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Transfer failed" }));
      return fail(res.status, { memberError: body.message });
    }
    return {};
  },

  createInviteLink: async ({ locals, params, request }) => {
    const data = await request.formData();
    const role = data.get("role") as string;
    const expiresAtRaw = data.get("expires_at") as string | null;
    const expires_at = expiresAtRaw
      ? new Date(expiresAtRaw).toISOString()
      : undefined;
    if (!["editor", "viewer"].includes(role))
      return fail(400, { linkError: "Invalid role" });
    const res = await locals.api.post(`/projects/${params.id}/invite-links`, {
      role,
      expires_at,
    });
    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Failed to create link" }));
      return fail(res.status, { linkError: body.message });
    }
    return { newLink: await res.json() };
  },

  revokeInviteLink: async ({ locals, params, request }) => {
    const data = await request.formData();
    const linkId = data.get("link_id") as string;
    const res = await locals.api.delete(
      `/projects/${params.id}/invite-links/${linkId}`,
    );
    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Failed to revoke link" }));
      return fail(res.status, { linkError: body.message });
    }
    return {};
  },

  delete: async ({ locals, params }) => {
    const res = await locals.api.delete(`/projects/${params.id}`);
    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: "Delete failed" }));
      return fail(res.status, { deleteError: body.message });
    }
    redirect(303, "/projects");
  },
};
```

- [ ] **Step 10: Update `routes/projects/[id]/stats/+page.server.ts`**

```typescript
import type { PageServerLoad } from "./$types";
import type { PlayerStats } from "$lib/types";

export const load: PageServerLoad = async ({ locals, params }) => {
  const res = await locals.api.get(`/projects/${params.id}/stats`);
  const stats: PlayerStats[] = res.ok ? await res.json() : [];
  return { stats };
};
```

- [ ] **Step 11: Update `routes/projects/[id]/tournaments/+page.server.ts`**

```typescript
import type { PageServerLoad } from "./$types";
import type { Tournament } from "$lib/types";

export const load: PageServerLoad = async ({ locals, params }) => {
  const res = await locals.api.get(`/projects/${params.id}/tournaments`);
  const tournaments: Tournament[] = res.ok ? await res.json() : [];
  return { tournaments };
};
```

- [ ] **Step 12: Update `routes/invite/[token]/+page.server.ts`**

```typescript
import { redirect } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";

export const load: PageServerLoad = async ({ params }) => {
  return { token: params.token };
};

export const actions: Actions = {
  accept: async ({ locals, params }) => {
    const res = await locals.api.post(`/invite/${params.token}/accept`);
    if (!res.ok) {
      if (res.status === 401) {
        redirect(303, `/login?next=/invite/${params.token}`);
      }
      const body = await res
        .json()
        .catch(() => ({ message: "Failed to accept invite" }));
      return { error: body.message };
    }
    const data = await res.json();
    redirect(303, `/projects/${data.project_id}`);
  },
};
```

- [ ] **Step 13: Update `routes/account/+page.server.ts`**

```typescript
import { fail, redirect } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";

export const load: PageServerLoad = ({ locals }) => {
  if (!locals.user) redirect(303, "/login");
  return { user: locals.user };
};

export const actions: Actions = {
  updateProfile: async ({ locals, request }) => {
    if (!locals.user) return fail(401, { error: "Unauthorized" });
    const data = await request.formData();
    const display_name = data.get("display_name") as string | null;
    const email = data.get("email") as string | null;

    const body: Record<string, string> = {};
    if (display_name) body.display_name = display_name;
    if (email) body.email = email;

    if (Object.keys(body).length === 0) {
      return fail(422, {
        profileError: "Provide at least one field to update.",
      });
    }

    const res = await locals.api.patch("/account/profile", body);

    if (!res.ok) {
      const json = await res.json().catch(() => ({ message: "Update failed" }));
      return fail(res.status, {
        profileError: json.message ?? "Update failed",
      });
    }

    return { profileSuccess: true };
  },

  updatePassword: async ({ locals, request }) => {
    if (!locals.user) return fail(401, { error: "Unauthorized" });
    const data = await request.formData();
    const current_password = data.get("current_password") as string;
    const new_password = data.get("new_password") as string;
    const confirm_password = data.get("confirm_password") as string;

    if (new_password !== confirm_password) {
      return fail(400, { passwordError: "New passwords do not match." });
    }

    const res = await locals.api.patch("/account/password", {
      current_password,
      new_password,
    });

    if (!res.ok) {
      const json = await res
        .json()
        .catch(() => ({ message: "Password change failed" }));
      return fail(res.status, {
        passwordError: json.message ?? "Password change failed",
      });
    }

    return { passwordSuccess: true };
  },

  setStartggKey: async ({ locals, request }) => {
    if (!locals.user) return fail(401, { error: "Unauthorized" });
    const data = await request.formData();
    const api_key = data.get("api_key") as string | null;
    if (!api_key?.trim()) {
      return fail(422, { startggKeyError: "API key must not be empty." });
    }

    const res = await locals.api.put("/account/startgg-key", {
      api_key: api_key.trim(),
    });

    if (!res.ok) {
      const json = await res
        .json()
        .catch(() => ({ message: "Failed to save key" }));
      return fail(res.status, {
        startggKeyError: json.message ?? "Failed to save key",
      });
    }

    return { startggKeySuccess: true };
  },

  removeStartggKey: async ({ locals }) => {
    if (!locals.user) return fail(401, { error: "Unauthorized" });

    const res = await locals.api.delete("/account/startgg-key");

    if (!res.ok) {
      return fail(res.status, { startggKeyError: "Failed to remove key." });
    }

    return { startggKeyRemoved: true };
  },

  deleteAccount: async ({ locals }) => {
    if (!locals.user) return fail(401, { error: "Unauthorized" });

    const res = await locals.api.delete("/account");

    if (!res.ok) {
      const json = await res.json().catch(() => ({ message: "Delete failed" }));
      return fail(res.status, { deleteError: json.message ?? "Delete failed" });
    }

    redirect(303, "/login");
  },
};
```

- [ ] **Step 14: Run unit tests**

```bash
cd web && npm run test:unit
```
Expected: all tests pass.

- [ ] **Step 15: Commit**

```bash
git add web/src/routes
git commit -m "refactor: replace makeApi+cookies pattern with locals.api in all server routes"
```

---

### Task 6: Add `SessionResponse` to Rust auth handlers

**Files:**
- Modify: `backend/crates/api/src/routes/auth.rs`

The login and register handlers currently return `Json(UserResponse::from(user))`. We add a `SessionResponse` wrapper that also carries `session_id`, allowing the SvelteKit layer to read it from the body instead of parsing the `Set-Cookie` header.

- [ ] **Step 1: Add `SessionResponse` struct after `UserResponse` in `auth.rs`**

After line 98 (the closing `}` of `impl From<User> for UserResponse`), add:

```rust
#[derive(Serialize)]
pub struct SessionResponse {
    pub session_id: Uuid,
    pub user: UserResponse,
}
```

- [ ] **Step 2: Update the `login` handler return type**

Find the line:
```rust
    Ok((jar, Json(UserResponse::from(user))))
```
Replace with:
```rust
    Ok((jar, Json(SessionResponse { session_id, user: UserResponse::from(user) })))
```

- [ ] **Step 3: Update the `register` handler return type**

Find the line:
```rust
    Ok((StatusCode::CREATED, jar, Json(UserResponse::from(user))))
```
Replace with:
```rust
    Ok((StatusCode::CREATED, jar, Json(SessionResponse { session_id, user: UserResponse::from(user) })))
```

- [ ] **Step 4: Run backend tests**

```bash
bash backend/test.sh
```
Expected: all tests pass. (The response body shape changed, but existing tests that check auth flows check status codes and cookie presence, not the body shape of login/register.)

- [ ] **Step 5: Commit**

```bash
git add backend/crates/api/src/routes/auth.rs
git commit -m "feat: include session_id in login and register response bodies"
```

---

### Task 7: Update SvelteKit login and register to read `session_id` from body

**Files:**
- Modify: `web/src/routes/login/+page.server.ts`
- Modify: `web/src/routes/register/+page.server.ts`

Both files currently parse `Set-Cookie` with a regex. Replace with reading `body.session_id`.

- [ ] **Step 1: Replace `routes/login/+page.server.ts`**

```typescript
import { fail, redirect } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";
import { env } from "$env/dynamic/private";

export const load: PageServerLoad = ({ locals, url }) => {
  if (locals.user) redirect(303, "/projects");
  const redirectTo = url.searchParams.get("redirect") ?? "/projects";
  return { redirectTo };
};

export const actions: Actions = {
  default: async ({ locals, request, cookies }) => {
    const data = await request.formData();
    const email = data.get("email") as string;
    const password = data.get("password") as string;
    const redirectTo = (data.get("redirect") as string) ?? "/projects";

    const res = await locals.api.post("/auth/login", { email, password });

    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: "Login failed" }));
      return fail(res.status, { error: body.message ?? "Login failed" });
    }

    const body = await res.json();
    cookies.set("session_id", body.session_id, {
      path: "/",
      httpOnly: true,
      sameSite: "strict",
      maxAge: 60 * 60 * 24 * 30,
      ...(env.COOKIE_DOMAIN ? { domain: env.COOKIE_DOMAIN } : {}),
    });

    const safe =
      redirectTo.startsWith("/") && !redirectTo.startsWith("//")
        ? redirectTo
        : "/projects";
    redirect(303, safe);
  },
};
```

- [ ] **Step 2: Replace `routes/register/+page.server.ts`**

```typescript
import { fail, redirect } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";
import { env } from "$env/dynamic/private";

export const load: PageServerLoad = ({ locals }) => {
  if (locals.user) redirect(303, "/projects");
};

export const actions: Actions = {
  default: async ({ locals, request, cookies }) => {
    const data = await request.formData();
    const email = data.get("email") as string;
    const display_name = data.get("display_name") as string;
    const password = data.get("password") as string;
    const confirmPassword = data.get("confirm_password") as string;

    if (password !== confirmPassword) {
      return fail(400, { error: "Passwords do not match" });
    }

    const res = await locals.api.post("/auth/register", {
      email,
      display_name,
      password,
    });

    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Registration failed" }));
      return fail(res.status, { error: body.message ?? "Registration failed" });
    }

    const body = await res.json();
    cookies.set("session_id", body.session_id, {
      path: "/",
      httpOnly: true,
      sameSite: "strict",
      maxAge: 60 * 60 * 24 * 30,
      ...(env.COOKIE_DOMAIN ? { domain: env.COOKIE_DOMAIN } : {}),
    });

    redirect(303, "/projects");
  },
};
```

- [ ] **Step 3: Run the full test suite**

```bash
bash test.sh
```
Expected: all backend and frontend tests pass.

- [ ] **Step 4: Commit**

```bash
git add web/src/routes/login/+page.server.ts web/src/routes/register/+page.server.ts
git commit -m "refactor: read session_id from login/register response body instead of Set-Cookie header"
```

---

### Task 8: Fix logout cookie deletion

**Files:**
- Modify: `web/src/routes/logout/+page.server.ts`

The logout action currently deletes the cookie without a `domain` option, which doesn't match the cookie set with `domain=COOKIE_DOMAIN` in production. Also switch to `locals.api` for the API call.

- [ ] **Step 1: Replace `routes/logout/+page.server.ts`**

```typescript
import type { Actions, PageServerLoad } from "./$types";
import { redirect } from "@sveltejs/kit";
import { env } from "$env/dynamic/private";

export const load: PageServerLoad = () => {
  redirect(303, "/");
};

export const actions: Actions = {
  default: async ({ locals, cookies }) => {
    await locals.api.post("/auth/logout").catch(() => {});
    cookies.delete("session_id", {
      path: "/",
      ...(env.COOKIE_DOMAIN ? { domain: env.COOKIE_DOMAIN } : {}),
    });
    redirect(303, "/login");
  },
};
```

- [ ] **Step 2: Run unit tests**

```bash
cd web && npm run test:unit
```
Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add web/src/routes/logout/+page.server.ts
git commit -m "fix: use locals.api for logout call and add domain to cookie deletion"
```

---

### Task 9: Update client `.svelte` files

**Files:**
- Modify: `web/src/routes/projects/new/+page.svelte`
- Modify: `web/src/routes/projects/[id]/(editor)/import/+page.svelte`
- Modify: `web/src/routes/projects/[id]/h2h/+page.svelte`
- Modify: `web/src/routes/projects/[id]/ranking/+page.svelte`
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`
- Modify: `web/src/lib/components/HandleTab.svelte`
- Modify: `web/src/lib/components/NameTab.svelte`
- Modify: `web/src/lib/components/TournamentTab.svelte`

In every file: remove `import { env } from "$env/dynamic/public"` and replace `makeApi(fetch, env.PUBLIC_API_URL)` with `makeApi(fetch)`. In `projects/new/+page.svelte`, replace the raw `fetch(...)` call with `makeApi(fetch).get(...)`.

- [ ] **Step 1: Update `routes/projects/new/+page.svelte`**

In the `<script>` block, remove:
```typescript
import { env } from "$env/dynamic/public";
```
Replace the `onCommandInput` function body that does the raw fetch:
```typescript
// Before:
const res = await fetch(
  `${env.PUBLIC_API_URL}/games?q=${encodeURIComponent(value)}`,
  { credentials: "include" },
);
// After:
const res = await makeApi(fetch).get(`/games?q=${encodeURIComponent(value)}`);
```
Add the missing import at the top of the script:
```typescript
import { makeApi } from "$lib/api";
```

- [ ] **Step 2: Update `routes/projects/[id]/(editor)/import/+page.svelte`**

Remove `import { env } from "$env/dynamic/public"`.

Change line 62 from:
```typescript
const api = makeApi(fetch, env.PUBLIC_API_URL);
```
To:
```typescript
const api = makeApi(fetch);
```

- [ ] **Step 3: Update `routes/projects/[id]/h2h/+page.svelte`**

Remove `import { env } from "$env/dynamic/public"`.

Change line 42 from:
```typescript
const api = makeApi(fetch, env.PUBLIC_API_URL);
```
To:
```typescript
const api = makeApi(fetch);
```

- [ ] **Step 4: Update `routes/projects/[id]/ranking/+page.svelte`**

Remove `import { env } from "$env/dynamic/public"`.

Change line 90 from:
```typescript
const api = makeApi(fetch, env.PUBLIC_API_URL);
```
To:
```typescript
const api = makeApi(fetch);
```

- [ ] **Step 5: Update `routes/projects/[id]/tournaments/+page.svelte`**

Remove `import { env } from "$env/dynamic/public"`.

Change line 137 from:
```typescript
const api = makeApi(fetch, env.PUBLIC_API_URL);
```
To:
```typescript
const api = makeApi(fetch);
```

- [ ] **Step 6: Update `src/lib/components/HandleTab.svelte`**

Remove `import { env } from "$env/dynamic/public"`.

Change line 27 from:
```typescript
const api = makeApi(fetch, env.PUBLIC_API_URL);
```
To:
```typescript
const api = makeApi(fetch);
```

- [ ] **Step 7: Update `src/lib/components/NameTab.svelte`**

Remove `import { env } from "$env/dynamic/public"`.

Change line 20 from:
```typescript
const api = makeApi(fetch, env.PUBLIC_API_URL);
```
To:
```typescript
const api = makeApi(fetch);
```

- [ ] **Step 8: Update `src/lib/components/TournamentTab.svelte`**

Remove `import { env } from "$env/dynamic/public"`.

Change lines 137 and 164 from:
```typescript
const api = makeApi(fetch, env.PUBLIC_API_URL);
```
To:
```typescript
const api = makeApi(fetch);
```

- [ ] **Step 9: Run unit tests**

```bash
cd web && npm run test:unit
```
Expected: all tests pass.

- [ ] **Step 10: Commit**

```bash
git add web/src/routes/projects/new/+page.svelte \
        web/src/routes/projects/[id]/(editor)/import/+page.svelte \
        web/src/routes/projects/[id]/h2h/+page.svelte \
        web/src/routes/projects/[id]/ranking/+page.svelte \
        web/src/routes/projects/[id]/tournaments/+page.svelte \
        web/src/lib/components/HandleTab.svelte \
        web/src/lib/components/NameTab.svelte \
        web/src/lib/components/TournamentTab.svelte
git commit -m "refactor: remove PUBLIC_API_URL from svelte files, use makeApi(fetch)"
```

---

### Task 10: Add `.env.example` and update `CLAUDE.md`

**Files:**
- Create: `web/.env.example`
- Modify: `CLAUDE.md` (root)

- [ ] **Step 1: Create `web/.env.example`**

```bash
# web/.env.example
PUBLIC_API_URL=http://localhost:8080
INTERNAL_API_URL=http://localhost:8080

# Required in production: set to the root domain so the session cookie is
# accessible to the API subdomain (e.g. rankingforge.com for api.rankingforge.com).
# Leave unset for local development.
# COOKIE_DOMAIN=
```

- [ ] **Step 2: Add `COOKIE_DOMAIN` to the env var table in `CLAUDE.md`**

Find the environment variables table in `CLAUDE.md`. Add a row:

```markdown
| `COOKIE_DOMAIN` | web (SvelteKit) | Production only. Set to root domain (e.g. `rankingforge.com`) so the session cookie is sent by the browser to the API subdomain. Leave unset locally. |
```

- [ ] **Step 3: Commit**

```bash
git add web/.env.example CLAUDE.md
git commit -m "docs: add COOKIE_DOMAIN env var documentation and .env.example"
```

---

### Task 11: Run full test suite and smoke test

- [ ] **Step 1: Run full test suite from root**

```bash
bash test.sh
```
Expected: `PASS` for all sections — backend unit, backend e2e, frontend unit, frontend e2e.

- [ ] **Step 2: Start the dev stack and smoke test manually**

```bash
# In one terminal
cd backend && cargo run --bin api

# In another terminal
cd backend && cargo run --bin worker

# In another terminal
cd web && npm run dev
```

Visit `http://localhost:5173` and verify:
1. Register a new account → redirected to `/projects`
2. Log out → redirected to `/login`
3. Log back in → redirected to `/projects`
4. Create a project → redirected to players page
5. Navigate to Import → start an import, verify the progress polling updates live
6. Navigate to Rankings → drag a player, click Save, verify no console errors
7. Navigate to H2H → click a cell, verify set list loads

- [ ] **Step 3: Confirm no `PUBLIC_API_URL` references remain in `.svelte` files**

```bash
grep -r "PUBLIC_API_URL" web/src --include="*.svelte"
```
Expected: no output.

- [ ] **Step 4: Confirm no `makeApi` calls with arguments remain in `.svelte` files**

```bash
grep -r "makeApi(fetch," web/src --include="*.svelte"
```
Expected: no output.

- [ ] **Step 5: Confirm no `cookies.get("session_id")` calls remain in server files**

```bash
grep -r 'cookies.get("session_id")' web/src
```
Expected: no output.
