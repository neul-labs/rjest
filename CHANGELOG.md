# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-05-09

### Added
- Explicit state machines for worker lifecycle, daemon lifecycle, watch sessions, and CLI daemon management.
- Committed PyPI package (`packages/pypi`) for `pip install rjest-cli`.
- `CHANGELOG.md` to track releases.
- PyPI publishing via OIDC Trusted Publishing in GitHub Actions.

### Changed
- Updated npm package (`packages/npm`) to `rjest-cli` v0.1.1 with correct repository URLs and missing dependencies.
- Updated documentation site with correct repository URLs and expanded installation instructions.
- Homebrew tap README now lists `rjest` as an available formula.

### Fixed
- Fixed worker health checks to correctly report `busy` state instead of hardcoding `false`.
- Fixed watch session race condition where a session could be removed between lock drops.
- Fixed CI/CD pipeline: resolved `cargo fmt` failures, `cargo clippy` warnings, `cargo doc` warnings, and broken integration tests.
- Fixed `.cargo/config.toml` osxcross linkers that broke native macOS builds.
- Fixed CI auto-tagging logic to correctly compare version strings.
- Fixed `build-matrix` artifact upload to include both `jest` and `jestd` binaries.
- Fixed `crates/rjest-bench/Cargo.toml` to use workspace version.

## [0.1.0] - 2025-03-15

### Added
- Initial release of `rjest`, a Rust-based drop-in replacement for Jest.
- Daemon-based architecture with persistent SWC transform caching via `sled`.
- Worker pool for parallel test execution using Node.js child processes.
- CLI shim compatible with common Jest flags (`--watch`, `--coverage`, `--runInBand`, etc.).
- Multi-platform support: Linux (x86_64, aarch64), macOS (x86_64, arm64), Windows (x86_64).
