# Installation

## Requirements

- **Rust** 1.70 or later (for building from source)
- **Node.js** 18 or later (for running tests)
- **npm** or **yarn** (for project dependencies)

## Install from Source

Currently, rjest is installed by building from source:

```bash
# Clone the repository
git clone https://github.com/user/rjest.git
cd rjest

# Build release binaries
cargo build --release

# The binaries are in target/release/
# - jest    (CLI)
# - jestd   (daemon)
```

### Add to PATH

Add the binaries to your PATH for easy access:

```bash
# Option 1: Symlink to local bin
ln -s $(pwd)/target/release/jest ~/.local/bin/rjest
ln -s $(pwd)/target/release/jestd ~/.local/bin/rjestd

# Option 2: Add to PATH in your shell config
export PATH="$PATH:/path/to/rjest/target/release"
```

## Verify Installation

```bash
# Check version
jest --version

# Run help
jest --help
```

## Project Setup

rjest works with existing Jest projects. Ensure your project has:

### 1. A Jest Configuration

```javascript title="jest.config.js"
module.exports = {
  testMatch: ['**/*.test.ts', '**/*.test.js'],
  moduleFileExtensions: ['ts', 'tsx', 'js', 'jsx'],
};
```

### 2. Required Dependencies

```bash
# TypeScript projects need these peer dependencies
npm install --save-dev typescript @types/jest
```

!!! note "No Babel Required"
    Unlike Jest with `ts-jest` or `@swc/jest`, rjest has built-in TypeScript
    compilation using native SWC. You don't need to configure any transform.

## Daemon Management

rjest runs a background daemon for fast subsequent runs:

```bash
# The daemon starts automatically on first run
jest

# Check daemon status
jest --daemon-status

# Stop the daemon (frees memory)
jest --daemon-stop
```

## Next Steps

- [Quick Start](quick-start.md) - Write and run your first test
- [Migration Guide](migration.md) - Migrate an existing Jest project
