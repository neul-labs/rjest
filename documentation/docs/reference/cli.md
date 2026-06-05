# CLI Reference

Complete reference for `rjest` command-line options. All flags below are
sourced from [`crates/rjest-cli/src/args.rs`][args].

[args]: https://github.com/neul-labs/rjest/blob/main/crates/rjest-cli/src/args.rs

## Basic Usage

```bash
rjest [options] [file-patterns...]

# or, via npx
npx rjest [options] [file-patterns...]

# or, via the jest entry point shim
jest [options] [file-patterns...]
```

The CLI binary is named `jest` (so existing `npx jest`, `npm test`, and
`yarn test` commands continue to work). `rjest` and `jestd` are also installed
as aliases by the npm/Cargo/Homebrew distributions.

## Test Selection

### File patterns

```bash
# Run all tests discovered via testMatch / testRegex
rjest

# Run a specific file
rjest src/utils.test.ts

# Run multiple files
rjest src/utils.test.ts src/api.test.ts

# Shell-expanded glob
rjest src/**/*.test.ts
```

### `--testNamePattern`, `-t`

Filter individual tests by regex on the full test name (including parent
`describe` blocks):

```bash
rjest -t "adds two numbers"
rjest --testNamePattern="add|subtract"
rjest -t "should.*error"
```

### `--onlyChanged`, `-o`

Run only tests affected by files changed since the last git commit. The
daemon resolves "affected" using the in-memory import graph built by SWC.

```bash
rjest --onlyChanged
rjest -o
```

### `--findRelatedTests <files...>`

Run only tests that import the given source files (one-shot equivalent of
`--onlyChanged` for an explicit file list):

```bash
rjest --findRelatedTests src/utils.ts src/api.ts
```

## Execution

### `--runInBand`, `-i`

Run all tests serially in a single worker. Useful for debugging or when
tests share global state.

```bash
rjest --runInBand
rjest -i
```

### `--bail`, `-b`

Exit immediately after the first test failure.

```bash
rjest --bail
rjest -b
```

### `--maxWorkers`, `-w`

Override the number of worker processes (default: capped at 4):

```bash
rjest --maxWorkers=2
rjest -w 4
```

