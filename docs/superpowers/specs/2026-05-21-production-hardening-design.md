# Production Hardening Design

**Date:** 2026-05-21
**Scope:** Five targeted fixes to make the production deployment correct, complete, and self-documenting before going live.

---

## Overview

Five independent changes grouped into one implementation plan:

1. **COOKIE_DOMAIN in compose** — required env var missing from `docker-compose.prod.yml`
2. **Real domain defaults** — placeholder `rankingforge.example.com` replaced with real domains
3. **Session cleanup** — expired sessions accumulate forever; worker cleans them up hourly
4. **Import rate limiting** — import trigger endpoint has no rate limit; add one
5. **Caddy TLS** — no reverse proxy in compose; add Caddy with automatic Let's Encrypt

---

## Fix 1: COOKIE_DOMAIN in `docker-compose.prod.yml`

Add `COOKIE_DOMAIN: ${COOKIE_DOMAIN}` to the `web:` service environment block. No default — omitting it causes a silent auth failure (session cookie not sent to `api.rankingforge.com`) so the operator must set it explicitly.

---

## Fix 2: Replace placeholder domain defaults

In `docker-compose.prod.yml`, update three `${VAR:-rankingforge.example.com}` defaults:

| Variable | New default |
|---|---|
| `CORS_ORIGIN` (api) | `https://rankingforge.com` |
| `ORIGIN` (web) | `https://rankingforge.com` |
| `PUBLIC_API_URL` (web) | `https://api.rankingforge.com` |

In `DESIGN.md`, update the Infrastructure → URLs table:

| Role | Public URL |
|---|---|
| Frontend | `https://rankingforge.com` |
| API | `https://api.rankingforge.com` |

Also update the paragraph below it to reference the real domains.

---

## Fix 3: Session cleanup in worker

**File:** `backend/crates/worker/src/main.rs`

Add a `tokio::time::interval` set to 1 hour before the main loop. Place a new arm in the existing `tokio::select!` to handle each tick:

```rust
let mut cleanup_interval = tokio::time::interval(Duration::from_secs(3600));

// inside tokio::select!:
_ = cleanup_interval.tick() => {
    if let Err(e) = sqlx::query!("DELETE FROM sessions WHERE expires_at < NOW()")
        .execute(&pool)
        .await
    {
        tracing::error!(%e, "failed to clean up expired sessions");
    }
}
```

`tokio::time::interval` fires immediately on the first tick, so this also handles the startup case with no separate query needed. Errors are logged but do not crash the worker.

No migration required — the sessions table is small enough for a sequential scan.

---

## Fix 4: Rate limiting on import endpoint

**Approach:** Extract the shared IP extractor, apply a governor to POST /import only (not GET, which is status polling).

### New file: `backend/crates/api/src/extractors.rs`

Move `ClientIpExtractor` (currently defined inline in `auth.rs`) into this module. It extracts the client IP from `X-Forwarded-For` (first value), falling back to `ConnectInfo`, then `127.0.0.1` for tests.

`auth.rs` imports it from `crate::extractors::ClientIpExtractor` instead of defining it locally.

### `backend/crates/api/src/lib.rs`

Add `pub mod extractors;` to expose the module.

### `backend/crates/api/src/routes/import.rs`

Add a public `rate_limited_post_router()` function that wraps only `POST /{id}/import` with a `GovernorLayer`:

- **Rate:** 1 request per 20 seconds per IP
- **Burst:** 3

This allows brief retry bursts while preventing import hammering.

```rust
pub fn rate_limited_post_router() -> Router<AppState> {
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(ClientIpExtractor)
            .per_second(20)
            .burst_size(3)
            .finish()
            .expect("invalid rate-limit config"),
    );
    Router::new()
        .route("/{id}/import", post(start_import))
        .layer(GovernorLayer::new(governor_conf))
}
```

### `backend/crates/api/src/routes/projects.rs`

Split the current combined route:
```rust
// before:
.route("/{id}/import", post(import::start_import).get(import::get_import_status))

// after:
.route("/{id}/import", get(import::get_import_status))
.merge(import::rate_limited_post_router())
```

`Router::merge()` correctly combines the GET (no rate limit) and POST (rate limited) handlers on the same path without conflict.

---

## Fix 5: Caddy reverse proxy in `docker-compose.prod.yml`

### New file: `Caddyfile` (repo root)

```
rankingforge.com {
    reverse_proxy web:3000
}

api.rankingforge.com {
    reverse_proxy api:3000
}
```

Caddy handles TLS certificate acquisition and renewal via Let's Encrypt automatically.

### `docker-compose.prod.yml` changes

**Add `caddy` service:**

```yaml
caddy:
  image: caddy:2-alpine
  ports:
    - "80:80"
    - "443:443"
  volumes:
    - ./Caddyfile:/etc/caddy/Caddyfile:ro
    - caddy_data:/data
  depends_on:
    - api
    - web
  restart: unless-stopped
```

**Add `caddy_data` volume** to the `volumes:` block (stores Let's Encrypt certs across restarts).

**Remove host port bindings** from `api` and `web`:
- Remove `ports: ["127.0.0.1:3000:3000"]` from `api`
- Remove `ports: ["127.0.0.1:5173:3000"]` from `web`

Caddy reaches both services over Docker's internal network. Direct host exposure is unnecessary.

### Deployment prerequisites

The host machine must have:
- Ports 80 and 443 open
- DNS for `rankingforge.com` and `api.rankingforge.com` pointing at the host

These are operational concerns outside this codebase.

---

## What is not changing

- Session expiry duration (30 days) — unchanged
- Auth rate limits (1/sec, burst 5) — unchanged
- Cookie attributes (`HttpOnly`, `SameSite=strict`, `secure` via SvelteKit `ORIGIN`) — unchanged
- No new migrations
- No new Rust dependencies (tower-governor already present)
