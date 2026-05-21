# Production Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Five targeted fixes to make the production deployment correct and complete before going live: COOKIE_DOMAIN in compose, real domain defaults, Caddy TLS, hourly session cleanup in the worker, and import endpoint rate limiting.

**Architecture:** Config/docs changes are self-contained. Session cleanup is added as a `tokio::time::interval` arm in the existing worker `select!` loop. Rate limiting extracts the shared `ClientIpExtractor` into its own module and applies a `GovernorLayer` only to `POST /{id}/import` via `Router::merge`. Caddy is added as a compose service with a `Caddyfile` at the repo root.

**Tech Stack:** Rust (Axum, tower-governor, sqlx, tokio), Docker Compose, Caddy 2

---

## File Map

| Action | Path | Purpose |
|---|---|---|
| Modify | `docker-compose.prod.yml` | COOKIE_DOMAIN, real domains, Caddy service, remove host port bindings |
| Modify | `DESIGN.md` | Update Infrastructure URLs table |
| Create | `Caddyfile` | Caddy reverse proxy config for both domains |
| Modify | `backend/crates/worker/src/main.rs` | Hourly session cleanup via interval |
| Create | `backend/crates/api/src/extractors.rs` | Shared `ClientIpExtractor` (moved from auth.rs) |
| Modify | `backend/crates/api/src/lib.rs` | Expose `extractors` module |
| Modify | `backend/crates/api/src/routes/auth.rs` | Import `ClientIpExtractor` from new module |
| Modify | `backend/crates/api/src/routes/import.rs` | Add `rate_limited_post_router()` + tests |
| Modify | `backend/crates/api/src/routes/projects.rs` | Split import route: GET stays, POST via merge |

---

## Task 1: Config — COOKIE_DOMAIN and real domain defaults

**Files:**
- Modify: `docker-compose.prod.yml`
- Modify: `DESIGN.md`

No automated tests for config changes — verify by inspection.

- [ ] **Step 1: Add COOKIE_DOMAIN to the web service in `docker-compose.prod.yml`**

  In the `web:` → `environment:` block, add after `INTERNAL_API_URL`:

  ```yaml
        COOKIE_DOMAIN: ${COOKIE_DOMAIN}
  ```

  No default. If omitted from `.env`, Docker Compose will warn and the cookie won't be cross-subdomain scoped.

- [ ] **Step 2: Update the three placeholder domain defaults in `docker-compose.prod.yml`**

  ```yaml
  # api service — change:
  CORS_ORIGIN: ${CORS_ORIGIN:-https://rankingforge.example.com}
  # to:
  CORS_ORIGIN: ${CORS_ORIGIN:-https://rankingforge.com}

  # web service — change:
  ORIGIN: ${ORIGIN:-https://rankingforge.example.com}
  PUBLIC_API_URL: ${PUBLIC_API_URL:-https://rankingforge.example.com}
  # to:
  ORIGIN: ${ORIGIN:-https://rankingforge.com}
  PUBLIC_API_URL: ${PUBLIC_API_URL:-https://api.rankingforge.com}
  ```

