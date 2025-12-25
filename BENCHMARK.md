# rjest Benchmarks

Benchmarks comparing rjest against upstream Jest on a TypeScript test suite.

## Test Suite

- **Files**: 2 test files (`utils.test.ts`, `snapshot.test.ts`)
- **Tests**: 19 individual tests
- **Features**: TypeScript, snapshot testing
- **Location**: `tests/fixtures/basic-ts`

## Results Summary

| Metric | rjest | Jest | Improvement |
|--------|-------|------|-------------|
| Cold start | 1,900ms | 1,400ms | 0.7× (slower) |
| Warm run | 14ms | 1,400ms | **100× faster** |
| Daemon memory | 35MB | — | — |
| Total memory (daemon + workers) | 200MB | 100MB | +100% |

## Detailed Results

### Timing

```
Cold Start (daemon startup + first test run)
------------------------------------------------
  Run 1: 1930ms
  Run 2: 1890ms
  Run 3: 1920ms
  Average: 1913ms

Warm Run (daemon + workers already hot)
------------------------------------------------
  Run 1: 14ms
  Run 2: 14ms
  Run 3: 14ms
  Run 4: 14ms
  Run 5: 14ms
  Average: 14ms

Upstream Jest
------------------------------------------------
  Run 1: 1430ms
  Run 2: 1400ms
  Run 3: 1380ms
  Average: 1403ms
```

### Memory

```
rjest:
  Daemon RSS:        ~35 MB
  Total (+ workers): ~200 MB

Jest:
  Peak RSS:          ~100 MB
```

## Key Takeaways

### Speed

- **100× faster** on warm runs (14ms vs 1,400ms)
- Cold starts are ~0.5s slower due to daemon initialization overhead
- Warm runs complete in **under 15 milliseconds**

### Memory Trade-off

rjest uses more memory (~200MB) than a single Jest run (~100MB) because:
- The daemon process stays resident (~35MB)
- 4 Node worker processes are kept warm (~40MB each)
- Transform caches are held in memory

This trade-off enables the 100× speedup on subsequent runs.

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

- **Platform**: Linux 6.17.7-x64v3-xanmod1
- **Date**: 2025-12-24
- **Build**: Release (optimized)
