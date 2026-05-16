# Auth Security Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix four auth security issues — secure cookie flag, input max-length validation, login timing equalization, and per-IP rate limiting on auth endpoints.

**Architecture:** All changes are confined to the `api` crate. Fixes 1–3 are in-place guards in existing handlers. Fix 4 (rate limiting) uses `tower_governor` as a Tower middleware layer applied only to the `/auth` router. A custom IP extractor falls back to `LOCALHOST` when no peer IP is available (tests), so rate-limit state stays per-test-instance.

**Tech Stack:** Rust/Axum 0.8, `tower_governor`, `argon2`, `std::sync::LazyLock`

---

## File Map

| File | Change |
|---|---|
| `backend/crates/api/Cargo.toml` | Add `tower_governor` |
| `backend/crates/api/src/routes/auth.rs` | `.secure(true)` on cookies; max-length guards; `LazyLock` dummy hash; timing equalization; `ClientIpExtractor`; `GovernorLayer` |
| `backend/crates/api/src/routes/projects.rs` | Max-length guard on project name |
| `backend/crates/api/tests/api.rs` | Tests for all four fixes |

---

## Task 1: Add `tower_governor` dependency

**Files:**
- Modify: `backend/crates/api/Cargo.toml`

- [ ] **Step 1: Add the crate**

```bash
cd backend && cargo add tower_governor -p api
```

Expected: `Cargo.toml` gains a `tower_governor` entry, `Cargo.lock` updated.

- [ ] **Step 2: Verify it compiles**

```bash
cd backend && cargo build -p api
```

Expected: compiles without errors.

- [ ] **Step 3: Commit**

```bash
git add backend/crates/api/Cargo.toml backend/Cargo.lock
git commit -m "chore(api): add tower_governor dependency"
```

---

## Task 2: Secure cookie flag

**Files:**
- Modify: `backend/crates/api/src/routes/auth.rs`
- Modify: `backend/crates/api/tests/api.rs`

- [ ] **Step 1: Write the failing tests**

Append to `backend/crates/api/tests/api.rs`:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn auth_register_cookie_is_secure(pool: PgPool) {
    let app = make_app(pool, "");
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"username": "alice", "password": "password123"})).unwrap(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let cookie = resp.headers().get("set-cookie").unwrap().to_str().unwrap();
    assert!(cookie.contains("Secure"), "register cookie must have Secure flag; got: {cookie}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn auth_login_cookie_is_secure(pool: PgPool) {
    let app = make_app(pool, "");
    register(&app, "alice", "password123").await;

    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"username": "alice", "password": "password123"})).unwrap(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cookie = resp.headers().get("set-cookie").unwrap().to_str().unwrap();
    assert!(cookie.contains("Secure"), "login cookie must have Secure flag; got: {cookie}");
}
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cd backend && cargo test -p api -- auth_register_cookie_is_secure auth_login_cookie_is_secure 2>&1 | tail -20
```

Expected: FAILED — assertion `cookie.contains("Secure")` fails.

- [ ] **Step 3: Add `.secure(true)` to both cookie builders**

In `backend/crates/api/src/routes/auth.rs`, find `session_cookie` (line ~124) and add `.secure(true)`:

```rust
fn session_cookie(id: Uuid) -> Cookie<'static> {
    Cookie::build(("session_id", id.to_string()))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .secure(true)
        .max_age(cookie::time::Duration::seconds(2_592_000))
        .build()
}
```

Find `clear_cookie` (line ~133) and add `.secure(true)`:

```rust
fn clear_cookie() -> Cookie<'static> {
    Cookie::build(("session_id", ""))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .secure(true)
        .max_age(cookie::time::Duration::seconds(0))
        .build()
}
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cd backend && cargo test -p api -- auth_register_cookie_is_secure auth_login_cookie_is_secure 2>&1 | tail -10
```

Expected: both PASS.

- [ ] **Step 5: Commit**

```bash
git add backend/crates/api/src/routes/auth.rs backend/crates/api/tests/api.rs
git commit -m "fix(api): add Secure flag to session cookies"
```

---

## Task 3: Max-length validation

**Files:**
- Modify: `backend/crates/api/src/routes/auth.rs`
- Modify: `backend/crates/api/src/routes/projects.rs`
- Modify: `backend/crates/api/tests/api.rs`

- [ ] **Step 1: Write the failing tests**

Append to `backend/crates/api/tests/api.rs`:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn auth_register_long_username(pool: PgPool) {
    let app = make_app(pool, "");
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({
                "username": "a".repeat(51),
                "password": "password123"
            }))
            .unwrap(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../../migrations")]
async fn auth_register_long_password(pool: PgPool) {
    let app = make_app(pool, "");
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({
                "username": "alice",
                "password": "x".repeat(129)
            }))
            .unwrap(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../../migrations")]
async fn projects_create_long_name(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({"name": "x".repeat(101)}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cd backend && cargo test -p api -- auth_register_long_username auth_register_long_password projects_create_long_name 2>&1 | tail -20
```

