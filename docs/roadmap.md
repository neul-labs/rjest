# Roadmap to Full Functionality

The goal is to deliver a drop-in Jest replacement with superior performance, rigorous compatibility guarantees, and battle-tested tooling for both humans and AI agents. This roadmap outlines the milestones required to reach “full functionality” while keeping the work incremental and measurable.

## Phase 0 – Foundations (current)

- Document the product vision, architecture, performance expectations, and compatibility story.
- Establish the Rust workspace, select core crates (`async-nng`, `sled`, `ryv`, SWC bindings), and scaffold the daemon + CLI crates.
- Implement a bootstrap CLI that can start/stop the daemon, negotiate RPCs, and print placeholder results.

## Phase 1 – MVP daemon + CLI

- Parse Jest configuration through Node and feed normalized JSON into the daemon.
- Build the SWC transform pipeline with disk-backed caches in `sled`.
- Spawn Node worker pools managed by `ryv`, run simple test modules, and stream structured results through `async-nng`.
- Support essential CLI flags (`--runInBand`, `--watch`, patterns, `--json`) and ensure fallback-to-Jest works when a flag is missing.
- Add smoke tests that run a sample TypeScript/React project through both `rjest` and Jest, comparing pass/fail output.
- Establish core unit test suites for the daemon (graph updates, cache invalidation, worker scheduling) and CLI (flag parsing, fallback logic) so regressions surface early.

## Phase 2 – Compatibility coverage

- Implement support for `setupFiles`, `setupFilesAfterEnv`, CommonJS + ESM interop, manual `jest.mock`, timers, and snapshots.
- Honor `moduleNameMapper`, `moduleDirectories`, `projects`, and multi-root monorepos.
- Launch the compatibility suite:
  - Run Jest’s upstream test suite (where feasible) through `rjest` to detect runtime regressions.
  - Maintain additional “real world” fixtures (React, Next.js, Node services) that exercise different configs.
- Set up CI gating so compatibility tests must pass before merging changes.
- Expand documentation to include a compatibility matrix, migration guides, and troubleshooting references for all supported features introduced in this phase.

## Phase 3 – Performance hardening

- Finish incremental dependency graph updates and “affected tests” queries so `--onlyChanged` and `--findRelatedTests` become near-instant.
- Add daemon health probes, worker recycling, resource leak detection, and structured tracing.
- Build the benchmark harness:
  - Collect representative projects (small, medium, large) with varying TypeScript/JS mixes.
  - Measure cold start, warm single-test, warm full-suite, and zero-change reruns for both `rjest` and upstream Jest.
  - Publish benchmark dashboards per release and fail CI if regressions exceed allowed thresholds.
- Document tuning guides (e.g., cache sizing, worker configuration) and add unit/perf tests covering watchdogs, leaks, and scheduling edge cases introduced here.

## Phase 4 – Advanced feature parity

- Implement jsdom and custom environment support, including lifecycle hooks and environment-specific configuration.
- Expand mocking to cover automocking modes, virtual mocks, and edge cases that rely on Jest internals.
- Add coverage reporters (text, JSON, lcov) wired to SWC instrumentation and confirm parity with Jest output.
- Support reporters API so community reporters can plug in with no code changes.
- Ensure snapshot updates (`-u`) work identically, including custom serializers.
- Accompany each major feature with unit/integration tests plus dedicated documentation sections (how-to guides, API references, known limitations).

## Phase 5 – Multi-repo & multi-session resilience

- Strengthen multi-tenant handling so a single daemon can serve numerous repositories concurrently with strict namespace isolation (config, caches, worker pools).
- Persist daemon metadata per repo, enabling quick restart and continuity across developer sessions.
- Introduce session APIs for AI agents (session IDs, last failures, selective reruns) while keeping standard CLI semantics.

## Phase 6 – Ecosystem polish

- Provide first-class VS Code/JetBrains integrations that proxy their Jest tooling through `rjest`.
- Publish migration guides, FAQ, and troubleshooting docs covering fallback scenarios and known limitations.
- Offer telemetry (opt-in) for anonymized metrics that help prioritize new compatibility features.
- Tag stable releases once both compatibility suite and benchmark gates have passed reliably for multiple versions.

## Deliverables summary

- **Benchmarks:** Automated suite comparing `rjest` vs Jest latency across scenarios; must run in CI and release pipelines.
- **Compatibility tests:** Jest’s official suite plus curated fixtures running under both runtimes with diffing to flag behavior mismatches.
- **Unit & smoke tests:** Comprehensive coverage of daemon subsystems, CLI parsing, RPC wiring, and per-feature regression tests.
- **Documentation:** Living documents (README, architecture, compatibility, performance, roadmap, how-tos) updated alongside each milestone so adopters know what works and how to use it.

By following this roadmap, the project marches from documentation to a production-ready, fully compatible Jest replacement with quantifiable performance benefits and strong validation infrastructure.
