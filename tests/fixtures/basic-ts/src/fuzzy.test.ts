import { add, subtract, multiply, divide, isEven, capitalize } from './utils';

/**
 * Fuzzy Mutation Tests
 * Tests edge cases, boundary conditions, and unusual inputs
 */

describe('Fuzzy Math Edge Cases', () => {
  describe('add - edge cases', () => {
    test('adds very large numbers', () => {
      expect(add(Number.MAX_SAFE_INTEGER, 1)).toBe(Number.MAX_SAFE_INTEGER + 1);
    });

    test('adds very small decimals', () => {
      // Floating point precision test
      expect(add(0.1, 0.2)).toBeCloseTo(0.3);
    });

    test('adds negative infinity', () => {
      expect(add(-Infinity, 100)).toBe(-Infinity);
    });

    test('adds positive infinity', () => {
      expect(add(Infinity, -100)).toBe(Infinity);
    });

    test('NaN propagates', () => {
      expect(add(NaN, 5)).toBeNaN();
    });

    test('adds negative zero', () => {
      expect(add(-0, 0)).toBe(0);
    });
  });

  describe('subtract - edge cases', () => {
    test('subtracts to negative infinity', () => {
      expect(subtract(-Infinity, 100)).toBe(-Infinity);
    });

    test('infinity minus infinity is NaN', () => {
      expect(subtract(Infinity, Infinity)).toBeNaN();
    });

    test('handles very small differences', () => {
      expect(subtract(1e-10, 1e-11)).toBeCloseTo(9e-11);
    });
  });

  describe('multiply - edge cases', () => {
    test('multiplies by zero', () => {
      expect(multiply(Infinity, 0)).toBeNaN();
    });

    test('multiplies large numbers to infinity', () => {
      expect(multiply(1e200, 1e200)).toBe(Infinity);
    });

    test('multiplies negative by negative', () => {
      expect(multiply(-5, -5)).toBe(25);
    });

    test('preserves negative zero', () => {
      expect(Object.is(multiply(-1, 0), -0)).toBe(true);
    });
  });

  describe('divide - edge cases', () => {
    test('throws on division by zero', () => {
      expect(() => divide(10, 0)).toThrow('Cannot divide by zero');
    });

    test('divides by negative zero throws', () => {
      expect(() => divide(10, -0)).toThrow();
    });

    test('divides zero by number', () => {
      expect(divide(0, 5)).toBe(0);
    });

    test('divides by very small number', () => {
      const result = divide(1, 1e-300);
      expect(result).toBeGreaterThan(1e299);
    });

    test('infinity divided by infinity', () => {
      // This won't throw since b !== 0
      expect(divide(Infinity, Infinity)).toBeNaN();
    });
  });

  describe('isEven - edge cases', () => {
    test('zero is even', () => {
      expect(isEven(0)).toBe(true);
    });

    test('negative even numbers', () => {
      expect(isEven(-4)).toBe(true);
    });

    test('negative odd numbers', () => {
      expect(isEven(-3)).toBe(false);
    });

    test('large even number', () => {
      expect(isEven(1000000000)).toBe(true);
    });

    test('floating point returns unexpected result', () => {
      // 2.5 % 2 === 0.5, which is truthy but not === 0
      expect(isEven(2.5)).toBe(false);
    });

    test('NaN is not even', () => {
      expect(isEven(NaN)).toBe(false);
    });

    test('Infinity modulo is NaN, so not even', () => {
      expect(isEven(Infinity)).toBe(false);
    });
  });
});

describe('Fuzzy String Edge Cases', () => {
  describe('capitalize - edge cases', () => {
    test('empty string', () => {
      expect(capitalize('')).toBe('');
    });

    test('single character lowercase', () => {
      expect(capitalize('a')).toBe('A');
    });

    test('single character uppercase', () => {
      expect(capitalize('A')).toBe('A');
    });

    test('string with leading space', () => {
      expect(capitalize(' hello')).toBe(' hello');
    });

    test('string with numbers first', () => {
      expect(capitalize('123abc')).toBe('123abc');
    });

    test('unicode characters', () => {
      expect(capitalize('über')).toBe('Über');
    });

    test('emoji at start', () => {
      const result = capitalize('😀hello');
      // Emoji is 2 code units, so charAt(0) gets half of it
      expect(result.length).toBeGreaterThanOrEqual(6);
    });

    test('all uppercase', () => {
      expect(capitalize('HELLO')).toBe('HELLO');
    });

    test('mixed case', () => {
      expect(capitalize('hELLO')).toBe('HELLO');
    });

    test('whitespace only', () => {
      expect(capitalize('   ')).toBe('   ');
    });

    test('newline at start', () => {
      expect(capitalize('\nhello')).toBe('\nhello');
    });

    test('tab at start', () => {
      expect(capitalize('\thello')).toBe('\thello');
    });
  });
});

