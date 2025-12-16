# Build Plan

This document details the implementation plan for Phase 0 (scaffolding) and Phase 1 (MVP daemon + CLI).

## Workspace Structure

```
rjest/
├── Cargo.toml              # Workspace root
├── Cargo.lock
├── README.md
├── LICENSE
├── docs/
│   ├── architecture.md
│   ├── compatibility.md
│   ├── performance.md
│   ├── roadmap.md
│   └── build-plan.md
├── crates/
│   ├── jestd/              # Daemon crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── lib.rs
│   │       ├── config/     # Jest config parsing
│   │       ├── graph/      # Dependency graph
│   │       ├── transform/  # SWC transform pipeline
│   │       ├── cache/      # Sled-backed caching
│   │       ├── worker/     # Node worker pool management
│   │       ├── rpc/        # async-nng server
│   │       └── scheduler/  # ryv task scheduling
│   ├── rjest-cli/          # CLI shim crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── args.rs     # CLI argument parsing
│   │       ├── client.rs   # async-nng client
│   │       ├── output.rs   # Jest-style output rendering
│   │       └── fallback.rs # Upstream Jest fallback
│   ├── rjest-protocol/     # Shared RPC types
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs      # Request/Response types
│   └── rjest-runtime/      # Node worker runtime (JS)
│       ├── package.json
│       └── src/
│           ├── index.js    # Worker entry point
│           ├── globals.js  # test/describe/expect shims
│           └── ipc.js      # Worker ↔ daemon communication
└── tests/
    ├── fixtures/           # Sample projects for testing
    │   ├── basic-ts/
    │   ├── react-app/
    │   └── monorepo/
    └── integration/        # End-to-end tests
```

## Phase 0 Deliverables

### 0.1 – Initialize Cargo workspace

```toml
# Cargo.toml (workspace root)
[workspace]
resolver = "2"
members = [
    "crates/jestd",
    "crates/rjest-cli",
    "crates/rjest-protocol",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
repository = "https://github.com/user/rjest"

[workspace.dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# IPC
nng = "1"

# Caching
sled = "0.34"

# Transforms
swc_core = "0.90"
swc_ecma_parser = "0.143"
swc_ecma_transforms = "0.229"
swc_ecma_codegen = "0.148"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# CLI
clap = { version = "4", features = ["derive"] }

# Error handling
anyhow = "1"
thiserror = "1"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"
```

### 0.2 – Scaffold crate stubs

Create minimal `Cargo.toml` and placeholder `src/main.rs` or `src/lib.rs` for each crate so the workspace compiles.

### 0.3 – Define RPC protocol

```rust
// crates/rjest-protocol/src/lib.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RunRequest {
    pub project_root: String,
    pub patterns: Vec<String>,
    pub flags: RunFlags,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunFlags {
    pub run_in_band: bool,
    pub watch: bool,
    pub bail: bool,
    pub json_output: bool,
    pub machine_output: bool,
    pub max_workers: Option<u32>,
    pub config_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunResponse {
    pub success: bool,
    pub num_passed: u32,
    pub num_failed: u32,
    pub num_skipped: u32,
    pub duration_ms: u64,
    pub results: Vec<TestFileResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestFileResult {
    pub path: String,
    pub tests: Vec<TestResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub status: TestStatus,
    pub duration_ms: u64,
    pub error: Option<TestError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
    Todo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestError {
    pub message: String,
    pub stack: Option<String>,
    pub diff: Option<String>,
}
```

### 0.4 – Bootstrap CLI argument parsing

```rust
// crates/rjest-cli/src/args.rs
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "rjest", about = "Fast Jest-compatible test runner")]
pub struct Args {
    /// Test file patterns
    #[arg(trailing_var_arg = true)]
    pub patterns: Vec<String>,

    /// Run tests serially
    #[arg(short = 'i', long)]
    pub run_in_band: bool,

    /// Watch for changes
    #[arg(long)]
    pub watch: bool,

    /// Exit after first failure
    #[arg(short, long)]
    pub bail: bool,

    /// Output JSON results
    #[arg(long)]
    pub json: bool,

    /// Machine-readable output for agents
    #[arg(long)]
    pub machine: bool,

    /// Max worker processes
    #[arg(short = 'w', long)]
    pub max_workers: Option<u32>,

    /// Path to Jest config
    #[arg(short, long)]
    pub config: Option<String>,

    /// Force upstream Jest fallback
    #[arg(long)]
    pub fallback_to_jest: bool,

    /// Show daemon status
    #[arg(long)]
    pub daemon_status: bool,

    /// Stop the daemon
    #[arg(long)]
    pub daemon_stop: bool,
}
```

---

## Phase 1 Implementation Steps

### 1.1 – Daemon lifecycle management

**Goal:** CLI can start/stop the daemon and verify it's running.

