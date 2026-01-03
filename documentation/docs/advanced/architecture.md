# Architecture

Understanding how rjest achieves its performance.

## Overview

rjest uses a daemon architecture to eliminate the startup overhead that makes Jest slow:

```
┌─────────────────────────────────────────────────────────────┐
│                         rjest CLI                            │
│                        (Rust binary)                         │
└─────────────────────────┬───────────────────────────────────┘
                          │ IPC (Unix socket)
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                      rjest Daemon                            │
│                      (Rust process)                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Config    │  │   Test      │  │    SWC Transform    │  │
│  │   Loader    │  │  Discovery  │  │       Cache         │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────┬───────────────────────────────────┘
                          │ stdio JSON
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                     Worker Pool                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐    │
│  │ Worker 1 │  │ Worker 2 │  │ Worker 3 │  │ Worker 4 │    │
│  │ (Node.js)│  │ (Node.js)│  │ (Node.js)│  │ (Node.js)│    │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## Components

### CLI (`rjest-cli`)

The command-line interface is a lightweight Rust binary that:

1. Parses command-line arguments
2. Connects to the daemon via Unix socket
3. Sends test run requests
4. Receives and displays results

```rust
// Simplified flow
fn main() {
    let args = parse_args();
    let socket = connect_to_daemon()?;
    socket.send(RunRequest { ... })?;
    let results = socket.recv()?;
    display_results(results);
}
```

### Daemon (`jestd`)

The daemon is a long-running Rust process that:

1. **Stays resident** between test runs
2. **Caches configuration** - Jest config is parsed once
3. **Caches transforms** - TypeScript compilation results are cached
4. **Manages workers** - Node.js worker pool stays warm

```rust
// Daemon main loop
async fn run_daemon() {
    let state = DaemonState::new();

    loop {
        let request = socket.recv().await?;
        let response = handle_request(request, &state).await?;
        socket.send(response).await?;
    }
}
```

### Transform Cache

TypeScript/JSX compilation uses native SWC and is cached by content hash:

```rust
pub struct TransformCache {
    // Persistent disk cache
    db: sled::Db,
    // In-memory LRU cache for hot files
    lru: LruCache<String, TransformResult>,
}

impl TransformCache {
    fn get(&self, path: &Path, hash: &str) -> Option<TransformResult> {
        // Check LRU first (microseconds)
        // Fall back to disk cache (milliseconds)
    }
}
```

### Worker Pool

Node.js workers run tests and stay warm between runs:

```javascript
// worker.js - simplified
async function main() {
    while (true) {
        const request = await readRequest();
        const result = await runTest(request);
        sendResult(result);
    }
}
```

Workers are:
- Pre-spawned at daemon startup
- Kept warm with V8 JIT-compiled code
- Recycled after 1000 tests (memory hygiene)
- Killed after 60 seconds of idle time

## Data Flow

### Cold Start

```
1. CLI starts
2. CLI checks for daemon → not running
3. CLI spawns daemon
4. Daemon initializes:
   - Opens transform cache
   - Spawns worker pool
   - Sends "ready" signal
5. CLI sends run request
6. Daemon:
   - Loads Jest config
   - Discovers test files
   - Transforms files (SWC)
   - Distributes to workers
7. Workers run tests
8. Daemon aggregates results
9. CLI displays output
```

### Warm Run

```
1. CLI starts
2. CLI connects to running daemon
3. CLI sends run request
4. Daemon:
   - Uses cached config
   - Uses cached transforms (if unchanged)
   - Uses warm workers
5. Workers run tests (V8 already JIT'd)
6. Results returned immediately
```

## Why It's Fast

### 1. No Process Startup

Jest spawns a new Node.js process every run:
- Parse Node.js CLI arguments
- Initialize V8
- Load Jest runtime
- Parse configuration
- Compile TypeScript

rjest does this once and reuses it.

### 2. Native TypeScript Compilation

| Compiler | Language | Speed |
|----------|----------|-------|
| ts-jest (tsc) | JavaScript | Slow |
| @swc/jest | JavaScript → SWC | Fast |
| rjest (SWC) | Rust → SWC | Fastest |

rjest calls SWC directly from Rust, avoiding Node.js overhead entirely.

### 3. Cached Transforms

Unchanged files are never recompiled:

```
File: src/utils.ts
Hash: abc123...
Cache: HIT → Return cached JavaScript instantly
```

### 4. Warm V8 JIT

V8 compiles JavaScript to machine code progressively. Warm workers have:
- Hot functions already optimized
- Inline caches populated
- Hidden classes stable

### 5. Low-Latency IPC

Unix domain sockets are faster than:
- Spawning processes
- HTTP communication
- Named pipes

## Memory Layout

```
Daemon Process (~17 MB)
├── Transform cache (in-memory LRU)
├── Configuration cache
├── Worker pool manager
└── IPC socket handler

Worker Process (~50 MB each, 4 workers)
├── Node.js runtime
├── Jest globals (describe, test, expect)
├── Loaded test modules
└── V8 heap

Disk Cache (~variable)
├── transforms.sled (compiled JavaScript)
└── Indexes
```

Total: ~17 MB + (4 × 50 MB) = ~217 MB

## Configuration Loading

```
jest.config.js
     │
     ▼
┌─────────────┐
│ Parse with  │
│   Node.js   │
└─────────────┘
     │
     ▼
┌─────────────┐
│   Cache     │
│  in Daemon  │
└─────────────┘
     │
     ▼
Subsequent runs use cached config
```

## Test Execution

```
Test File
     │
     ▼
┌─────────────┐
│   Read &    │
│   Hash      │
└─────────────┘
     │
     ▼
┌─────────────┐     ┌─────────────┐
│ Cache HIT?  │────▶│ Return      │
│             │ Yes │ Cached JS   │
└─────────────┘     └─────────────┘
     │ No
     ▼
┌─────────────┐
│ SWC         │
│ Transform   │
└─────────────┘
     │
     ▼
┌─────────────┐
│ Cache       │
│ Result      │
└─────────────┘
     │
     ▼
┌─────────────┐
│ Send to     │
│ Worker      │
└─────────────┘
     │
     ▼
┌─────────────┐
│ Execute     │
│ in VM       │
└─────────────┘
```

## Crate Structure

```
rjest/
├── crates/
│   ├── rjest-cli/        # CLI binary
│   │   └── src/
│   │       ├── main.rs
│   │       └── args.rs
│   │
│   ├── jestd/            # Daemon binary
│   │   └── src/
│   │       ├── main.rs
│   │       ├── server.rs
│   │       ├── config/
│   │       ├── discovery/
│   │       ├── transform/
│   │       └── worker/
│   │
│   ├── rjest-protocol/   # IPC messages
│   │   └── src/
│   │       └── lib.rs
│   │
│   └── rjest-runtime/    # Node.js worker
│       └── src/
│           └── worker.js
│
└── tests/
    └── fixtures/         # Test projects
```
