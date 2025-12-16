const { add, subtract } = require('./math');

describe('Math operations', () => {
  test('add returns sum of two numbers', () => {
    expect(add(1, 2)).toBe(3);
  });

  test('subtract returns difference of two numbers', () => {
    expect(subtract(5, 3)).toBe(2);
  });
});
