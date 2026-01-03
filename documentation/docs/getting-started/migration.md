# Migration from Jest

rjest is designed as a drop-in replacement for Jest. Most projects can migrate with minimal changes.

## Compatibility

### Fully Supported

- All standard matchers (`toBe`, `toEqual`, `toContain`, etc.)
- Mock functions (`jest.fn()`, `jest.spyOn()`)
- Snapshot testing
- Async testing (Promises, async/await)
- `describe`, `test`, `it` blocks
- `beforeAll`, `afterAll`, `beforeEach`, `afterEach` hooks
- `--testNamePattern` / `-t` filtering
- JSON output (`--json`)
- TypeScript and JSX out of the box

### Partially Supported

| Feature | Status | Notes |
|---------|--------|-------|
| `jest.mock()` | Basic | Module mocking works, factory functions supported |
| Fake timers | Basic | `useFakeTimers()`, `advanceTimersByTime()` |
| Coverage | Not yet | Code coverage not implemented |
| Watch mode | Basic | File watching works, interactive mode limited |

### Not Yet Supported

- Code coverage (`--coverage`)
- Interactive watch mode commands
- Custom reporters
- `jest.requireActual()`
- Module reset between tests
- `testEnvironment` options (jsdom, etc.)

## Migration Steps

### Step 1: Install rjest

```bash
# Build from source
git clone https://github.com/user/rjest.git
cd rjest
cargo build --release

# Add to PATH
export PATH="$PATH:$(pwd)/target/release"
```

### Step 2: Verify Configuration

rjest reads your existing `jest.config.js`:

```javascript title="jest.config.js"
module.exports = {
  // These options are supported
  testMatch: ['**/*.test.ts', '**/*.test.tsx'],
  moduleFileExtensions: ['ts', 'tsx', 'js', 'jsx', 'json'],
  testPathIgnorePatterns: ['/node_modules/'],

  // These can be removed (built-in to rjest)
  // transform: { ... },  // Not needed - native SWC
  // preset: 'ts-jest',   // Not needed
};
```

### Step 3: Remove Transform Configuration

rjest has built-in TypeScript/JSX support via SWC. Remove these:

```javascript title="jest.config.js"
module.exports = {
  // REMOVE these - not needed with rjest
  // transform: {
  //   '^.+\\.tsx?$': 'ts-jest',
  // },
  // preset: 'ts-jest',
  // globals: {
  //   'ts-jest': { ... }
  // },
};
```

### Step 4: Run Tests

```bash
# Run all tests
jest

# If you see issues, try with verbose output
jest --verbose
```

## Common Migration Issues

### Issue: "Cannot use import statement outside a module"

This error means rjest isn't transforming your TypeScript files.

**Solution**: Ensure the daemon is running with the latest code:

```bash
jest --daemon-stop
jest
```

### Issue: Tests Pass in Jest but Fail in rjest

Check if you're using unsupported features:

```typescript
// These may not work yet
jest.requireActual('./module');
jest.isolateModules(() => { ... });
```

### Issue: Mock Not Working

Ensure you're using supported mock patterns:

```typescript
// Supported
const mockFn = jest.fn();
const mockFn = jest.fn().mockReturnValue(42);
const mockFn = jest.fn().mockResolvedValue('data');

// Supported
jest.spyOn(object, 'method');

// Basic support
jest.mock('./module');
```

### Issue: Snapshot Mismatch

Snapshot format is compatible, but if you see mismatches:

```bash
# Update snapshots
jest -u
```

## Running Both Jest and rjest

During migration, you can run both:

```json title="package.json"
{
  "scripts": {
    "test": "jest",
    "test:fast": "rjest"
  }
}
```

## Performance Comparison

After migration, you should see significant speedups:

| Scenario | Jest | rjest | Speedup |
|----------|------|-------|---------|
| Cold start | ~14s | ~9s | 1.5x |
| Warm run | ~14s | ~100ms | **95x** |

## Getting Help

If you encounter issues:

1. Check [Troubleshooting](../advanced/troubleshooting.md)
2. Run with `RUST_LOG=debug jest` for verbose output
3. File an issue on GitHub
