# Performance Guide

This guide captures realistic expectations for how much faster `rjest` can be compared to traditional Jest workflows. The daemon uses native SWC transforms, persistent caching, and a pool of pre-warmed Node workers to minimize latency.

## Benchmark results

Measured on a test suite with 2 test files containing 19 tests total (TypeScript with snapshot tests):

| Metric | rjest | Jest | Speedup |
| --- | --- | --- | --- |
| Cold start | 1.9s | 1.4s | 0.7× |
| Warm run | **14ms** | 1.4s | **100×** |
| Memory (daemon + workers) | 200MB | 100MB peak | — |

The warm run speedup is the key advantage: once the daemon and workers are warmed up, test results return in **~14 milliseconds**.

See [BENCHMARK.md](../BENCHMARK.md) for detailed benchmark methodology and raw results.

## Cold vs. warm runs

- **Cold daemon run:** First invocation after starting the daemon must spawn workers, warm up their V8 runtimes, parse config, discover tests, and compile everything. Cold starts are ~0.5s slower than Jest due to daemon initialization overhead.
- **Warm daemon run:** Once caches are populated and workers are hot, subsequent runs only recompile files whose content hash changed. Workers stay alive with V8 already JIT-compiled, yielding **100× speedups** (14ms vs 1.4s).

## Scenario estimates

| Scenario | Jest today | `rjest` warm | Expected gain |
| --- | --- | --- | --- |
| **A. Small test suite (2 files, 19 tests)** | 1.4s | 14ms | **100× faster** |
| **B. Iterating on a single test file** | 1–3s (rebootstrap, retransform) | 10–50ms (only changed files recompiled) | **30–100× faster** |
| **C. Large suite, most files unchanged** | 30–60s each time | 1–5s (parallel execution, cached transforms) | **10–30× faster** |

These numbers assume TypeScript/React applications where transform cost and Jest bootstrap dominate run time. The speedup is most dramatic for small-to-medium test suites where Jest's startup overhead is the bottleneck.

## Why repeated runs get faster

1. **No repeated bootstrap:** Loading Jest, parsing config, and crawling the filesystem happens once. Later runs simply reuse the in-memory graph.
2. **Cached transforms:** SWC outputs are keyed by content hash, so editing one module re-emits just that module and its dependents.
3. **Persistent workers:** Node workers remain live, keeping fake timers, expect libraries, and module state ready, which eliminates process launch latency.
4. **Targeted execution:** The daemon can run “tests affected by these files” or “last failures only,” letting agents avoid rerunning entire suites unnecessarily.
5. **Purpose-built tooling:** `async-nng` keeps CLI↔daemon messaging latency low, `sled` makes cache lookups and writes effectively instant even across restarts, and `ryv` coordinates asynchronous tasks so transform, watch, and scheduling work never block one another.

## Optimizations implemented

### Native SWC transforms
TypeScript and JSX are compiled using Rust-native SWC instead of Node-based Babel or ts-jest. This provides:
- Faster compilation (10-100× faster than Babel)
- No Node process spawning for transforms
- Parallelized compilation using rayon

### Two-tier transform cache
Transform results are cached at two levels:
1. **In-memory LRU cache** (1000 entries) for hot files
2. **On-disk sled database** for persistence across daemon restarts

Cache keys are content hashes (blake3), so unchanged files never recompile.

### Worker pool management
Workers are persistent Node processes that stay alive across test runs:
- **Pre-warming**: Workers receive a warmup request on startup to pre-initialize V8, load core modules, and JIT-compile common code paths
- **Dynamic scaling**: Limited to 4 workers maximum to balance parallelism vs. memory
- **Idle timeout**: Workers unused for 60 seconds are killed to free memory
- **Automatic recycling**: Workers are respawned after 1000 tests to prevent memory bloat

### Low-latency IPC
CLI-to-daemon communication uses nng (nanomsg-next-gen) sockets:
- Unix domain sockets for minimal latency
- Binary-efficient message passing
- Request-reply pattern with JSON payloads

### Parallel test execution
Test files run in parallel across the worker pool:
- Work-stealing job queue distributes tests to available workers
- Arc-wrapped shared state avoids unnecessary cloning
- Results collected in original order for consistent output

## Memory management

The daemon trades memory for speed. Typical memory usage:
- **Daemon process**: ~35-40MB (Rust binary + caches)
- **Per worker**: ~40-50MB (Node + Jest runtime)
- **Total with 4 workers**: ~200MB

To reduce memory usage:
- Stop the daemon when not actively testing: `jest --daemon-stop`
- Idle workers are automatically cleaned up after 60 seconds of inactivity
- Transform caches persist on disk, so restarting the daemon doesn't lose compilation work

## Guidance for users and agents

- Expect the **first** run to feel similar to a fresh Jest execution; measure gains on the second and third runs.
- Favor targeted commands (`jest path/to/test`, `--onlyChanged`, "tests affected by edited files") to exploit the dependency graph.
- Use machine-readable output (`--json` or `--machine`) when integrating with AI agents so they can quickly respond to per-test results without parsing human-oriented logs.
- Keep the daemon alive during active development or automated refactors; shutting it down discards caches and forfeits the speed advantage.

## Running benchmarks

The project includes a benchmark script to measure performance:

```bash
# Run the full benchmark suite
./benchmarks/run.sh
```

This measures:
1. **Cold start**: Time to start daemon + run tests
2. **Warm run**: Time to run tests with warmed daemon
3. **Memory usage**: Daemon and worker RSS
4. **Jest comparison**: Side-by-side timing vs upstream Jest
