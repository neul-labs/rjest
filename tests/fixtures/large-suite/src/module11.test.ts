// Module 11 tests - testing various utility functions

describe('Module 11 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(11 + 1).toBe(12);
  });

  test('subtraction works correctly', () => {
    expect(11 - 1).toBe(10);
  });

  test('multiplication works correctly', () => {
    expect(11 * 2).toBe(22);
  });

  test('division works correctly', () => {
    expect(11 * 2 / 2).toBe(11);
  });

  test('modulo works correctly', () => {
    expect(11 % 3).toBe(2);
  });
});

describe('Module 11 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '11').toBe('module11');
  });

  test('string length', () => {
    expect('module11'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module11'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
