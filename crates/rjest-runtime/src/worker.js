#!/usr/bin/env node
/**
 * rjest worker process
 *
 * Receives test execution requests via stdin (JSON lines),
 * runs tests, and returns results via stdout.
 */

const vm = require('vm');
const path = require('path');
const fs = require('fs');
const Module = require('module');
const { spawnSync } = require('child_process');

// Transform cache to avoid re-transforming files
const transformCache = new Map();

// Current project config
let currentConfig = null;

/**
 * Transform TypeScript/TSX/JSX to JavaScript
 */
function transformCode(filePath, source) {
  const ext = path.extname(filePath);

  // Only transform TS/TSX/JSX files
  if (!['.ts', '.tsx', '.jsx', '.mts', '.cts'].includes(ext)) {
    return source;
  }

  // Check cache
  const cacheKey = filePath + ':' + Buffer.from(source).toString('base64').slice(0, 32);
  if (transformCache.has(cacheKey)) {
    return transformCache.get(cacheKey);
  }

  let transformed = null;

  // Try esbuild first (faster, more reliable)
  try {
    const result = spawnSync('npx', [
      'esbuild',
      '--loader=' + (ext === '.tsx' ? 'tsx' : ext === '.jsx' ? 'jsx' : 'ts'),
      '--format=cjs',
      '--target=es2020',
    ], {
      input: source,
      encoding: 'utf-8',
      timeout: 10000,
    });

    if (result.status === 0 && result.stdout) {
      transformed = result.stdout;
    }
  } catch (e) {
    // esbuild failed, try SWC
  }

  // Try SWC if esbuild failed
  if (!transformed) {
    try {
      const result = spawnSync('npx', [
        'swc',
        '-',
        '--filename', filePath,
        '-C', 'jsc.parser.syntax=typescript',
        '-C', 'jsc.parser.tsx=true',
        '-C', 'jsc.target=es2020',
        '-C', 'module.type=commonjs',
      ], {
        input: source,
        encoding: 'utf-8',
        timeout: 10000,
      });

      if (result.status === 0 && result.stdout) {
        transformed = result.stdout;
      }
    } catch (e) {
      // SWC failed too
    }
  }

  // Fallback: basic type stripping (very limited)
  if (!transformed) {
    transformed = basicTypeStrip(source);
  }

  transformCache.set(cacheKey, transformed);
  return transformed;
}

/**
 * Very basic type stripping fallback
 */
function basicTypeStrip(source) {
  let result = source;

  // Remove type imports
  result = result.replace(/import\s+type\s+[^;]+;/g, '');

  // Remove export type
  result = result.replace(/export\s+type\s+[^;]+;/g, '');

  // Remove interface declarations
  result = result.replace(/interface\s+\w+\s*\{[^}]*\}/g, '');

  // Remove type aliases
  result = result.replace(/type\s+\w+\s*=[^;]+;/g, '');

  // Remove simple type annotations
  result = result.replace(/:\s*(number|string|boolean|void|any|null|undefined|never|object)\b/g, '');

  // Remove generic type parameters from function calls/definitions
  result = result.replace(/<[A-Z][^>]*>/g, '');

  // Remove 'as Type' assertions
  result = result.replace(/\s+as\s+\w+/g, '');

  return result;
}

/**
 * Install require hook for TypeScript files
 */
function installRequireHook() {
  const originalLoad = Module._load;

  Module._load = function(request, parent, isMain) {
    // Resolve the full path
    let resolvedPath;
    try {
      resolvedPath = Module._resolveFilename(request, parent, isMain);
    } catch (e) {
      // Try adding extensions
      const extensions = ['.ts', '.tsx', '.js', '.jsx'];
      const basePath = parent ? path.resolve(path.dirname(parent.filename), request) : request;

      for (const ext of extensions) {
        try {
          const tryPath = basePath + ext;
          if (fs.existsSync(tryPath)) {
            resolvedPath = tryPath;
            break;
          }
        } catch (e2) {
          // Continue trying
        }
      }

      // Try index files
      if (!resolvedPath) {
        for (const ext of extensions) {
          try {
            const tryPath = path.join(basePath, 'index' + ext);
            if (fs.existsSync(tryPath)) {
              resolvedPath = tryPath;
              break;
            }
          } catch (e2) {
            // Continue trying
          }
        }
      }

      if (!resolvedPath) {
        throw e;
      }
    }

    const ext = path.extname(resolvedPath);

    // If it's a TypeScript file, transform it
    if (['.ts', '.tsx', '.mts', '.cts'].includes(ext)) {
      // Check if already in cache
      if (require.cache[resolvedPath]) {
        return require.cache[resolvedPath].exports;
      }

      // Read and transform
      const source = fs.readFileSync(resolvedPath, 'utf-8');
      const transformed = transformCode(resolvedPath, source);

      // Create a new module and compile it
      const mod = new Module(resolvedPath, parent);
      mod.filename = resolvedPath;
      mod.paths = Module._nodeModulePaths(path.dirname(resolvedPath));

      // Compile the transformed code
      mod._compile(transformed, resolvedPath);

      // Cache it
      require.cache[resolvedPath] = mod;

      return mod.exports;
    }

    // Default behavior for other files
    return originalLoad.call(this, request, parent, isMain);
  };
}

