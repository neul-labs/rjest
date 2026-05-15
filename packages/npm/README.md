# rjest-install

[![npm version](https://img.shields.io/npm/v/rjest-install.svg)](https://www.npmjs.com/package/rjest-install)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![CI](https://github.com/neul-labs/rjest/actions/workflows/ci.yml/badge.svg)](https://github.com/neul-labs/rjest/actions/workflows/ci.yml)

> **A blazing-fast, drop-in replacement for Jest.** Install via npm and run your tests in ~14ms on warm runs — 100x faster than standard Jest.

`rjest-install` is the official npm distribution of **rjest**, a Rust-based test runner that keeps a background daemon (`jestd`) alive across invocations. It reads your existing `jest.config.*` files with zero configuration changes and supports the same CLI flags you already use (`--watch`, `--coverage`, `--runInBand`, `--testNamePattern`, `--json`, `--machine`).

## Why rjest?

- **100x faster warm runs** — 14ms vs 1.4s on typical TypeScript projects
- **Zero config** — reads `jest.config.js`, `jest.config.ts`, or `package.json` Jest settings automatically
- **Drop-in replacement** — same CLI flags, test syntax, matchers, and snapshot format
- **Built for AI agents** — structured JSON and `--machine` output for fast feedback loops
- **Persistent caching** — SWC transforms cached on disk via `sled`; survive restarts

## Installation

```bash
npm install -D rjest-install
```

Requires Node.js 16+ and a Rust binary (downloaded automatically on first run).

## Quick Start

```bash
# Run all tests (daemon starts automatically)
npx rjest

# Watch mode
npx rjest --watch

# Coverage
npx rjest --coverage

# Filter by test name
npx rjest --testNamePattern="add"

# Structured JSON output for CI / AI agents
npx rjest --json
npx rjest --machine
```

## How It Works

1. **Daemon (`jestd`)** — a Rust process that runs in the background, parsing Jest config once, building a dependency graph, and pre-warming Node.js workers.
2. **SWC Transforms** — TypeScript/JSX is compiled natively in Rust and cached by content hash (blake3). No Babel or `ts-jest` needed.
3. **Worker Pool** — persistent Node.js processes execute tests in a VM context, avoiding repeated V8 cold-start overhead.
4. **CLI Shim** — forwards commands to the daemon over low-latency IPC (`nng`), then renders Jest-style output.

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

## Daemon Management

```bash
# Check if the daemon is running
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

## Performance

| Metric | rjest | Jest | Speedup |
| --- | --- | --- | --- |
| Cold start | 1.9s | 1.4s | 0.7x |
| Warm run | **14ms** | 1.4s | **100x** |

## AI Agent Usage

```bash
# Fast, structured output for automated workflows
npx rjest --onlyChanged --machine

# Filter by test name pattern
npx rjest --testNamePattern="authentication" --json
```

## Documentation

- [Full Documentation](https://docs.neullabs.com/rjest)
- [Architecture](https://github.com/neul-labs/rjest/blob/main/docs/architecture.md)
- [Compatibility](https://github.com/neul-labs/rjest/blob/main/docs/compatibility.md)
- [Performance](https://github.com/neul-labs/rjest/blob/main/docs/performance.md)
- [Changelog](https://github.com/neul-labs/rjest/blob/main/CHANGELOG.md)

## Contributing

Contributions are welcome! Please open an issue or PR on the [main repository](https://github.com/neul-labs/rjest).

## License

MIT
