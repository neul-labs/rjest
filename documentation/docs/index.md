# rjest

**A blazing-fast Jest-compatible test runner written in Rust.**

rjest is a drop-in replacement for Jest that runs your existing test suites up to **95x faster** through a persistent daemon architecture and native TypeScript compilation.

## Why rjest?

| Feature | rjest | Jest |
|---------|-------|------|
| Warm run (136 tests) | ~100ms | ~14,000ms |
| Cold start | ~9s | ~14s |
| TypeScript support | Native (SWC) | Babel |
| Architecture | Persistent daemon | Process per run |

## Key Features

- **Jest Compatible** - Works with your existing Jest tests, configuration, and matchers
- **95x Faster** - Warm runs complete in milliseconds instead of seconds
- **Native TypeScript** - Built-in SWC compiler, no Babel configuration needed
- **Snapshot Testing** - Full compatibility with Jest snapshots
- **Mock Functions** - Complete `jest.fn()` and `jest.spyOn()` support
- **Async Testing** - Promises, async/await, and fake timers

## Quick Start

```bash
# Install rjest
cargo install rjest

# Run your tests (drop-in Jest replacement)
rjest

# Or use the jest alias
jest
```

## How It Works

rjest uses a daemon architecture that keeps Node.js workers warm between test runs:

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   CLI       │────▶│   Daemon    │────▶│   Workers   │
│  (Rust)     │ IPC │   (Rust)    │     │  (Node.js)  │
└─────────────┘     └─────────────┘     └─────────────┘
                           │
                    ┌──────┴──────┐
                    │  SWC Cache  │
                    │   (Rust)    │
                    └─────────────┘
```

1. **First run**: Daemon starts, compiles TypeScript, warms up workers
2. **Subsequent runs**: Instant execution with cached transforms and warm V8

## Documentation

- [Installation](getting-started/installation.md) - How to install rjest
- [Quick Start](getting-started/quick-start.md) - Get up and running in minutes
- [Migration Guide](getting-started/migration.md) - Migrate from Jest
- [CLI Reference](reference/cli.md) - Command-line options
- [Configuration](reference/configuration.md) - jest.config.js options

## License

MIT License - see LICENSE file for details.
