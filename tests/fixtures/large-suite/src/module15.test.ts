// Module 15 tests - testing various utility functions

describe('Module 15 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(15 + 1).toBe(16);
  });

  test('subtraction works correctly', () => {
    expect(15 - 1).toBe(14);
  });

  test('multiplication works correctly', () => {
    expect(15 * 2).toBe(30);
  });

  test('division works correctly', () => {
    expect(15 * 2 / 2).toBe(15);
  });

  test('modulo works correctly', () => {
    expect(15 % 3).toBe(0);
  });
});

describe('Module 15 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '15').toBe('module15');
  });

  test('string length', () => {
    expect('module15'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module15'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
