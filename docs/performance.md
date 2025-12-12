# Performance Guide

This guide captures realistic expectations for how much faster `rjest` can be compared to traditional Jest workflows. All figures assume “Option A”: a Rust daemon that caches SWC transforms and orchestrates a pool of persistent Node workers while exposing a Jest-compatible CLI.

## Cold vs. warm runs

- **Cold daemon run:** First invocation after starting the daemon must parse config, discover all tests, compile every file once, and spin up workers. Expect roughly a 1.2–2× speedup versus Jest because SWC replaces Babel/`ts-jest` and the daemon parallelizes work efficiently, but the cold start is still bounded by project size.
- **Warm daemon run:** Once caches are populated and workers are hot, subsequent runs only recompile files whose content hash changed and reuse the existing dependency graph. This is where the daemon delivers the largest gains.

## Scenario estimates

| Scenario | Jest today | `rjest` warm | Expected gain |
| --- | --- | --- | --- |
| **A. Iterating on a single test file**: Edit `src/foo.ts`, rerun `npm test src/foo.test.ts`. | 5–15 s (rebootstrap, retransform) | 0.5–3 s (only changed files recompiled) | ≈5–10× faster |
| **B. Re-running entire suite after small edits**: `npm test` repeatedly on a large repo. | 30–60 s each time | 10–25 s when most files unchanged | ≈2–3× faster |
| **C. Re-running without code changes**: Run suite twice in a row. | 30–60 s every time | 5–15 s (dominated by the tests themselves) | ≈2–5× faster |

These numbers assume medium-to-large TypeScript/React applications where transform cost and Jest bootstrap dominate run time. Projects whose tests are inherently slow (e.g., heavy integration tests) will still benefit from lower overhead but may not see double-digit multipliers.

## Why repeated runs get faster

1. **No repeated bootstrap:** Loading Jest, parsing config, and crawling the filesystem happens once. Later runs simply reuse the in-memory graph.
2. **Cached transforms:** SWC outputs are keyed by content hash, so editing one module re-emits just that module and its dependents.
3. **Persistent workers:** Node workers remain live, keeping fake timers, expect libraries, and module state ready, which eliminates process launch latency.
4. **Targeted execution:** The daemon can run “tests affected by these files” or “last failures only,” letting agents avoid rerunning entire suites unnecessarily.
5. **Purpose-built tooling:** `async-nng` keeps CLI↔daemon messaging latency low, `sled` makes cache lookups and writes effectively instant even across restarts, and `ryv` coordinates asynchronous tasks so transform, watch, and scheduling work never block one another.

## Guidance for users and agents

- Expect the **first** run to feel similar to a fresh Jest execution; measure gains on the second and third runs.
- Favor targeted commands (`jest path/to/test`, `--onlyChanged`, “tests affected by edited files”) to exploit the dependency graph.
- Use machine-readable output (`--json` or `--machine`) when integrating with AI agents so they can quickly respond to per-test results without parsing human-oriented logs.
- Keep the daemon alive during active development or automated refactors; shutting it down discards caches and forfeits the speed advantage.
