# Mock Functions

Guide to using mock functions for testing in rjest.

## Creating Mocks

### `jest.fn()`

Create a basic mock function:

```typescript
const mockFn = jest.fn();
mockFn('arg1', 'arg2');

expect(mockFn).toHaveBeenCalled();
expect(mockFn).toHaveBeenCalledWith('arg1', 'arg2');
```

### Mock with Implementation

```typescript
const mockFn = jest.fn((x: number) => x * 2);

expect(mockFn(5)).toBe(10);
expect(mockFn).toHaveBeenCalledWith(5);
```

## Return Values

### `mockReturnValue(value)`

Always return a value:

```typescript
const mock = jest.fn().mockReturnValue(42);

expect(mock()).toBe(42);
expect(mock()).toBe(42);
expect(mock()).toBe(42);
```

### `mockReturnValueOnce(value)`

Return value for next call only:

```typescript
const mock = jest.fn()
  .mockReturnValueOnce(1)
  .mockReturnValueOnce(2)
  .mockReturnValue(0);

expect(mock()).toBe(1);  // First call
expect(mock()).toBe(2);  // Second call
expect(mock()).toBe(0);  // Third and subsequent calls
expect(mock()).toBe(0);
```

### Chaining Return Values

```typescript
const mock = jest.fn()
  .mockReturnValueOnce('first')
  .mockReturnValueOnce('second')
  .mockReturnValueOnce('third');

const results = [mock(), mock(), mock()];
expect(results).toEqual(['first', 'second', 'third']);
```

## Async Mocks

### `mockResolvedValue(value)`

Return a resolved promise:

```typescript
const mockFetch = jest.fn().mockResolvedValue({ data: 'result' });

const result = await mockFetch();
expect(result).toEqual({ data: 'result' });
```

### `mockResolvedValueOnce(value)`

Resolved promise for next call:

```typescript
const mock = jest.fn()
  .mockResolvedValueOnce('first')
  .mockResolvedValueOnce('second');

expect(await mock()).toBe('first');
expect(await mock()).toBe('second');
```

### `mockRejectedValue(error)`

Return a rejected promise:

```typescript
const mockFetch = jest.fn().mockRejectedValue(new Error('Network error'));

await expect(mockFetch()).rejects.toThrow('Network error');
```

### `mockRejectedValueOnce(error)`

Rejected promise for next call:

```typescript
const mock = jest.fn()
  .mockRejectedValueOnce(new Error('First failure'))
  .mockResolvedValue('success');

await expect(mock()).rejects.toThrow('First failure');
expect(await mock()).toBe('success');
```

## Mock Implementations

### `mockImplementation(fn)`

Set implementation:

```typescript
const mock = jest.fn().mockImplementation((a, b) => a + b);

expect(mock(1, 2)).toBe(3);
expect(mock(5, 10)).toBe(15);
```

### `mockImplementationOnce(fn)`

Implementation for next call:

```typescript
const mock = jest.fn()
  .mockImplementationOnce(() => 'first call')
  .mockImplementationOnce(() => 'second call')
  .mockImplementation(() => 'default');

expect(mock()).toBe('first call');
expect(mock()).toBe('second call');
expect(mock()).toBe('default');
```

## Spying on Methods

### `jest.spyOn(object, method)`

Spy on an existing method:

```typescript
const calculator = {
  add: (a: number, b: number) => a + b,
};

const spy = jest.spyOn(calculator, 'add');
calculator.add(1, 2);

expect(spy).toHaveBeenCalledWith(1, 2);
expect(spy).toHaveReturnedWith(3);

spy.mockRestore(); // Restore original
```

### Spying on Built-ins

```typescript
// Spy on console.log
const consoleSpy = jest.spyOn(console, 'log').mockImplementation();

myFunction(); // calls console.log internally

expect(consoleSpy).toHaveBeenCalledWith('expected message');
consoleSpy.mockRestore();
```

### Mocking Return Values on Spies

```typescript
const spy = jest.spyOn(Date, 'now').mockReturnValue(1234567890);

expect(Date.now()).toBe(1234567890);

spy.mockRestore();
```

## Mock State

### Accessing Call Information

