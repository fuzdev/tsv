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
discovered tests (46,544 graded after skips).

- Positive (should parse) — 42,113 passed, 0 failed
- Negative (should reject) — 2,151 passed, 2,280 failed

- **Overall**: 44,264/46,544 (95.1%)
- **Positive pass rate**: 100% — every test tsv grades and that should parse does,
  graded at each test's declared goal (see [Goal axis](#design-decision-strict-mode-only-explicit-goal-axis))
- **Skipped**: 2,592 (sloppy mode: 2,520, unimplemented feature: 0, runtime: 38, resolution: 34)

The remaining negative failures are early-error *under-enforcement* (programs that
parse under the syntactic grammar but the spec rejects semantically — duplicate
params, escaped reserved words, etc.), deferred to a future diagnostics layer by
design, not parser bugs.

**Feature filtering.** Tests whose `features:` frontmatter names a syntactic
proposal tsv does not implement are skipped, not graded — scoring them as parse
failures would measure scope, not a conformance gap. The set
(`UNIMPLEMENTED_FEATURES` in `crates/tsv_debug/src/test262/frontmatter.rs`) is
**currently empty**: tsv parses the Stage-3 import-phase proposals
(`source-phase-imports` / `import.source(…)` and `import-defer` /
`import.defer(…)`, ~396 graded files) rather than skipping them — a deliberate
divergence from acorn, which rejects them (see
[conformance_svelte.md](./conformance_svelte.md#import-phase-proposals)).
See [Scope](#what-we-skip).

**Positive parse conformance is 100%** at each test's declared goal (`module`-flagged
→ `Module`; the run-both-ways default + `onlyStrict` → strict `Script`). The lone
exception — the sloppy-by-content `raw` test `language/comments/hashbang/use-strict.js`
— is **skipped** as out of scope for a strict-only parser (the sloppy-mode bucket; see
[Goal axis](#design-decision-strict-mode-only-explicit-goal-axis)), not graded; the
other 27 in-scope `raw` tests stay graded. _(Methodology for any future failure: parse
each `../test262/<path>` with `canonical_parse` and bucket on whether it yields an AST.)_

**tsv's positive conformance exceeds the drop-in oracle** in several places —
constructs acorn rejects but the spec accepts, which tsv parses per spec. Each is
pinned by a fixture (fixtures-first per the repo TDD gate):

- **Rest parameter with a destructuring pattern** — `function f(...[a, b]) {}` /
  `function f(...{ a }) {}` (a rest element can be a `BindingPattern`, not only an
  identifier).
- **`for await` with an async LHS** — `for await (async of [7])` parses `async` as an
  `IdentifierReference`, while plain `for (async of …)` stays rejected (the for-of
  `[lookahead ∉ { async of }]` restriction).
- **Decorated class *expression*** — `x = @dec class {}` (decorators in expression
  position, not only statement position); the assignment breaks after `=` with each
  decorator on its own line, like prettier.
- **Tagged-template invalid escapes** (ES2018) — tolerated in a tagged template
  (cooked `undefined`, raw preserved).
- **`[+In]` for-header reset** — the for-init disables `in` (`[~In]`), but nested
  sub-expressions restore `[+In]` (computed class member name, ternary consequent,
  dynamic-import argument, function/class bodies). The formatter parenthesizes an `in`
  anywhere under a for-init (matching prettier), keeping it distinct from the
  `for (x in y)` separator.
- **Unicode identifiers per `ID_Start` / `ID_Continue`** — ECMAScript keys identifier
  validity on the `ID_*` properties (ecma262 §sec-names-and-keywords → UAX #31), a
  superset of `unicode-ident`'s `XID_*` sets: the `Other_ID_Start` voiced/semi-voiced
  sound marks (`゛` U+309B, `゜` U+309C) plus letters whose NFKC decomposition leaves
  the set (U+037A, U+0E33, U+0EB3, the halfwidth katakana marks, the Arabic
  ligature/presentation forms).
- **Hashbang line/paragraph-separator terminators** — a `#!…` comment ends at U+2028 /
  U+2029, like any other LineTerminator.
- **ECMAScript-exact inter-token whitespace** — U+FEFF (ZWNBSP) is WhiteSpace and is
  skipped mid-stream, not only as a leading BOM, while U+0085 (NEL), which ECMAScript
  excludes, is not.
- **do-while ASI** — a `;` is inserted unconditionally after the `)`
  (`do x; while (c) y`).
- **`undefined` as an ordinary identifier** — a valid binding name (`var undefined`)
  and assignment target (`undefined = 12`), modeled as an `Identifier` like acorn
  rather than a literal.
- **`new import.meta()`** — `import.meta` is a valid `new` callee (a MetaProperty),
  while `new import(…)` stays rejected.
- **`await` as an identifier in a strict Script** — `var await = 1`, `function
  foo(await)`, `await => x`, `class await {}`, `await:` / `break await` / `continue
  await` labels, `({ await })` shorthand, `catch (await)`, `function await(){}`,
  `new await()`, and the `static-init-await` nests. tsv grades these at `Goal::Script`,
  where `await` is an ordinary identifier in a `[~Await]` context; the same
  `await_is_identifier` read sites are gated the other way at `Goal::Module` /
  `[+Await]`, where `await` stays reserved (so `function await(){}` is rejected in a
  module, matching acorn). This makes tsv more spec-correct than acorn-*as-module*,
  which is module-only. See [Goal axis](#design-decision-strict-mode-only-explicit-goal-axis).

`yield` is unaffected by the goal axis — a strict reserved word in both `Script` and
`Module`. Out of scope (skipped, not graded as failures): **sloppy-mode-only**
constructs (`with`, the AnnexB `f() = g()` / `for (var a = x in b)` forms, legacy
octal — tsv is strict-only) and **plugin-gated syntax** not in the oracle config
(some decorator forms).

Most negative failures are the early-error under-enforcement noted above (duplicate
parameter names, escaped reserved words, strict-mode-only restrictions) — tsv enforces
the syntactic grammar; early-error enforcement is future diagnostics-layer work.

A smaller share are genuinely _syntactic_ over-acceptances — the grammar
itself forbids the construct — which tsv rejects. The rest-element
constraints: a rest/spread element must be the
final element of a parameter list (value and TS function-type) or an
array/object destructuring pattern (binding and assignment targets), with no
element or trailing comma after it and no default initializer — so
`function f(...a, b)`, `function f(...a, ...b)`, `[...a, b] = c`,
`{...a, b} = c`, `[...a = 1] = c`, and the trailing-comma form `[...a,] = c` /
`{...a,} = c` are all rejected, matching acorn. The trailing comma is
recovered from a `spread_trailing_comma` flag the array/object literal parser
records (the literal itself is valid, so the parser can't reject it outright;
the cover-grammar conversion consults the flag). A for-in/of destructuring LHS
is routed through the same cover-grammar conversion (`to_assignable`), so
the rest constraints hold there too (`for ([...a, b] of y)`,
`for ([...x = 1] of y)`, `for ({...a, b} of y)` are rejected) — matching the
spec's "an `ObjectLiteral`/`ArrayLiteral` for-in/of LHS must cover an
`AssignmentPattern`" rule, which additionally drops invalid non-pattern LHS
targets like `for (a + b of y)` and `for ((a, b) of y)`. A still-open adjacent
gap is the object rest _target_ shape — `({...[a]} = c)` / `const {...[a]} = c`
(the spec forbids an `ArrayLiteral`/`ObjectLiteral` target on an object rest,
and a `BindingRestProperty` must be a plain identifier) — is a separate
constraint left over-accepted.

tsv also enforces the **`[no LineTerminator here]` restricted productions**: a
line terminator between an arrow's parameters and `=>` (`(a)⏎=> x`), a conditional
type's check type and `extends` (`U⏎extends T ? 1 : 2`), a type predicate's
parameter and `is` (`x⏎is T`), or a definite-assignment binding and `!`
(`let x⏎!: T`) is a syntax error — matching acorn-typescript's
`hasPrecedingLineBreak` guards (a line comment in the gap counts, since it ends in
a newline). And three **positional grammar** rules: a for-in/of head binds exactly
one declarator (`for (let a, b of x)` rejected), a labeled statement's body is a
`Statement` or `FunctionDeclaration` and never a lexical/class declaration
(`a: class C {}`, `a: let x = 1`, `a: function f(){}` rejected; `a: var x = 1`,
`a: enum E {}` and ordinary statements accepted), and `import`/`export`
declarations appear only at the module top level or inside a TS
`namespace`/`module` body (`{ import x from 'y' }`, `function () { export … }`,
`if (c) import …` rejected, while `import(…)` / `import.meta` expressions stay
valid in any position).

tsv also enforces the **numeric-literal lexical grammar**. A
`NumericLiteralSeparator` (`_`) must sit between two digits, so it is rejected at
the start of a digit group, at the end, when doubled, or adjacent to a
prefix/`.`/`e` (`0x_12`, `12_`, `1__2`, `1.5_`, `1e_3` rejected; `1_000`,
`0xff_ff`, `1_000.000_5`, `1_000e1_0` accepted). A radix literal must carry at
least one digit after its prefix (`0x`, `0b`, `0o` with no digits rejected). And
the BigInt suffix `n` attaches only to an integer-form literal — never to a
fraction or exponent (`1.5n`, `1e3n` rejected; `123n`, `0xffn`, `0o7n`, `1_000n`
accepted) — matching acorn's "Identifier directly after number". Two adjacent
edges stay accepted, tangled with the intentional sloppy-mode `08`/`09`
carve-out: `0_1` (separator in a legacy-octal-shaped literal) and `5.n` (lexed as
member access `(5).n`). A `\u{…}` string escape must be terminated with `}`
(`'\u{41'` rejected).

Three **TypeScript-grammar** over-acceptances are likewise rejected. A mapped-type
`+`/`-` modifier is a single optional sign that must be followed by `readonly`
(key position) or `?` (value position): a stray sign (`{ [K in S]+: V }`,
`{ +[K in S]: V }`, `{ -+readonly [K in S]: V }`) is a syntax error, not a
silently dropped token. An `import X = …` reference must be `require('…')` or an
entity name (`A.B.C`): a string/number/empty reference (`import x = 'foo'`,
`import x = 5`, `import x =`) is rejected.

## Scope

### What We Test

- **Positive tests**: Parse should succeed (no syntax errors)
- **Negative parse tests**: Parse should fail with a syntax error

### What We Skip

- `negative.phase: runtime` - Requires execution
- `negative.phase: resolution` - Requires module resolution
- `flags: [noStrict]` - Requires sloppy mode (tsv is strict-only). A `flags: [raw]`
  test (verbatim source, no harness) also runs in non-strict mode only per
  test262/INTERPRETING.md, but nearly all exercise mode-independent syntax (hashbang,
  HTML-close comments, `"use strict"` directive prologues) tsv grades correctly at
  their goal, so those stay graded. Only a raw test whose verdict genuinely needs
  sloppy semantics — it uses a construct tsv rejects as strict-only (`with`, legacy
  octal) — is skipped, like `noStrict`. That list (`SLOPPY_ONLY_RAW_TESTS` in
  `crates/tsv_debug/src/test262/runner.rs`) is currently the single
  `language/comments/hashbang/use-strict.js`
- `features:` naming an **unimplemented syntactic proposal** - skipped in both
  polarities so the score reflects conformance on syntax tsv aims to support, not
  unimplemented scope. The skip set lives in
  `crates/tsv_debug/src/test262/frontmatter.rs` (`UNIMPLEMENTED_FEATURES`) and is
  **currently empty** — tsv parses the Stage-3 import-phase proposals
  (`source-phase-imports` / `source-phase-imports-module-source` / `import-defer`),
  so their ~396 graded files count. Add a name here when tsv meets a new proposal it
  doesn't parse; drop it once it lands.
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

Found 49136 test files

Processing: 49136/49136

Results:
  Positive tests: 41898 passed, 0 failed
  Negative tests: 1795 passed, 2455 failed
  Skipped:        2988 (sloppy mode: 2494, unimplemented feature: 422, runtime: 38, resolution: 34)

Pass rate: 43693/46148 (94.7%)
```

### Verbose (Failures)

`--verbose` lists each failure under a `Failures:` block. The remaining failures are
negative early-error under-enforcement (programs that parse syntactically but the spec
rejects semantically); filtering to one shows the format:

```
test262 validation
==================
Path: ../test262

Found 49136 test files
Filtered to 1 tests matching: RegExp/property-escapes/binary-property-with-value-ASCII_-_F.js

Processing: 1/1

Failures:
---------
test/built-ins/RegExp/property-escapes/binary-property-with-value-ASCII_-_F.js
  Expected: Parse error (phase: parse)
  Got: Parse success

Results:
  Positive tests: 0 passed, 0 failed
  Negative tests: 0 passed, 1 failed

Pass rate: 0/1 (0.0%)
```

## Design Decision: Strict Mode Only, Explicit Goal Axis

**tsv parses as strict mode only** — there is no sloppy mode and no `"use strict"`
detection. This matches our use cases (TypeScript is always strict; ES modules and
Svelte `<script>` are always strict). Tests with `flags: [noStrict]` (sloppy) are
skipped as out of scope. A `flags: [raw]` test (verbatim source, no harness) also runs
in non-strict mode only per test262/INTERPRETING.md, but nearly all exercise
mode-independent syntax (hashbang, HTML-close comments, directive prologues) a
strict-only parser grades correctly, so those stay graded at their goal. The lone raw
test that is sloppy *by content* — `hashbang/use-strict.js`, whose `#!` turns
`"use strict"` into a comment, leaving a sloppy `with` — is out of scope for the same
reason `noStrict` is, so it is skipped (sloppy-mode bucket), not graded as a failure.

**Strict and the *goal* symbol are orthogonal axes** (ECMAScript §11.2.2): a parse
runs against either `Goal::Module` or `Goal::Script`, both strict. tsv exposes this
as `tsv_ts::parse_with_goal` (and `tsv parse|format --goal script|module`),
defaulting to **`Module`** — correct for Svelte `<script>` and ~all real TS. The
goal toggles only the four goal-specific constructs:

| construct | `Module` | `Script` |
| --- | --- | --- |
| `await` as identifier / binding / label / class name / param | reserved | **identifier** (`[~Await]`) |
| top-level `await` *expression* | allowed | error |
| `import.meta` | allowed | error |
| top-level `import` / `export` *declarations* | allowed | error |

(Dynamic `import(...)` is valid under both.) `sourceType` in the public AST follows
the goal.

**The runner grades each test at its declared goal**: a `module`-flagged test as
`Module`, everything else it grades (the run-both-ways default + `onlyStrict`) as a
strict `Script`. So the `await`-as-identifier tests (valid only in a strict Script)
parse correctly — making tsv **more spec-correct than acorn-as-module**, which is
module-only. This is a deliberate, spec-grounded divergence from the drop-in oracle's
*module-mode* behavior, not a bug. The context tracking is a single `[Await]` flag
saved/restored at every function-like scope boundary (async → `[+Await]`, non-async
→ `[~Await]`, class static block → `[+Await]`); a side effect is that `await` as an
*expression* in a non-async function is correctly rejected in module code too.

## Differential Comparison (tsv vs oxc-parser)

The pass rate above is **un-baselined** — a positive failure could be a genuine
tsv parser gap, or a test even other parsers reject. To triage, the harness can
emit a **manifest** of tsv's graded strict subset and a Deno consumer compares
each verdict against [oxc-parser](https://github.com/oxc-project/oxc):

```bash
# 1. Rust emits the manifest: one row per graded test (relative path, module
#    flag, expected verdict, tsv verdict). Honors the same path filters.
cargo run -p tsv_debug test262 --emit-manifest /tmp/t262.json

# 2. Deno consumer runs oxc-parser over the same files at each test's goal
#    (module-flagged → module, else script — mirroring tsv) and buckets the
#    agreement. Run from the repo root:
deno run --allow-read --allow-env --allow-ffi --allow-net --allow-sys \
  --config benches/js/deno.json \
  benches/js/diagnostics/test262_compare.ts --manifest /tmp/t262.json
```

**Why oxc-parser only (no biome).** test262 is a *parse*-conformance suite, and
oxc-parser is the alternative with a real, gradable accept/reject verdict
(`parseSync` + `errors`). Biome's `@biomejs/js-api` exposes **no parser** (only
format/lint), so it has no verdict to grade; it stays a *formatter* subject in
the bench, not here.

**Fairness — same subset, same goal.** The consumer runs oxc over *only* the
tests tsv grades (the strict, non-sloppy, parse-phase subset), parsing each at the
**same goal tsv grades it at** (`module`-flagged → `sourceType: 'module'`, else
`'script'`) — so the two sides agree on the goal axis. One caveat: oxc's `'script'`
is **sloppy** while tsv's `Goal::Script` is strict, so a *sloppy-by-content* script
would show up as a positive "tsv rejects, oxc accepts" candidate even though it's a
sanctioned strict-only divergence, not a bug. The one known such test
(`hashbang/use-strict.js` — see [Goal axis](#design-decision-strict-mode-only-explicit-goal-axis))
is skipped before grading, so it never enters the manifest; any future sloppy-by-content
script would surface here and want the same treatment. The two
actionable buckets:

- **positives where tsv rejects but oxc accepts** → tsv real-bug candidates (modulo
  the strict-vs-sloppy-script caveat above)
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
