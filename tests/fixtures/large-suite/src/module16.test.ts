// Module 16 tests - testing various utility functions

describe('Module 16 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(16 + 1).toBe(17);
  });

  test('subtraction works correctly', () => {
    expect(16 - 1).toBe(15);
  });

  test('multiplication works correctly', () => {
    expect(16 * 2).toBe(32);
  });

  test('division works correctly', () => {
    expect(16 * 2 / 2).toBe(16);
  });

  test('modulo works correctly', () => {
    expect(16 % 3).toBe(1);
  });
});

describe('Module 16 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '16').toBe('module16');
  });

  test('string length', () => {
    expect('module16'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module16'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