- [ ] **Step 3: Update DESIGN.md Infrastructure URLs table**

  Find the table under `### URLs` and replace:

  ```markdown
  | Frontend | `https://rankingforge.example.com` |
  | API | `https://api.rankingforge.example.com` |
  ```

  with:

  ```markdown
  | Frontend | `https://rankingforge.com` |
  | API | `https://api.rankingforge.com` |
  ```

  Also update the paragraph below the table: replace both occurrences of `rankingforge.example.com` / `api.rankingforge.example.com` with `rankingforge.com` / `api.rankingforge.com`.

- [ ] **Step 4: Commit**

  ```bash
  git add docker-compose.prod.yml DESIGN.md
  git commit -m "fix(deploy): add COOKIE_DOMAIN and update domain defaults to rankingforge.com"
  ```

---

## Task 2: Caddy reverse proxy

**Files:**
- Create: `Caddyfile`
- Modify: `docker-compose.prod.yml`

No automated tests — Caddy integration is verified at deploy time (DNS + port 80/443 required).

- [ ] **Step 1: Create `Caddyfile` at the repo root**

  ```
  rankingforge.com {
      reverse_proxy web:3000
  }

  api.rankingforge.com {
      reverse_proxy api:3000
  }
  ```

  Caddy auto-provisions Let's Encrypt TLS for both domains. Both `web` and `api` refer to Docker service names on the internal network.

- [ ] **Step 2: Add the `caddy` service to `docker-compose.prod.yml`**

  Add after the `worker:` service block:

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

- [ ] **Step 3: Remove host port bindings from `api` and `web`**

  Caddy routes to these services over Docker's internal network; they do not need direct host exposure.

  In the `api:` service, remove:
  ```yaml
      ports:
        - "127.0.0.1:3000:3000"
  ```

  In the `web:` service, remove:
  ```yaml
      ports:
        - "127.0.0.1:5173:3000"
  ```

- [ ] **Step 4: Add `caddy_data` volume to the `volumes:` block**

  ```yaml
  volumes:
    postgres_data:
    caddy_data:
  ```

- [ ] **Step 5: Commit**

  ```bash
  git add Caddyfile docker-compose.prod.yml
  git commit -m "feat(deploy): add Caddy reverse proxy with automatic TLS"
  ```

---

## Task 3: Hourly session cleanup in the worker

**Files:**
- Modify: `backend/crates/worker/src/main.rs`
- Test: `backend/crates/api/src/routes/auth.rs` (tests section)

- [ ] **Step 1: Write a failing test for the cleanup query**

  In `backend/crates/api/src/routes/auth.rs`, add to the existing `#[cfg(test)]` block:

  ```rust
  #[sqlx::test(migrations = "../../migrations")]
  async fn test_cleanup_deletes_expired_sessions_not_active(pool: PgPool) {
      let app = make_app(pool.clone());

      // Register creates one active session
      let _cookie = register(&app, "cleanup_user").await;

      // fetch_one returns Result<Option<Uuid>>; id is NOT NULL so inner Option is Some
      let user_id = sqlx::query_scalar!(
          "SELECT id FROM users WHERE email = 'cleanup_user@test.com'"
      )
      .fetch_one(&pool)
      .await
      .unwrap()
      .unwrap();

      // Insert an already-expired session
      sqlx::query!(
          "INSERT INTO sessions (id, user_id, expires_at)
           VALUES (gen_random_uuid(), $1, NOW() - INTERVAL '1 hour')",
          user_id
      )
      .execute(&pool)
      .await
      .unwrap();

      // Two sessions now: one active, one expired
      let before = sqlx::query_scalar!("SELECT COUNT(*) FROM sessions")
          .fetch_one(&pool)
          .await
          .unwrap();
      assert_eq!(before, Some(2));

      // Run the cleanup
      let deleted = sqlx::query!("DELETE FROM sessions WHERE expires_at < NOW()")
          .execute(&pool)
          .await
          .unwrap();
      assert_eq!(deleted.rows_affected(), 1);

      // Only the active session remains
      let after = sqlx::query_scalar!("SELECT COUNT(*) FROM sessions")
          .fetch_one(&pool)
          .await
          .unwrap();
      assert_eq!(after, Some(1));
  }
  ```

- [ ] **Step 2: Run the test to verify it passes (it tests the SQL, not the interval)**

  ```bash
  cd backend && DATABASE_URL=postgres://... cargo test -p api -- test_cleanup_deletes_expired_sessions_not_active
  ```

  Expected: PASS (the SQL is correct — this test documents the expected behavior and validates the query compiles).

- [ ] **Step 3: Add the hourly cleanup interval to the worker**

  In `backend/crates/worker/src/main.rs`, add `cleanup_interval` before the main loop. The existing `use std::time::Duration;` import already covers the duration.

  After the `PgListener` setup and before the `in_flight` vec declaration, add:

  ```rust
  let mut cleanup_interval = tokio::time::interval(Duration::from_secs(3600));
  ```

  Inside the `tokio::select!` block (after the existing `_ = tokio::time::sleep(...)` arm), add:

  ```rust
              _ = cleanup_interval.tick() => {
                  if let Err(e) = sqlx::query!("DELETE FROM sessions WHERE expires_at < NOW()")
                      .execute(&pool)
                      .await
                  {
                      tracing::error!(%e, "failed to clean up expired sessions");
                  }
              }
  ```

  `tokio::time::interval` delivers its first tick immediately, so this also handles cleanup at startup without a separate query.

- [ ] **Step 4: Verify the worker builds**

  ```bash
  cd backend && cargo build --bin worker
  ```

  Expected: compiles cleanly. No new dependencies needed.

- [ ] **Step 5: Run the sqlx prepare script to update the offline cache**

  The new `sqlx::query!` in the worker must be cached:

  ```bash
  bash backend/prepare-sqlx.sh
  ```

- [ ] **Step 6: Commit**

  ```bash
  git add backend/crates/worker/src/main.rs \
          backend/crates/api/src/routes/auth.rs \
          backend/.sqlx/
  git commit -m "feat(worker): clean up expired sessions hourly"
  ```

---

## Task 4: Rate limiting on POST /import

