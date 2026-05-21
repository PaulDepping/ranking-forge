# Login/Register Cookie Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove `Set-Cookie` response headers from the Axum `login`, `register`, and `logout` handlers so SvelteKit is the sole manager of the browser session cookie, eliminating the double-cookie conflict that causes "button does nothing" on login/register.

**Architecture:** The Axum API already returns `session_id` in the JSON response body. The SvelteKit form actions already read it and call `cookies.set()`. Removing the `CookieJar` from the Axum response handlers leaves one cookie-setter — SvelteKit — without changing any other auth logic. The `logout` handler keeps `CookieJar` as an *input* extractor (to read and delete the DB session), just not as a response modifier.

**Tech Stack:** Rust / Axum 0.8, axum-extra CookieJar, SvelteKit (no changes needed in web/).

---

### Task 1: Update tests to expect new behavior (failing tests first)

**Files:**
- Modify: `backend/crates/api/tests/api.rs`

- [ ] **Step 1: Update the `register` helper to read `session_id` from the JSON body**

Replace lines 33–60 (the `register` helper) with:

```rust
/// Register a user and return the session cookie string ("session_id=<uuid>").
async fn register(app: &Router, username: &str, password: &str) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"email": format!("{username}@test.com"), "display_name": username, "password": password})).unwrap(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "register should return 201");

    let body = read_json(resp).await;
    format!("session_id={}", body["session_id"].as_str().unwrap())
}
```

- [ ] **Step 2: Update `auth_login` test to assert no `set-cookie` and read from body**

Replace the block at lines 400–422 (inside `auth_login`) with:

```rust
    assert_eq!(resp.status(), StatusCode::OK);

    // Login must NOT set a session cookie — SvelteKit sets it from the JSON body
    assert!(
        !resp.headers().contains_key("set-cookie"),
        "login must not set a session cookie"
    );

    let body = read_json(resp).await;
    let cookie = format!("session_id={}", body["session_id"].as_str().unwrap());

    // session_id from login body must work for authenticated endpoints
    let resp = get_req(&app, "/auth/me", &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(read_json(resp).await["display_name"], "alice");
```

- [ ] **Step 3: Replace the three "cookie is secure" tests with "no set-cookie" tests**

