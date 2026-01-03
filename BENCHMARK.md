# rjest Benchmarks

Benchmarks comparing rjest against upstream Jest on a TypeScript test suite.

## Test Suite

- **Files**: 3 test files (`utils.test.ts`, `snapshot.test.ts`, `fuzzy.test.ts`)
- **Tests**: 136 individual tests
- **Features**: TypeScript, snapshot testing, mocks, async tests
- **Location**: `tests/fixtures/basic-ts`

## Results Summary

| Metric | rjest | Jest | Improvement |
|--------|-------|------|-------------|
| Cold start | 9,200ms | 14,200ms | 1.5× faster |
| Warm run | 150ms | 14,200ms | **95× faster** |
| Daemon memory | 17MB | — | — |
| Total memory (daemon + workers) | 212MB | 108MB | +96% |

## Detailed Results

### Timing

```
Cold Start (daemon startup + first test run)
------------------------------------------------
  Run 1: 9220ms
  Run 2: 9530ms
  Run 3: 8597ms
  Run 4: 9167ms
  Run 5: 9409ms
  Average: 9185ms

Warm Run (daemon + workers already hot)
------------------------------------------------
  Run 1: 101ms
  Run 2: 96ms
  Run 3: 124ms
  Run 4: 82ms
  Run 5: 95ms
  Average: 100ms (can vary 80-200ms)

Upstream Jest
------------------------------------------------
  Run 1: 13698ms
  Run 2: 14041ms
  Run 3: 13554ms
  Run 4: 14402ms
  Run 5: 15117ms
  Average: 14162ms
```

### Memory

```
rjest:
  Daemon RSS:        ~17 MB
  Total (+ workers): ~212 MB

Jest:
  Peak RSS:          ~108 MB
```

## Key Takeaways

### Speed

- **95× faster** on warm runs (~150ms vs ~14,200ms)
- Cold starts are now faster than Jest (9.2s vs 14.2s)
- Warm runs complete in **under 200 milliseconds** for 136 tests

### Memory Trade-off

rjest uses more memory (~212MB) than a single Jest run (~108MB) because:
- The daemon process stays resident (~17MB)
- 4 Node worker processes are kept warm (~50MB each)
- Transform caches are held in memory

This trade-off enables the 95× speedup on subsequent runs.

### Memory Management

To reduce memory when not actively testing:
- Workers idle for 60+ seconds are automatically killed
- Run `jest --daemon-stop` to fully shut down
- Transform caches persist on disk, so restarts don't lose compilation work

## Why rjest is Faster

1. **No repeated bootstrap**: Jest reloads Node, parses config, and crawls the filesystem on every run. rjest does this once.

2. **Cached transforms**: TypeScript/JSX compilation results are cached by content hash. Unchanged files never recompile.

3. **Pre-warmed workers**: Node workers stay alive with V8 already JIT-compiled, eliminating process startup latency.

4. **Native SWC**: TypeScript compilation uses Rust-native SWC instead of Node-based Babel (10-100× faster compilation).

5. **Low-latency IPC**: CLI-to-daemon communication uses nng Unix domain sockets instead of spawning processes.

## Running Benchmarks

```bash
# Run the full benchmark suite
./benchmarks/run.sh

# Or manually:
# Cold start
jest --daemon-stop
time jest

# Warm run (run twice, measure second)
jest  # warmup
time jest  # measure
```

## Environment

- **Platform**: Linux 6.14.0-37-generic
- **Date**: 2026-01-03
- **Build**: Release (optimized)
- **Tests**: 136 (3 test files)
