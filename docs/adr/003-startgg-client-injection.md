# ADR 003: All start.gg Calls Through `StartggClient`

## Context

The worker and some API routes need to call the start.gg GraphQL API. The question is
whether route or worker code should use `reqwest` directly, or go through an
abstraction.

## Decision

All start.gg calls must go through `StartggClient` (`common::startgg`). Using `reqwest`
directly in route or worker code is not permitted.

## Rationale

- **Testability.** `StartggClient::new_with_base_url` accepts a URL at construction time.
  Tests pass a `wiremock::MockServer` URL, so no real network calls are made during
  the test suite. Direct `reqwest` usage bypasses this and causes tests to hit the
  real API or fail unpredictably.
- **Single point for auth and error handling.** API key headers, rate-limit retries, and
  GraphQL error mapping live in one place. Bypassing `StartggClient` would scatter this
  logic.

## Consequences

- Adding a new start.gg operation means adding a method or operation to `StartggClient`,
  not writing a one-off `reqwest` call.
- Tests for start.gg behaviour use `wiremock::MockServer` + `StartggClient::new_with_base_url`.
  See existing tests in `crates/common/src/startgg/operations/tests.rs` for examples.
