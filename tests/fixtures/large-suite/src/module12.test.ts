// Module 12 tests - testing various utility functions

describe('Module 12 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(12 + 1).toBe(13);
  });

  test('subtraction works correctly', () => {
    expect(12 - 1).toBe(11);
  });

  test('multiplication works correctly', () => {
    expect(12 * 2).toBe(24);
  });

  test('division works correctly', () => {
    expect(12 * 2 / 2).toBe(12);
  });

  test('modulo works correctly', () => {
    expect(12 % 3).toBe(0);
  });
});

describe('Module 12 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '12').toBe('module12');
  });

  test('string length', () => {
    expect('module12'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module12'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