// Install the hook immediately
installRequireHook();

// Test state
let currentTestFile = null;
let testResults = [];
let currentDescribe = [];
let hooks = { beforeAll: [], afterAll: [], beforeEach: [], afterEach: [] };

// Snapshot state
let snapshotState = {
  snapshotPath: null,
  snapshots: {},      // key -> snapshot value
  dirty: false,       // whether snapshots need to be written
  counter: {},        // test name -> counter for multiple snapshots in same test
  currentTestName: null,
  updateSnapshot: false, // -u flag
  added: 0,
  updated: 0,
  matched: 0,
  unmatched: 0,
};

/**
 * Initialize snapshot state for a test file
 */
function initSnapshotState(testPath, updateSnapshot) {
  const dir = path.dirname(testPath);
  const base = path.basename(testPath);
  const snapshotDir = path.join(dir, '__snapshots__');
  const snapshotPath = path.join(snapshotDir, base + '.snap');

  snapshotState = {
    snapshotPath,
    snapshots: {},
    dirty: false,
    counter: {},
    currentTestName: null,
    updateSnapshot: updateSnapshot || false,
    added: 0,
    updated: 0,
    matched: 0,
    unmatched: 0,
  };

  // Load existing snapshots
  if (fs.existsSync(snapshotPath)) {
    try {
      const content = fs.readFileSync(snapshotPath, 'utf-8');
      snapshotState.snapshots = parseSnapshotFile(content);
    } catch (e) {
      console.error('Error loading snapshots:', e.message);
    }
  }
}

/**
 * Parse Jest snapshot file format
 */
function parseSnapshotFile(content) {
  const snapshots = {};
  // Match exports[`key`] = `value`;
  const regex = /exports\[`([^`]+)`\]\s*=\s*`([\s\S]*?)`;/g;
  let match;
  while ((match = regex.exec(content)) !== null) {
    const key = match[1];
    // Unescape backticks and backslashes in the value
    const value = match[2].replace(/\\`/g, '`').replace(/\\\\/g, '\\');
    snapshots[key] = value;
  }
  return snapshots;
}

/**
 * Serialize a value for snapshot
 */
function serializeSnapshot(value) {
  if (typeof value === 'string') {
    return value;
  }
  return JSON.stringify(value, null, 2);
}

/**
 * Get the snapshot key for current test
 */
function getSnapshotKey(hint) {
  const testName = snapshotState.currentTestName || 'unknown test';
  const counter = snapshotState.counter[testName] || 1;
  snapshotState.counter[testName] = counter + 1;
  return hint ? `${testName}: ${hint} ${counter}` : `${testName} ${counter}`;
}

/**
 * Match against snapshot
 */
function matchSnapshot(actual, hint) {
  const key = getSnapshotKey(hint);
  const serialized = serializeSnapshot(actual);
  const existing = snapshotState.snapshots[key];

  if (existing === undefined) {
    // New snapshot
    if (snapshotState.updateSnapshot || process.env.CI !== 'true') {
      snapshotState.snapshots[key] = serialized;
      snapshotState.dirty = true;
      snapshotState.added++;
      return { pass: true };
    } else {
      snapshotState.unmatched++;
      return {
        pass: false,
        message: `New snapshot was not written. Run with -u to update.`,
      };
    }
  }

  if (existing === serialized) {
    snapshotState.matched++;
    return { pass: true };
  }

  // Mismatch
  if (snapshotState.updateSnapshot) {
    snapshotState.snapshots[key] = serialized;
    snapshotState.dirty = true;
    snapshotState.updated++;
    return { pass: true };
  }

  snapshotState.unmatched++;
  return {
    pass: false,
    message: `Snapshot mismatch`,
    expected: existing,
    received: serialized,
  };
}

/**
 * Save snapshots to file if dirty
 */
