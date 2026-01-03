# Quick Start

Get up and running with rjest in 5 minutes.

## Create a Test File

Create a simple test file to verify rjest is working:

```typescript title="src/math.test.ts"
describe('Math operations', () => {
  test('adds two numbers', () => {
    expect(1 + 2).toBe(3);
  });

  test('subtracts two numbers', () => {
    expect(5 - 3).toBe(2);
  });

  test('multiplies two numbers', () => {
    expect(4 * 3).toBe(12);
  });
});
```

## Run Your Tests

```bash
# Run all tests
jest

# Run specific file
jest src/math.test.ts

# Run with pattern matching
jest --testNamePattern="adds"
```

## Expected Output

```
 PASS  src/math.test.ts

Test Suites: 1 passed, 1 total
Tests:       3 passed, 3 total
Time:        0.015s
```

## Common Options

### Watch Mode

```bash
# Re-run tests when files change
jest --watch
```

### Verbose Output

```bash
# Show individual test names
jest --verbose
```

### Run Specific Tests

```bash
# By file path
jest src/utils.test.ts

# By test name pattern
jest -t "should handle errors"

# Multiple patterns
jest -t "add|subtract"
```

### Update Snapshots

```bash
# Update all snapshots
jest -u
jest --updateSnapshot
```

## JSON Output

For CI/CD integration:

```bash
# Output results as JSON
jest --json

# Save to file
jest --json --outputFile=results.json
```

## Writing More Tests

### Testing Functions

```typescript title="src/utils.ts"
export function capitalize(str: string): string {
  if (!str) return '';
  return str.charAt(0).toUpperCase() + str.slice(1);
}
```

```typescript title="src/utils.test.ts"
import { capitalize } from './utils';

describe('capitalize', () => {
  test('capitalizes first letter', () => {
    expect(capitalize('hello')).toBe('Hello');
  });

  test('handles empty string', () => {
    expect(capitalize('')).toBe('');
  });

  test('handles already capitalized', () => {
    expect(capitalize('Hello')).toBe('Hello');
  });
});
```

### Testing Async Code

```typescript title="src/api.test.ts"
describe('async operations', () => {
  test('resolves with data', async () => {
    const result = await Promise.resolve({ id: 1 });
    expect(result.id).toBe(1);
  });

  test('using resolves matcher', async () => {
    await expect(Promise.resolve(42)).resolves.toBe(42);
  });

  test('using rejects matcher', async () => {
    await expect(Promise.reject(new Error('fail'))).rejects.toThrow('fail');
  });
});
```

### Testing with Mocks

```typescript title="src/service.test.ts"
describe('mock functions', () => {
  test('tracks calls', () => {
    const mockFn = jest.fn();
    mockFn('arg1', 'arg2');

    expect(mockFn).toHaveBeenCalled();
    expect(mockFn).toHaveBeenCalledWith('arg1', 'arg2');
  });

  test('returns mock values', () => {
    const mockFn = jest.fn().mockReturnValue(42);
    expect(mockFn()).toBe(42);
  });
});
```

## Performance Tips

### Warm Runs

The first run starts the daemon (~9 seconds). Subsequent runs are nearly instant (~100ms):

```bash
# First run - cold start
jest  # ~9 seconds

# Second run - warm
jest  # ~100 milliseconds (95x faster!)
```

### Stop Daemon When Done

To free memory when you're done testing:

```bash
jest --daemon-stop
```

## Next Steps

- [Matchers Reference](../guides/matchers.md) - All available matchers
- [Mock Functions](../guides/mocks.md) - Mocking and spying
- [Snapshot Testing](../guides/snapshots.md) - Snapshot testing guide
- [CLI Reference](../reference/cli.md) - All command-line options
