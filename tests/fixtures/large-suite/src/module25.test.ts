// Module 25 tests - testing various utility functions

describe('Module 25 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(25 + 1).toBe(26);
  });

  test('subtraction works correctly', () => {
    expect(25 - 1).toBe(24);
  });

  test('multiplication works correctly', () => {
    expect(25 * 2).toBe(50);
  });

  test('division works correctly', () => {
    expect(25 * 2 / 2).toBe(25);
  });

  test('modulo works correctly', () => {
    expect(25 % 3).toBe(1);
  });
});

describe('Module 25 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '25').toBe('module25');
  });

  test('string length', () => {
    expect('module25'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module25'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
