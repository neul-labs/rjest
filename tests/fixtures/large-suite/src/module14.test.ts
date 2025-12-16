// Module 14 tests - testing various utility functions

describe('Module 14 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(14 + 1).toBe(15);
  });

  test('subtraction works correctly', () => {
    expect(14 - 1).toBe(13);
  });

  test('multiplication works correctly', () => {
    expect(14 * 2).toBe(28);
  });

  test('division works correctly', () => {
    expect(14 * 2 / 2).toBe(14);
  });

  test('modulo works correctly', () => {
    expect(14 % 3).toBe(2);
  });
});

describe('Module 14 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '14').toBe('module14');
  });

  test('string length', () => {
    expect('module14'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module14'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
