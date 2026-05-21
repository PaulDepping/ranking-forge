# Worker Transient HTTP Error Retry — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the start.gg HTTP client retry transient 5xx errors (e.g. Cloudflare 520) with bounded backoff, while keeping the existing unlimited 429 retry unchanged.

**Architecture:** Split the current `gql` method into `gql_once` (handles a single attempt including 429 retries) and `gql` (wraps `gql_once` with a bounded outer retry for 5xx errors). The two retry policies are independent: a 429 inside a 5xx retry attempt is fully resolved by the inner layer before the outer one sees anything.

**Tech Stack:** Rust, `backon` (exponential backoff), `reqwest`, `wiremock` (tests)

---

## Files

| File | Change |
|---|---|
| `backend/crates/common/src/startgg/mod.rs` | Add `gql_once` private method; replace body of `gql` with outer 5xx retry; add 3 new tests |

No other files change.

---

## Task 1: Write the failing tests

**Files:**
- Modify: `backend/crates/common/src/startgg/mod.rs` (inside `#[cfg(test)] mod tests`)

The three new tests exercise: (1) recovering after one 5xx, (2) exhausting all 5xx retries, and (3) a 429 occurring inside a 5xx retry attempt.

- [ ] **Step 1: Add the three tests to the test module**

Open `backend/crates/common/src/startgg/mod.rs`. Locate the `#[cfg(test)] mod tests` block (starts around line 147). Add the following tests at the end of that block, before the closing `}`:

```rust
// ── 5xx server error retry ────────────────────────────────────────────────

#[tokio::test]
async fn server_error_once_then_succeeds() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(520))
        .up_to_n_times(1)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": { "videogames": { "nodes": [
                {"id": 1, "name": "Melee", "displayName": null}
            ] } }
        })))
        .mount(&mock)
        .await;

    let games = client(&mock.uri()).search_games("melee").await.unwrap();
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].id, 1);
}

#[tokio::test]
async fn server_error_exhausts_retries() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock)
        .await;

    let err = client(&mock.uri()).search_games("melee").await.unwrap_err();
    assert!(matches!(err, StartggError::Http(_)));
}

#[tokio::test]
async fn rate_limited_during_server_error_retry() {
    let mock = MockServer::start().await;
    // Attempt 1 → 503 (outer 5xx retry fires, calls gql_once again)
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&mock)
        .await;
    // Attempt 2, request 1 → 429 (inner 429 retry fires within that gql_once call)
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(1)
        .mount(&mock)
        .await;
    // Attempt 2, request 2 → 200 success
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": { "videogames": { "nodes": [
                {"id": 1, "name": "Melee", "displayName": null}
            ] } }
        })))
        .mount(&mock)
        .await;

    let games = client(&mock.uri()).search_games("melee").await.unwrap();
    assert_eq!(games.len(), 1);
}
```

---

## Task 2: Run tests to verify they fail

**Files:** (read-only)