Expected: all three FAIL (201/201/201 instead of 422).

- [ ] **Step 3: Add upper-bound checks in `auth.rs`**

In `backend/crates/api/src/routes/auth.rs`, inside `register`, add these checks immediately after the existing min-length checks:

```rust
// After: if body.username.len() < 3 { ... }
if body.username.len() > 50 {
    return Err(AppError::UnprocessableEntity(
        "username must be at most 50 characters".into(),
    ));
}
// After: if body.password.len() < 8 { ... }
if body.password.len() > 128 {
    return Err(AppError::UnprocessableEntity(
        "password must be at most 128 characters".into(),
    ));
}
```

The resulting validation block should be:

```rust
if body.username.len() < 3 {
    return Err(AppError::UnprocessableEntity(
        "username must be at least 3 characters".into(),
    ));
}
if body.username.len() > 50 {
    return Err(AppError::UnprocessableEntity(
        "username must be at most 50 characters".into(),
    ));
}
if body.password.len() < 8 {
    return Err(AppError::UnprocessableEntity(
        "password must be at least 8 characters".into(),
    ));
}
if body.password.len() > 128 {
    return Err(AppError::UnprocessableEntity(
        "password must be at most 128 characters".into(),
    ));
}
```

- [ ] **Step 4: Add upper-bound check in `projects.rs`**

In `backend/crates/api/src/routes/projects.rs`, inside the project creation handler, add this check immediately after the existing empty-name check:

```rust
// After: if body.name.trim().is_empty() { ... }
if body.name.trim().len() > 100 {
    return Err(AppError::UnprocessableEntity(
        "name must be at most 100 characters".into(),
    ));
}
```

- [ ] **Step 5: Run tests to confirm they pass**

```bash
cd backend && cargo test -p api -- auth_register_long_username auth_register_long_password projects_create_long_name 2>&1 | tail -10
```

Expected: all three PASS.

- [ ] **Step 6: Run the full API test suite to check for regressions**

```bash
cd backend && cargo test -p api 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add backend/crates/api/src/routes/auth.rs backend/crates/api/src/routes/projects.rs backend/crates/api/tests/api.rs
git commit -m "fix(api): add max-length validation for username, password, and project name"
```

---

## Task 4: Timing equalization on login

**Files:**
- Modify: `backend/crates/api/src/routes/auth.rs`

The existing tests `auth_login_unknown_user` and `auth_login_wrong_password` already verify that both paths return 401 — they cover the observable behavior. This task only changes the internal timing (no new test needed).

- [ ] **Step 1: Add `LazyLock` import and `DUMMY_HASH` static**

At the top of `backend/crates/api/src/routes/auth.rs`, add `LazyLock` to the existing `use` block:

```rust
use std::sync::LazyLock;
```

Then add the static immediately above the `// ── Helpers` section:

```rust
// Equalize login response time when a username isn't found: verify_password against
// this hash so the code path matches a real wrong-password attempt (~100ms Argon2).
static DUMMY_HASH: LazyLock<String> = LazyLock::new(|| {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(b"dummy_sentinel_never_matches", &salt)
        .unwrap()
        .to_string()
});
```

Note: `LazyLock` initializes synchronously on first access. This blocks the async thread for ~100ms on the very first login attempt after startup — a one-time cost.

- [ ] **Step 2: Restructure the `login` handler to equalize timing**

Replace the current `login` handler body with:

