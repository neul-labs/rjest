# Performance

Guide to getting the best performance from rjest.

## Benchmark Results

| Metric | rjest | Jest | Improvement |
|--------|-------|------|-------------|
| Cold start (136 tests) | 9,200ms | 14,200ms | 1.5× faster |
| Warm run (136 tests) | ~100ms | 14,200ms | **95× faster** |
| Daemon memory | 17MB | — | — |
| Total memory | 212MB | 108MB | +96% |

## Understanding Cold vs Warm

### Cold Start

First run after daemon stop:

1. Daemon spawns and initializes
2. Worker pool starts
3. Configuration loads
4. TypeScript files compile
5. Tests execute

**Time: ~9 seconds** for 136 tests

### Warm Run

Subsequent runs with daemon active:

1. CLI connects to daemon
2. Send test request
3. Cached transforms used
4. Warm workers execute
5. Results return

**Time: ~100ms** for 136 tests

## Optimizing Performance

### Keep the Daemon Running

The daemon is the key to performance. Keep it running during development:

```bash
# First run starts daemon
jest

# Subsequent runs are instant
jest  # ~100ms
jest  # ~100ms
jest  # ~100ms
```

### Use Watch Mode

Watch mode keeps everything warm:

```bash
jest --watch
```

### Optimize Test Discovery

Limit test discovery scope:

```javascript
// jest.config.js
module.exports = {
  // Only look in specific directories
  roots: ['<rootDir>/src'],

  // Specific patterns
  testMatch: ['**/*.test.ts'],

  // Ignore large directories
  testPathIgnorePatterns: [
    '/node_modules/',
    '/dist/',
    '/build/',
  ],
};
```

### Run Specific Tests

During development, run only relevant tests:

```bash
# Single file
jest src/utils.test.ts

# By name pattern
jest -t "handles errors"
```

## Memory Management

### Memory Usage Breakdown

```
Component           Memory
─────────────────────────
Daemon              ~17 MB
Worker 1            ~50 MB
Worker 2            ~50 MB
Worker 3            ~50 MB
Worker 4            ~50 MB
─────────────────────────
Total              ~217 MB
```

### Reducing Memory

#### Stop When Not Testing

```bash
# Free all memory
jest --daemon-stop
```

#### Reduce Worker Count

```bash
# Use fewer workers
jest --maxWorkers=2
```

#### Idle Cleanup

Workers automatically stop after 60 seconds of inactivity.

### Memory vs Speed Trade-off

| Workers | Memory | Parallel Speed |
|---------|--------|----------------|
| 1 | ~67 MB | Slowest |
| 2 | ~117 MB | Faster |
| 4 | ~217 MB | Fastest |

## Profiling

### Debug Logging

```bash
# Enable debug output
RUST_LOG=debug jest

# Trace level
RUST_LOG=trace jest
```

### Timing Breakdown

```bash
# Get timing details
RUST_LOG=rjest=debug jest 2>&1 | grep -E "(ms|duration)"
```

### Daemon Status

```bash
jest --daemon-status
```

Output:
```
Daemon Status:
  Running: true
  PID: 12345
  Uptime: 3600s
  Projects: 1
  Workers: 4
  Cached transforms: 150
```

## Performance Tips

### 1. Avoid Unnecessary Transforms

rjest caches by content hash, but large files still take time on first compile:

```typescript
// Avoid: Large auto-generated files in test scope
// Better: Keep generated code out of testMatch patterns
```

### 2. Use TypeScript Project References

For monorepos, use project references to scope compilation:

```json
{
  "compilerOptions": {
    "composite": true
  },
  "references": [
    { "path": "../shared" }
  ]
}
```

### 3. Optimize Jest Configuration

```javascript
module.exports = {
  // Cache transform results (rjest does this automatically)
  // but helps with config parsing
  cache: true,

  // Don't transform node_modules
  transformIgnorePatterns: ['/node_modules/'],
};
```

### 4. Parallel Test Files

rjest runs test files in parallel across workers:

```
worker-1: auth.test.ts, cart.test.ts
worker-2: user.test.ts, product.test.ts
worker-3: api.test.ts, utils.test.ts
worker-4: integration.test.ts
```

More test files = better parallelization.

### 5. Avoid Heavy Setup

```typescript
// Slow: Heavy setup in each test file
beforeAll(async () => {
  await setupDatabase();
  await seedTestData();
});

// Better: Light setup, mock heavy operations
beforeAll(() => {
  mockDatabase();
});
```

## Benchmarking

### Run Benchmarks

```bash
./benchmarks/run.sh
```

### Manual Benchmarking

```bash
# Stop daemon for cold start test
jest --daemon-stop

# Time cold start
time jest

# Time warm run
time jest
```

### Compare with Jest

```bash
# rjest warm run
jest --daemon-stop && jest && time jest

# Jest (every run is cold)
time npx jest
```

## When Performance Differs

### Fewer Tests

With very few tests (<10), the overhead difference is smaller:

| Tests | rjest (warm) | Jest |
|-------|--------------|------|
| 5 | 50ms | 2s |
| 50 | 80ms | 5s |
| 500 | 200ms | 30s |

### First Run

First run includes daemon startup. Subsequent runs are faster.

### Changed Files

When source files change, transforms are recomputed:

- Unchanged files: Cache hit (instant)
- Changed files: Recompile (~10ms per file with SWC)

### Large Files

Very large TypeScript files take longer to transform:

| File Size | Transform Time |
|-----------|----------------|
| 1 KB | ~1ms |
| 10 KB | ~5ms |
| 100 KB | ~50ms |
| 1 MB | ~500ms |

Transform results are cached, so subsequent runs are instant.
