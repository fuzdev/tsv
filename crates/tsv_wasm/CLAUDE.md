# tsv_wasm

WebAssembly bindings for `tsv`. Three npm packages from one Rust crate via
the `format` + `parse` cargo features (default = both):
`--no-default-features --features format` → `@fuzdev/tsv-format-wasm`
(format only); `--no-default-features --features parse` →
`@fuzdev/tsv-parse-wasm` (parse only — the printers drop out at link time;
bundles `tsv_ast.d.ts` for typed returns); default build →
`@fuzdev/tsv-wasm` (everything, plus the `tsv` CLI from `npm/cli.js`).

See [../../CLAUDE.md §Publishing](../../CLAUDE.md#publishing) for the
package shape, version-of-truth rule, and the `deno task publish` /
`build:npm:*` commands. A separate types-only `@fuzdev/tsv-ast` package
is deferred.

## The npm Facade: Options & Typed Returns

The raw exports are **flat** — `parse_<lang>(source, source_type?)`,
`parse_<lang>_json(source, source_type?)`, `format_<lang>(source, source_type?)`
(`src/lib.rs`'s `lang_bindings!`), the same shape `tsv_napi`'s addon takes. The published
surface is a **hand-written JS facade** over them: `npm/api.js` (the options reader and
the format family) and `npm/api_parse.js` (the parse family), staged verbatim into every
package by `scripts/patch_npm_package.ts` — and into the native `@fuzdev/tsv` by
`scripts/build_napi_packages.ts`, so the two package sets share **one** options reader and
one set of error texts by construction rather than by a parity test. Each package entry
(`index.js`, `browser.js`, the napi loader) is a thin adapter: it hands
`create_format_api` / `create_parse_api` its engine's functions and re-exports the result
by name.

Every export takes `(source, options?)` with an acorn-style bag:
`parse_<lang>(source, {locations?, sourceType?})`, `parse_<lang>_json(source,
{sourceType?})`, `format_<lang>(source, {sourceType?})`. `locations` (default `false`) adds
per-node `loc` to the span-only tree — see [The Span-Only Wire](#the-span-only-wire-and-locations-true);
the `_json` exports return the wire string itself and take no `locations`, since `loc` is a
view over objects. `sourceType` (`'script'` / `'module'`) is TypeScript-only — Svelte
hard-wires `Module`, CSS has no goal — so the other languages reject a set value. Unset, it
means `'module'` **for a parse** (the wire's `Program.sourceType` is a claim one settled
grammar has to produce); the format exports read the same absence differently — see
[Format Options](#format-options). Unknown keys always error, whatever their value (a typo
like `{locatons: true}` — or `{locatons: undefined}` — silently succeeding would hand back
the default while the caller believes they asked for more); a supported key explicitly set
to `undefined` means that key's default — including the TS-only `sourceType` on a language
that rejects it, which is what lets a caller forward one bag to whichever parser
(`npm/cli.js` does). A non-object argument errors, arrays included. The error texts are the
facade's own — `read_options` / `read_source` in `npm/api.js`, the two readers in
`npm/locations.js` — and follow one pattern a caller can match on: `<noun> options must be
an object`, `unknown <noun> option '<name>' (…)`, `<noun> option '<name>' must be …`, with
a refused option value described as `(got …)` by one rule (a string single-quoted with JSON's
escapes and clipped past 40 UTF-16 units with a `…`, so no raw line break, control, bidi control or lone surrogate reaches
a message; a number or boolean as written; `null`; `none` for `undefined`; else its `typeof`);
`locations` follows it too. A refused `source` is described by its kind alone (its `typeof`, or
`null`), never echoed; a string source that is not well-formed UTF-16 is refused as well, naming
the offset of its first lone surrogate (`<noun> source must be well-formed UTF-16 (a lone
surrogate at offset N)`) — both engines read the source as UTF-8, whose conversion would
silently turn it into U+FFFD. The raw decoders below keep
`tsv_arena`'s own texts (`decode_source_type`, the string-axis decoder both raw engines share),
which no facade call reaches, since the facade grades the source type first. **Every
argument refusal is a `TypeError`** — the
bag's, `read_source`'s (a non-string or ill-formed source), and `locations.js`'s (a non-string source, a non-object bag or unknown key, a
missing or unknown `language`, an uninferable root, a non-number offset to `position_at`); **every parse failure is a
`SyntaxError`** — from `parse_*`, `parse_*_json` and `format_*` alike — so a caller can tell
"you called it wrong" from "the source doesn't parse" by class. `locations.js`'s single
lookups add the one `RangeError` (a numeric offset the text doesn't hold —
[the helper](#linecolumn-reconstruction-helper-npmlocationsjs)).

**The parse failure's shape.** The published error's own enumerable keys are exactly
`start` then `loc`: the error's point in the wire's coordinates (`ParseError::wire_point` —
`start` the UTF-16 offset a wire node there would carry, `loc` its `{line, column}` under the
document's line rule, so `loc` is `create_locator(source, {language}).position_at(start)`),
and its message keeps the Rust `ParseError` text, whose `line:col` header prints the same
point ([docs/cli.md §Parse errors](../../docs/cli.md#parse-errors)). The engines throw a plain
`Error` with own ENUMERABLE integer `start`, `line` and `column` properties — `call_engine`
converts nothing looser, since Bun puts its own non-enumerable numeric `line` / `column` on
every `Error` — here a hand-declared `#[wasm_bindgen] extern` `Error` (`PointedError` in
`src/lib.rs`: a constructor, three setters and `Reflect.deleteProperty`, which clears Bun's own
`line` / `column` ahead of each set, since the format-only build carries no `js-sys`), in
`tsv_napi` an `Error` built on the injected `Env` the same way — and `api.js`'s `call_engine`,
wrapped around the engine call alone, rebuilds that as the `SyntaxError`, so both engines' errors for one input are identical
(`scripts/syntax_error_suite.ts` holds both package suites to one table, and the napi suite
compares the two engines directly; `deno task test:bun` runs that table under Bun, the one
runtime where the delete is not a no-op, so it is what fails without it). Anything else an engine throws passes through as itself:
the raw engines' own source-type refusals, a source over the 4 GiB cap and a format's refusal
of a source that parses (plain `Error`s with no point), a caught panic, a WASM trap
(`RuntimeError`) or stack exhaustion (`RangeError`). A format parses the CR-folded text through
the language's `parse_folded` (`FoldedSource::parse_with`), which maps the error back onto the
caller's source, so it reports the point a parse of that source would — and `format_svelte`
refuses a lone CR inside an in-tag `//` comment, which the fold would end early
(`tsv_svelte::parse_folded`): the message names the CR's `line:col`, but the source is valid
Svelte, so the error is positionless (`ParseError::refusal`) and never the `SyntaxError`. Where the facade `JSON.parse`s an engine's wire itself (the native engine), a failure
there is rethrown as `Error('internal error: AST serialized to invalid JSON', {cause})` —
the WASM engine's own text for that case — never as the bare `SyntaxError` that would read
as a parse failure.

**Why the reader is JS.** Both package sets need it, and the native one has no WASM to
host a Rust reader — so a reader in Rust would need a hand restatement in the napi loader
and a parity test between the two. One hand-written JS reader shared by both is strictly
less to drift: it is unit-tested over a fake engine by `scripts/npm_api_test.ts` (a `deno
task test:deno` leg, so it gates in `check` with no package built), and exercised through
the real engines by `scripts/test_npm.ts` / `scripts/test_napi_npm.ts`, whose exact-string
tests pin the texts.

**The raw exports refuse rather than default.** A caller importing the wasm-bindgen
module past the facade gets a typed error, not a silent `Module`: `wasm_source_type` in
`src/lib.rs` decodes the optional string through the decoder `tsv_napi`'s `napi_source_type` calls too (`tsv_arena::decode_source_type`) —
a source type on a goalless language and a value naming neither goal both throw, in
`tsv_arena`'s words (`invalid sourceType 'sloppy' (expected 'script' or 'module')`), not the
facade's. The argument is a `JsValue`, not an `Option<String>`, because
wasm-bindgen's release glue coerces a wrong-typed string argument rather than refusing it
(a number, boolean or object becomes `""`, an array throws from inside the glue); read off
the `JsValue`, a value that is not a string throws by its kind (`invalid sourceType:
expected a string ('script' or 'module'), got a number`) — where N-API refuses it at its own
conversion. `JsValue` is wasm-bindgen's own, so the format-only build still links no
`js-sys`. The raw `parse_<lang>` returns an object (`js_sys::JSON::parse` over
the wire), so the bench's fairness probe and `scripts/validate_artifacts.ts`'s deno-bundle
smoke read the raw module directly.

**The format-only package loads no parse code.** `api.js` imports nothing;
`api_parse.js` imports `api.js`'s reader and `locations.js`. `@fuzdev/tsv-format-wasm`
ships `api.js` alone — neither `api_parse.js` nor `locations.js` is in its `files`, and its
entries import neither (`scripts/test_npm.ts` asserts both). The parse-only package ships
`api.js` too (`api_parse.js` imports its reader) but not the format declarations.

**Every published function is named for its export.** `create_format_api` /
`create_parse_api` define each function under a computed key (`Object.assign(api,
{[name]: …})`), which names it; an assignment to a computed member (`api[name] = …`)
does not, and the entries' `export const {…} = create_*_api(…)` destructuring never
names anything. Both package suites assert `.name` over every exported function, the
node entry's `init` / `init_sync` included (declared there rather than re-exported
under an alias, since the glue's own are `__wbg_init` / `initSync`).

**Typed returns.** The parse failure is declared once, `TsvSyntaxError` in
`npm/syntax_error.d.ts` (its own file, so every package ships it beside whichever family's
declarations it carries), re-exported by name from every entry and from the
napi `index.d.ts`, and named in each published function's `@throws`. The published declarations are the facade's hand-written
`npm/facade_format.d.ts` (format; in the format-capable packages) and
`npm/facade_parse.d.ts` (parse), re-exported **by name** from
each entry's `index.d.ts` / `browser.d.ts` (the napi `index.d.ts` the same way):
`parse_svelte(...): Root`, `parse_typescript(...): Program`, `parse_css(...):
StyleSheetFile`. They are named apart from `api.js` / `api_parse.js` on purpose: they
declare the entries' re-exports, not those modules' own exports (`read_options`,
`create_*_api`), and a `.d.ts` sharing a `.js`'s basename is read as that module's types —
under `moduleResolution: node10`, which ignores `exports`, a deep import of `api.js` would
type-check as the published functions and be `undefined` at runtime. No overloads: `tsv_ast.d.ts` declares `loc?` optional on every node, as
estree and acorn's own declarations do, so a caller who passed `{locations: true}` narrows
or `!`s. `patch_npm_package.ts` fails the build when a published function or option type
has no declaration there. The wasm-bindgen-generated `tsv_wasm.d.ts` types only the raw
module (`source_type: any`, required, from the `JsValue` decoder; an `any` return) — no entry re-exports a function
from it. Their exact option shapes are [The Option Interfaces](#the-option-interfaces).

## Panic Reporting

A `#[wasm_bindgen(start)]` hook forwards panic messages to `console.error`
(**measured at under +0.1% raw and gzipped** on `@fuzdev/tsv-format-wasm`, ~1.3 KB
raw), because under the shipped `panic = "abort"` + `strip` a
panic reaches the host as a bare `RuntimeError: unreachable`. The why — and why
the `console.error` binding is hand-rolled rather than a dep — is in the comment
above the hook in `src/lib.rs`.

Diagnostic only: the call still traps. What makes the **instance survive** it is
[`tsv_arena`](../tsv_arena/CLAUDE.md#abort-safety-take-and-park)'s take/park.

⚠️ **A stack overflow is the one trap the instance does NOT survive** — a different
trap with a different cause, and take/park has no reach over it. The shadow stack is
1 MiB of linear memory (wasm-ld's default `-z stack-size`, placed first so an overflow
walks off address 0 into `memory access out of bounds` rather than quietly over the
data segments), and `__stack_pointer` is a plain mutable global that a trap does not
restore. So the pointer stays where the deep call left it and **every later call on
that instance throws the same error, in every language and on every entry point** —
verified across `format_typescript` / `format_css` / `format_svelte` /
`parse_typescript_json` after one deep `format_typescript`. Only a fresh instance
recovers. The depth is ~2,510 nested parens at ~0.41 KiB of shadow stack per level, the
lowest of any tsv surface — though above acorn's 497 and prettier's 805 — and unlike the
native builds it does not move with the host, since the shadow stack is inside the
module.

**A fresh instance is what the npm packages' `reinstantiate()` export provides.**
wasm-bindgen's `initSync` short-circuits once initialized, so the hook is patched
into the generated glue by `scripts/patch_npm_package.ts` — the only module that can
reach the glue's module-level `wasm` binding. It swaps in a fresh instance
**synchronously** from the retained compiled `WebAssembly.Module` (no recompile),
and because every glue export reads `wasm` at call time, every already-imported
binding follows the swap with no rebind. It rides all three variants and every
entry (the lazy entries re-export it; it self-guards until initialized). `npm/cli.js`
calls it in `format_one` on any `WebAssembly.RuntimeError`, and on the `RangeError` V8
raises when a deep call exhausts its native stack first (which strands the instance too)
— both roles, the
sequential path and each pool worker — so one outsized file costs one honest
per-file error instead of poisoning the rest of the run (a stranded instance reports
every later call as `memory access out of bounds`).
Consumers that loop over files on one instance should do the same. Wasm-backed
objects from before the swap (`IgnoreStack`) are invalidated — rebuild them after.
The patcher stamps every handle with the instance generation it was minted under
(on the object for `free()`, in the `FinalizationRegistry` held value for the GC
callback); both free paths no-op on a stale stamp, so a handle from a discarded
instance leaks its old bytes instead of freeing a stale pointer into the fresh
instance's allocator, and every exported **method** routes its receiver through the
same compare (`__tsv_live`) and throws on a stale one, since its pointer would
otherwise read the fresh memory's unrelated bytes as the object — while handles
minted after the swap free and work normally. (A
module-level "has any reinstantiation happened" check is the wrong shape: a one-way
fuse that leaks every handle after the first recovery, fresh ones included.) Gated by
`scripts/test_npm.ts` (the poison-then-recover API contract + both CLI paths) and
smoked per variant by `scripts/validate_artifacts.ts`.

## JSON-String Transport

The AST crosses the JS↔WASM boundary as **one compact JSON string** — the span-only wire:
`parse_*` builds it with the lang crate's `convert_ast_json_string` (each
language's wire-JSON writer emits it directly from the internal AST — no intermediate
`serde_json::Value` or typed public tree) and calls the engine's native `JSON.parse` from
Rust via `js_sys::JSON::parse`, so the export returns the object. Building the JS object
graph node-by-node with `serde_wasm_bindgen` is measurably slower. `js-sys` rides the
`parse` feature alone; the format-only build does not link it.

`parse_*_json` exports return the JSON string itself — for consumers that
forward the wire format (disk, network, another tool) without paying
`JSON.parse` for an object they don't need.

## Format Options

The format exports take the **same bag** — `format_<lang>(source, options?)`, read by the
same facade reader — so one package never teaches two calling conventions: a caller
holding a `{sourceType}` bag hands it to a parser or a formatter without branching
(`npm/cli.js` does exactly that on both paths). Format's bag carries **one** key, the
TypeScript-only `sourceType` (Svelte `<script>` is always a module; CSS has no goal),
because formatting itself is non-configurable. Everything else is the parse semantics
verbatim: unknown keys error whatever their value, a supported key set to `undefined` means
its default (including the TS-only `sourceType` on a language that rejects it), and a
non-object argument errors, arrays included.

**An unset `sourceType` is the one key whose default differs between the two
families.** The facade forwards it unset, and the raw format exports hand that to
`parse_format!` → `parse_ast_for_format!` → `tsv_ts::parse_folded` →
`tsv_ts::parse_with_goal_or_fallback`:
the module grammar, retried as a script only if that parse *fails*, reporting the
module's error when both do if the script retry died on a top-level `import`/`export`, an `import.meta`, a top-level `for await` or a top-level `await`'s operand, else the further-reaching one (the module's on a tie). That is what lets `format_typescript(source)` with no bag
— an editor's whole call, and `npm/cli.js`'s path mode — format a legacy sloppy
script (`with`, a leading-zero literal or escape, `await` as a name). A **set** value
is exact, so `{sourceType: 'module'}` refuses one; nothing the module grammar
accepts is ever reinterpreted, and the printer reads no goal, so no output moves. The
parse exports unwrap the same unset value to `Module` (see
[../../docs/cli.md §Multi-File Formatting](../../docs/cli.md#multi-file-formatting)).

**`locations` is rejected here, not accepted-and-inert.** It asks for `loc` on a parse
tree, and format emits none — an inert spelling would let a caller believe they had asked
a formatter for something it does not produce. The forwarding argument that makes
`sourceType` lenient doesn't reach it: nothing hands a *parse* bag to a format export
(`npm/cli.js` builds each bag at its own call site), so the key is simply unknown. On
`format_svelte`/`format_css`, where no key is settable, the unknown-key error says so —
`unknown format option 'locations' (this export takes no options)`. `sourceType` is the
exception on those two: it matches its own arm first and reports `format option
'sourceType' is only supported for TypeScript`, which is the more useful message and the
reason the key stays leniently `undefined`-tolerant there. The `parse_*_json` exports refuse
`locations` too, for the same reason — they return the wire itself — but in their own words,
whatever the value (`undefined` included): `parse option 'locations' is not supported by
parse_<lang>_json — the JSON string is the span-only wire; use parse_<lang>(source,
{locations: true})`. There the key is not a typo but a near miss with a one-line remedy, so
the error names it (`read_options`' `parse_json` kind); a format export has no
such neighbour, so `locations` stays an unknown key there.

## The Option Interfaces

The bags, declared in the facade's `.d.ts`: `FormatOptions` / `TypeScriptFormatOptions`
(`facade_format.d.ts`), and `ParseOptions` / `TypeScriptParseOptions` (the object parsers) and
`ParseJsonOptions` / `TypeScriptParseJsonOptions` (the `_json` exports, no `locations`)
in `facade_parse.d.ts`. The non-TypeScript bags — `FormatOptions`, `ParseOptions`,
`ParseJsonOptions` — all declare **`sourceType?: undefined`**. None may be `{}`, and
none may omit the key. Two independent reasons:

- **An empty interface guards nothing.** `{}` opts out of *both*
  excess-property checking and weak-type detection, so it accepts every
  non-nullish value: `format_svelte(src, {locatons: false})`,
  `format_svelte(src, 'script')`, `format_css(src, ['script'])` would all
  compile and all throw. That is the one shape where the types are *looser*
  than the runtime rather than stricter. One declared key restores both checks.
  (`ParseOptions` would not be empty without it — it has `locations` — so this reason
  binds the other two.)
- **Omitting the key breaks forwarding.** `npm/cli.js` builds
  `{sourceType: <maybe undefined>}` and hands it to whichever export rather than
  branching the call, and the runtime reads a `sourceType` set to `undefined` as
  its default even on a language that rejects a *set* value. A bag with a
  `sourceType: undefined` key must therefore type-check on those exports —
  which an omitted key rejects (excess property) and `sourceType?: never` rejects
  under `exactOptionalPropertyTypes`. `undefined` is the spelling that works.

The TypeScript bags are standalone interfaces, **not** `extends` of their base: a settable
`sourceType` is incompatible with the undefined-only one. The assignability that `extends`
would buy (handing a `TypeScriptParseOptions`-typed variable to `parse_svelte`) is unsound
anyway, since it throws the moment `sourceType` is actually set; the only bag that forwards
at runtime is one whose `sourceType` is `undefined`, which is exactly what these shapes
accept. The `_json` pair exists because `locations` is unknown there: typed as
`ParseOptions`, `parse_svelte_json(src, {locations: true})` would compile and throw.

**Every key spells `| undefined` on top of `?`** — `locations?: boolean |
undefined`, `sourceType?: 'script' | 'module' | undefined`. Same reason as above,
applied to the settable keys: without it,
`format_typescript(src, {sourceType: undefined})` — precisely what `npm/cli.js`
builds when no `--source-type` was passed — fails to type-check under
`exactOptionalPropertyTypes` while working at runtime.

The residual looseness is the usual TypeScript one: excess-property checking
fires on fresh object literals, so a **non-literal** bag with an unknown key
still forwards past the compiler and lands on the runtime's unknown-key error.
That is the intended division of labor, not a gap.

`npm/cli.js` routes `tsv format --source-type` and `tsv parse --source-type` through the same
option; see [../../docs/cli.md §Input Handling](../../docs/cli.md).

## The Span-Only Wire and `{locations: true}`

Every parse export emits **one wire, span-only**: the AST with `start`/`end` (UTF-16
code-unit offsets) on every node and no per-node `loc` (Svelte also no `name_loc`) —
the same on every binding (`tsv_napi`, `tsv_ffi`). Line/column is a pure function of an
offset plus the source, so `loc` is a **view**: `{locations: true}` on a `parse_<lang>`
call runs the shipped reconstruction (below) over the tree it just parsed and returns
the tree with `loc` on every node — acorn-exact for TypeScript. The `loc`-bearing JSON
form is Rust-only (`convert_ast_json_bytes_with_locations` in each language crate, behind the
opt-in `locations` cargo feature no binding enables), reached by the native CLI's `tsv parse --locations`, `tsv_debug`, and the
corpus tools. `npm/cli.js`'s `parse --locations` is reconstruct-then-stringify, so its
trees deep-equal the native CLI's while their key order differs (the Rust writer puts
`loc` after `end`, the reconstruction appends it last).

### Line/Column Reconstruction Helper (`npm/locations.js`)

`npm/locations.js` (pure JS, zero deps, no WASM) is the reconstruction, shipped so callers
don't reimplement the line rules — in every package that parses, native `@fuzdev/tsv`
included, and run by the facade for `{locations: true}`: `reconstruct_locations(ast,
source, options?)` (one-shot, adds `loc` to every object carrying `start`/`end`, **mutates in
place**; `options.language` inferred from the root when omitted) and `create_locator(source,
{language})` (amortized — builds the line table once and exposes `position_at(offset)` /
`loc_of(node)` / `reconstruct(ast)` over it). There is no per-call single-node form: one
would rebuild the O(source) table per lookup. `create_locator` takes the language
**required** (types and a runtime throw): a bare source names no document, and the language
decides the line rule, the BOM, and the Svelte stamping, so a default would silently answer
for the wrong document.

**The single lookups check, the walk does not.** A locator's `position_at` and `loc_of` throw a
`RangeError` for a numeric offset outside the indexed text (an integer from 0 to its length — the
BOM-elided text for Svelte and CSS) or a `start` after `end`; `position_at` throws a
`TypeError` for an offset that is not a number at all (an argument error, like every other),
while `loc_of` still answers `null` for a value with no numeric `start`/`end` — the same
objects the walk gives no `loc`. The whole-tree walk (`reconstruct`,
`reconstruct_locations`) checks no node: it is the hot path `{locations: true}` runs, and it
trusts its tree to be a parse of the source — and acyclic, since it keeps no visited set (a
tree given `parent` back-pointers never finishes; documented, not guarded). Argument errors
(a non-string source, a non-object bag or unknown key — the bag read by its own enumerable
keys alone, as `read_options` reads one — the language) are `TypeError`s; the source check restates `api.js`'s `read_source` rather than
importing it, since this module imports nothing.

**It is also its own entry point**, the `./locations` subpath of every package that ships
it (`types: './locations.d.ts'`): a consumer holding a tree — from disk, another process, a
`_json` export — reaches line/column without loading the engine. Both package suites import
it by bare specifier from a staging that holds no engine (the wasm suite from a copy holding
only the helper, the napi suite from the staging without a platform package).

**It implements the same definition the Rust writers do**, so its result deep-equals the
Rust emitter's loc-bearing wire of the same parse in every language: every object with numeric
`start`/`end` (objects without a `type` included — `options`, `StyleSheet.content`,
comments) gets the line (1-based) and UTF-16 column (0-based) of its own offsets, under one
line-terminator rule per document — ECMAScript's for TypeScript, `\n` alone for a whole
Svelte document and for CSS — with a leading BOM counted for TypeScript and elided for
Svelte and CSS, as each canonical parser does (`tsv_lang::LeadingBom` on the Rust side).
On top of that it restores the Svelte-only fields, each an exact function of a node's span
and type: `name_loc` on elements, attributes and directives, and the `character` field on
the positions Svelte's own template reader creates — a shorthand attribute's identifier, a
snippet name, a simple-identifier block pattern, and an in-tag comment, told apart by
the key order the wire gives it (Svelte's template reader writes `{type, start, end,
value}`, acorn's collector `{type, value, start, end}`), so a tree whose comments' keys
were reordered loses that stamp. Svelte's own `loc` quirks are
reproduced by neither implementation; the corpus comparator grades them as named
tolerances (see [docs/conformance_svelte.md](../../docs/conformance_svelte.md)).

It rides every package that parses —
`@fuzdev/tsv-parse-wasm`, `@fuzdev/tsv-wasm`, and the native `@fuzdev/tsv` loader
(`build_napi_packages.ts` stages it there) — and only the format-only package goes
without. `patch_npm_package.ts` copies it + the hand-written `npm/locations.d.ts` into the
package root and re-exports the functions from index.js/browser.js/index.d.ts (directly,
with no init guard — it never touches WASM). Its agreement with the Rust writer is gated at
two cadences: `deno task check:loc` (`scripts/check_loc.ts`) deep-equals the two over every
fixture document in `deno task check`, and `corpus:compare:parse`'s loc arm requires it of
every corpus file before grading tsv's `loc` against the canonical parser's. The package
suites grade the wiring — `{locations: true}` is exactly this reconstruction of the default
parse — and `scripts/test_napi_npm.ts` deep-equals `cli.js`'s `parse --locations` against
the native binary's. The bench's `tsv-json-no-locations+reconstruct` /
`tsv-wasm-json-no-locations+reconstruct` rows run the shipped helper over the perf corpus,
timing what `{locations: true}` costs over the default.

⚠️ **Two implementations of one definition are each other's drift check, not an oracle.**
A definition that is itself wrong makes both halves wrong the same way, and the
cross-grade stays green. The outside reference is the canonical parsers' own `loc`,
graded by `corpus:compare:parse` — exactly against acorn for TypeScript, against Svelte
through its cataloged tolerance rows.

**`.d.ts` export-name constraint.** `index.d.ts` re-exports both `tsv_ast.d.ts`
(`export type *`) and `locations.d.ts` (`export *`), so a name exported by BOTH is
ambiguated away (TS2308) — silently dropping that name from the package. `tsv_ast`
owns `Position` / `SourceLocation` / `NameLocation` / `NamePosition` (+ every AST
node type), so `locations.d.ts` must not export any of those — it **imports**
`Position` / `SourceLocation` (`import type … from './tsv_ast.js'`) for its return
types and re-exports neither. That import resolves only where the two files sit side by
side, i.e. in a package; in the source tree they don't, so in-repo TypeScript importing
`locations.js` takes its types from the module's JSDoc, not from this file
(`benches/js/lib/loc_cross_grade.ts`). Any future hand-written
`.d.ts` added to the parse packages faces the same rule. `deno task typecheck:packages`
(`scripts/typecheck_packages.ts`) is the gate: it installs each staged package into a temp
consumer and compiles consumer modules against the merged `.d.ts`, so a collision fails
from both sides — the entry's own TS2308 (graded, since the consumer runs without
`skipLibCheck`), and a consumer import of `Position` from `./locations` that must fail. It
needs the `benches/js` TypeScript, so it runs from `scripts/publish.ts` (Step 6) and on
demand, not in `check` ([docs/audits.md §Package-Declaration Check](../../docs/audits.md#package-declaration-check-typecheckpackages)).

A relative specifier inside a shipped `.d.ts` must carry the **`.js`** extension
(`'./tsv_ast.js'`, which TypeScript resolves to `./tsv_ast.d.ts`): extensionless
is TS2834/TS2835 under `moduleResolution: node16`/`nodenext`, raised from inside
the package at every consumer without `skipLibCheck`. `typecheck:packages` grades that
under `nodenext`, and both package suites also assert it over every declared `.d.ts` — a
regex, not a typechecker, but it runs wherever the suites do, with no TypeScript install
(`scripts/dts_specifiers.ts`, one test both suites register). The facade's functions and
its option and error types are re-exported **by name** from `facade_format.d.ts` /
`facade_parse.d.ts` / `syntax_error.d.ts`, an explicit form star-export ambiguation can't
drop. The type names are read off those files (`scripts/npm_facade.ts`'s `facade_type_names`), so a type
the facade declares reaches every wasm entry with no list to keep; the napi loader's
hand-written `index.d.ts` is held to the same derived set by `scripts/test_napi_npm.ts`.

A declaration can also need a **lib** the consumer did not ask for. wasm-bindgen declares
each class's `[Symbol.dispose]()`, which only the `esnext.disposable` lib types, so a
consumer on a plain `es2022` lib without `skipLibCheck` would get TS2550 from inside the
package; `patch_npm_package.ts` prepends `/// <reference lib="esnext.disposable" />` to the
generated `tsv_wasm.d.ts` of every variant whose classes declare it. The entries' own
`init` / `init_sync` / `wasm_module` types name the DOM lib's `RequestInfo`, `Response` and
`WebAssembly`, so the wasm packages type-check under `lib: dom` or `skipLibCheck`, as their
READMEs say (the napi loader declares none of those). `typecheck:packages` grades both
claims: the wasm packages under `es2022` + `dom` — one program per package, so another
package's lib reference cannot stand in — and the napi loader and every `./locations`
subpath under `es2022` alone.

## The `./worker` Entry

Every published variant exports two halves of one worker-pool contract, and
neither is obtainable without them: the node entry's **`wasm_module`** (the
compiled `WebAssembly.Module` behind its exports — the `.wasm` file is not an
exports entry, so a consumer cannot reach it) and the **`./worker` subpath**,
which is `browser.js` re-exported under the name a worker reaches for. A worker
`init_sync({module})`s the module it was handed instead of reading and compiling
the WASM again. V8 keeps a process-wide native-module cache, so a worker that
recompiles the same bytes is not paying full codegen either — but it is paying
module resolution, a redundant 2.4 MB read, and per-isolate setup, which the
handoff skips.

Two reasons the subpath aliases `browser.js` rather than adding a second copy of
the wrappers. It is already exactly the entry a worker wants — lazy `init` /
`init_sync` plus not-initialized guards — and `export *` keeps it the **same
module instance**, so `init_sync` there initializes the singleton every other
import of it observes. What it fixes is reachability: Node and Bun resolve the
bare specifier through the `node` condition, which always lands on the auto-init
`index.js`, leaving the lazy entry unreachable off the browser path.

`npm/cli.js` is the first consumer (see [Files](#files)), but the pair is public
API, and reaches its own copy by relative path — so the SUBPATH is exercised only
by the package suites: `scripts/test_npm.ts` drives a real worker through it by
bare specifier, out of a temp `node_modules` (the only place the `exports` map is
walked the way a consumer walks it, conditions included), and
`scripts/validate_artifacts.ts` pins the same-module-instance claim under Deno by
initializing through `./worker` and then calling `browser.js`.

The declarations are **one file per entry**, and that is what makes
`wasm_module` non-optional where it exists. `index.d.ts` (the Node/Bun entry)
declares it `WebAssembly.Module`; `browser.d.ts` — serving both `browser.js`
and the `./worker` subpath that re-exports it — does not declare it at all,
because neither compiles anything until initialized and neither *has* the
export. A single shared declaration would have to spell it
`WebAssembly.Module | undefined`, which reads as merely inconvenient but is
worse than that: it names an export the browser entry does not have, so a
bundler build fails on something the compiler waved through. The `exports` map
therefore nests `types` inside each condition rather than hoisting it.

## Discovery Matcher + Policy (`IgnoreStack`)

The `format` feature exports an `IgnoreStack` class wrapping
`tsv_ignore::IgnoreStack` — tsv's hierarchical, git-faithful matcher — plus the
`tsv_discover` discovery *policy* layered on it (the build-output heuristic +
safety-net pruning). It rides the format-capable packages
(`@fuzdev/tsv-format-wasm`, `@fuzdev/tsv-wasm`) and is absent from the parse-only
package; `tsv_ignore` **and** `tsv_discover` are **optional** deps pulled in by
`format`. This gives the JS CLI (`npm/cli.js`) and the VS Code extension the exact
same matcher *and* prune decision as the native CLI, so all three agree by
construction. The caller builds it up: `new IgnoreStack()`, then
`push_gitignore(anchor, content)` per discovered `.gitignore` and
`push_formatignore(anchor, content)` per discovered `.formatignore` and, inside a repo,
`push_prettierignore(anchor, content)` per `.prettierignore` a directory reads in its
place (all shallowest-first; `pop_gitignore()`/`pop_tsv()` to unwind a DFS — tsv
layers are hierarchical, and the two tsv pushes match identically, differing only in
the file a warning names), then queries:

- `classify_dir(name, child_rel, heuristic_active) -> 'descend' | 'prune' |
  'prune_warn'` — the shared per-directory verdict (`tsv_discover::classify_dir`:
  safety nets, the build-output heuristic, the matcher). On `'prune_warn'` fetch
  the message via `shadow_warning(dir, loose_root?)`. Typed as that union in both
  packages' declarations: here by `unchecked_return_type` on the `#[wasm_bindgen]`
  method (the Rust return stays a `String`), in the napi `index.d.ts` by hand.
- `should_format_file(name, child_rel) -> bool` — the per-file verdict (a
  formattable extension and not ignored).
- `is_path_pruned(rel) -> bool` — the per-file form of the directory-prune verdict
  for a consumer with **no top-down traversal** (the VS Code extension formats one
  open document at a time). It walks `rel`'s ancestor directories itself and
  reconstructs each level's `heuristic_active` from the stack's own pushed
  `.gitignore` anchors, so it takes no extra arguments; pair it with
  `is_ignored(rel, false)` for the file-level match. (`classify_dir` stays the
  primitive for `npm/cli.js`, which threads `heuristic_active` down a real walk.)
- `path_shadow_warning(rel, loose_root?) -> string | undefined` — the same
  per-file replay's warning: `shadow_warning`'s text for the first ancestor
  `is_path_pruned` stops at, when it was pruned — by the heuristic or by a rule — under
  a tsv-layer re-include (the `'prune_warn'` a walk would have reached); `undefined`
  for any other prune, or none.
- `excluded_argument_warning(display, rel, is_dir, loose_root?) -> string | undefined` (`is_dir` as the matcher reads it — never for a symbolic link, whatever it points at, as git reads one)
  — the warning for a path an argument named (a file, or a directory root) that an
  ignore file puts out of scope, naming the file whose rule did it; `undefined` when no
  rule excludes the path, and for a named file a `.formatignore`/`.prettierignore` rule
  excludes (skipped quietly). The scope decision is `is_ignored(rel, is_dir)`, which
  `npm/cli.js` gates every named path on alone (the safety nets and the heuristic grade
  no named path). `loose_root` is the format root's display path outside a repo.
- `shadow_warning(dir, loose_root?) -> string | undefined` (naming the file
  holding the re-include it read from the stack), `prettierignore_outside_repo_warning`,
  `prettierignore_shadowed_warning`, and `gitignore_symlink_warning(path)` — the four
  warning templates, beside `unresolvable_root_error(root)`'s traversal error (methods,
  not free functions, so they ride the class re-export; single source of truth
  with the native CLI, never re-templated in JS).
- `unsupported_extension_error(path) -> string | undefined` — the argument error
  for an explicitly named **file** tsv doesn't format, `undefined` when the
  extension is formattable. Also a method for the class-re-export reason, and its
  receiver is likewise unused (an argument check runs before any matcher exists).
  It carries the rendered extension list, so `npm/cli.js` never hand-mirrors
  `FORMATTABLE_EXTENSIONS`.
- `is_ignored(path, is_dir)` — the raw matcher primitive, still exposed for direct
  consumers.

**Every method has a twin on the N-API `IgnoreStack`** (`crates/tsv_napi/src/lib.rs`,
declared in `crates/tsv_napi/npm/index.d.ts`): `npm/cli.js` ships in both packages and
calls the same methods, and `scripts/test_napi_npm.ts` fails on a method one class has and
the other lacks — a suite `deno task check` does not run, so add the twin in the same edit.

The string-tag return for `classify_dir` (rather than a wasm-bindgen enum or a
returned struct) needs no `patch_npm_package.ts` change and allocates no JS object
on the common descend path. The `is_reincluded` / `negation_under`
primitives are not exported across the WASM boundary — they're folded inside
`classify_dir` (and stay public on the Rust `tsv_ignore::IgnoreStack`), so
JS callers consume the verdict instead of re-deriving the prune decision.

Unlike the parse exports, the class is emitted as `export class` (not
`export function`); `scripts/patch_npm_package.ts` detects `export class` and
carries it through the package facade alongside the functions — verbatim from the
auto-init `index.js`, and as a **guarded subclass** in the two lazy entries
(`browser.js` and the `./worker` subpath it backs), because a constructor cannot
take the per-call init guard the functions do and `new IgnoreStack()` before
`init()` would otherwise report an opaque `TypeError` from the glue where every
other export names the mistake. The subclass adds the guard and nothing else:
the prototype methods, `free()`, `[Symbol.dispose]`, and the generation stamping
the constructor does are all still the base class's, and so is `instanceof` —
in the direction that is asked. What subclassing costs is the OTHER direction:
the name is no longer one constructor across the entries, so an instance from
the auto-init `index.js` is not `instanceof` the lazy entries' `IgnoreStack`.
Two ways to meet that, and only one is guarded. A class RETURNED from Rust would
be built on the base prototype by the glue's own `__wrap` and so fail the check
against the very entry that handed it out — silently, in browsers only — which
`scripts/patch_npm_package.ts` fails the build on (a second
`FinalizationRegistry.register` site is the tell); no class is returned today.
The other is a consumer importing `.` and `./worker` into ONE thread and
comparing across them, which the subpath exists precisely not to be — a worker
imports it, and a bundler resolves both names to the same module. Both package
suites smoke the class (`scripts/validate_artifacts.ts` per variant under Deno,
`scripts/test_npm.ts` under Node — present in format/all, absent in parse-only).
The wasm-bindgen-generated `tsv_wasm.d.ts` declares the class, so no
`tsv_ast.d.ts` entry is needed (the subclass is assignable to it).

## TS Type Maintenance

`types/tsv_ast.d.ts` is **hand-maintained**. Any change to the wire JSON a
writer emits — a field, its key name, when it's omitted, or a discriminator
`type` string — in `crates/tsv_*/src/ast/convert/write*` must also update the
`.d.ts`. Reviewers (human or agent) flag drift at PR time.

Maintenance checklist when a writer's emitted shape changes:

1. Update the `write_*` function (the emitted field / key / skip condition).
2. Locate the matching `interface` / `type` in `types/tsv_ast.d.ts`.
3. Apply the same change. The JSON key is exactly what the writer emits
   (e.g. `w.raw(",\"typeParameters\":")` → `typeParameters`).
4. A field the writer emits only conditionally (`if let Some(..)` / `if flag`)
   is optional in TS (`T?`); one it never emits is absent from the interface.
5. If the field carries positions (`start`/`end`/`loc`/`character`), make sure
   the writer (`ast/convert/write*`) emits them through the document's
   `WirePositions` (each writer's `Ctx::pos` and node-header helpers and
   `JsonWriter::span_start_end*` for offsets, `span_loc` / `loc_field` for `loc`) — a raw byte offset means
   silently untranslated positions on multibyte sources.
6. Run `cargo test --workspace` and `deno task check:ast-types`.

`deno task check:ast-types` (also part of `deno task check`) runs three arms
against `tsv_ast.d.ts`, described in full at
[docs/audits.md §Wire-Type Drift Check](../../docs/audits.md#wire-type-drift-check-checkast-types):
**(A)** `tsv parse --locations` on a curated set of source snippets, each JSON output
embedded as a typed literal and `deno check`ed — TypeScript's
excess-property checking catches both directions of drift, missing/added
fields and discriminator-string mismatches; **(B)** every `type`
discriminant the fixture corpus produces must be declared here or be
deliberately opaque (the CSS node vocabulary plus the `customElement`
config-data `type:` key); **(C)** a computed minimal
cover of the corpus's committed `expected*.json` — every field SLOT
(`ParentType.key -> ChildType`), in as few files as possible — typed the
same way. Arm C grades this file against the CANONICAL parser's output
(`loc`/`name_loc` stripped) rather than against tsv's own, since `expected.json` is what
`fixtures_update_parsed` regenerates from Svelte / acorn-typescript /
`parseCss`.

Extend the `samples` array when a shape wants stating explicitly; arms B
and C need no maintenance — they follow the fixture tree. A `.d.ts` field
typed `unknown` is invisible to arm C, so widening one (as `Script.content`
was, from `unknown` to `Program`) is what puts a region behind the gate.

⚠️ **A node SVELTE builds is not the acorn node of the same `type`, and the tell — in
Svelte's own wire — is `loc`.** Where Svelte constructs a node instead of handing the
text to acorn, the result carries the acorn discriminator but a different field set —
so it needs its own interface rather than reuse, and a position holding either takes a
union. The tell is Svelte's alone: tsv's wire gives every positioned object a `loc`,
these included (each interface types it `loc?`), so on tsv's wire the difference
is the field set, not the `loc`. The three in the wire today, each with its own type, and
the acorn node the last is easily confused with:

| position | shape | why |
| --- | --- | --- |
| shorthand `bind:x` / `class:x` `expression` | `SvelteShorthandIdentifier` | the directive name IS the expression; nothing parsed it, so Svelte writes **no `loc`** |
| `<svelte:element this="div">` `tag` | `SvelteFusedLiteral` | no `loc` in Svelte's wire, and `raw` is Svelte's re-quoted form, not the author's bytes |
| `{@const}` `declaration` | `SvelteConstDeclaration` / `…Declarator` | Svelte builds the wrapper: no `loc` on either in its wire, though the `id` inside has one (from Svelte's own reader, so its `loc` carries `character`) |
| `{const}` / `{let}` `declaration` | the ordinary acorn `VariableDeclaration` | parsed, so `loc` throughout — and unlike `{@const}` it may carry MORE THAN ONE declarator |

The inverse mistake is just as easy: typing one of these as its acorn namesake
compiles until a fixture reaches it. The wire-type gate's fixture-cover arm is what
holds the distinction, and it found every one of them.

⚠️ **Comment attachment is a Svelte-only wire fact with a modelling rule.**
`leadingComments` / `trailingComments` (`AttachedComment[]`) are appended by
Svelte's acorn pass, never by `parse_typescript` / `parse_css`, and the
attachment DFS reaches essentially every acorn node. They are therefore
declared once as `AcornCommentAttachment` and applied two ways, which
between them cover every acorn node: **intersected** into the node unions
(`Expression`, `Statement`, `TSType`, `TSTypeElement`), and **extended**
by the acorn interfaces that positions reference directly by name
(`Identifier`, `Property`, `BlockStatement`, `SwitchCase`, …). A new acorn
node reachable only by name needs the `extends`; the gate is what says so.

The acorn-typescript vs vanilla-acorn deltas the writer emits (`vanilla_acorn`)
require dual updates.

## Files

- `src/lib.rs` — WASM bindings (`lang_bindings!` macro, over the `parse_ast!` / `goal_allowed!` goal axis shared with the two native bindings via [`tsv_arena`](../tsv_arena/), every export flat `(source, source_type?)`, + `wasm_source_type`, the raw module's source-type decoder) + the wasm32-gated talc `#[global_allocator]` and panic hook
- `types/tsv_ast.d.ts` — Hand-maintained TS types, bundled into the parse-capable packages
- `npm/cli.js` — The `tsv` bin shipped in `@fuzdev/tsv-wasm` — mirrors `tsv_cli`'s contract (flags, exit codes, traversal); argv parsed by a transcription of argh's grammar, zero deps. Path mode fans onto `node:worker_threads` behind `--jobs`, spawning **itself** as the worker (`isMainThread` splits the two roles) and claiming work off one `Atomics` cursor. `WORKER_FILE_THRESHOLD` gates the **default** only — a JS pool costs tens of milliseconds to bring up against the native pool's ~50 µs thread spawn — and only on the WASM engine does a width of 1 stay on the main thread: over the N-API engine it is a pool of one worker (`resolve_route`), since a native stack overflow on the main thread is a `SIGSEGV` no catch survives and only a pool worker carries the reserved stack — while an explicit `--jobs N` bypasses the threshold at any file count and is held to the native CLI's `4 × logical` ceiling (`clamp_worker_count`, restated by hand, over the same logical count — the affinity mask capped by the cgroup CPU quota, which `cgroup_cpu_quota` transcribes from Rust std because Node's and Bun's `availableParallelism()` leave it out), giving the threshold something to be calibrated against. Both the threshold and `default_jobs` are **per engine**, keyed off the same `wasm_module` export that decides how a worker binds: the crossover and the knee are properties of the engine, not the driver (see [../../docs/cli.md](../../docs/cli.md) §Binary Structure). On WASM the pool peaks at *half* the physical cores because V8's wasm tier-up is itself multithreaded and has claimed the rest before the first worker exists; over the N-API addon there is no compiler thread to compete with and it peaks at the core count. A WASM trap is contained to its file on both roles: `format_one` calls `reinstantiate` on any `WebAssembly.RuntimeError`, and on the `RangeError` V8 raises when a deep call exhausts the engine's native stack before its shadow stack (feature-detected — the native engine exports no hook and its overflow is process-fatal), so a too-deep file costs one per-file error instead of poisoning the rest of the run (see [§Panic Reporting](#panic-reporting)). Every pool worker reserves the native CLI's `STACK_SIZE` (`WORKER_STACK_SIZE_MB`, gated against `cli/stack.rs` by `scripts/test_npm.ts`), and the sequential route re-runs a file whose main-thread format hit that `RangeError` in a one-worker pool (`retry_overflowed_files`), so a deep file's verdict does not depend on which route the file count picked — on Node, whose workers honor the reservation (Bun and Deno ignore it; see [../../docs/cli.md §Recursion Depth](../../docs/cli.md#recursion-depth)). Which engine a worker binds is decided by whether the main thread's `./index.js` exported a `wasm_module`: here it did, so the worker takes it through the [`./worker` entry](#the-worker-entry) and recompiles nothing; in the native package it didn't, so the worker loads the addon. That is why the engine import is **dynamic** — a static one is hoisted above the branch, and the worker would have paid for `./index.js` before it could ask. Also copied into the native `@fuzdev/tsv` by `scripts/build_napi_packages.ts` — one source for both packages (it imports its engine from `./index.js`, so each copy binds to its own package's engine), which the ESM loader bought like `locations.js` below. In the native package it is the *fallback*: the bin there is a napi-only dispatcher (`tsv_napi/npm/bin.js`) that execs the platform package's real `tsv_cli` binary, deferring to cli.js only when no binary is reachable — the dispatcher deliberately does NOT live in this shared source, so the wasm copy stays byte-identical and can never resolve a sibling-installed native binary. Every path it names itself — the `--list` and changed-path lines, `error:` lines, its traversal and argument errors, `parse`'s read failure — goes through its hand restatement of `tsv_discover::quote_path` (a name holding a control character or a double quote is C-quoted as `git ls-files` prints it; the binding's warnings arrive quoted), pinned beside the native rule by `scripts/test_npm.ts` (see [../../docs/cli.md §Multi-File Formatting](../../docs/cli.md#multi-file-formatting))
- `npm/api.js` + `npm/facade_format.d.ts`, `npm/api_parse.js` + `npm/facade_parse.d.ts`, `npm/syntax_error.d.ts` (the `TsvSyntaxError` type) — The hand-written facade every package publishes through: the options reader + the format family, and the parse family with the `{locations: true}` sugar (see [The npm Facade](#the-npm-facade-options--typed-returns)); the declarations are named apart from the modules they sit beside, since they type the entries' re-exports, not those modules. Staged verbatim by `patch_npm_package.ts` (`api_parse.js` + `facade_parse.d.ts` into the parse-capable packages only, `facade_format.d.ts` into the format-capable ones) and by `scripts/build_napi_packages.ts` into the native `@fuzdev/tsv` — one source for both package sets. `scripts/npm_facade.ts` owns what both staging scripts share: the facade file table (with `locations.*` and `types/tsv_ast.d.ts` in the parse half), the `./locations` exports entry, and the re-exported type names
- `npm/locations.js` + `npm/locations.d.ts` — Pure-JS line/column reconstruction for the span-only wire; ships in the parse-capable packages, run by `api_parse.js` for `{locations: true}`, re-exported from index.js/browser.js by `patch_npm_package.ts`, and exported alone as the `./locations` subpath. Also copied into the native `@fuzdev/tsv` by `scripts/build_napi_packages.ts` — this file is the single source for both, which is what the napi loader being ESM bought (see [Line/Column Reconstruction Helper](#linecolumn-reconstruction-helper-npmlocationsjs))
- `README_format.md` — Shipped as `README.md` in `@fuzdev/tsv-format-wasm` (copied by `patch_npm_package.ts`)
- `README_parse.md` — Shipped as `README.md` in `@fuzdev/tsv-parse-wasm` (copied by `patch_npm_package.ts`)
- `README_all.md` — Shipped as `README.md` in `@fuzdev/tsv-wasm` (copied by `patch_npm_package.ts`). Each README's TypeScript and JavaScript blocks (```ts / ```typescript / ```js / ```javascript, the JavaScript under `checkJs`) compile against its package in `deno task typecheck:packages`, so every one must be a standalone module with its own imports; a block that only compiles under Node carries `<!-- typecheck: node -->` on the line above its fence
- `pkg/` — Build output (gitignored), `pkg/<variant>/<target>/`

## Build Targets

Variant-first output dirs (`pkg/<variant>/<target>/`) so builds never clobber
each other. Subsets build `--no-default-features --features format|parse`;
the `all` builds use the default features (both).

| Target | format output dir  | parse output dir  | all output dir  | Command (format / parse / all)                                      |
| ------ | ------------------ | ----------------- | --------------- | ------------------------------------------------------------------- |
| deno   | `pkg/format/deno/` | `pkg/parse/deno/` | `pkg/all/deno/` | `build:wasm:deno` / `build:wasm:parse:deno` / `build:wasm:all:deno` |
| npm    | `pkg/format/npm/`  | `pkg/parse/npm/`  | `pkg/all/npm/`  | `build:npm:format` / `build:npm:parse` / `build:npm:all`            |
| nodejs | —                  | —                 | `pkg/all/nodejs/` | — / — / `build:wasm:all:nodejs` (bench-only)                      |

The `pkg/all/deno` build feeds the benches and sidecar (it has every
export); the deno builds — subsets and `all` — are size-tracked by `binary_sizes.ts`. The
`npm` builds are the published artifacts: a wasm-pack `web`-target build
patched by `scripts/patch_npm_package.ts` into the multi-entry package shape
(Node auto-init entry, guarded browser entry, conditional `exports`,
metadata, README, the `reinstantiate` glue hook — plus `cli.js` and the `tsv`
bin for the `all` variant).
`deno task test:npm[:parse|:all]` builds the package and then runs Node tests
against it (the `all` variant adds CLI subprocess tests), then the parse-failure table
under Bun (`deno task test:bun <variant>`, warn-skipped without bun). The `:run` suffix —
e.g. `test:npm:run` — skips the rebuild, as in the publish/CI pipelines, and is
freshness-guarded: `scripts/check_staged_freshness.ts` aborts it when a staged
artifact is older than its sources. `deno task validate:artifacts`
checks tight wasm size bounds plus a Deno runtime smoke of every built
bundle — the auto-init entries, and the two lazy ones with their
not-initialized guards — under the same freshness guard, since `pkg/` is
gitignored and a stale bundle sizes and smokes exactly as cleanly as a fresh
one (both run in the publish pipeline, followed by `deno task typecheck:packages`,
which compiles the staged packages' declarations as a consumer does). The npm package itself covers
Node/browser/bundler consumers, so there is no standalone `web`-target
build beyond the npm artifacts; the `nodejs`-target `pkg/all/nodejs/` build
exists solely to feed the Node bench runner (`build:bench:node`).

The generated `tsv_wasm_bg.wasm.d.ts` is intentionally excluded from the
npm `files` list: it types direct `.wasm` ES-module imports, which the
package shape never uses (bytes via `readFileSync`, URL via `init()`), and
nothing in `tsv_wasm.d.ts` references it — matching blake3's packages.
