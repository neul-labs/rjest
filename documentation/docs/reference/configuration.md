# Configuration

rjest reads configuration from your `jest.config.js` or `jest.config.ts` file.

## Configuration File

rjest looks for configuration in this order:

1. `jest.config.js`
2. `jest.config.ts`
3. `jest.config.mjs`
4. `jest.config.json`
5. `package.json` (`jest` key)

## Supported Options

### Test Matching

#### `testMatch`

Glob patterns for test files:

```javascript
module.exports = {
  testMatch: [
    '**/__tests__/**/*.ts',
    '**/*.test.ts',
    '**/*.spec.ts',
  ],
};
```

#### `testRegex`

Regex pattern for test files (alternative to `testMatch`):

```javascript
module.exports = {
  testRegex: '(/__tests__/.*|(\\.|/)(test|spec))\\.tsx?$',
};
```

#### `testPathIgnorePatterns`

Patterns to exclude from testing:

```javascript
module.exports = {
  testPathIgnorePatterns: [
    '/node_modules/',
    '/dist/',
    '/__fixtures__/',
  ],
};
```

### Module Resolution

#### `moduleFileExtensions`

File extensions to consider:

```javascript
module.exports = {
  moduleFileExtensions: ['ts', 'tsx', 'js', 'jsx', 'json'],
};
```

#### `moduleNameMapper`

Map module paths:

```javascript
module.exports = {
  moduleNameMapper: {
    '^@/(.*)$': '<rootDir>/src/$1',
    '^~/(.*)$': '<rootDir>/src/$1',
  },
};
```

#### `roots`

Directories to search for tests:

```javascript
module.exports = {
  roots: ['<rootDir>/src', '<rootDir>/tests'],
};
```

### Setup Files

#### `setupFiles`

Scripts to run before test framework loads:

```javascript
module.exports = {
  setupFiles: ['<rootDir>/setup/env.ts'],
};
```

#### `setupFilesAfterEnv`

Scripts to run after test framework loads:

```javascript
module.exports = {
  setupFilesAfterEnv: ['<rootDir>/setup/jest.setup.ts'],
};
```

Example setup file:

```typescript title="jest.setup.ts"
// Add custom matchers
expect.extend({
  toBeWithinRange(received, floor, ceiling) {
    const pass = received >= floor && received <= ceiling;
    return {
      message: () => `expected ${received} to be within range ${floor} - ${ceiling}`,
      pass,
    };
  },
});
```

### Test Behavior

#### `testTimeout`

Default timeout for tests (milliseconds):

```javascript
module.exports = {
  testTimeout: 5000, // 5 seconds
};
```

#### `bail`

Stop after first failure:

```javascript
module.exports = {
  bail: true,
  // or specify number of failures
  bail: 3,
};
```

### Mock Behavior

#### `clearMocks`

Clear mock calls between tests:

```javascript
module.exports = {
  clearMocks: true,
};
```

#### `resetMocks`

Reset mock state between tests:

```javascript
module.exports = {
  resetMocks: true,
};
```

#### `restoreMocks`

Restore original implementations between tests:

```javascript
module.exports = {
  restoreMocks: true,
};
```

### Snapshot

#### `snapshotSerializers`

Custom snapshot serializers:

```javascript
module.exports = {
  snapshotSerializers: ['my-custom-serializer'],
};
```

## Example Configurations

### TypeScript Project

```javascript title="jest.config.js"
module.exports = {
  testMatch: ['**/*.test.ts', '**/*.test.tsx'],
  moduleFileExtensions: ['ts', 'tsx', 'js', 'jsx', 'json'],
  moduleNameMapper: {
    '^@/(.*)$': '<rootDir>/src/$1',
  },
  setupFilesAfterEnv: ['<rootDir>/jest.setup.ts'],
  testPathIgnorePatterns: ['/node_modules/', '/dist/'],
};
```

### Monorepo

```javascript title="jest.config.js"
module.exports = {
  roots: ['<rootDir>/packages'],
  testMatch: ['**/src/**/*.test.ts'],
  moduleNameMapper: {
    '^@myorg/(.*)$': '<rootDir>/packages/$1/src',
  },
};
```

### Minimal Configuration

```javascript title="jest.config.js"
module.exports = {
  testMatch: ['**/*.test.ts'],
};
```

## Options Not Supported

These Jest options are not yet supported:

| Option | Status |
|--------|--------|
| `transform` | Not needed (built-in SWC) |
| `preset` | Not supported |
| `testEnvironment` | Only Node environment |
| `coverageThreshold` | Coverage not implemented |
| `globalSetup` | Not supported |
| `globalTeardown` | Not supported |
| `projects` | Not supported |
| `reporters` | Only default reporter |

## TypeScript Configuration

For TypeScript config files:

```typescript title="jest.config.ts"
import type { Config } from 'jest';

const config: Config = {
  testMatch: ['**/*.test.ts'],
  moduleFileExtensions: ['ts', 'tsx', 'js', 'jsx'],
};

export default config;
```

!!! note
    rjest compiles `jest.config.ts` automatically using the built-in SWC compiler.
