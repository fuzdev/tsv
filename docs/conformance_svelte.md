# Svelte Conformance

The tsv parser aims for **exact AST compatibility** with Svelte's parser. This document catalogs tsv's compatibility behaviors and intentional corrections.

## Mental Model

**Matched**: tsv produces identical AST to Svelte (the goal). This includes replicating Svelte's quirky behaviors for tool compatibility.

**Unmatched**: tsv produces different AST. The suffix `_svelte_divergence` marks these fixtures. tsv only differs when Svelte violates spec or lacks features.

## Classification

| Category            | Description                             | tsv action                        |
| ------------------- | --------------------------------------- | --------------------------------- |
| **Compat behavior** | Svelte has quirky but harmless behavior | tsv replicates it in AST output   |
| **Correction**      | Svelte violates spec or lacks features  | tsv produces correct/complete AST |

**Critical distinction**: Compat behaviors apply ONLY to **AST/JSON output** for tool compatibility. The tsv **formatter** always produces clean, standards-compliant code.

## Decision Framework

**When to match Svelte (replicate compat behaviors):**

- Design choices (harmless metadata, span differences)
- Tokenization quirks (source extraction oddities)
- Output that doesn't affect semantics

**When to correct Svelte:**

- Spec violations (incorrect AST structure)
- Semantic corruption (unparseable values)
- Missing features (spec-defined syntax Svelte doesn't support)

---

## Corrections Catalog

Cases where tsv intentionally produces different AST than Svelte. Fixtures use `_svelte_divergence` suffix.

**Corpus-scale enforcement**: `deno task corpus:compare:parse` deep-diffs tsv's
parse output against the canonical parsers on real codebases and classifies
diffs against this catalog (the `DOCUMENTED_MATCHERS` list in
`benches/deno/corpus_compare_parse.ts` covers the divergences that parse on
both sides). Keep the two in sync: a new documented AST divergence gets a
matcher, and an unmatched corpus diff group is either a bug or a missing
catalog entry.

### CSS Corrections

| Feature                           | Issue                                | Fixture                                                                                             |
| --------------------------------- | ------------------------------------ | --------------------------------------------------------------------------------------------------- |
| :nth-child(An+B of S)             | Incorrect AST structure              | [nth_child_of](../tests/fixtures/css/selectors/pseudo_class/nth_child_of_svelte_divergence/)        |
| Attribute namespaces `[ns\|attr]` | Not supported                        | [namespace](../tests/fixtures/css/selectors/attribute/namespace_svelte_divergence/)                 |
| No-namespace `\|element`          | Not supported                        | [no_namespace](../tests/fixtures/css/selectors/namespace/no_namespace_svelte_divergence/)           |
| Forgiving :is()/:where()          | Strict parsing (should be forgiving) | [forgiving_is_where](../tests/fixtures/css/selectors/forgiving_is_where_svelte_divergence/)         |
| Empty-after-comment declarations  | Rejected (`css_empty_declaration`)   | [comment_empty_value](../tests/fixtures/css/tokens/comments/comment_empty_value_svelte_divergence/) |
| Block-valued custom properties    | Rejected (`css_expected_identifier`) | [block_value](../tests/fixtures/css/values/variables/block_value_svelte_prettier_divergence/)       |

### CSS Parser Corrections (corpus-enforced)

Corrections where the divergent input is not prettier-stable, so no fixture can
exist (the Core Invariant requires prettier-formatted inputs) — the corpus AST
differential (`deno task corpus:compare:parse`) is the regression oracle, via
the `DOCUMENTED_MATCHERS` named below.

- **BOM offset shift** (matcher `bom_offset`; corpus oracle
  `prettier/tests/format/css/bom/bom.css`). Svelte's `parseCss` and `parse` call
  `remove_bom` before parsing, so in a BOM-prefixed file every canonical offset
  is 1 UTF-16 unit lower than the true file position. tsv deliberately keeps
  file-true offsets: its lexers skip the BOM but never shift positions, so
  consumers can index the string they actually passed in (acorn behaves the
  same way on the TS side, so tsv is also uniform across languages where Svelte
  is not).
- **Declaration tokenization garbage** (matcher `css_declaration_tokenization`;
  corpus oracles `prettier/tests/format/css/empty/empty.css`,
  `prettier/tests/format/css/comments/declaration.css`). Svelte's
  `read_declaration` produces corrupt declarations in two adjacency cases tsv
  parses per spec: a stray semicolon (`border-box;;`) becomes a declaration
  with `property: ";"` that swallows the next declaration into its value
  (tsv skips the empty declaration, CSS Syntax 3 §5.4.4), and a comment
  touching the property name (`color/* c */:`) yields `property: "color/*"`
  with the comment tail leaking into the value, because `read_until` scans to
  the first whitespace — which sits _inside_ the comment (tsv tokenizes the
  comment; the comment-between-property-and-colon _quirk_ with whitespace,
  `color /* c */ :`, is still replicated — see
  `split_declaration_svelte_compat`).

### CSS Parser Scope & Error Model

**Goal: CSS-spec compliance. Near-term: match Svelte's `parseCss`.** tsv targets
standard CSS (CSS Syntax 3, Selectors 4, values/at-rules). The north star is full
CSS-spec conformance — grammar-correct _and_ implementing the spec's
**error-recovery** model (drop an invalid declaration/rule, keep parsing). The
immediate, enforced goal is **parity with Svelte's `parseCss`** on the conformant
subset: tsv is a drop-in replacement and Svelte's parser is the fixture baseline.
Where the two goals conflict on conformant input, Svelte-parity wins for now.

- **Current behavior is hard-fail; recovery is the target, not the design.**
  Today tsv (like Svelte's `parseCss`) **errors on the first invalid construct**,
  which aborts the whole stylesheet — so one bad rule currently discards the
  file's valid rules too. That is a way-station: a spec-compliant parser drops
  only the offending rule and continues (CSS Syntax 3 §9). Hard-fail is inherited
  from Svelte's throw model; error recovery is tracked as future work.
- **A corpus "CSS failure" is usually a deliberate rejection, not a gap.** In the
  benchmark corpus tsv parses a lower share of `.css` than prettier/biome/oxfmt,
  but that gap is **scope, not deficiency**: those tools run the lenient PostCSS /
  `postcss-scss` / `postcss-less` stack; tsv does not. The rejected files are
  overwhelmingly **non-standard CSS** — SCSS/Sass (`$vars`, `@mixin`, `@extend`),
  LESS, CSS Modules (`:global`, `composes`), PostCSS plugin syntax, YAML
  front-matter, and IE hacks. "Skipped CSS" is **not** a synonym for "SCSS" — most
  are other non-CSS dialects.
- **The "Svelte over-accepts" cases are not a tsv correctness win.** Svelte
  accepts some grammar-invalid CSS that tsv rejects — an invalid attribute
  case-flag (`[type=a x]`; Selectors 4 allows only `i`/`s`), a function token as
  an attribute value (`[id=func("foo")]`), a `url` keyword split across whitespace
  in `@import`. tsv is **grammar-stricter**, but _not_ more spec-correct: the spec
  neither keeps these (Svelte's leniency is wrong) nor aborts the file (tsv's
  hard-fail is wrong) — it drops the bad rule and keeps the rest. All three differ
  from the spec; recovery is the resolution that subsumes both, and until then
  these stay documented near-term divergences from Svelte.

**Explicit non-goals.** Preprocessor and vendor dialects — SCSS/Sass, LESS, CSS
Modules, PostCSS plugin syntax, YAML front-matter, and IE hacks (`*zoom`,
`_width`, `+color`, `color: red\9`) — are **permanent** non-goals. tsv targets the
CSS spec, not these dialects, and will not add handling to parse or preserve them.
This is distinct from error recovery: recovery is about not letting one invalid
construct abort an otherwise-valid _standard-CSS_ file; these dialects are input
tsv never chases regardless.

The authoritative gap-vs-intentional-rejection survey is maintained with the
project's planning notes; non-standard `.css` is auto-classified into `expected
errors` by the corpus comparator (`benches/deno/lib/divergence/expected_errors.ts`).

### Svelte Template Corrections (corpus-enforced)

Like the CSS section above: not prettier-stable (or not expressible) as fixture
inputs, so the corpus AST differential is the regression oracle.

- **each-`as` stale `loc.end`** (matcher `each_as_stale_loc`; corpus oracles
  `svelte.dev` DocsContents.svelte, ConsoleLine.svelte). Under `lang="ts"`,
  Svelte parses `{#each contents ?? [] as section}` by letting the TS parser
  read `contents ?? [] as section` as an as-expression, then unwraps it —
  patching the expression's `end` _offset_ back to `contents ?? []` but leaving
  `loc.end` at the as-expression's end (the column after `section`). tsv's
  `loc` agrees with the corrected offset. The matcher is scoped to EachBlock
  `expression.loc.end` entries; offsets and `loc.start` are never absorbed, so
  a real loc bug still surfaces as undocumented.

### TypeScript Corrections

Svelte uses acorn + acorn-typescript, which lags behind TypeScript's parser. tsv implements the full spec.

Svelte ❌ / Prettier ✅ / tsv ✅ in every case below:

- `using` declarations (ES2024) — [basic](../tests/fixtures/typescript/typescript_specific/using/basic_svelte_divergence/)
- `await using` declarations — [await](../tests/fixtures/typescript/typescript_specific/using/await_svelte_divergence/)
- `const` type params in classes — [const_type_param_class](../tests/fixtures/typescript/typescript_specific/generics/const_type_param_class_svelte_divergence/)
- Import type options — [dynamic_attributes](../tests/fixtures/typescript/modules/imports/dynamic_attributes_svelte_divergence/)
- ES2024 v-flag regex — [unicode_sets_advanced](../tests/fixtures/typescript/expressions/literals/regex/unicode_sets_advanced_svelte_divergence/)
- Async generic arrow params — see fixtures below

**`using` keyword-name comments**: Both acorn and tsv reject comments between `using` and the binding name (`using /* c */ x = fn()`), and between `await` and `using` (`await /* c */ using x = fn()`). Per the ECMAScript spec, comments behave like white space and are discarded between any two tokens (§12.4), so these should be valid. However, since `using` is a contextual keyword requiring lookahead disambiguation, both parsers check the next token before comment processing. tsv matches acorn's behavior here. If acorn adds support, tsv should follow.

**Async generic arrow params**: acorn-typescript drops all function parameters from `async` arrow functions that have type parameters (`async <T,>(x: T) => x` → `params: []`). Non-async generic arrows are unaffected. This is semantic corruption — tools consuming the AST would see zero-argument functions. **Upstream candidate**: acorn-typescript async arrow parsing.

Fixtures: [async_generic/basic](../tests/fixtures/typescript/expressions/arrow/async_generic/basic_svelte_prettier_divergence/), [async_generic/basic_ts](../tests/fixtures/typescript/expressions/arrow/async_generic/basic_ts_svelte_divergence/), [async_generic/long](../tests/fixtures/typescript/expressions/arrow/async_generic/long_svelte_divergence/), [curried_typed_callback](../tests/fixtures/typescript/expressions/arrow/curried_typed_callback_svelte_prettier_divergence/), [indexed_access/basic](../tests/fixtures/typescript/types/indexed_access/basic_svelte_divergence/)

The `async_generic/basic` and `curried_typed_callback` fixtures carry a second,
independent divergence — prettier's forced `<T,>` trailing comma on single-unconstrained
arrow type params (hence the `_svelte_prettier_divergence` suffix). See
[conformance_prettier.md](./conformance_prettier.md) §TypeScript.

**Member access on a parenthesized decorator expression** (`@(f()).g a;`):
acorn-typescript only accepts a call after a parenthesized decorator
expression — member access is a parse error. tsc parses it (decorators accept
a full LeftHandSideExpression, beyond the TC39 grammar's strict
`@ DecoratorParenthesizedExpression` production); babel rejects it like
acorn. tsv follows tsc. No fixture: the form is not format-stable — both tsv
and prettier-typescript normalize `@(f()).g` to `@(f().g)`, which every
parser accepts (see the
[paren_member](../tests/fixtures/typescript/typescript_specific/decorators/paren_member/)
normalization fixture) — so the parse gap only surfaces on unformatted
source, where the corpus parse comparison skips it as a canonical parse
failure.

### TypeScript Parser Corrections (corpus-enforced)

Intentional AST divergences from acorn-typescript that have no prettier-stable
fixture form (prettier rewrites the triggering syntax), so the corpus parse
differential enforces them via `DOCUMENTED_MATCHERS` in
`benches/deno/corpus_compare_parse.ts` instead.

**Rest param type-annotation end** (`rest_param_type_end`): acorn-typescript
ends a typed `RestElement` at the binding (`(...args: Array<any>)` → `end`
after `args`), excluding the type annotation — inconsistent with its own
`Identifier` params, and with babel and typescript-eslint, which include the
annotation. tsv ends the param after the annotation. **Upstream candidate**:
acorn-typescript rest-param end position.

**static member ladder** (`static_member_ladder`): for `static` ⏎ `static` ⏎
`static` ⏎ `a() {}` in a class body, tsc parses modifier + member pairs (a
static field named `static`, then a static method `a`); acorn ASI-splits every
bare `static` into its own value-less field and leaves `a()` plain. tsv
follows tsc. **Upstream candidate**: acorn class-field ASI for bare `static`.

**extends instantiation line-break shape**
(`extends_instantiation_linebreak`): with type arguments on the heritage and a
line break before the next clause (`extends Base<T>` ⏎ `implements I` — how
prettier formats long class headers), acorn-typescript leaves the superClass
as a `TSInstantiationExpression`; on one line it emits
`superClass: Identifier` + `superTypeParameters`. The shape depends only on a
line break (its instantiation bail checks `hasPrecedingLineBreak`). tsv emits
the same-line shape uniformly.

**Lone surrogates in string values** (`lone_surrogate_value`): a lone UTF-16
surrogate (`"\ud800"`) decodes to U+FFFD in tsv — Rust strings are UTF-8 and
cannot represent WTF-16 lone surrogates — where acorn keeps the lone
surrogate in the JS string value. `raw` is a source slice and unaffected.
This is a representation limit, not a parse difference.

**Parenthesized decorator subscript start**
(`decorator_paren_subscript_start`): when a parenthesized decorator
expression is followed by subscripts (`@(f)() a;`, `@(a?.b)() b;`),
acorn-typescript starts the resulting call/member nodes after the opening
paren (at the inner expression) — inconsistent with its own non-decorator
parse of `(f)()`, and with babel and tsc, which both start at the `(`. tsv
starts at the `(` uniformly. No prettier-stable fixture form: both formatters
normalize these decorators (`@(f)()` → `@f()`, `@(a?.b)()` → `@((a?.b)())` —
see the
[parenthesized](../tests/fixtures/typescript/typescript_specific/decorators/parenthesized/)
fixture's variants), and the normalized forms parse identically. **Upstream
candidate**: acorn-typescript decorator subscript start position.

### Upstream Fix Candidates

All corrections exist because of upstream bugs. If fixed upstream, tsv would remove the `_svelte_divergence` suffix, delete `expected_ours.json`, and rename `expected_svelte.json` → `expected.json`.

**acorn-typescript** — fix in acorn-typescript, then Svelte updates its dependency:

- Async generic arrow params — params dropped when `async` + type params
- `using` / `await using` — ES2024 declarations not recognized
- `const` type params — `const` modifier on class type params
- Import type options — `import()` type assertion options

**acorn** — fix in acorn core:

- ES2024 v-flag regex — Unicode sets `v` flag not supported

**Svelte CSS parser** — fix directly in Svelte:

- Forgiving :is()/:where() — Strict parsing where spec requires forgiving
- :nth-child(An+B of S) — Incorrect AST structure for `of S` syntax
- Attribute namespaces — `[ns\|attr]` not supported
- No-namespace selectors — `\|element` not supported
- Empty-after-comment decl — Rejects `prop: /* c */;` after stripping comments (5.55.x) — Prettier still formats it
- Stray `;;` garbage declaration — `border-box;;` yields `{property: ";"}` swallowing the next declaration (spec: drop empty declarations)
- Comment-touching-property garbage — `color/* c */:` yields `property: "color/*"` (`read_until` scans to the whitespace inside the comment)

**Svelte template parser** — fix directly in Svelte:

- each-`as` stale `loc.end` — TS-mode as-expression unwrap patches the expression's `end` offset but not `loc.end`

### Comment Attachment Differences

Acorn-typescript's backtrack-reparse behavior causes comment duplication in `trailingComments`/`leadingComments` (and, for some constructs, in the root `comments` array itself) that tsv doesn't replicate. These are cosmetic AST differences — the set of distinct comments is identical, only multiplicity differs, and `ast_diff` confirms semantic equivalence. Fixtures use `expected_ours.json` + `expected_svelte.json`.

| Context                               | Acorn attachment                                           | tsv attachment | Fixture                                                                                                                                                   |
| ------------------------------------- | ---------------------------------------------------------- | -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Return type to `;` (class methods)    | `trailingComments` on type annotation (duplicate)          | Not duplicated | [method_trailing_semicolon_comment](../tests/fixtures/typescript/declarations/class/method_trailing_semicolon_comment_svelte_prettier_divergence/)                 |
| Return type to `;` (type members)     | `trailingComments` on type annotation (duplicate)          | Not duplicated | [trailing_semicolon_comment](../tests/fixtures/typescript/types/type_members/trailing_semicolon_comment_svelte_divergence/)                               |
| Index-signature in-bracket comments   | Root `comments` duplicate (before-key; type-lit after-key) | Single entry   | [index_signature_bracket_comment_positions](../tests/fixtures/typescript/types/type_members/index_signature_bracket_comment_positions_svelte_divergence/) |
| Class index-signature in-bracket cmts | Root `comments` duplicate (before-key, after-key)          | Single entry   | [index_signature_bracket_comment_positions](../tests/fixtures/typescript/declarations/class/index_signature_bracket_comment_positions_svelte_divergence/) |

### Known Acorn-TypeScript Bugs (Not Corrections)

These are bugs in standalone acorn-typescript that **don't affect Svelte users** (Svelte's wrapper handles them):

**Abstract methods break namespace export scope tracking** (acorn-typescript@1.4.13): Abstract methods inside abstract classes corrupt the module scope, causing subsequent namespace imports to fail. Raw `.ts` parsing fails but `.svelte` files work fine. No fixture needed.

---

## Compat Behaviors

Implementation oddities in Svelte's parser that tsv replicates for AST compatibility. These are NOT in divergence directories—tsv matches Svelte exactly.

### CSS Compat Behaviors

- Backslash doubling in values — raw source extraction in `crates/tsv_css/src/ast/convert.rs`
- Unicode escape first-digit duplication — raw source extraction in `crates/tsv_css/src/ast/convert.rs`
- Comment-before-colon in declaration value — `crates/tsv_css/src/ast/convert.rs`
- Block-comment stripping in declaration value — `strip_css_comments` in `crates/tsv_css/src/ast/convert.rs`
- Block-comment stripping in at-rule prelude — `strip_css_comments` in `crates/tsv_css/src/ast/convert.rs`
- ::slotted()/::part() span truncation — `crates/tsv_css/src/ast/convert.rs`
- :dir()/:lang()/::highlight() identifier wrapping — `crates/tsv_css/src/ast/convert.rs`

Backslash doubling and unicode-escape duplication are inherited "for free" by extracting raw bytes (`source[span]`) into the public JSON value — Svelte's parser embeds those quirks in its span, so reproducing the bytes reproduces the quirks. No quirk-specific encoder runs.

**Block-comment stripping** (added in Svelte 5.55.x): the public `Declaration.value` and `Atrule.prelude` strings have `/* … */` comments removed in place (surrounding whitespace preserved) and the result trimmed. tsv applies this in `strip_css_comments` at the conversion boundary; the helper is string- and `url()`-aware so `/*` sequences inside `"…"`, `'…'`, or `url(…)` are kept verbatim.

### TypeScript Compat Behaviors

- Radix-literal digit-fold accumulation — `parse_radix_f64` in
  `crates/tsv_ts/src/parser/scan.rs` mirrors acorn's `readInt`
  (`total = total * radix + val` in doubles), which past 2^53 can land one
  ulp below the correctly rounded value (V8/`parseInt` round exactly; acorn
  doesn't). Matching acorn is the conformance target — don't "fix" with a
  u128 cast. Pinned by
  [literals/numeric/edge_cases](../tests/fixtures/typescript/expressions/literals/numeric/edge_cases/)
  (`hexBeyondSafe`/`octBeyondSafe`).
- LF-only line tracking in Svelte contexts — Svelte's `locate-character`
  counts only `\n` as a line start, so `LocationTracker::new` does too for
  Svelte template/CSS/embedded-script locations. Standalone TypeScript uses
  `LocationTracker::new_ecmascript` (LF, CR, CRLF, U+2028, U+2029 — acorn's
  `LineTerminator` set, applied even inside string literals). The same file
  content can therefore carry different `loc` values by context — pinned by
  [syntax/unicode_line_terminators](../tests/fixtures/typescript/syntax/unicode_line_terminators/)
  (`.ts` deliberately; see `INTENTIONAL_TS` in `ts_fixture_audit`).

Compat behaviors live in the **conversion layer** wherever possible: the
internal AST stays clean and semantic, and quirks apply only when generating
Svelte-compatible JSON. Two exceptions sit deeper by design: the radix
digit-fold runs in the parser (the internal numeric value is the folded one —
formatting reads raw source, and every JSON consumer wants acorn's value, so
a spec-rounded internal value would have no consumer), and line tracking is a
per-context tracker choice rather than a conversion step.

**At-rule preludes — source-extracted at the boundary.** The public `Atrule.prelude` is reproduced from the raw source span (`strip_css_comments(span.extract(source))`) for every prelude shape — the structured `@import`/`@scope`/`@supports`/`@container`, raw `@media`, and the raw path (`@layer`, `@keyframes`, `@namespace`, `@page`, …) — so it stays byte-for-byte with Svelte's verbatim string even on non-canonical whitespace (`@layer a , b` → `a , b`; `@namespace url(  x  )` → `url(  x  )`). The parser still builds a _normalized_ prelude string, but it is now printer-facing only: the formatter consumes it, the public AST does not. (`@media` normalizes its query; `@namespace` is value-normalized to match postcss; other raw at-rules keep the prelude verbatim — all only on the formatter side.) The internal-vs-public split is therefore complete for preludes.

### Escape Handling Layers

Understanding CSS escapes requires understanding 5 layers:

1. **CSS Syntax**: `\\` = one literal backslash
2. **Lexer Tokens**: Escapes preserved as-is
3. **Parser AST**: Semantic representation (no compat behaviors)
4. **JSON Serialization**: serde_json escapes backslashes
5. **Shell/Testing**: Additional escaping

The same backslash: source `\\` (2 bytes) → Svelte value `\\\\` (4 bytes) → JSON `\\\\\\\\` (8 bytes)

### Svelte Source References

- `node_modules/svelte/src/compiler/phases/1-parse/read/style.js`
  - `read_value()` (lines 502-536) — backslash doubling

---

## Svelte Behavior Reference

Documentation of Svelte parser behavior (not compat behaviors or corrections).

### Directive Modifiers

Svelte's parser accepts `|modifier` syntax on all directive types (permissive parsing), but only three have official support:

| Directive             | Syntax                                      | Modifiers                                                                                                                      |
| --------------------- | ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `OnDirective`         | `on:event\|mod`                             | `preventDefault`, `stopPropagation`, `stopImmediatePropagation`, `passive`, `nonpassive`, `capture`, `once`, `self`, `trusted` |
| `TransitionDirective` | `transition:\|mod`, `in:\|mod`, `out:\|mod` | `local`, `global`                                                                                                              |
| `StyleDirective`      | `style:prop\|mod`                           | `important`                                                                                                                    |

Directives without official modifiers: `AnimateDirective`, `BindDirective`, `ClassDirective`, `LetDirective`, `UseDirective`.

**tsv behavior**: Match Prettier—only output `modifiers` field for directives that officially support them.

**Reference**: `svelte/packages/svelte/src/compiler/types/template.d.ts`

---

## Related

- ./conformance_prettier.md — Prettier formatter differences
- ./checklist_css.md — CSS feature matrix
- ./fixture_overview.md — Fixture system details
