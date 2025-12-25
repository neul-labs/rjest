# rjest

> **Status:** Working prototype with **100× speedup** on warm runs (14ms vs 1.4s)

`rjest` is a Jest-compatible test runner that keeps a long-lived Rust daemon in the background so repeated test runs return results in **~14 milliseconds** instead of seconds. The daemon caches SWC transforms, maintains pre-warmed Node workers, and orchestrates parallel test execution while a thin CLI shim forwards commands and renders Jest-style output. Under the hood it uses nng for low-latency IPC, sled for persistent caches, and native SWC for TypeScript/JSX compilation.

## Why it exists

- Traditional Jest startup repeats the same expensive work (loading Node, parsing config, crawling the filesystem) every time an agent or developer runs `npm test`.
- TypeScript and JSX need to be transformed afresh on each invocation when using Babel or `ts-jest`, even if no files changed.
- Jest starts and tears down isolated workers for every test file, reinitializing globals, fake timers, and environments needlessly.
- Pretty watch UIs are nice for humans but add overhead when the caller is an automated agent that only needs structured results.

`rjest` amortizes all of that cost by keeping a daemon in memory and reusing as much work as possible between test runs.

## Architecture at a glance

1. **Rust daemon (`jestd`)**  
   - Parses Jest config once, builds a dependency graph, and watches the filesystem for edits.  
   - Uses SWC to compile TypeScript/JSX and caches the content-hash → compiled-code mapping on disk (via `sled`) and in memory.  
   - Maintains pools of warm Node workers and orchestrates test execution across them.  
   - Streams structured test results (JSON with file/name/duration/error data) back to callers over `async-nng`.
2. **CLI shim (`jest`)**
   - Drop-in replacement for the Jest CLI; supports common flags like patterns, `--runInBand`, `--watch`, `--coverage`, `--bail`, `--maxWorkers`, `--testNamePattern`, and `--json`.
   - Starts the daemon on demand, forwards every invocation as an RPC using `async-nng`, and renders Jest-style output (human-readable or JSON).
   - Can fall back to upstream Jest when a requested feature is not yet supported.
3. **Node-based workers**  
   - Persistent worker processes stay alive across runs, preload a Jest-like runtime, and execute SWC output directly.  
   - Provide familiar globals (`test`, `expect`, fake timers, `jest.fn`, etc.) by reusing Jest runtime libraries or compatible reimplementations.  
   - Avoid re-running Babel/`ts-jest` by consuming cached transforms supplied by the daemon, with task distribution coordinated by `ryv`.

## Compatibility goals

- **CLI & UX:** Mirror the standard Jest CLI for the most common workflows. Unsupported flags emit helpful warnings, and a fallback mode can run upstream Jest in a pinch.
- **Configuration:** Load project configs (package.json, `jest.config.*`, multi-project setups) through Node once, serialize them to JSON, and feed the resolved values to the daemon. Respect `testMatch`, `testRegex`, `roots`, `moduleNameMapper`, `modulePaths`, and other high-signal fields out of the box.
- **Module loading & mocking:** Execute in Node so existing modules, mocks, and runtime assumptions just work. Focus first on manual `jest.mock()` flows, `setupFiles`, `setupFilesAfterEnv`, and CommonJS/ESM interop; expand to advanced automocking later.
- **Snapshots & coverage:** Keep Jest’s snapshot file format and matcher APIs. Instrument coverage via SWC → Istanbul-compatible reports (text, lcov, JSON).
- **Reporting:** Offer a machine-friendly `--json`/`--machine` flag for AI agents while preserving human-readable output for developers.

## Getting started

```bash
# Install as a dev dependency
npm install -D rjest

# Or use npx directly
npx rjest

# Run your tests (same commands as Jest)
npx rjest
npx rjest src/utils.test.ts
npx rjest --watch
npx rjest --coverage

# Filter tests by name pattern
npx rjest --testNamePattern="add"
npx rjest -t "Math"
```

The daemon starts automatically on first run and stays alive to accelerate subsequent invocations. No configuration changes required—`rjest` reads your existing `jest.config.*` files.

### Daemon management

```bash
# Check daemon status
npx rjest --daemon-status

# Stop the daemon manually (caches persist on disk)
npx rjest --daemon-stop

# Force a cold restart
npx rjest --daemon-restart
```

## Drop-in usage

`rjest` is meant to replace Jest without rewrites:

