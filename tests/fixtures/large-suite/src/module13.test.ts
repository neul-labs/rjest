// Module 13 tests - testing various utility functions

describe('Module 13 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(13 + 1).toBe(14);
  });

  test('subtraction works correctly', () => {
    expect(13 - 1).toBe(12);
  });

  test('multiplication works correctly', () => {
    expect(13 * 2).toBe(26);
  });

  test('division works correctly', () => {
    expect(13 * 2 / 2).toBe(13);
  });

  test('modulo works correctly', () => {
    expect(13 % 3).toBe(1);
  });
});

describe('Module 13 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '13').toBe('module13');
  });

  test('string length', () => {
    expect('module13'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module13'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
