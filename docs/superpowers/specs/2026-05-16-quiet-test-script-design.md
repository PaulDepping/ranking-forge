---
title: Quiet-by-default test.sh
date: 2026-05-16
status: approved
---

## Problem

`test.sh` streams all output from backend (cargo), unit (Vitest), and e2e (Playwright) test suites even when every test passes. A clean run produces hundreds of lines of noise.

## Design

### Default (quiet) mode

Each test section runs with stdout and stderr captured to a temp file. A per-section status line is printed immediately:

```
=== Backend tests ===          PASS
=== Frontend unit tests ===    PASS
=== Frontend e2e tests ===     PASS

All tests passed.
```

On failure the captured output is dumped in full, then the script exits 1. Full context is preserved for debugging.

### Verbose mode (`-v` / `--verbose`)

Streams all output in real-time (current behaviour). Section headers are still printed.

### Arg passthrough

`-v` / `--verbose` is consumed by `test.sh`. All remaining args are forwarded to `backend/test.sh` and on to `cargo test`, so `bash test.sh -- my_test_name` continues to work. Frontend tests receive no extra args.

## Scope

One file changed: `test.sh` in the repo root. `backend/test.sh` is unchanged.
