# Auth Security Hardening — Design Spec

**Date:** 2026-05-16

## Overview

Four targeted security fixes for the Axum auth layer before public launch. All changes are confined to `crates/api/src/routes/auth.rs`, `crates/api/src/routes/projects.rs`, and `crates/api/Cargo.toml` (plus the auth router mount point for the rate-limit layer).

Security headers (issue 4 from the review) are a deployment-time concern handled at the reverse proxy / SvelteKit layer, not in Rust — out of scope for this spec.

---

## Fix 1 — Secure cookie flag

**File:** `crates/api/src/routes/auth.rs`

Add `.secure(true)` to both `session_cookie()` (line 124) and `clear_cookie()` (line 133). This instructs browsers to only transmit the cookie over HTTPS. Harmless in local dev — cookies still work, they're just not sent over plain HTTP (which doesn't exist in local dev anyway).

---

## Fix 2 — Rate limiting on auth endpoints

**Dependency:** Add `tower_governor` to `crates/api/Cargo.toml` via `cargo add`.

**Configuration:** 5 requests per 60 seconds per IP, applied as a `GovernorLayer` wrapping only the `/auth` router. `tower_governor` reads client IP from `X-Forwarded-For` (falling back to socket address), so behind the production reverse proxy it correctly identifies individual clients rather than the proxy.

On limit breach, `tower_governor` returns `429 Too Many Requests` automatically — no custom error handling needed.

The layer is applied at the router mount point (wherever `auth::router()` is nested), not globally, so it does not affect other API routes.

---

## Fix 3 — Max-length validation

**File:** `crates/api/src/routes/auth.rs`

Add upper-bound checks immediately after the existing lower-bound checks in `register`:

| Field | Min | Max |
|---|---|---|
| `username` | 3 | 50 |
| `password` | 8 | 128 |

Password max of 128 prevents DoS via intentionally large inputs to Argon2 (which is CPU-intensive proportional to input size). All violations return `422 Unprocessable Entity` with a descriptive message, consistent with existing error handling.

**File:** `crates/api/src/routes/projects.rs`

Add an upper-bound check after the existing non-empty check in the project creation handler:

| Field | Min | Max |
|---|---|---|
| `name` | 1 (non-empty) | 100 |

---

## Fix 4 — Timing equalization on login

**File:** `crates/api/src/routes/auth.rs`

**Problem:** The current `login` handler returns `401` immediately when the username is not found, but takes ~100ms when the password is wrong (Argon2 verification). An attacker can enumerate valid usernames by measuring response time.

**Fix:** Add a module-level `static DUMMY_HASH: &str` — a real Argon2 hash of a throwaway string, hardcoded as a string literal. When `login` reaches the "user not found" branch, call `verify_password(body.password, DUMMY_HASH.to_string()).await` (ignoring its result) before returning `401`. This runs the full Argon2 verification path in both cases, equalizing response time.

Using `verify_password` (not `hash_password`) is deliberate: it mirrors the exact operation performed on the real code path. `hash_password` uses a different Argon2 operation (generates a new salt) and would not accurately match the timing.

The dummy hash is not a secret — it's a hash of a throwaway string that corresponds to no real user account.

---

## Scope boundary

- No changes to `main.rs`, `state.rs`, `config.rs`, migrations, or any crate other than `api`.
- No changes to existing tests; the fixes are additive guards and do not alter happy-path behaviour.
- Security headers (X-Content-Type-Options, X-Frame-Options, CSP) are handled at the reverse proxy / SvelteKit layer at deployment time — documented separately, not implemented here.
