# CLI Reference

Complete reference for rjest command-line options.

## Basic Usage

```bash
jest [options] [file-patterns...]
```

## Test Selection

### File Patterns

```bash
# Run all tests
jest

# Run specific file
jest src/utils.test.ts

# Run multiple files
jest src/utils.test.ts src/api.test.ts

# Run with glob pattern
jest src/**/*.test.ts
```

### Test Name Filtering

```bash
# Filter by test name
jest --testNamePattern="adds two numbers"
jest -t "adds two numbers"

# Regex patterns
jest -t "add|subtract"
jest -t "should.*error"
```

## Output Options

### `--json`

Output results in JSON format:

```bash
jest --json
```

JSON structure:

```json
{
  "success": true,
  "num_passed_suites": 2,
  "num_failed_suites": 0,
  "num_passed_tests": 19,
  "num_failed_tests": 0,
  "duration_ms": 150,
  "test_results": [...]
}
```

### `--verbose`

Show individual test names:

```bash
jest --verbose
```

### `--outputFile`

Save JSON output to file:

```bash
jest --json --outputFile=results.json
```

## Snapshot Options

### `--updateSnapshot`, `-u`

Update snapshots instead of comparing:

```bash
jest -u
jest --updateSnapshot
```

## Watch Mode

### `--watch`

Re-run tests when files change:

```bash
jest --watch
```

### `--watchAll`

Run all tests when any file changes:

```bash
jest --watchAll
```

## Daemon Management

### `--daemon-status`

Check if the daemon is running:

```bash
jest --daemon-status
```

Output:

```
Daemon Status:
  Running: true
  PID: 12345
  Uptime: 300s
  Projects: 1
  Workers: 4
```

### `--daemon-stop`

Stop the background daemon:

```bash
jest --daemon-stop
```

!!! tip "Free Memory"
    Run `--daemon-stop` when you're done testing to free up the ~200MB
    used by the daemon and workers.

## Configuration

### `--config`, `-c`

Specify a custom config file:

```bash
jest --config=jest.config.custom.js
jest -c jest.config.custom.js
```

### `--rootDir`

Set the root directory:

```bash
jest --rootDir=/path/to/project
```

## Other Options

### `--version`

Show version:

```bash
jest --version
```

### `--help`

Show help:

```bash
jest --help
```

### `--bail`

Stop on first failure:

```bash
jest --bail
```

### `--maxWorkers`

Set the number of worker processes:

```bash
jest --maxWorkers=4
jest --maxWorkers=50%  # 50% of CPU cores
```

## Environment Variables

### `RUST_LOG`

Enable debug logging:

```bash
RUST_LOG=debug jest
RUST_LOG=rjest=trace jest
```

### `NO_COLOR`

Disable colored output:

```bash
NO_COLOR=1 jest
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All tests passed |
| 1 | Some tests failed |
| 2 | Configuration or runtime error |

## Examples

### CI/CD Pipeline

```bash
# Run tests with JSON output for parsing
jest --json --outputFile=test-results.json

# Fail fast in CI
jest --bail
```

### Development Workflow

```bash
# Start daemon and run tests
jest

# Make changes, run again (fast!)
jest

# Run specific test while debugging
jest -t "handles edge case" --verbose

# When done, stop daemon
jest --daemon-stop
```

### Debugging

```bash
# Verbose output with debug logs
RUST_LOG=debug jest --verbose

# Check daemon status
jest --daemon-status
```