1. Install the CLI alongside your project (e.g., `npm i -D rjest` or run it via `npx`).
2. Run the same commands you already use (`npx jest`, `npm test`, pattern arguments, `--watch`, etc.). The CLI forwards every invocation to the daemon using the same flag semantics.
3. Keep your existing `jest.config.*`, `setupFiles`, `moduleNameMapper`, and snapshot files; the daemon consumes all of them as-is by delegating config resolution to Node.
4. Run multiple repos or terminals simultaneously; a single daemon multiplexes commands from different working directories by mapping each request to its project root, keeping caches and workers namespaced per repo.
5. When you need an unsupported feature, pass `--fallback-to-jest` (or set `RJEST_FALLBACK=1`) to rerun that command with upstream Jest automatically while `rjest` continues to service compatible invocations.

See `docs/compatibility.md` for a detailed matrix of supported flags, config fields, and known limitations plus guidance on when the fallback path activates.

## For AI agents

`rjest` is optimized for automated workflows where fast, structured feedback matters:

```bash
# Machine-readable JSON output
npx rjest --json

# Compact structured output for agents
npx rjest --machine

# Run only tests affected by recent changes
npx rjest --onlyChanged

# Run tests related to specific files
npx rjest --findRelatedTests src/api.ts src/utils.ts
```

### Why agents benefit

- **14ms feedback loops:** Warm runs return results in ~14 milliseconds instead of seconds, enabling rapid edit-test cycles.
- **Structured output:** `--json` and `--machine` flags provide parse-friendly results with file paths, test names, durations, and error details.
- **Selective execution:** Use `--testNamePattern` or file patterns to run only relevant tests, reducing noise and latency.
- **Session continuity:** The daemon maintains state across invocations, so agents don't pay cold-start costs repeatedly.

### Example agent workflow

```bash
# 1. Agent makes code changes
# 2. Run affected tests with machine output
npx rjest --onlyChanged --machine

# Or filter by test name pattern
npx rjest --testNamePattern="authentication" --json

# 3. Parse JSON results
# 4. If failures: read error details, fix code, re-run
# 5. Repeat until green
```

## Technology choices

- **nng (nanomsg-next-gen)** handles the bidirectional messaging between CLI and daemon with low-latency Unix domain sockets.
- **SWC** provides native Rust TypeScript/JSX compilation, 10-100× faster than Babel.
- **sled** stores transform artifacts keyed by content hash (blake3) so caches survive restarts without external services.
- **rayon** parallelizes file transforms across CPU cores.

## Performance expectations

Measured benchmarks on a TypeScript test suite (2 files, 19 tests):

| Metric | rjest | Jest | Speedup |
| --- | --- | --- | --- |
| Cold start | 1.9s | 1.4s | 0.7× |
| Warm run | **14ms** | 1.4s | **100×** |

- **Cold start:** First run spawns workers, warms up V8, parses config, discovers tests, and compiles everything. Cold starts are slightly slower due to daemon initialization overhead.
- **Warm runs:** Once workers are hot and transforms cached, tests return in **under 15 milliseconds**. This is where rjest delivers its largest gains.
- **Memory:** The daemon + 4 workers uses ~200MB total. Idle workers are automatically cleaned up after 60 seconds.

See [BENCHMARK.md](BENCHMARK.md) for detailed benchmark methodology and results.

## Roadmap highlights

- Ship a reliable Node-backed worker pool first; evaluate a custom Rust+JS runtime later for additional gains.
- Harden watch mode, change detection, and selective test execution (e.g., “only run tests affected by these files”).
- Broaden environment support (jsdom, custom environments), mocking modes, and multi-project setups.
- Improve diagnostics with rich JSON payloads, better stack traces, and observable daemon health metrics.
- Build an automated benchmark suite that compares cold/warm run latency against upstream Jest across representative repos, publishing results with each release.
- Maintain compatibility tests that execute Jest’s official suite (and additional regression projects) through both `rjest` and upstream Jest to catch behavioral drift early.
- See `docs/roadmap.md` for the full milestone plan from foundations to ecosystem polish.

With these pieces in place, `rjest` becomes an ideal test runner for AI agents and developers who need tight edit-test cycles on large TypeScript or React codebases.

## Documentation

- [Architecture](docs/architecture.md) – How the daemon, CLI, and workers fit together
- [Compatibility](docs/compatibility.md) – Supported flags, config fields, and fallback behavior
- [Performance](docs/performance.md) – Benchmarks and tuning guidance
- [Roadmap](docs/roadmap.md) – Milestones from MVP to ecosystem polish

## Contributing

Contributions are welcome. Please open an issue to discuss significant changes before submitting a PR.

```bash
# Clone and build
git clone https://github.com/user/rjest.git
cd rjest
cargo build

# Run tests
cargo test
```

## License

MIT
