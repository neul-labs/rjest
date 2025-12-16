// Module 21 tests - testing various utility functions

describe('Module 21 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(21 + 1).toBe(22);
  });

  test('subtraction works correctly', () => {
    expect(21 - 1).toBe(20);
  });

  test('multiplication works correctly', () => {
    expect(21 * 2).toBe(42);
  });

  test('division works correctly', () => {
    expect(21 * 2 / 2).toBe(21);
  });

  test('modulo works correctly', () => {
    expect(21 % 3).toBe(0);
  });
});

describe('Module 21 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '21').toBe('module21');
  });

  test('string length', () => {
    expect('module21'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module21'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
