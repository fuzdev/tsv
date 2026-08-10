# Svelte Conformance

The tsv parser aims for **exact AST compatibility** with Svelte's parser. This document catalogs tsv's compatibility behaviors and intentional corrections.

## Mental Model

**Matched**: tsv produces identical AST to Svelte (the goal). This includes replicating Svelte's quirky behaviors for tool compatibility.

**Unmatched**: tsv produces different AST. The suffix `_svelte_divergence` marks these fixtures. tsv differs when Svelte or acorn-typescript is wrong — a spec violation, a missing feature, or a bug tsv corrects (e.g. Svelte's comment glue duplicating a comment across `<script>` boundaries). One exception isn't a correction: a lone UTF-16 surrogate can't survive tsv's UTF-8 strings (→ U+FFFD), so tsv differs there despite acorn being right.

## Classification

- **Compat behavior** — Svelte has quirky but harmless behavior (design choices, tokenization quirks, output that doesn't affect semantics). tsv replicates it in AST output
- **Correction** — Svelte/acorn violates spec, corrupts semantics, or lacks a spec-defined feature (e.g. acorn omitting an anonymous class expression's `id` for implements-first heritage). tsv produces correct/complete AST
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
- Comments at combinator boundaries — Rejected (`css_expected_identifier`); tsv accepts them as inter-token trivia (CSS Syntax 3 — removed at tokenization, producing no token, not even whitespace) in every position — descendant/child/sibling gap (`div /* c */ p`), before/after an explicit combinator, glued between compound members (`.a/* c */.b`), and a `:has()` relative-selector leading combinator. tsv normalizes the gap spacing to a single space (prettier freezes it — a `_prettier_divergence`, see [conformance_prettier_css.md §CSS: Comments](conformance_prettier_css.md#css-comments)) — [combinator_comment](../tests/fixtures/css/selectors/combinator_comment_svelte_prettier_divergence/)
- Glued comment run in a compound — Rejected (`css_expected_identifier`); tsv keeps `.a/* c *//* d */.b` a compound (two adjacent glued comments are inter-token trivia, not a descendant) and emits the run verbatim. Prettier agrees it's a compound but relocates the `{` (a `_prettier_divergence`, see [conformance_prettier_css.md §CSS: Comments](conformance_prettier_css.md#css-comments)) — [compound_comment_run](../tests/fixtures/css/selectors/compound_comment_run_svelte_prettier_divergence/)
- Comments between `::part()` names — Rejected (`css_expected_identifier`); a comment in an interior gap (`::part(a /* c */ b)`) reads as whitespace and splits the identifier run in Svelte's scanner, while tsv accepts it as inter-token trivia (CSS Syntax 3) and normalizes the gap to a single space (prettier freezes it — a `_prettier_divergence`, see [conformance_prettier_css.md §CSS: Comments](conformance_prettier_css.md#css-comments)). The edge positions (before/after the run) are accepted by parseCss — see [part_comment](../tests/fixtures/css/selectors/pseudo_element/part_comment_prettier_divergence/) — [part_interior_comment](../tests/fixtures/css/selectors/pseudo_element/part_interior_comment_svelte_prettier_divergence/)
- Consecutive combinators (`> > .a`, `+ ~ .d`, glued `>>.a`) — parseCss **collapses** a run of combinators to its last: its `read_selector` never emits an empty relative selector, so on the second combinator it drops the earlier anchorless one. tsv **preserves** every authored combinator, emitting an empty-compound `RelativeSelector` per anchorless one (`+ ~ .d` → `[+, []]` then `[~, [.d]]`), so `expected_ours.json` carries relative selectors `expected_svelte.json` drops. The collapse is a lossy recovery tsv declines — the dropped combinator is authorship the future diagnostics layer needs, and in a relative context it silently *validates* the invalid selector (`:has(+ ~ .d)` → `:has(~ .d)`). Prettier also collapses (or freezes a glued run), so this is a `_prettier_divergence` too (see [conformance_prettier_css.md §CSS: Selectors](conformance_prettier_css.md#css-selectors)); a *trailing* combinator (`.a > > {}`) still rejects in both — [consecutive_combinator](../tests/fixtures/css/selectors/consecutive_combinator_svelte_prettier_divergence/)
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

### Entity Decoding Corrections

Svelte decodes character references with a generated regex over its entity table
(`1-parse/utils/html.js`), and its `validate_code` deliberately answers some codes
differently from [HTML5](https://html.spec.whatwg.org/multipage/parsing.html#character-reference-state).
Those deliberate answers are **matched**, quirks and all — NUL rather than U+FFFD for a
code with no character to emit (a surrogate half, or one past U+10FFFF), and `&#10;`
becoming a space. Five others are slips in the implementation rather than choices, so tsv
follows the spec — the first four pinned by
[spec_decoding](../tests/fixtures/svelte/syntax/entities/spec_decoding_svelte_divergence/),
whose README carries the per-case argument, the fifth by its own fixture, and every one of
them an upstream candidate:

- **Uppercase hex marker** — the numeric-character-reference state opens a hex reference
  on `U+0078 x` or `U+0058 X`; Svelte's pattern (`#(?:x[a-fA-F\d]+|\d+)(?:;)?`) spells only
  the lowercase one, so `&#X41;` stays literal text where tsv decodes it to `A`.
- **A zero code** — `if (!code) return match` guards the decode against an unknown or
  unparseable reference and catches a code of `0` as the other falsy value, so `&#0;` (any
  spelling) stays literal text. tsv decodes it, to NUL — the sentinel above, rather than the
  spec's U+FFFD, so that a zero code and a surrogate half keep the same answer.
- **An omitted plane** — the spec replaces only a surrogate half and a value past U+10FFFF;
  Svelte's `validate_code` enumerates the planes it will emit (0–2, plus two ranges of plane
  14) and drops the rest to NUL, destroying assigned characters — `&#x30000;` is CJK
  Extension G. The enumeration is a slip rather than a policy: plane 14 was *added* by
  [sveltejs/svelte#15823](https://github.com/sveltejs/svelte/pull/15823) after a user hit the
  hole. tsv emits every code point Unicode defines, keeping Svelte's NUL sentinel for the two
  it cannot.
- **The attribute-value boundary** — a semicolon-less reference is held literal only before
  `=` or an ASCII alphanumeric, but Svelte spells the test as JS's `\b(?!=)`, whose word
  class also holds `_` (its own comment beside the regex quotes the spec rule). So
  `<div a="&AMP_">` decodes to `&_` in tsv and stays literal in Svelte. The ASCII half of
  that class is not a divergence — both decoders leave `&AMP中` decoding, since `\b` is
  ASCII-only, and tsv's test is `is_ascii_alphanumeric`, never `char::is_alphanumeric`
  ([attributes/entity_no_semicolon_boundary](../tests/fixtures/svelte/attributes/entity_no_semicolon_boundary/)).
- **A two-code-point reference** — 93 of the 2,231 names in the
  [named character references table](https://html.spec.whatwg.org/entities.json) stand for
  two code points, and Svelte's table (`1-parse/utils/entities.js`) is generated with one
  per name, so the second — a combining mark, a variation selector, or a second character —
  is dropped. A negated relation then decodes to the relation itself: `&NotEqualTilde;`
  loses the combining solidus that negates it, leaving the U+2242 of `&esim;`.
  This is a slip in the generated data rather than a rule, so tsv emits both; the base
  characters, which have their own single-code-point references, are unaffected. Pinned by
  [multi_codepoint](../tests/fixtures/svelte/syntax/entities/multi_codepoint_svelte_divergence/).

### Static Attribute Reader Corrections

A **top-level** `<script>` / `<style>` head is the one place Svelte reads
attributes with `read_static_attribute` rather than `read_attribute`
(`1-parse/state/element.js`): no comments, no whitespace before `=`, no
directives, no `{expr}` — a name run plus a raw value, both taken by regex. tsv
reproduces that reader, including the shapes it admits that the element reader
rejects (`a=<b`, ``a=`b` ``, `a=b'c`, a lone or unbalanced brace in a quoted
value). One case is corrected:

- **Unterminated quoted value** — the value regex's third alternative
  (`[^>\s]+`) matches the run `"b` when neither quoted alternative does, and
  Svelte then decides it was quoted from `raw[0]` alone and strips a character
  off *each* end. `"b`.slice(1, -1) is empty, so `<script a="b>` keeps an empty
  value and the `b` is gone from the AST. The one-sided test is a slip: the
  value never terminated, so any reading loses source, and a formatter matching
  it would print `<script a=""></script>` and discard the author's bytes. tsv
  rejects (`Unterminated string literal in template`). Pinned by
  [static_attribute_unterminated_quote](../tests/fixtures/svelte/script/static_attribute_unterminated_quote_svelte_divergence/);
  the matched half of the reader is
  [static_attribute_grammar](../tests/fixtures/svelte/script/static_attribute_grammar/).

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

- `using` declarations (Explicit Resource Management — a finished/Stage 4 proposal, not ES2024; see [checklist_typescript.md](./checklist_typescript.md#explicit-resource-management)) — [basic](../tests/fixtures/typescript/typescript_specific/using/basic_svelte_divergence/)
- `await using` declarations — [await](../tests/fixtures/typescript/typescript_specific/using/await_svelte_divergence/); with a comment inside the keyword (`await /* c */ using`), which tsv preserves where prettier relocates it past `using` — [await_keyword_comment](../tests/fixtures/typescript/typescript_specific/using/await_keyword_comment_svelte_prettier_divergence/)
- `const` type params in classes — [const_type_param_class](../tests/fixtures/typescript/typescript_specific/generics/const_type_param_class_svelte_divergence/)
- `const` type params in interfaces, incl. `const` before variance (`<const in T>`) — [const_type_param_interface](../tests/fixtures/typescript/typescript_specific/generics/const_type_param_interface_svelte_divergence/). acorn rejects the `const` token (tsc defers it to the TS1277 checker error); the mis-ordered `<in const T>` is instead a *grammar* error tsv rejects like acorn, pinned by the regular fixture [type_param_modifier_order](../tests/fixtures/typescript/typescript_specific/generics/type_param_modifier_order/)
- Import type options — [dynamic_attributes](../tests/fixtures/typescript/modules/imports/dynamic_attributes_svelte_divergence/)
- An **invalid** v-flag regex (`/[a-z--[aeiou]]/v` — a range is not a `ClassSetOperand`, so V8 throws too): tsv accepts it by deferring the `IsValidRegularExpressionLiteral` early error, since regex bodies are opaque. Svelte is *correct* to reject; this is not a `v`-flag gap — [unicode_sets_advanced](../tests/fixtures/typescript/expressions/literals/regex/unicode_sets_advanced_svelte_divergence/)
- `export default class implements I {}` (anonymous default class, implements-first heritage) — [export_default_implements](../tests/fixtures/typescript/declarations/class/export_default_implements_svelte_divergence/)
- A **reversed** class-member modifier pair, `override abstract` — acorn rejects it outright (`'abstract' modifier must precede 'override' modifier`) and so does tsc (TS1029), while prettier parses it and reorders it to the canonical `abstract override`. tsv accepts and reorders identically, so the canonical spelling round-trips and the reversed one is silently corrected rather than refused. Pinned by the regular fixture [abstract_override](../tests/fixtures/typescript/declarations/class/abstract_override/): the canonical order as `input` (which acorn accepts, so `expected.json` matches), the reversed order as its `unformatted_reversed_modifier_order` variant, since an input tsv rewrites cannot be `input`. Flipping tsv to reject it is a reject-flip whose honest form is a modifier **loop** in `parse_class_member` — the shape tsc and acorn-typescript each use — not a third ordered probe
- A cast as the left operand of `**` (`x as number ** 2`) — see below; the rejection itself is not pinnable
- A `>=` following a `<` operand (`a < b >= d`) — see below — [relational_lt_greater_equal](../tests/fixtures/typescript/expressions/binary/relational_lt_greater_equal_svelte_divergence/)
- A **non-null assertion in a decorator expression** — `@x!`, `@x.y!`, `@x!.y`. The `!` is not in the TC39 decorator grammar and acorn rejects it, but tsc accepts all three with no diagnostic and prettier formats them, which is the accept test. It is the one member of the decorator grammar's deliberate exclusion list (binary operators, `[…]`, `?.`, tagged templates, `++`/`--`) that carries no argument of its own: the others stay unconsumed so the construct *after* the decorator can parse — the `*` of `@fn *a() {}` being the motivating case — and nothing valid after a decorator begins with `!`. tsv prints the parenthesized form prettier does, so there is no formatting divergence; the bare and doubly-parenthesized spellings ride as `unformatted_*` variants — [non_null_expression](../tests/fixtures/typescript/typescript_specific/decorators/non_null_expression/)
- **A reserved statement keyword as a type-reference name** — `let x: break;`, and every other reserved keyword prettier accepts there. TypeScript's type space is a separate namespace where a `TypeName` is an `IdentifierName`, so tsc and prettier both accept; acorn is over-strict in type position and rejects. It is a real `TSTypeReference`, not a special case — it takes a qualified name (`break.foo`), type arguments (`yield<T>`) and a union member (`A | default`). Keywords that head their own type production do NOT collapse to one (`void`/`null`/`string` stay `TSKeyword*`, `this` a `TSThisType`, `typeof x` a `TSTypeQuery`) — [keyword_type_reference](../tests/fixtures/typescript/types/keyword_type_reference_svelte_divergence/)
- **A function body inside a `declare namespace` / `declare module`** — `declare namespace N { function f() {} }`. The function carries no `declare` of its own, so it is grammatically an ordinary declaration with a body; the ambient-context violation is tsc's **TS1183**, a static-semantic early error tsv defers, and prettier formats it. acorn enforces TS1183 and rejects. A *bodiless* signature in the same position stays a `TSDeclareFunction` (not a divergence — acorn accepts it), and a **top-level** `declare function f() {}` still rejects on both sides, the `declare` keyword grammatically forcing a bodiless signature — [function_body](../tests/fixtures/typescript/declarations/namespace/function_body_svelte_divergence/)
- **Explicit type arguments on a decorator's trailing call** — `@g<number>()`, `@a.b<T>()`. A TypeScript extension to the TC39 grammar (`DecoratorMemberExpression Arguments`); tsc accepts both with no diagnostic and prettier formats them **unparenthesized**, since the callee is still the plain member chain the bare grammar allows — so tsv needs no printer rule for it. acorn abandons the decorator at the `<` and reports its generic "Leading decorators must be attached to a class declaration". A trailing call is required: `@g<number>` alone is a syntax error in tsc and prettier too (the `<` reads as relational), and tsv rejects it like `a?.<T>` with no call — [call_type_arguments](../tests/fixtures/typescript/typescript_specific/decorators/call_type_arguments_svelte_divergence/)
- **A TypeScript import-equals at `Goal::Script`** — `import x = A.B`, `import x = require('y')`, `import await = foo.await`. Not an ES `ImportDeclaration` and so not a `ModuleItem`: it predates ES modules and is how a script or namespace aliases, which is why tsv's goal gate fires at the two `ImportDeclaration` construction sites rather than on the `import` keyword. tsc asserts the shape in `conformance/externalModules/topLevelAwait.2.ts` (commented *"await allowed in import=namespace when not a module"*, no `.errors.txt`). acorn's rejection is base acorn's ES-grammar check firing before the TS plugin sees the statement — a slip, not a judgement. Every genuine ES import shape still rejects at that goal — [import_equals](../tests/fixtures/typescript/script_goal/import_equals_svelte_divergence/)
- A **non-simple assignment target** — a call (`foo() = bar`, `foo() += 1`), a literal (`1 >>= 2`), or `this` (`this = x`). The production is `LeftHandSideExpression = AssignmentExpression`; the "is it assignable?" refinement (`AssignmentTargetType`) is an early error layered on top, which tsv defers, so all four parse and prettier formats all four. acorn enforces it (`Assigning to rvalue`). The deferral does **not** reach a no-declaration `for`-in/of head — that is a `LeftHandSideExpression` position but not an assignment context, so a non-simple target there stays a parse error in tsv as in prettier — [nonsimple_target](../tests/fixtures/typescript/expressions/assignment/nonsimple_target_svelte_divergence/)
- A **shorthand property carrying an initializer** in an object *literal* — `({ a = 1 })`. `PropertyDefinition : CoverInitializedName` is a real production, present so `ObjectLiteral` can cover `ObjectAssignmentPattern`; the rejection is the early error layered on top ("It is a Syntax Error if any source text is matched by this production", [§13.2.5.1](https://tc39.es/ecma262/#sec-object-initializer-static-semantics-early-errors)), which tsv defers, so it parses as a `shorthand` `Property` whose `value` is an `AssignmentExpression`. acorn enforces it (`Shorthand property assignments are valid only in destructuring patterns`). Every *valid* spelling is refined to an `ObjectPattern` before it is printed, so the literal shape is reachable only here — which is why the property's comment seam had never been asked about it — [shorthand_initializer_name_comment](../tests/fixtures/typescript/expressions/objects/shorthand_initializer_name_comment_svelte_divergence/)

**A strict-mode-reserved word as a name** ([strict_reserved_name](../tests/fixtures/typescript/declarations/variable/strict_reserved_name_svelte_divergence/); the load-bearing parens that follow from it are [statement_head_paren](../tests/fixtures/typescript/statements/expression/statement_head_paren_svelte_divergence/)) — `implements`, `interface`, `let`, `package`, `private`, `protected`, `public`, `static`, `yield` are barred as names by a *single* bullet of ecma262 §sec-identifiers-static-semantics-early-errors, a Static Semantics early error tsv defers. So tsv parses all nine as names in every position — `var let = 1`, `function f(yield) {}`, `class implements {}`, `function f(private) {}`, `enum yield {}`, `private: for (;;) break private;`, `type T = X extends Y ? infer let : never` — as tsc's parser and prettier do, while acorn enforces the early error and rejects. Most of the list was always accepted, because tsv's lexer leaves those words as plain `Identifier`s; the holes were `let`/`yield` (keyword-lexed) and `implements`/`private`/`protected`/`public` (swallowed by a competing syntactic role), and both are artifacts of tokenization and lookahead rather than rules.

The competing roles are resolved with tsc's own one-token lookaheads, so the word's real role still wins where it should. After `class`, `implements` opens a heritage clause iff an identifier-or-keyword follows (tsc's `isImplementsClause`), so `class implements {}` names the class while `export default class implements I {}` stays an anonymous class with heritage. In a parameter list, an accessibility keyword is a modifier iff a binding follows it on the same line (tsc's `canFollowModifier`) — the rule `readonly`/`override` already used — so `class C { constructor(private x) {} }` is a parameter property and `function f(private) {}` is a parameter *named* `private`. tsv deliberately does **not** copy tsc's error *recovery*: `class implements extends B {}` leaves the declaration nameless and stays rejected, which is also prettier's verdict.

`let` and `yield` in detail: Neither word is excluded by a *production*: `let` is not a `ReservedWord` at all, and `BindingIdentifier[Yield, Await] : Identifier | `yield` | `await`` admits `yield` unconditionally — ecma262 §sec-identifiers writes the `[Yield]`/`[Await]` bars as early errors rather than production guards, with a note explaining why (so ASI cannot split `let ⏎ await 0;`). What remains is the strict-mode bullet of §sec-identifiers-static-semantics-early-errors, which tsv defers — the *same* bullet, and the same deferral, that already made `implements` / `interface` / `package` / `private` / `static` binding names, those being words tsv's lexer never keyword-izes. Widening is what makes the seven consistent.

**Three channels, and the spec draws the lines, not tsv.** Whether a context bar is deferrable turns on how ecma262 wrote it:

| channel | production | `yield` in a generator / `await` in an async fn |
| --- | --- | --- |
| `BindingIdentifier` | `Identifier \| yield \| await` — no guard | **admitted**, early error deferred |
| `IdentifierReference` | `Identifier \| [~Yield] yield \| [~Await] await` | **barred** by the guard |
| `LabelIdentifier` | identical to `IdentifierReference` | **barred** by the guard |

So `function* g() { var yield = 1; }` and `async function h() { var await = 1; }` (Script goal) parse, while `{ yield }`, `yield: ;`, `{ await }` and `await: ;` reject inside those same functions. A guard is not a deferrable early error: in a `[+Yield]` / `[+Await]` context the word is the **operator**, so the name reading is unreachable rather than merely invalid. tsv reads the two reference-shaped heads it checks before committing — a heritage element's `TypeName` and an import-equals module reference — through the guarded predicate for that reason, which is where tsc lands too by parsing heritage with its *expression* parser (`function* g() { interface A extends yield {} }` is TS1109). A plain type annotation reaches no expression parser in either implementation and is unaffected (`function* g() { let x: yield; }` parses).

One case shares the verdict but not the rule: `function* g() { var f = yield => 1; }` rejects, yet an arrow parameter is `ArrowParameters : BindingIdentifier[?Yield]`, which carries **no** guard — grammatically that *is* an arrow, killed by the very early error tsv defers in `var yield = 1`. What rejects it is commit order: at an expression start in a generator, `yield` commits to a `YieldExpression` before the arrow reading is reachable. acorn, tsc and prettier all commit the same way.

**`await` carries a second, independent bar** — the **goal** bullet (a Syntax Error as a name when the goal is `Module`) — and tsv enforces that one in every channel, which is what makes `Goal::Script` observable at all. It is orthogonal to the `[Await]` parameter above: `var await = 1` rejects at Module goal and parses at Script goal, in or out of an async function.

**`let` and `yield` as `IdentifierReference`s.** Both are ordinary references, so `let;`, `x = let`, `let.x = 1`, `typeof let`, `let++`, `new let()`, and `yield()`, `yield++`, `new yield()`, `class C extends yield {}` all parse, as tsc and prettier parse them. For `yield` this is a *shape* fix as much as an acceptance one: an unconditional yield-expression reading accepted most of these already but built a `YieldExpression` for them, so `yield.foo` emitted a `MemberExpression` over a node the enclosing non-generator function cannot legally contain. Both now emit a plain `Identifier`.

What separates `let`'s two readings is a lookahead, not a rule: statement-initial `let` heads a declaration exactly when a binding name, `{` or `[` follows (tsc's `isLetDeclaration`). The one grammatical bar is `ExpressionStatement`'s `[lookahead ∉ { …, `let` `[` }]`, so `let [` can never begin an expression statement — `let[0] = 1` stays a syntax error (a declaration with an invalid array binding pattern, tsc TS1181) even though `let.x = 1` parses. A for-head asks no lookahead at all and commits on the keyword, exactly as tsc does, so `for (let[0] of a)` and `for (let.x of a)` also stay rejected.

There is no fixture for the strict-reserved words themselves, because acorn rejects every case and no `expected.json` oracle is producible; the shapes are pinned by [`tests/strict_reserved_word_as_name.rs`](../tests/strict_reserved_word_as_name.rs), several of whose assertions check a **node type** rather than an accept, since an over-permissive parser can take a widened word and still build the wrong node for it.

Binding-pattern **elements** follow from the reference widening rather than needing their own rule: tsv parses a binding pattern by running the expression parser and converting (`parse_destructured_binding` → `to_assignable`), so an element head asks the `IdentifierReference` channel. Widening that channel closed a declaration (`var [let] = a`, `var {a: let} = o`), a parameter (`function f([let]) {}`) and a `catch` binding (`try {} catch ([let]) {}`) in one move; object shorthand always worked, having its own keyword arm. A `void` element still rejects — a production bar, not a deferred early error.

**A contextual keyword as a label or an `infer` name** — the two channels the same work reached, and here acorn agrees, so they are ordinary fixtures. A `LabelIdentifier` and an `infer` type-parameter name are both plain names, but tsv reached each through a `TokenKind::Identifier` test that only saw the words its lexer never keyword-izes: `async: for (;;) break async;`, `string: while (0) continue string;` and `type A<T> = T extends Array<infer string> ? string : never` all rejected while `foo:` / `infer U` worked. Both now use the shared name channel, so a label can be declared *and* referenced with any of these words. Pinned by [labeled/contextual_keyword_name](../tests/fixtures/typescript/statements/labeled/contextual_keyword_name/) and [infer/contextual_keyword_name](../tests/fixtures/typescript/types/infer/contextual_keyword_name/). What may be *labelled* is unchanged: `LabelledItem : Statement | FunctionDeclaration`, and the `FunctionDeclaration` arm carries the "It is a Syntax Error if any source text is matched by this production" phrasing (browser carve-out for non-strict code only, and tsv is strict-only), so `lbl: function f() {}` and `label: let x = 1` both still reject.

**Reserved word in a heritage clause** — the one entry here where tsv is *stricter* than acorn, and the one place all three oracles disagree in three different directions. A heritage element is a type **reference** (`TypeReference: TypeName`, `TypeName: IdentifierReference | NamespaceName . IdentifierReference`), so a reserved word can never head one. tsv follows that grammar, which is exactly prettier's line — its error states the rule outright ("Interface declaration can only extend an identifier/qualified name with optional type arguments") and tsv matches prettier on every element form tested:

| heritage element | tsc parser | acorn | prettier | tsv |
| --- | --- | --- | --- | --- |
| `A` `number` `string` `any` `undefined` `A.B` `A<T>` | accept | accept | accept | **accept** |
| `void` | **TS1109** | accept | reject | **reject** |
| `null` `true` `this` | accept | accept | **reject** | **reject** |
| `super` | **TS1034** | **accept** | reject | **reject** |
| `1` `'s'` `(A)` `typeof A` `[A]` `A[]` `{a: 1}` | mostly accept | reject | reject | **reject** |

The other two are lenient for structural reasons, not by decision: acorn reads the heritage name as a bare `IdentifierName`, so every reserved word slips through, while tsc parses heritage with its *expression* parser and defers primitive-ness to the checker — which is why literals and parenthesized expressions get in, and why `void` and `super` (not left-hand-side expressions either) are the two it still rejects. Per the oracle note above, "tsc's parser accepts" means only *not a grammar error*. Rejection pinned by [heritage_reserved_keyword](../tests/fixtures/typescript/types/interfaces/heritage_reserved_keyword_svelte_divergence/); the **contextual** type keywords are ordinary identifiers and are accepted, pinned by [heritage_type_keyword](../tests/fixtures/typescript/types/interfaces/heritage_type_keyword/). Same reserved-vs-contextual line as the qualified-name **head** below; the qualified **tail** after a `.` is the opposite case (a full `IdentifierName`, reserved words admitted — [reserved_keyword_qualified_tail](../tests/fixtures/typescript/types/reserved_keyword_qualified_tail/)).

The rule is `ReservedWord`-shaped, not keyword-shaped, so `let` and `yield` land on the accepting side with the contextual type keywords, barred only by the strict-mode early error tsv defers — pinned by [heritage_let](../tests/fixtures/typescript/types/interfaces/heritage_let/) and [heritage_yield](../tests/fixtures/typescript/types/interfaces/heritage_yield/), and consistent with tsv reading both as ordinary type names in every other type position (`let x: let`, `type T = yield.Foo`, `typeof let`).

The head is an `IdentifierReference` in the full sense, though, so it takes the `[~Yield]` / `[~Await]` **production guards** with it: `function* g() { interface A extends yield {} }` and `async function h() { interface A extends await {} }` reject, exactly as tsc rejects them (TS1109 — it reaches the same place by parsing heritage with its *expression* parser, where both words are the operator). A plain type annotation reaches no expression parser in either implementation and is unaffected (`function* g() { let x: yield; }` parses). `await` carries the extra **goal** bullet on top, which tsv enforces rather than defers, so `interface A extends await {}` rejects at `Goal::Module` and is accepted at `Goal::Script` like the other two. Prettier accepts `await` under both goals — the one spot where its heritage line is looser than the rule its own error message states, and the one spot tsv declines to follow it.

**`using` keyword-name comments**: tsv **accepts** a comment between `using` and the binding name (`using /* c */ x = fn()`) and round-trips it, which is correct — per ecma262 §sec-comments a comment "behave[s] like white space and [is] discarded", so any two tokens may be separated by one. A comment *containing a line terminator* is the exception the same clause names: it counts as a `LineTerminator`, which the `[no LineTerminator here]` in `await [no LT] using` and `using [no LT] BindingIdentifier` then demotes — so `await /* c⏎ */ using x = fn()` correctly fails to read as a declaration. acorn's verdict is not comparable here: it rejects `using` / `await using` outright (see the list above), so it never reaches the comment question.

**`using` line-break demotion, and where it lands**: the same two `[no LineTerminator here]` restrictions have opposite *consequences* by position. In a **statement** the demotion is graceful — ASI splits `using⏎x = 1` into two statements (an expression statement and an assignment), and likewise `await using⏎x = 1` — but a **for head** has no ASI, so the demoted `using` / `await using` expression leaves a head no `of` can continue and the whole file is a syntax error. Both are pinned by [line_break_demotion](../tests/fixtures/typescript/typescript_specific/using/line_break_demotion/): the statement forms as the fixture's `unformatted_line_break` variant (prettier restores the `;`, so the trigger cannot live in `input.*`), the head forms as seven `input_invalid_*` files covering each gap under a raw break and under a comment-borne one.

**tsc is the outlier at the `await`→`using` gap, and tsv does not follow it.** tsc enforces the second restriction only (`nextTokenIsUsingKeywordThenBindingIdentifierOrStartOfObjectDestructuringOnSameLine` checks `hasPrecedingLineBreak` before the *binding* and never before `using`), so it — and prettier's `typescript` parser with it — reads `await⏎using x = 1` and `for await (await⏎using x of items)` as declarations. Babel (`babel-ts`) and oxc reject both, matching the grammar the cover production `await [no LT] using` exists to disambiguate; tsv rejects too. This is a **slip, not a choice**: the same predicate spells the restriction for one gap and drops it for the other. It is the one place in the `using` family where tsv is deliberately stricter than tsc.

**`>=` after a `<` operand**: acorn-typescript rejects `a < b >= d` and `a < B[c] >= d` (`Assigning to rvalue`). It rescans the `>=` into `>` + `=`, closes a type-argument list on the `>`, and reads the `=` as an assignment to the resulting instantiation expression. tsc does not: its `canFollowTypeArgumentsInExpression` disqualifies the type-argument reading here, so the `<` stays relational and the `>=` stays one operator. Prettier agrees — it reprints the unspaced `a<b>=d` as `a < b >= d`, which it would not do for an instantiation — and tsv matches tsc and prettier. Both operand forms are pinned because they take different paths through tsv's type-argument lookahead (bare identifier vs indexed member access). **Upstream candidate**: acorn-typescript `>=` rescan at a type-argument close.

**Cast as the left operand of `**`**: acorn-typescript rejects `x as number ** 2` / `x satisfies number ** 2` (`Unexpected token` at the `**`). tsc accepts both — it parses the cast as the `**` left operand, and its "unary expression is not allowed in the left-hand side of an exponentiation expression" grammar error (TS17006) fires only for a *prefix-unary* operand (`-2 ** 3`), not for a cast. Prettier agrees, printing `(x as number) ** 2`, and tsv matches. **Upstream candidate**: acorn-typescript exponentiation after `as`/`satisfies`.

The rejection is the one case here that **cannot be pinned**. The `expected_svelte.json` = `{"error": "failed to parse"}` sentinel every fixture above uses attaches to `input.*`, and an `input.*` must be a formatting fixed point (F1) — `x as number ** 2` is not one, since both formatters normalize it to `(x as number) ** 2`. The source form can therefore only live in an `unformatted_*` variant, and the validator runs the canonical parser over `input.*` and `input_invalid_*` only, never over variants. So [as_satisfies_exponentiation](../tests/fixtures/typescript/expressions/as_satisfies_exponentiation/) is a *regular* fixture: it pins the parse shape and the paren insertion (both operand sides — a cast on the right needs parens too, since `as` otherwise binds looser and takes the whole exponentiation), and its `unformatted_no_parens` variant carries the source form. That variant formats at all only because prettier-plugin-svelte re-parses `<script>` content with prettier's own TypeScript parser rather than with Svelte's — Svelte's parser sees the fixture's parenthesized `input.svelte` and is happy.

**Async generic arrow param decorator**: a parameter decorator is invalid on an arrow function in every form, and prettier rejects all four spellings — `(@dec a) => a`, `<T>(@dec a) => a`, `async (@dec a) => a`, `async <T>(@dec a) => a`. tsv rejects all four too, uniformly. ⚠️ **tsc's parser is not what draws that line**: it raises a *parse* diagnostic only on the two non-generic forms (TS1109 `Expression expected.`) and accepts both generic forms outright — `parseDiagnostics` and the syntactic pass are empty on each. The familiar `Decorators are not valid here` is **TS1206, a semantic diagnostic** from the checker's grammar pass, which is why prettier (which runs those checks) surfaces it where tsc's parser does not; a TS1xxx code is not by itself evidence of a parser rejection. So tsv's rejection rests on prettier plus the kind of error TS1206 is — *unconditional-local*, invalid in every context, the bucket tsv rejects rather than defers (see [§Strict Mode Only](../CLAUDE.md#strict-mode-only)). What needs a fixture is acorn's split: it rejects three forms (`Leading decorators must be attached to a class declaration` on the non-generic ones, `Unexpected token` on the plain generic one) and accepts `async <T>(@dec a) => a` alone, because that form takes a separate path through its arrow parsing where the decorator check every other arrow form applies is never reached. Because the canonical parser accepts, this is pinned from the other side, by a `tsv_rejects.txt` fixture: [async_generic/param_decorator](../tests/fixtures/typescript/expressions/arrow/async_generic/param_decorator_svelte_divergence/); the drop-in rejections it contrasts with are the `input_invalid_*` cases in [decorators/parameter_arrow](../tests/fixtures/typescript/typescript_specific/decorators/parameter_arrow/). **Upstream candidate**: acorn-typescript — the async-generic arrow path should reject a parameter decorator like every other arrow form does.

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

**Decorated class modifier line break**: a class modifier keyword carries a `[no LineTerminator here]` restriction, so `declare` / `abstract` bind to the `class` head only on the same line. Undecorated, all three parsers agree that `declare⏎class A {}` is two statements. Behind a **decorator** the input has no valid reading — the decorator is left with no declaration to attach to, and tsc raises **TS1146 "Declaration expected"**. tsv rejects with it. acorn-typescript accepts, building a degenerate tree: an `ExpressionStatement` for the bare modifier plus a `ClassDeclaration` whose span runs *back over* that statement to the decorator, so two siblings overlap. Matching a self-overlapping tree is worse than rejecting. Pinned by the `tsv_rejects.txt` fixture [decorators/declare_line_break](../tests/fixtures/typescript/typescript_specific/decorators/declare_line_break_svelte_divergence/) (`@d⏎declare⏎class A {}`; `@d abstract⏎class B {}` is the same shape); the forms where acorn agrees there is no parse are ordinary `input_invalid_*` files in [decorators/declare](../tests/fixtures/typescript/typescript_specific/decorators/declare/). **Upstream candidate**: acorn-typescript — `canHaveLeadingDecorator`'s `isDeclareClass` / `isAbstractClass` lookaheads skip line terminators, admitting a decorator in front of a modifier that then fails to bind.

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

**Line break before a tuple element's `?` (`[T` ⏎ `?]`, rejected)**: the postfix
optional `?` of a tuple element is a `[no LineTerminator here]` position. tsc runs
its whole postfix suffix loop — `?`, `!` and `[` alike — under
`while (!scanner.hasPrecedingLineBreak())` (`parsePostfixTypeOrHigher`), so the
element ends at `T` and the stray `?` fails (`',' expected`); oxc rejects it the
same way. acorn-typescript accepts it: its `tsParseTupleElementType` bare-`eat`s
the `?` while spelling the guard for the array suffix one function below
(`tsParseArrayTypeOrHigher`), and babel — which it ports — has the same asymmetry,
so this is a slip rather than a choice. tsv applies the guard at both, which is
also what makes its own postfix family consistent (it already guards `B` ⏎ `[]`
and the type-argument sites above). Per
[ecma262 §sec-comments](https://tc39.es/ecma262/#sec-comments) a block comment
holding a line terminator *is* one, so the comment-borne authorings `[T // c` ⏎
`?]` and `[T /* c` ⏎ `*/?]` reject on the same rule. The **named**-member marker
is a different grammar position and does take the break (`[a` ⏎ `?: T]` — tsc
reads it through `parseOptionalToken`, outside that loop); tsv accepts it. Since
acorn accepts, the rejection is pinned by the
[tuple_optional_marker_line_break](../tests/fixtures/typescript/types/tuple_optional_marker_line_break_svelte_divergence/)
`tsv_rejects.txt` fixture, with the comment-borne triggers and the two accept
controls in
[tuple_optional_marker_line_break.rs](../tests/tuple_optional_marker_line_break.rs)
(one fixture carries one expected-error substring, and the accept rows have no
divergence to record). This is deliberate tsc-over-acorn strictness, the same
reverse direction as the two entries above. **Upstream candidate**:
acorn-typescript — `tsParseTupleElementType` omits the `hasPrecedingLineBreak`
guard on the optional `?`.

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

The **source-phase imports** and **import defer** proposals — not yet standard — add a
phase to both static and dynamic imports:

- `import source x from 'mod'` / `import.source('mod')` — phase `'source'` — [source_phase](../tests/fixtures/typescript/modules/imports/source_phase_svelte_prettier_divergence/), which pins BOTH oracles failing in one fixture: `expected_svelte.json` for acorn's rejection, `prettier_rejects.txt` for prettier's throw (`'=' expected.` — it reads `source` as an ordinary default binding)
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

⚠️ **This paragraph used to claim "No `_svelte_divergence` fixture — the fixture
pipeline needs acorn to produce `expected.json`, and acorn rejects the syntax." That is
WRONG, and the belief was load-bearing:** a canonical-parser rejection IS representable —
`expected_ours.json` + an `expected_svelte.json` holding `{"error": "failed to parse"}`,
in a `_svelte_divergence` dir (23 fixtures already do this;
[fixture_overview.md](./fixture_overview.md) states it). Because the belief said no
fixture was possible, none was written, and the prettier-side claim next door
(`import defer` phase drop) went stale unnoticed for want of an `output_prettier.*` to
regenerate. `import defer`'s comment handling is now fixtured as
[phase_keyword_comment](../tests/fixtures/typescript/modules/imports/phase_keyword_comment_svelte_prettier_divergence/);
the rest of the syntax could be too.

The parser is additionally graded by the
test262 suite — ~396 graded files, all passing; see
[conformance_test262.md](./conformance_test262.md). Prettier throws on `import source`
(it no longer drops `import defer`'s phase), so the remaining *printer* round-trips are
covered by `tests/import_phase.rs`; the prettier side is cataloged in
[conformance_prettier_ts.md](./conformance_prettier_ts.md#import-phase-proposals).
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

**Lone surrogates in string values** (`lone_surrogate_value`): an **unpaired** UTF-16
surrogate decodes to U+FFFD in tsv — Rust strings are UTF-8 and cannot represent
WTF-16 lone surrogates — where acorn keeps the lone surrogate in the JS string value.
Both escape spellings agree (`'\ud800'` and `'\u{D800}'`), and `raw` is a source
slice, so the printed output is exact. This is a representation limit, not a parse
difference.

⚠️ **Only an UNPAIRED half.** A lead escape followed by a trail escape denotes one code
point, and the pairing is a property of the code units rather than of how each half was
spelled — so `'\uD83D\uDE00'`, `'\u{D83D}\u{DE00}'` and the two mixed forms all
decode to `😀` exactly as acorn does, and none of them reach this divergence. That value
IS representable, so a spelling that came out as two U+FFFDs would be a plain bug rather
than this sanctioned limit — which is what the four spellings are pinned for in
[unicode_lone_surrogate](../tests/fixtures/typescript/expressions/literals/string/escapes/unicode_lone_surrogate/).

⚠️ **The divergence is real but deliberately UNPINNABLE by a fixture.** An
`expected.json` is captured through the debug sidecar, and a lone surrogate
cannot cross that boundary: `JSON.stringify` emits it, but the resulting document
is not encodable text and `serde_json` rejects it outright ("lone leading
surrogate"), so the whole response fails rather than the one value differing. The
sidecar therefore substitutes U+FFFD *in the canonical capture* — matching what
tsv's lexer already emits — which makes both sides of
[unicode_lone_surrogate](../tests/fixtures/typescript/expressions/literals/string/escapes/unicode_lone_surrogate/)
agree, so it is an ordinary fixture rather than a `_svelte_divergence` one. What
that fixture pins is the *parse and print* of both spellings, not the value gap.
The value gap is held by the **corpus** detector named above instead, whose
canonical AST never crosses a Rust boundary and so keeps acorn's true value —
see the ⚠️ note on `bigint_replacer` in `benches/js/corpus_compare_parse.ts`,
which exists to keep it that way.

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

- Async generic arrow param decorator — `async <T>(@dec a) => a` accepted, where every other arrow form correctly rejects
- Decorated class modifier line break — `canHaveLeadingDecorator`'s `isDeclareClass` / `isAbstractClass` lookaheads skip line terminators, so `@dec⏎declare⏎class A {}` admits a decorator whose modifier then fails to bind, yielding two overlapping sibling nodes
- `using` / `await using` — Explicit Resource Management declarations not recognized
- `import defer` / `import source` — import-phase declarations not recognized (`Unexpected token` at the phase keyword). tsv parses both; prettier accepts `defer` and rejects `source`, so only the `defer` form has a formatting oracle — [phase_keyword_comment](../tests/fixtures/typescript/modules/imports/phase_keyword_comment_svelte_prettier_divergence/)
- `const` type params — `const` modifier on class type params
- Import type options — `import()` type assertion options
- Anonymous class-expression `id` — omitted for implements-first heritage
- `export default class implements I {}` — anonymous default class with implements-first heritage rejected (`implements` read as a reserved-word name)
- Type assertion vs. generic arrow — `<T>` before any arrow (even a parenthesized one) reads as type parameters; the parenthesized-arrow abort check is dead code

(No **acorn core** candidates. The `v`-flag regex entry that used to sit here was withdrawn: acorn
supports the `v` flag and its set operations, and correctly rejects the one construct
[unicode_sets_advanced](../tests/fixtures/typescript/expressions/literals/regex/unicode_sets_advanced_svelte_divergence/)
exercises — `/[a-z--[aeiou]]/v` is invalid ECMAScript, which V8 throws on too. That fixture is not
an upstream bug at all; it is tsv deferring the `IsValidRegularExpressionLiteral` early error
because regex bodies are opaque, so it does not meet this section's bar and will become an
`input_invalid_*` fixture when the diagnostics layer lands.)

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

- **Block binding-pattern interior comment — node attachment + column offset.** Svelte parses the `{#each … as}` context and the `{#await … then}` / `{:then}` / `{:catch}` binding patterns with a separate acorn parse that (a) **attaches** an interior comment to its adjacent pattern node as `leadingComments` / `trailingComments`, and (b) for any such comment past the pattern's first line reports its `loc.column` **one too high** (an offset-translation slip in the context reparse — byte `start`/`end` are correct; the same context-reparse-loc family as the `each_as_stale_loc` correction above). tsv keeps each comment once in the root `comments` array, unattached, with the correct column. These fixtures also drop the comment in prettier-plugin-svelte, so they carry the `_svelte_prettier_divergence` suffix — see [conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](./conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments).
  - [each/destructure_comment_svelte_prettier_divergence](../tests/fixtures/svelte/blocks/each/destructure_comment_svelte_prettier_divergence/)
  - [await/destructure_comment_svelte_prettier_divergence](../tests/fixtures/svelte/blocks/await/destructure_comment_svelte_prettier_divergence/)
  - The binding's **type annotation** (`as x: /* c */ T`) is the same region seen one token later, and takes the same verdict — attached by Svelte, unattached by tsv — with one difference in tsv's favour: the column shift stops at the bare pattern's end, because canonical hands the annotation to `read_type_annotation`, a separate parse that prefixes `_ as ` and preserves every column. So an annotation comment keeps its true column where a destructure-interior one is bumped. [each/context_annotation_comment_svelte_prettier_divergence](../tests/fixtures/svelte/blocks/each/context_annotation_comment_svelte_prettier_divergence/)

- **`{@const}` with a type annotation duplicates every comment from the `:` to the tag close.** Svelte's `read_type_annotation` tricks acorn into parsing the annotation by building `_ as <annotation> = <init>`; that parse is an `AssignmentExpression`, so the reader's own "gets mangled — fix it" branch **re-parses** the slice up to the `=`. The first parse is discarded, but its `onComment` has already pushed everything it scanned into the shared `root.comments`, and the two real parses then push their own copies — order [pass 1: all, pass 2: annotation region, pass 3: init region]. `add_comments` re-filters the *whole accumulated* array rather than its own parse's pushes, so the duplicates are attached as well. The trigger is the annotation's **presence**, not a comment's position: an *init* comment is doubled too when the binding carries an annotation, and listed once when it does not. tsv parses the annotation as part of the binding, once, so each comment exists once and attaches once; the formatter matches prettier on every shape.
  - [const_annotation_comment_svelte_divergence](../tests/fixtures/svelte/tags/const/const_annotation_comment_svelte_divergence/)

- **Leading HTML comment duplicated onto the instance script.** A leading fragment HTML comment (`<!-- @component … -->`) before a `<script module>` + instance `<script>` pair is attached to *both* the module Program and the instance Program. tsv attaches it once, to the nearest (module) script Program; the comment is also a `Comment` node in the fragment in both parsers, so nothing is lost. (With no module script there is a single instance Program and tsv matches Svelte — the divergence needs a second script root to be copied onto.)
  - [leading_html_comment_instance_duplication_svelte_divergence](../tests/fixtures/svelte/script/leading_html_comment_instance_duplication_svelte_divergence/)

- **Template-expression comment before a parenthesized subexpression.** Svelte's `parse_expression_at` sets acorn's `preserveParens: true`, so a leading comment before a parenthesized subexpression attaches to the synthetic `ParenthesizedExpression`; Svelte's subsequent `remove_parens` discards that wrapper and its `leadingComments`, leaving the comment only in the root `comments` array. tsv (which has no `ParenthesizedExpression` node, matching Svelte's *final* shape) attaches it to the inner expression. This is template-only — a plain `<script>` parse does not set `preserveParens`, so the same comment attaches in both parsers there. The common real-world trigger is a JSDoc cast `/** @type {T} */ (expr)`.
  - [template_expr_paren_comment_svelte_divergence](../tests/fixtures/svelte/syntax/comments/template_expr_paren_comment_svelte_divergence/) — precedence parens, isolating the parser difference
  - [jsdoc_cast_template_svelte_prettier_divergence](../tests/fixtures/svelte/syntax/comments/jsdoc_cast_template_svelte_prettier_divergence/) — the JSDoc-cast trigger across template / attribute / directive positions; also a `_prettier_divergence` (prettier strips the cast there)
  - [assignment_prettier_ignore_head_svelte_prettier_divergence](../tests/fixtures/svelte/tags/assignment_prettier_ignore_head_svelte_prettier_divergence/) — an own-line directive freezing an assignment head. Unavoidable rather than incidental: tsv's canonical output for that construct *is* a directive leading a parenthesized expression (the clarity parens), so every fixture for the rule carries this divergence. Also a `_prettier_divergence` (prettier deletes the directive there — the same `remove_parens` pass, seen from the formatter side)


### Known Acorn-TypeScript Bugs (Not Corrections)

These are bugs in **upstream/standalone `acorn-typescript`** — the non-fork npm
package, distinct from the `@sveltejs/acorn-typescript` fork this project
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

**HTML comment (CDO/CDC) swallow.** The legacy `<!-- … -->` markers (CSS Syntax's CDO/CDC tokens, from the old `<style><!-- … --></style>` browser-hiding idiom) are read by Svelte's `parseCss` as a *comment span* at its `allow_comment_or_whitespace` boundaries — the stylesheet/block body (`read_body`) and the selector-list start / after a complete selector / after a comma (`read_selector_list`). It reads to the required `-->` and **discards everything between**, emitting no node. This departs from CSS Syntax 3, where `<!--` (CDO) and `-->` (CDC) are two *independent* no-op tokens and the content between them parses as ordinary CSS: per spec `<style><!-- h1 { color: red } --></style>` keeps the `h1` rule, but `parseCss` (and thus tsv) drops it, and the whole-stylesheet idiom `<!-- …rules… -->` parses to an **empty** stylesheet (so `format` deletes the wrapped CSS — matching Svelte's compiled output, where the rules are already dead). tsv matches `parseCss` via `skip_html_comment_markers` (`crates/tsv_css/src/parser/mod.rs`): the boundary skip discards the span (unterminated `-->` is an error, mirroring Svelte's `eat('-->', true)`); in **value** and **at-rule-prelude** position the markers are NOT special (those readers scan raw, so a `;`/`{` between them stays significant), and `<!--` between compounds (`h1 <!-- --> p`) is rejected — all matching `parseCss`. Pinned by [tests/css_cdo_cdc.rs](../tests/css_cdo_cdc.rs) and the svelte-fixtures gate (`css/samples/comment-html`); the formatter drop-on-format and prettier's invalid-CSS mangling are the `_prettier_divergence` at [css/tokens/html_comment_prettier_divergence](../tests/fixtures/css/tokens/html_comment_prettier_divergence/), cataloged in [conformance_prettier_css.md §CSS: HTML comments (CDO/CDC)](conformance_prettier_css.md#css-html-comments-cdocdc). **Residual** (a near-term, non-fixtured limit): a marker at the *start* of a **pseudo-argument** selector list — `:has(<!-- --> > img)` (rejects), `:is(<!-- --> .a)` (accepts, but with a divergent `Invalid`-selector shape) — and a marker interleaved with a `/* */` comment at a selector boundary are not matched. Both are deeply pathological (a legacy HTML comment inside a `:has()`/`:is()` argument) and reach neither the gate nor the corpus; normal rule selector lists match exactly.

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

**tsv behavior**: Every directive carries a `modifiers` array, and tsv preserves the modifier text **verbatim for all eight directive types** — matching Svelte's permissive runtime parser exactly, including unofficial modifiers on the five types whose published `.d.ts` declares none (`use:foo|bar` → `['bar']`, `on:click|preventDefault|bogus` → `['preventDefault', 'bogus']`, in both parsers). So this is **not** a `_svelte_divergence` — tsv's parser AST matches Svelte's. On **format**, the two formatters diverge for the five types without official support: prettier-plugin-svelte silently drops the `|mod` text, while tsv preserves it — a `_prettier_divergence` (content preservation), pinned by [modifier_preservation](../tests/fixtures/svelte/directives/modifier_preservation_prettier_divergence/). See [conformance_prettier_svelte.md §Svelte: Attributes](./conformance_prettier_svelte.md#svelte-attributes).

**Reference**: `svelte/packages/svelte/src/compiler/types/template.d.ts`

### Template Whitespace (`clean_nodes`)

Svelte deals with template whitespace at **compile time**, not parse time. The parser — and tsv's drop-in AST — keeps every whitespace character verbatim: boundary runs, inter-sibling runs, and whitespace-only `Text` nodes all appear in the AST byte-for-byte, so no whitespace behavior here is a `_svelte_divergence`. The trimming happens in the transform phase (`clean_nodes`): comments are removed first (unless `preserveComments`), whitespace-only text nodes at a fragment's edges are dropped, and the first/last text node's edge run is stripped — unconditionally, for **every** fragment in the language (element/component content and block bodies alike), even when the neighbor is an `ExpressionTag`. Interior runs collapse to one space (text adjacent to an `ExpressionTag` stays verbatim). The whitespace set is `[ \t\r\n]` exactly — NBSP and other Unicode spaces are content. Exemptions: `preserveWhitespace` and `<pre>`/`<textarea>` subtrees.

The **formatter** mirrors this: in inline layout tsv deletes exactly the render-free boundary runs `clean_nodes` deletes — a `_prettier_divergence`, cataloged in [conformance_prettier_svelte.md §Svelte: Inline content block-style](./conformance_prettier_svelte.md#svelte-inline-content-block-style) (elements) and [§Svelte: Blocks](./conformance_prettier_svelte.md#svelte-blocks) (block sections). tsv is deliberately more conservative than the compiler in one respect: comments **block** the trim (they are treated as nodes rather than removed first), so the formatted output renders identically under `preserveComments` too. `<svelte:options preserveWhitespace />` is not detected — any reformatting is already render-visible under it (prettier behaves the same).

Where the formatter rewrites an **interior separator's spelling** it is answering to the *browser's* whitespace collapse rather than to `clean_nodes`, and the two do not agree everywhere. `clean_nodes` collapses only where neither neighbor is an `ExpressionTag`, so `{a}⇥{b}` reaches compiled output with its tab intact while `<code>a</code>⇥<code>b</code>` compiles to a single space; tsv (and prettier) print both as one space, which is render-identical under `white-space: normal` — the model `tsv_debug`'s `svelte-render-key` oracle and `render_normalize` both implement — and divergent under `pre`/`pre-wrap`. tsv already relies on that same browser model wherever a spaced tag pair goes block-style and each tag takes its own line.

The **character class** that decides all of this is `internal::is_collapsible_ws` = `[ \t\n\r]`, matching `clean_nodes`' `regex_not_whitespace` rather than Rust's `is_ascii_whitespace` or prettier-plugin-svelte's `[\t\n\f\r ]`, both of which are one character wider. A **form feed** (U+000C) is rendered content per CSS — white-space processing reaches only U+0020, U+0009 and segment breaks — so tsv preserves it verbatim in every position, where prettier respells it as a space and, at a content boundary, tsv itself used to delete it outright. That is a `_prettier_divergence` no corpus diff against prettier can surface, since prettier shares the wider class: [conformance_prettier_svelte.md §Whitespace: Form feed](./conformance_prettier_svelte.md#whitespace-form-feed).

**Entity-encoded whitespace** (`&#9;`, `&#x20;`, `&Tab;`) splits the question in two, and tsv answers each on its own axis. Svelte tests the decoded `data`, so such a node is whitespace to `clean_nodes`; tsv's whitespace *scalars* test `raw`, as prettier does (`node.raw || node.data`), so it is **content** to them and its bytes print verbatim — respelling an entity is a content edit, and that half is deliberate. But the two *layout* questions asked about a separator are about what the characters are, not how they are spelled, so both read the decoded text: "does this node carry a word for a `fill` to pack?" (no — so it is not the run's prose) and "is this separator interchangeable with a plain space?" (yes for whitespace the compiler collapses, no for an `&nbsp;`, which renders as itself). An entity separator therefore lays out exactly like the literal character it decodes to, while keeping its own spelling — [inline_separator_entity_newline](../tests/fixtures/svelte/elements/inline_separator_entity_newline/) and [inline_separator_entity_collapse](../tests/fixtures/svelte/elements/inline_separator_entity_collapse_prettier_divergence/).

**Reference**: `svelte/packages/svelte/src/compiler/phases/3-transform/utils.js` (`clean_nodes`), `phases/patterns.js` (the whitespace regexes)

#### Source `trimEnd` — a known parse-time divergence

One whitespace decision *is* made at parse time, and tsv currently gets its character
class wrong. Svelte's parser opens with `this.template = template.trimEnd()`
(`phases/1-parse/index.js`) — JavaScript's `trimEnd`, i.e. ECMAScript
`WhiteSpace` ∪ `LineTerminator`. tsv's counterpart (`parser/mod.rs`, the trailing-text
capture) uses Rust's `str::trim_end`, i.e. the Unicode `White_Space` property. The two
classes differ at exactly two code points, one in each direction:

| Trailing code point | JS `trimEnd` | Rust `trim_end` | Effect |
| --- | --- | --- | --- |
| `U+FEFF` (`<ZWNBSP>`) | strips | keeps | tsv emits a trailing `Text` node Svelte does not |
| `U+0085` (`<NEL>`) | keeps | strips | Svelte emits a trailing `Text` node tsv does not |

Every other separator (`U+00A0`, `U+2000`, `U+202F`, `U+3000`, `U+180E`, `U+200B`) is in
both classes or in neither, so it round-trips. The divergence is trailing-position-only —
a leading or interior occurrence, and any occurrence inside an element, is unaffected.

This is a **bug, not a sanctioned divergence**: it changes the drop-in parse AST and
propagates into compiled output. The fix is to match the JS class rather than Rust's, and
it wants a `_svelte_divergence`-shaped fixture pinning both directions.

---

## Related

- ./conformance_prettier.md — Prettier formatter differences
- ./checklist_css.md — CSS feature matrix
- ./fixture_overview.md — Fixture system details
