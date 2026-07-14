# Svelte Conformance

The tsv parser aims for **exact AST compatibility** with Svelte's parser. This document catalogs tsv's compatibility behaviors and intentional corrections.

## Mental Model

**Matched**: tsv produces identical AST to Svelte (the goal). This includes replicating Svelte's quirky behaviors for tool compatibility.

**Unmatched**: tsv produces different AST. The suffix `_svelte_divergence` marks these fixtures. tsv differs when Svelte or acorn-typescript is wrong — a spec violation, a missing feature, or a bug tsv corrects (e.g. Svelte's comment glue duplicating a comment across `<script>` boundaries). One exception isn't a correction: a lone UTF-16 surrogate can't survive tsv's UTF-8 strings (→ U+FFFD), so tsv differs there despite acorn being right.

## Classification

- **Compat behavior** — Svelte has quirky but harmless behavior (design choices, tokenization quirks, output that doesn't affect semantics). tsv replicates it in AST output
- **Correction** — Svelte/acorn violates spec, corrupts semantics, or lacks a spec-defined feature (e.g. acorn dropping all params from an `async <T>()` arrow). tsv produces correct/complete AST
- **Representation limit** — a value acorn keeps can't round-trip tsv's UTF-8 strings (lone surrogate → U+FFFD; `raw` unaffected). Rare, not a correction

**Critical distinction**: Compat behaviors apply ONLY to **AST/JSON output** for tool compatibility. The tsv **formatter** always produces clean, standards-compliant code.

---

## Corrections Catalog

Cases where tsv intentionally produces different AST than Svelte. Fixtures use `_svelte_divergence` suffix.

**Corpus-scale enforcement**: `deno task corpus:compare:parse` deep-diffs tsv's
parse output against the canonical parsers on real codebases and classifies
diffs against this catalog (the `DOCUMENTED_MATCHERS` list in
`benches/js/corpus_compare_parse.ts` covers the divergences that parse on
both sides). Keep the two in sync: a new documented AST divergence gets a
matcher, and an unmatched corpus diff group is either a bug or a missing
catalog entry.

### CSS Corrections

- :nth-child(An+B of S) — Incorrect AST structure; Svelte reads the ` of ` into `Nth.value` (`"2n of "`, from its `REGEX_NTH_OF` terminator) and flattens `S` as sibling simple selectors of the Nth. Per [Selectors 4 §the-nth-child-pseudo](https://drafts.csswg.org/selectors/#the-nth-child-pseudo) the `S` in `:nth-child(An+B [of S]?)` is a nested `<complex-real-selector-list>` scoped to the nth term, so tsv keeps `Nth.value = "2n"` with `S` under a `Nth.selector` field (matcher `nth_of_structure`) — [nth_child_of](../tests/fixtures/css/selectors/pseudo_class/nth_child_of_svelte_prettier_divergence/). The same nesting applies when `S` is a bare `<number>`/`<an+b>` term (`2n of 123`), which both parsers over-accept as an `Nth` (the `in_pseudo_args` production, parsed like a direct `:is()` arg) — [nth_child_of_number](../tests/fixtures/css/selectors/pseudo_class/nth_child_of_number_svelte_divergence/)
- Negative An+B in :nth-child() — Svelte's `:nth-child` reader over-rejects spec-valid negative forms (`-3`, `-2n`, `-2n - 3`, `-0`; a bare negative `<integer>`, a negative `<n-dimension>`, or `<n-dimension> ['+' | '-'] <signless-integer>` per [css-syntax-3 §An+B](https://drafts.csswg.org/css-syntax/#anb-microsyntax)) while accepting the leading-`-n` and `+`-tailed forms. tsv's lenient `:nth-child` reader follows the spec (matching prettier, format-stable) — [nth_child_negative](../tests/fixtures/css/selectors/pseudo_class/nth_child_negative_svelte_divergence/)
- Leading `-n` An+B in :nth-child() — the accept-but-mis-parse sibling of the row above: Svelte reads `-n` / `-n - 3` as a `TypeSelector` / flattened type-selector+combinator chain (only the `+`-tailed `-n + 6` reads as one `Nth`), where tsv reads a single spec-conformant `Nth` — [nth_child_leading_n](../tests/fixtures/css/selectors/pseudo_class/nth_child_leading_n_svelte_divergence/)
- Comments in :nth-\*() args — Rejected (`css_expected_identifier`) except before the An+B — [nth_comment](../tests/fixtures/css/selectors/pseudo_class/nth_comment_svelte_prettier_divergence/)
- Comments at combinator boundaries — Rejected (`css_expected_identifier`); tsv accepts them as inter-token trivia (CSS Syntax 3 — removed at tokenization, producing no token, not even whitespace) in every position — descendant/child/sibling gap (`div /* c */ p`), before/after an explicit combinator, glued between compound members (`.a/* c */.b`), and a `:has()` relative-selector leading combinator. tsv normalizes the gap spacing to a single space (prettier freezes it — a `_prettier_divergence`, see [conformance_prettier.md §CSS: Comments](conformance_prettier.md#css-comments)) — [combinator_comment](../tests/fixtures/css/selectors/combinator_comment_svelte_prettier_divergence/)
- Glued comment run in a compound — Rejected (`css_expected_identifier`); tsv keeps `.a/* c *//* d */.b` a compound (two adjacent glued comments are inter-token trivia, not a descendant) and emits the run verbatim. Prettier agrees it's a compound but relocates the `{` (a `_prettier_divergence`, see [conformance_prettier.md §CSS: Comments](conformance_prettier.md#css-comments)) — [compound_comment_run](../tests/fixtures/css/selectors/compound_comment_run_svelte_prettier_divergence/)
- Comments between `::part()` names — Rejected (`css_expected_identifier`); a comment in an interior gap (`::part(a /* c */ b)`) reads as whitespace and splits the identifier run in Svelte's scanner, while tsv accepts it as inter-token trivia (CSS Syntax 3) and normalizes the gap to a single space (prettier freezes it — a `_prettier_divergence`, see [conformance_prettier.md §CSS: Comments](conformance_prettier.md#css-comments)). The edge positions (before/after the run) are accepted by parseCss — see [part_comment](../tests/fixtures/css/selectors/pseudo_element/part_comment_prettier_divergence/) — [part_interior_comment](../tests/fixtures/css/selectors/pseudo_element/part_interior_comment_svelte_prettier_divergence/)
- Consecutive combinators (`> > .a`, `+ ~ .d`, glued `>>.a`) — parseCss **collapses** a run of combinators to its last: its `read_selector` never emits an empty relative selector, so on the second combinator it drops the earlier anchorless one. tsv **preserves** every authored combinator, emitting an empty-compound `RelativeSelector` per anchorless one (`+ ~ .d` → `[+, []]` then `[~, [.d]]`), so `expected_ours.json` carries relative selectors `expected_svelte.json` drops. The collapse is a lossy recovery tsv declines — the dropped combinator is authorship the future diagnostics layer needs, and in a relative context it silently *validates* the invalid selector (`:has(+ ~ .d)` → `:has(~ .d)`). Prettier also collapses (or freezes a glued run), so this is a `_prettier_divergence` too (see [conformance_prettier.md §CSS: Selectors](conformance_prettier.md#css-selectors)); a *trailing* combinator (`.a > > {}`) still rejects in both — [consecutive_combinator](../tests/fixtures/css/selectors/consecutive_combinator_svelte_prettier_divergence/)
- Attribute namespaces `[ns|attr]` — Not supported — [namespace](../tests/fixtures/css/selectors/attribute/namespace_svelte_divergence/)
- No-namespace `|element` — Not supported — [no_namespace](../tests/fixtures/css/selectors/namespace/no_namespace_svelte_divergence/)
- Forgiving :is()/:where() — Strict parsing (should be forgiving); tsv drops both syntactically invalid items (`.`, `[`) and contextually invalid ones (known syntax in the wrong place — e.g. an `An+B`/`of S` term, valid only in `:nth-*()`, so `:is(2n of)` → empty), while Svelte fails the whole parse — [forgiving_is_where](../tests/fixtures/css/selectors/forgiving_is_where_svelte_divergence/)
- Forgiving :is()/:where() dropped-item newline — the formatter side of the row above: a dropped invalid item spanning a newline (`:is(.a > .⏎> .b)`) has its preserved verbatim text's whitespace runs (including the newline) collapsed to single spaces, matching prettier (which collapses whitespace inside a selector) — the same rule tsv applies to every other selector-argument position. Parser behavior is unchanged from the row above (the item is still dropped from the AST) — [forgiving_is_where_newline](../tests/fixtures/css/selectors/forgiving_is_where_newline_svelte_divergence/)
- Empty-after-comment declarations — Rejected (`css_empty_declaration`) — [comment_empty_value](../tests/fixtures/css/tokens/comments/comment_empty_value_svelte_divergence/)
- `;` inside a function value (`prop: fn(a; b)`) — Rejected (`css_empty_declaration`); the inner `;` is truncated as a declaration terminator, but per CSS Syntax 3 a `;` inside a `fn(…)` simple block is block content — tsv (and prettier) keep the declaration whole — [function_semicolon](../tests/fixtures/css/values/function_semicolon_svelte_divergence/)
- `;` inside a simple block or `var()` fallback (`(x;y)`, `[x;y]`, `var(--d, ;)`) — Rejected (`css_empty_declaration`); the same class as the function case, extended to `()` / `[]` simple blocks and the `var()` fallback — all balanced units per CSS Syntax 3, so an inner `;` is content — tsv (and prettier) keep the declaration whole — [balanced_semicolon](../tests/fixtures/css/values/balanced_semicolon_svelte_divergence/)
- `<general-enclosed>` `@supports` condition with `;` (`@supports (margin: 0;)`, `@supports foo(a; b)`) — Rejected (`css_empty_declaration`); per CSS Conditional 3 a `<general-enclosed>` = `(<any-value>)` / `fn(<any-value>)` admits any balanced token run incl. `;`, so it parses (evaluates false) — tsv (and prettier) keep it stable — [supports_general_enclosed](../tests/fixtures/css/at_rules/supports_general_enclosed_svelte_divergence/)
- Block-valued custom properties — Rejected (`css_expected_identifier`) — [block_value](../tests/fixtures/css/values/variables/block_value_svelte_prettier_divergence/)

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
  Today tsv **errors on the first invalid construct**, which aborts the whole
  stylesheet — so one bad rule currently discards the file's valid rules too. That
  is a way-station: a spec-compliant parser drops only the offending
  declaration/rule and keeps going (CSS Syntax 3's _consume a declaration_ /
  _consume a block's contents_, §5.5 — a missing colon is a parse error that
  "returns nothing," and the block skips the item rather than aborting). The
  throw-don't-recover model is inherited from Svelte — but tsv is now _stricter_
  than `parseCss`, not equal to it: `parseCss`'s declaration reader is
  colon-optional and scan-based (`read_declaration`), so it **lenient-accepts**
  malformed `prop value;` — and even `//`-comment — lines as `{property, value}`
  nodes that tsv rejects. prettier/postcss rejects those same lines, so tsv's
  stricter parse currently tracks the _formatter_ oracle; spec error recovery
  matches **neither** oracle (parseCss keeps the bad declaration, prettier rejects
  the whole file) and is tracked as future work.
- **A corpus "CSS failure" is usually a deliberate rejection, not a gap.** In the
  benchmark corpus tsv parses a lower share of `.css` than prettier/biome/oxfmt,
  but that gap is **scope, not deficiency**: those tools run the lenient PostCSS /
  `postcss-scss` / `postcss-less` stack; tsv does not. The rejected files are
  overwhelmingly the non-goal dialects listed under "Explicit non-goals" below.
  "Skipped CSS" is **not** a synonym for "SCSS" — most are other non-CSS dialects.
- **A leading combinator is accepted in every context (contextual invalidity,
  deferred to diagnostics).** A complex selector may begin with a combinator
  (`> span {}`, `+ p {}`, `~ p {}`) at the top level, in an `@media`/`@supports`/
  `@layer` body, in a functional pseudo-class arg (`:not(> .a)`, `:is(> .a)`,
  `:where(> .a)`), and in an `@scope` prelude (`@scope (> .b)`, `to (> .b)`).
  Outside a relative-selector context (nesting, `:has()`, the `@scope` *body*) a
  leading combinator has no anchor element, so it is spec-invalid per Selectors 4
  (a top-level `<complex-selector>` / non-relative `<scope-start>`/`<scope-end>`
  cannot lead with `>`/`+`/`~`). But this is a **contextual** invalidity — valid
  combinator grammar in an invalid position — not a malformed token, so tsv parses
  it into the same `RelativeSelector`-with-combinator AST Svelte's `parseCss`
  produces (dropping the empty implied anchor, exactly as `read_selector` does) and
  defers the "no anchor here" judgment to the future diagnostics layer. This is the
  same permissive-parser posture tsv takes for TS early-errors: Svelte's own
  *validator* (a stage tsv doesn't run) rejects these with `css-selector-invalid` —
  they are its `validator/samples/css-invalid-combinator-selector` fixtures, which
  its *parser* accepts — and prettier formats them unchanged. A **trailing**
  combinator (`p > {}`, a combinator with nothing after it) is a genuine parse
  error both parsers reject. A **run** of consecutive combinators (two or more with
  no compound between them — `> > .a`, `+ ~ .d`, glued `>>.a`) is a separate matter:
  parseCss *collapses* the run (dropping all but the last combinator), while tsv
  **preserves** every authored combinator — a deliberate `_svelte_prettier_divergence`
  cataloged in [§CSS Corrections](#css-corrections) below. Distinct from the
  grammar-invalid tokens/values in the bullet below, which tsv still rejects. Fixture:
  [css/selectors/leading_combinator](../tests/fixtures/css/selectors/leading_combinator/input.svelte).
- **The "Svelte over-accepts" cases are not a tsv correctness win.** Svelte
  accepts some grammar-invalid CSS that tsv rejects — an invalid attribute
  case-flag (`[type=a x]`; Selectors 4 allows only `i`/`s`), a function token as
  an attribute value (`[id=func("foo")]`), a `url` keyword split across whitespace
  in `@import`, and a
  backslash immediately before a newline outside a string
  (`color: red\` + newline — an invalid escape per CSS Syntax 3 §4.3.7; Svelte
  reads the `\` into the value, and prettier never converges on it). tsv is
  **grammar-stricter**, but _not_ more spec-correct: the spec
  neither keeps these (Svelte's leniency is wrong) nor aborts the file (tsv's
  hard-fail is wrong) — it drops the bad rule and keeps the rest. All of these
  differ from the spec; recovery is the resolution that subsumes both, and until
  then these stay documented near-term divergences from Svelte. (A backslash at
  **end of input**, by contrast, is rejected by both parsers — pinned by the
  `input_invalid_escape_eof_*` files in
  [css/tokens/escapes/escape_eof](../tests/fixtures/css/tokens/escapes/escape_eof/input.svelte).)

**Explicit non-goals.** Preprocessor and vendor dialects — SCSS/Sass, LESS, CSS
Modules, PostCSS plugin syntax, YAML front-matter, and IE hacks (`*zoom`,
`_width`, `+color`, `color: red\9`) — are **permanent** non-goals. tsv targets the
CSS spec, not these dialects, and will not add handling to parse or preserve them.
This is distinct from error recovery: recovery is about not letting one invalid
construct abort an otherwise-valid _standard-CSS_ file; these dialects are input
tsv never chases regardless.

Non-standard `.css` is auto-classified into `expected errors` by the corpus
comparator (`benches/js/lib/divergence/expected_errors.ts`).

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

- **Typed block-pattern `end`/`loc` split** — reproduced, not corrected. Svelte's
  `read_pattern` (`1-parse/read/context.js`) handles a typed block binding two
  different ways, and tsv matches both. For a plain identifier
  (`{#each xs as item: T}`) it returns the identifier with `start`/`end`/`loc`
  untouched and the annotation as a sibling field — so the binding's span covers
  only the name, unlike an ordinary TS binding identifier, whose span is
  tail-anchored over its `: T`. For a **destructuring** pattern
  (`{#each xs as { a }: T}`, `{:then { a }: T}`, `{:catch { a }: T}`) it patches
  `expression.end = typeAnnotation.end` but **never touches `expression.loc`** —
  so `end` and `loc.end` genuinely disagree. tsv keeps the internal span on the
  bare pattern (which `loc` derives from) and widens only the emitted `end`, via a
  `max` in the wire writer, so a plain signature parameter — whose span already
  covers its annotation — is unaffected. Same context-reparse-loc family as the
  each-`as` correction above and the block binding-pattern interior-comment column
  offset below, but here the quirk is *matched* rather than fixed: it is a shape in
  the wire AST, not a slip in a position tsv can independently derive. Pinned by
  [each/typed_context_destructured](../tests/fixtures/svelte/blocks/each/typed_context_destructured/)
  and [await/typed_value_destructured](../tests/fixtures/svelte/blocks/await/typed_value_destructured/).

### TypeScript Corrections

Svelte uses acorn + acorn-typescript, which lags behind TypeScript's parser. tsv implements the full spec.

**Oracle note.** acorn-typescript is tsv's AST-**shape** drop-in target, *not* its
correctness oracle — it is both over-lenient and over-strict versus the real
compiler. For **validity** (what is or isn't a TS error) the oracle is **tsc**, and
tsv's parser is deliberately **permissive**: it accepts the full syntactic grammar
and defers static-semantic early-errors (e.g. the ambient-context rules — a `declare`
member body, initializer, or decorator) to a future diagnostics layer. The practical
test for accept-vs-reject is **whether prettier formats it** — if prettier formats
it, tsv accepts it (and defers any error), because tsv is first a formatter and must
format everything well-formed. So a "correction" below is tsv matching **tsc/spec**
(and prettier), not acorn.

Svelte ❌ / Prettier ✅ / tsv ✅ in every case below:

- `using` declarations (ES2024) — [basic](../tests/fixtures/typescript/typescript_specific/using/basic_svelte_divergence/)
- `await using` declarations — [await](../tests/fixtures/typescript/typescript_specific/using/await_svelte_divergence/)
- `const` type params in classes — [const_type_param_class](../tests/fixtures/typescript/typescript_specific/generics/const_type_param_class_svelte_divergence/)
- `const` type params in interfaces, incl. `const` before variance (`<const in T>`) — [const_type_param_interface](../tests/fixtures/typescript/typescript_specific/generics/const_type_param_interface_svelte_divergence/). acorn rejects the `const` token (tsc defers it to the TS1277 checker error); the mis-ordered `<in const T>` is instead a *grammar* error tsv rejects like acorn, pinned by the regular fixture [type_param_modifier_order](../tests/fixtures/typescript/typescript_specific/generics/type_param_modifier_order/)
- Import type options — [dynamic_attributes](../tests/fixtures/typescript/modules/imports/dynamic_attributes_svelte_divergence/)
- ES2024 v-flag regex — [unicode_sets_advanced](../tests/fixtures/typescript/expressions/literals/regex/unicode_sets_advanced_svelte_divergence/)
- `export default class implements I {}` (anonymous default class, implements-first heritage) — [export_default_implements](../tests/fixtures/typescript/declarations/class/export_default_implements_svelte_divergence/)
- A cast as the left operand of `**` (`x as number ** 2`) — see below; the rejection itself is not pinnable
- Async generic arrow params — see fixtures below

**`using` keyword-name comments**: Both acorn and tsv reject comments between `using` and the binding name (`using /* c */ x = fn()`), and between `await` and `using` (`await /* c */ using x = fn()`). Per the ECMAScript spec, comments behave like white space and are discarded between any two tokens (§12.4), so these should be valid. However, since `using` is a contextual keyword requiring lookahead disambiguation, both parsers check the next token before comment processing. tsv matches acorn's behavior here. If acorn adds support, tsv should follow.

**Cast as the left operand of `**`**: acorn-typescript rejects `x as number ** 2` / `x satisfies number ** 2` (`Unexpected token` at the `**`). tsc accepts both — it parses the cast as the `**` left operand, and its "unary expression is not allowed in the left-hand side of an exponentiation expression" grammar error (TS17006) fires only for a *prefix-unary* operand (`-2 ** 3`), not for a cast. Prettier agrees, printing `(x as number) ** 2`, and tsv matches. **Upstream candidate**: acorn-typescript exponentiation after `as`/`satisfies`.

The rejection is the one case here that **cannot be pinned**. The `expected_svelte.json` = `{"error": "failed to parse"}` sentinel every fixture above uses attaches to `input.*`, and an `input.*` must be a formatting fixed point (F1) — `x as number ** 2` is not one, since both formatters normalize it to `(x as number) ** 2`. The source form can therefore only live in an `unformatted_*` variant, and the validator runs the canonical parser over `input.*` and `input_invalid_*` only, never over variants. So [as_satisfies_exponentiation](../tests/fixtures/typescript/expressions/as_satisfies_exponentiation/) is a *regular* fixture: it pins the parse shape and the paren insertion (both operand sides — a cast on the right needs parens too, since `as` otherwise binds looser and takes the whole exponentiation), and its `unformatted_no_parens` variant carries the source form. That variant formats at all only because prettier-plugin-svelte re-parses `<script>` content with prettier's own TypeScript parser rather than with Svelte's — Svelte's parser sees the fixture's parenthesized `input.svelte` and is happy.

**Async generic arrow params**: acorn-typescript drops all function parameters from `async` arrow functions that have type parameters (`async <T,>(x: T) => x` → `params: []`). Non-async generic arrows are unaffected. This is semantic corruption — tools consuming the AST would see zero-argument functions. **Upstream candidate**: acorn-typescript async arrow parsing.

**Import-phase proposals (forward-looking, ungated).** tsv accepts the TC39
import-phase syntax — `import defer * as ns from '…'` / `import source x from '…'`
and the dynamic `import.defer(…)` / `import.source(…)` — and emits a `phase` field
(`'defer'` / `'source'`) on the `ImportDeclaration` / `ImportExpression` wire node
(declared in `crates/tsv_wasm/types/tsv_ast.d.ts`). Unlike every case above, this one
is **un-fixturable** — and not because the canonical parsers reject it. A canonical
rejection on its own is pinnable, via the `expected_svelte.json` = `{"error": "failed
to parse"}` sentinel that every fixture above uses; what those fixtures still have, and
import-phase does not, is a *second* oracle. **prettier** is no oracle here — it drops
the `defer` keyword (silent content loss) and rejects `import source`, so there is no
format claim to pin. With `expected_ours.json` self-generated and no formatter to check
it against, the fixture would assert only that tsv agrees with tsv. The syntax is also
not yet in the finished ECMAScript standard. The emitted `phase`
shape mirrors the TC39 proposals' AST; because there is no oracle, it is a deliberate
extension rather than a drop-in guarantee, and **if acorn-typescript later implements
import-phase with a different shape, tsv should re-align to it**. Emitted from
`crates/tsv_ts/src/ast/convert/write/statements.rs` (declaration) and
`crates/tsv_ts/src/ast/convert/write/expressions.rs` (expression).

Its one accept-side consequence is a *reverse* divergence — tsv **over-rejects** here. A parameter decorator is invalid on an arrow in every form (tsc + prettier + acorn all reject `(@dec a) => a`, `<T>(@dec a) => a`, `async (@dec a) => a` — the drop-in rejections pinned by the `input_invalid_*` cases in [decorators/parameter_arrow](../tests/fixtures/typescript/typescript_specific/decorators/parameter_arrow/)). But in the async-generic form acorn *accepts* `async <T>(@dec a) => a`, only because the param-drop bug above silently discards the parameter and its decorator. tsc still rejects the decorator, so tsv rejects too — matching every other arrow form and diverging from acorn's lossy accept (a `tsv_rejects.txt` fixture).

Fixtures: [async_generic/stacked](../tests/fixtures/typescript/expressions/arrow/async_generic/stacked_svelte_prettier_divergence/), [async_generic/forms](../tests/fixtures/typescript/expressions/arrow/async_generic/forms_svelte_prettier_divergence/), [async_generic/basic_ts](../tests/fixtures/typescript/expressions/arrow/async_generic/basic_ts_svelte_divergence/), [async_generic/long](../tests/fixtures/typescript/expressions/arrow/async_generic/long_svelte_divergence/), [async_generic/param_decorator](../tests/fixtures/typescript/expressions/arrow/async_generic/param_decorator_svelte_divergence/), [curried_typed_callback](../tests/fixtures/typescript/expressions/arrow/curried_typed_callback_svelte_prettier_divergence/). `async_generic/forms` adds the optional-param (`x?`) drop, distinct from the plain param (`stacked`) and rest param (`long`); `async_generic/param_decorator` is the over-rejection direction (tsv rejects a decorator acorn's param-drop swallows).

The `async_generic/stacked`, `async_generic/forms`, and `curried_typed_callback` fixtures carry a second,
independent divergence — prettier's forced `<T,>` trailing comma on single-unconstrained
arrow type params (hence the `_svelte_prettier_divergence` suffix). See
[conformance_prettier.md](./conformance_prettier.md) §TypeScript.

**Type assertion vs. generic arrow**: at a `<` in expression position,
acorn-typescript tries the generic-arrow reading first, and its Babel-ported
"abort on a parenthesized arrow" check is dead code (acorn never sets
`extra.parenthesized`), so `<T>` followed by *any* arrow parses as the arrow's
type parameters. TypeScript (and Babel) instead read a type assertion in
JSX-free `.ts`. tsv follows TypeScript, in three forms: `<any>(() => {})` is a
`TSTypeAssertion` over the parenthesized arrow
([type_assertion_paren_arrow](../tests/fixtures/typescript/expressions/type_assertion_paren_arrow_svelte_divergence/);
also corpus-enforced via the `type_assertion_paren_arrow` matcher — the
divergent reading shows up in real code, e.g. prettier's own test corpus);
`<T>x => x` and `<T,>(() => {})` are parse errors tsv rejects while acorn
accepts — a rejection can't be an `input_invalid_*` fixture when the canonical
parser accepts, so each is a `tsv_rejects.txt` fixture pinning both halves:
[type_assertion_arrow/operand](../tests/fixtures/typescript/expressions/type_assertion_arrow/operand_svelte_divergence/)
and
[type_assertion_arrow/type_params](../tests/fixtures/typescript/expressions/type_assertion_arrow/type_params_svelte_divergence/).
The ordinary generic-arrow forms (`<T>(x: T) => x`) and assertion forms whose
type can't parse as type parameters (`<any[]>(() => {})`) agree in both parsers
(standalone-TS accept boundaries pinned by `tests/type_assertion_arrow.rs`).
**Upstream candidate**: @sveltejs/acorn-typescript — the dead
`extra.parenthesized` abort in `parseMaybeAssign`'s arrow `tryParse`.

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

**Decorator private-name member chains** (`@C.#p`): the TC39 decorators
grammar includes `DecoratorMemberExpression . PrivateIdentifier`, and test262
grades it (`decorator-member-expr-private-identifier.js`, including escaped
and keyword-named forms like `#\u{6F}` and `#await`). acorn-typescript
rejects the bare form (`Unexpected token`); tsv parses it per the grammar, as
does prettier's typescript parser. The bare form is not format-stable — a
private name in the chain fails prettier's `isDecoratorMemberExpression`
check, so both tsv and prettier normalize `@C.#p` to the parenthesized
`@(C.#p)`, which every parser accepts — so the divergence only surfaces on
unformatted source. The
[private_member](../tests/fixtures/typescript/typescript_specific/decorators/private_member/)
normalization fixture pins the acceptance via its `unformatted_no_parens`
variant; a bare private-name head (`@#p`) is not in the grammar and stays
rejected. **Upstream candidate**: acorn-typescript decorator
`PrivateIdentifier` member step.

**Anonymous class-expression `id` for implements-first heritage**
(`class implements I {}`): acorn-typescript omits the `id` key entirely from an
anonymous class *expression* whose first heritage clause is `implements` with no
name, type parameters, or `extends` — yet emits `id: null` for every other
anonymous class (`class {}`, `class extends B {}`, `class<T> implements I {}`).
ESTree specifies `id: Identifier | null` (always present), so tsv emits
`id: null` consistently across all anonymous classes. Harmless metadata only —
the `id` key is the sole difference, `ast_diff` confirms semantic equivalence,
and formatting is unaffected. Fixture:
[expression_implements](../tests/fixtures/typescript/declarations/class/expression_implements_svelte_divergence/).
**Upstream candidate**: acorn-typescript class-expression `id` omission.

**Dynamic-import trailing comma** (`import('x',)`, `import('x', opts,)`): the
ECMAScript `ImportCall` grammar permits an optional trailing comma after the
source and after the options argument
([ecma262 §16.2.4.1](https://tc39.es/ecma262/#prod-ImportCall)).
acorn-typescript rejects it (`Unexpected token`); tsv accepts it per spec
(prettier/babel and oxc accept it too). The comma is not format-stable — both
tsv and prettier strip it (`trailingComma: 'none'`) — so it surfaces only on
unformatted source; the
[import_trailing_comma](../tests/fixtures/typescript/expressions/calls/import_trailing_comma/)
normalization fixture pins the acceptance via an `unformatted_*` variant.
Conversely, acorn-typescript *over-accepts* three or more arguments
(`import('x', a, b)`), which the grammar forbids — tsv rejects them, staying
spec-faithful in both directions. **Upstream candidate**: acorn-typescript
`ImportCall` argument handling.

**Legacy import-assertions `assert` clause (rejected)**: the abandoned Stage-3
predecessor of import attributes spelled the clause
`import x from 'm' assert { type: 'json' }`. It never merged into ecma262 —
the final grammar is `WithClause : with { … }`
([ecma262 §16.2.2](https://tc39.es/ecma262/#prod-WithClause)) — and engines
have since removed it. acorn-typescript still accepts it; tsv rejects it
(`Expected ';'`), parsing only the spec's `with` form. This is deliberate
spec-over-acorn strictness in the reverse direction of most entries here (tsv
stricter, not broader). A tsv-rejects/acorn-accepts input can't be an
`input_invalid_*` fixture (which requires both parsers to reject), so it is
pinned by the
[legacy_import_assert](../tests/fixtures/typescript/modules/imports/legacy_import_assert_svelte_divergence/)
`tsv_rejects.txt` fixture and the parse-parity gate's sanctioned list
(`benches/js/diagnostics/skip_triage.ts`).

**Reserved-keyword qualified type head (`void.X` / `null.X`, rejected)**: a type
keyword immediately followed by `.` is the HEAD of a qualified type name
(`string.X` → `TSTypeReference` over a `TSQualifiedName`). acorn-typescript's
`tsParseNonArrayType` accepts this for every keyword-type name *plus* the
reserved `void`/`null`, so `void.X` / `null.X` parse as a `TSQualifiedName`.
tsc and prettier reject them — `void`/`null` are reserved operators, not
entity-name heads — so tsv qualifies only the *contextual* type keywords
(`string`/`number`/`any`/`undefined`/…, matching tsc + prettier) and rejects the
reserved heads (`Expected ';'`). `true`/`false` are literal types on both sides,
so `true.X` rejects everywhere (the
[type_keyword_qualified_head](../tests/fixtures/typescript/types/type_keyword_qualified_head/)
fixture pins the accept direction, and its `input_invalid_true_qualified_head`
pins the both-reject `true.X`). This is deliberate tsc-over-acorn strictness, the
same reverse direction as the legacy import-assertions entry above. The
reserved-head rejection can't be an `input_invalid_*` fixture (acorn accepts it),
so it is pinned by the
[reserved_keyword_qualified_head](../tests/fixtures/typescript/types/reserved_keyword_qualified_head_svelte_divergence/)
`tsv_rejects.txt` fixture. **Upstream candidate**: acorn-typescript
`tsParseNonArrayType` — `void`/`null` accepted as qualified-name heads.

**Type-reference type arguments after a line break (`B` ⏎ `<T>`, rejected)**: a
type-argument list binds to the preceding type reference only when no line
terminator intervenes — TypeScript's `parseTypeArgumentsOfTypeReference` is
guarded by `!scanner.hasPrecedingLineBreak()`. So `B` ⏎ `<T>` is the type `B`
followed by a separate `<T>`, not `B<T>`. tsv applies the same guard it already
uses at the sibling type-argument sites (`typeof X` ⏎ `<T>`, `extends B` ⏎ `<T>`,
postfix `B` ⏎ `[]`). In a **type-member** list both parsers agree: the line break
splits `a: B` ⏎ `<T>(): C` into a property member and a call-signature member (and
`a: B` ⏎ `<T>;`, a bare type-argument list with no `(`, rejects in both) — pinned
as the ordinary fixture
[type_members/type_args_line_break](../tests/fixtures/typescript/types/type_members/type_args_line_break/).
In a **non-member** position (`let a: B` ⏎ `<T>;`, `type Y = B` ⏎ `<T>;`) tsc and
prettier reject, but acorn-typescript *recovers* — it parses the type as `B` and
treats the leftover `<T>;` as a floating `TSTypeParameterDeclaration`
expression-statement. tsv rejects (`Expected expression, found ';'`), matching
tsc/prettier and diverging from acorn's recovery. Since acorn accepts, that half
can't be an `input_invalid_*` fixture, so it is pinned by the
[type_args/line_break](../tests/fixtures/typescript/types/type_args/line_break_svelte_divergence/)
`tsv_rejects.txt` fixture. This is deliberate tsc-over-acorn strictness, the same
reverse direction as the reserved-keyword-qualified-head and arrow-as-operand
entries. **Upstream candidate**: acorn-typescript — `tsParseTypeReference`
consumes type arguments across a line break (no `hasPrecedingLineBreak` guard).

**Arrow function as an operand (rejected)**: an `ArrowFunction` is a complete
`AssignmentExpression` — a top-level alternative of that production
([ecma262 §13.15](https://tc39.es/ecma262/#prod-AssignmentExpression)), not a
`ConditionalExpression`, binary operand, or `LeftHandSideExpression`. So a *bare*
(unparenthesized) arrow cannot be extended by any operator: a trailing
binary/logical operator (`() => {} || a`), `as`/`satisfies` assertion
(`() => {} as T`), assignment target (`() => {} = a`), or ternary `?`
(`() => {} ? b : c`) is a syntax error — only a sequence `,` or a statement
terminator may follow. Parenthesizing the arrow (`(() => {}) || a`) makes it a
primary and lifts the restriction. tsc and prettier reject all of these
(`Expected ';'`, TS1005). acorn-typescript rejects the operator / assertion /
assignment forms too — pinned as the ordinary both-reject `input_invalid_*` cases
in [block_body_not_operand](../tests/fixtures/typescript/expressions/arrow/block_body_not_operand/) —
but *over-leniently accepts the ternary*: its arrow guard lives only in
`parseExprOps` (blocking a binary operator), while `parseMaybeConditional` sits
above it and still folds `?` onto the arrow test. tsc/prettier/spec reject it, so
tsv rejects it, matching the compiler and diverging from acorn's lone accept.
Since acorn accepts the ternary, that half can't be an `input_invalid_*` fixture,
so it is pinned by the
[block_body_ternary](../tests/fixtures/typescript/expressions/arrow/block_body_ternary_svelte_divergence/)
`tsv_rejects.txt` fixture. (Subscripts and calls on a bare arrow — `() => {}()`,
`() => {}.x` — are the same principle, pinned separately by
[block_body_not_callable](../tests/fixtures/typescript/expressions/arrow/block_body_not_callable/).)
This is deliberate tsc-over-acorn strictness, the same reverse direction as the
legacy import-assertions and reserved-keyword-qualified-head entries above.
**Upstream candidate**: acorn-typescript — `parseMaybeConditional` folds a
ternary onto an unparenthesized arrow above the `parseExprOps` arrow guard.

#### Import-phase proposals

The Stage-3 **source-phase imports** and **import defer** proposals add a phase to
both static and dynamic imports:

- `import source x from 'mod'` / `import.source('mod')` — phase `'source'`
- `import defer * as ns from 'mod'` / `import.defer('mod')` — phase `'defer'`

acorn-typescript implements neither (`import source x` → `Unexpected token`,
`import.source(…)` → `The only valid meta property for import is 'import.meta'`),
so accepting them is a deliberate, forward-looking divergence from the
Svelte/acorn oracle. tsv parses the valid forms and rejects the invalid ones per
the proposals' grammars (`import source ImportedBinding FromClause` takes a
**single** binding — no namespace, no named clause, no second specifier, so
`import source x, { a }` / `import source x, * as ns` are rejected; `import defer`
allows only the `* as ns` namespace shape; `import.source`/`import.defer` must be a
call, never a bare meta-property or member access; neither dynamic form takes a
spread argument), and tags the public AST node with a `phase: 'source' | 'defer'`
field (omitted for an ordinary import). `source` and `defer` stay contextual —
`import defer from 'mod'` still imports a default binding named `defer`.

**Known limitation — source-phase binding named like a contextual keyword.** The
spec disambiguates `import source x from 'mod'` (phase, binding `x`) from `import
source from 'mod'` (a default import named `source`) by which production yields a
complete parse: the source-phase reading needs a trailing `from` FromClause after
the binding. tsv approximates this with a one-token lookahead — `source` is the
phase only when the next token lexes as an `Identifier`, then enforces the
single-binding restriction after parsing it. That covers every binding except one
whose name is itself a contextual keyword the lexer emits as a non-`Identifier`
token (`from`, `as`): `import source from from 'mod'` is spec-valid (source-phase,
binding named `from`) but tsv rejects it. This is **deliberately not closed** —
spec-faithful resolution would need lookahead past the binding to the `from` plus a
binding parser that accepts keyword-lexed names, and a source-phase import whose
binding is literally named `from`/`as` is vanishingly rare. It is also **never
graded**: test262 encodes it only as a `_FIXTURE.js` (run by the host, not the
parser grader), so it doesn't dent the 100% positive rate. Pinned in
`tests/import_phase.rs` (`static_import_source_keyword_binding_rejected`, alongside
`static_import_source_single_binding_enforced`). The identifier-named-`source`
binding (`import source source from 'mod'`) parses fine.

**No `_svelte_divergence` fixture** (the fixture pipeline needs acorn to produce
`expected.json`, and acorn rejects the syntax). The parser is graded instead by the
test262 suite — ~396 graded files, all passing; see
[conformance_test262.md](./conformance_test262.md). Prettier diverges too (it drops
`import defer`'s phase and throws on `import source`), so the *printer* is covered by
`tests/import_phase.rs` rather than a fixture; the prettier side is cataloged in
[conformance_prettier.md](./conformance_prettier.md#import-phase-proposals).
**Upstream candidate**: acorn-typescript import-phase support — drop the divergence
and promote to fixtures once it lands.

### TypeScript Parser Corrections (corpus-enforced)

Intentional AST divergences from acorn-typescript that have no prettier-stable
fixture form (prettier rewrites the triggering syntax), so the corpus parse
differential enforces them via `DOCUMENTED_MATCHERS` in
`benches/js/corpus_compare_parse.ts` instead.

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
- Anonymous class-expression `id` — omitted for implements-first heritage
- `export default class implements I {}` — anonymous default class with implements-first heritage rejected (`implements` read as a reserved-word name)
- Type assertion vs. generic arrow — `<T>` before any arrow (even a parenthesized one) reads as type parameters; the parenthesized-arrow abort check is dead code

**acorn** — fix in acorn core:

- ES2024 v-flag regex — Unicode sets `v` flag not supported

**Svelte CSS parser** — fix directly in Svelte:

- Forgiving :is()/:where() — Strict parsing where spec requires forgiving
- :nth-child(An+B of S) — Incorrect AST structure for `of S` syntax
- Attribute namespaces — `[ns\|attr]` not supported
- No-namespace selectors — `\|element` not supported
- Empty-after-comment decl — Rejects `prop: /* c */;` after stripping comments — Prettier still formats it
- Block-valued custom properties — Rejects `--x: { … }` (`css_expected_identifier`) — Prettier still formats it
- Stray `;;` garbage declaration — `border-box;;` yields `{property: ";"}` swallowing the next declaration (spec: drop empty declarations)
- Comment-touching-property garbage — `color/* c */:` yields `property: "color/*"` (`read_until` scans to the whitespace inside the comment)

**Svelte template parser** — fix directly in Svelte:

- each-`as` stale `loc.end` — TS-mode as-expression unwrap patches the expression's `end` offset but not `loc.end`

### Comment Attachment Differences

**Svelte's comment glue duplicates or drops comments at `<script>` and template boundaries.** tsv attaches each comment once, in its source region. In every case below the distinct-comment set is identical (the comment is preserved on its source node and/or in the root `comments` array), `ast_diff` confirms semantic equivalence, and the formatter — which locates comments by position — is unaffected.

- **Module-script comment duplicated onto the instance script.** Svelte parses the `<script module>` and instance `<script>` against one shared `root.comments` array, and the instance parse's `add_comments` walk is not given a fresh queue, so every module-script comment (leading *or* trailing) is also shifted into the instance script's first statement (`instance.content.body[0].leadingComments`). tsv keeps each module comment only on the module body.
  - [module_comment_instance_duplication_svelte_divergence](../tests/fixtures/svelte/script/module_comment_instance_duplication_svelte_divergence/)

- **Block binding-pattern interior comment — node attachment + column offset.** Svelte parses the `{#each … as}` context and the `{#await … then}` / `{:then}` / `{:catch}` binding patterns with a separate acorn parse that (a) **attaches** an interior comment to its adjacent pattern node as `leadingComments` / `trailingComments`, and (b) for any such comment past the pattern's first line reports its `loc.column` **one too high** (an offset-translation slip in the context reparse — byte `start`/`end` are correct; the same context-reparse-loc family as the `each_as_stale_loc` correction above). tsv keeps each comment once in the root `comments` array, unattached, with the correct column. These fixtures also drop the comment in prettier-plugin-svelte, so they carry the `_svelte_prettier_divergence` suffix — see [conformance_prettier.md §Svelte: destructuring binding-pattern comments](./conformance_prettier.md#svelte-destructuring-binding-pattern-comments).
  - [each/destructure_comment_svelte_prettier_divergence](../tests/fixtures/svelte/blocks/each/destructure_comment_svelte_prettier_divergence/)
  - [await/destructure_comment_svelte_prettier_divergence](../tests/fixtures/svelte/blocks/await/destructure_comment_svelte_prettier_divergence/)

- **Leading HTML comment duplicated onto the instance script.** A leading fragment HTML comment (`<!-- @component … -->`) before a `<script module>` + instance `<script>` pair is attached to *both* the module Program and the instance Program. tsv attaches it once, to the nearest (module) script Program; the comment is also a `Comment` node in the fragment in both parsers, so nothing is lost. (With no module script there is a single instance Program and tsv matches Svelte — the divergence needs a second script root to be copied onto.)
  - [leading_html_comment_instance_duplication_svelte_divergence](../tests/fixtures/svelte/script/leading_html_comment_instance_duplication_svelte_divergence/)

- **Template-expression comment before a parenthesized subexpression.** Svelte's `parse_expression_at` sets acorn's `preserveParens: true`, so a leading comment before a parenthesized subexpression attaches to the synthetic `ParenthesizedExpression`; Svelte's subsequent `remove_parens` discards that wrapper and its `leadingComments`, leaving the comment only in the root `comments` array. tsv (which has no `ParenthesizedExpression` node, matching Svelte's *final* shape) attaches it to the inner expression. This is template-only — a plain `<script>` parse does not set `preserveParens`, so the same comment attaches in both parsers there. The common real-world trigger is a JSDoc cast `/** @type {T} */ (expr)`.
  - [template_expr_paren_comment_svelte_divergence](../tests/fixtures/svelte/syntax/comments/template_expr_paren_comment_svelte_divergence/) — precedence parens, isolating the parser difference
  - [jsdoc_cast_template_svelte_prettier_divergence](../tests/fixtures/svelte/syntax/comments/jsdoc_cast_template_svelte_prettier_divergence/) — the JSDoc-cast trigger across template / attribute / directive positions; also a `_prettier_divergence` (prettier strips the cast there)


### Known Acorn-TypeScript Bugs (Not Corrections)

These are bugs in **upstream/standalone `acorn-typescript`** — the non-fork npm
package, distinct from the `@sveltejs/acorn-typescript@1.0.11` fork this project
pins (`crates/tsv_debug/src/deno/sidecar.ts`) and that every other
"acorn-typescript" mention in this doc refers to. They **don't affect Svelte
users** (Svelte's fork handles them):

**Abstract methods break namespace export scope tracking** (upstream `acorn-typescript`, reported at 1.4.13): Abstract methods inside abstract classes corrupt the module scope, causing subsequent namespace imports to fail. Raw `.ts` parsing fails but `.svelte` files work fine. No fixture needed.

---

## Compat Behaviors

Implementation oddities in Svelte's parser that tsv replicates for AST compatibility. These are NOT in divergence directories—tsv matches Svelte exactly.

### CSS Compat Behaviors

- Backslash doubling in values — raw source extraction in `crates/tsv_css/src/ast/convert/mod.rs`
- Unicode escape first-digit duplication — raw source extraction in `crates/tsv_css/src/ast/convert/mod.rs`
- Comment-before-colon in declaration value — `crates/tsv_css/src/ast/convert/mod.rs`
- Block-comment stripping in declaration value — `strip_css_comments` in `crates/tsv_css/src/ast/convert/mod.rs`
- Block-comment stripping in at-rule prelude — `strip_css_comments` in `crates/tsv_css/src/ast/convert/mod.rs`
- ::slotted()/::part() span truncation — `crates/tsv_css/src/ast/convert/mod.rs`
- :dir()/:lang()/::highlight() identifier wrapping — `crates/tsv_css/src/ast/convert/mod.rs`
- Selector-name half-decoding (class/id/type, pseudo-class/element, **and** attribute names) — `raw_selector_name` in `crates/tsv_css/src/ast/convert/mod.rs`
- HTML comment (CDO/CDC) `<!-- ... -->` swallow at statement/selector-list boundaries — `skip_html_comment_markers` in `crates/tsv_css/src/parser/mod.rs`

Backslash doubling and unicode-escape duplication are inherited "for free" by extracting raw bytes (`source[span]`) into the public JSON value — Svelte's parser embeds those quirks in its span, so reproducing the bytes reproduces the quirks. No quirk-specific encoder runs.

**Selector-name half-decoding.** Svelte's `read_identifier` decodes a selector name only *half*-way: a **hex** escape (`\3A `, `\1F600`, with an optional single-whitespace terminator) decodes to its codepoint, but an **identity** escape (a backslash before a non-hex char — `\?`, `\@`, `:f\oo`) keeps the backslash. tsv's internal lexer fully decodes (the spec-canonical `<ident-token>` value, e.g. `:f\oo` → `foo`), so the public `name` is reconstructed half-decoded from the raw span by `raw_selector_name` for **every** selector kind — class/id/type, pseudo-class/element, and attribute. (For class/id/type and pseudo names the formatter already emitted the raw source from the span, so formatting was unaffected; **attribute** names additionally needed the formatter fixed — it had reconstructed the selector from the *decoded* `name`, so `[f\oo]` printed as `[foo]` and even `[\41 b]` as `[Ab]`, silently dropping escapes. The internal `Attribute` selector now carries a `name_span` (the name token within `[ns|name op 'value' flags]`); the printer emits it raw and convert half-decodes it, so escapes are preserved in output and the AST matches Svelte.) **Why match the half-form and not the spec:** the public AST's contract is byte-for-byte parity with Svelte's `parseCss` (tsv is a drop-in for it), so where Svelte's scan-based decode diverges from the CSS Syntax spec's full ident decode, tsv mirrors Svelte. Pinned by [css/selectors/escaped_names](../tests/fixtures/css/selectors/escaped_names/) (class/id/type identity escapes), [css/selectors/pseudo_escaped_identity](../tests/fixtures/css/selectors/pseudo_escaped_identity/) (pseudo identity escapes — `:f\oo` → `"f\\oo"`, never `"foo"`), and [css/selectors/attribute/escaped_identity](../tests/fixtures/css/selectors/attribute/escaped_identity/) (attribute names — both the AST half-decode and the formatter preserving the raw escape).

**Block-comment stripping**: the public `Declaration.value` and `Atrule.prelude` strings have `/* … */` comments removed in place (surrounding whitespace preserved) and the result trimmed. tsv applies this in `strip_css_comments` at the conversion boundary; the helper is string- and `url()`-aware so `/*` sequences inside `"…"`, `'…'`, or `url(…)` are kept verbatim.

**HTML comment (CDO/CDC) swallow.** The legacy `<!-- … -->` markers (CSS Syntax's CDO/CDC tokens, from the old `<style><!-- … --></style>` browser-hiding idiom) are read by Svelte's `parseCss` as a *comment span* at its `allow_comment_or_whitespace` boundaries — the stylesheet/block body (`read_body`) and the selector-list start / after a complete selector / after a comma (`read_selector_list`). It reads to the required `-->` and **discards everything between**, emitting no node. This departs from CSS Syntax 3, where `<!--` (CDO) and `-->` (CDC) are two *independent* no-op tokens and the content between them parses as ordinary CSS: per spec `<style><!-- h1 { color: red } --></style>` keeps the `h1` rule, but `parseCss` (and thus tsv) drops it, and the whole-stylesheet idiom `<!-- …rules… -->` parses to an **empty** stylesheet (so `format` deletes the wrapped CSS — matching Svelte's compiled output, where the rules are already dead). tsv matches `parseCss` via `skip_html_comment_markers` (`crates/tsv_css/src/parser/mod.rs`): the boundary skip discards the span (unterminated `-->` is an error, mirroring Svelte's `eat('-->', true)`); in **value** and **at-rule-prelude** position the markers are NOT special (those readers scan raw, so a `;`/`{` between them stays significant), and `<!--` between compounds (`h1 <!-- --> p`) is rejected — all matching `parseCss`. Pinned by [tests/css_cdo_cdc.rs](../tests/css_cdo_cdc.rs) and the svelte-fixtures gate (`css/samples/comment-html`); the formatter drop-on-format and prettier's invalid-CSS mangling are the `_prettier_divergence` at [css/tokens/html_comment_prettier_divergence](../tests/fixtures/css/tokens/html_comment_prettier_divergence/), cataloged in [conformance_prettier.md §CSS: HTML comments (CDO/CDC)](conformance_prettier.md#css-html-comments-cdocdc). **Residual** (a near-term, non-fixtured limit): a marker at the *start* of a **pseudo-argument** selector list — `:has(<!-- --> > img)` (rejects), `:is(<!-- --> .a)` (accepts, but with a divergent `Invalid`-selector shape) — and a marker interleaved with a `/* */` comment at a selector boundary are not matched. Both are deeply pathological (a legacy HTML comment inside a `:has()`/`:is()` argument) and reach neither the gate nor the corpus; normal rule selector lists match exactly.

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
  - `read_value()` (the `value += '\\' + char` escape branch) — backslash doubling

---

## Svelte Behavior Reference

Documentation of Svelte parser behavior (not compat behaviors or corrections).

### Directive Modifiers

Svelte's parser accepts `|modifier` syntax on all directive types (permissive parsing), but only three have official support:

- `OnDirective` — `on:event|mod` — `preventDefault`, `stopPropagation`, `stopImmediatePropagation`, `passive`, `nonpassive`, `capture`, `once`, `self`, `trusted`
- `TransitionDirective` — `transition:|mod`, `in:|mod`, `out:|mod` — `local`, `global`
- `StyleDirective` — `style:prop|mod` — `important`

Directives without official modifiers: `AnimateDirective`, `BindDirective`, `ClassDirective`, `LetDirective`, `UseDirective`.

**tsv behavior**: Every directive carries a `modifiers` array, and tsv preserves the modifier text **verbatim for all eight directive types** — matching Svelte's permissive runtime parser exactly, including unofficial modifiers on the five types whose published `.d.ts` declares none (`use:foo|bar` → `['bar']`, `on:click|preventDefault|bogus` → `['preventDefault', 'bogus']`, in both parsers). So this is **not** a `_svelte_divergence` — tsv's parser AST matches Svelte's. On **format**, the two formatters diverge for the five types without official support: prettier-plugin-svelte silently drops the `|mod` text, while tsv preserves it — a `_prettier_divergence` (content preservation), pinned by [modifier_preservation](../tests/fixtures/svelte/directives/modifier_preservation_prettier_divergence/). See [conformance_prettier.md §Svelte: Attributes](./conformance_prettier.md#svelte-attributes).

**Reference**: `svelte/packages/svelte/src/compiler/types/template.d.ts`

---

## Related

- ./conformance_prettier.md — Prettier formatter differences
- ./checklist_css.md — CSS feature matrix
- ./fixture_overview.md — Fixture system details