Delete `auth_register_cookie_is_secure`, `auth_login_cookie_is_secure`, and `auth_logout_cookie_is_secure` entirely and replace with:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn auth_register_no_set_cookie(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"email": "alice@test.com", "display_name": "alice", "password": "password123"})).unwrap(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    assert!(
        !resp.headers().contains_key("set-cookie"),
        "register must not set a cookie (SvelteKit owns browser cookies)"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn auth_login_no_set_cookie(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    register(&app, "alice", "password123").await;

    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"email": "alice@test.com", "password": "password123"}))
                .unwrap(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        !resp.headers().contains_key("set-cookie"),
        "login must not set a cookie (SvelteKit owns browser cookies)"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn auth_logout_no_set_cookie(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice", "password123").await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/logout")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert!(
        !resp.headers().contains_key("set-cookie"),
        "logout must not set a cookie (SvelteKit owns browser cookies)"
    );
}
```

- [ ] **Step 4: Run the backend suite and confirm the new tests fail**

```bash
cd /home/pd/private_projects/ranking_forge && bash backend/test.sh --verbose 2>&1 | grep -E "FAILED|auth_login|auth_register_no|auth_login_no|auth_logout_no"
```

Expected: `auth_login`, `auth_register_no_set_cookie`, `auth_login_no_set_cookie`, and `auth_logout_no_set_cookie` all show FAILED with assertion messages like `"login must not set a session cookie"` or `"register must not set a cookie"`.

---

### Task 2: Remove `CookieJar` from auth response handlers

**Files:**
- Modify: `backend/crates/api/src/routes/auth.rs`

- [ ] **Step 1: Update the import block**

Replace:
```rust
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
```
With:
```rust
use axum_extra::extract::CookieJar;
```

- [ ] **Step 2: Delete `session_cookie()` and `clear_cookie()` helper functions**

Delete the entire `session_cookie` function (currently lines 225–233):
```rust
pub(super) fn session_cookie(id: Uuid) -> Cookie<'static> {
    Cookie::build(("session_id", id.to_string()))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .secure(true)
        .max_age(cookie::time::Duration::seconds(2_592_000))
        .build()
}
```

Delete the entire `clear_cookie` function (currently lines 235–243):
```rust
pub(super) fn clear_cookie() -> Cookie<'static> {
    Cookie::build(("session_id", ""))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .secure(true)
        .max_age(cookie::time::Duration::seconds(0))
        .build()
}
```

- [ ] **Step 3: Update the `login` handler — remove `jar`, update return**

Replace the `login` handler signature and return:

```rust
async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse> {
    let user = sqlx::query_as!(
        User,
        "SELECT id, email, display_name, password_hash, startgg_api_key, created_at FROM users WHERE email = $1",
        body.email.to_lowercase(),
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

    Ok(Json(SessionResponse {
        session_id,
        user: UserResponse::from(user),
    }))
}
```

- [ ] **Step 4: Update the `register` handler — remove `jar`, update return**

Replace the `register` handler signature and return (keep all validation logic unchanged):

```rust
async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse> {
    if !is_valid_email(&body.email) {
        return Err(AppError::UnprocessableEntity(
            "invalid email address".into(),
        ));
    }
    if body.email.chars().count() > 255 {
        return Err(AppError::UnprocessableEntity(
            "email must be at most 255 characters".into(),
        ));
    }
    if body.display_name.chars().count() < 1 {
        return Err(AppError::UnprocessableEntity(
            "display name must not be empty".into(),
        ));
    }
    if body.display_name.chars().count() > 50 {
        return Err(AppError::UnprocessableEntity(
            "display name must be at most 50 characters".into(),
        ));
    }
    if body.password.chars().count() < 8 {
        return Err(AppError::UnprocessableEntity(
            "password must be at least 8 characters".into(),
        ));
    }
    if body.password.chars().count() > 128 {
        return Err(AppError::UnprocessableEntity(
            "password must be at most 128 characters".into(),
        ));
    }

    let password_hash = hash_password(body.password).await?;

    let user = sqlx::query_as!(
        User,
        "INSERT INTO users (email, display_name, password_hash) VALUES ($1, $2, $3)
         RETURNING id, email, display_name, password_hash, startgg_api_key, created_at",
        body.email.to_lowercase(),
        body.display_name,
        password_hash,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.constraint() == Some("users_email_key") => {
            AppError::UnprocessableEntity("email already registered".into())
        }
        other => AppError::Db(other),
    })?;

    let session_id = create_session(&state.db, user.id).await?;

    Ok((
        StatusCode::CREATED,
        Json(SessionResponse {
            session_id,
            user: UserResponse::from(user),
        }),
    ))
}
```

- [ ] **Step 5: Update the `logout` handler — remove response cookie, keep request `jar`**

Replace the last two lines of `logout` (the `jar.add(clear_cookie())` and the `Ok(...)` return):

```rust
async fn logout(State(state): State<AppState>, jar: CookieJar) -> Result<impl IntoResponse> {
    if let Some(cookie) = jar.get("session_id") {
        if let Ok(session_id) = cookie.value().parse::<Uuid>() {
            sqlx::query!("DELETE FROM sessions WHERE id = $1", session_id)
                .execute(&state.db)
                .await?;
        }
    }

    Ok(StatusCode::NO_CONTENT)
}
```

- [ ] **Step 6: Run `cargo check` to confirm no compilation errors**

```bash
cd backend && cargo check -p api 2>&1 | tail -20
```

Expected: no errors.

- [ ] **Step 7: Remove the `cookie` direct dependency**

```bash
cd backend && cargo remove -p api cookie
```

The `cookie` crate is still a transitive dependency of `axum-extra` (via `features = ["cookie"]`), so removing it as a direct dependency is safe. Confirm:

```bash
cargo check -p api 2>&1 | tail -10
```

Expected: no errors.

---

### Task 3: Run full test suite and commit

- [ ] **Step 1: Run the backend auth tests**

```bash
cd backend && bash test.sh 2>&1 | tail -30
```

Expected: all tests PASS, including the four new/updated auth tests.

- [ ] **Step 2: Run the full test suite**

```bash
cd /home/pd/private_projects/ranking_forge && bash test.sh 2>&1 | tail -20
```

Expected: PASS for backend, frontend unit, and frontend e2e sections.

- [ ] **Step 3: Commit**

```bash
cd /home/pd/private_projects/ranking_forge
git add backend/crates/api/src/routes/auth.rs backend/crates/api/tests/api.rs backend/crates/api/Cargo.toml backend/Cargo.lock
git commit -m "fix: remove Set-Cookie from Axum auth responses — SvelteKit owns browser cookies"
```
