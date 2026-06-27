# test262 Integration for Parser Testing

Integration of the ECMAScript conformance test suite (test262) to validate tsv's TypeScript parser against ~50,000 JS test cases.

## Goal

Use test262 to validate that tsv's parser correctly:

1. **Accepts valid syntax** - All tests without `negative.phase: parse` should parse successfully
2. **Rejects invalid syntax** - Tests with `negative.phase: parse` should fail to parse

## Current Results

Regenerate with `cargo run -p tsv_debug test262` (expects a test262 checkout
at `../test262`); refresh this list when the parser or the test262 snapshot
changes — at minimum per release. Counts below are from a snapshot of ~49k
discovered tests (46,545 graded after skips).

- Positive (should parse) — 41,907 passed, 207 failed
- Negative (should reject) — 1,339 passed, 3,092 failed

- **Overall**: 43,246/46,545 (92.9%)
- **Positive pass rate**: 99.5% — valid syntax tsv accepts
- **Skipped**: 2,591 (sloppy mode: 2,519, runtime: 38, resolution: 34)

**Triaging the positive failures against the drop-in oracle.** Each of the 207 is
parsed with the canonical parser (acorn-typescript in module mode — what the
fixtures' `expected.json` is generated from). **~18 are genuine tsv-vs-acorn bugs
(acorn accepts, tsv rejects) — real parser gaps to close.** The remaining ~189
are rejected by acorn too (not tsv-specific). _(Methodology: parse each
`../test262/<path>` with `canonical_parse` and bucket on whether it yields an
AST. An earlier triage used a wrong path prefix, so every file came back
"not found" and was mis-bucketed as rejected — the corrected sweep below is
authoritative.)_

**The ~18 real bugs**, by cluster:

- **Unicode `Other_ID_Start` / `Other_ID_Continue` identifiers (4)** — characters
  like `゛` (U+309B) and their escaped forms. tsv's `unicode-ident` uses the XID
  sets, which exclude the legacy `Other_ID_*` compatibility code points.
- **Module export/import name = reserved word (4)** — `export * as default`,
  `export { x as class }`, import specifiers naming a reserved word. An
  export/import name is an `IdentifierName` (reserved words allowed), not a
  `BindingIdentifier`.
- **Rest parameter with a destructuring pattern (2)** — `function f(...[a, b]) {}`,
  `function f(...{ a }) {}`. A rest element can be a `BindingPattern`, not only an
  identifier.
- **Hashbang terminated by U+2028 / U+2029 (2)** — a line/paragraph separator
  should end a `#!` comment.
- **Singletons (6)** — `import.meta` in some positions, `for await (… of …)` with
  an async LHS, `var undefined` (`undefined` is not reserved), do-while ASI on one
  line, a ZWNBSP (U+FEFF) after a regex literal, a decorator member-expression
  position, and an assignment-target case.

These are the actionable positive-conformance backlog (fixtures-first per the
repo TDD gate). Two such gaps are now fixed: ✅ the tagged-template invalid-escape
gap (ES2018), and ✅ the `[+In]` for-header reset — the for-init disables `in`
(`[~In]`), but nested sub-expressions restore `[+In]` (computed class member name,
ternary consequent, dynamic-import argument, function/class bodies). tsv had leaked
the for-header `[~In]` into them; now they parse, and the formatter parenthesizes an
`in` anywhere under a for-init (matching prettier, keeping it distinct from the
`for (x in y)` separator).

**The ~189 acorn-also-rejects** are not tsv bugs — they split into:
**sloppy-mode-only** (`with`, AnnexB `f() = g()` / `for (var a = x in b)`, legacy
octal — tsv is strict-only); **strict-*Script*-only** (top-level `await` as a
*binding*, e.g. `var await = 1` — valid in a strict Script but not a Module;
**strict-script support is planned**, sloppy is not — `yield` is unaffected, being
a strict reserved word in both goals); **`await`-as-identifier inside a non-async
function/generator/method** (`function foo(await) {}`, the `static-init-await`
cluster — valid in *module* per spec but acorn rejects it anyway; the planned
**`await`-context tracking** fixes this and the strict-Script bindings together);
**Stage-3 proposals** (`import.source`, `import.defer` — the dynamic-import
cluster, a roadmap question); and **plugin-gated syntax** (decorators, not in the
oracle config). When the await-context work lands, the strict-Script + non-async
buckets move from "acorn-also-rejects" to deliberate, more-spec-correct
divergences from acorn-as-module.

Most negative failures are over-acceptance of _early errors_ — programs that
parse under the syntactic grammar but that the spec rejects semantically
(duplicate parameter names, rest parameters with initializers, escaped
reserved words, strict-mode-only restrictions). tsv currently enforces the
syntactic grammar; early-error enforcement is future diagnostics-layer work.

## Scope

### What We Test

- **Positive tests**: Parse should succeed (no syntax errors)
- **Negative parse tests**: Parse should fail with a syntax error

### What We Skip

- `negative.phase: runtime` - Requires execution
- `negative.phase: resolution` - Requires module resolution
- `*_FIXTURE.js` files - Module dependencies, not standalone tests

### Test Directories

- `test/language/` — ~23,659 — Primary - language syntax
- `test/built-ins/` — ~23,039 — Secondary - valid syntax in test bodies
- `test/annexB/` — Various — Tertiary - web compat features
- `test/staging/` — Various — Skip - in-progress proposals

## Design

### Location

`crates/tsv_debug/src/cli/commands/test262.rs` - the test262 command in tsv_debug

### Command Interface

```bash
# Basic usage
cargo run -p tsv_debug test262                     # Run all tests (default: ../test262)
cargo run -p tsv_debug test262 --path /path/to/test262  # Custom path

# Filtering
cargo run -p tsv_debug test262 language/expressions     # Filter by path pattern
cargo run -p tsv_debug test262 --list                   # List tests only
cargo run -p tsv_debug test262 --negative-only          # Only parse-error tests
cargo run -p tsv_debug test262 --positive-only          # Only should-parse tests

# Output control
cargo run -p tsv_debug test262 --verbose                # Show all results
```

### Frontmatter Parsing

Pure string operations, no regex dependency. The YAML frontmatter has these key fields:

```yaml
/*---
features: [BigInt, class-fields-private]   # Optional array
flags: [async, module, onlyStrict]         # Optional array
negative:
  phase: parse                             # parse | runtime | resolution
  type: SyntaxError                        # Error type
---*/
```

Algorithm:

1. Extract block between `/*---` and `---*/`
2. Parse line-by-line: `features:`/`flags:` as arrays, `negative:` as a block with `phase:` and `type:`
3. Handle edge cases gracefully (multiline arrays, quoted strings, missing frontmatter, BOM) — log warning and skip on failure

See `crates/tsv_debug/src/test262/frontmatter.rs`.

### Test Execution Flow

```
1. Discover tests
   - Walk test262/test/ directory
   - Filter out *_FIXTURE.js files
   - Apply path filters if specified

2. For each test file:
   a. Read file content
   b. Extract frontmatter
   c. Determine test type:
      - negative.phase == "parse" → expect failure
      - negative.phase == "runtime"|"resolution" → skip
      - no negative field → expect success
   d. Parse with tsv_ts::parse()
   e. Compare result with expected

3. Aggregate results
   - Passed: Result matched expectation
   - Failed: Result didn't match expectation
   - Skipped: Runtime/resolution tests, or parse failure

4. Report summary
```

### Module Structure

```
crates/tsv_debug/src/
├── cli/commands/
│   ├── mod.rs              # Registers the test262 subcommand
│   └── test262.rs          # Test262Command + Test262Executable
└── test262/                # Test262 support module
    ├── mod.rs              # Public API
    ├── discovery.rs        # Find test files
    ├── frontmatter.rs      # Parse YAML frontmatter (pure string ops, no regex)
    └── runner.rs           # Execute tests
```

## Output Format

Numbers below are illustrative — the live run prints current counts (the
latest full-suite results are in [Current Results](#current-results)).

### Default (Summary)

```
test262 validation
==================
Path: ../test262

Scanning test/language/...

Results:
  Positive tests: 20432 passed, 127 failed
  Negative tests: 3100 passed, 23 failed
  Skipped:        2591 (sloppy mode: 2519, runtime: 38, resolution: 34)

Pass rate: 23532/23682 (99.4%)

Run with --verbose to see failure details
```

### Verbose (Failures)

```
test262 validation
==================
Path: ../test262

Failed positive tests (should parse but didn't):
  test/language/expressions/class/syntax-error.js
    Error: Unexpected token at line 5, column 3

  test/language/statements/for/invalid-init.js
    Error: Expected ';' at line 2, column 10

Failed negative tests (should fail but parsed):
  test/language/statements/for/invalid-lhs.js
    Expected: SyntaxError (phase: parse)
    Got: Parsed successfully

Results:
  Positive tests: 20432 passed, 2 failed
  Negative tests: 3100 passed, 1 failed
  Skipped:        2591 (sloppy mode: 2519, runtime: 38, resolution: 34)

Pass rate: 23535/23538 (99.9%)
```

## Design Decision: Strict Mode Only

**tsv parses as strict mode only.** This matches our actual use cases:

- **TypeScript**: Always strict (implicitly)
- **ES Modules**: Always strict (`import`/`export` implies strict)
- **Svelte `<script>`**: ES modules, always strict

Tests with `noStrict` flag (requiring sloppy mode) are skipped. This is intentional.

## Differential Comparison (tsv vs oxc-parser)

The pass rate above is **un-baselined** — a positive failure could be a genuine
tsv parser gap, or a test even other parsers reject. To triage, the harness can
emit a **manifest** of tsv's graded strict subset and a Deno consumer compares
each verdict against [oxc-parser](https://github.com/oxc-project/oxc):

```bash
# 1. Rust emits the manifest: one row per graded test (relative path, module
#    flag, expected verdict, tsv verdict). Honors the same path filters.
cargo run -p tsv_debug test262 --emit-manifest /tmp/t262.json

# 2. Deno consumer runs oxc-parser over the same files (parsed as a module, to
#    mirror tsv) and buckets the agreement. Run from the repo root:
deno run --allow-read --allow-env --allow-ffi --allow-net --allow-sys \
  --config benches/js/deno.json \
  benches/js/diagnostics/test262_compare.ts --manifest /tmp/t262.json
```

**Why oxc-parser only (no biome).** test262 is a *parse*-conformance suite, and
oxc-parser is the alternative with a real, gradable accept/reject verdict
(`parseSync` + `errors`). Biome's `@biomejs/js-api` exposes **no parser** (only
format/lint), so it has no verdict to grade; it stays a *formatter* subject in
the bench, not here.

**Fairness — same subset, same mode.** The consumer runs oxc over *only* the
tests tsv grades (the strict, non-sloppy, parse-phase subset), and parses every
one as `sourceType: 'module'` — because tsv has no script mode (it parses
everything as a strict ES module). A genuinely script-only test therefore
rejects on both sides and lands in `both-reject`, correctly *not* attributed to
tsv. The two actionable buckets:

- **positives where tsv rejects but oxc accepts** → tsv real-bug candidates
- **negatives where oxc rejects but tsv accepts** → tsv early-error gaps (the
  deferred-diagnostics map; tsv under-enforces early errors by design)

The consumer prints a same-subset pass-rate baseline (`tsv X% vs oxc Y%`) plus
the bucket counts to stderr, and the full per-bucket path lists as JSON to
stdout. It's an **on-demand diagnostic** (not committed, not a CI gate) — its
numbers move with the pinned oxc version.

## Dependencies

No new crate dependencies — frontmatter parsing uses string operations, not a regex crate.

## Notes

- test262 is pure ECMAScript, not TypeScript - TS-specific syntax coverage comes from our fixtures
- The parser should accept valid JS as valid TS (TypeScript is a superset)
- Use `cargo run -p tsv_debug test262 <filter>` to focus on specific test categories
