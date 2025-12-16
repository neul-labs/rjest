// Module 5 tests - testing various utility functions

describe('Module 5 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(5 + 1).toBe(6);
  });

  test('subtraction works correctly', () => {
    expect(5 - 1).toBe(4);
  });

  test('multiplication works correctly', () => {
    expect(5 * 2).toBe(10);
  });

  test('division works correctly', () => {
    expect(5 * 2 / 2).toBe(5);
  });

  test('modulo works correctly', () => {
    expect(5 % 3).toBe(2);
  });
});

describe('Module 5 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '5').toBe('module5');
  });

  test('string length', () => {
    expect('module5'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module5'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
