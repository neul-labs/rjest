describe('failing tests', () => {
  it('should fail when expectation is wrong', () => {
    expect(1 + 1).toBe(3);
  });

  it('should fail on string mismatch', () => {
    expect('hello').toBe('world');
  });
});

describe('async tests', () => {
  it('should handle async operations', async () => {
    const result = await Promise.resolve(42);
    expect(result).toBe(42);
  });
});
