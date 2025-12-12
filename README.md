# rjest

`rjest` is a Jest-compatible test runner that keeps a long-lived Rust daemon in the background so repeated `npm test` runs return results in seconds instead of tens of seconds. The daemon caches transforms, file graphs, and worker processes while a thin CLI shim forwards every user command to the daemon and renders Jest-style output. Under the hood it relies on `async-nng` for low-latency CLI↔daemon communication, `sled` for durable caches, and `ryv` for concurrent task scheduling.

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
   - Drop-in replacement for the Jest CLI; supports common flags like patterns, `--runInBand`, `--watch`, `--coverage`, `--bail`, `--maxWorkers`, and `--json`.  
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

## Drop-in usage

`rjest` is meant to replace Jest without rewrites:

1. Install the CLI alongside your project (e.g., `npm i -D rjest` or run it via `npx`).
2. Run the same commands you already use (`npx jest`, `npm test`, pattern arguments, `--watch`, etc.). The CLI forwards every invocation to the daemon using the same flag semantics.
3. Keep your existing `jest.config.*`, `setupFiles`, `moduleNameMapper`, and snapshot files; the daemon consumes all of them as-is by delegating config resolution to Node.
4. Run multiple repos or terminals simultaneously; a single daemon multiplexes commands from different working directories by mapping each request to its project root, keeping caches and workers namespaced per repo.
5. When you need an unsupported feature, pass `--fallback-to-jest` (or set `RJEST_FALLBACK=1`) to rerun that command with upstream Jest automatically while `rjest` continues to service compatible invocations.

See `docs/compatibility.md` for a detailed matrix of supported flags, config fields, and known limitations plus guidance on when the fallback path activates.

## Technology choices

- `async-nng` handles the bidirectional messaging between CLI and daemon with low-latency, backpressure-aware sockets.
- `sled` stores transform artifacts, dependency metadata, and daemon bookkeeping so caches survive restarts without external services.
- `ryv` coordinates asynchronous tasks inside the daemon (file watching, transform pipelines, worker scheduling) while keeping latency predictable.

## Performance expectations

- **Cold start:** First run still has to parse config, discover tests, compile everything once, and spin up workers. Expect a modest 1.2–2× speedup over classic Jest primarily from SWC transforms and tighter orchestration.
- **Warm incremental runs:** Re-running an individual test file after editing nearby code typically drops from 5–15 seconds to 0.5–3 seconds because only changed files are recompiled and workers stay hot (≈5–10× faster).
- **Warm full-suite runs:** When most files are unchanged, repeated `npm test` runs take roughly half the time (≈2–3× faster) because the daemon reuses cached transforms, a prebuilt graph, and persistent workers.
- **Zero-change reruns:** If nothing changed between runs, execution time approaches the pure cost of the tests themselves because the daemon simply redispatches to idle workers (often 2–5× faster than re-running Jest cold).

## Roadmap highlights

- Ship a reliable Node-backed worker pool first; evaluate a custom Rust+JS runtime later for additional gains.
- Harden watch mode, change detection, and selective test execution (e.g., “only run tests affected by these files”).
- Broaden environment support (jsdom, custom environments), mocking modes, and multi-project setups.
- Improve diagnostics with rich JSON payloads, better stack traces, and observable daemon health metrics.

With these pieces in place, `rjest` becomes an ideal test runner for AI agents and developers who need tight edit-test cycles on large TypeScript or React codebases.
