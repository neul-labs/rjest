# Matchers

Complete reference for all available matchers in rjest.

## Basic Matchers

### `toBe(value)`

Strict equality using `Object.is`:

```typescript
expect(2 + 2).toBe(4);
expect('hello').toBe('hello');
expect(true).toBe(true);

// Note: Objects compare by reference
const obj = { a: 1 };
expect(obj).toBe(obj); // passes
expect({ a: 1 }).toBe({ a: 1 }); // fails!
```

### `toEqual(value)`

Deep equality for objects and arrays:

```typescript
expect({ a: 1, b: 2 }).toEqual({ a: 1, b: 2 });
expect([1, 2, 3]).toEqual([1, 2, 3]);
expect({ a: { b: { c: 1 } } }).toEqual({ a: { b: { c: 1 } } });
```

### `toStrictEqual(value)`

Strict deep equality (checks `undefined` properties):

```typescript
expect({ a: 1 }).toStrictEqual({ a: 1 });
expect({ a: undefined }).not.toStrictEqual({}); // different!
```

## Truthiness

### `toBeTruthy()`

Value is truthy:

```typescript
expect(1).toBeTruthy();
expect('hello').toBeTruthy();
expect([]).toBeTruthy();
expect({}).toBeTruthy();
```

### `toBeFalsy()`

Value is falsy:

```typescript
expect(0).toBeFalsy();
expect('').toBeFalsy();
expect(null).toBeFalsy();
expect(undefined).toBeFalsy();
expect(NaN).toBeFalsy();
```

### `toBeNull()`

Value is `null`:

```typescript
expect(null).toBeNull();
expect(undefined).not.toBeNull();
```

### `toBeUndefined()`

Value is `undefined`:

```typescript
expect(undefined).toBeUndefined();
expect(null).not.toBeUndefined();
```

### `toBeDefined()`

Value is not `undefined`:

```typescript
expect(1).toBeDefined();
expect(null).toBeDefined();
expect(undefined).not.toBeDefined();
```

### `toBeNaN()`

Value is `NaN`:

```typescript
expect(NaN).toBeNaN();
expect(0 / 0).toBeNaN();
expect(parseFloat('not a number')).toBeNaN();
```

## Numbers

### `toBeGreaterThan(number)`

```typescript
expect(10).toBeGreaterThan(5);
expect(0.1 + 0.2).toBeGreaterThan(0.3);
```

### `toBeGreaterThanOrEqual(number)`

```typescript
expect(10).toBeGreaterThanOrEqual(10);
expect(10).toBeGreaterThanOrEqual(5);
```

### `toBeLessThan(number)`

```typescript
expect(5).toBeLessThan(10);
```

### `toBeLessThanOrEqual(number)`

```typescript
expect(5).toBeLessThanOrEqual(5);
expect(5).toBeLessThanOrEqual(10);
```

### `toBeCloseTo(number, precision?)`

Floating point comparison:

```typescript
expect(0.1 + 0.2).toBeCloseTo(0.3);
expect(0.1 + 0.2).toBeCloseTo(0.3, 5); // 5 decimal places
```

## Strings

### `toMatch(regexp | string)`

String matches pattern:

```typescript
expect('hello world').toMatch('world');
expect('hello world').toMatch(/world/);
expect('hello@example.com').toMatch(/^[\w.]+@[\w.]+$/);
```

### `toHaveLength(number)`

String (or array) has length:

```typescript
expect('hello').toHaveLength(5);
expect('').toHaveLength(0);
```

## Arrays

### `toContain(item)`

Array contains item (uses `===`):

```typescript
expect([1, 2, 3]).toContain(2);
expect(['a', 'b', 'c']).toContain('b');
```

### `toContainEqual(item)`

Array contains item with deep equality:

```typescript
expect([{ a: 1 }, { b: 2 }]).toContainEqual({ a: 1 });
```

### `toHaveLength(number)`

Array has length:

```typescript
expect([1, 2, 3]).toHaveLength(3);
expect([]).toHaveLength(0);
```

## Objects

### `toHaveProperty(path, value?)`

Object has property:

