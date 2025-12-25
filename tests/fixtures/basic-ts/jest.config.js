module.exports = {
  testEnvironment: 'node',
  testMatch: ['**/*.test.ts'],
  moduleFileExtensions: ['ts', 'tsx', 'js', 'jsx', 'json'],
  // Use @swc/jest for faster transforms (works with both Jest and rjest)
  transform: {
    '^.+\\.(t|j)sx?$': '@swc/jest',
  },
};