- Daemon listens on Unix socket at `$XDG_RUNTIME_DIR/rjest.sock` or `/tmp/rjest-$UID.sock`
- CLI checks if socket exists and daemon responds to ping
- If not running, CLI spawns daemon as background process
- Implement `--daemon-status`, `--daemon-stop` commands

### 1.2 – Basic RPC round-trip

**Goal:** CLI sends a `RunRequest`, daemon echoes it back.

- Use `nng` request/reply pattern
- JSON-serialize protocol types
- Add timeout handling (5s default)
- Test with a simple ping/pong

### 1.3 – Jest config loading via Node

**Goal:** Daemon can resolve Jest configuration.

- Spawn a Node subprocess to load `jest.config.js` / `jest.config.ts` / `package.json`
- Node script outputs normalized JSON config
- Parse into Rust structs
- Cache resolved config per project root

**Node config loader (simplified):**

```javascript
// scripts/load-config.js
const { readConfig } = require('jest-config');

async function main() {
  const { projectConfig } = await readConfig(
    {}, // argv
    process.cwd()
  );
  console.log(JSON.stringify(projectConfig));
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
```

### 1.4 – Test file discovery

**Goal:** Daemon finds test files matching config patterns.

- Parse `testMatch` and `testRegex` from config
- Walk `roots` directories
- Apply `testPathIgnorePatterns`
- Return list of test file paths

### 1.5 – SWC transform pipeline

**Goal:** Compile TypeScript/JSX to JavaScript with caching.

- Content-hash each source file
- Check sled cache for existing transform
- If miss: parse with SWC, transform, generate code + sourcemap
- Store result in sled
- Return compiled code to worker

### 1.6 – Node worker pool

**Goal:** Spawn and manage persistent Node worker processes.

- Spawn N workers (based on `maxWorkers` or CPU count)
- Workers load the runtime shim (`rjest-runtime`)
- IPC via stdio (JSON lines) or Unix socket
- Workers receive test file + compiled code
- Workers execute and return results
- Handle worker crashes and restarts

### 1.7 – Test execution flow

**Goal:** End-to-end test run with real results.

1. CLI sends `RunRequest` to daemon
2. Daemon loads config (cached)
3. Daemon discovers test files (cached graph)
4. Daemon transforms files (cached)
5. Daemon dispatches to worker pool
6. Workers execute tests, stream results
7. Daemon aggregates results
8. Daemon sends `RunResponse` to CLI
9. CLI renders output (human or JSON)

### 1.8 – Jest-style output rendering

**Goal:** CLI output matches Jest's familiar format.

```
 PASS  src/utils.test.ts
 FAIL  src/api.test.ts
  ● API > should fetch data

    expect(received).toBe(expected)

    Expected: 200
    Received: 404

      12 |   const res = await fetch('/api');
      13 |   expect(res.status).toBe(200);
         |                      ^

Test Suites: 1 failed, 1 passed, 2 total
Tests:       1 failed, 3 passed, 4 total
Time:        1.234 s
```

### 1.9 – Fallback to upstream Jest

**Goal:** Graceful degradation for unsupported features.

- Detect unsupported flags at CLI parse time
- Detect unsupported config fields after loading
- If `--fallback-to-jest` or `RJEST_FALLBACK=1`: spawn `npx jest` with original args
- Log which feature triggered fallback

### 1.10 – Smoke tests

**Goal:** Verify basic functionality works.

- Create `tests/fixtures/basic-ts/` with simple TS test
- Run via `rjest` and `jest`, compare output
- Assert pass/fail counts match
- Run in CI on each commit

---

## Implementation Order

```
Phase 0:
  [0.1] Cargo workspace setup
  [0.2] Crate scaffolding
  [0.3] Protocol types
  [0.4] CLI argument parsing

Phase 1 (sequential dependencies):
  [1.1] Daemon lifecycle ─────┐
  [1.2] RPC round-trip ───────┤
                              ▼
  [1.3] Config loading ───────┐
  [1.4] Test discovery ───────┤
                              ▼
  [1.5] SWC transforms ───────┐
  [1.6] Worker pool ──────────┤
                              ▼
  [1.7] Execution flow ───────┐
  [1.8] Output rendering ─────┤
  [1.9] Fallback logic ───────┤
                              ▼
  [1.10] Smoke tests
```

## Success Criteria for Phase 1

- [ ] `rjest` CLI starts daemon automatically on first run
- [ ] Daemon persists between invocations
- [ ] Jest config (JS/TS/JSON) is loaded correctly
- [ ] Test files are discovered based on `testMatch`
- [ ] TypeScript/JSX files are transformed via SWC
- [ ] Transforms are cached in sled
- [ ] Tests execute in Node worker pool
- [ ] Results stream back with pass/fail status
- [ ] CLI renders Jest-compatible output
- [ ] `--json` flag produces valid JSON output
- [ ] `--fallback-to-jest` invokes upstream Jest
- [ ] Smoke test passes for basic TS project
