// Module 3 tests - testing various utility functions

describe('Module 3 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(3 + 1).toBe(4);
  });

  test('subtraction works correctly', () => {
    expect(3 - 1).toBe(2);
  });

  test('multiplication works correctly', () => {
    expect(3 * 2).toBe(6);
  });

  test('division works correctly', () => {
    expect(3 * 2 / 2).toBe(3);
  });

  test('modulo works correctly', () => {
    expect(3 % 3).toBe(0);
  });
});

describe('Module 3 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '3').toBe('module3');
  });

  test('string length', () => {
    expect('module3'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module3'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