`jestd` enforces a hard ceiling of 4 workers regardless of this flag — see
[Architecture](../advanced/architecture.md#worker-pool).

### `--coverage`

Collect Istanbul-compatible coverage via SWC instrumentation. Coverage is
roadmap status "Planned" in the [compatibility matrix][compat] — for any
project that hits unsupported reporters, combine with `--fallback-to-jest`.

[compat]: https://github.com/neul-labs/rjest/blob/main/docs/compatibility.md

```bash
rjest --coverage
```

### `--updateSnapshot`, `-u`

Update stored snapshots instead of comparing:

```bash
rjest -u
rjest --updateSnapshot
```

## Watch Mode

### `--watch`

Start an interactive watch session. The CLI opens a long-lived watch
session against the daemon, polls every second for filesystem changes, and
re-runs the affected tests. Press `Ctrl+C` to exit.

```bash
rjest --watch
```

### `--watchAll`

Re-run **all** tests on every change instead of only the affected subset.
Currently treated as a synonym of `--watch` at the daemon level (see
[`compatibility.md`][compat]).

```bash
rjest --watchAll
```

## Output

### `--json`

Emit a single JSON document containing the full [`RunResponse`][proto]
structure: pass/fail counts, per-file results, per-test errors, durations,
and snapshot summary.

[proto]: https://github.com/neul-labs/rjest/blob/main/crates/rjest-protocol/src/lib.rs

```bash
rjest --json
```

Example shape (fields from `rjest-protocol`):

```json
{
  "type": "Run",
  "success": true,
  "num_passed_suites": 3,
  "num_failed_suites": 0,
  "num_passed_tests": 136,
  "num_failed_tests": 0,
  "num_skipped_tests": 0,
  "num_todo_tests": 0,
  "duration_ms": 102,
  "test_results": [
    {
      "path": "/abs/path/utils.test.ts",
      "passed": true,
      "duration_ms": 12,
      "tests": [
        { "name": "Math operations > adds two numbers", "status": "passed", "duration_ms": 1, "error": null }
      ],
      "console_output": null
    }
  ],
  "snapshot_summary": null
}
```

### `--machine`

Machine-readable streaming output tuned for AI agents. Like `--json` but
optimized for quick parsing per file as results arrive.

```bash
rjest --machine
rjest --onlyChanged --machine
```

### `--verbose`

Show individual test names instead of just per-file dots:

```bash
rjest --verbose
```

## Configuration

### `--config`, `-c`

Point at a specific Jest config file. Otherwise the daemon walks the
project root looking for `jest.config.js`, `jest.config.ts`,
`jest.config.mjs`, `jest.config.json`, or a `jest` key in `package.json`.

```bash
rjest --config=jest.config.custom.js
rjest -c jest.config.custom.js
```

## Daemon Management

The daemon (`jestd`) is started automatically on the first run and keeps
running in the background. These flags let you inspect or control it
directly.

### `--daemon-status`

Print version, uptime, project count, cache stats, and worker stats. The
fields are sourced from [`StatusResponse`][proto].

```bash
rjest --daemon-status
```

Example output:

```
rjest daemon v0.1.3
Uptime: 312s
Projects tracked: 1

Cache:
  Transforms: 47 (1834291 bytes)
  ...
Workers:
  Active: 0
  Idle: 4
  Total tests run: 408
```

### `--daemon-stop`

Send a `Shutdown` request and wait for the daemon to exit. Transform caches
persist on disk in `sled`, so the next run still benefits from cached
compilation.

```bash
rjest --daemon-stop
```

### `--daemon-restart`

Stop the daemon (best-effort) then run tests as normal — which spawns a
fresh daemon. Useful after upgrading the binary.

```bash
rjest --daemon-restart
```

### `--daemon-health`

Detailed diagnostics: per-worker state machine (`spawning`, `warming_up`,
`idle`, `running`, `recycling`, `terminating`, `dead`), measured response
latency, approximate memory, and detected issues. Fields are sourced from
[`HealthResponse`][proto].

```bash
rjest --daemon-health
```

## Fallback to Upstream Jest

### `--fallback-to-jest`

Skip rjest entirely and invoke `npx jest` with the same flags forwarded.
Useful while migrating, or when a single command needs an unsupported
feature (custom reporters, `jest-environment-jsdom`, custom Babel
transformers).

```bash
rjest --fallback-to-jest
```

The `RJEST_FALLBACK=1` environment variable is equivalent:

```bash
RJEST_FALLBACK=1 rjest
```

## Environment Variables

### `RUST_LOG`

Enable structured logging via the `tracing` crate:

```bash
RUST_LOG=info rjest
RUST_LOG=debug rjest
RUST_LOG=jestd=debug rjest                 # only daemon spans
RUST_LOG=jestd::transform=debug rjest      # only SWC transform spans
```

### `RJEST_FALLBACK`

Set to any non-empty value to force fallback to upstream Jest for the
invocation (equivalent to `--fallback-to-jest`).

### `XDG_RUNTIME_DIR`

On Unix, the daemon socket is placed at `$XDG_RUNTIME_DIR/rjest-<uid>.sock`
if set, otherwise `/tmp/rjest-<uid>.sock`. On Windows a named pipe
`\\.\pipe\rjest-<username>` is used.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0    | All tests passed |
| 1    | One or more tests failed |
| Non-zero (other) | Daemon, config, transform, or worker error |

Daemon-side errors map to the [`ErrorCode`][proto] variants:
`config_error`, `no_tests_found`, `transform_error`, `worker_error`,
`internal_error`, `invalid_request`.

## Examples

### CI / CD pipeline

```bash
# Structured output for downstream tooling
rjest --json > test-results.json

# Fail fast on first failure
rjest --bail

# Coverage with a stop-the-daemon at the end to free RAM
rjest --coverage && rjest --daemon-stop
```

### Local development

```bash
# First run — daemon spawns automatically (~9s cold)
rjest

# Edit a file, run again — ~100ms warm
rjest

# Focus while debugging
rjest -t "handles edge case" --verbose

# Watch mode
rjest --watch

# When you're done for the day
rjest --daemon-stop
```

### AI agent workflows

```bash
# Smallest possible feedback loop: only affected tests, machine output
rjest --onlyChanged --machine

# Targeted by source file
rjest --findRelatedTests src/auth.ts --machine

# Targeted by name
rjest --testNamePattern="authentication" --json
```

### Debugging the daemon itself

```bash
# Verbose tracing
RUST_LOG=debug rjest --verbose

# Health check
rjest --daemon-health

# Hard reset (clear caches, kill stale processes)
pkill -f jestd
rm -f /tmp/rjest-*.sock ~/.cache/rjest
rjest
```
