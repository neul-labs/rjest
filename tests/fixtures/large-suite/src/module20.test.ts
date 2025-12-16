// Module 20 tests - testing various utility functions

describe('Module 20 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(20 + 1).toBe(21);
  });

  test('subtraction works correctly', () => {
    expect(20 - 1).toBe(19);
  });

  test('multiplication works correctly', () => {
    expect(20 * 2).toBe(40);
  });

  test('division works correctly', () => {
    expect(20 * 2 / 2).toBe(20);
  });

  test('modulo works correctly', () => {
    expect(20 % 3).toBe(2);
  });
});

describe('Module 20 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '20').toBe('module20');
  });

  test('string length', () => {
    expect('module20'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module20'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
