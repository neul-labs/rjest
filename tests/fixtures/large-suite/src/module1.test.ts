// Module 1 tests - testing various utility functions

describe('Module 1 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(1 + 1).toBe(2);
  });

  test('subtraction works correctly', () => {
    expect(1 - 1).toBe(0);
  });

  test('multiplication works correctly', () => {
    expect(1 * 2).toBe(2);
  });

  test('division works correctly', () => {
    expect(1 * 2 / 2).toBe(1);
  });

  test('modulo works correctly', () => {
    expect(1 % 3).toBe(1);
  });
});

describe('Module 1 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '1').toBe('module1');
  });

  test('string length', () => {
    expect('module1'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module1'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
