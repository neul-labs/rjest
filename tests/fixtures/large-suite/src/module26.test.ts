// Module 26 tests - testing various utility functions

describe('Module 26 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(26 + 1).toBe(27);
  });

  test('subtraction works correctly', () => {
    expect(26 - 1).toBe(25);
  });

  test('multiplication works correctly', () => {
    expect(26 * 2).toBe(52);
  });

  test('division works correctly', () => {
    expect(26 * 2 / 2).toBe(26);
  });

  test('modulo works correctly', () => {
    expect(26 % 3).toBe(2);
  });
});

describe('Module 26 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '26').toBe('module26');
  });

  test('string length', () => {
    expect('module26'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module26'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
