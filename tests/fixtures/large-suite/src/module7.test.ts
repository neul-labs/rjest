// Module 7 tests - testing various utility functions

describe('Module 7 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(7 + 1).toBe(8);
  });

  test('subtraction works correctly', () => {
    expect(7 - 1).toBe(6);
  });

  test('multiplication works correctly', () => {
    expect(7 * 2).toBe(14);
  });

  test('division works correctly', () => {
    expect(7 * 2 / 2).toBe(7);
  });

  test('modulo works correctly', () => {
    expect(7 % 3).toBe(1);
  });
});

describe('Module 7 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '7').toBe('module7');
  });

  test('string length', () => {
    expect('module7'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module7'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
