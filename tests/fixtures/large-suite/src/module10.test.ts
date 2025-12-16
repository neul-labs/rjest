// Module 10 tests - testing various utility functions

describe('Module 10 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(10 + 1).toBe(11);
  });

  test('subtraction works correctly', () => {
    expect(10 - 1).toBe(9);
  });

  test('multiplication works correctly', () => {
    expect(10 * 2).toBe(20);
  });

  test('division works correctly', () => {
    expect(10 * 2 / 2).toBe(10);
  });

  test('modulo works correctly', () => {
    expect(10 % 3).toBe(1);
  });
});

describe('Module 10 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '10').toBe('module10');
  });

  test('string length', () => {
    expect('module10'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module10'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