```rust
async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<AuthRequest>,
) -> Result<impl IntoResponse> {
    let user = sqlx::query_as!(
        User,
        "SELECT id, username, password_hash, created_at FROM users WHERE username = $1",
        body.username,
    )
    .fetch_optional(&state.db)
    .await?;

    let user = match user {
        Some(u) => u,
        None => {
            let _ = verify_password(body.password, (*DUMMY_HASH).clone()).await;
            return Err(AppError::Unauthorized);
        }
    };

    verify_password(body.password, user.password_hash.clone()).await?;

    let session_id = create_session(&state.db, user.id).await?;
    let jar = jar.add(session_cookie(session_id));

    Ok((jar, Json(UserResponse::from(user))))
}
```

- [ ] **Step 3: Verify existing auth tests still pass**

```bash
cd backend && cargo test -p api -- auth_login 2>&1 | tail -20
```

Expected: `auth_login`, `auth_login_wrong_password`, `auth_login_unknown_user` all PASS.

- [ ] **Step 4: Commit**

```bash
git add backend/crates/api/src/routes/auth.rs
git commit -m "fix(api): equalize login timing to prevent username enumeration"
```

---

## Task 5: Rate limiting on auth endpoints

**Files:**
- Modify: `backend/crates/api/src/routes/auth.rs`
- Modify: `backend/crates/api/tests/api.rs`

- [ ] **Step 1: Write the failing test**

Append to `backend/crates/api/tests/api.rs`:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn auth_rate_limit_after_burst(pool: PgPool) {
    let app = make_app(pool, "");

    // burst_size=5: first 5 requests must go through (returned as 422 — short username
    // fails validation before any expensive work, making the test fast)
    for i in 0..5 {
        let req = Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&json!({"username": "ab", "password": "x"})).unwrap(),
            ))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_ne!(
            resp.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "request {i} should not be rate limited within burst"
        );
    }

    // 6th request must be rate limited
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"username": "ab", "password": "x"})).unwrap(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}
```

- [ ] **Step 2: Run to confirm it fails**

```bash
cd backend && cargo test -p api -- auth_rate_limit_after_burst 2>&1 | tail -10
```

Expected: FAIL — 6th request returns 422 instead of 429.

- [ ] **Step 3: Add imports and `ClientIpExtractor` to `auth.rs`**

Add these imports at the top of `backend/crates/api/src/routes/auth.rs`:

```rust
use std::sync::Arc;
use tower_governor::{GovernorConfigBuilder, GovernorLayer};
use tower_governor::key_extractor::KeyExtractor;
```

Add the custom key extractor struct immediately after the imports, before the `// ── Request / response types` section:

```rust
// Falls back to LOCALHOST when no IP header/extension is present (test environments).
// Each test creates a fresh GovernorLayer instance, so test-isolation is preserved.
#[derive(Clone)]
struct ClientIpExtractor;

impl KeyExtractor for ClientIpExtractor {
    type Key = std::net::IpAddr;

    fn extract<T>(&self, req: &axum::http::Request<T>) -> Result<Self::Key, tower_governor::GovernorError> {
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

- [ ] **Step 4: Add the `GovernorLayer` to the auth router**

Replace the `router()` function at the bottom of `auth.rs` with:

```rust
pub fn router() -> Router<AppState> {
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(ClientIpExtractor)
            .per_second(1)
            .burst_size(5)
            .finish()
            .unwrap(),
    );

    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .layer(GovernorLayer { config: governor_conf })
}
```

- [ ] **Step 5: Run the rate-limit test**

```bash
cd backend && cargo test -p api -- auth_rate_limit_after_burst 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 6: Run the full API test suite**

```bash
cd backend && cargo test -p api 2>&1 | tail -20
```

Expected: all tests pass. If any test fails with 429, it means that test makes more than 5 auth requests — check the test and reduce request count or restructure to use a fresh `make_app`.

- [ ] **Step 7: Run the full backend test suite**

```bash
cd backend && bash test.sh 2>&1 | tail -30
```

Expected: PASS across all crates.

- [ ] **Step 8: Commit**

```bash
git add backend/crates/api/src/routes/auth.rs backend/crates/api/tests/api.rs
git commit -m "fix(api): add per-IP rate limiting (burst 5, 1/s) to auth endpoints"
```