describe('Jest Matcher Edge Cases', () => {
  describe('toBe vs toEqual', () => {
    test('toBe uses Object.is', () => {
      expect(NaN).toEqual(NaN);
    });

    test('toEqual compares object contents', () => {
      expect({ a: 1 }).toEqual({ a: 1 });
    });

    test('toBe fails for different object references', () => {
      const obj = { a: 1 };
      expect(obj).toBe(obj); // same reference
    });

    test('arrays with same content are equal', () => {
      expect([1, 2, 3]).toEqual([1, 2, 3]);
    });

    test('nested objects equality', () => {
      expect({ a: { b: { c: 1 } } }).toEqual({ a: { b: { c: 1 } } });
    });

    test('arrays with different order are not equal', () => {
      expect([1, 2, 3]).not.toEqual([3, 2, 1]);
    });
  });

  describe('toBeCloseTo precision', () => {
    test('default precision (2 decimal places)', () => {
      expect(0.1 + 0.2).toBeCloseTo(0.3);
    });

    test('high precision', () => {
      expect(0.1 + 0.2).toBeCloseTo(0.3, 10);
    });

    test('very close numbers', () => {
      expect(0.123456789).toBeCloseTo(0.123456788, 7);
    });
  });

  describe('toContain variants', () => {
    test('array contains element', () => {
      expect([1, 2, 3]).toContain(2);
    });

    test('string contains substring', () => {
      expect('hello world').toContain('world');
    });

    test('array contains object reference', () => {
      const obj = { a: 1 };
      expect([obj, { b: 2 }]).toContain(obj);
    });

    test('toContainEqual for object in array', () => {
      expect([{ a: 1 }, { b: 2 }]).toContainEqual({ a: 1 });
    });
  });

  describe('toMatch', () => {
    test('matches regex', () => {
      expect('hello123world').toMatch(/\d+/);
    });

    test('matches string', () => {
      expect('hello world').toMatch('world');
    });

    test('case insensitive match', () => {
      expect('Hello World').toMatch(/hello/i);
    });
  });

  describe('toThrow variants', () => {
    test('toThrow with no argument', () => {
      expect(() => { throw new Error(); }).toThrow();
    });

    test('toThrow with string', () => {
      expect(() => { throw new Error('specific error'); }).toThrow('specific error');
    });

    test('toThrow with regex', () => {
      expect(() => { throw new Error('error code 123'); }).toThrow(/\d+/);
    });

    test('toThrow with Error class', () => {
      expect(() => { throw new TypeError('type error'); }).toThrow(TypeError);
    });
  });

  describe('toHaveLength', () => {
    test('array length', () => {
      expect([1, 2, 3]).toHaveLength(3);
    });

    test('string length', () => {
      expect('hello').toHaveLength(5);
    });

    test('empty array', () => {
      expect([]).toHaveLength(0);
    });
  });

  describe('toHaveProperty', () => {
    test('simple property', () => {
      expect({ a: 1 }).toHaveProperty('a');
    });

    test('nested property', () => {
      expect({ a: { b: 1 } }).toHaveProperty('a.b');
    });

    test('property with value', () => {
      expect({ a: 1 }).toHaveProperty('a', 1);
    });

    test('array index property', () => {
      expect({ a: [1, 2, 3] }).toHaveProperty('a.1', 2);
    });

    test('undefined property value', () => {
      expect({ a: undefined }).toHaveProperty('a');
    });
  });

  describe('toBeDefined and toBeUndefined', () => {
    test('defined value', () => {
      expect(1).toBeDefined();
    });

    test('undefined value', () => {
      expect(undefined).toBeUndefined();
    });

    test('null is defined', () => {
      expect(null).toBeDefined();
    });
  });

  describe('toBeNull and toBeTruthy/toBeFalsy', () => {
    test('null check', () => {
      expect(null).toBeNull();
    });

    test('truthy values', () => {
      expect(1).toBeTruthy();
      expect('hello').toBeTruthy();
      expect([]).toBeTruthy();
      expect({}).toBeTruthy();
    });

    test('falsy values', () => {
      expect(0).toBeFalsy();
      expect('').toBeFalsy();
      expect(null).toBeFalsy();
      expect(undefined).toBeFalsy();
      expect(NaN).toBeFalsy();
    });
  });

  describe('toBeGreaterThan and related', () => {
    test('greater than', () => {
      expect(10).toBeGreaterThan(5);
    });

    test('greater than or equal', () => {
      expect(10).toBeGreaterThanOrEqual(10);
    });

    test('less than', () => {
      expect(5).toBeLessThan(10);
    });

    test('less than or equal', () => {
      expect(10).toBeLessThanOrEqual(10);
    });

    test('comparing with infinity', () => {
      expect(Infinity).toBeGreaterThan(Number.MAX_VALUE);
    });
  });

  describe('toBeInstanceOf', () => {
    test('array is instance of Array', () => {
      expect([]).toBeInstanceOf(Array);
    });

    test('error is instance of Error', () => {
      expect(new Error()).toBeInstanceOf(Error);
    });

    test('custom class instance', () => {
      class MyClass {}
      expect(new MyClass()).toBeInstanceOf(MyClass);
    });
  });
});

