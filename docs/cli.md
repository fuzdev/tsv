# CLI Architecture

## Command Pattern

The CLI uses [argh](https://crates.io/crates/argh) for declarative arg parsing:

- Each command is a `FromArgs` struct in its own module under `src/cli/commands/`
- `cli::TopLevel` holds a top-level `--version` switch plus the `Subcommand` enum; `main.rs` parses argv and dispatches, answering a bare `--version` itself ahead of argh (`cli::is_bare_version`) since the subcommand argh requires is not one, so a bare `tsv` gets argh's own required-subcommand error
- argh has no struct-flattening attribute, so the shared input fields (`--content`, `--stdin`, `--parser`, file path) are declared per command and assembled into `cli::input::InputArgs` for resolution

**Adding Commands**: Create `src/cli/commands/newcmd.rs` with a `FromArgs` struct and a `run()` method, declare it in `cli/commands/mod.rs`, and add a variant to `Subcommand` plus its `TopLevel::run` arm in `cli/mod.rs`.

## Shared Infrastructure

`tsv_cli` exports CLI infrastructure as a library, reused by `tsv_debug` and the integration tests for consistent UX:

- Input handling (file, `--content`, `--stdin`) — `cli/input.rs`
- File/directory discovery with extension filter, gitignore-aware ignore evaluation (hierarchical `.gitignore`/`.formatignore`/`.prettierignore`), and the build-output heuristic fallback where no `.gitignore` is in scope (the per-entry verdicts from `tsv_discover`) — `cli/discover.rs`
- The thread stack reservation on every route and the `--jobs` ceiling (`sized_thread`, `run_on_sized_stack`, `clamp_worker_count`, over the library's `tsv_lang::STACK_SIZE` / `run_with_stack`) — `cli/stack.rs`
- The `--pretty` re-indenter (tab-indented form of the compact wire, no deserializer) — `json_utils.rs`

## Binary Structure

- **`tsv` (production)**: Pure Rust, no external tool dependencies
  - Crates: `tsv_cli`
  - Commands: `parse`, `format`, `strip` (plus the top-level `--version` switch)
- **`tsv` npm bin, native (`@fuzdev/tsv`)**: the production `tsv` binary
  itself, shipped inside each `@fuzdev/tsv-<triple>` platform package and
  exec'd by the loader's `crates/tsv_napi/npm/bin.js` dispatcher (argv,
  stdio, exit codes, and signals forwarded verbatim) — so `npx tsv` on the
  native package has this CLI's exact contract, real `--jobs` parallelism
  included. When no binary is reachable the dispatcher falls back to the JS
  mirror below.
- **`tsv` GitHub Release asset**: the same production binary, one per platform
  package (`tsv-<triple>`, `tsv-win32-x64.exe`), attached to each
  [GitHub Release](https://github.com/fuzdev/tsv/releases) beside a
  `SHA512SUMS` (`sha512sum -c`) and the binaries' third-party `NOTICE`, for use
  without npm (`curl` + `chmod +x`).
  Pulled back from the npm registry at release time, so it is byte-identical to
  the binary the platform package ships; every asset carries a build provenance
  attestation (`gh attestation verify <file> -R fuzdev/tsv`). The checksum file
  and the attestation are this channel's whole integrity story; it has no
  signed release log of its own.
- **`tsv` npm bin, WASM (`@fuzdev/tsv-wasm`)**: `crates/tsv_wasm/npm/cli.js`
  — a hand-written Node mirror of this CLI's contract (subcommands, flags,
  exit codes, output streams, traversal rules). `--jobs` is real: path mode
  fans onto `node:worker_threads`, spawning `cli.js` as its own worker. Its two
  **defaults** are both smaller than the native CLI's:
  - **When to pool at all.** A pool costs tens of milliseconds to bring up here
    against the native pool's near-free thread spawn, so with no `--jobs` given
    the run stays single-threaded until the in-scope file count clears a
    threshold — measured per engine (lower on N-API than on WASM), with the
    shipped constants set above each so a pool is only taken where it clearly
    pays.
  - **How wide.** Not the native `min(logical, ceil(1.5 × physical))`. That
    rule assumes an idle machine, and on the WASM engine V8's own wasm tier-up
    is already using part of one before the first worker exists — so
    the pool peaks at half the physical cores and *regresses* past it. The
    N-API mirror has no compiler thread to compete with and peaks at the
    physical core count.

  An explicit `--jobs N` is held to the native CLI's `4 × logical` ceiling,
  warned about on stderr when it bites (see
  [§Multi-File Formatting](#multi-file-formatting)'s parallelism note; `cli.js`
  restates the native `clamp_worker_count` by hand — same constant, same
  message, so both surfaces refuse the same numbers). The logical count under
  that ceiling, and under both default widths, is Rust's `available_parallelism`
  on either side: the affinity mask capped by the cgroup CPU quota. Node's and
  Bun's `availableParallelism()` leave the quota out (Deno's applies it), so
  `cli.js` reads it itself (`cgroup_cpu_quota`, a transcription of std's) —
  under a `--cpus` or `CPUQuota=` limit every runtime names the native CLI's
  ceiling rather than one sized past the quota.

  Both also ACCEPT the same values, which takes its own restatement: the flag's
  value is parsed as a Rust `usize` on one side and by a hand transcription of
  `usize::from_str` on the other (`usize_from_str`) — ASCII digits, an optional
  leading `+`, nothing past `usize::MAX`, refused in `ParseIntError`'s own words
  — and `scripts/test_napi_npm.ts`, the only place both bins exist at once, runs
  the accept/reject table through both. Repetition needs no restatement:
  `cli.js` parses argv by a transcription of argh's grammar (`parse_argv`), so a
  second value for any value-taking flag (`--content`, `--parser`,
  `--source-type`, `--jobs`) is `duplicate values provided` on both, while
  repeated SWITCHES stay fine on both (argh counts them).

  The bound does a different job here than natively: a JS worker is a whole V8
  isolate (megabytes resident on either engine, where the native thread's
  reservation is lazily committed and costs almost nothing), so an unbounded width on a
  large tree hits the machine's *memory* — ending in an uncatchable OOM SIGKILL
  — long before the OS refuses a thread, and the file-count clamp bounds nothing
  on exactly the trees large enough to matter. An explicit width remains the
  only way to compare the two paths at a given size, which calibrating those
  defaults needed; every size that calibration uses is far under the ceiling on
  both.

  `--content`, `--stdin`, `--list` and `--jobs 1` are single-threaded on both
  CLIs — except `--jobs 1` in `cli.js` over the N-API engine, a pool of one
  worker, since a native overflow on the main thread is a `SIGSEGV` no catch
  survives and only a pool worker carries the reserved stack (§Recursion Depth).
  The parallel and single-threaded paths report identically (same sorted stdout,
  summary and exit code), so the split is a cost decision, not a contract one.

  One source: `cli.js` imports its engine from `./index.js`, so the copy staged
  into the native `@fuzdev/tsv` (as the dispatcher's fallback) binds to the
  N-API engine with no adapter — its workers, having no compiled module to
  inherit, load that engine themselves, while WASM workers take the main
  thread's module through the package's `./worker` entry and recompile nothing.
  Behavioral changes to `format`/`parse`/`strip` in either CLI must be mirrored in the
  other and in the CLI tests of `scripts/test_npm.ts` (wasm) and
  `scripts/test_napi_npm.ts` (native) — `strip`'s rows in `scripts/strip_suite.ts`, which both run.
- **`tsv_debug` (development)**: Uses embedded Deno sidecar for external tools
  - Reuses `tsv_cli` infrastructure
  - Commands, by family (the full catalog is the root [CLAUDE.md §Debug Tooling](../CLAUDE.md#debug-tooling) and [audits.md](audits.md), not duplicated here): fixtures (`fixture_init`, `fixtures_validate`, `fixtures_update*`, `fixtures_audit`), oracles (`check`, `compare`, `ast_diff`, `canonical_parse`, `format_prettier`, `test262`, `tsc_conformance` — see [typechecker.md](typechecker.md)), the standing audit family (`comment_audit`, `gap_audit`, `census_audit`, …), the compiler harnesses (`compile_*`, `canonical_compile`, `render_compare`), the type strip (`strip`, its `--stdin` the strip-corpus side), and profiling/metrics (`profile`, `json_profile`, `arena_stats`, `buffer_sizes`, `metrics`, `line_width`, `lex_diff`)

### External Tools (via Embedded Deno Sidecar)

`tsv_debug` calls these external tools via an embedded Deno sidecar (spawned lazily on first use; bulk commands spawn a small pool of sidecar processes — see `crates/tsv_debug/CLAUDE.md`):

| tool | used by | purpose |
| --- | --- | --- |
| **prettier** + **prettier-plugin-svelte** | `compare`, `format_prettier`, fixture management | format code, compare outputs, validate formatter behavior |
| **svelte** | `canonical_parse`, `ast_diff`, fixture management | parse Svelte code with the official compiler |
| **acorn** + **@sveltejs/acorn-typescript** | `canonical_parse`, `ast_diff`, fixture management | parse TypeScript code (matches Svelte's TS parser) |

Versions are pinned exactly in `crates/tsv_debug/src/deno/sidecar.ts`, the source of truth (not repeated here); `benches/js/package.json` pins the same versions for the bench harness — keep the two in sync (`deno task pins:audit` gates it).

## Input Handling

All content-processing commands support three input methods (`strip`, TypeScript's alone,
takes them without `--parser` — [§Stripping types](#stripping-types-tsv-strip)):

- **File path**: `command <file>` - parser from the extension, which must be one tsv handles (the dispatch has no unknown arm, so a `.md` would otherwise parse as TypeScript); an unsupported one is an argument error (`format <file>`'s message) unless `--parser` names the grammar. A directory is refused by name ahead of either check (`<dir>: is a directory (one file is expected)`): every command resolving a file this way — `tsv_debug`'s too — reads one file, and the read would otherwise fail under a message calling it one
- **Content**: `command --content <string> --parser <type>` - Requires explicit `--parser svelte|typescript|css`
- **Stdin**: `command --stdin --parser <type>` - Requires explicit `--parser svelte|typescript|css`

`parse`, `format` and `strip` also take `--source-type script|module` — ESTree's own spelling, and
the value the wire's `Program.sourceType` carries — for TypeScript only: naming one for a
Svelte or CSS input is an error, as on every JS binding, never a silent drop (the C FFI
alone accepts its module code `0` on every language, the neutral value of a required
`u32`, while rejecting the script code there — `tsv_ffi`'s `lib.rs` states the
carve-out). It selects the parse goal: at `script`, `await` is an ordinary identifier
and top-level `import`/`export`, `for await` and `import.meta` are errors (a TypeScript
namespace body keeps its `import`/`export`). Unset, `parse` uses `module`, while
`format` and `strip` use `module` **retried as `script`** if that parse fails (see
[§Multi-File Formatting](#multi-file-formatting)). For `format` and `strip` the flag applies to
`--content`/`--stdin` only — a path argument with `--source-type` is a usage error
(exit 2), since a path's goal is its extension's (and in `format`'s path mode Svelte and
CSS have no goal at all); `parse` honors `--source-type` on file paths too. The goal does not decide strictness:
Module code is strict, Script code is strict only once a `"use strict"` directive
prologue says so (see
[CLAUDE.md §Strictness](../CLAUDE.md#strictness-module-strict-script-by-directive); the
goal axis itself is
[conformance_test262.md §Module Strict, Script by Directive](./conformance_test262.md#design-decision-module-strict-script-by-directive-annex-b-out)).
Three constructs follow strictness rather than the goal: a `with` statement, a
leading-zero numeric literal (`010`, `08`) and a legacy string escape (`'\7'`,
`'\8'`) all parse in a sloppy script and are syntax errors in strict code.

### `--source-type` is a grammar input, not a style setting

tsv is non-configurable by design — "no config files, CLI flags, or runtime options"
([CLAUDE.md §Configuration](../CLAUDE.md#configuration)) — and `--source-type` is no
exception, because the contract governs **style**. What this flag
selects is which grammar symbol the parse starts from: ecma262 gives `ParseScript` and
`ParseModule` as two separate entry points over the same text, and a source that is a
script is not a module with a setting flipped. The flag shapes only the parse the
formatter runs; no `--source-type` value changes how anything is printed. The same axis appears on the bindings as the
`sourceType` option (`tsv_wasm`, `@fuzdev/tsv`) and as the C-ABI source-type code
(`tsv_ffi`); there is no style knob on any of them either.

`parse` emits the span-only wire by default — `start`/`end` offsets but no per-node `loc`
(line/column) object, and for Svelte no `name_loc` either — the same wire every binding
emits; `loc` is derivable from the offsets plus source, so nothing is lost for a consumer
that has the source. `--locations` adds it, through the Rust `loc` emitter (the one
product surface that writer has; it mirrors acorn's `locations: true`): the line (1-based)
and column (0-based, UTF-16 code units) of each object's own `start`/`end`, on every object
that carries them — CSS's included, which `parseCss` itself leaves bare — under the
document's line rule (ECMAScript's terminators for TypeScript, LF alone for Svelte and
everything in it, and for CSS). The JS mirror's `parse --locations` (`npm/cli.js`) gets the
same `loc` from the packages' `{locations: true}` — the JS reconstruction over the span-only
tree — so its tree deep-equals the native one while the key order differs (`loc` after
`end` natively, appended last in JS). Orthogonal to `--source-type` (the source type drives
the parser, `--locations` the writer), so the two compose.

Implemented in `tsv_cli/src/cli/input.rs`

### Parse errors

A source that does not parse is reported as the message, a located line, and a caret — after
`Parse error: ` from `parse` and from `format --content`/`--stdin`, and after `error: <path>: `
from a path-mode `format`:

```
Parse error: Expected identifier or destructuring pattern, found '='
2:7 const = ;
          ^ here
```

The `line:col` header is the error's point in the **wire's coordinates** — the `loc` a node
at the error would carry, under the same definition as `--locations` (above): the line
(1-based) under the document's line rule, and the column counted in UTF-16 code units,
printed 1-based. A
leading byte-order mark is counted on line 1 for TypeScript and elided for Svelte and CSS, as
their wires do. `format` reports the point in the file's own coordinates: its parse reads the
CR-folded text (see [CLAUDE.md §Line Terminators](../CLAUDE.md#line-terminators-parse-preserves-format-folds)),
and the error is mapped back onto the file's own bytes before it prints, so for the same error
it prints exactly what `parse` does. The same *error* is not guaranteed: with no
`--source-type` named, a TypeScript `format` retries a failed module parse as a script
([§Multi-File Formatting](#multi-file-formatting)) and can report the script attempt's error
where a module-only `parse` reports the module's — at one goal the two agree. The excerpt is a
display line, bounded by every ECMAScript terminator whatever the language, since a raw CR,
U+2028 or U+2029 printed mid-line garbles the text the caret points into; it never echoes a
leading byte-order mark, in any language (the header alone carries the wire's reading of one).
The caret is padded by the display width of what the excerpt prints ahead of it (a tab echoed
as a tab), so it sits under the error's character even where the header's UTF-16 column and
the screen column differ. The text is `tsv_lang`'s `Error` in its full (`{:#}`) form and is the same
on both bins and in every package — where the `{kind: 'parse'}` error the facade returns
carries it split in two, the summary as `message` and the header, excerpt and caret lines as
`frame` (so `` `${message}\n${frame}` `` is the text), beside a `start` / `end` / `loc` span
over the same point (`crates/tsv_wasm/npm/api.js`).

Two errors are not parse errors, and a single input prints them after `Error: ` rather than
`Parse error: ` (a path-mode `format` names the file either way): a source over the 4 GiB
size cap the parsers' `u32` offsets index, and a `format` refusal. Neither is a located
error (`Error::kind` is `TooLarge` / `Refusal`, not `Syntax` / `TooDeep`), which is how the
native bin tells them from a parse error (`cli::out::exit_with_parse_error`, which prints
the error's full `{:#}` form); `cli.js` tells them by the facade: the size cap throws,
and the refusal is a returned error of `kind` `'refused'` where a parse failure's is `'parse'`.

The refusal: a Svelte document holding a lone CR (not part of a CRLF) with comment text after
it inside a `//` comment between a tag's attributes. Svelte ends that comment at LF alone, so
the CR is comment text, and the format path's CR fold would end the comment there and print
the rest of its line as markup. (A CR with only whitespace after it, as in a doubled
`CR CR LF` ending, ends the comment where the fold does, and formats.) The message names the
CR's `line:col`; `parse` accepts the same bytes. It is a per-file error (exit 2) like any
other, and in the packages a returned `{kind: 'refused', message, frame, start, end, loc}`
error spanning the CR rather than a `kind: 'parse'` one (`tsv_svelte::__private::parse_folded`). A
document whose own bytes do not parse reports that parse error first, exactly as `parse` does.

## Stripping types (`tsv strip`)

`tsv strip` runs the type strip ([strip.md](./strip.md)) on **one** input — a file path,
`--content`, or `--stdin` — and prints the JS to stdout byte for byte: no newline appended,
every line terminator, a leading BOM and a hashbang kept. The file is never written. It
takes no `--parser` (the strip is TypeScript's), and a file path's extension must be one of
`.ts` `.mts` `.cts` (a `.svelte.ts` rune module and a `.d.ts` are `.ts`) — anything else is
an argument error naming that list (`tsv_discover::strip_unsupported_extension_error`), as is
a directory. A JavaScript path (`.js` `.mjs` `.cjs`) is refused with a reason of its own
(`a JavaScript file has no types to strip`) rather than read as TypeScript, where
`f(a < b, c > (d))` is a generic call whose type arguments the strip would blank and
JavaScript reads two comparisons. The goal is `format`'s
([strip.md §The parse goal](./strip.md#the-parse-goal)): unset, a module retried as a
script; `--source-type script|module` names it exactly for `--content` / `--stdin` (a value
naming neither is a usage error), and beside a path it is refused in `format`'s words, since
a path's goal is its extension's — a `.mts` a module with no retry, a `.ts` / `.cts` the
fallback.
`--content` and `--stdin` follow `parse`'s precedence between themselves, but either beside
a path is refused (`--content/--stdin cannot be combined with a file path`), as `format`
refuses it. Every argument is graded before anything is read, so a refused `--stdin` never
waits on its writer.

Exit codes — the `format` scheme, with "would change" read as "refused":

- **0** — stripped (printed, or with `--check` nothing printed);
- **1** — refused: the source holds TypeScript the strip cannot erase. stderr is
  `Error: cannot strip <the construct>` over the `line:col` header, line and caret of the
  refused span's start, in the same coordinates a parse error prints;
- **2** — a parse error (`Parse error: …`, the text `tsv format --content` prints for the same
  source and goal — with none named, the error of the reading it belongs to, per
  [§Multi-File Formatting](#multi-file-formatting)), a read error, invalid UTF-8, a source over
  the size cap, or a usage error.

`--check` prints nothing on stdout and keeps the same codes (a refusal still exits 1 and
says why on stderr), so CI can ask whether a file strips. argh's own flag-parsing refusals
exit 1, as for every command; stderr tells them apart. One input per run: several paths
are argh's `Unrecognized argument`. The JS mirror (`npm/cli.js`) answers every row alike,
byte for byte (`scripts/strip_suite.ts`, held to the native binary by
`scripts/test_napi_npm.ts`).

Implemented in `tsv_cli/src/cli/commands/strip.rs`

## Recursion Depth

The parser, the wire-JSON writers, the printers and the type strip all recurse over the
tree, and a stack overflow is not a catchable panic: no `catch_unwind` and no panic
contract can turn it into a per-file error. Natively it aborts the process (exit 134 on
Unix for the CLI; a bare `SIGSEGV` with no message for a Node or Deno process holding the
N-API addon or the FFI library, since Rust's guard-page handler is installed by its
runtime startup, which a library loaded into a host never runs), and a directory format
that dies that way has already rewritten files it never got to report. In WASM it traps,
and the trap strands the instance (below). So **the parsers bound the depth of the tree
they build**, and refuse a deeper input with a syntax error before any walk sees it.

### The depth limits

Two counts, both in `tsv_lang` (`NESTING_LIMIT`, `SPINE_LIMIT`), because no single
counter bounds both shapes a walk recurses through:

- **Nesting, at most 256 wire levels.** The parsers' own recursion, counted where the
  grammar recurses: each step charges **at least** the wire-JSON nodes it puts on the path
  from the root — one for a paren, an array, a call's arguments, a computed member, a
  template's `${` (a tagged template's quasi one more), an operator's operand, an
  assignment's right side, a conditional's branch, a unary operand, a `new`'s callee, a
  spread, a statement, a class expression, a parameter (a type-member signature's is paid
  for by its list's level, the signature's own being the member's), a parameter property's
  binding (its decorators sit on that binding, a level below a plain parameter's), a rest
  parameter's pattern (in any parameter list but a type member's signature the parens'
  level pays for it, since no wire node stands for them), a shorthand property's default
  (`{ a = d }`, the `AssignmentPattern` a pattern reads it as), a declarator, a `for`
  head's declaration, a `catch` clause, an
  enum member, a decorator, a declaration under its `export`, a tuple element (a named
  one's type one more), a conditional type's extends type and branches,
  an arrow's concise body; two for an object literal (the object and its `Property`), a
  type argument (the `TSTypeParameterInstantiation` and the argument), a function type's
  parameter (the function type and the parameter), an index signature's parameter type
  (the parameter and its `TSTypeAnnotation`), a type predicate's type, a mapped type's
  constraint, a template literal type's span, a `{@const}`'s binding and init (the
  `VariableDeclaration` and its `VariableDeclarator`), a Svelte element or block (the node
  and its `Fragment`), a CSS rule or at-rule (the node and its `Block`); three for a
  function expression's or an arrow function's block body, whatever the arrow's head. An
  arrow's bare parameter (`a => …`) is charged as the format writes it, in a pair: the
  parameter and the pair, a step that returns before the body. A step
  can charge more than its nodes — a list's own level where its first element's node
  already counts one, a parameter list's level beside a declaration's — but never fewer:
  `tests/depth_limit.rs` checks, for every shape it generates, that no path through the
  accepted input's wire nests past the limit, and for the calibrated shapes that it sits at
  it. An assignment over a target other than a bare name (`a[k] = x`, `({ [k]: a } = x)`)
  is one more level, found only once its `=` is read, when the target is already parsed:
  it raises the deepest nesting its frame has reached to the deepest the frame had reached
  by the end of the target, plus one, which every check after it charges, as it charges a
  printer's pairs (below). The right side, parsed after the target, is not under that
  level, so a right-nested chain of them (`a.b = c.d = …`) is charged its own depth, as
  any other nesting is. An
  optional chain's `ChainExpression` is the one wire node no walk recurses through
  (the wire writer emits it inline, and no other walk's tree holds it), so it is charged
  nothing. A CSS declaration value is a string on the wire but a tree to the
  printer, so its functions and parenthesized groups are counted too, one level each, as
  the value parser enters them — which counts what every walk of the value recurses
  through, an unquoted `url(`'s interior parens included, though the lexer reads that
  url-token whole.
- **Spine, at most 6,000 units.** The chains the parsers build *iteratively*, without
  recursing — a binary or logical operator chain (`a + b + c` nests to the left), a member
  / call / non-null / tagged-template / instantiation / `as` / `satisfies` chain, a type's
  `[]` suffixes, a qualified name — and the other nodes that wrap an operand parsed before
  the parser knew it was one: a conditional's test, a sequence's first expression, a
  conditional type's check type, a tuple element's `?`. Each repetition of one of those is
  a level no nesting count can see coming (`((a ? b : c) ? d : e)` nests a conditional per
  paren), so it is counted as a link. Their parse costs no stack, and neither do the walks
  that iterate a spine rather than recurse down it — every walk over an operator chain
  (the wire writers, the printers' chain collectors, the type strip), the wire writers
  over a member, call and non-null chain, the printer's questions about one and its
  member-chain flattening of one past its first few links, and the type strip over any expression chain
  (member, call, non-null, tagged-template, instantiation, `as` / `satisfies`) past its
  first few links — but every other walk recurses down the spine, so a flat chain needs a
  bound of its own; a nesting count never sees one. A link counts the units its walks
  spend stack for: one for an operator, a qualified name's `.`, a member access, a call or
  an instantiation; two for a non-null assertion; eight for an assertion; five for a
  type's `[]` or `[K]`, a conditional type's check type or a tuple element's `?`;
  fourteen for a tagged template; and ten for a conditional's test, a sequence's first
  expression, and a link whose walk **re-enters the expression printer**
  for its operand rather than continuing the chain it is printing. Which links
  re-enter is the printer's own answer, asked of the same predicates the printer decides
  by, so the price follows the walk: an operator whose left operand the binary-chain
  printer does not flatten into its chain (`a == b == c`, `a * b / c`, `a % b % c`,
  `a << b << c`, `a && b || c`, every `in` in a `for` header's init, an operator over an
  `as` / `satisfies`); a call on a call, through any run of `!` and instantiations
  (`f()()`, `f()!!()`, `f()<T>()`); a link over a tagged template, which the member-chain
  printer takes for its chain's base; and a member or `!` over a call the printer prints
  whole — a curried call on a member chain, or a call whose sole argument is a multi-line
  template — and that call itself. The fixed units are set by the build that spends the
  most on the link, over every build the contract covers, and graded by the gate below,
  which generates every pair of link kinds and a seeded sample of triples, rather than
  trusting a list. The re-entry, assertion and type-suffix weights keep about a quarter over
  the smallest one at which every leg holds its margin, and the WASM engine sets them: the
  re-entry weight by its shadow stack once V8 has tiered the walks up, an assertion's by
  that shadow stack in either tier, a type suffix's by the engine's own stack once tiered
  up. A non-null assertion's is the smallest a debug build holds, above a subscript's: a
  run of `!`, each with a block comment in its gap (`a!/*c*/!/*c*/`), is printed
  iteratively, so its links stay plain non-null assertions, but its doc nests a level per
  link, so the doc walks pay for it: the
  renderer and its fits walk run on explicit stacks, the flattening walk (`remove_lines` /
  `atomize`, which the template printer runs over an interpolation) recurses a few dozen
  doc levels and walks the rest on one too, and the build-time layout walk behind
  `will_break` recurses a small frame per level, which the stack gate below grades.

**The spine count bounds every root-to-leaf path, not one chain.** A path can pass
several chains (a chain whose leaf holds another, `(a + … + a) + a + …`), and the leaf is
parsed before the chain over it is known, so the count runs over frames: within a frame
every link adds up, and a closed frame hands its count up as the greater of it and its
siblings'. Frames open where siblings can run long — the statements of a list, the
operands of an operator chain, and the elements of every other list (array and object
elements, call arguments, sequences, declarators, class and type members, union and
intersection constituents, switch cases, enum members, parameters, type parameters and
type arguments, tuple elements, function-type parameters, heritage lists, and a template
literal's interpolations) — once the count has run past a few dozen units or a list past
eight elements, so short code pays nothing for them. The count is exact for
a lone chain and loose by at most those few dozen units elsewhere: a long list of short
chains (a data table, a minified file) is charged one element's chains, never their total. **Nesting is charged against the spine too**, at
15 units a level: each limit alone fits the stack, but a chain at its limit whose leaf is
nested to its limit costs both walks' stack at once, which neither limit sees. A link is
checked with the deepest nesting its frame has reached charged on top (at least 32 levels,
below which the count is not tracked), so a flat chain at shallow nesting gets 5,520 units.
The deepest nesting is framed as the count is — per statement, and per element of a long
list — so a chain is charged the nesting of its own paths, never a sibling's, and whether
a document parses does not depend on the order of its statements.

**The count runs document-wide.** A Svelte template expression, a `<script>` or a
`<style>` starts its own count at the depth of the markup around it (the embedded entry
points take that depth, `host_nesting`), so an expression deep in the markup has what the
markup left of the 256 levels, not all of them. A `<script>` or `<style>` nested inside the markup is raw text to the parser and
is parsed by the printer, from the depth of the deepest one; a body past the budget keeps
its author's bytes.

**The pairs the printer adds are charged as nesting.** A format writes grouping parens,
and argument lists, the source does not hold — `(a == b) == c`, `-(-a)`, `f((a = b))`,
`new A()`, `keyof (keyof T)` — and in the output each one is a level of nesting, which the
output's own parse counts. So the parse charges every such pair as it reads the input, at
the node the printer writes it around, asked of the printer's own predicates where it has
one:

- **an operand's position**: a binary's operand (the clarity pairs, the non-flattening
  chains, a function dividend), an `as` / `satisfies` operand (`async` included), a unary,
  prefix update, `await` or angle-bracket cast's operand, a spread's argument, a numeric
  literal a member access reads (`(1).a`), a function expression a chain calls or tags, an
  instantiation instantiated again (`(f<T>)<U>`), a class heritage, a `new` callee, a
  conditional's test and branches (`(a ?? b) ? (c as T) : d`, a consequent that is itself
  a conditional), an arrow's body (a conditional, a decorated class), and a decorator's
  expression where the bare decorator grammar cannot spell it (`@a!(…)` prints
  `@(a!(…))`; asked of the printer's own predicate, and counted only where the author's
  pair does not wrap the whole expression — a pair around a callee alone, `@(a!)(…)`,
  holds less than the printer's);
- **a node that takes a pair wherever it stands**: an assignment used as a value
  (everywhere but a statement's expression, a `for` header's own clause and another
  assignment's right side — a right side that becomes a destructuring default once the
  `=` after its pattern is read takes one again, `[a = (b = c)] = x`), a sequence (everywhere but a `for` header's init and update),
  and every `in` binary in a `for` header's init;
- **a shell or list the printer writes**: an angle-bracket cast's break shell
  (`<T>(⏎	x⏎)`), a unary's comment holder (`!(/* c */ x)`), a `return` / `throw` argument
  that is a binary chain (`return (⏎	a &&⏎	b⏎)`), an argument-less `new`'s argument
  list, a bare arrow parameter's pair (`a => …` prints `(a) => …`), a `let` heading a
  `for`-in head;
- **types**: a type operator over a type operator or an `infer`, a union or intersection
  member that binds looser than its neighbours (`(A & B) | C`), an array element or
  indexed access's object (`(typeof a)[]`, `(-1)[]`), an optional tuple element
  (`[(A | B)?]`), a union element of a tuple, a conditional true branch, a conditional
  type-parameter constraint, and an arrow's function return type.

The count at a node is the pairs the printer may write there, less the pairs the author
wrote: what the printer writes is a function of the tree and its comments, which a format
keeps, so on the output — whose pairs are the ones the printer wrote — the count is none,
and the reparse counts no more than the parse did. A pair is counted in the frame of its
node, against the nesting limit with the frame's deepest nesting under it, and against the
spine limit with the frame's links beside it. The frames open where they would for the
output, since no frame decision reads the pairs, so **the output of an accepted input
parses again**. `tests/depth_limit.rs` grades that — with the output being its own format —
for every shape it generates, a prefix-operator alphabet among them that nests every
prefix operator over every other, bare and commented, so a site the parse does not charge
fails the gate. The cost is a lower limit wherever the printer parenthesizes link by link:
an `==`, `%` or `<<` chain ends below the nesting limit's 256 operators — the 32 levels
every chain is charged come off it first — rather than near the spine limit, and a run of
`- - -` or of `<T>` casts at a half or a third of the 256 levels.

The refusal is an ordinary syntax error at the token where the count tripped (`Nesting
exceeds tsv's depth limit of 256 levels`, `Chain of operators, member accesses, calls,
assertions or type suffixes exceeds tsv's length limit` — no number, since the spine limit
counts weighted units with the nesting charged on top, which no count of links names; a
CSS value's at the `(` that passes the limit): `parse` and `format` report it like any other,
and its `Error::kind` (`ErrorKind::TooDeep`) tells it apart. A speculative parse that meets it hands it
back rather than trying the other reading, since that reading would be chosen for the
stack's sake, not the grammar's — and for the same reason a format's module goal refused by
it is not retried as a script, which could only succeed by reading `await` as a name. It is a deliberate over-rejection — every canonical
parser accepts deeper input, up to its own stack
([conformance_tsc.md §The reject-vs-defer line](./conformance_tsc.md#the-reject-vs-defer-line),
[conformance_svelte.md §Depth Limits](./conformance_svelte.md#depth-limits)) — set above
every input the conformance gates parse, the decision both limits rest on: the deepest
nesting is tsc's `parsingDeepParenthensizedExpression.ts` at 137 counted levels, the
longest spine tsc's two `binderBinaryExpressionStress` files, whose 4,953 operators come
to 5,440 units with the nesting charge.

### The stack contract

**Every walk of an accepted tree fits a 2 MiB thread in an optimized build, and a 16 MiB
thread in a debug one.** `tests/depth_limit.rs` holds the contract: for every shape the
limits count — each TypeScript expression, statement and type shape, the flat chains, CSS
rules, at-rules, selectors and value parens, Svelte elements, blocks and islands, the two
counts at once, a **catalog of grammar positions** (positions that hold an
expression, a statement, a type, a pattern, a member or a selector which can hold the
position again, each nested in itself — a position the catalog lacks is one the gate cannot see), and a **generated sweep** of every ordered pair, and a seeded sample of
triples, over the whole link alphabet (every chain link kind and every binary operator)
behind each of a set of heads, with the same links again with comments in their gaps; the prefix
alphabet: every prefix operator over every other, bare and with a comment in its gap, over
operands ending in each link and as each binary operator's operand, and over an authored
pair in each commented form; and the decorators: a class nested in a decorator's argument
on a class expression, a member and a parameter, behind every callee shape and inside
every payload (a spread, an assertion, a `!`, a member) — it generates
the deepest accepted input, checks that no path through its wire passes either limit (the
count charges at least what the walks recurse through, so a construct the parse leaves
uncounted fails here, whatever its stack), runs parse, both wire writers (span-only and
`loc`), format and the type strip on a thread nine tenths of the contract's size, checks
that the format's output parses again and is its own format, and that one level more is
refused. An overflow
aborts the test binary. The tenth kept in hand is the minimum margin the gate enforces: a frame that grows
fails the gate while every accepted input still fits the contract. Frame sizes move with
every printer change, and differ between optimizations by more than that margin, so the
gate runs in each world it speaks for: `deno task test:depth:napi` in the shipped
optimization (the `napi` profile, gating every release in publish.ts and every push in
CI's `artifacts` job) over the whole sweep, `deno task test:depth` in `deno task check`'s
`corpus` world over a smaller one — the pairs behind two of the heads, a smaller triple
sample, every commented link, the bare prefix pairs, a sample of the rest of the prefix
alphabet and of the decorators — and a plain `cargo test` against the debug contract, over the
named shapes and a fixed stride of that smaller sweep. `deno task test:depth:wasm` runs the
whole sweep's shapes at the same depths through the WASM package on a Node main thread
(below), in CI's `artifacts` job and publish's artifact validation.

**A consumer's debug tests sit below the debug contract.** Rust's test harness runs each
test on a 2 MiB thread, so `cargo test` over code that hands tsv its deepest accepted
input can overflow in a debug build. Each language crate re-exports a helper for exactly
this: `run_with_stack(f)` (`tsv_ts::run_with_stack`, and the same item from `tsv_css` and
`tsv_svelte`) runs `f` on a scoped thread reserving `STACK_SIZE` (32 MiB, twice the debug
contract), so `f` may borrow, and returns its value; it fails only when the OS refuses the
thread, and a panic in `f` resumes on the caller, so a test's assertion fails as it would
inline. Any thread of your own that runs tsv on input it does not control can take it the
same way, or reserve `STACK_SIZE` itself. Two alternatives need no code: raise the test
threads' stack to the debug contract with `RUST_MIN_STACK=16777216`, or build tsv optimized
in the dev profile (`[profile.dev.package.tsv_ts] opt-level = 2`, and the same for
`tsv_lang`, `tsv_css` and `tsv_svelte`), which keeps a dev build's checks and brings its
frames within the optimized contract's 2 MiB — the harness's own default.

### The CLI's stack reservation

`main` runs the whole subcommand on a thread with `STACK_SIZE` reserved (32 MiB — the
constant lives in `tsv_lang`, re-exported by each language crate beside `run_with_stack`,
which the CLI's `cli/stack.rs` builds on), and the format workers reserve the same, so the
stack is a property of tsv rather than of the route, the host and the platform: inherited,
the main thread's stack is the process `RLIMIT_STACK` on Unix — commonly 8 MiB, but
whatever the machine says — and **1 MiB on Windows**, where the linker writes it into the
executable header,
below the contract; a spawned thread takes Rust's 2 MiB. The reservation also keeps a
debug build of the CLI above its contract, and it names the threads (`tsv`,
`tsv-format`), because the runtime's `thread '<name>' has overflowed its stack` line is
the one diagnostic an overflow leaves. A reservation is address space, not memory: pages
commit lazily. Where the library's helper reports a refused spawn as an error, the CLI
warns and runs the subcommand on the inherited stack rather than failing the run, and it
answers a panic that reaches the top with exit 101.

**The wire has no reader ceiling of its own.** `tsv parse --pretty` re-indents the compact
wire bytes in one linear pass (`json_utils::indent_json_with_tabs`) rather than reading
them back into a tree, so it reads whatever the parser accepts; fixture
[`typescript/expressions/objects/nested_deep`](../tests/fixtures/typescript/expressions/objects/nested_deep/)
pins the pipeline past `serde_json`'s default recursion limit of 128 JSON levels. The
one reader of the wire left is `tsv_debug`'s `json` module, which reads with that limit
disabled on the same reservation.

### The other surfaces

The bindings run on their host's thread, so the host's stack decides whether the
contract holds there:

| surface | stack | at the limits |
| --- | --- | --- |
| `tsv` (this CLI), every route | `STACK_SIZE`, explicit | holds |
| N-API addon on the host's main thread | the host process's `RLIMIT_STACK` | holds at 2 MiB and up |
| N-API addon on a `worker_threads` worker | Node's 4 MiB `stackSizeMb` default | holds |
| FFI library | the calling thread (Deno's main thread: the process stack) | the `release` contract: holds at 2 MiB and up |
| WASM on Node, Deno or Bun, main thread | the 1 MiB shadow stack, and the engine's own | holds, cold and tiered up |
| WASM on a Node, Deno or Bun worker | as above, on the worker's stack | holds |

Every row was measured with every shape `tests/depth_limit.rs` generates, at its deepest
accepted depth, through the packages' API (parse with and without `locations`, the `_json`
string, format, strip); the WASM main-thread row on Node is the standing gate,
`deno task test:depth:wasm`, which runs V8's stack at nine tenths of its default, the
native gate's margin. A WASM build keeps its spilled frames on a 1 MiB shadow stack in
linear memory (wasm-ld's default) and its call frames on the engine's native stack, about
1 MiB on a V8 main thread — and **an engine's tier decides those frames**: V8's optimizing
compiler lays a recursive walk's frames out larger than its baseline compiler does, so a
call that fits a fresh process can overflow the same process once its code has tiered up.
The gate grades both, each pass in fresh processes: on the baseline compiler alone, and
after a warm-up pass has tiered up every function the run makes hot — under V8's default
dynamic tiering, which optimizes what runs hot; an engine flagged to compile everything
optimized up front (`--no-liftoff`) lays out larger frames still, and is outside the
contract. That is also why the
walks over a binary operator chain, the wire writers over a member, call and non-null
chain, and the type strip over any expression chain past its first few links, iterate the
spine rather than recurse down it: a frame per link of a chain thousands of links long
spends the optimized engine's main-thread stack whatever the per-link frame, where an
iteration spends none.

The packages' `{locations: true}` reconstruction (`locations.js`) walks the tree with an
explicit stack rather than recursion, so it adds no depth of its own.

Below those stacks, a WASM overflow is a trap the process survives but the *instance* does
not — the shadow stack pointer stays where the deep call left it, poisoning every later
call. The npm packages ship a `reinstantiate()` recovery hook, and the JS CLI (`cli.js`)
calls it on any trap in `format_one` — and on the `RangeError` an engine raises when a deep
call exhausts its native stack first, which strands the instance too — so a file that meets
either is one per-file error (`… (WASM engine trapped and was reinstantiated)`) and the
rest of the run formats normally, on the sequential path and in every pool worker alike. See
[tsv_wasm/CLAUDE.md §Panic Reporting](../crates/tsv_wasm/CLAUDE.md#panic-reporting).
`cli.js` gives every pool worker the native CLI's `STACK_SIZE` (`resourceLimits.stackSizeMb`,
restated as `WORKER_STACK_SIZE_MB`) where the engine honors it, and re-runs a file whose
main-thread format hit a `RangeError` in a one-worker pool (`retry_overflowed_files`). On
the N-API engine an overflow is a process-fatal `SIGSEGV` rather than a `RangeError`, so
`cli.js` never formats a path on the N-API main thread: a `--jobs 1` run there is a pool of
one worker (`resolve_route`); its single-input mode (`--content`/`--stdin`, `parse`) runs
on the main thread, under the table's main-thread row.

### What a level costs

A walk's stack per level is its frames', and **a frame is sized once for its widest arm**:
every level pays all of it whichever arm it takes, so a dispatcher that holds one by-value
AST node per arm multiplies that node's size by its arm count, at every level. That is why
no `parse_*` on the expression cycle hands its caller a bare `Expression` by value, and no
`parse_*` on the type ladder a bare `TSType`: a node builder either boxes into the arena at
its own tail (`alloc_expr` for expressions, `Parser::alloc` for types, leaving the caller
an 8-byte reference) or returns its own concrete node struct, whose wrapping temporary the
compiler merges with its sibling arms'. The printer answers the same pressure the same
way — the chain entry points fill a caller-owned `ChainNodeVec` rather than returning one.
The node enums answer it by density: a variant wide enough to set its enum's size on its
own, and rare enough that an arena allocation apiece is free, holds its payload by
reference (`Expression`'s five widest variants, `TSType`'s three, `Statement`'s rare
declaration heads), and a by-value field that only copies a node out of the arena
borrows it instead (`Property`, `VariableDeclarator`, every `Expression`-holding statement
head). The `size_of` asserts beside the AST pin those widths, so a variant that widens an
enum fails the test build (`cargo test`) instead of quietly eating the limits' margin (each
is rustc's layout, so `cfg(test)`-only; the wasm32 widths are pinned in `tsv_wasm`); the
margin itself is graded by `tests/depth_limit.rs`.

## Multi-File Formatting

`tsv format` accepts any mix of files and directories:

- **Discovery**: directories recurse over the JS/TS family (`.ts`/`.mts`/`.cts`/`.js`/`.mjs`/`.cjs`, all parsed as TypeScript — `.jsx`/`.tsx` are out of scope), `.svelte`, and `.css` (compound forms like `.svelte.ts` included), each extension read ASCII-case-insensitively (`A.TS`, `styles.CSS`), as prettier infers a parser from the lowercased name — a walk, a named path, the parser dispatch and the `.mjs`/`.mts` goal all read it the same way, on both bins.

  The **safety nets** `.git`, `node_modules`, `.sl`, `.hg`, `.svn`, `.jj` are always pruned. **A path an argument names is bounded by the ignore files alone.** The safety nets and the build-output heuristic prune what a walk *discovers* — they are tsv's guesses about a tree, and a guess never overrides a path someone typed — so they grade neither a named path nor its ancestors: `tsv format node_modules/pkg`, `dist/sub` or `.cache/x` walks what naming its parent walks there, and below the named root they classify every child as usual. An ignore rule, which the user or their repo wrote, bounds a named path — file or directory — exactly as it bounds the walk that would have reached it, through an ancestor or at the path itself: naming `pkg/dist` doesn't override a `dist/` rule, nor naming `vendor/x.ts` a `vendor/` one, the line prettier, ESLint, oxfmt and deno fmt draw too. Such a path is skipped.

  A named **file** a `.formatignore` or `.prettierignore` rule excludes is skipped quietly, as prettier skips it, whether or not a `.gitignore` excludes it too — at the file or an ancestor, and even where that `.gitignore` rule sits above the override rule and bounds the path on its own, the case the directory warning below names both blockers for: those files exist to say what not to format, the rule is the author's verdict on the file however the path is bounded, and a pre-commit hook handing over its staged files names such a file on every commit that touches one (ESLint, which warns there, ships `--no-warn-ignored` for exactly that noise; tsv has no flags to offer).

  Every other exclusion prints a stderr **warning** naming the file the excluding rule sits in and how to undo it — a `.gitignore` rule is about version control, so one excluding a named file is a surprise, and a named directory is a scope someone typed. A path only a `.gitignore` excludes gets the lines that re-include it and nothing beside it, for the repo root's own override file (read after every `.gitignore`, so its `!` wins): `!/src/a.gen.ts` for a path the rule matched itself, and for one under an excluded directory that directory re-included, its contents excluded again, and so on one level at a time down to the path (`!/build/`, `/build/*`, `!/build/a.ts`) — every line anchored with a leading `/`, without which a one-segment pattern matches at every depth, and spelling its path literally (each `*`, `?`, `[`, `]`, `\` and trailing space escaped), so a `[slug]` directory names itself rather than a character class. A path holding a control character other than a tab — a line feed, which splits any line; a carriage return at a file name's end, which a line's end drops; or any other, which an ignore file could hold only raw and the warning will not print — gets no lines, and the warning says to narrow the rule instead. The file named is the one the root reads: its `.prettierignore` where it has no `.formatignore`, since a `.formatignore` created beside it would shadow every rule in it.

  For a named directory, the rule named is the shallowest exclusion's — a `.formatignore`/`.prettierignore` rule over a `.gitignore` rule at the same prefix, the `.gitignore` rule where it excludes an ancestor above the override rule's — and the remedy turns on what *else* excludes the path. Only override rules: the rule is the user's own to narrow or negate. A `.gitignore` rule anywhere at or below the named one: narrowing an override rule would leave it standing, and every `!` under an excluded directory is inert, so the warning gives the lines that re-include the path level by level from the shallowest exclusion down, for the repo-root override file — "after that rule" when the named rule is a root override rule (a later line wins there), "which also override" a root override rule below a named `.gitignore` one — while an override rule in a *deeper* file, read after the root's lines, stays the user's to narrow and is named as the second blocker: one warning stating both, not one per rerun — wherever that deeper file was read. No ignore file inside a directory a rule excludes is read for a named path, as the walk that prunes the directory reads none — so nothing in one is warned about, and a rule in one is named, and can exclude the path, only once the directory is re-included and the path named again.

  A run whose every argument was an excluded **file** exits 0 rather than failing with `No files to format` — what a pre-commit hook hands over when only ignored files are staged — while a run left empty by an excluded **directory** is still that error, so a mis-scoped command fails loudly.

  File arguments share one ignore scope, moved from each argument's directory to the next's (popping back to their common ancestor and pushing down), so the ignore files above many named files are read and parsed once rather than once per directory holding one.

  A file arg is held to the extension check first — the parser dispatch behind a path has no unknown arm (everything that isn't `.svelte` or `.css` goes to the TypeScript parser), so a named `.json`/`.md`/extensionless file would be parsed as TypeScript: usually a baffling syntax error, and occasionally a *successful* rewrite of a file tsv doesn't support (a top-level-array `.json` reprints as a TS expression statement, semicolon and all, which is no longer valid JSON). Naming one is an **argument** error instead — reported alongside the unresolvable-path errors, failing the run upfront with nothing written, the same line prettier draws with "No parser could be inferred". A **directory** arg is a scope rather than a target, so the check doesn't apply to it: unsupported files inside are filtered out by the walk. A shell glob (`tsv format *`) names such directories as well: a safety-net or heuristic one (`node_modules`, `dist`) it walks, and one an ignore file excludes it skips, with the directory warning.

  Symlinks inside directories are not followed; pass them explicitly — and a passed one is graded **where it was typed**: a path argument that is itself a symbolic link is bounded by the ignore files at its own path (its parent canonicalized, its name kept, the repo root found from there), as `git check-ignore` and prettier read it, not where it points — a `.gitignore` naming `link.ts` excludes it, a link into a gitignored `build/` is not under `build/`, and a linked directory root is bounded by the rules of the repo it sits in even when it points outside that repo; only the overlap dedup below reads the link's target. And it is graded **as the link it is**, whatever it points at, as git grades it: a directory-only rule (`foo/`) does not match a symlinked directory argument (`git check-ignore foo` says not ignored), the link is walked as a directory once it is in scope, a warning's re-include names it as a file (`!/foo`, since a `!/foo/` reaches no link), and one a rule does exclude counts as an excluded *file* argument — a run naming only such links exits 0.

  Hard links are not detected either: two names for one inode are two files in scope, each read and formatted by its own name — a `--check` lists both, and a format rewrites the inode through whichever name it reaches first and finds the other already formatted, so it reports one name or both (the same bytes either way; under a parallel run the second name can also read the first's write in flight, the exposure any process writing a file while tsv reads it has).
- **Ignore files (two regimes, keyed on `.git`)**: for each directory root, the **format root** — the scope boundary, derived from the argument, never the cwd — is the **repo root** inside a git tree (a hard stop where the upward walk ends, so nothing above the repo is read and `--check` is reproducible) or the **filesystem root** outside one. The regime is decided **once at the target root**, and any ignored directory is pruned (its whole subtree is skipped).
  - **Inside a repo**, discovery honors, relative to the repo root:
    - **`.gitignore`** — hierarchical and repo-rooted exactly like git ([gitignore syntax](https://git-scm.com/docs/gitignore#_pattern_format), matched against `git check-ignore` on case-sensitive filesystems). This goes beyond Prettier, which reads only one `.gitignore` and one `.prettierignore`, both relative to its own directory (the cwd by default), and ignores nested ones entirely.
    - **`.formatignore`** — hierarchical (one per directory from the repo root down, deeper wins), applied after `.gitignore` so its `!` can re-include a gitignore'd path (subject to git's parent-directory rule). It and `.prettierignore` (below) are the **override layers**, one file per directory, read after every `.gitignore`.
    - **`.prettierignore`** — drop-in compat, honored **hierarchically** the same way, read as the override-layer fallback in any directory with no `.formatignore` of its own; a *sibling* `.formatignore` shadows it per-directory (used alone when present, even if that `.formatignore` is present-but-unreadable — a read error can't silently demote tsv's native file to prettier's). Like the hierarchical `.gitignore` above, this goes beyond Prettier's single cwd-relative `.prettierignore` — so a monorepo that runs `prettier` per-package (each package with its own `.prettierignore`) is honored from one repo-root tsv invocation. Because the shadow silently drops the sibling `.prettierignore`'s rules for that directory (Prettier applies *both* files), tsv emits a non-fatal stderr warning wherever a `.formatignore` shadows a `.prettierignore`, pointing at merging the patterns into `.formatignore`. **Compat caveat:** as an override layer a `.prettierignore` `!` can re-include a path `.gitignore` excluded (subject to git's parent-directory rule), whereas Prettier treats `.gitignore` and `.prettierignore` as independent sources OR'd together, where a `.prettierignore` `!` can't rescue a gitignore'd file — tsv's model is the more powerful superset, and the divergence only surfaces for a `.prettierignore` `!` targeting a gitignore'd path (rare).
  - **Outside a repo**, `.gitignore` and `.prettierignore` are not read (as git itself does); only `.formatignore` governs, hierarchically from the filesystem root down — so a `~/.formatignore` is global config for loose files. A `.prettierignore` in the **target root** (the directory tsv was pointed at, where prettier would have read it) raises a non-fatal stderr warning — rename it to `.formatignore`, or `git init` — without changing what gets formatted. The warning is bounded to the target root: outside a repo tsv's regime is `.formatignore`-only at every depth, so this is one courtesy heads-up at the entry point (not a per-directory scan), and an ancestor of a subdirectory target has no repo boundary to anchor on.
  - **Heuristic fallback**: a `.gitignore` in scope is **authoritative** and turns the heuristic off; with no `.gitignore`, the heuristic — hidden directories plus `dist`/`build`/`target` — is the fallback "not source" guess, except that an explicit override-layer `!` re-include overrides it.
  - **Re-include idiom**: to selectively re-include under a pruned (or otherwise ignored) directory, re-include the directory itself first — `!/dist/` admits the whole directory, then `/dist/*` + `!/dist/keep.ts` narrows it back to just the files you want. The leading `/` anchors each line to the directory of the ignore file holding it; without it a one-segment `!dist/` re-includes a `dist` at every depth. A bare `!dist/keep.ts` (without the directory re-include) is a **no-op** — the heuristic prunes `dist` before descending, mirroring git's parent-directory rule, and a gitignored (or tsv-excluded) `dist/` blocks a later `!dist/keep.ts` the same way.

    tsv emits a **stderr warning** in either case (non-fatal — no effect on the exit code, stdout, or `--list`/`--check` output), once per pruned directory, saying what pruned it (the heuristic, or the rule and its file), naming the file the re-include was written in, and spelling the lines that reach what the rule named, for that file — anchored and relative to its directory, since a line spelled from the repo root does nothing in a nested file, and outside a repo, where the format root is the filesystem root, in any file: `!/dist/`, `/dist/*`, `!/dist/keep.ts` for `!dist/keep.ts`, with `!/dist/sub/`, `/dist/sub/*` between for a nested `!dist/sub/keep.ts` (every directory down to the target has to be opened, and a `/dist/*` alone would close `dist/sub` again), the last lines being every re-include written under the directory, each the author's own pattern anchored (a `/dist/*` line would silence any it did not re-spell after it) — so a glob (`!dist/*.ts`) keeps its glob, and a rule reaching below the directory (`!dist/**/keep.ts`) opens it with `/dist/**` + `!/dist/**/` in place of `/dist/*`, git's idiom for every directory and no file. An override layer is read after every `.gitignore` and a later line wins within a file, so the lines override the excluding rule wherever it sits, except an override rule in a file *deeper* than the re-include's, which is read after it and which the warning says to narrow or negate instead — beside the lines that pass a `.gitignore` rule standing behind it, which narrowing alone would leave in force.
  - **Subdirectory invocation**: because the boundary is found by walking up, the repo-root rules apply even from a subdirectory, and a subdirectory named directly is bounded by the same ignore rules as when it is reached via an ancestor — only the safety nets and the heuristic, which grade no named path, can tell the two apart. But a tree that *contains* repos (a non-repo directory with `.git` subdirectories below it) does not honor the inner repos' `.gitignore`s — run tsv per repo.
  - **Unreadable ignore files**: a `.gitignore`/`.formatignore`/`.prettierignore` that is present but can't be read (invalid UTF-8 — read strictly, as in the invalid-UTF-8 bullet below — or a permission error) is **not** silently treated as absent: tsv emits a non-fatal stderr warning and drops that file's rules (so an unreadable `.gitignore` also leaves the build-output heuristic *on* for its subtree). A file that genuinely isn't there, or is deleted between the directory listing and the read, stays silent. This is also a `--check` reproducibility hazard — surfacing it is the point. *Present* means a regular file, reached through a symlink when the name is one; a directory of that name is not an ignore file and stays silent — the same rule in a walked directory and a preloaded ancestor. The one exception is `.gitignore`, which git never reads through a symbolic link in a working tree (gitignore(5)): a symlinked `.gitignore` is not applied — so the build-output heuristic stays on for its subtree, as for an unreadable one — and warns, by the same rule in both walks. `.formatignore` and `.prettierignore` keep reading through links, as prettier does.
  - **`--check` reproducibility** assumes the ignore files are **committed**: a local/uncommitted `.formatignore` or `.prettierignore` (or git's unread `.git/info/exclude` / `core.excludesFile`) makes a clean CI checkout disagree.
  - **Shared by construction**: the matcher is the `tsv_ignore` crate's `IgnoreStack`; the per-directory prune/descend policy (heuristic, safety nets, the shadow warning) is the `tsv_discover` crate's verdict. The WASM CLI, the native npm package, and editors call into the same two crates, so every surface agrees rather than hand-mirroring the logic. See `cli/discover.rs`.
- **Piped output — a closed consumer is not a failure.** `tsv format . | head`
  fills the 64 KiB pipe buffer on any tree whose changed-path report exceeds
  it, so `head` has exited by the time the rest is written. **Both bins stop
  writing and finish the run on their own terms**: the exit code still reports
  the work (0 clean, 1 `--check` would-change, 2 errors) and the stderr summary
  still prints when stderr is not the closed fd, so `tsv format . | head` stays
  informative. The rule covers **both fds** — `2>&1 | head` closes the same
  pipe for both, and a stdout-only rule would just move the failure one line
  down, onto the summary.

  Why not the two alternatives, since this is a shipped exit-code contract:
  **141** (what a tool killed by `SIGPIPE` reports) and **restoring `SIGPIPE`
  to `SIG_DFL`** both replace the 0/1/2 verdict with "the reader left", and for
  `--check` the exit code *is* the API. Exiting 0 is also the honest answer for
  `format`, whose stdout is a *report* of files already rewritten rather than
  the product: a reader that left does not un-format them. And only this answer
  is one both bins can give identically — Node ignores `SIGPIPE` too, so
  `cli.js` could never die by the signal, only fake a code where the native
  side died by one. Any **other** write error still aborts loudly on both: a
  report truncated by a full disk, with nothing said about it, is worse than a
  crash.

  The mechanism differs because the two runtimes fail differently. Native: Rust
  sets `SIGPIPE` to `SIG_IGN` at startup, so the write returns `EPIPE` and
  `println!`/`eprintln!` panic on it — every byte therefore goes through
  `cli/out.rs` (`write_stdout` / `write_stderr`, and the `out_line!` / `err_line!`
  macros over them), which absorbs `BrokenPipe`, waits out `WouldBlock` (a backoff
  capped at a millisecond, partial writes honored), and panics on anything else.
  `WouldBlock` is reachable because the fd's blocking-ness belongs to the open
  file description a child shares with its parent: a Node parent that opens its
  own piped `process.stdout` after spawning `tsv` asynchronously (a task runner
  logging beside it) flips that description to non-blocking under the running
  child — libuv resets fds 0–2 to blocking at the spawn itself, so the
  `@fuzdev/tsv` loader, waiting in `spawnSync`, never does. `parse` rides the same
  writer, so a closed reader there does not report `1`, its parse-error code.

  `cli.js` writes both fds **synchronously** (an async `process.stdout.write`
  before `process.exit` truncates), which is only safe while the fd stays blocking
  — and it takes that away from itself: spawning the worker pool pipes the
  workers' stdio through the parent, which flips fd 1 to non-blocking. Its
  `write_fd` therefore loops over `writeSync`, honors partial writes, sleeps 1 ms
  and retries on `EAGAIN`, goes quiet on `EPIPE`, and on any other error prints
  `failed printing to stdout: …` and exits with the shipped native binary's abort
  status (134) — never a verdict code, since an uncaught throw's 1 reads as
  `--check`'s would-change. The two bins reach `EAGAIN` by different roads —
  `cli.js` flips its own fd, the native CLI inherits a flipped one — and answer it
  alike.

  **The read side takes the same rule**: a parent that opens its own piped
  `process.stdin` flips fd 0 the same way, and `--stdin` on either bin
  (`Input::from_stdin`, `cli.js`'s `read_fd_to_end`) waits out `EAGAIN` there too
  rather than reporting a slow writer as a read error the moment the pipe is
  momentarily empty.

  A consumer that is merely **slow** gets every line on both bins: `EPIPE`
  ends the output, `EAGAIN` is waited out, and any other write error is a
  failure. Pinned by `tests/cli_tests/` (`*_closed_pipe_*`, `*_slow_pipe_*`,
  `*_non_blocking_stdout_*`) and `scripts/test_npm.ts`'s twin rows.
- **A non-UTF-8 file NAME**: the native walk joins each entry's raw bytes, so a file or directory whose name is not UTF-8 is discovered, formatted, and listed by its own bytes on unix (the changed-path report and `--list` print paths as bytes there, `cli::out::path_bytes`; elsewhere the U+FFFD spelling is all there is). A stderr diagnostic spells such a name with U+FFFD on every platform (`cli::out::path_text`), deliberately: a diagnostic is prose for a reader and a script reads stdout, and a byte-faithful stderr would put raw non-UTF-8 into the one channel every consumer reads as text.

  Two limits: such a name reaches the matcher in its lossy spelling, so a `?` or class counts one U+FFFD where git counts each byte (the multibyte edge `tsv_ignore`'s CLAUDE.md scopes the git parity to ASCII by), and a non-UTF-8 path given as an *argument* is refused at the argv boundary (`Invalid utf8: …`, exit 1 — argh reads `&str`), so it is reachable only through a directory.

  `cli.js` sees such a name only as Node spells it, lossily, on every route, so it reports the file as unreadable rather than formatting it — a deliberate, permanent split: Node hands the bytes back only as `Buffer`s (`readdir`'s `encoding: 'buffer'`), which would have to travel as a second path type through the walk, the ignore-file reads, the sort, the canonical-path dedup (`realpathSync` resolves a `Buffer` path through its lossy spelling and fails), the worker handoff and the report, for a name no published repo holds.
- **A path holding a control character or a double quote is printed C-quoted, as git prints one.** One rule for every path either bin prints — the `--list` and changed-path lines on stdout, the per-file `error:` lines, the traversal errors, the ignore-file warnings and the warning naming an excluded argument, a bad path argument, `parse`'s read failure — stated once in `tsv_discover::quote_path` (the warnings are built there already; the native CLI routes its own paths through `cli::out::path_bytes` / `path_text`, and `cli.js` restates the rule by hand, as it does `clamp_worker_count`): a path prints verbatim unless it holds a control character (U+0000–U+001F, U+007F) or a double quote, in which case the whole path is wrapped in double quotes and C-escaped the way `git ls-files` prints such a name under `core.quotePath=false` — `\a` `\b` `\t` `\n` `\v` `\f` `\r` `\"` `\\` by name, any other control character as three octal digits (`"r\033s.ts"`), every other byte as itself (a character outside ASCII prints raw even inside the quotes, as git prints it, and so does a non-UTF-8 byte on a unix stdout listing; a stderr diagnostic spells that byte as U+FFFD, above). A backslash escapes inside a quoted path but does not trigger the quoting on its own — every Windows path holds one — so a quoted path unquotes exactly as git's does while a plain `src\a.ts` prints as it is. Two things follow. A diagnostic that names such a path stays one line (raw, a line feed in the name would split the `warning:` line in two and a carriage return garble the terminal). And the stdout listings stay parseable by whatever already reads `git ls-files` or `git status`: a line beginning with `"` is a quoted path, any other line names the file exactly, so a script over `tsv format --list` or the changed-path report can unquote with the routine it uses for git (or treat the leading `"` as the tell that a name needs it).

  The one text that stays literal is a re-include **pattern** a warning offers (`` `!/build/n<TAB>o.ts` ``): it is meant to be pasted into an ignore file, which reads no escapes, so it is spelled as the file must hold it — and a pattern that would hold any control character but a tab is not offered at all, as above (a tab is spelled raw: it renders as the whitespace it is, and the pasted line reads it back as the file holds it). The ignore-file names in prose (`the repo-root .gitignore`) hold nothing to quote.

  Pinned by `tests/cli_tests/` (`test_format_quotes_a_path_holding_a_control_character_wherever_it_prints_it`, `test_argument_errors_quote_a_path_holding_a_control_character`) and `scripts/test_npm.ts`'s twin row; the spellings themselves by `tsv_discover`'s `quote_path_*` tests, taken from git 2.47's own output.
- **Invalid UTF-8 in a source file**: reading is **strict UTF-8 on both CLIs**
  — the native one because Rust's `read_to_string` refuses invalid bytes, and
  `cli.js` because it decodes through `TextDecoder(..., {fatal: true})` rather
  than `readFileSync(path, 'utf-8')`, which would substitute U+FFFD. The
  distinction is not cosmetic on the format path: a stray byte inside a string
  literal still parses after substitution, so a lossy reader would write the
  repaired text back over the author's file and call it formatted. Both bins
  instead report `read failed: stream did not contain valid UTF-8`, count the
  file as an error, and leave every byte in place. The same strict-UTF-8 rule holds
  for `--stdin` and for `parse`, each with its own message (`Error reading from
  stdin: …`, `Error reading file <path>: …`).
- **Source type: module, retried as a script.** Path mode names no source type — a directory can hold Svelte and CSS beside JS/TS, and there is no one grammar to declare for the run — so each JS/TS file is parsed as a **module**, and only if that parse *fails* is it retried as a **script** — on the grammar: a module the depth limit refuses takes no retry, since the limit is no verdict on the grammar and the script grammar could accept the text only by reading `await` as a name (`await <T>a` as `await < T > a`), so the refusal stands ([§Recursion Depth](#recursion-depth)). That is what lets a legacy sloppy script (a `with` statement, a leading-zero literal or escape, `await` as an ordinary name) format from a bare path. The retry runs on the error path only, so nothing the module grammar already accepts is ever reinterpreted, and the printer never reads the goal — no formatted output changes for any module-valid file.

  When **both** grammars reject the file, the reported error is the attempt's whose grammar the file was written against, decided in two steps. A script retry that died on a **goal gate** — a top-level `import`/`export`, an `import.meta`, a top-level `for await`, or the operand a module reads after a top-level `await`, the constructs only a module holds — has proved the file a module wherever that construct sits, so the module error is reported: a broken module's script attempt dies there even when its real error comes first (definitions first, `export` at the bottom — position alone would blame the valid `export` line). Otherwise the error that reached **further into the source** is reported, the module's on a tie: a broken *sloppy script*'s module attempt dies early at its first `with`/legacy literal/`await` name — the construct the retry exists to admit — so the script error (the typo) is reported rather than a pointer at a line tsv accepts. prettier's babel parser reaches the same answer by tolerating those strict-mode productions at the module goal (`allowedReasonCodes`). Pinned by `tests/format_fallback_error_attribution.rs`.

  An explicit `--source-type` is **exact** — `--source-type module` refuses a script-only source rather than retrying — which is why it is a usage error in path mode rather than a per-run override. `parse` has no fallback at either surface: its wire's `Program.sourceType` is a claim about which grammar produced the AST, and one settled goal has to produce it. The same rule reaches every format surface that takes no source type from its caller: the JS CLI's path mode, an editor's `format_typescript(source)`, and the `format_*` exports of all three bindings called with no `sourceType`, and the type strip — `tsv strip` and `strip_typescript` read an unset goal the same way ([strip.md §The parse goal](./strip.md#the-parse-goal)).

- **Two extensions settle the goal themselves, and skip the retry.** `.mjs` and `.mts` are ES modules whatever any config says — Node loads a `.mjs` as ESM unconditionally, and TypeScript maps both to `ModuleKind.ESNext` with the extension overriding `module` — so a path with one of those names is parsed as a **module with no script retry** (`tsv_ts::Goal::from_extension`). The fallback above exists to reach a *legacy sloppy script*, and a file that is a module by its own name cannot be one; without the narrowing, `tsv format a.mjs` would format a `with` statement that no runtime would load. Nothing else settles a goal: `.js`/`.ts` are ambiguous by design, and `.cjs`/`.cts` are the CommonJS half of that same switch — script *code*, but nothing in tsv's output turns on it, so they keep the fallback with the rest. The narrowing can only ever *reject* a file the fallback would have formatted: the retry runs on a module-parse failure alone, so no module-valid source formats differently. Both `tsv` bins apply it — the native CLI per file in `format_file`, and `crates/tsv_wasm/npm/cli.js` from a hand-restated copy (as with `clamp_worker_count`), pinned on both sides. `parse <file>` reads no extension rule: with no flag every file parses as a module already, and an explicit `--source-type script` on a `.mts` is honored as the caller's exact claim rather than refused — the precedence prettier draws too (its `__babelSourceType` option beats `getSourceType(filepath)`), and the one parse takes on every surface, since the wire's `sourceType` is what was asked for.

  An **editor** keeps the fallback, because it has no path to read: the VS Code extension dispatches on the document's `languageId` (`.mjs` arrives as `javascript`, `.mts` as `typescript`) and calls the binding's bare `format_typescript(source)`. So a `.mjs` holding a sloppy script is unformattable from the CLI and formats on save — the same split the goal axis draws everywhere between a surface that names a file and one that is handed a buffer.
- **Fail-fast args, isolated traversal**: path args that don't resolve to a file or directory fail the whole run before anything is written (every bad arg reported); traversal errors below a valid root (e.g. an unreadable subdirectory) report to stderr and discovery continues. A relative directory root that cannot be made absolute — its working directory was deleted out from under the run — is such an error for that root (`cannot resolve a relative path: the working directory is unavailable`) rather than a walk anchored on no format root, which would read none of its ancestors' ignore files.
- **No per-file options**: formatting style is fixed (see [CLAUDE.md §Configuration](../CLAUDE.md#configuration)). In particular `<svelte:options preserveWhitespace />` is not detected — whitespace handling is uniform, with only `<pre>`/`<textarea>` content whitespace-sensitive; see [conformance_svelte.md §Template Whitespace](./conformance_svelte.md#template-whitespace-clean_nodes).
- **Deduplication**: with multiple path args, overlapping spellings of the same file (`src` vs `./src`, absolute vs relative, symlink aliases) dedupe by canonical path, keeping the first spelling in sorted order. Only arguments that can overlap pay for it — a file argument among them, or one directory root an ancestor-or-self of another; a single root or disjoint roots can't produce duplicates, so the per-file canonicalization is skipped.

  Discovery's **warnings and traversal errors** collapse the same way: an ignore-file warning (a shadowed `.prettierignore`, an unreadable ignore file, a `.prettierignore` outside a repo) and a traversal error (an unreadable directory) name their directory by its absolute path, whichever root or argument spelling reached it, so `tsv format . sub` — the repo root walked as `.` and preloaded as `sub`'s ancestor — and `tsv format . ./` each warn once, and `tsv format t ./t` reports an unreadable `t/locked` once.

  Every canonical path the native walk takes — the dedup key, the working directory, a named path's parent — is read through one `canonicalize` that strips the verbatim prefix Windows' `fs::canonicalize` returns (`\\?\C:\…` → `C:\…`, `\\?\UNC\s\v` → `\\s\v`; a no-op on unix), so a diagnostic naming the format root outside a repo prints `C:\`, and one argument whose parent would not canonicalize (spelled by the lexical fallback) does not read as a different format root from its neighbor that did. Verified against std's own Windows path code rather than on a Windows runner: every fs call re-adds the prefix itself where a path is long enough to need it (`maybe_verbatim`, behind `read_dir`, `metadata`, and the `File::open` under `read_to_string` and `write`), and passes a shorter absolute path to Win32 as spelled — the spelling every walked path, joined from the argument, already takes — so the strip opens no access path the walk did not already use. The one residual is a name only a verbatim path reaches, a trailing dot or space Win32 normalizes away or a reserved device name, which git refuses to check out on Windows and which the walk never reached through the argument's spelling either. Node's `realpathSync` never returns the prefix, so `cli.js` needs no strip.
- **In-place writes**: files are rewritten only when output differs (no mtime churn), in place — a plain truncate-and-write, as prettier does, which keeps the inode, its mode and any hard link to it, and means a process killed or a disk filled mid-write can leave the file truncated (an atomic temp-file-and-rename would swap the inode and break hard links; not planned). `--content`/`--stdin` keep printing to stdout.
- **`--check`**: lists files that would change without writing; exits 1 if any would. For CI. Also works with `--content`/`--stdin` (nothing printed to stdout; the exit code is the API) for editor integrations.
- **`--list`**: prints the discovered in-scope files (one per line, C-quoted by the path-quoting rule above where a name needs it) without formatting — a read-only view of the set `format` would touch, after the ignore files are applied. Path mode only (errors with `--content`/`--stdin`), mutually exclusive with `--check`, and takes no `--jobs` (it spawns no pool, so a width there is refused as it is with `--content`). Unlike the format action, an empty scope is a valid answer (exit 0, no output) rather than the "no supported files" error; traversal errors still exit 2. Useful for debugging ignore-file scoping and for scripting over the set.
- **Parallelism**: files format concurrently on `std::thread::scope` workers claiming one file at a time from a shared queue — dynamic load balancing with no thread-pool dependency. `--jobs N` overrides the worker count, floored at 1 (`--jobs 0` is a width, not an opt-out — it means `--jobs 1`) and clamped to the file count where that is known up front — explicit file arguments and multiple roots; a single directory root streams (below) and cannot know the count before it walks, so it spawns the full width and the surplus workers park; path mode only, an error with `--content`/`--stdin`. Each worker reserves the same stack every other tsv thread runs on (`STACK_SIZE`, `cli/stack.rs`), so the pool is not a route with a depth ceiling of its own — see [§Recursion Depth](#recursion-depth).

  **An explicit `--jobs` is held to `4 × logical CPUs`**, warned about on stderr when it bites. Four per core is far past what the workload can use — the *default* lands below the logical count for measured reasons — so the ceiling is about blast radius, not throughput: each worker reserves `STACK_SIZE` of address space, and an unbounded count takes task slots until the OS refuses, which on a systemd machine is the login session's whole `TasksMax` and wedges every other process on it.

  **And a `--jobs` the OS still won't give narrows the pool rather than failing the run.** The count is a user-supplied number, so a refused thread is an ordinary outcome of an ordinary argument, and the work is *claimed* rather than partitioned — however many workers exist drain the whole list between them. tsv warns (`warning: only N of M format workers started`) and formats the tree; if not one thread could be started, it says so and formats on the calling thread. Both messages are the JS CLI's, word for word.

  The default is **`min(logical CPUs, ceil(1.5 × physical cores))`**, not one worker per logical CPU. This workload does not scale onto SMT siblings — the per-file work is memory-bound, and on a large tree the discovery walk is the bottleneck, so extra workers compete with it for cores. One worker per logical CPU costs up to 28% on walk-bound trees while buying nothing on flat repos. The SMT width is read once from `/sys/devices/system/cpu/cpu0/topology/thread_siblings_list`; where that is unavailable (no SMT, or a non-Linux platform) the cap is inert and the default is the logical count, so it can only ever lower the worker count.
- **Streaming discovery**: a single directory root — the common invocation — feeds the workers *as the walk finds files*, so the directory walk runs beside the first files' parse+format rather than in front of an idle pool. It is worth having: the walk is 5–10% of the wall on an application repo, and 40–67% on a repo with a large tree, where it can outrun what the pool consumes. Other argument shapes (explicit files, multiple roots) discover the whole set first, because the canonical-path dedup above is set-wide. The set of files formatted is identical either way, as is the reporting order below — only the order work is handed out differs.
- **Error isolation**: a per-file read/parse/write error (or panic, caught via `catch_unwind` — effective only in builds with `panic = "unwind"`; release uses `panic = "abort"`) reports to stderr and processing continues.
- **Deterministic reporting**: changed paths print to stdout in sorted-path order regardless of completion order — component-wise, each component by code point, the order a path's UTF-8 bytes sort in, and the same order on both bins (`cli.js` compares code points rather than UTF-16 units, which would put an astral-plane name ahead of U+E000..U+FFFF) — each spelled by the path-quoting rule above; errors (traversal and per-file) and the summary line go to stderr.
- **Exit codes**: 0 clean, 1 would-change (`--check` only), 2 errors.