**Files:**
- Create: `backend/crates/api/src/extractors.rs`
- Modify: `backend/crates/api/src/lib.rs`
- Modify: `backend/crates/api/src/routes/auth.rs`
- Modify: `backend/crates/api/src/routes/import.rs`
- Modify: `backend/crates/api/src/routes/projects.rs`

### Step group A — Extract `ClientIpExtractor`

- [ ] **Step 1: Create `backend/crates/api/src/extractors.rs`**

  ```rust
  use tower_governor::{GovernorError, key_extractor::KeyExtractor};

  // In production, the reverse proxy sets X-Forwarded-For — that's the primary extraction path.
  // ConnectInfo is a fallback for direct connections (not used with the current axum::serve setup).
  // LOCALHOST fallback is intentional for tests: each test creates a fresh GovernorLayer with
  // an independent bucket, so no rate-limit state leaks between tests.
  #[derive(Clone)]
  pub struct ClientIpExtractor;

  impl KeyExtractor for ClientIpExtractor {
      type Key = std::net::IpAddr;

      fn extract<T>(
          &self,
          req: &axum::http::Request<T>,
      ) -> std::result::Result<Self::Key, GovernorError> {
          let forwarded = req
              .headers()
              .get("x-forwarded-for")
              .and_then(|v| v.to_str().ok())
              .and_then(|s| s.split(',').next())
              .and_then(|s| s.trim().parse::<std::net::IpAddr>().ok());

          if let Some(ip) = forwarded {
              return Ok(ip);
          }

          if let Some(info) = req
              .extensions()
              .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
          {
              return Ok(info.0.ip());
          }

          Ok(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
      }
  }
  ```

- [ ] **Step 2: Expose `extractors` in `backend/crates/api/src/lib.rs`**

  Add to the module list:

  ```rust
  pub mod config;
  pub mod error;
  pub mod extractors;
  pub mod routes;
  pub mod state;
  ```

- [ ] **Step 3: Update `backend/crates/api/src/routes/auth.rs` to use the shared extractor**

  Remove the `ClientIpExtractor` struct definition and its `impl KeyExtractor` block entirely (lines 31–61 in the current file).

  Remove `use tower_governor::key_extractor::KeyExtractor;` from the imports.

  Add to the `use crate::` block:

  ```rust
  use crate::{
      error::{AppError, Result},
      extractors::ClientIpExtractor,
      state::AppState,
  };
  ```

- [ ] **Step 4: Build to verify no regressions**

  ```bash
  cd backend && cargo build -p api
  ```

  Expected: compiles cleanly.

- [ ] **Step 5: Run existing auth tests**

  ```bash
  cd backend && DATABASE_URL=postgres://... cargo test -p api -- routes::auth
  ```

  Expected: all pass. Rate limiting behaviour is unchanged.

### Step group B — Add rate-limited import router

- [ ] **Step 6: Write a failing test for the import rate limit**

  In `backend/crates/api/src/routes/import.rs`, add at the bottom:

  ```rust
  #[cfg(test)]
  mod tests {
      use crate::{routes, state::AppState};
      use axum::{
          Router,
          body::Body,
          http::{Request, StatusCode},
      };
      use http_body_util::BodyExt;
      use serde_json::{Value, json};
      use sqlx::PgPool;
      use tower::ServiceExt;

      fn make_app(pool: PgPool) -> Router {
          let state = AppState {
              db: pool,
              cors_origin: "http://localhost".into(),
              startgg_base_url: "http://localhost:1".into(),
          };
          routes::router().with_state(state)
      }

      async fn register(app: &Router, name: &str) -> String {
          let resp = app
              .clone()
              .oneshot(
                  Request::builder()
                      .method("POST")
                      .uri("/auth/register")
                      .header("content-type", "application/json")
                      .body(Body::from(
                          serde_json::to_vec(&json!({
                              "email": format!("{name}@test.com"),
                              "display_name": name,
                              "password": "password123"
                          }))
                          .unwrap(),
                      ))
                      .unwrap(),
              )
              .await
              .unwrap();
          assert_eq!(resp.status(), StatusCode::CREATED);
          let bytes = resp.into_body().collect().await.unwrap().to_bytes();
          let body: Value = serde_json::from_slice(&bytes).unwrap();
          format!("session_id={}", body["session_id"].as_str().unwrap())
      }

      async fn create_project(app: &Router, cookie: &str) -> String {
          let resp = app
              .clone()
              .oneshot(
                  Request::builder()
                      .method("POST")
                      .uri("/projects")
                      .header("content-type", "application/json")
                      .header("cookie", cookie)
                      .body(Body::from(
                          serde_json::to_vec(&json!({"name": "Test Project"})).unwrap(),
                      ))
                      .unwrap(),
              )
              .await
              .unwrap();
          assert_eq!(resp.status(), StatusCode::CREATED);
          let bytes = resp.into_body().collect().await.unwrap().to_bytes();
          let v: Value = serde_json::from_slice(&bytes).unwrap();
          v["id"].as_str().unwrap().to_string()
      }

      #[sqlx::test(migrations = "../../migrations")]
      async fn test_import_post_is_rate_limited(pool: PgPool) {
          let app = make_app(pool.clone());
          let cookie = register(&app, "rl_import").await;

          sqlx::query!(
              "UPDATE users SET startgg_api_key = 'test-key' WHERE email = 'rl_import@test.com'"
          )
          .execute(&pool)
          .await
          .unwrap();

          let project_id = create_project(&app, &cookie).await;

          // First 3 requests consume the burst — must NOT be 429
          for i in 0..3 {
              let resp = app
                  .clone()
                  .oneshot(
                      Request::builder()
                          .method("POST")
                          .uri(format!("/projects/{project_id}/import"))
                          .header("cookie", &cookie)
                          .body(Body::empty())
                          .unwrap(),
                  )
                  .await
                  .unwrap();
              assert_ne!(
                  resp.status(),
                  StatusCode::TOO_MANY_REQUESTS,
                  "request {i} should not be rate-limited"
              );
          }

          // 4th request should be rate-limited
          let resp = app
              .clone()
              .oneshot(
                  Request::builder()
                      .method("POST")
                      .uri(format!("/projects/{project_id}/import"))
                      .header("cookie", &cookie)
                      .body(Body::empty())
                      .unwrap(),
              )
              .await
              .unwrap();
          assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
      }
  }
  ```