```typescript
const mock = jest.fn();
mock('first', 1);
mock('second', 2);
mock('third', 3);

// All calls
expect(mock.mock.calls).toEqual([
  ['first', 1],
  ['second', 2],
  ['third', 3],
]);

// Specific call
expect(mock.mock.calls[0]).toEqual(['first', 1]);

// Number of calls
expect(mock.mock.calls.length).toBe(3);
```

### Accessing Return Values

```typescript
const mock = jest.fn()
  .mockReturnValueOnce(1)
  .mockReturnValueOnce(2);

mock();
mock();

expect(mock.mock.results).toEqual([
  { type: 'return', value: 1 },
  { type: 'return', value: 2 },
]);
```

## Resetting Mocks

### `mockClear()`

Clear call history (keeps implementation):

```typescript
const mock = jest.fn().mockReturnValue(42);
mock();
mock();

expect(mock).toHaveBeenCalledTimes(2);

mock.mockClear();

expect(mock).toHaveBeenCalledTimes(0);
expect(mock()).toBe(42); // Implementation preserved
```

### `mockReset()`

Clear calls and reset implementation:

```typescript
const mock = jest.fn().mockReturnValue(42);
mock();

mock.mockReset();

expect(mock).toHaveBeenCalledTimes(0);
expect(mock()).toBeUndefined(); // No implementation
```

### `mockRestore()`

Restore original (for spies):

```typescript
const spy = jest.spyOn(console, 'log').mockImplementation();
spy.mockRestore();
// console.log is back to normal
```

### Resetting All Mocks

```typescript
afterEach(() => {
  jest.clearAllMocks();  // Clear all mock calls
  // or
  jest.resetAllMocks();  // Reset all mocks
  // or
  jest.restoreAllMocks(); // Restore all spies
});
```

## Module Mocking

### `jest.mock(modulePath)`

Mock an entire module:

```typescript
jest.mock('./api');

import { fetchUser } from './api';

// fetchUser is now a mock function
(fetchUser as jest.Mock).mockResolvedValue({ id: 1, name: 'Alice' });

const user = await fetchUser(1);
expect(user.name).toBe('Alice');
```

### Mock with Factory

```typescript
jest.mock('./api', () => ({
  fetchUser: jest.fn().mockResolvedValue({ id: 1 }),
  fetchPosts: jest.fn().mockResolvedValue([]),
}));
```

### `jest.unmock(modulePath)`

Remove a mock:

```typescript
jest.unmock('./api');
// Module now uses real implementation
```

## Common Patterns

### Testing Callbacks

```typescript
function processItems(items: string[], callback: (item: string) => void) {
  items.forEach(callback);
}

test('calls callback for each item', () => {
  const mockCallback = jest.fn();

  processItems(['a', 'b', 'c'], mockCallback);

  expect(mockCallback).toHaveBeenCalledTimes(3);
  expect(mockCallback).toHaveBeenCalledWith('a');
  expect(mockCallback).toHaveBeenCalledWith('b');
  expect(mockCallback).toHaveBeenCalledWith('c');
});
```

### Testing Event Handlers

```typescript
test('handles click event', () => {
  const handleClick = jest.fn();
  const button = { onClick: handleClick };

  button.onClick({ target: 'button' });

  expect(handleClick).toHaveBeenCalledWith({ target: 'button' });
});
```

### Testing API Calls

```typescript
const mockFetch = jest.fn();
global.fetch = mockFetch;

test('fetches user data', async () => {
  mockFetch.mockResolvedValueOnce({
    json: () => Promise.resolve({ id: 1, name: 'Alice' }),
  });

  const user = await fetchUser(1);

  expect(mockFetch).toHaveBeenCalledWith('/api/users/1');
  expect(user.name).toBe('Alice');
});
```

### Verifying Call Order

```typescript
test('calls functions in order', () => {
  const first = jest.fn();
  const second = jest.fn();
  const third = jest.fn();

  runSequence(first, second, third);

  expect(first.mock.invocationCallOrder[0]).toBeLessThan(
    second.mock.invocationCallOrder[0]
  );
  expect(second.mock.invocationCallOrder[0]).toBeLessThan(
    third.mock.invocationCallOrder[0]
  );
});
```
