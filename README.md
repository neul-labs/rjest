# rjest

[![Crates.io](https://img.shields.io/crates/v/jestd.svg)](https://crates.io/crates/jestd)
[![npm](https://img.shields.io/npm/v/rjest-install.svg)](https://www.npmjs.com/package/rjest-install)
[![PyPI](https://img.shields.io/pypi/v/rjest-install.svg)](https://pypi.org/project/rjest-install/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![CI](https://github.com/neul-labs/rjest/actions/workflows/ci.yml/badge.svg)](https://github.com/neul-labs/rjest/actions/workflows/ci.yml)
[![Documentation](https://img.shields.io/badge/docs-neullabs.com-blue)](https://docs.neullabs.com/rjest)

> **A blazing-fast, drop-in replacement for Jest.** Warm runs complete in ~14ms — **100x faster** than standard Jest. Zero configuration changes required.

`rjest` keeps a Rust daemon (`jestd`) running in the background, caching SWC transforms and pre-warming Node.js workers across test invocations. It reads your existing `jest.config.*` files with **zero config changes** and supports the same CLI flags you already use: `--watch`, `--coverage`, `--runInBand`, `--testNamePattern`, `--json`, `--machine`.

## Installation

```bash
# npm (recommended for Node.js projects)
npm install -D rjest-install

# Homebrew (macOS / Linux)
brew tap neul-labs/tap
brew install rjest

# Cargo (Rust toolchain)
cargo install rjest

# pip (Python ecosystem)
pip install rjest-install
```

See [CHANGELOG.md](CHANGELOG.md) for release notes.

## Quick Start

```bash
# Run all tests — daemon starts automatically on first use
npx rjest

# Watch mode
npx rjest --watch

# Coverage
npx rjest --coverage

# Filter by test name
npx rjest --testNamePattern="add"

# Structured JSON / machine output for CI & AI agents
npx rjest --json
npx rjest --machine
```

## Why rjest?

| Metric | rjest | Jest | Speedup |
| --- | --- | --- | --- |
| Cold start | 1.9s | 1.4s | 0.7x |
| **Warm run** | **~14ms** | 1.4s | **~100x** |

- **100x faster warm runs** — amortized config parsing, transform caching, and worker pooling
- **Zero config** — reads `jest.config.js`, `jest.config.ts`, or `package.json` Jest settings automatically
- **Drop-in replacement** — same CLI flags, test syntax, matchers, snapshots, and coverage
- **Built for AI agents** — `--json` and `--machine` flags provide structured, parse-friendly output
- **Persistent caching** — SWC transforms cached on disk via `sled`; survive daemon restarts

## Architecture

```
┌─────────────┐     IPC (nng)      ┌──────────────┐
│   jest CLI  │  ◄──────────────►  │    jestd     │
│  (rjest)    │                    │   (Rust)     │
└─────────────┘                    └──────┬───────┘
                                        │
                     ┌──────────────────┼──────────────────┐
                     │                  │                  │
                ┌────▼────┐      ┌────▼────┐      ┌────▼────┐
                │ Worker 1│      │ Worker 2│      │ Worker N│
                │ (Node)  │      │ (Node)  │      │ (Node)  │
                └─────────┘      └─────────┘      └─────────┘
```

1. **Rust daemon (`jestd`)** — parses Jest config once, builds a dependency graph, watches the filesystem, and maintains a pool of warm Node.js workers.
2. **SWC Transforms** — TypeScript/JSX compiled natively in Rust and cached by content hash (blake3). No Babel or `ts-jest` required.
3. **Worker Pool** — persistent Node.js processes execute tests in a VM context, avoiding repeated V8 cold-start overhead.
4. **CLI Shim** — forwards commands to the daemon over low-latency IPC (`nng`), then renders Jest-style output.

## Daemon Management

```bash
# Check daemon status
npx rjest --daemon-status

# Stop the daemon (caches persist on disk)
npx rjest --daemon-stop

# Force a cold restart
npx rjest --daemon-restart
```

## Compatibility

- **Node.js**: 16+
- **Platforms**: macOS (Intel & Apple Silicon), Linux (x86_64 & aarch64), Windows (x86_64)
- **Config files**: `jest.config.js`, `jest.config.ts`, `jest.config.mjs`, `package.json`
- **Matchers**: `toBe`, `toEqual`, `toThrow`, `toHaveBeenCalled`, `resolves`, `rejects`, etc.
- **Features**: snapshots, fake timers, `jest.fn()`, `jest.mock()`, coverage (Istanbul), watch mode

See the [compatibility matrix](https://github.com/neul-labs/rjest/blob/main/docs/compatibility.md) for full details.

## AI Agent Usage

```bash
# Fast, structured output for automated workflows
npx rjest --onlyChanged --machine

# Filter by test name pattern
npx rjest --testNamePattern="authentication" --json
```

### Why agents benefit

- **14ms feedback loops** — warm runs return results in ~14 milliseconds instead of seconds, enabling rapid edit-test cycles
- **Structured output** — `--json` and `--machine` flags provide parse-friendly results with file paths, test names, durations, and error details
- **Selective execution** — run only relevant tests by name pattern or file path
- **Session continuity** — the daemon maintains state across invocations, so agents don't pay cold-start costs repeatedly

## Technology Choices

- **nng (nanomsg-next-gen)** — low-latency IPC between CLI and daemon via Unix domain sockets
- **SWC** — native Rust TypeScript/JSX compilation, 10-100x faster than Babel
- **sled** — embedded disk cache for transforms keyed by content hash (blake3)
- **rayon** — parallelizes file transforms across CPU cores

## Documentation

- [Full Documentation](https://docs.neullabs.com/rjest)
- [Architecture](https://github.com/neul-labs/rjest/blob/main/docs/architecture.md)
- [Compatibility](https://github.com/neul-labs/rjest/blob/main/docs/compatibility.md)
- [Performance](https://github.com/neul-labs/rjest/blob/main/docs/performance.md)
- [Changelog](https://github.com/neul-labs/rjest/blob/main/CHANGELOG.md)

## Contributing

Contributions are welcome! Please open an issue to discuss significant changes before submitting a PR.

```bash
# Clone and build
git clone https://github.com/neul-labs/rjest.git
cd rjest
cargo build

# Run tests
cargo test
```

## License

MIT
