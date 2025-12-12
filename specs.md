Cool, this is a fun one. Let’s treat it like a product/architecture spec and poke at the edges.

---

## 1. What actually makes Jest painful?

If an AI agent runs `npm test` (or `jest path/to/file.test.ts`) over and over, the pain is usually:

1. **Cold start every time**

   * Load Node.
   * Load Jest core.
   * Parse config, resolve projects.
   * Discover test files via glob + FS crawl.

2. **Transform overhead**

   * Babel / ts-jest / SWC run fresh for every invocation.
   * No long-lived AST / transform cache across separate CLI calls.

3. **Per-test-file isolation**

   * Each test file in its own worker/vm context.
   * Jest’s mocking layer, fake timers, jsdom, etc. all reinitialized.

4. **Pretty but heavy reporting**

   * Rendering watch UI, spinners, etc. even when the caller is a script.

So re-running `npm test` for every agent iteration is “full IDE warmup” each time.

---

## 2. High-level idea: Rust “Jest daemon” + thin Jest-compatible CLI

Assuming we *can* have a daemon, the shape could be:

**Processes:**

* **`jestd`** (daemon, written in Rust)

  * Long-lived.
  * Holds FS graph, transform cache, test manifest, and worker pools.
* **`jest` CLI shim** (small Node or Rust binary)

  * Drop-in: supports (most of) Jest’s CLI flags.
  * Forwards the request to `jestd` via a local socket / named pipe.
  * Pretty-prints results in Jest-compatible format.

User/agent perspective:

```bash
# Human or AI agent:
npx jest src/foo.test.ts --runInBand

# Under the hood:
#  - if daemon not running, start it
#  - send "run tests with these args" RPC
#  - get back results in slightly extended JSON
#  - render like Jest
```

---

## 3. Responsibilities of the Rust daemon

### 3.1 Project & file graph

* On first run:

  * Read Jest config(s) (jest.config, package.json, etc.).
  * Discover test files.
  * Build a dependency graph (using SWC to parse imports).
* Watch the filesystem:

  * On file change, update graph incrementally.
  * Know which tests are affected by a changed source file.

This is basically “Jest + ts-jest + dependency graph” but persistent and written in Rust.

### 3.2 Transform & cache via SWC

* Use SWC for:

  * TS/JS → runnable JS
  * JSX → JS
  * Optional: coverage instrumentation (Istanbul-compatible) via SWC plugins.
* Maintain:

  * **Content hash → transformed code**, persisted on disk.
  * Optional: **source map index**.

Inter-run speedup:

* AI agent changes one file → only that file and its dependents get re-transformed.
* All other tests reuse existing compiled output.

### 3.3 Test runners (workers)

Two main paths:

#### Path A: **Node-backed execution (pragmatic)**

Workers are **Node child processes**, but:

* Each worker process stays alive across runs.
* Tests are loaded from **precompiled SWC output** (no Babel, no ts-jest).
* We preload a small “jest-shim” JS library in each worker:

  * Re-implements Jest APIs (`test`, `it`, `describe`, `beforeEach`, `afterEach`, `expect` wired to `@jest/expect` or your own impl).
  * Handles `jest.fn`, `jest.spyOn`, timers, etc.
* Daemon dispatches test files to workers (like Jest’s worker pool, but long-lived).

Pros:

* Maximum compatibility with existing code that assumes Node APIs.
* Can rely on existing JS libraries for `expect`, `jsdom`, etc.
* Less reimplementation risk.

Cons:

* Still pay the cost of Node processes (though amortized).
* Need careful lifecycle management to avoid memory leaks.

#### Path B: **Custom Rust+JS runtime (ambitious)**

* Embed a JS engine (e.g., V8 via `rusty_v8`/`deno_core`) or QuickJS.
* Implement:

  * Module loader (CJS/ESM + Jest-like resolution).
  * Global Jest APIs via JS/TS library in this runtime.
  * Node-ish shims for `fs`, `path`, etc., or provide a limited environment.

Pros:

* Potentially even faster & more controllable.
* Deep integration with Rust, easier parallelization, cross-platform consistency.

Cons:

* Huge surface area to match Jest behaviour + Node quirks.
* Any missing Node/Jest feature breaks real-world test suites.

**Realistically**: start with **Node-backed workers**; explore custom runtime later if it’s worth it.

---