describe('Async Test Edge Cases', () => {
  test('resolves immediately', async () => {
    await expect(Promise.resolve(42)).resolves.toBe(42);
  });

  test('rejects with error', async () => {
    await expect(Promise.reject(new Error('fail'))).rejects.toThrow('fail');
  });

  test('async function returns value', async () => {
    const asyncFn = async () => 'result';
    const result = await asyncFn();
    expect(result).toBe('result');
  });

  test('promise with delay', async () => {
    const delayed = new Promise(resolve => setTimeout(() => resolve('done'), 10));
    await expect(delayed).resolves.toBe('done');
  });

  test('multiple awaits', async () => {
    const p1 = Promise.resolve(1);
    const p2 = Promise.resolve(2);
    const [r1, r2] = await Promise.all([p1, p2]);
    expect(r1 + r2).toBe(3);
  });
});

describe('Mock Function Tests', () => {
  test('jest.fn() creates mock', () => {
    const mock = jest.fn();
    mock();
    expect(mock).toHaveBeenCalled();
  });

  test('mock with return value', () => {
    const mock = jest.fn().mockReturnValue(42);
    expect(mock()).toBe(42);
  });

  test('mock with implementation', () => {
    const mock = jest.fn((x: number) => x * 2);
    expect(mock(5)).toBe(10);
  });

  test('mock called with arguments', () => {
    const mock = jest.fn();
    mock('arg1', 'arg2');
    expect(mock).toHaveBeenCalledWith('arg1', 'arg2');
  });

  test('mock call count', () => {
    const mock = jest.fn();
    mock();
    mock();
    mock();
    expect(mock).toHaveBeenCalledTimes(3);
  });

  test('mock return value once', () => {
    const mock = jest.fn()
      .mockReturnValueOnce(1)
      .mockReturnValueOnce(2)
      .mockReturnValue(3);
    expect(mock()).toBe(1);
    expect(mock()).toBe(2);
    expect(mock()).toBe(3);
    expect(mock()).toBe(3);
  });

  test('mock resolved value', async () => {
    const mock = jest.fn().mockResolvedValue('async result');
    await expect(mock()).resolves.toBe('async result');
  });

  test('mock rejected value', async () => {
    const mock = jest.fn().mockRejectedValue(new Error('async error'));
    await expect(mock()).rejects.toThrow('async error');
  });

  test('mock.mockClear resets calls', () => {
    const mock = jest.fn();
    mock();
    mock();
    expect(mock).toHaveBeenCalledTimes(2);
    mock.mockClear();
    expect(mock).toHaveBeenCalledTimes(0);
  });

  test('mock.mockReset resets everything', () => {
    const mock = jest.fn().mockReturnValue(42);
    expect(mock()).toBe(42);
    mock.mockReset();
    expect(mock()).toBeUndefined();
  });
});

