describe('Package B', () => {
  test('should concatenate strings', () => {
    expect('hello ' + 'world').toBe('hello world');
  });

  test('should check string length', () => {
    expect('test'.length).toBe(4);
  });

  test('should convert to uppercase', () => {
    expect('test'.toUpperCase()).toBe('TEST');
  });
});