```typescript
// Check property exists
expect({ a: 1 }).toHaveProperty('a');

// Check nested property
expect({ a: { b: 1 } }).toHaveProperty('a.b');

// Check property value
expect({ a: 1 }).toHaveProperty('a', 1);

// Check array index
expect({ items: [1, 2, 3] }).toHaveProperty('items.1', 2);
```

### `toBeInstanceOf(Class)`

Value is instance of class:

```typescript
expect(new Date()).toBeInstanceOf(Date);
expect([]).toBeInstanceOf(Array);
expect(new Error()).toBeInstanceOf(Error);
```

### `toMatchObject(object)`

Object contains subset:

```typescript
expect({ a: 1, b: 2, c: 3 }).toMatchObject({ a: 1, b: 2 });
```

## Errors

### `toThrow(error?)`

Function throws:

```typescript
// Any error
expect(() => { throw new Error(); }).toThrow();

// Error message contains string
expect(() => { throw new Error('Invalid input'); }).toThrow('Invalid');

// Error message matches regex
expect(() => { throw new Error('Error code: 404'); }).toThrow(/\d+/);

// Error is instance of class
expect(() => { throw new TypeError(); }).toThrow(TypeError);
```

## Mock Functions

### `toHaveBeenCalled()`

Mock was called:

```typescript
const mock = jest.fn();
mock();
expect(mock).toHaveBeenCalled();
```

### `toHaveBeenCalledTimes(number)`

Mock was called N times:

```typescript
const mock = jest.fn();
mock();
mock();
expect(mock).toHaveBeenCalledTimes(2);
```

### `toHaveBeenCalledWith(arg1, arg2, ...)`

Mock was called with arguments:

```typescript
const mock = jest.fn();
mock('hello', 123);
expect(mock).toHaveBeenCalledWith('hello', 123);
```

### `toHaveBeenLastCalledWith(arg1, arg2, ...)`

Last call had arguments:

```typescript
const mock = jest.fn();
mock('first');
mock('second');
expect(mock).toHaveBeenLastCalledWith('second');
```

### `toHaveReturned()`

Mock returned (didn't throw):

```typescript
const mock = jest.fn(() => 42);
mock();
expect(mock).toHaveReturned();
```

### `toHaveReturnedWith(value)`

Mock returned specific value:

```typescript
const mock = jest.fn(() => 42);
mock();
expect(mock).toHaveReturnedWith(42);
```

## Async Matchers

### `resolves`

Promise resolves:

```typescript
await expect(Promise.resolve(42)).resolves.toBe(42);
await expect(fetchData()).resolves.toEqual({ id: 1 });
```

### `rejects`

Promise rejects:

```typescript
await expect(Promise.reject(new Error('fail'))).rejects.toThrow('fail');
await expect(failingOperation()).rejects.toBeInstanceOf(Error);
```

## Negation

### `.not`

Invert any matcher:

```typescript
expect(1).not.toBe(2);
expect([1, 2]).not.toContain(3);
expect({ a: 1 }).not.toHaveProperty('b');
expect(() => {}).not.toThrow();
```

## Snapshot Matchers

### `toMatchSnapshot(hint?)`

Match against stored snapshot:

```typescript
expect({ user: 'Alice', role: 'admin' }).toMatchSnapshot();
expect(render(<Component />)).toMatchSnapshot('component render');
```

### `toMatchInlineSnapshot(snapshot?)`

Match against inline snapshot:

```typescript
expect({ a: 1 }).toMatchInlineSnapshot(`
  Object {
    "a": 1,
  }
`);
```

## Common Patterns

### Testing Objects

```typescript
const user = { id: 1, name: 'Alice', email: 'alice@example.com' };

expect(user).toEqual({
  id: expect.any(Number),
  name: 'Alice',
  email: expect.stringMatching(/@/),
});
```

### Testing Arrays

```typescript
const items = [1, 2, 3, 4, 5];

expect(items).toHaveLength(5);
expect(items).toContain(3);
expect(items[0]).toBe(1);
expect(items).toEqual(expect.arrayContaining([1, 3, 5]));
```

### Testing Errors

```typescript
function validateEmail(email: string) {
  if (!email.includes('@')) {
    throw new Error('Invalid email format');
  }
}

expect(() => validateEmail('invalid')).toThrow('Invalid email');
expect(() => validateEmail('valid@email.com')).not.toThrow();
```
