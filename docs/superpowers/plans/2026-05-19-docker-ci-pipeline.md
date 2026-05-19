# Docker CI/CD Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create `.github/workflows/ci.yml` — a single GitHub Actions workflow that runs tests on every push/PR and builds + publishes Docker images to GHCR on pushes to `main` and semver tags.

**Architecture:** One workflow file with three jobs: `test` (runs `bash test.sh`) gates two parallel build jobs (`build-backend`, `build-web`). Build jobs push to `ghcr.io/pauldepping/ranking-forge/{api,worker,web}` on non-PR events only. Auth uses the built-in `GITHUB_TOKEN`; no extra secrets needed.

**Tech Stack:** GitHub Actions, `docker/build-push-action@v6`, `docker/metadata-action@v5`, `docker/login-action@v3`, `docker/setup-buildx-action@v3`, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, `actions/setup-node@v4`, `actions/checkout@v4`

---

## File Map

| Action | Path |
|---|---|
| Create | `.github/workflows/ci.yml` |

---

### Task 1: Install actionlint for local YAML validation

**Files:**
- No project files changed (tool install only)

- [ ] **Step 1: Install actionlint**

  `actionlint` is a static checker for GitHub Actions workflow files. Install it with:

  ```bash
  go install github.com/rhysd/actionlint/cmd/actionlint@latest
  ```

  If Go is not available, download a binary release directly:

  ```bash
  bash <(curl https://raw.githubusercontent.com/rhysd/actionlint/main/scripts/download-actionlint.bash)
  # Move to somewhere on PATH, e.g.:
  mv actionlint ~/.local/bin/
  ```

  Verify:

  ```bash
  actionlint --version
  ```

  Expected output: `actionlint 1.x.x`

---

### Task 2: Create the workflow file

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create the directory**

  ```bash
  mkdir -p .github/workflows
  ```

- [ ] **Step 2: Write the complete workflow file**

  Create `.github/workflows/ci.yml` with the following content:

  ```yaml
  name: CI

  on:
    push:
      branches: [main]
      tags: ['v*.*.*']
    pull_request:
      branches: [main]

  concurrency:
    group: ${{ github.workflow }}-${{ github.ref }}
    cancel-in-progress: true

  jobs:
    test:
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v4

        - uses: dtolnay/rust-toolchain@stable

        - uses: Swatinem/rust-cache@v2
          with:
            workspaces: backend/

        - uses: actions/setup-node@v4
          with:
            node-version: '22'
            cache: npm
            cache-dependency-path: web/package-lock.json

        - name: Install frontend deps
          run: npm ci
          working-directory: web

        - name: Install Playwright browsers
          run: npx playwright install --with-deps chromium
          working-directory: web

        - name: Run tests
          run: bash test.sh

    build-backend:
      needs: test
      runs-on: ubuntu-latest
      permissions:
        contents: read
        packages: write
      steps:
        - uses: actions/checkout@v4

        - uses: docker/setup-buildx-action@v3

        - name: Log in to GHCR
          if: github.event_name != 'pull_request'
          uses: docker/login-action@v3
          with:
            registry: ghcr.io
            username: ${{ github.actor }}
            password: ${{ secrets.GITHUB_TOKEN }}

        - name: Docker metadata for api
          id: meta-api
          uses: docker/metadata-action@v5
          with:
            images: ghcr.io/pauldepping/ranking-forge/api
            tags: |
              type=semver,pattern={{version}}
              type=semver,pattern={{major}}.{{minor}}
              type=semver,pattern={{major}}
              type=raw,value=latest,enable={{is_default_branch}}
              type=ref,event=pr

        - name: Docker metadata for worker
          id: meta-worker
          uses: docker/metadata-action@v5
          with:
            images: ghcr.io/pauldepping/ranking-forge/worker
            tags: |
              type=semver,pattern={{version}}
              type=semver,pattern={{major}}.{{minor}}
              type=semver,pattern={{major}}
              type=raw,value=latest,enable={{is_default_branch}}
              type=ref,event=pr

        - name: Build and push api
          uses: docker/build-push-action@v6
          with:
            context: backend/
            target: api
            push: ${{ github.event_name != 'pull_request' }}
            tags: ${{ steps.meta-api.outputs.tags }}
            labels: ${{ steps.meta-api.outputs.labels }}
            cache-from: type=gha,scope=backend
            cache-to: type=gha,mode=max,scope=backend

        - name: Build and push worker
          uses: docker/build-push-action@v6
          with:
            context: backend/
            target: worker
            push: ${{ github.event_name != 'pull_request' }}
            tags: ${{ steps.meta-worker.outputs.tags }}
            labels: ${{ steps.meta-worker.outputs.labels }}
            cache-from: type=gha,scope=backend
            cache-to: type=gha,mode=max,scope=backend

    build-web:
      needs: test
      runs-on: ubuntu-latest
      permissions:
        contents: read
        packages: write
      steps:
        - uses: actions/checkout@v4

        - uses: docker/setup-buildx-action@v3

        - name: Log in to GHCR
          if: github.event_name != 'pull_request'
          uses: docker/login-action@v3
          with:
            registry: ghcr.io
            username: ${{ github.actor }}
            password: ${{ secrets.GITHUB_TOKEN }}

        - name: Docker metadata for web
          id: meta-web
          uses: docker/metadata-action@v5
          with:
            images: ghcr.io/pauldepping/ranking-forge/web
            tags: |
              type=semver,pattern={{version}}
              type=semver,pattern={{major}}.{{minor}}
              type=semver,pattern={{major}}
              type=raw,value=latest,enable={{is_default_branch}}
              type=ref,event=pr

        - name: Build and push web
          uses: docker/build-push-action@v6
          with:
            context: web/
            target: web
            push: ${{ github.event_name != 'pull_request' }}
            tags: ${{ steps.meta-web.outputs.tags }}
            labels: ${{ steps.meta-web.outputs.labels }}
            build-args: PUBLIC_API_URL=${{ vars.PUBLIC_API_URL }}
            cache-from: type=gha,scope=web
            cache-to: type=gha,mode=max,scope=web
  ```

  **Key design notes:**
  - `concurrency` cancels in-progress runs on the same ref when a new push arrives (prevents queue pile-up on rapid pushes to `main`)
  - The `api` and `worker` build steps both use `scope=backend` so they share BuildKit's GHA layer cache — the expensive Rust builder stage is only compiled once per cache miss
  - `cache-to: mode=max` stores all intermediate layers, not just the final image layer
  - `push: ${{ github.event_name != 'pull_request' }}` — images are built on PRs (validates the Dockerfile works) but only pushed on `main`/tags