## 4. Jest compatibility surface (how close do we need to be?)

To be “Jest-compatible CLI,” there are layers:

### 4.1 CLI flags & UX

* Support the usual suspects:

  * `jest [pattern]`, `--runInBand`, `--watch`, `--coverage`, `--bail`, `--maxWorkers`, `--config`, `--json`, etc.
* For anything unsupported:

  * Emit a clear “not fully supported yet” summary.
  * Optionally have a `--fallback-to-jest` mode that shells out to real Jest.

### 4.2 Config

* Need to handle at least:

  * `testMatch`, `testRegex`, `testEnvironment`, `transform`, `moduleNameMapper`, `modulePaths`, `roots`, `projects`.
* Strategy:

  * For v1, **parse config using Node**:

    * Node loads config (JS/TS) and serializes the resolved config to JSON once.
    * Daemon consumes that JSON and doesn’t have to understand dynamic config.
  * Later, implement more in Rust.

### 4.3 Module loading & mocking

Big, hairy part:

* `jest.mock()`, `jest.doMock()`, `jest.unmock()`
* Auto-mocking & hoisting semantics
* ESM + CJS interop
* `setupFiles`, `setupFilesAfterEnv`

If workers are Node-based, you can:

* Reuse a Jest-style runtime library (or heavily borrow their semantics/algorithm).
* Focus on **transform + orchestration**, not re-implementing mocking from scratch.

For v1, you might accept “no automocking, only manual `jest.mock()`,” etc., then expand.

### 4.4 Snapshots & coverage

* **Snapshots**:

  * Reuse Jest snapshot file format to avoid surprising people.
  * Implement snapshot matchers (`toMatchSnapshot`, etc.) in your environment libraries.

* **Coverage**:

  * Use SWC instrumenter → Istanbul JSON output.
  * Convert to coverage reports (`lcov`, text-summary) like Jest does.

---

## 5. Why a daemon helps AI agents specifically

For agents running `npm test` in a loop:

1. **Near-instant incremental runs**

   * Daemon knows the project.
   * Transforms already cached.
   * Workers already hot.
   * “Run these 3 tests” is almost just “dispatch to worker + print results”.

2. **Machine-friendly output**

   * The CLI can support `--json` / `--machine`:

     * Per-test-case info with file, name, duration, error stack, etc.
   * Agent can quickly map failing tests back to files/functions.

3. **Stateful sessions**

   * You could optionally have:

     * `jestd session start` → returns session ID.
     * Subsequent runs share history, last-failing tests, etc.
   * Might help agents prioritize tests or do `–only-changed`-like behaviour.

4. **Selective running per change**

   * Daemon maintains a git- or FS-based “changed files since X” view.
   * Agent asks “run tests affected by the files I just edited,” not entire suite.

---

## 6. Rust-specific advantages & tradeoffs

### Advantages

* **Performance**: FS scanning, dependency graph, transforms pipeline all in Rust with SWC → huge speed-ups over ad-hoc JS.
* **Memory predictability**: Daemon stays alive, but Rust-side memory use is easier to control than a massive Node process with huge GC pauses.
* **Cross-platform single binary**: ship a `jestd` binary and a tiny front CLI; minimal install time.

### Challenges

* Keeping up with Jest changes and Node ecosystem.
* Making Windows-friendly IPC & watchers (named pipes, etc.).
* Debug story: if tests crash, giving good stack traces & source maps.

---

## 7. Minimal viable first version (pragmatic scope)

If we were scoping an MVP:

1. **Use Node for execution.**
2. Support:

   * TS/JS with SWC, Node test environment.
   * No jsdom at first (or optional).
   * No custom transformers; just SWC config.
3. Provide:

   * Jest-like CLI for the most common flags.
   * Basic watch mode (daemon-driven).
   * JSON output for machine consumption.
4. De-prioritize:

   * Advanced mocking behaviours.
   * Multi-project monorepo support (or do a simple version).
   * Exotic environments / transformers.

Even *just* “same CLI flags, faster TS transforms, persistent workers” would already be a big win for agent workflows.

---

If you want, next we can zoom into one of these:

* The exact daemon <-> CLI protocol (what the RPC payloads look like).
* How to wrap Node workers in a way that gives close-to-Jest semantics.
* Or: how to detect and gracefully fallback to “real Jest” when a project uses features we don’t support yet.

