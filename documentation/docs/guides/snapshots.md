# Snapshot Testing

Guide to using snapshot testing in rjest.

## What is Snapshot Testing?

Snapshot testing captures the output of a function or component and compares it against a stored reference. If the output changes, the test fails, prompting you to review the change.

```typescript
test('renders correctly', () => {
  const output = render(<Button label="Click me" />);
  expect(output).toMatchSnapshot();
});
```

## Basic Usage

### Creating Snapshots

On first run, rjest creates a snapshot file:

```typescript
test('formats user data', () => {
  const user = formatUser({ id: 1, name: 'Alice', email: 'alice@example.com' });
  expect(user).toMatchSnapshot();
});
```

This creates `__snapshots__/yourtest.test.ts.snap`:

```javascript
exports[`formats user data 1`] = `
{
  "displayName": "Alice",
  "email": "alice@example.com",
  "id": 1
}
`;
```

### Updating Snapshots

When intentional changes occur, update snapshots:

```bash
# Update all snapshots
jest -u
jest --updateSnapshot

# Update snapshots for specific file
jest -u src/user.test.ts
```

## Snapshot Matchers

### `toMatchSnapshot(hint?)`

Match against stored snapshot:

```typescript
test('user object', () => {
  expect(createUser()).toMatchSnapshot();
});

// With hint for multiple snapshots
test('multiple snapshots', () => {
  expect(createAdmin()).toMatchSnapshot('admin user');
  expect(createGuest()).toMatchSnapshot('guest user');
});
```

### `toMatchInlineSnapshot(snapshot?)`

Store snapshot inline in test file:

```typescript
test('simple value', () => {
  expect({ a: 1, b: 2 }).toMatchInlineSnapshot(`
    {
      "a": 1,
      "b": 2
    }
  `);
});
```

## What to Snapshot

### Good Candidates

- Serializable data structures
- Formatted output (JSON, strings)
- Configuration objects
- Component render output
- API response shapes

```typescript
// JSON data
test('API response shape', () => {
  const response = createApiResponse(userData);
  expect(response).toMatchSnapshot();
});

// Formatted strings
test('error message format', () => {
  const error = formatError(new ValidationError('Invalid email'));
  expect(error).toMatchSnapshot();
});

// Complex objects
test('configuration defaults', () => {
  const config = loadDefaultConfig();
  expect(config).toMatchSnapshot();
});
```

### Poor Candidates

- Randomly generated data
- Timestamps
- Large data sets
- Frequently changing data

```typescript
// Bad - includes timestamp
test('created user', () => {
  const user = createUser();
  expect(user).toMatchSnapshot(); // Will fail every time!
});

// Better - exclude dynamic fields
test('created user', () => {
  const user = createUser();
  expect({
    ...user,
    createdAt: expect.any(Date),
    id: expect.any(String),
  }).toMatchSnapshot();
});
```

## Snapshot Files

### Location

Snapshots are stored in `__snapshots__` directories:

```
src/
  __snapshots__/
    user.test.ts.snap
  user.test.ts
  user.ts
```

### Format

Snapshot files are JavaScript:

```javascript
// Jest Snapshot v1, https://goo.gl/fbAQLP

exports[`test name 1`] = `
{
  "key": "value"
}
`;

exports[`test name 2`] = `
"string value"
`;
```

## Best Practices

### Keep Snapshots Small

```typescript
// Good - focused snapshot
test('user display name', () => {
  expect(user.displayName).toMatchSnapshot();
});

// Avoid - entire object may have irrelevant changes
test('user object', () => {
  expect(entireUserObject).toMatchSnapshot();
});
```

### Use Descriptive Test Names

```typescript
// Good - clear what's being tested
test('formats phone number for US locale', () => {
  expect(formatPhone('+14155551234')).toMatchSnapshot();
});

// Bad - unclear
test('phone', () => {
  expect(formatPhone('+14155551234')).toMatchSnapshot();
});
```

### Review Snapshot Changes

When snapshots change, carefully review the diff:

```diff
- "status": "active"
+ "status": "verified"
```

Ask yourself:
- Is this change intentional?
- Does it reflect a bug or a feature?
- Should the test be updated or fixed?

### Commit Snapshots

Snapshots should be committed to version control:

```gitignore
# Don't ignore snapshots
# !**/__snapshots__/
```

## Multiple Snapshots

### In Same Test

```typescript
test('user lifecycle', () => {
  const pending = createPendingUser();
  expect(pending).toMatchSnapshot('pending state');

  const active = activateUser(pending);
  expect(active).toMatchSnapshot('active state');

  const suspended = suspendUser(active);
  expect(suspended).toMatchSnapshot('suspended state');
});
```

### Numbered Snapshots

When no hint is provided, snapshots are numbered:

```typescript
test('multiple values', () => {
  expect(value1).toMatchSnapshot(); // "multiple values 1"
  expect(value2).toMatchSnapshot(); // "multiple values 2"
  expect(value3).toMatchSnapshot(); // "multiple values 3"
});
```

## Handling Dynamic Data

### Using Property Matchers

```typescript
test('user with dynamic fields', () => {
  const user = createUser();
  expect(user).toMatchSnapshot({
    id: expect.any(String),
    createdAt: expect.any(Date),
    email: expect.stringMatching(/@example\.com$/),
  });
});
```

### Normalizing Data

```typescript
function normalizeForSnapshot(data) {
  return {
    ...data,
    id: '[ID]',
    createdAt: '[TIMESTAMP]',
    updatedAt: '[TIMESTAMP]',
  };
}

test('normalized snapshot', () => {
  const data = fetchData();
  expect(normalizeForSnapshot(data)).toMatchSnapshot();
});
```

## Troubleshooting

### Obsolete Snapshots

Remove unused snapshots when tests are deleted:

```bash
# rjest will show warning about obsolete snapshots
jest -u  # Updates will remove obsolete ones
```

### Large Diffs

If snapshot diffs are too large to review:

1. Break into smaller snapshots
2. Use inline snapshots for small values
3. Consider if snapshot testing is appropriate

### Snapshot Mismatch in CI

```bash
# Never update snapshots in CI
# If this fails, update locally and commit
jest  # Fails if snapshots don't match
```

## Example: Testing a Formatter

```typescript
// formatter.ts
export function formatError(error: Error): string {
  return `[ERROR] ${error.name}: ${error.message}`;
}

export function formatUser(user: User): FormattedUser {
  return {
    displayName: `${user.firstName} ${user.lastName}`,
    email: user.email.toLowerCase(),
    role: user.role || 'user',
  };
}

// formatter.test.ts
import { formatError, formatUser } from './formatter';

describe('formatError', () => {
  test('formats standard error', () => {
    const error = new Error('Something went wrong');
    expect(formatError(error)).toMatchSnapshot();
  });

  test('formats custom error', () => {
    const error = new TypeError('Invalid type');
    expect(formatError(error)).toMatchSnapshot();
  });
});

describe('formatUser', () => {
  test('formats complete user', () => {
    const user = {
      firstName: 'Alice',
      lastName: 'Smith',
      email: 'ALICE@EXAMPLE.COM',
      role: 'admin',
    };
    expect(formatUser(user)).toMatchSnapshot();
  });

  test('formats minimal user', () => {
    const user = {
      firstName: 'Bob',
      lastName: 'Jones',
      email: 'bob@test.com',
    };
    expect(formatUser(user)).toMatchSnapshot();
  });
});
```
