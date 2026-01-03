# Jest Globals

Reference for all global functions and objects available in tests.

## Test Structure

### `describe(name, fn)`

Create a test suite:

```typescript
describe('Math operations', () => {
  // tests go here
});
```

Nested describes:

```typescript
describe('Calculator', () => {
  describe('add', () => {
    test('adds positive numbers', () => {});
  });

  describe('subtract', () => {
    test('subtracts numbers', () => {});
  });
});
```

### `test(name, fn, timeout?)`

Define a test:

```typescript
test('adds 1 + 2 to equal 3', () => {
  expect(1 + 2).toBe(3);
});

// With custom timeout (milliseconds)
test('slow operation', async () => {
  await slowOperation();
}, 10000);
```

### `it(name, fn, timeout?)`

Alias for `test`:

```typescript
it('should add numbers', () => {
  expect(1 + 2).toBe(3);
});
```

### `test.skip(name, fn)` / `it.skip(name, fn)`

Skip a test:

```typescript
test.skip('not implemented yet', () => {
  // This test won't run
});
```

### `test.only(name, fn)` / `it.only(name, fn)`

Run only this test:

```typescript
test.only('focus on this test', () => {
  // Only this test runs
});
```

### `test.todo(name)`

Mark a test as todo:

```typescript
test.todo('implement error handling');
```

## Lifecycle Hooks

### `beforeAll(fn, timeout?)`

Run once before all tests in a describe block:

```typescript
describe('database tests', () => {
  beforeAll(async () => {
    await db.connect();
  });

  // tests...
});
```

### `afterAll(fn, timeout?)`

Run once after all tests in a describe block:

```typescript
describe('database tests', () => {
  afterAll(async () => {
    await db.disconnect();
  });

  // tests...
});
```

### `beforeEach(fn, timeout?)`

Run before each test:

```typescript
describe('user tests', () => {
  let user;

  beforeEach(() => {
    user = createTestUser();
  });

  test('user has name', () => {
    expect(user.name).toBeDefined();
  });
});
```

### `afterEach(fn, timeout?)`

Run after each test:

```typescript
describe('file tests', () => {
  afterEach(() => {
    cleanup();
  });

  // tests...
});
```

## Expect

### `expect(value)`

Create an expectation:

```typescript
expect(2 + 2).toBe(4);
expect({ a: 1 }).toEqual({ a: 1 });
```

See [Matchers Guide](../guides/matchers.md) for all available matchers.

## Jest Object

### `jest.fn(implementation?)`

Create a mock function:

```typescript
const mockFn = jest.fn();
const mockFn = jest.fn(() => 42);
const mockFn = jest.fn((x) => x * 2);
```

### `jest.spyOn(object, method)`

Spy on an object method:

```typescript
const spy = jest.spyOn(console, 'log');
console.log('hello');
expect(spy).toHaveBeenCalledWith('hello');
spy.mockRestore();
```

### `jest.mock(moduleName, factory?)`

Mock a module:

```typescript
jest.mock('./utils', () => ({
  calculate: jest.fn(() => 42),
}));
```

### `jest.unmock(moduleName)`

Remove a module mock:

```typescript
jest.unmock('./utils');
```

### `jest.useFakeTimers()`

Enable fake timers:

```typescript
jest.useFakeTimers();

setTimeout(() => callback(), 1000);
jest.advanceTimersByTime(1000);

expect(callback).toHaveBeenCalled();
```

### `jest.useRealTimers()`

Restore real timers:

```typescript
jest.useRealTimers();
```

### `jest.advanceTimersByTime(ms)`

Advance fake timers:

```typescript
jest.advanceTimersByTime(1000); // Advance 1 second
```

### `jest.runAllTimers()`

Run all pending timers:

```typescript
jest.runAllTimers();
```

### `jest.clearAllMocks()`

Clear all mock call history:

```typescript
jest.clearAllMocks();
```

### `jest.resetAllMocks()`

Reset all mocks to initial state:

```typescript
jest.resetAllMocks();
```

### `jest.restoreAllMocks()`

Restore all mocks to original implementations:

```typescript
jest.restoreAllMocks();
```

## Mock Function Methods

### `mockFn.mockReturnValue(value)`

Set return value:

```typescript
const mock = jest.fn().mockReturnValue(42);
expect(mock()).toBe(42);
```

### `mockFn.mockReturnValueOnce(value)`

Set return value for next call only:

```typescript
const mock = jest.fn()
  .mockReturnValueOnce(1)
  .mockReturnValueOnce(2)
  .mockReturnValue(3);

expect(mock()).toBe(1);
expect(mock()).toBe(2);
expect(mock()).toBe(3);
expect(mock()).toBe(3);
```

### `mockFn.mockResolvedValue(value)`

Return a resolved promise:

```typescript
const mock = jest.fn().mockResolvedValue('data');
await expect(mock()).resolves.toBe('data');
```

### `mockFn.mockRejectedValue(error)`

Return a rejected promise:

```typescript
const mock = jest.fn().mockRejectedValue(new Error('fail'));
await expect(mock()).rejects.toThrow('fail');
```

### `mockFn.mockImplementation(fn)`

Set mock implementation:

```typescript
const mock = jest.fn().mockImplementation((x) => x * 2);
expect(mock(5)).toBe(10);
```

### `mockFn.mockClear()`

Clear call history:

```typescript
mock();
mock();
expect(mock).toHaveBeenCalledTimes(2);
mock.mockClear();
expect(mock).toHaveBeenCalledTimes(0);
```

### `mockFn.mockReset()`

Clear calls and reset return value:

```typescript
mock.mockReset();
```

### `mockFn.mockRestore()`

Restore original (for spies):

```typescript
const spy = jest.spyOn(obj, 'method');
spy.mockRestore();
```

### `mockFn.mock.calls`

Array of call arguments:

```typescript
mock('a', 'b');
mock('c');
console.log(mock.mock.calls);
// [['a', 'b'], ['c']]
```

### `mockFn.mock.results`

Array of return values:

```typescript
mock.mockReturnValue(42);
mock();
console.log(mock.mock.results);
// [{ type: 'return', value: 42 }]
```