- [ ] **Step 7: Run the test to verify it fails**

  ```bash
  cd backend && DATABASE_URL=postgres://... cargo test -p api -- routes::import::tests::test_import_post_is_rate_limited
  ```

  Expected: FAIL — the import POST route has no rate limiting yet.

- [ ] **Step 8: Add `rate_limited_post_router()` to `backend/crates/api/src/routes/import.rs`**

  Replace the existing imports block at the top of the file with the full merged version:

  ```rust
  use axum::{
      Json, Router,
      extract::{Path, State},
      http::StatusCode,
      response::IntoResponse,
      routing::post,
  };
  use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};
  use serde::{Deserialize, Serialize};
  use std::sync::Arc;
  use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
  use uuid::Uuid;

  use crate::{
      error::{AppError, Result},
      extractors::ClientIpExtractor,
      routes::auth::{AuthUser, OptionalAuthUser},
      routes::projects::{require_project_access, require_project_read_access},
      state::AppState,
  };
  use common::{
      jobs::ImportParams,
      models::{Job, UserRole},
  };
  ```

  Add the function at the bottom of the file (above `#[cfg(test)]`):

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

- [ ] **Step 9: Update the import route in `backend/crates/api/src/routes/projects.rs`**

  Find:

  ```rust
          .route(
              "/{id}/import",
              post(crate::routes::import::start_import).get(crate::routes::import::get_import_status),
          )
  ```

  Replace with:

  ```rust
          .route("/{id}/import", get(crate::routes::import::get_import_status))
          .merge(crate::routes::import::rate_limited_post_router())
  ```

  The GET (status polling) has no rate limit. The POST (trigger) goes through the governor.

- [ ] **Step 10: Run the failing test — it should now pass**

  ```bash
  cd backend && DATABASE_URL=postgres://... cargo test -p api -- routes::import::tests::test_import_post_is_rate_limited
  ```

  Expected: PASS.

- [ ] **Step 11: Run `prepare-sqlx.sh` to cache the new test queries**

  ```bash
  bash backend/prepare-sqlx.sh
  ```

- [ ] **Step 12: Run the full API test suite to check for regressions**

  ```bash
  cd backend && DATABASE_URL=postgres://... cargo test -p api
  ```

  Expected: all tests pass.

- [ ] **Step 13: Run the full test suite**

  ```bash
  bash test.sh
  ```

  Expected: PASS across backend and frontend.

- [ ] **Step 14: Commit**

  ```bash
  git add backend/crates/api/src/extractors.rs \
          backend/crates/api/src/lib.rs \
          backend/crates/api/src/routes/auth.rs \
          backend/crates/api/src/routes/import.rs \
          backend/crates/api/src/routes/projects.rs \
          backend/.sqlx/
  git commit -m "feat(api): rate-limit POST /import at 1 req/20s burst 3"
  ```
