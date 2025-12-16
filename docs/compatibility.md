# Compatibility & Drop-in Behavior

`rjest` is designed to be a seamless replacement for the standard Jest CLI. This document captures the current expectations for CLI parity, configuration handling, and fallback behavior so teams can adopt the daemon without rewriting existing test infrastructure.

## CLI parity

- **Entry points:** `npx jest`, `npm test`, `yarn test`, and direct `jest` invocations all work once the `rjest` CLI is on the PATH. The shim accepts positional patterns (e.g., `jest src/foo.test.ts`) just like Jest.
- **Core flags supported from day one:**
  - Execution: `--runInBand`, `--watch`, `--watchAll`, `--bail`, `--maxWorkers`, `--onlyChanged`, `--findRelatedTests`
  - Filtering: `--testNamePattern`, `--testPathPattern`, `--env`, `--config`
  - Output: `--coverage`, `--json`, `--machine`, `--listTests`, `--reporters`
- **Multi-project isolation:** The CLI identifies each repository by its project root path and tags every RPC with that identifier. A single daemon multiplexes all repos but keeps caches and worker pools namespaced, so concurrent commands never interfere with each other.
- **Graceful degradation:** When a user provides a flag that `rjest` does not yet implement, the CLI:
  1. Emits a warning describing the missing feature.
  2. Automatically falls back to upstream Jest if `--fallback-to-jest` (or `RJEST_FALLBACK=1`) is set.
  3. Otherwise continues with the supported subset, ensuring unexpected behavior is never silent.

## Configuration handling

- `rjest` defers to Node to load `jest.config.*`, `package.json` `jest` fields, and multi-project setups so existing JavaScript/TypeScript configs continue to work.
- Resolved configs are serialized to JSON and handed to the daemon, which honors:
  - `roots`, `projects`, `testMatch`, `testRegex`
  - `modulePaths`, `moduleNameMapper`, `moduleDirectories`
  - `setupFiles`, `setupFilesAfterEnv`, `testEnvironment`
  - Snapshot serializers, coverage reporters, and `transform` settings that map cleanly onto SWC.
- Custom Babel transforms or `ts-jest` hooks are replaced by the daemon’s SWC pipeline; when a transform cannot be expressed through SWC today, the CLI can fall back to upstream Jest for that run.

## Runtime behavior

- **Module system:** Both CommonJS and ESM modules run inside persistent Node workers to preserve Jest semantics around hoisting, mocks, and globals.
- **Mocking:** Manual `jest.mock`, `jest.spyOn`, and fake timers behave the same because the workers preload Jest-compatible runtime libraries. Advanced automocking is a roadmap item; until then, the CLI warns if a project enables strict automocking modes.
- **Snapshots:** Snapshot files are read and written in the standard Jest format. Tools like `jest -u` continue to behave the same because `rjest` implements the same update flow via the daemon.
- **Coverage:** SWC-powered instrumentation produces Istanbul-compatible output that feeds existing reporters and CI integrations.

## Fallback workflow

- Set the environment variable `RJEST_FALLBACK=1` or pass `--fallback-to-jest` to instruct the CLI to invoke upstream Jest automatically whenever a run uses unsupported options or encounters a compatibility guard.
- The CLI clearly prints which runs went through the fallback path so teams can track remaining gaps.
- Fallback still benefits from `rjest`’s CLI ergonomics because the same command entry points are used; only the execution engine switches to Jest for that invocation.

## CLI Flag Matrix

