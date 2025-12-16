// Module 8 tests - testing various utility functions

describe('Module 8 - Math Operations', () => {
  test('addition works correctly', () => {
    expect(8 + 1).toBe(9);
  });

  test('subtraction works correctly', () => {
    expect(8 - 1).toBe(7);
  });

  test('multiplication works correctly', () => {
    expect(8 * 2).toBe(16);
  });

  test('division works correctly', () => {
    expect(8 * 2 / 2).toBe(8);
  });

  test('modulo works correctly', () => {
    expect(8 % 3).toBe(2);
  });
});

describe('Module 8 - String Operations', () => {
  test('string concatenation', () => {
    expect('module' + '8').toBe('module8');
  });

  test('string length', () => {
    expect('module8'.length).toBeGreaterThan(0);
  });

  test('string includes', () => {
    expect('module8'.includes('module')).toBe(true);
  });

  test('string split', () => {
    expect('a,b,c'.split(',')).toEqual(['a', 'b', 'c']);
  });

  test('string trim', () => {
    expect('  test  '.trim()).toBe('test');
  });
});
