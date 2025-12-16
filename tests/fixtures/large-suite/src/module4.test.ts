// Module 4 tests - testing various utility functions

describe('Module 4 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(4 + 1).toBe(5);
  });

  test('subtraction works correctly', () => {
    expect(4 - 1).toBe(3);
  });

  test('multiplication works correctly', () => {
    expect(4 * 2).toBe(8);
  });

  test('division works correctly', () => {
    expect(4 * 2 / 2).toBe(4);
  });

  test('modulo works correctly', () => {
    expect(4 % 3).toBe(1);
  });
});

describe('Module 4 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '4').toBe('module4');
  });

  test('string length', () => {
    expect('module4'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module4'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
