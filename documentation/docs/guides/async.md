# Async Testing

Guide to testing asynchronous code in rjest.

## Async/Await

The most straightforward way to test async code:

```typescript
test('fetches user data', async () => {
  const user = await fetchUser(1);
  expect(user.name).toBe('Alice');
});
```

### Multiple Awaits

```typescript
test('processes data in order', async () => {
  const data = await fetchData();
  const processed = await processData(data);
  const saved = await saveData(processed);

  expect(saved.success).toBe(true);
});
```

### Parallel Awaits

```typescript
test('fetches multiple resources', async () => {
  const [users, posts] = await Promise.all([
    fetchUsers(),
    fetchPosts(),
  ]);

  expect(users).toHaveLength(10);
  expect(posts).toHaveLength(25);
});
```

## Resolves and Rejects

### `resolves`

Test that a promise resolves:

```typescript
test('resolves with value', async () => {
  await expect(Promise.resolve(42)).resolves.toBe(42);
});

test('resolves with object', async () => {
  await expect(fetchUser(1)).resolves.toEqual({
    id: 1,
    name: 'Alice',
  });
});

test('resolves and matches', async () => {
  await expect(fetchUser(1)).resolves.toHaveProperty('name');
});
```

### `rejects`

Test that a promise rejects:

```typescript
test('rejects with error', async () => {
  await expect(failingOperation()).rejects.toThrow('Operation failed');
});

test('rejects with error type', async () => {
  await expect(failingOperation()).rejects.toBeInstanceOf(Error);
});

test('rejects with matching message', async () => {
  await expect(failingOperation()).rejects.toThrow(/failed/i);
});
```

## Error Handling

### Testing Rejected Promises

```typescript
async function fetchUser(id: string): Promise<User> {
  if (!id) {
    throw new Error('ID is required');
  }
  // ... fetch logic
}

test('throws when ID is missing', async () => {
  await expect(fetchUser('')).rejects.toThrow('ID is required');
});
```

### Try/Catch Pattern

```typescript
test('handles errors correctly', async () => {
  try {
    await riskyOperation();
    fail('Expected error was not thrown');
  } catch (error) {
    expect(error.message).toContain('expected error');
  }
});
```

## Testing Callbacks

### Using Promises

Convert callback-based APIs to promises:

```typescript
function fetchWithCallback(url: string, callback: (err: Error | null, data?: any) => void) {
  // ...
}

test('fetches data', async () => {
  const result = await new Promise((resolve, reject) => {
    fetchWithCallback('/api/data', (err, data) => {
      if (err) reject(err);
      else resolve(data);
    });
  });

  expect(result).toEqual({ success: true });
});
```

### Testing Error Callbacks

```typescript
test('calls error callback', async () => {
  await expect(new Promise((resolve, reject) => {
    fetchWithCallback('/invalid', (err, data) => {
      if (err) reject(err);
      else resolve(data);
    });
  })).rejects.toThrow();
});
```

## Timeouts

### Custom Test Timeout

```typescript
// Override default timeout for slow operations
test('slow operation completes', async () => {
  const result = await verySlowOperation();
  expect(result).toBeDefined();
}, 30000); // 30 seconds
```

### Testing Timeouts

```typescript
test('times out correctly', async () => {
  const promise = new Promise((resolve) => {
    setTimeout(resolve, 100);
  });

  await expect(promise).resolves.toBeUndefined();
});
```

## Fake Timers

### Basic Usage

```typescript
test('uses fake timers', () => {
  jest.useFakeTimers();

  const callback = jest.fn();
  setTimeout(callback, 1000);

  // Time hasn't passed yet
  expect(callback).not.toHaveBeenCalled();

  // Fast-forward time
  jest.advanceTimersByTime(1000);

  expect(callback).toHaveBeenCalled();

  jest.useRealTimers();
});
```

### Running All Timers

```typescript
test('runs all timers', () => {
  jest.useFakeTimers();

  const callback = jest.fn();
  setTimeout(callback, 1000);
  setTimeout(callback, 2000);
  setTimeout(callback, 3000);

  jest.runAllTimers();

  expect(callback).toHaveBeenCalledTimes(3);

  jest.useRealTimers();
});
```

### Testing Intervals

```typescript
test('interval fires multiple times', () => {
  jest.useFakeTimers();

  const callback = jest.fn();
  setInterval(callback, 1000);

  jest.advanceTimersByTime(3500);

  expect(callback).toHaveBeenCalledTimes(3);

  jest.useRealTimers();
});
```

## Testing Debounced Functions

```typescript
function debounce(fn: Function, delay: number) {
  let timeoutId: NodeJS.Timeout;
  return (...args: any[]) => {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => fn(...args), delay);
  };
}

test('debounces function calls', () => {
  jest.useFakeTimers();

  const fn = jest.fn();
  const debouncedFn = debounce(fn, 1000);

  // Call multiple times in quick succession
  debouncedFn('a');
  debouncedFn('b');
  debouncedFn('c');

  // Function not called yet
  expect(fn).not.toHaveBeenCalled();

  // Fast-forward past debounce delay
  jest.advanceTimersByTime(1000);

  // Only called once with last argument
  expect(fn).toHaveBeenCalledTimes(1);
  expect(fn).toHaveBeenCalledWith('c');

  jest.useRealTimers();
});
```

## Testing Polling

```typescript
async function pollUntilReady(checkFn: () => Promise<boolean>, maxAttempts = 10): Promise<void> {
  for (let i = 0; i < maxAttempts; i++) {
    if (await checkFn()) return;
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw new Error('Polling timeout');
}

test('polls until condition is met', async () => {
  jest.useFakeTimers();

  let attempts = 0;
  const checkFn = jest.fn(async () => {
    attempts++;
    return attempts >= 3;
  });

  const pollPromise = pollUntilReady(checkFn);

  // Advance through polling intervals
  await jest.advanceTimersByTimeAsync(300);

  await pollPromise;

  expect(checkFn).toHaveBeenCalledTimes(3);

  jest.useRealTimers();
});
```

## Best Practices

### Always Await Assertions

```typescript
// Good
test('async test', async () => {
  await expect(asyncFn()).resolves.toBe('value');
});

// Bad - test might pass before promise settles
test('async test', () => {
  expect(asyncFn()).resolves.toBe('value');
});
```

### Clean Up After Tests

```typescript
describe('async tests', () => {
  afterEach(() => {
    jest.useRealTimers();
    jest.clearAllMocks();
  });

  test('uses fake timers', () => {
    jest.useFakeTimers();
    // ...
  });
});
```

### Handle Unhandled Rejections

```typescript
test('handles rejection', async () => {
  // Use try/catch or expect().rejects
  await expect(riskyOperation()).rejects.toThrow();

  // Don't leave floating promises
  // Bad: riskyOperation(); // Might reject after test completes
});
```
