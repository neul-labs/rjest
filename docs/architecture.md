# Architecture

`rjest` splits responsibility between a persistent Rust daemon and lightweight clients so repeated test runs reuse as much work as possible. This document describes the major components, how they communicate, and the trade-offs behind key design decisions.

## Process topology

1. **Daemon (`jestd`)**  
   Long-lived Rust process that manages configuration, filesystem state, transforms, and worker pools. A single daemon instance can serve multiple repositories at once by keeping separate namespaces for each project root. Internally it uses `ryv` to schedule asynchronous jobs and `sled` to persist caches.
2. **CLI shim (`jest`)**  
   Thin binary (Node or Rust) that mimics the Jest CLI, translates flags into RPC payloads over `async-nng`, and renders results. It is stateless outside of managing the daemon lifecycle.
3. **Worker pool**  
   A configurable number of persistent Node child processes responsible for executing compiled tests inside a Jest-like runtime environment. Workers are managed entirely by the daemon.

All communication between CLI and daemon occurs over a local IPC channel implemented with `async-nng` (Unix domain socket or named pipe) using JSON-encoded request/response messages. Each RPC includes the calling project’s root path so the daemon can route work to the correct namespace. Worker coordination uses in-process queues inside the daemon scheduled by `ryv`, and results stream back to the CLI through the same IPC channel.

## Daemon responsibilities

### Configuration ingestion

- Loads project configuration (package.json, `jest.config.*`, multi-project arrays) via Node once so arbitrary JavaScript/TypeScript configs are supported.
- Normalizes and serializes the resolved config into JSON for the Rust daemon, eliminating the need for the daemon to evaluate arbitrary user code.
- Tracks relevant settings such as `testMatch`, `testRegex`, `roots`, `projects`, `modulePaths`, `moduleNameMapper`, `setupFiles`, and `setupFilesAfterEnv`.

### Project graph and file watching

- Discovers test files and source dependencies using the resolved config.
- Parses each module with SWC to extract import/export edges and stores them in an incremental dependency graph scoped to the project root that issued the request.
- Subscribes to filesystem notifications per project namespace; when a file changes, the matching graph updates incrementally, and the daemon can determine which tests are affected without cross-project bleed.
- Exposes queries such as “tests affected by <list of files>” and “all tests in project <name>” to drive selective runs.

### Transform and cache management

- Uses SWC for TypeScript, JSX, and JS compilation, plus optional coverage instrumentation via SWC plugins for Istanbul compatibility.
- Computes a content hash per file and caches the resulting compiled code plus sourcemaps both in memory and on disk using `sled`.
- On each request, only re-transforms files that changed since their last cached hash, drastically reducing incremental run time.
- Maintains a separate cache namespace per project to respect differing configs (e.g., module aliases, JSX runtimes).

### Worker orchestration

- Spawns worker pools per project namespace sized according to `--maxWorkers` or configuration defaults so different repos cannot starve one another.
- Workers stay alive between runs; each receives compiled test bundles directly from the daemon rather than reading from disk.
- Preloads a Jest-compatible runtime shim that defines globals (`test`, `it`, `describe`, `beforeEach`, `afterEach`, `expect`, fake timers, mock helpers).
- Tracks worker health, restarts crashed workers, and cleans up leaked resources (timers, mocked modules) between test files.

### Result handling

- Aggregates per-test-case results, console output, coverage summaries, and snapshot status into structured JSON payloads.
- Streams incremental updates back to the CLI via `async-nng` so humans can see progress and agents can parse machine-friendly data as soon as it is available.
- Supports both human-readable summaries and `--json`/`--machine` output modes.

## CLI behavior

- Accepts the same entrypoints as Jest (`jest`, `npx jest`, pattern arguments, config flags).
- On each invocation, ensures the daemon is running (start if needed) and sends an RPC containing the resolved CLI args and optional session metadata.
- Handles unsupported flags by either emitting a warning with details or, if `--fallback-to-jest` is provided, delegating to upstream Jest for that invocation.
- Renders output consistent with Jest’s format (status bar, pass/fail counts, per-test errors) or writes raw JSON to stdout when machine output is requested.

## Execution model within workers

- Each worker loads a lightweight runtime shim that wires Jest APIs to compatible implementations (`@jest/expect`, timer mocks, spies).
- Test modules are executed from the SWC-compiled code provided by the daemon; CommonJS and ESM modules are supported with the same semantics as Jest wherever possible.
- `setupFiles` run once per worker boot, while `setupFilesAfterEnv` run before each test file to mirror Jest behavior.
- Snapshots use the standard Jest snapshot serializer and file format, ensuring snapshot updates remain compatible with existing tooling.

## Optional future direction: custom runtime

The initial release keeps Node workers for maximal compatibility. A future iteration could embed a JavaScript engine (e.g., V8 via `rusty_v8` or QuickJS) directly into the daemon to execute tests without Node. Benefits include lower overhead and tighter integration, but the trade-off is the need to reimplement Node APIs and Jest runtime semantics. Any move toward a custom runtime must preserve practical compatibility with typical React/TypeScript test suites before it can replace Node-backed workers.
