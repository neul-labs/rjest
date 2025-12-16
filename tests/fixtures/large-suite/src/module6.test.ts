// Module 6 tests - testing various utility functions

describe('Module 6 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(6 + 1).toBe(7);
  });

  test('subtraction works correctly', () => {
    expect(6 - 1).toBe(5);
  });

  test('multiplication works correctly', () => {
    expect(6 * 2).toBe(12);
  });

  test('division works correctly', () => {
    expect(6 * 2 / 2).toBe(6);
  });

  test('modulo works correctly', () => {
    expect(6 % 3).toBe(0);
  });
});

describe('Module 6 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '6').toBe('module6');
  });

  test('string length', () => {
    expect('module6'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module6'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
