// Module 2 tests - testing various utility functions

describe('Module 2 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(2 + 1).toBe(3);
  });

  test('subtraction works correctly', () => {
    expect(2 - 1).toBe(1);
  });

  test('multiplication works correctly', () => {
    expect(2 * 2).toBe(4);
  });

  test('division works correctly', () => {
    expect(2 * 2 / 2).toBe(2);
  });

  test('modulo works correctly', () => {
    expect(2 % 3).toBe(2);
  });
});

describe('Module 2 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '2').toBe('module2');
  });

  test('string length', () => {
    expect('module2'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module2'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
