# RankingForge

A platform for creating and publishing power rankings in the Super Smash Bros. scene.

TOs and community figureheads select a roster of players, point the tool at their start.gg history, and get a statistically grounded ranking based on upset factor — who beat whom, and by how much of a seed differential.

## Features

- **Import from start.gg** — fetches all relevant tournaments and events automatically
- **Tournament curation** — manually deselect events you don't want counted
- **Upset-factor stats** — per-player win/loss lists sorted by upset factor, plus a full head-to-head table
- **Multi-user** — each user manages their own ranking projects independently

## Tech stack

| Layer | Technology |
|---|---|
| API server | Rust + [Axum](https://github.com/tokio-rs/axum) |
| Background worker | Rust (async, same workspace) |
| Database | PostgreSQL 18 |
| Job queue | Postgres `LISTEN`/`NOTIFY` + `SKIP LOCKED` |
| Frontend | SvelteKit + TypeScript + Tailwind CSS |
| Components | [shadcn-svelte](https://shadcn-svelte.com/) |

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Docker](https://www.docker.com/) + Docker Compose
- [Node.js](https://nodejs.org/) 20+
- A [start.gg API key](https://developer.start.gg/docs/authentication)

## Running locally

```bash
# 1. Copy and fill in the environment file
cp .env.example .env
# Set STARTGG_API_KEY to your start.gg token

# 2. Start the database
docker compose up db -d

# 3. Run the API server (auto-runs migrations on startup)
cd backend && cargo run --bin api

# 4. Run the background worker (separate terminal)
cd backend && cargo run --bin worker

# 5. Run the frontend dev server (separate terminal)
cd web && npm install && npm run dev
```

The app is available at `http://localhost:5173`.

## Environment variables

| Variable | Service | Notes |
|---|---|---|
| `DATABASE_URL` | api, worker | Postgres connection string |
| `STARTGG_API_KEY` | api, worker | Your start.gg API token |
| `CORS_ORIGIN` | api | Origin of the frontend, e.g. `http://localhost:5173` |

## Running with Docker Compose

```bash
# Requires STARTGG_API_KEY and CORS_ORIGIN set in .env
docker compose up
```

## Testing

```bash
# Full test suite (spins up an ephemeral Postgres container)
bash test.sh

# Backend only
bash backend/test.sh

# Frontend unit tests
cd web && npm run test:unit

# Frontend e2e tests (auto-starts mock API + dev server)
cd web && npm run test:e2e
```

## Project structure

```
backend/          Rust workspace
  crates/
    common/       Shared library: DB models, job queue, start.gg client, upset-factor logic
    api/          Axum HTTP server binary
    worker/       Background import worker binary
    e2e/          End-to-end tests
  migrations/     SQL migrations
  openapi.yaml    REST API spec
web/              SvelteKit frontend
```