- [ ] **Step 1: Run the three new tests**

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo test -p common -- server_error rate_limited_during
```

Expected output — all three tests FAIL:
- `server_error_once_then_succeeds`: fails because the 520 propagates immediately as an error instead of being retried
- `server_error_exhausts_retries`: **passes** even before the fix (the current code returns `StartggError::Http` on the first 503 — correct result, wrong reason). Note this and continue; the test will become meaningful after implementation because it verifies the error type survives retry exhaustion.
- `rate_limited_during_server_error_retry`: fails because the 503 propagates immediately; success is never reached

If `server_error_once_then_succeeds` and `rate_limited_during_server_error_retry` both fail as expected, proceed.

---

## Task 3: Implement `gql_once` and update `gql`

**Files:**
- Modify: `backend/crates/common/src/startgg/mod.rs`

Replace the current `gql` method with `gql_once` + a new `gql`. Find the current `gql` method (starts around line 74, the `async fn gql` inside the `impl StartggClient` block) and replace it entirely with the two methods below.

- [ ] **Step 1: Replace the `gql` method with `gql_once` + `gql`**

Delete the current `async fn gql` method (lines ~74–143) and replace with:

```rust
async fn gql_once<T>(&self, query: &'static str, vars: &serde_json::Value) -> Result<T, StartggError>
where
    T: serde::de::DeserializeOwned,
{
    use backon::{ExponentialBuilder, Retryable};
    use queries::{GqlRequest, GqlResponse};

    (|| async {
        let body = self
            .http
            .post(&self.base_url)
            .bearer_auth(&self.api_key)
            .json(&GqlRequest {
                query,
                variables: vars,
            })
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        let resp: GqlResponse<serde_json::Value> =
            serde_json::from_str(&body).map_err(|e| {
                let preview: String = body.chars().take(500).collect();
                tracing::error!(body = %preview, "failed to decode start.gg response: {e}");
                StartggError::Decode(e.to_string())
            })?;

        if let Some(errors) = resp.errors {
            let msg = errors
                .into_iter()
                .map(|e| e.message)
                .collect::<Vec<_>>()
                .join("; ");
            if let Some(err) = parse_complexity_error(&msg) {
                return Err(err);
            }
            tracing::error!(body = %body, "start.gg returned GraphQL errors: {msg}");
            return Err(StartggError::GraphQL(msg));
        }

        let data_value = resp
            .data
            .ok_or_else(|| StartggError::GraphQL("empty data field in response".into()))?;
        serde_json::from_value(data_value).map_err(|e| {
            tracing::error!("failed to decode start.gg data: {e}");
            StartggError::Decode(e.to_string())
        })
    })
    .retry(
        ExponentialBuilder::default()
            .with_min_delay(self.retry_min_delay)
            .with_max_delay(Duration::from_secs(60))
            .with_max_times(usize::MAX)
            .with_jitter(),
    )
    .when(|e| {
        matches!(e, StartggError::Http(re) if re.status()
            == Some(reqwest::StatusCode::TOO_MANY_REQUESTS))
    })
    .notify(|_err, dur| {
        tracing::debug!(?dur, "start.gg rate limited; retrying");
    })
    .await
}

async fn gql<V, T>(&self, query: &'static str, variables: V) -> Result<T, StartggError>
where
    V: Serialize,
    T: serde::de::DeserializeOwned,
{
    use backon::{ExponentialBuilder, Retryable};

    let vars =
        serde_json::to_value(variables).map_err(|e| StartggError::GraphQL(e.to_string()))?;

    (|| self.gql_once(query, &vars))
        .retry(
            ExponentialBuilder::default()
                .with_min_delay(self.retry_min_delay)
                .with_max_delay(Duration::from_secs(30))
                .with_max_times(5)
                .with_jitter(),
        )
        .when(|e| {
            matches!(e, StartggError::Http(re) if re.status()
                .map(|s| s.is_server_error())
                .unwrap_or(false))
        })
        .notify(|_err, dur| {
            tracing::warn!(?dur, "start.gg server error; retrying");
        })
        .await
}
```

**Notes on the implementation:**
- `gql_once` takes `vars: &serde_json::Value` instead of the generic `V: Serialize` — serialization is done once in `gql` and the reference is reused across retry attempts.
- `GqlRequest { query, variables: vars }` — `vars` is already `&serde_json::Value`, same type as `&vars` in the original code.
- Both retry layers share `self.retry_min_delay` as their base delay so the test client's 1ms override applies to both.
- `gql_once` is not `pub` — callers outside `StartggClient` are unaffected.

---

## Task 4: Run all common tests

**Files:** (read-only)

- [ ] **Step 1: Build to catch compilation errors**

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo build -p common
```

Expected: compiles without errors. If there are type errors on `GqlRequest { variables: vars }`, check the definition of `GqlRequest` in `crates/common/src/startgg/queries.rs` — the field may need `variables: vars` or `variables: *vars` depending on its type. Match the field type to what the original `variables: &vars` provided.

- [ ] **Step 2: Run the full common test suite**

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo test -p common
```

Expected: all tests pass, including the 3 new ones. The `server_error_once_then_succeeds` test will now succeed because the 520 triggers the outer retry, which calls `gql_once` again and gets the 200 response. The `rate_limited_during_server_error_retry` test will succeed because the 429 inside the second `gql_once` call is caught by the inner retry.

If `server_error_exhausts_retries` is slow (unexpectedly), check that `with_retry_min_delay(Duration::from_millis(1))` is applied in the `client()` test helper — it must flow through to both the inner and outer builders via `self.retry_min_delay`.

---

## Task 5: Format and commit

**Files:**
- Modify: `backend/crates/common/src/startgg/mod.rs` (formatting)

- [ ] **Step 1: Run rustfmt**

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo fmt --all
```

- [ ] **Step 2: Commit**

```bash
cd /home/pd/private_projects/ranking_forge/backend
git add crates/common/src/startgg/mod.rs
git commit -m "feat(worker): retry transient 5xx errors from start.gg with bounded backoff"
```