---

### Task 3: Lint the workflow file

**Files:**
- Possibly modify: `.github/workflows/ci.yml` (fix any lint errors)

- [ ] **Step 1: Run actionlint**

  From the repo root:

  ```bash
  actionlint .github/workflows/ci.yml
  ```

  Expected output: no output (exit code 0 = no errors).

  If actionlint reports errors, fix them in `.github/workflows/ci.yml` before continuing. Common issues:
  - `${{ }}` expressions that reference undefined step IDs
  - Invalid `on:` syntax
  - Unknown action versions (actionlint checks these against a built-in database)

---

### Task 4: Set the `PUBLIC_API_URL` repository variable on GitHub

**Files:** None (GitHub settings UI)

- [ ] **Step 1: Open repository variables**

  Go to: `https://github.com/PaulDepping/ranking-forge/settings/variables/actions`

- [ ] **Step 2: Create the variable**

  Click **New repository variable** and set:
  - **Name:** `PUBLIC_API_URL`
  - **Value:** The public URL where your API is reachable from the browser (e.g. `https://api.rankingforge.example.com`). Use a placeholder value for now if the deployment URL isn't decided yet.

  This value is baked into the web container at build time as a Next.js/SvelteKit public env variable (`PUBLIC_API_URL`). It can be changed by rebuilding the image.

---

### Task 5: Commit, push, and verify first workflow run

**Files:**
- Commit: `.github/workflows/ci.yml`

- [ ] **Step 1: Stage and commit**

  ```bash
  git add .github/workflows/ci.yml
  git commit -m "ci: add GitHub Actions workflow for testing and Docker image publishing"
  ```

- [ ] **Step 2: Push to main**

  ```bash
  git push origin main
  ```

- [ ] **Step 3: Watch the workflow run**

  Open: `https://github.com/PaulDepping/ranking-forge/actions`

  You should see a new run named **CI** triggered by the push. Verify:
  - The `test` job starts and all three sections (backend, frontend unit, frontend e2e) pass
  - After `test` passes, `build-backend` and `build-web` start in parallel
  - Both build jobs complete successfully and show "Pushed" in the `build-push-action` step output

- [ ] **Step 4: Verify images are published**

  After a successful push, navigate to:
  - `https://github.com/PaulDepping?tab=packages`

  You should see three packages: `ranking-forge/api`, `ranking-forge/worker`, `ranking-forge/web`, each tagged `:latest`.

---

### Task 6: Smoke-test a versioned release

**Files:** None (git tag only)

- [ ] **Step 1: Push a semver tag**

  ```bash
  git tag v0.1.0
  git push origin v0.1.0
  ```

- [ ] **Step 2: Verify versioned tags**

  Open the Actions tab and watch the tagged run complete. Then check the packages page — each image should now have tags: `v0.1.0`, `0.1`, `0`, and `latest`.

  If this is not yet a real release, delete the tag afterwards:

  ```bash
  git tag -d v0.1.0
  git push origin --delete v0.1.0
  ```
