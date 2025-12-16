// Module 17 tests - testing various utility functions

describe('Module 17 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(17 + 1).toBe(18);
  });

  test('subtraction works correctly', () => {
    expect(17 - 1).toBe(16);
  });

  test('multiplication works correctly', () => {
    expect(17 * 2).toBe(34);
  });

  test('division works correctly', () => {
    expect(17 * 2 / 2).toBe(17);
  });

  test('modulo works correctly', () => {
    expect(17 % 3).toBe(2);
  });
});

describe('Module 17 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '17').toBe('module17');
  });

  test('string length', () => {
    expect('module17'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module17'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
