// Module 22 tests - testing various utility functions

describe('Module 22 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(22 + 1).toBe(23);
  });

  test('subtraction works correctly', () => {
    expect(22 - 1).toBe(21);
  });

  test('multiplication works correctly', () => {
    expect(22 * 2).toBe(44);
  });

  test('division works correctly', () => {
    expect(22 * 2 / 2).toBe(22);
  });

  test('modulo works correctly', () => {
    expect(22 % 3).toBe(1);
  });
});

describe('Module 22 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '22').toBe('module22');
  });

  test('string length', () => {
    expect('module22'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module22'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
