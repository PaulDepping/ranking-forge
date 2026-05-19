# Docker CI/CD Pipeline Design

**Date:** 2026-05-19  
**Status:** Approved

## Overview

A single GitHub Actions workflow (`ci.yml`) that runs the full test suite on every push and pull request, then builds and publishes Docker images to GitHub Container Registry (GHCR) on pushes to `main` and semver tags.

## Scope

- Three images: `api`, `worker` (from `backend/Dockerfile`), and `web` (from `web/Dockerfile`)
- Registry: `ghcr.io/pauldepping/ranking-forge/{api,worker,web}`
- Architecture: `linux/amd64` only
- No new Dockerfiles or compose changes — existing files are used as-is

## Triggers

| Event | Jobs run | Images pushed |
|---|---|---|
| Pull request → `main` | `test`, `build-backend`, `build-web` | No |
| Push to `main` | `test`, `build-backend`, `build-web` | Yes (`:latest`) |
| Push of `v*.*.*` tag | `test`, `build-backend`, `build-web` | Yes (versioned + `:latest`) |

## Job Structure

```
test
  ├── build-backend  (runs in parallel after test passes)
  └── build-web      (runs in parallel after test passes)
```

## Job: `test`

**Runner:** `ubuntu-latest`  
**Permissions:** default (read-only)

**Setup:**
- `dtolnay/rust-toolchain@stable` — installs stable Rust, respects `rust-toolchain.toml` if present
- `Swatinem/rust-cache@v2` with `workspaces: backend/` — caches Cargo registry and build artifacts
- `actions/setup-node@v4` with `node-version: '22'` and `cache: 'npm'`, `cache-dependency-path: web/package-lock.json`
- `npm ci` in `web/`
- `npx playwright install --with-deps chromium` — installs Chromium and OS-level browser deps

**Execution:** `bash test.sh`

`backend/test.sh` launches its own ephemeral Postgres 18 container via Docker on port 15432. No `services:` block is needed — the runner's pre-installed Docker daemon handles it.

## Job: `build-backend`

**Runner:** `ubuntu-latest`  
**Permissions:** `packages: write`, `contents: read`  
**Needs:** `test`

**Steps:**
1. Checkout
2. `docker/setup-buildx-action@v3` — enables BuildKit
3. `docker/login-action@v3` — authenticates to `ghcr.io` using `${{ github.token }}` (skipped on PRs)
4. Two `docker/metadata-action@v5` steps — one for `api`, one for `worker` — generating tags and OCI labels
5. Two `docker/build-push-action@v6` steps:
   - `context: backend/`, `target: api`, image `ghcr.io/pauldepping/ranking-forge/api`
   - `context: backend/`, `target: worker`, image `ghcr.io/pauldepping/ranking-forge/worker`
   - Both use `cache-from: type=gha` and `cache-to: type=gha,mode=max` with a shared cache key so BuildKit reuses the common builder layer across both targets
   - `push: ${{ github.event_name != 'pull_request' }}`

## Job: `build-web`

**Runner:** `ubuntu-latest`  
**Permissions:** `packages: write`, `contents: read`  
**Needs:** `test`

**Steps:**
1. Checkout
2. `docker/setup-buildx-action@v3`
3. `docker/login-action@v3` (skipped on PRs)
4. `docker/metadata-action@v5` — tags and labels for `web`
5. `docker/build-push-action@v6`:
   - `context: web/`, `target: web`, image `ghcr.io/pauldepping/ranking-forge/web`
   - `build-args: PUBLIC_API_URL=${{ vars.PUBLIC_API_URL }}` — set as a repository variable in GitHub settings
   - `cache-from: type=gha`, `cache-to: type=gha,mode=max`
   - `push: ${{ github.event_name != 'pull_request' }}`

## Image Tagging

Managed by `docker/metadata-action` with the `semver` flavor:

| Git ref | Tags |
|---|---|
| Push to `main` | `latest` |
| Tag `v1.2.3` | `v1.2.3`, `1.2`, `1`, `latest` |
| Pull request #42 | `pr-42` (not pushed) |

## Build Caching

- **Rust layers:** GHA cache backend (`type=gha`) for BuildKit layer cache. The builder stage (dependency compile) changes only when `Cargo.lock` changes, so it hits the cache on most runs. The `api` and `worker` build steps share a cache key so the common builder layer is reused between them.
- **Cargo registry/artifacts:** `Swatinem/rust-cache@v2` in the `test` job (does not apply to Docker builds, which use BuildKit's own cache).
- **npm:** `actions/setup-node` npm cache in the `test` job.

## Secrets and Variables

| Name | Type | Used by | Purpose |
|---|---|---|---|
| `GITHUB_TOKEN` | Built-in secret | `build-backend`, `build-web` | GHCR authentication via `github.token` |
| `PUBLIC_API_URL` | Repository variable (`vars.*`) | `build-web` | Passed as `PUBLIC_API_URL` build arg to web Dockerfile |

No additional secrets need to be created. `PUBLIC_API_URL` should be set in **GitHub → Settings → Secrets and variables → Actions → Variables** before the first tag push.

## File to Create

```
.github/
  workflows/
    ci.yml
```
