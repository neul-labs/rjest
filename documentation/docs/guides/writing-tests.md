# Writing Tests

A comprehensive guide to writing tests with rjest.

## Test File Location

By default, rjest looks for test files matching:

- `**/*.test.ts`
- `**/*.test.tsx`
- `**/*.test.js`
- `**/*.test.jsx`
- `**/__tests__/**/*.ts`

## Basic Test Structure

### Simple Test

```typescript
test('adds 1 + 2 to equal 3', () => {
  expect(1 + 2).toBe(3);
});
```

### Grouped Tests

```typescript
describe('Calculator', () => {
  test('adds numbers', () => {
    expect(add(1, 2)).toBe(3);
  });

  test('subtracts numbers', () => {
    expect(subtract(5, 3)).toBe(2);
  });
});
```

### Nested Groups

```typescript
describe('User', () => {
  describe('validation', () => {
    test('requires email', () => {
      expect(() => validateUser({})).toThrow('Email required');
    });

    test('validates email format', () => {
      expect(() => validateUser({ email: 'invalid' })).toThrow('Invalid email');
    });
  });

  describe('creation', () => {
    test('creates user with defaults', () => {
      const user = createUser({ email: 'test@example.com' });
      expect(user.role).toBe('user');
    });
  });
});
```

## Setup and Teardown

### Per-Test Setup

```typescript
describe('Database', () => {
  let db;

  beforeEach(() => {
    db = new MockDatabase();
    db.seed(testData);
  });

  afterEach(() => {
    db.clear();
  });

  test('finds user by id', () => {
    const user = db.findById(1);
    expect(user.name).toBe('Alice');
  });
});
```

### Per-Suite Setup

```typescript
describe('API Tests', () => {
  let server;

  beforeAll(async () => {
    server = await startTestServer();
  });

  afterAll(async () => {
    await server.close();
  });

  test('GET /users returns list', async () => {
    const response = await fetch(`${server.url}/users`);
    expect(response.status).toBe(200);
  });
});
```

## Testing Functions

### Pure Functions

```typescript
import { capitalize, isEven, add } from './utils';

describe('capitalize', () => {
  test('capitalizes first letter', () => {
    expect(capitalize('hello')).toBe('Hello');
  });

  test('handles empty string', () => {
    expect(capitalize('')).toBe('');
  });

  test('preserves rest of string', () => {
    expect(capitalize('hELLO')).toBe('HELLO');
  });
});
```

### Functions with Side Effects

```typescript
describe('Logger', () => {
  let consoleSpy;

  beforeEach(() => {
    consoleSpy = jest.spyOn(console, 'log').mockImplementation();
  });

  afterEach(() => {
    consoleSpy.mockRestore();
  });

  test('logs messages', () => {
    log('hello');
    expect(consoleSpy).toHaveBeenCalledWith('hello');
  });
});
```

## Testing Classes

```typescript
class Calculator {
  private value = 0;

  add(n: number) {
    this.value += n;
    return this;
  }

  subtract(n: number) {
    this.value -= n;
    return this;
  }

  getResult() {
    return this.value;
  }
}

describe('Calculator', () => {
  let calc: Calculator;

  beforeEach(() => {
    calc = new Calculator();
  });

  test('starts at zero', () => {
    expect(calc.getResult()).toBe(0);
  });

  test('adds numbers', () => {
    calc.add(5).add(3);
    expect(calc.getResult()).toBe(8);
  });

  test('chains operations', () => {
    calc.add(10).subtract(3).add(5);
    expect(calc.getResult()).toBe(12);
  });
});
```

## Testing Error Handling

### Synchronous Errors

```typescript
function divide(a: number, b: number): number {
  if (b === 0) throw new Error('Cannot divide by zero');
  return a / b;
}

describe('divide', () => {
  test('throws on division by zero', () => {
    expect(() => divide(10, 0)).toThrow('Cannot divide by zero');
  });

  test('throws Error instance', () => {
    expect(() => divide(10, 0)).toThrow(Error);
  });

  test('throws matching pattern', () => {
    expect(() => divide(10, 0)).toThrow(/divide by zero/);
  });
});
```

### Async Errors

```typescript
async function fetchUser(id: string) {
  if (!id) throw new Error('ID required');
  // ... fetch logic
}

describe('fetchUser', () => {
  test('rejects without ID', async () => {
    await expect(fetchUser('')).rejects.toThrow('ID required');
  });
});
```

## Testing Edge Cases

```typescript
describe('isEven', () => {
  // Normal cases
  test('returns true for even numbers', () => {
    expect(isEven(2)).toBe(true);
    expect(isEven(4)).toBe(true);
    expect(isEven(100)).toBe(true);
  });

  test('returns false for odd numbers', () => {
    expect(isEven(1)).toBe(false);
    expect(isEven(3)).toBe(false);
  });

  // Edge cases
  test('handles zero', () => {
    expect(isEven(0)).toBe(true);
  });

  test('handles negative numbers', () => {
    expect(isEven(-2)).toBe(true);
    expect(isEven(-3)).toBe(false);
  });

  test('handles floating point', () => {
    expect(isEven(2.5)).toBe(false);
  });

  test('handles special values', () => {
    expect(isEven(NaN)).toBe(false);
    expect(isEven(Infinity)).toBe(false);
  });
});
```

## Skipping and Focusing Tests

### Skip Tests

```typescript
test.skip('not ready yet', () => {
  // This test won't run
});

describe.skip('incomplete feature', () => {
  // All tests in this block are skipped
});
```

### Focus Tests

```typescript
test.only('debug this test', () => {
  // Only this test runs
});

describe.only('focus on this suite', () => {
  // Only tests in this block run
});
```

### Todo Tests

```typescript
test.todo('implement validation');
test.todo('add error handling');
```

## Best Practices

### 1. One Assertion Per Test (Usually)

```typescript
// Good - clear what failed
test('user has correct name', () => {
  expect(user.name).toBe('Alice');
});

test('user has correct email', () => {
  expect(user.email).toBe('alice@example.com');
});

// Also OK - related assertions
test('creates valid user', () => {
  const user = createUser({ name: 'Alice' });
  expect(user.id).toBeDefined();
  expect(user.createdAt).toBeInstanceOf(Date);
});
```

### 2. Descriptive Test Names

```typescript
// Good
test('returns empty array when no users match filter', () => {});
test('throws ValidationError when email is invalid', () => {});

// Bad
test('filter works', () => {});
test('error case', () => {});
```

### 3. Arrange-Act-Assert

```typescript
test('adds item to cart', () => {
  // Arrange
  const cart = new Cart();
  const item = { id: 1, name: 'Widget', price: 10 };

  // Act
  cart.addItem(item);

  // Assert
  expect(cart.items).toContainEqual(item);
  expect(cart.total).toBe(10);
});
```

### 4. Test Behavior, Not Implementation

```typescript
// Good - tests behavior
test('removes item from cart', () => {
  cart.addItem(item);
  cart.removeItem(item.id);
  expect(cart.items).not.toContainEqual(item);
});

// Bad - tests implementation details
test('removes item from internal array', () => {
  cart.addItem(item);
  cart.removeItem(item.id);
  expect(cart._items.length).toBe(0); // Don't test private state
});
```