| Flag | Phase | Status | Notes |
|------|-------|--------|-------|
| `<pattern>` | 1 | Planned | Regex patterns to filter test files |
| `--runInBand`, `-i` | 1 | Planned | Run tests serially |
| `--watch` | 1 | Planned | Re-run on file changes |
| `--watchAll` | 1 | Planned | Re-run all tests on changes |
| `--bail`, `-b` | 1 | Planned | Exit after first failure |
| `--maxWorkers`, `-w` | 1 | Planned | Number of worker processes |
| `--json` | 1 | Planned | Output results as JSON |
| `--machine` | 1 | Planned | Structured output for AI agents |
| `--config`, `-c` | 1 | Planned | Path to config file |
| `--fallback-to-jest` | 1 | Planned | Force upstream Jest |
| `--testNamePattern`, `-t` | 2 | Planned | Filter by test name |
| `--onlyChanged`, `-o` | 3 | Planned | Run affected tests only |
| `--findRelatedTests` | 3 | Planned | Run tests related to files |
| `--coverage` | 4 | Planned | Collect coverage |
| `-u`, `--updateSnapshot` | 4 | Planned | Update snapshots |
| `--env` | 4 | Planned | Test environment |
| `--reporters` | 4 | Planned | Custom reporters |
| `--notify` | 6 | Fallback | Desktop notifications |
| `--watchman` | — | Fallback | Watchman integration |

## Configuration Field Matrix

| Field | Phase | Status | Notes |
|-------|-------|--------|-------|
| `testMatch` | 1 | Planned | Glob patterns for test files |
| `testRegex` | 1 | Planned | Regex for test files |
| `roots` | 1 | Planned | Directories to search |
| `transform` | 1 | Partial | SWC only; custom triggers fallback |
| `setupFiles` | 2 | Planned | Pre-framework scripts |
| `setupFilesAfterEnv` | 2 | Planned | Post-framework scripts |
| `moduleNameMapper` | 2 | Planned | Path aliases |
| `moduleDirectories` | 2 | Planned | Module search directories |
| `projects` | 2 | Planned | Multi-project support |
| `testEnvironment` | 4 | Partial | `node` first; `jsdom` Phase 4 |
| `snapshotSerializers` | 4 | Planned | Custom serializers |
| `coverageReporters` | 4 | Planned | Output formats |
| `automock` | — | Fallback | Auto-mocking deferred |
| `resolver` | — | Fallback | Custom resolvers |

## Runtime API Matrix

| API | Phase | Status | Notes |
|-----|-------|--------|-------|
| `test()` / `it()` | 1 | Planned | Define tests |
| `describe()` | 1 | Planned | Test suites |
| `beforeEach()` / `afterEach()` | 1 | Planned | Test hooks |
| `beforeAll()` / `afterAll()` | 1 | Planned | Suite hooks |
| `expect()` + core matchers | 1 | Planned | Assertions |
| `jest.fn()` | 2 | Planned | Mock functions |
| `jest.spyOn()` | 2 | Planned | Spy on methods |
| `jest.mock()` | 2 | Planned | Manual mocking |
| `jest.useFakeTimers()` | 2 | Planned | Fake timers |
| `toMatchSnapshot()` | 4 | Planned | Snapshot testing |
| `test.each()` | 4 | Planned | Parameterized tests |
| `test.concurrent()` | — | Fallback | Concurrent tests |
| `jest.createMockFromModule()` | — | Fallback | Auto-generate mocks |

## Known gaps (initial release)

- **Automocking modes:** Auto mock hoisting and virtual mocks that rely on Jest internals are not yet implemented; users enabling them should opt into fallback.
- **Exotic environments:** Custom environments that depend on private Jest APIs (beyond the documented `jest-environment-*` contract) may require fallback until we cover the necessary integration points.
- **Custom transformers:** Projects that rely on bespoke Babel plugins or preprocessors outside of SWC's reach will need fallback until those transforms can be re-expressed or replaced.

## Adoption checklist

1. Install `rjest` and ensure `npx jest` points to the shim.
2. Run your existing test commands; confirm they produce identical output.
3. If any command warns about missing features, add `--fallback-to-jest` temporarily and file an issue with the specific flag/config so we can prioritize it.
4. Remove fallback once the gaps are closed; subsequent runs will use the daemon automatically.

By following this guidance, teams can adopt `rjest` incrementally while preserving the familiar Jest developer experience.
