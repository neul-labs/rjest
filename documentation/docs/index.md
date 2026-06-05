# rjest

**A blazing-fast, drop-in replacement for Jest.**

rjest keeps a Rust daemon (`jestd`) running in the background, caching SWC
transforms and pre-warming Node.js workers across test invocations. It reads
your existing `jest.config.*` files with **zero config changes** and supports
the same CLI flags you already use: `--watch`, `--coverage`, `--runInBand`,
`--testNamePattern`, `--json`, `--machine`.

On the project's benchmark suite (136 TypeScript tests across 3 files in
`tests/fixtures/basic-ts`), warm runs complete in **~100ms** vs ~14,200ms for
upstream Jest — a **~95x** speedup. See [BENCHMARK.md][bench] for raw numbers.

[bench]: https://github.com/neul-labs/rjest/blob/main/BENCHMARK.md

## Why rjest?

| Metric | rjest | Jest | Speedup |
|---|---|---|---|
| Cold start (136 tests) | 9.2s | 14.2s | 1.5x |
| **Warm run (136 tests)** | **~100ms** | ~14.2s | **~95x** |
| Daemon resident memory | ~17 MB | — | — |
| Total memory (daemon + 4 workers) | ~212 MB | ~108 MB | +96% |

## Key Features

- **Jest-compatible** — same CLI flags, matchers, snapshots, hooks, and `jest.*` globals
- **Zero config** — reads `jest.config.{js,ts,mjs,json}` or the `jest` key in `package.json`
- **Native TypeScript / JSX** — compiled in Rust via SWC; no Babel or `ts-jest` required
- **Persistent transform cache** — content-hashed (blake3) results stored on disk in `sled`; survive daemon restarts
- **Pre-warmed worker pool** — up to 4 Node.js workers stay hot across runs (idle workers recycled after 60s, all workers recycled after 1000 tests)
- **Structured output** — `--json` and `--machine` flags for CI pipelines and AI agents
- **Drop-in fallback** — `--fallback-to-jest` (or `RJEST_FALLBACK=1`) forwards to upstream Jest for unsupported edge cases

## Install

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

See [Installation](getting-started/installation.md) for full instructions.

## Quick Start

```bash
# Run all tests — daemon starts automatically on first use
npx rjest

# Watch mode
npx rjest --watch

# Filter by test name
npx rjest --testNamePattern="add"

# Structured output for CI and AI agents
npx rjest --json
npx rjest --machine
```

## How it works

```
┌─────────────┐     IPC (nng over           ┌──────────────┐
│   jest CLI  │     Unix socket)            │    jestd     │
│  (rjest)    │ ◄──────────────────────────► │   (Rust)     │
└─────────────┘                              └──────┬───────┘
                                                    │
                          ┌─────────────────────────┼─────────────────────────┐
                          │                         │                         │
                     ┌────▼────┐               ┌────▼────┐               ┌────▼────┐
                     │ Worker 1│   ...         │ Worker 2│   ...         │ Worker N│
                     │ (Node)  │               │ (Node)  │               │ (Node)  │
                     └─────────┘               └─────────┘               └─────────┘
```

1. **First run** — daemon spawns, parses config via Node, discovers tests, compiles files with SWC, warms workers.
2. **Subsequent runs** — CLI re-connects over the existing Unix socket; cached transforms are reused; warm V8 returns results in ~100ms.

See [Architecture](advanced/architecture.md) for details.

## Where to go next

- [Installation](getting-started/installation.md) — how to install rjest
- [Quick Start](getting-started/quick-start.md) — get up and running in minutes
- [Migration from Jest](getting-started/migration.md) — drop-in migration steps and known gaps
- [CLI Reference](reference/cli.md) — every supported flag, including daemon management
- [Configuration](reference/configuration.md) — supported `jest.config.*` fields
- [Architecture](advanced/architecture.md) — daemon, transform cache, worker pool
- [Troubleshooting](advanced/troubleshooting.md) — common errors and fixes

## License

MIT — see [LICENSE](https://github.com/neul-labs/rjest/blob/main/LICENSE).
