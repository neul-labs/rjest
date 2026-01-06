describe('simple tests', () => {
  it('should pass', () => {
    expect(1 + 1).toBe(2);
  });

  it('should handle strings', () => {
    expect('hello').toContain('hell');
  });
});

describe('math utilities', () => {
  it('should add numbers correctly', () => {
    const result = 5 + 3;
    expect(result).toBe(8);
  });

  it('should multiply numbers correctly', () => {
    const result = 4 * 7;
    expect(result).toBe(28);
  });
});