function saveSnapshots() {
  if (!snapshotState.dirty || !snapshotState.snapshotPath) {
    return;
  }

  const dir = path.dirname(snapshotState.snapshotPath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }

  // Generate snapshot file content in Jest format
  let content = '// Jest Snapshot v1, https://goo.gl/fbAQLP\n\n';

  const sortedKeys = Object.keys(snapshotState.snapshots).sort();
  for (const key of sortedKeys) {
    const value = snapshotState.snapshots[key];
    // Escape backticks and backslashes
    const escaped = value.replace(/\\/g, '\\\\').replace(/`/g, '\\`');
    content += `exports[\`${key}\`] = \`${escaped}\`;\n\n`;
  }

  fs.writeFileSync(snapshotState.snapshotPath, content);
  snapshotState.dirty = false;
}

/**
 * Get snapshot summary for test results
 */
function getSnapshotSummary() {
  return {
    added: snapshotState.added,
    updated: snapshotState.updated,
    matched: snapshotState.matched,
    unmatched: snapshotState.unmatched,
  };
}

// Create expect from jest-expect or a simple implementation
let expect;
try {
  expect = require('expect');
} catch (e) {
  // Fallback simple expect
  expect = createSimpleExpect();
}

function createSimpleExpect() {
  return function expect(actual) {
    return {
      toBe(expected) {
        if (actual !== expected) {
          throw new Error(`Expected ${JSON.stringify(expected)} but got ${JSON.stringify(actual)}`);
        }
      },
      toEqual(expected) {
        if (JSON.stringify(actual) !== JSON.stringify(expected)) {
          throw new Error(`Expected ${JSON.stringify(expected)} but got ${JSON.stringify(actual)}`);
        }
      },
      toBeTruthy() {
        if (!actual) {
          throw new Error(`Expected truthy but got ${JSON.stringify(actual)}`);
        }
      },
      toBeFalsy() {
        if (actual) {
          throw new Error(`Expected falsy but got ${JSON.stringify(actual)}`);
        }
      },
      toBeNull() {
        if (actual !== null) {
          throw new Error(`Expected null but got ${JSON.stringify(actual)}`);
        }
      },
      toBeUndefined() {
        if (actual !== undefined) {
          throw new Error(`Expected undefined but got ${JSON.stringify(actual)}`);
        }
      },
      toBeDefined() {
        if (actual === undefined) {
          throw new Error(`Expected defined but got undefined`);
        }
      },
      toContain(item) {
        if (Array.isArray(actual)) {
          if (!actual.includes(item)) {
            throw new Error(`Expected array to contain ${JSON.stringify(item)}`);
          }
        } else if (typeof actual === 'string') {
          if (!actual.includes(item)) {
            throw new Error(`Expected string to contain ${JSON.stringify(item)}`);
          }
        }
      },
      toThrow(expected) {
        let threw = false;
        let error;
        try {
          actual();
        } catch (e) {
          threw = true;
          error = e;
        }
        if (!threw) {
          throw new Error('Expected function to throw');
        }
        if (expected !== undefined) {
          if (typeof expected === 'string' && !error.message.includes(expected)) {
            throw new Error(`Expected error message to contain "${expected}" but got "${error.message}"`);
          }
          if (expected instanceof RegExp && !expected.test(error.message)) {
            throw new Error(`Expected error message to match ${expected} but got "${error.message}"`);
          }
        }
      },
      toHaveLength(length) {
        if (actual.length !== length) {
          throw new Error(`Expected length ${length} but got ${actual.length}`);
        }
      },
      toBeGreaterThan(n) {
        if (!(actual > n)) {
          throw new Error(`Expected ${actual} to be greater than ${n}`);
        }
      },
      toBeLessThan(n) {
        if (!(actual < n)) {
          throw new Error(`Expected ${actual} to be less than ${n}`);
        }
      },
      toBeGreaterThanOrEqual(n) {
        if (!(actual >= n)) {
          throw new Error(`Expected ${actual} to be greater than or equal to ${n}`);
        }
      },
      toBeLessThanOrEqual(n) {
        if (!(actual <= n)) {
          throw new Error(`Expected ${actual} to be less than or equal to ${n}`);
        }
      },
      toMatch(pattern) {
        const regex = typeof pattern === 'string' ? new RegExp(pattern) : pattern;
        if (!regex.test(actual)) {
          throw new Error(`Expected "${actual}" to match ${pattern}`);
        }
      },
      toHaveProperty(path, value) {
        const parts = path.split('.');
        let current = actual;
        for (const part of parts) {
          if (current === undefined || current === null || !(part in current)) {
            throw new Error(`Expected object to have property "${path}"`);
          }
          current = current[part];
        }
        if (value !== undefined && current !== value) {
          throw new Error(`Expected property "${path}" to be ${JSON.stringify(value)} but got ${JSON.stringify(current)}`);
        }
      },
      toBeInstanceOf(constructor) {
        if (!(actual instanceof constructor)) {
          throw new Error(`Expected value to be instance of ${constructor.name}`);
        }
      },
      toBeNaN() {
        if (!Number.isNaN(actual)) {
          throw new Error(`Expected NaN but got ${JSON.stringify(actual)}`);
        }
      },
      toBeCloseTo(expected, precision = 2) {
        const pow = Math.pow(10, precision + 1);
        const delta = Math.abs(expected - actual);
        const maxDelta = Math.pow(10, -precision) / 2;
        if (delta >= maxDelta) {
          throw new Error(`Expected ${actual} to be close to ${expected} (precision: ${precision})`);
        }
      },
      toContainEqual(expected) {
        if (!Array.isArray(actual)) {
          throw new Error('toContainEqual expects an array');
        }
        const found = actual.some(item => JSON.stringify(item) === JSON.stringify(expected));
        if (!found) {
          throw new Error(`Expected array to contain equal to ${JSON.stringify(expected)}`);
        }
      },
      toHaveBeenCalled() {
        if (!actual.mock || actual.mock.calls.length === 0) {
          throw new Error('Expected mock function to have been called');
        }
      },
      toHaveBeenCalledTimes(n) {
        if (!actual.mock || actual.mock.calls.length !== n) {
          throw new Error(`Expected mock to be called ${n} times but was called ${actual.mock?.calls.length || 0} times`);
        }
      },
      toHaveBeenCalledWith(...args) {
        if (!actual.mock) {
          throw new Error('Expected a mock function');
        }
        const found = actual.mock.calls.some(call =>
          JSON.stringify(call) === JSON.stringify(args)
        );
        if (!found) {
          throw new Error(`Expected mock to have been called with ${JSON.stringify(args)}`);
        }
      },
      toMatchSnapshot(hint) {
        const result = matchSnapshot(actual, hint);
        if (!result.pass) {
          const error = new Error(result.message);
          if (result.expected !== undefined) {
            error.matcherResult = {
              message: `Snapshot mismatch:\n\nExpected:\n${result.expected}\n\nReceived:\n${result.received}`,
            };
          }
          throw error;
        }
      },
      toMatchInlineSnapshot(inlineSnapshot) {
        const serialized = serializeSnapshot(actual);
        // Normalize whitespace for comparison
        const normalizedActual = serialized.trim();
        const normalizedExpected = (inlineSnapshot || '').trim();

        if (inlineSnapshot === undefined) {
          // No inline snapshot provided - this would need source code rewriting
          // For now, just pass (Jest would update the source file)
          return;
        }

        if (normalizedActual !== normalizedExpected) {
          throw new Error(
            `Inline snapshot mismatch:\n\nExpected:\n${normalizedExpected}\n\nReceived:\n${normalizedActual}`
          );
        }
      },
      not: {
        toBe(expected) {
          if (actual === expected) {
            throw new Error(`Expected not ${JSON.stringify(expected)}`);
          }
        },
        toEqual(expected) {
          if (JSON.stringify(actual) === JSON.stringify(expected)) {
            throw new Error(`Expected not equal to ${JSON.stringify(expected)}`);
          }
        },
        toBeNull() {
          if (actual === null) {
            throw new Error('Expected not null');
          }
        },
        toBeUndefined() {
          if (actual === undefined) {
            throw new Error('Expected not undefined');
          }
        },
        toBeTruthy() {
          if (actual) {
            throw new Error(`Expected not truthy but got ${JSON.stringify(actual)}`);
          }
        },
        toBeFalsy() {
          if (!actual) {
            throw new Error('Expected not falsy');
          }
        },
        toContain(item) {
          if (Array.isArray(actual) && actual.includes(item)) {
            throw new Error(`Expected not to contain ${JSON.stringify(item)}`);
          }
          if (typeof actual === 'string' && actual.includes(item)) {
            throw new Error(`Expected not to contain "${item}"`);
          }
        },
        toThrow() {
          let threw = false;
          try {
            actual();
          } catch (e) {
            threw = true;
          }
          if (threw) {
            throw new Error('Expected function not to throw');
          }
        },
        toHaveBeenCalled() {
          if (actual.mock && actual.mock.calls.length > 0) {
            throw new Error('Expected mock function not to have been called');
          }
        },
        toBeNaN() {
          if (Number.isNaN(actual)) {
            throw new Error('Expected not NaN');
          }
        },
        toContainEqual(expected) {
          if (Array.isArray(actual)) {
            const found = actual.some(item => JSON.stringify(item) === JSON.stringify(expected));
            if (found) {
              throw new Error(`Expected array not to contain equal to ${JSON.stringify(expected)}`);
            }
          }
        },
      },
      resolves: {
        async toBe(expected) {
          const resolved = await actual;
          if (resolved !== expected) {
            throw new Error(`Expected ${JSON.stringify(expected)} but got ${JSON.stringify(resolved)}`);
          }
        },
        async toEqual(expected) {
          const resolved = await actual;
          if (JSON.stringify(resolved) !== JSON.stringify(expected)) {
            throw new Error(`Expected ${JSON.stringify(expected)} but got ${JSON.stringify(resolved)}`);
          }
        },
      },
      rejects: {
        async toThrow(expected) {
          try {
            await actual;
            throw new Error('Expected promise to reject');
          } catch (e) {
            if (e.message === 'Expected promise to reject') throw e;
            if (expected !== undefined) {
              if (typeof expected === 'string' && !e.message.includes(expected)) {
                throw new Error(`Expected rejection to contain "${expected}"`);
              }
            }
          }
        },
      },
    };
  };
}

// Fake timers implementation
let fakeTimersEnabled = false;
let fakeTime = 0;
let timers = [];
let timerIdCounter = 1;
const realSetTimeout = global.setTimeout;
const realSetInterval = global.setInterval;
const realClearTimeout = global.clearTimeout;
const realClearInterval = global.clearInterval;
const realDate = Date;

function installFakeTimers() {
  fakeTimersEnabled = true;
  fakeTime = Date.now();
  timers = [];

  global.setTimeout = (fn, delay, ...args) => {
    const id = timerIdCounter++;
    timers.push({ id, fn, time: fakeTime + (delay || 0), args, type: 'timeout' });
    return id;
  };

  global.setInterval = (fn, delay, ...args) => {
    const id = timerIdCounter++;
    timers.push({ id, fn, time: fakeTime + (delay || 0), delay, args, type: 'interval' });
    return id;
  };

  global.clearTimeout = (id) => {
    timers = timers.filter(t => t.id !== id);
  };

  global.clearInterval = (id) => {
    timers = timers.filter(t => t.id !== id);
  };

  global.Date = class extends realDate {
    constructor(...args) {
      if (args.length === 0) {
        super(fakeTime);
      } else {
        super(...args);
      }
    }
    static now() {
      return fakeTime;
    }
  };
}

function uninstallFakeTimers() {
  fakeTimersEnabled = false;
  global.setTimeout = realSetTimeout;
  global.setInterval = realSetInterval;
  global.clearTimeout = realClearTimeout;
  global.clearInterval = realClearInterval;
  global.Date = realDate;
}

function advanceTimersByTime(ms) {
  const targetTime = fakeTime + ms;
  while (timers.length > 0) {
    timers.sort((a, b) => a.time - b.time);
    const next = timers[0];
    if (next.time > targetTime) break;

    fakeTime = next.time;
    timers.shift();
    next.fn(...(next.args || []));

    if (next.type === 'interval') {
      timers.push({
        ...next,
        time: fakeTime + next.delay,
      });
    }
  }
  fakeTime = targetTime;
}

function runAllTimers() {
  const maxIterations = 100000;
  let iterations = 0;
  while (timers.length > 0 && iterations < maxIterations) {
    timers.sort((a, b) => a.time - b.time);
    const next = timers.shift();
    fakeTime = next.time;
    next.fn(...(next.args || []));

    if (next.type === 'interval') {
      timers.push({
        ...next,
        time: fakeTime + next.delay,
      });
    }
    iterations++;
  }
}

// Jest globals
const jestGlobals = {
  describe(name, fn) {
    currentDescribe.push(name);
    const prevHooks = { ...hooks };
    hooks = { beforeAll: [], afterAll: [], beforeEach: [], afterEach: [] };

    fn();

    // Run beforeAll hooks before tests
    for (const hook of hooks.beforeAll) {
      try {
        hook();
      } catch (e) {
        testResults.push({
          name: [...currentDescribe, 'beforeAll hook'].join(' > '),
          status: 'failed',
          duration_ms: 0,
          error: formatError(e),
        });
      }
    }

    // Run afterAll hooks
    for (const hook of hooks.afterAll) {
      try {
        hook();
      } catch (e) {
        testResults.push({
          name: [...currentDescribe, 'afterAll hook'].join(' > '),
          status: 'failed',
          duration_ms: 0,
          error: formatError(e),
        });
      }
    }

    hooks = prevHooks;
    currentDescribe.pop();
  },

  test(name, fn, timeout = 5000) {
    const fullName = [...currentDescribe, name].join(' > ');
    const startTime = Date.now();

    // Check if test matches the name pattern filter
    if (currentConfig?.test_name_pattern) {
      try {
        const pattern = new RegExp(currentConfig.test_name_pattern);
        if (!pattern.test(fullName)) {
          // Test doesn't match pattern, skip it
          testResults.push({
            name: fullName,
            status: 'skipped',
            duration_ms: 0,
            error: null,
          });
          return;
        }
      } catch (e) {
        // Invalid regex pattern, run all tests
        console.error('Invalid testNamePattern:', e.message);
      }
    }

    // Set current test name for snapshot tracking
    snapshotState.currentTestName = fullName;

    const result = {
      name: fullName,
      status: 'passed',
      duration_ms: 0,
      error: null,
    };

    try {
      // Run beforeEach hooks
      for (const hook of hooks.beforeEach) {
        hook();
      }

      const maybePromise = fn();

      if (maybePromise && typeof maybePromise.then === 'function') {
        // Async test - we need to handle this properly
        // For now, we'll use a synchronous approach with a nested event loop
        // This is a limitation - proper async handling requires more work
        let resolved = false;
        let error = null;

        maybePromise
          .then(() => { resolved = true; })
          .catch((e) => { resolved = true; error = e; });

        // Simple spin-wait with a timeout (not ideal but works for basic cases)
        const deadline = Date.now() + timeout;
        const checkInterval = 10;

        const waitForPromise = () => {
          return new Promise((resolve) => {
            const check = () => {
              if (resolved || Date.now() > deadline) {
                resolve();
              } else {
                realSetTimeout(check, checkInterval);
              }
            };
            check();
          });
        };

        // We can't properly await here in sync context, so mark as pending
        // and the main loop will handle async tests specially
        result._promise = maybePromise;
        result._timeout = timeout;
      }

      result.duration_ms = Date.now() - startTime;

      // Run afterEach hooks
      for (const hook of hooks.afterEach) {
        hook();
      }
    } catch (e) {
      result.status = 'failed';
      result.error = formatError(e);
      result.duration_ms = Date.now() - startTime;
    }

    testResults.push(result);
  },

  it(...args) {
    return jestGlobals.test(...args);
  },

  beforeAll(fn) {
    hooks.beforeAll.push(fn);
  },

  afterAll(fn) {
    hooks.afterAll.push(fn);
  },

  beforeEach(fn) {
    hooks.beforeEach.push(fn);
  },

  afterEach(fn) {
    hooks.afterEach.push(fn);
  },

  expect,

  jest: {
    fn(impl) {
      const mockFn = function (...args) {
        mockFn.mock.calls.push(args);
        mockFn.mock.instances.push(this);
        const result = { type: 'return', value: undefined };
        try {
          if (mockFn._implOnce.length > 0) {
            // Use one-time implementation first
            const onceImpl = mockFn._implOnce.shift();
            result.value = onceImpl.apply(this, args);
          } else if (mockFn._returnValues.length > 0) {
            // Use one-time return value
            result.value = mockFn._returnValues.shift();
          } else if (mockFn._impl) {
            result.value = mockFn._impl.apply(this, args);
          } else {
            result.value = mockFn._returnValue;
          }
        } catch (e) {
          result.type = 'throw';
          result.value = e;
          mockFn.mock.results.push(result);
          throw e;
        }
        mockFn.mock.results.push(result);
        return result.value;
      };
      mockFn.mock = { calls: [], instances: [], results: [] };
      mockFn._impl = impl;
      mockFn._implOnce = [];
      mockFn._returnValue = undefined;
      mockFn._returnValues = [];
      mockFn.mockReturnValue = (val) => {
        mockFn._returnValue = val;
        return mockFn;
      };
      mockFn.mockReturnValueOnce = (val) => {
        mockFn._returnValues.push(val);
        return mockFn;
      };
      mockFn.mockResolvedValue = (val) => {
        mockFn._impl = () => Promise.resolve(val);
        return mockFn;
      };
      mockFn.mockResolvedValueOnce = (val) => {
        mockFn._implOnce.push(() => Promise.resolve(val));
        return mockFn;
      };
      mockFn.mockRejectedValue = (val) => {
        mockFn._impl = () => Promise.reject(val);
        return mockFn;
      };
      mockFn.mockRejectedValueOnce = (val) => {
        mockFn._implOnce.push(() => Promise.reject(val));
        return mockFn;
      };
      mockFn.mockImplementation = (fn) => {
        mockFn._impl = fn;
        return mockFn;
      };
      mockFn.mockImplementationOnce = (fn) => {
        mockFn._implOnce.push(fn);
        return mockFn;
      };
      mockFn.mockClear = () => {
        mockFn.mock.calls = [];
        mockFn.mock.instances = [];
        mockFn.mock.results = [];
      };
      mockFn.mockReset = () => {
        mockFn.mockClear();
        mockFn._impl = undefined;
        mockFn._implOnce = [];
        mockFn._returnValue = undefined;
        mockFn._returnValues = [];
      };
      mockFn.mockRestore = () => {
        mockFn.mockReset();
      };
      mockFn.getMockName = () => 'jest.fn()';
      mockFn.mockName = (name) => {
        mockFn.getMockName = () => name;
        return mockFn;
      };
      return mockFn;
    },

    spyOn(obj, method) {
      const original = obj[method];
      const spy = jestGlobals.jest.fn(original.bind(obj));
      spy.mockRestore = () => {
        obj[method] = original;
      };
      obj[method] = spy;
      return spy;
    },

    mock(moduleName, factory) {
      if (!global.__mocks__) global.__mocks__ = {};
      global.__mocks__[moduleName] = factory ? factory() : {};
    },

    unmock(moduleName) {
      if (global.__mocks__) {
        delete global.__mocks__[moduleName];
      }
    },

    useFakeTimers() {
      installFakeTimers();
      return jestGlobals.jest;
    },

    useRealTimers() {
      uninstallFakeTimers();
      return jestGlobals.jest;
    },

    advanceTimersByTime(ms) {
      advanceTimersByTime(ms);
      return jestGlobals.jest;
    },

    runAllTimers() {
      runAllTimers();
      return jestGlobals.jest;
    },

    runOnlyPendingTimers() {
      const pending = [...timers];
      timers = [];
      for (const timer of pending) {
        if (timer.type === 'timeout') {
          timer.fn(...(timer.args || []));
        }
      }
      return jestGlobals.jest;
    },

    clearAllTimers() {
      timers = [];
      return jestGlobals.jest;
    },

    setSystemTime(time) {
      fakeTime = typeof time === 'number' ? time : new Date(time).getTime();
      return jestGlobals.jest;
    },

    getRealSystemTime() {
      return realDate.now();
    },

    clearAllMocks() {
      // Clear all mock state - would need to track all mocks
    },

    resetAllMocks() {
      // Reset all mock implementations
    },

    restoreAllMocks() {
      // Restore all mocks to original
    },

    resetModules() {
      // Clear module cache
      Object.keys(require.cache).forEach(key => {
        delete require.cache[key];
      });
    },
  },
};

// Add test.skip, test.only, etc.
jestGlobals.test.skip = function (name, fn) {
  testResults.push({
    name: [...currentDescribe, name].join(' > '),
    status: 'skipped',
    duration_ms: 0,
    error: null,
  });
};

jestGlobals.test.only = jestGlobals.test;
jestGlobals.test.todo = function (name) {
  testResults.push({
    name: [...currentDescribe, name].join(' > '),
    status: 'todo',
    duration_ms: 0,
    error: null,
  });
};

jestGlobals.test.each = function(cases) {
  return function(name, fn) {
    for (const args of cases) {
      const testName = name.replace(/%s/g, () => String(args.shift?.() ?? args));
      jestGlobals.test(testName, () => fn(...(Array.isArray(args) ? args : [args])));
    }
  };
};

jestGlobals.it.skip = jestGlobals.test.skip;
jestGlobals.it.only = jestGlobals.test.only;
jestGlobals.it.todo = jestGlobals.test.todo;
jestGlobals.it.each = jestGlobals.test.each;

jestGlobals.describe.skip = function (name, fn) {
  // Skip all tests in describe by not running fn
  currentDescribe.push(name);
  currentDescribe.pop();
};
jestGlobals.describe.only = jestGlobals.describe;
jestGlobals.describe.each = function(cases) {
  return function(name, fn) {
    for (const args of cases) {
      const suiteName = name.replace(/%s/g, () => String(args.shift?.() ?? args));
      jestGlobals.describe(suiteName, () => fn(...(Array.isArray(args) ? args : [args])));
    }
  };
};

function formatError(e) {
  return {
    message: e.message || String(e),
    stack: e.stack || null,
    diff: e.matcherResult?.message || null,
  };
}

// Track if setup files have been run for this worker
let setupFilesRun = false;
let setupFilesAfterEnvRun = false;

// Run setup files (before test framework)
function runSetupFiles(config) {
  if (!config || !config.setup_files || setupFilesRun) return;

  for (const setupFile of config.setup_files) {
    try {
      require(setupFile);
    } catch (e) {
      console.error(`Error running setupFile ${setupFile}:`, e.message);
    }
  }
  setupFilesRun = true;
}

// Run setup files after env (after test framework, before tests)
function runSetupFilesAfterEnv(config, context) {
  if (!config || !config.setup_files_after_env) return;

  for (const setupFile of config.setup_files_after_env) {
    try {
      // Run in context so it has access to Jest globals
      const setupCode = fs.readFileSync(setupFile, 'utf-8');
      const ext = path.extname(setupFile);

      let code = setupCode;
      if (['.ts', '.tsx'].includes(ext)) {
        code = transformCode(setupFile, setupCode);
      }

      const script = new vm.Script(code, { filename: setupFile });
      script.runInContext(context);
    } catch (e) {
      console.error(`Error running setupFilesAfterEnv ${setupFile}:`, e.message);
    }
  }
}

// Execute a test file
async function runTestFile(request) {
  const { path: testPath, code, config } = request;

  currentTestFile = testPath;
  currentConfig = config;
  testResults = [];
  currentDescribe = [];
  hooks = { beforeAll: [], afterAll: [], beforeEach: [], afterEach: [] };

  // Initialize snapshot state
  const updateSnapshot = config?.update_snapshots || false;
  initSnapshotState(testPath, updateSnapshot);

  // Reset fake timers
  if (fakeTimersEnabled) {
    uninstallFakeTimers();
  }

  // Clear module cache for test isolation
  Object.keys(require.cache).forEach(key => {
    // Don't clear node_modules
    if (!key.includes('node_modules')) {
      delete require.cache[key];
    }
  });

  // Clear mocks
  global.__mocks__ = {};

  // Run setupFiles (only once per worker)
  runSetupFiles(config);

  const startTime = Date.now();

  try {
    // Create a module context with Jest globals
    const context = {
      ...jestGlobals,
      console,
      setTimeout: global.setTimeout,
      setInterval: global.setInterval,
      clearTimeout: global.clearTimeout,
      clearInterval: global.clearInterval,
      setImmediate,
      clearImmediate,
      Buffer,
      process,
      global,
      __filename: testPath,
      __dirname: path.dirname(testPath),
      require: createRequire(testPath),
      module: { exports: {} },
      exports: {},
    };

    // Create VM context
    const vmContext = vm.createContext(context);

    // Run setupFilesAfterEnv (before each test file)
    runSetupFilesAfterEnv(config, vmContext);

    // Run the test code
    const script = new vm.Script(code, { filename: testPath });
    script.runInContext(vmContext);

    // Handle async tests
    for (const result of testResults) {
      if (result._promise) {
        try {
          await Promise.race([
            result._promise,
            new Promise((_, reject) =>
              realSetTimeout(() => reject(new Error('Test timeout')), result._timeout)
            ),
          ]);
        } catch (e) {
          result.status = 'failed';
          result.error = formatError(e);
        }
        delete result._promise;
        delete result._timeout;
      }
    }

    // Wait a tick for any pending operations
    await new Promise((resolve) => setImmediate(resolve));

  } catch (e) {
    // File-level error
    testResults.push({
      name: 'Test file execution',
      status: 'failed',
      duration_ms: Date.now() - startTime,
      error: formatError(e),
    });
  }

  // Cleanup fake timers
  if (fakeTimersEnabled) {
    uninstallFakeTimers();
  }

  // Save any new/updated snapshots
  saveSnapshots();

  const duration = Date.now() - startTime;
  const passed = testResults.every((t) => t.status === 'passed' || t.status === 'skipped' || t.status === 'todo');

  return {
    path: testPath,
    passed,
    duration_ms: duration,
    tests: testResults,
    snapshot_summary: getSnapshotSummary(),
  };
}

function createRequire(fromPath) {
  const originalRequire = Module.createRequire(fromPath);

  return function customRequire(id) {
    // Check for mocks first
    if (global.__mocks__ && global.__mocks__[id]) {
      return global.__mocks__[id];
    }

    // Apply moduleNameMapper if configured
    let resolvedId = id;
    if (currentConfig && currentConfig.module_name_mapper) {
      for (const [pattern, replacement] of Object.entries(currentConfig.module_name_mapper)) {
        try {
          const regex = new RegExp(pattern);
          if (regex.test(id)) {
            resolvedId = id.replace(regex, replacement);
            // Replace <rootDir> if present
            if (currentConfig.root_dir) {
              resolvedId = resolvedId.replace('<rootDir>', currentConfig.root_dir);
            }
            break;
          }
        } catch (e) {
          // Invalid regex, skip
        }
      }
    }

    // Handle relative paths with TypeScript extensions
    if (resolvedId.startsWith('.') || resolvedId.startsWith('/')) {
      const basePath = resolvedId.startsWith('/')
        ? resolvedId
        : path.resolve(path.dirname(fromPath), resolvedId);
      const extensions = ['.ts', '.tsx', '.js', '.jsx', ''];

      for (const ext of extensions) {
        const tryPath = basePath + ext;
        if (fs.existsSync(tryPath)) {
          return originalRequire(tryPath);
        }
      }

      // Try index files
      for (const ext of extensions) {
        const tryPath = path.join(basePath, 'index' + ext);
        if (fs.existsSync(tryPath)) {
          return originalRequire(tryPath);
        }
      }
    }

    return originalRequire(resolvedId);
  };
}

// Main message loop
async function main() {
  const readline = require('readline');

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    terminal: false,
  });

  for await (const line of rl) {
    try {
      const request = JSON.parse(line);

      if (request.type === 'run') {
        const result = await runTestFile(request);
        console.log(JSON.stringify({ type: 'result', ...result }));
      } else if (request.type === 'warmup') {
        // Warmup by initializing Jest runtime with a minimal test
        try {
          // Load core modules to warm up V8
          require('path');
          require('fs');
          require('vm');
          // Initialize a minimal test context
          global.expect = () => ({ toBe: () => {} });
          global.test = () => {};
          global.describe = () => {};
          console.log(JSON.stringify({ type: 'warmed' }));
        } catch (e) {
          console.log(JSON.stringify({ type: 'warmed' }));
        }
      } else if (request.type === 'ping') {
        console.log(JSON.stringify({ type: 'pong' }));
      } else if (request.type === 'exit') {
        process.exit(0);
      }
    } catch (e) {
      console.log(JSON.stringify({
        type: 'error',
        message: e.message,
        stack: e.stack,
      }));
    }
  }
}

main().catch((e) => {
  console.error('Worker error:', e);
  process.exit(1);
});
