# Troubleshooting

Solutions to common issues with rjest.

## Common Errors

### "Cannot use import statement outside a module"

**Cause**: TypeScript/ESM not being transformed to CommonJS.

**Solution**:

1. Stop and restart the daemon:
```bash
jest --daemon-stop
jest
```

2. Clear the transform cache:
```bash
rm -rf ~/.cache/rjest
jest
```

3. Ensure your file has a TypeScript extension (`.ts`, `.tsx`).

### "Daemon is not running"

**Cause**: The daemon hasn't started or crashed.

**Solution**:

1. Start the daemon by running any test:
```bash
jest
```

2. Check for port conflicts:
```bash
# See if something else is using the socket
ls -la /tmp/rjest-*
```

3. Check daemon logs:
```bash
RUST_LOG=debug jest
```

### "Connection refused" or Socket Errors

**Cause**: Daemon died or socket file is stale.

**Solution**:

1. Kill any stale processes:
```bash
pkill -f jestd
```

2. Remove stale socket files:
```bash
rm -f /tmp/rjest-*
```

3. Restart:
```bash
jest
```

### Tests Pass in Jest but Fail in rjest

**Cause**: Using unsupported features.

**Check for**:

1. `jest.requireActual()` - Not supported
2. `testEnvironment: 'jsdom'` - Not supported
3. Custom reporters - Not supported
4. `globalSetup/globalTeardown` - Not supported

**Solution**: Check the [Migration Guide](../getting-started/migration.md) for workarounds.

### Mock Not Working

**Cause**: Mock not set up correctly.

**Check**:

```typescript
// Ensure mock is created before use
const mockFn = jest.fn().mockReturnValue(42);

// Ensure you're checking the mock, not original
expect(mockFn).toHaveBeenCalled(); // ✓
expect(originalFn).toHaveBeenCalled(); // ✗ Wrong!
```

**For module mocks**:

```typescript
// Mock must be called before import
jest.mock('./module');

// Then import
import { fn } from './module';
```

### Snapshot Mismatch

**Cause**: Output changed or snapshot is stale.

**Solution**:

1. Review the diff carefully
2. If change is intentional:
```bash
jest -u
```

3. If change is unintentional, fix your code.

### "expect(...).toSomeMatcher is not a function"

**Cause**: Matcher not implemented in rjest.

**Check available matchers** in [Matchers Guide](../guides/matchers.md).

**Workaround**: Use available matchers:

```typescript
// Instead of: expect(x).toBeWithinRange(1, 10)
expect(x).toBeGreaterThanOrEqual(1);
expect(x).toBeLessThanOrEqual(10);
```

### Tests Timeout

**Cause**: Async operation taking too long.

**Solutions**:

1. Increase timeout:
```typescript
test('slow test', async () => {
  // ...
}, 30000); // 30 seconds
```

2. Check for unresolved promises:
```typescript
// Bad: Floating promise
test('async', () => {
  fetchData(); // Not awaited!
});

// Good: Await the promise
test('async', async () => {
  await fetchData();
});
```

3. Check for infinite loops in mocks.

## Daemon Issues

### Daemon Won't Start

**Check logs**:
```bash
RUST_LOG=debug jest 2>&1 | head -50
```

**Common causes**:

1. Port already in use
2. Insufficient permissions
3. Missing Node.js

**Solutions**:
```bash
# Check Node.js is available
which node
node --version

# Check for zombie processes
ps aux | grep jest
pkill -f jestd

# Try again
jest
```

### Daemon Crashes

**Check for errors**:
```bash
RUST_LOG=trace jest
```

**Common causes**:

1. Out of memory
2. Invalid configuration
3. Corrupt cache

**Solutions**:
```bash
# Clear cache
rm -rf ~/.cache/rjest

# Reduce workers
jest --maxWorkers=1
```

### Daemon Uses Too Much Memory

**Solution**: Stop when not testing:
```bash
jest --daemon-stop
```

**Or reduce workers**:
```bash
jest --maxWorkers=2
```

## Configuration Issues

### Config Not Found

**Check file exists**:
```bash
ls jest.config.*
```

**Check format**:
```javascript
// jest.config.js
module.exports = {
  // Must use CommonJS exports
};
```

### Config Changes Not Applied

**Solution**: Restart daemon:
```bash
jest --daemon-stop
jest
```

### TypeScript Config Not Working

Ensure you're exporting correctly:

```typescript
// jest.config.ts
import type { Config } from 'jest';

const config: Config = {
  testMatch: ['**/*.test.ts'],
};

export default config;
```

## Debug Mode

### Enable Verbose Logging

```bash
# Info level
RUST_LOG=info jest

# Debug level
RUST_LOG=debug jest

# Trace level (very verbose)
RUST_LOG=trace jest
```

### Log Specific Components

```bash
# Only daemon logs
RUST_LOG=jestd=debug jest

# Only transform logs
RUST_LOG=jestd::transform=debug jest
```

### Save Logs to File

```bash
RUST_LOG=debug jest 2>&1 | tee rjest.log
```

## Getting Help

### Check Daemon Status

```bash
jest --daemon-status
```

### Version Information

```bash
jest --version
```

### File an Issue

When filing an issue, include:

1. rjest version (`jest --version`)
2. Node.js version (`node --version`)
3. Operating system
4. Minimal reproduction
5. Debug logs (`RUST_LOG=debug jest`)

### Quick Reset

If all else fails:

```bash
# Kill all rjest processes
pkill -f jestd
pkill -f rjest

# Clear all caches
rm -rf ~/.cache/rjest

# Clear transform cache
rm -rf /tmp/rjest-*

# Start fresh
jest
```
