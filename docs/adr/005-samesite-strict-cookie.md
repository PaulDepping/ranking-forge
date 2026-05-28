# ADR 005: `SameSite=Strict` on the Cross-Subdomain Session Cookie

## Context

The session cookie is set by `api.rankingforge.com` but must be sent when the browser
makes requests from `rankingforge.com`. This is a cross-origin request (different
hosts), which raises questions about cookie `SameSite` policy.

## Decision

Use `SameSite=Strict` on the session cookie.

## Rationale

`SameSite` is evaluated against the *registrable domain* (eTLD+1), not the full
hostname. Both `rankingforge.com` and `api.rankingforge.com` share the registrable
domain `rankingforge.com`, so they are classified as **same-site** despite being
different origins.

Therefore:

- `SameSite=Strict` allows the cookie to be sent on all requests from `rankingforge.com`
  to `api.rankingforge.com`.
- It provides the strongest CSRF protection: the cookie is never sent from a
  third-party context.
- `SameSite=Lax` or `SameSite=None` would also work but offer weaker protection with
  no benefit for our topology.

## Consequences

- `COOKIE_DOMAIN` must be set to the root domain (`rankingforge.com`) so the cookie is
  scoped to both subdomains, not locked to `api.rankingforge.com` alone.
- Third-party embed scenarios (e.g., an iframe on an unrelated domain) would not
  receive the session cookie — this is intentional and not a use case we support.
- This analysis only holds while both frontend and API share the same registrable
  domain. If they are ever moved to different domains, `SameSite` policy must be
  re-evaluated.