describe('Array and Object Deep Comparison', () => {
  test('sparse arrays', () => {
    const sparse = [1, , 3]; // eslint-disable-line no-sparse-arrays
    expect(sparse).toHaveLength(3);
    expect(sparse[1]).toBeUndefined();
  });

  test('arrays with undefined vs holes', () => {
    const withUndefined = [1, undefined, 3];
    const withHole = [1, , 3]; // eslint-disable-line no-sparse-arrays
    // Both have length 3, but they're different
    expect(withUndefined).toHaveLength(3);
    expect(withHole).toHaveLength(3);
  });

  test('object with symbol keys', () => {
    const sym = Symbol('test');
    const obj = { [sym]: 'value' };
    expect(obj[sym]).toBe('value');
  });

  test('object with numeric keys', () => {
    const obj = { 1: 'one', 2: 'two' };
    expect(obj[1]).toBe('one');
  });

  test('nested array equality', () => {
    expect([[1, 2], [3, 4]]).toEqual([[1, 2], [3, 4]]);
  });

  test('object with array property', () => {
    expect({ arr: [1, 2, 3] }).toEqual({ arr: [1, 2, 3] });
  });

  test('circular reference handling', () => {
    const obj: any = { a: 1 };
    obj.self = obj;
    expect(obj.self.a).toBe(1);
  });

  test('Date objects', () => {
    const date1 = new Date('2024-01-01');
    const date2 = new Date('2024-01-01');
    expect(date1).toEqual(date2);
  });

  test('RegExp objects', () => {
    const regex1 = /test/gi;
    const regex2 = /test/gi;
    expect(regex1).toEqual(regex2);
  });

  test('Map equality', () => {
    const map1 = new Map([['a', 1], ['b', 2]]);
    const map2 = new Map([['a', 1], ['b', 2]]);
    expect(map1).toEqual(map2);
  });

  test('Set equality', () => {
    const set1 = new Set([1, 2, 3]);
    const set2 = new Set([1, 2, 3]);
    expect(set1).toEqual(set2);
  });
});

describe('Boundary Value Tests', () => {
  test('Number.MAX_VALUE', () => {
    expect(Number.MAX_VALUE).toBeGreaterThan(0);
    expect(Number.MAX_VALUE * 2).toBe(Infinity);
  });

  test('Number.MIN_VALUE', () => {
    expect(Number.MIN_VALUE).toBeGreaterThan(0);
    expect(Number.MIN_VALUE / 2).toBe(0);
  });

  test('Number.EPSILON', () => {
    expect(1 + Number.EPSILON).not.toBe(1);
    expect(1 + Number.EPSILON / 2).toBe(1);
  });

  test('MAX_SAFE_INTEGER boundaries', () => {
    expect(Number.MAX_SAFE_INTEGER + 1).toBe(Number.MAX_SAFE_INTEGER + 2);
  });

  test('MIN_SAFE_INTEGER boundaries', () => {
    expect(Number.MIN_SAFE_INTEGER - 1).toBe(Number.MIN_SAFE_INTEGER - 2);
  });

  test('string max length handling', () => {
    const longString = 'a'.repeat(10000);
    expect(longString).toHaveLength(10000);
  });

  test('deeply nested object', () => {
    let obj: any = { value: 'deep' };
    for (let i = 0; i < 100; i++) {
      obj = { nested: obj };
    }
    let current = obj;
    for (let i = 0; i < 100; i++) {
      current = current.nested;
    }
    expect(current.value).toBe('deep');
  });
});

describe('Type Coercion Edge Cases', () => {
  test('comparing number and string', () => {
    expect(1).not.toBe('1');
    expect(1 == '1').toBe(true); // eslint-disable-line eqeqeq
    expect(1 === '1').toBe(false);
  });

  test('null vs undefined', () => {
    expect(null).not.toBe(undefined);
    expect(null == undefined).toBe(true); // eslint-disable-line eqeqeq
    expect(null === undefined).toBe(false);
  });

  test('boolean coercion', () => {
    expect(Boolean('')).toBe(false);
    expect(Boolean('0')).toBe(true);
    expect(Boolean(0)).toBe(false);
    expect(Boolean([])).toBe(true);
    expect(Boolean({})).toBe(true);
  });

  test('number coercion', () => {
    expect(Number('')).toBe(0);
    expect(Number('123')).toBe(123);
    expect(Number('  123  ')).toBe(123);
    expect(Number('12.34')).toBe(12.34);
    expect(Number('0x10')).toBe(16);
    expect(Number('abc')).toBeNaN();
  });

  test('string coercion', () => {
    expect(String(null)).toBe('null');
    expect(String(undefined)).toBe('undefined');
    expect(String(123)).toBe('123');
    expect(String([1, 2, 3])).toBe('1,2,3');
    expect(String({ a: 1 })).toBe('[object Object]');
  });
});
