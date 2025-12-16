// Snapshot testing
describe('Snapshots', () => {
  test('should match a simple object snapshot', () => {
    const obj = {
      name: 'test-updated',  // Changed value
      value: 100,            // Changed value
      nested: {
        a: 1,
        b: 2,
      },
    };
    expect(obj).toMatchSnapshot();
  });

  test('should match a string snapshot', () => {
    const str = 'Hello, World!';
    expect(str).toMatchSnapshot();
  });

  test('should match an array snapshot', () => {
    const arr = [1, 2, 3, 'four', { five: 5 }];
    expect(arr).toMatchSnapshot();
  });

  test('should match multiple snapshots in same test', () => {
    expect('first').toMatchSnapshot();
    expect('second').toMatchSnapshot();
    expect({ third: 3 }).toMatchSnapshot();
  });

  test('should match inline snapshot', () => {
    const value = { foo: 'bar' };
    expect(value).toMatchInlineSnapshot(`{
  "foo": "bar"
}`);
  });
});
