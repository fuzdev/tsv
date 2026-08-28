# CLI Architecture

## Command Pattern

The CLI uses [argh](https://crates.io/crates/argh) for declarative arg parsing:

- Each command is a `FromArgs` struct in its own module under `src/cli/commands/`
- `cli::TopLevel` holds a top-level `--version` switch plus the `Subcommand` enum (`Option` solely so a bare `--version` parses; a bare `tsv` reproduces argh's required-subcommand error — see `TopLevel::run`); `main.rs` parses argv and dispatches
- argh has no struct-flattening attribute, so the shared input fields (`--content`, `--stdin`, `--parser`, file path) are declared per command and assembled into `cli::input::InputArgs` for resolution

**Adding Commands**: Create `src/cli/commands/newcmd.rs` with a `FromArgs` struct and a `run()` method, add a variant to `Subcommand` in `cli/mod.rs`.

## Shared Infrastructure

`tsv_cli` exports CLI infrastructure as a library, reused by `tsv_debug` for consistent UX:

- Input handling (file, `--content`, `--stdin`) — `cli/input.rs`
- File/directory discovery with extension filter, gitignore-aware ignore evaluation (hierarchical `.gitignore`/`.formatignore`/`.prettierignore`), and the non-git heuristic fallback — `cli/discover.rs`
- JSON utilities (tab-indented serialization) — `json_utils.rs`

## Binary Structure

- **`tsv` (production)**: Pure Rust, no external tool dependencies
  - Crates: `tsv_cli`
  - Commands: `parse`, `format` (plus the top-level `--version` switch)
- **`tsv` npm bin, native (`@fuzdev/tsv`)**: the production `tsv` binary
  itself, shipped inside each `@fuzdev/tsv-<triple>` platform package and
  exec'd by the loader's `crates/tsv_napi/npm/bin.js` dispatcher (argv,
  stdio, exit codes, and signals forwarded verbatim) — so `npx tsv` on the
  native package has this CLI's exact contract, real `--jobs` parallelism
  included. When no binary is reachable the dispatcher falls back to the JS
  mirror below.
- **`tsv` npm bin, WASM (`@fuzdev/tsv_wasm`)**: `crates/tsv_wasm/npm/cli.js`
  — a hand-written Node mirror of this CLI's contract (subcommands, flags,
  exit codes, output streams, traversal rules). `--jobs` is real: path mode
  fans onto `node:worker_threads`, spawning `cli.js` as its own worker. Where
  it differs from the native CLI is in the two **defaults**, both of which are
  smaller here:
  - **When to pool at all.** A pool costs tens of milliseconds to bring up here
    against the native pool's ~50 µs of thread spawn, so with no `--jobs` given
    the run stays single-threaded until the in-scope file count clears a
    threshold — measured at ~565 files on the WASM engine and ~394 on N-API,
    with the shipped constants set above each so a pool is only taken where it
    clearly pays.
  - **How wide.** Not the native `min(logical, ceil(1.5 × physical))`. That
    rule assumes an idle machine, and on the WASM engine V8's own wasm tier-up
    is already using roughly a third of one before the first worker exists — so
    the pool peaks at half the physical cores and *regresses* past it. The
    N-API mirror has no compiler thread to compete with and peaks at the
    physical core count.

  An explicit `--jobs N` is held to the same `4 × logical` ceiling as the
  native CLI, warned about on stderr when it bites (see
  [§Multi-File Formatting](#multi-file-formatting)'s parallelism note; `cli.js`
  restates the native `clamp_worker_count` by hand — same constant, same
  message, so both surfaces refuse the same numbers). The bound does a
  different job here than natively: a JS worker is a whole V8 isolate (~13 MB
  resident on either engine, where the native thread's reservation is lazily
  committed and costs ~none), so an unbounded width on a large tree hits the
  machine's *memory* — ending in an uncatchable OOM SIGKILL — long before the
  OS refuses a thread, and the file-count clamp bounds nothing on exactly the
  trees large enough to matter. An explicit width remains the only way to
  compare the two paths at a given size, which calibrating those defaults
  needed; every size that calibration uses is far under the ceiling on both.
  `--jobs 1`, `--content`, `--stdin`, and `--list` are single-threaded on both
  CLIs. The parallel and single-threaded paths report identically (same sorted
  stdout, same summary, same exit code), so the split is a cost decision and
  not a contract one. One source:
  it imports its engine from `./index.js`, so the copy staged into the native
  `@fuzdev/tsv` (as the dispatcher's fallback) binds to the N-API engine with
  no adapter — and its workers, having no compiled module to inherit, load
  that engine themselves, while WASM workers take the main thread's module
  through the package's `./worker` entry and recompile nothing.
  Behavioral changes to `format`/`parse` here must be mirrored there and in
  the CLI tests of `scripts/test_npm.ts` (wasm) and
  `scripts/test_napi_npm.ts` (native).
- **`tsv_debug` (development)**: Uses embedded Deno sidecar for external tools
  - Reuses `tsv_cli` infrastructure
  - Commands: ~50 subcommands — the full catalog lives in the root [CLAUDE.md §Debug Tooling](../CLAUDE.md#debug-tooling) and [audits.md](audits.md) (which sections this list deliberately doesn't duplicate): fixtures (`fixture_init`, `fixtures_validate`, `fixtures_update*`, `fixtures_audit`), oracles (`check`, `compare`, `ast_diff`, `canonical_parse`, `format_prettier`, `test262`, `tsc_conformance` — see [typechecker.md](typechecker.md)), the standing audit family (`comment_audit`, `gap_audit`, `census_audit`, …), the compiler harnesses (`compile_*`, `canonical_compile`, `render_compare`), and profiling/metrics (`profile`, `json_profile`, `arena_stats`, `buffer_sizes`, `metrics`, `line_width`, `lex_diff`)

### External Tools (via Embedded Deno Sidecar)

`tsv_debug` calls these external tools via an embedded Deno sidecar (spawned lazily on first use; bulk commands spawn a small pool of sidecar processes — see `crates/tsv_debug/CLAUDE.md`):

1. **prettier** + **prettier-plugin-svelte**
   - Used by: `compare`, `format_prettier`, fixture management
   - Purpose: Format code, compare outputs, validate formatter behavior

2. **svelte**
   - Used by: `canonical_parse`, `ast_diff`, fixture management
   - Purpose: Parse Svelte code with official compiler

3. **acorn** + **@sveltejs/acorn-typescript**
   - Used by: `canonical_parse`, `ast_diff`, fixture management
   - Purpose: Parse TypeScript code (matches Svelte's TS parser)

Versions are pinned (exact) in `crates/tsv_debug/src/deno/sidecar.ts` — the source of truth; they are not repeated here. `benches/js/package.json` pins the same versions independently for the bench harness; keep the two in sync.

## Input Handling

All content-processing commands support three input methods:

- **File path**: `command <file>` - Auto-detects parser/type from extension
- **Content**: `command --content <string> --parser <type>` - Requires explicit `--parser svelte|typescript|css`
- **Stdin**: `command --stdin --parser <type>` - Requires explicit `--parser svelte|typescript|css`

`parse` and `format` also take `--goal script|module` (TypeScript only; default
`module`). It selects the parse goal: at `script`, `await` is an ordinary identifier
and `import`/`export`/`import.meta` are errors. For `format` it applies to
`--content`/`--stdin` only — file paths are always formatted as modules (Svelte and CSS
have no goal), and a path argument with `--goal` is a usage error (exit 2); `parse`
honors `--goal` on file paths too. Both
goals are strict; see [conformance_test262.md §Strict Mode Only, Explicit Goal Axis](./conformance_test262.md#design-decision-strict-mode-only-explicit-goal-axis).

`parse` also takes `--no-locations`: it emits the span-only wire — `start`/`end`
offsets but no per-node `loc` (line/column) object, and for Svelte no `name_loc`
either. `loc` is derivable from the offsets plus source, so nothing is lost for a
consumer that has the source; it mirrors acorn's `locations: false`. No-op for CSS
(`parseCss` emits no `loc`). Orthogonal to `--goal` (goal drives the parser,
`--no-locations` the writer), so the two compose.

Implemented in `tsv_cli/src/cli/input.rs`

## Recursion Depth

The parser and the printer are recursive descents, so nesting depth costs stack — and a
stack overflow is not a catchable panic. No `catch_unwind` and no panic contract can
turn it into a per-file error the way they do every other failure; it kills the process,
and a directory format that dies that way has already rewritten some files, having
printed none of them (changed paths are reported after the run, not as they are
written). What the user sees is exit 134 (SIGABRT on Unix), two lines of runtime message naming
the thread that overflowed (`tsv` for the subcommand, `tsv-format` for a pool worker —
which is why those threads are named at all: it is the only diagnostic the failure
leaves), and no record of what changed.

So the ceiling is stated rather than inherited. `main` runs the whole subcommand on a
thread with `STACK_SIZE` reserved (`cli/stack.rs`), and the format workers reserve the
same, which makes the depth a property of tsv instead of a property of the route, the
host and the platform:

- inherited, the main thread's stack is the process `RLIMIT_STACK` on Unix — commonly
  8 MiB, but whatever the machine says — and **1 MiB on Windows**, where the linker
  writes it into the executable header and nothing at run time can raise it. A spawned
  thread inherits Rust's 2 MiB instead, and `RUST_MIN_STACK` moves that one but not the
  main thread's.
- so without the reservation, one binary has an **8x** depth difference between
  `tsv format <path>` and `tsv format --content` on the same input on the same Windows
  machine — and the asymmetry points the *other* way on a machine whose `RLIMIT_STACK`
  is above the pool's own reservation, where the pool becomes the shallower route.
  `tsv parse` has no pool at all, so it took the inherited stack on every platform.
- which recursion binds **depends on the shape**, and on parens — the shape the flat
  figure below is quoted on — the two sides are level: `parse` reaches within ~0.01% of
  `format` on the same input (37,411 vs 37,412 parens at 32 MiB). ⚠️ That does **not**
  generalize. Wherever a **member chain** is involved the printer binds, and by a wide
  margin: a nested memberish call (`a.f(a.f(…))`) parses to 27,566 levels and formats to
  9,436, and a nested computed subscript (`a[a[…]]`) parses to 34,345 and formats to
  11,574 — the chain printer's own frames set the ceiling at ~⅓ of the parser's. The
  wire-JSON writer adds nothing on top of the parser on any shape measured.

Measured on `const x = ((((…1…))));`, one nesting level costs ~0.88 KiB of stack in a
release build (~16 KiB in a debug build, where frames are much larger), so the shipped
CLI reaches ~37,400 levels on every route and every platform. For scale: the parsers tsv
stands in for stop earlier and on the same input — acorn + `@sveltejs/acorn-typescript`
at 497 levels and prettier at 805, both through V8's own checked stack limit, which is
why theirs is a catchable `RangeError` and tsv's is not. The deepest file in the tsc
corpus nests 69 levels; the exposure is generated and minified code.

Parens are not the tightest shape, only the easiest to state. Per nesting level, in a
release build: **nested arrow bodies (`() => {…}`) ~3.55 KiB** (the worst measured —
~9,200 levels, which is the depth every shape clears), nested memberish calls
(`a.f(a.f(…))`) ~3.47 a shade under it (~9,400 levels), nested computed subscripts
(`a[a[…]]`) ~2.8, TS object literals ~2.4, TS *types* ~2.35, statement nesting ~2.0,
Svelte elements ~1.7, nested binary chains ~1.5, unary chains ~1.25, calls ~1.2, array
literals ~1.14, parens ~0.88, ternary / assignment chains ~0.50, CSS rules ~0.4.

The two chain shapes used to head that list, because a member chain is printed from a
*grouped* view of a linearized chain and those frames sit on the expression cycle: they
cost 7.4 and 6.7 KiB a level until `ChainGroup` stopped owning a `SmallVec` of node
copies and became a borrowed sub-slice (16 bytes) — ~2.2 KiB a level back on both and
~0.5 on nested arrow bodies — and 5.1 and 4.5 until the peeled trailing member tail
stopped being collected into a second `SmallVec` and became a pair of borrowed runs,
another ~1.2 KiB a level on both. A third slice came off five shapes at once — the same
0.39 KiB from each — when the chain linearizer stopped *returning* its 464-byte node
buffer and started filling the caller's: the buffer lives in the caller either way, so a
returned one cost a second slot in the expression dispatcher's frame, which every shape on
the expression cycle pays. Both chain shapes, nested arrow bodies and TS object literals
each dropped 0.39, and so did unary chains (1.64 → 1.25 — nearly a quarter of what a level
there had cost). The two chain shapes are also the ones on which the printer, not the
parser, sets the ceiling — see the bullet above. ⚠️ And they are the only two shapes the
`Expression` enum's own width does *not* reach: every other row above moved when it went
from 176 bytes to 72 (Svelte elements 3.1 → 1.7, calls 1.9 → 1.25, parens 1.2 → 0.94),
while these two stayed put, because the chain printer's frames — not an `Expression`
slot — are what sets them. The `TSType` enum's width reaches a different subset again:
narrowing it 112 → 80 moved TS types 3.2 → 2.35, ternary 0.56 → 0.50, parens 0.94 → 0.88,
calls 1.25 → 1.2 and array literals 1.2 → 1.14, and left Svelte elements, TS object
literals and both chain shapes exactly where they were.

What sets a shape's cost is the stack slots its cycle's functions **reserve**, not the
work they do: a frame is sized once for the widest arm, and every level pays all of it
whichever arm it takes — so a dispatcher that holds one by-value AST node per arm
multiplies that node's size by its arm count, at every level, forever. This is why no
`parse_*` on the expression cycle hands its caller a bare `Expression` by value: a node
builder either boxes into the arena at its own tail (`ParsedExpr::from_expr`, leaving the
caller an 8-byte reference) or returns its own concrete node struct — an
`ObjectExpression` is 32 B, and the dispatcher arm that wraps one back into an
`Expression` builds a temporary the compiler merges with its sibling arms' rather than a
return slot it cannot. The printer answers the same pressure with the same move on its own
side: the chain entry points fill a caller-owned `ChainNodeVec` rather than returning one,
because a returned buffer needs a slot to be built in *and* a slot in the caller to be
handed to, and only the second of those is load-bearing.

The node enums also answer the same pressure from the other side, by *density*, and in
two different ways. The first is **rare-variant boxing** — a variant wide enough to set
the enum's size on its own, and rare enough that an arena allocation apiece is free,
holds its payload by reference. `Expression`'s five widest (`ClassExpression` /
`FunctionExpression` / `ArrowFunctionExpression` / `MetaProperty` /
`TaggedTemplateExpression`) make it 72 B rather than 176; `TSType`'s three widest
(`TSImportType`, `TSConstructorType`, `TSInferType`) make it 80 B rather than 112;
`Statement`'s rare declaration heads (`TSTypeAliasDeclaration`,
`ExportDefaultDeclaration`, `ClassDeclaration`, `FunctionDeclaration`,
`TSInterfaceDeclaration`, `TSDeclareFunction`, `ExportAllDeclaration`,
`TSImportEqualsDeclaration`, `TSEnumDeclaration`, `TSModuleDeclaration`,
`TSExportAssignment`) plus its four loop / `try` heads one level down do the same.
Rarity is what makes those free — each is ≤0.2% of statements, a
classic `for (;;)` is 0.05–0.22%, and the five expression variants together are ~3% of
expressions, of which the two widest are ~0.02% — while the width is paid on every
element of every slice and on every `?`-propagation copy. `Expression`'s ladder stops
where rarity does: the next-widest is `CallExpression` at 64 B and it is 14–21% of
expressions.

The second is a **slot borrow**, which needs no rarity argument at all, because the
inline slot was never where the node lives: the expression parser threads an
`&'arena Expression`, so a by-value `Expression` field is a 72-byte copy *out* of the
arena, and naming it by reference removes work rather than adding an allocation. That is
how `Property` (an object literal's `key: value`, and a destructuring pattern's) and
`VariableDeclarator` went from 160 B to 32, and how every `Expression`-holding statement
head followed (`ExpressionStatement` 88 → 24, `IfStatement` / `SwitchStatement` /
`SwitchCase` 96 → 32, `WhileStatement` / `DoWhileStatement` 88 → 24, `ReturnStatement` /
`ThrowStatement` 80 → 16). With those heads narrowed, `ImportDeclaration` (6.8–11.7% of
statements) and `ExportNamedDeclaration` (2.4–4.0%) were the only variants left setting
the enum's width, so they are arena-boxed too — not for rarity but because they are the
ceiling, and a boxed head copies the same bytes into the arena that it would have moved
into the enum. Together those take `Statement` to **72 B rather than 544**; the
next-widest inline variant is `TryStatement` at 64 B, which is where the ladder stops.

**The other surfaces have their own ceilings, set by their hosts, and the CLI's
reservation does not reach them:**

| surface | stack | depth |
| --- | --- | --- |
| `tsv` (this CLI), every route | `STACK_SIZE`, explicit | ~37,400 |
| N-API addon on the host's main thread | the host process's `RLIMIT_STACK` | ~7,810 at 8 MiB |
| N-API addon on a `worker_threads` worker | Node's 4 MiB `stackSizeMb` default | ~3,880 |
| WASM, any host | the wasm shadow stack, 1 MiB by link default | ~2,510 |

The two binding rows are the host's thread, so the addon cannot size them; a host that
needs the depth raises it itself (`new Worker(…, {resourceLimits: {stackSizeMb}})`), which
is the same shape as the arena-retention advice in
[tsv_napi/CLAUDE.md §Threading & host residency](../crates/tsv_napi/CLAUDE.md). A native
overflow there is a bare `SIGSEGV` with no message, since Rust's guard-page handler is
installed by its runtime startup and a cdylib loaded into Node never runs it.

The WASM overflow is a trap the process survives but the *instance* does not — it
poisons every later call. The npm packages ship a `reinstantiate()` recovery hook, and
the JS CLI (`cli.js`) calls it on any trap in `format_one`, so a too-deep file is one
per-file error (`… (WASM engine trapped and was reinstantiated)`) and the rest of the
run formats normally — on the sequential path and in every pool worker alike. See
[tsv_wasm/CLAUDE.md §Panic Reporting](../crates/tsv_wasm/CLAUDE.md#panic-reporting).

## Multi-File Formatting

`tsv format` accepts any mix of files and directories:

- **Discovery**: directories recurse over the JS/TS family (`.ts`/`.mts`/`.cts`/`.js`/`.mjs`/`.cjs`, all parsed as TypeScript — `.jsx`/`.tsx` are out of scope), `.svelte`, and `.css` (compound forms like `.svelte.ts` included). The **safety nets** `.git`, `node_modules`, `.sl`, `.hg`, `.svn`, `.jj` are always pruned. Explicit args are trusted to the extent the caller named the target: a **file** arg is included regardless of the ignore files, and a **hidden** dir passed explicitly recurses (the heuristic doesn't prune the root it was pointed at). What a file arg does **not** bypass is the extension check — the parser dispatch behind a path has no unknown arm (everything that isn't `.svelte` or `.css` goes to the TypeScript parser), so a named `.json`/`.md`/extensionless file would be parsed as TypeScript: usually a baffling syntax error, and occasionally a *successful* rewrite of a file tsv doesn't support (a top-level-array `.json` reprints as a TS expression statement, semicolon and all, which is no longer valid JSON). Naming one is an **argument** error instead — reported alongside the unresolvable-path errors, failing the run upfront with nothing written, the same line prettier draws with "No parser could be inferred". A **directory** arg is a scope rather than a target, so the check doesn't apply to it: unsupported files inside are filtered out by the walk. A **directory** arg is still subject to the ignore files, including via an ignored ancestor — naming `pkg/dist` doesn't override a `dist/` rule, so an ignore rule is a scope boundary a directory arg can't step past. Symlinks inside directories are not followed; pass them explicitly.
- **Ignore files (two regimes, keyed on `.git`)**: for each directory root, the **format root** — the scope boundary, derived from the argument, never the cwd — is the **repo root** inside a git tree (a hard stop where the upward walk ends, so nothing above the repo is read and `--check` is reproducible) or the **filesystem root** outside one. The regime is decided **once at the target root**, and any ignored directory is pruned (its whole subtree is skipped).
  - **Inside a repo**, discovery honors, relative to the repo root:
    - **`.gitignore`** — hierarchical and repo-rooted exactly like git ([gitignore syntax](https://git-scm.com/docs/gitignore#_pattern_format), matched against `git check-ignore` on case-sensitive filesystems). This goes beyond Prettier, which reads only one `.gitignore` and one `.prettierignore`, both relative to its own directory (the cwd by default), and ignores nested ones entirely.
    - **`.formatignore`** — hierarchical (one per directory from the repo root down, deeper wins), applied after `.gitignore` so its `!` can re-include a gitignore'd path (subject to git's parent-directory rule).
    - **`.prettierignore`** — drop-in compat, honored **hierarchically** as well (one per directory from the repo root down, deeper wins), read as the tsv-layer fallback in any directory with no `.formatignore` of its own; a *sibling* `.formatignore` shadows it per-directory (used alone when present, even if that `.formatignore` is present-but-unreadable — a read error can't silently demote tsv's native file to prettier's). Like the hierarchical `.gitignore` above, this goes beyond Prettier's single cwd-relative `.prettierignore` — so a monorepo that runs `prettier` per-package (each package with its own `.prettierignore`) is honored from one repo-root tsv invocation. Because the shadow silently drops the sibling `.prettierignore`'s rules for that directory (Prettier applies *both* files), tsv emits a non-fatal stderr warning wherever a `.formatignore` shadows a `.prettierignore`, pointing at merging the patterns into `.formatignore`. **Compat caveat:** as a tsv layer a `.prettierignore` `!` can re-include a path `.gitignore` excluded (subject to git's parent-directory rule), whereas Prettier treats `.gitignore` and `.prettierignore` as independent sources OR'd together, where a `.prettierignore` `!` can't rescue a gitignore'd file — tsv's model is the more powerful superset, and the divergence only surfaces for a `.prettierignore` `!` targeting a gitignore'd path (rare).
  - **Outside a repo**, `.gitignore` and `.prettierignore` are not read (as git itself does); only `.formatignore` governs, hierarchically from the filesystem root down — so a `~/.formatignore` is global config for loose files. A `.prettierignore` in the **target root** (the directory tsv was pointed at, where prettier would have read it) raises a non-fatal stderr warning — rename it to `.formatignore`, or `git init` — without changing what gets formatted. The warning is bounded to the target root: outside a repo tsv's regime is `.formatignore`-only at every depth, so this is one courtesy heads-up at the entry point (not a per-directory scan), and an ancestor of a subdirectory target has no repo boundary to anchor on.
  - **Heuristic fallback**: a `.gitignore` in scope is **authoritative** and turns the heuristic off; with no `.gitignore`, the heuristic — hidden directories plus `dist`/`build`/`target` — is the fallback "not source" guess, except that an explicit tsv-layer `!` re-include overrides it.
  - **Re-include idiom**: to selectively re-include under a pruned (or otherwise ignored) directory, re-include the directory itself first — `!dist/` admits the whole directory, then `dist/*` + `!dist/keep.ts` narrows it back to just the files you want. A bare `!dist/keep.ts` (without `!dist/`) is a **no-op** — the heuristic prunes `dist` before descending, mirroring git's parent-directory rule (a gitignored `dist/` likewise blocks a later `!dist/keep.ts`). tsv emits a **stderr warning** for this case (non-fatal — no effect on the exit code, stdout, or `--list`/`--check` output), pointing at the `!dir/` escape.
  - **Subdirectory invocation**: because the boundary is found by walking up, the repo-root rules apply even from a subdirectory, and formatting a subdirectory directly gives the same result as formatting it via an ancestor. But a tree that *contains* repos (a non-repo directory with `.git` subdirectories below it) does not honor the inner repos' `.gitignore`s — run tsv per repo.
  - **Unreadable ignore files**: a `.gitignore`/`.formatignore`/`.prettierignore` that is present but can't be read (invalid UTF-8 — reading is strict UTF-8 on both the native and WASM CLIs — or a permission error) is **not** silently treated as absent: tsv emits a non-fatal stderr warning and drops that file's rules (so an unreadable `.gitignore` also leaves the build-output heuristic *on* for its subtree). A file that genuinely isn't there, or is deleted between the directory listing and the read, stays silent. This is also a `--check` reproducibility hazard — surfacing it is the point.
  - **`--check` reproducibility** assumes the ignore files are **committed**: a local/uncommitted `.formatignore` or `.prettierignore` (or git's unread `.git/info/exclude` / `core.excludesFile`) makes a clean CI checkout disagree.
  - **Shared by construction**: the matcher is the `tsv_ignore` crate's `IgnoreStack`; the per-directory prune/descend policy (heuristic, safety nets, the shadow warning) is the `tsv_discover` crate's verdict. The WASM CLI, the native npm package, and editors call into the same two crates, so every surface agrees rather than hand-mirroring the logic. See `cli/discover.rs`.
- **Fail-fast args, isolated traversal**: path args that don't resolve to a file or directory fail the whole run before anything is written (every bad arg reported); traversal errors below a valid root (e.g. an unreadable subdirectory) report to stderr and discovery continues.
- **No per-file options**: formatting style is fixed (see [CLAUDE.md §Configuration](../CLAUDE.md#configuration)). In particular `<svelte:options preserveWhitespace />` is not detected — whitespace handling is uniform, with only `<pre>`/`<textarea>` content whitespace-sensitive; see [conformance_svelte.md §Template Whitespace](./conformance_svelte.md#template-whitespace-clean_nodes).
- **Deduplication**: with multiple path args, overlapping spellings of the same file (`src` vs `./src`, absolute vs relative, symlink aliases) dedupe by canonical path, keeping the first spelling in sorted order. A single root can't produce duplicates, so the canonicalization cost is skipped.
- **In-place writes**: files are rewritten only when output differs (no mtime churn). `--content`/`--stdin` keep printing to stdout.
- **`--check`**: lists files that would change without writing; exits 1 if any would. For CI. Also works with `--content`/`--stdin` (nothing printed to stdout; the exit code is the API) for editor integrations.
- **`--list`**: prints the discovered in-scope files (one per line) without formatting — a read-only view of the set `format` would touch, after the ignore files are applied. Path mode only (errors with `--content`/`--stdin`) and mutually exclusive with `--check`. Unlike the format action, an empty scope is a valid answer (exit 0, no output) rather than the "no supported files" error; traversal errors still exit 2. Useful for debugging ignore-file scoping and for scripting over the set.
- **Parallelism**: files format concurrently on `std::thread::scope` workers claiming one file at a time from a shared queue — dynamic load balancing with no thread-pool dependency. `--jobs N` overrides the worker count, clamped to the file count and floored at 1 (`--jobs 0` is a width, not an opt-out — it means `--jobs 1`); path mode only, an error with `--content`/`--stdin`. Each worker reserves the same stack every other tsv thread runs on (`STACK_SIZE`, `cli/stack.rs`), so the pool is not a route with a depth ceiling of its own — see [§Recursion Depth](#recursion-depth).

  **An explicit `--jobs` is held to `4 × logical CPUs`**, warned about on stderr when it bites. Four per core is far past what the workload can use — the *default* lands below the logical count for measured reasons — so the ceiling is about blast radius, not throughput: each worker reserves `STACK_SIZE` of address space, and an unbounded count takes task slots until the OS refuses, which on a systemd machine is the login session's whole `TasksMax` and wedges every other process on it.

  **And a `--jobs` the OS still won't give narrows the pool rather than failing the run.** The count is a user-supplied number, so a refused thread is an ordinary outcome of an ordinary argument, and the work is *claimed* rather than partitioned — however many workers exist drain the whole list between them. tsv warns (`warning: only N of M format workers started`) and formats the tree; if not one thread could be started, it says so and formats on the calling thread. Both messages are the JS CLI's, word for word.

  The default is **`min(logical CPUs, ceil(1.5 × physical cores))`**, not one worker per logical CPU. This workload does not scale onto SMT siblings — the per-file work is memory-bound, and on a large tree the discovery walk is the bottleneck, so extra workers compete with it for cores. One worker per logical CPU costs up to 28% on walk-bound trees while buying nothing on flat repos. The SMT width is read once from `/sys/devices/system/cpu/cpu0/topology/thread_siblings_list`; where that is unavailable (no SMT, or a non-Linux platform) the cap is inert and the default is the logical count, so it can only ever lower the worker count.
- **Streaming discovery**: a single directory root — the common invocation — feeds the workers *as the walk finds files*, so the directory walk runs beside the first files' parse+format rather than in front of an idle pool. It is worth having: the walk is 5–10% of the wall on an application repo, and 40–67% on a repo with a large tree, where it can outrun what the pool consumes. Other argument shapes (explicit files, multiple roots) discover the whole set first, because the canonical-path dedup above is set-wide. The set of files formatted is identical either way, as is the reporting order below — only the order work is handed out differs.
- **Error isolation**: a per-file read/parse/write error (or panic, caught via `catch_unwind` — effective only in builds with `panic = "unwind"`; release uses `panic = "abort"`) reports to stderr and processing continues.
- **Deterministic reporting**: changed paths print to stdout in sorted-path order regardless of completion order; errors (traversal and per-file) and the summary line go to stderr.
- **Exit codes**: 0 clean, 1 would-change (`--check` only), 2 errors.
