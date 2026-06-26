# Svelte Conformance

The tsv parser aims for **exact AST compatibility** with Svelte's parser. This document catalogs tsv's compatibility behaviors and intentional corrections.

## Mental Model

**Matched**: tsv produces identical AST to Svelte (the goal). This includes replicating Svelte's quirky behaviors for tool compatibility.

**Unmatched**: tsv produces different AST. The suffix `_svelte_divergence` marks these fixtures. tsv differs when Svelte or acorn-typescript is wrong — a spec violation, a missing feature, or a bug tsv corrects (e.g. acorn's double-fired `onComment` duplicating comments). One exception isn't a correction: a lone UTF-16 surrogate can't survive tsv's UTF-8 strings (→ U+FFFD), so tsv differs there despite acorn being right.

## Classification

- Compat behavior — Svelte has quirky but harmless behavior. tsv action: tsv replicates it in AST output
- Correction — Svelte/acorn violates spec, lacks a feature, or has a bug (e.g. acorn's duplicate `onComment` firing). tsv action: tsv produces correct/complete AST
- Representation limit — a value acorn keeps can't round-trip tsv's UTF-8 strings (lone surrogate → U+FFFD; `raw` unaffected). Rare, not a correction

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

- :nth-child(An+B of S) — Incorrect AST structure — [nth_child_of](../tests/fixtures/css/selectors/pseudo_class/nth_child_of_svelte_divergence/)
- Attribute namespaces `[ns|attr]` — Not supported — [namespace](../tests/fixtures/css/selectors/attribute/namespace_svelte_divergence/)
- No-namespace `|element` — Not supported — [no_namespace](../tests/fixtures/css/selectors/namespace/no_namespace_svelte_divergence/)
- Forgiving :is()/:where() — Strict parsing (should be forgiving) — [forgiving_is_where](../tests/fixtures/css/selectors/forgiving_is_where_svelte_divergence/)
- Empty-after-comment declarations — Rejected (`css_empty_declaration`) — [comment_empty_value](../tests/fixtures/css/tokens/comments/comment_empty_value_svelte_divergence/)
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

Non-standard `.css` is auto-classified into `expected errors` by the corpus
comparator (`benches/deno/lib/divergence/expected_errors.ts`).

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
- `export default class implements I {}` (anonymous default class, implements-first heritage) — [export_default_implements](../tests/fixtures/typescript/declarations/class/export_default_implements_svelte_divergence/)
- Async generic arrow params — see fixtures below

**`using` keyword-name comments**: Both acorn and tsv reject comments between `using` and the binding name (`using /* c */ x = fn()`), and between `await` and `using` (`await /* c */ using x = fn()`). Per the ECMAScript spec, comments behave like white space and are discarded between any two tokens (§12.4), so these should be valid. However, since `using` is a contextual keyword requiring lookahead disambiguation, both parsers check the next token before comment processing. tsv matches acorn's behavior here. If acorn adds support, tsv should follow.

**Async generic arrow params**: acorn-typescript drops all function parameters from `async` arrow functions that have type parameters (`async <T,>(x: T) => x` → `params: []`). Non-async generic arrows are unaffected. This is semantic corruption — tools consuming the AST would see zero-argument functions. **Upstream candidate**: acorn-typescript async arrow parsing.

Fixtures: [async_generic/stacked](../tests/fixtures/typescript/expressions/arrow/async_generic/stacked_svelte_prettier_divergence/), [async_generic/forms](../tests/fixtures/typescript/expressions/arrow/async_generic/forms_svelte_prettier_divergence/), [async_generic/basic_ts](../tests/fixtures/typescript/expressions/arrow/async_generic/basic_ts_svelte_divergence/), [async_generic/long](../tests/fixtures/typescript/expressions/arrow/async_generic/long_svelte_divergence/), [curried_typed_callback](../tests/fixtures/typescript/expressions/arrow/curried_typed_callback_svelte_prettier_divergence/). `async_generic/forms` adds the optional-param (`x?`) drop, distinct from the plain param (`stacked`) and rest param (`long`).

The `async_generic/stacked`, `async_generic/forms`, and `curried_typed_callback` fixtures carry a second,
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
- Anonymous class-expression `id` — omitted for implements-first heritage
- `export default class implements I {}` — anonymous default class with implements-first heritage rejected (`implements` read as a reserved-word name)

**acorn** — fix in acorn core:

- ES2024 v-flag regex — Unicode sets `v` flag not supported

**Svelte CSS parser** — fix directly in Svelte:

- Forgiving :is()/:where() — Strict parsing where spec requires forgiving
- :nth-child(An+B of S) — Incorrect AST structure for `of S` syntax
- Attribute namespaces — `[ns\|attr]` not supported
- No-namespace selectors — `\|element` not supported
- Empty-after-comment decl — Rejects `prop: /* c */;` after stripping comments (5.55.x) — Prettier still formats it
- Block-valued custom properties — Rejects `--x: { … }` (`css_expected_identifier`) — Prettier still formats it
- Stray `;;` garbage declaration — `border-box;;` yields `{property: ";"}` swallowing the next declaration (spec: drop empty declarations)
- Comment-touching-property garbage — `color/* c */:` yields `property: "color/*"` (`read_until` scans to the whitespace inside the comment)

**Svelte template parser** — fix directly in Svelte:

- each-`as` stale `loc.end` — TS-mode as-expression unwrap patches the expression's `end` offset but not `loc.end`

### Comment Attachment Differences

Acorn-typescript speculatively re-parses many TypeScript constructs (a backtrack-and-reparse), and its `onComment` callback fires **twice** for any comment inside the re-parsed region — duplicating that comment in the root `comments` array (and in any `leadingComments`/`trailingComments` attachment). **tsv emits each comment once everywhere**: it corrects this duplication rather than replicating it. The set of distinct comments is identical, only multiplicity differs, `ast_diff` confirms semantic equivalence, and the formatter is unaffected (it locates comments by position, not by their count). Fixtures carry `expected_ours.json` + `expected_svelte.json`; one that also diverges from prettier on the comment's placement additionally carries the `_svelte_prettier_divergence` suffix.

The constructs acorn re-parses (root `comments` duplication tsv corrects):

- **Type literal `{ … }` body — a comment between `{` and the first member (or on a member key):**
  - [type_literal_open_brace_comment_svelte_divergence](../tests/fixtures/typescript/types/type_literal_open_brace_comment_svelte_divergence/)
  - [type_literal_open_brace_comment_svelte_prettier_divergence](../tests/fixtures/typescript/types/type_literal_open_brace_comment_svelte_prettier_divergence/)
  - [type_literal_property_keys_svelte_divergence](../tests/fixtures/typescript/types/type_literal_property_keys_svelte_divergence/)
  - [type_literal_member_trailing_comment_long_svelte_divergence](../tests/fixtures/typescript/types/type_literal_member_trailing_comment_long_svelte_divergence/)
  - [optional_marker_before_comment_svelte_divergence](../tests/fixtures/typescript/types/type_literal/optional_marker_before_comment_svelte_divergence/)
  - [optional_marker_comment_svelte_prettier_divergence](../tests/fixtures/typescript/types/type_literal/optional_marker_comment_svelte_prettier_divergence/)
  - [literal_body_empty_svelte_divergence](../tests/fixtures/typescript/types/comments/literal_body_empty_svelte_divergence/)
  - [type_literal_jsdoc_svelte_divergence](../tests/fixtures/typescript/types/comments/type_literal_jsdoc_svelte_divergence/)
  - [type_literal_leading_svelte_divergence](../tests/fixtures/typescript/types/comments/type_literal_leading_svelte_divergence/)
  - [type_literal_leading_mixed_svelte_divergence](../tests/fixtures/typescript/types/comments/type_literal_leading_mixed_svelte_divergence/)
  - [type_literal_line_before_block_svelte_divergence](../tests/fixtures/typescript/types/comments/type_literal_line_before_block_svelte_divergence/)
  - [union_hug_object_interior_comment_svelte_divergence](../tests/fixtures/typescript/types/union_hug_object_interior_comment_svelte_divergence/)
  - [union_nonhug_object_interior_comment_svelte_divergence](../tests/fixtures/typescript/types/union_nonhug_object_interior_comment_svelte_divergence/)
  - [index_signature_union_intersection_value_svelte_divergence](../tests/fixtures/typescript/types/type_members/index_signature_union_intersection_value_svelte_divergence/) — index-signature value formatting; the divergence is the first label comment after `{`
  - [call_type_arg_empty_comment_svelte_divergence](../tests/fixtures/typescript/typescript_specific/generics/call_type_arg_empty_comment_svelte_divergence/) — empty object type literal as a call type argument (`fn<{ /* … */ }>()`); the comment is inside the empty `{ }` body
  - [call_type_arg_member_comment_svelte_divergence](../tests/fixtures/typescript/typescript_specific/generics/call_type_arg_member_comment_svelte_divergence/) — populated object type literal **and** mapped type as a call type argument (`fn<{ // … \n a: V }>()`, `fn<{ // … \n [K in keyof T]: V }>()`); a leading comment in the body/mapped header duplicates, a trailing member comment does not (control)
  - [prettier_ignore_members_svelte_divergence](../tests/fixtures/typescript/syntax/comments/prettier_ignore_members_svelte_divergence/)

- **Mapped type `{ [K in … ] }` header — a comment from `{` up to `in`:**
  - [mapped_bracket_comment_svelte_divergence](../tests/fixtures/typescript/types/mapped_bracket_comment_svelte_divergence/)
  - [mapped_leading_comment_svelte_divergence](../tests/fixtures/typescript/types/mapped_leading_comment_svelte_divergence/)

- **Function type parameter list — a comment in the parens before the param colon (the `tsIsUnambiguouslyStartOfFunctionType` lookahead; typed params are not exempt):**
  - [function_type_empty_param_comment_svelte_divergence](../tests/fixtures/typescript/types/function_type_empty_param_comment_svelte_divergence/)
  - [empty_param_line_comment_svelte_prettier_divergence](../tests/fixtures/typescript/types/function_type/empty_param_line_comment_svelte_prettier_divergence/)
  - [typed_param_comment_positions_svelte_divergence](../tests/fixtures/typescript/types/function_type/typed_param_comment_positions_svelte_divergence/)
  - [function_type_param_trailing_svelte_divergence](../tests/fixtures/typescript/types/comments/function_type_param_trailing_svelte_divergence/)

- **Index signature `[k: T]` — a comment inside the brackets, before the key or after the key (type-member and class):**
  - [index_signature_comment_svelte_divergence](../tests/fixtures/typescript/types/type_members/index_signature_comment_svelte_divergence/)
  - [index_signature_bracket_comment_positions_svelte_divergence](../tests/fixtures/typescript/types/type_members/index_signature_bracket_comment_positions_svelte_divergence/)
  - [index_signature_key_colon_line_comment_svelte_prettier_divergence](../tests/fixtures/typescript/types/type_members/index_signature_key_colon_line_comment_svelte_prettier_divergence/)
  - [index_signature_key_type_line_comments_svelte_prettier_divergence](../tests/fixtures/typescript/types/type_members/index_signature_key_type_line_comments_svelte_prettier_divergence/)
  - [index_signature_open_bracket_line_comment_svelte_prettier_divergence](../tests/fixtures/typescript/types/type_members/index_signature_open_bracket_line_comment_svelte_prettier_divergence/)
  - [index_signature_bracket_comment_positions_svelte_divergence](../tests/fixtures/typescript/declarations/class/index_signature_bracket_comment_positions_svelte_divergence/)
  - [index_signature_bracket_line_comment_positions_svelte_prettier_divergence](../tests/fixtures/typescript/declarations/class/index_signature_bracket_line_comment_positions_svelte_prettier_divergence/)

- **Computed key `[ … ]` — a class computed method or a type-member computed method:**
  - [computed_key_comment_svelte_divergence](../tests/fixtures/typescript/types/type_members/computed_key_comment_svelte_divergence/)
  - [computed_key_open_bracket_line_comment_svelte_prettier_divergence](../tests/fixtures/typescript/statements/class/computed_key_open_bracket_line_comment_svelte_prettier_divergence/)

- **Angle-bracket type assertion `<T>x` — a block or line comment anywhere in the cast (inside `<…>`, after the type before `>`, or after `>` before the expression); a comment before `<` sits outside the reparse window and is not duplicated:**
  - [type_assertion_comment_svelte_divergence](../tests/fixtures/typescript/types/type_assertion_comment_svelte_divergence/)
  - [type_assertion_line_comment_svelte_prettier_divergence](../tests/fixtures/typescript/types/type_assertion_line_comment_svelte_prettier_divergence/)
  - [type_assertion_close_own_line_comment_svelte_prettier_divergence](../tests/fixtures/typescript/types/type_assertion_close_own_line_comment_svelte_prettier_divergence/)
  - [type_assertion_expr_own_line_comment_svelte_prettier_divergence](../tests/fixtures/typescript/types/type_assertion_expr_own_line_comment_svelte_prettier_divergence/)
  - [type_assertion_line_comment_robustness_svelte_prettier_divergence](../tests/fixtures/typescript/types/type_assertion_line_comment_robustness_svelte_prettier_divergence/)

- **Arrow return type — a comment between the return type and `=>`:**
  - [after_return_type_comment_svelte_divergence](../tests/fixtures/typescript/expressions/arrow/after_return_type_comment_svelte_divergence/)
  - [return_type_untyped_param_comment_svelte_divergence](../tests/fixtures/typescript/expressions/arrow/return_type_untyped_param_comment_svelte_divergence/)

A comment on a return/property type annotation immediately followed by `;` (the
member-type reparse) — root `comments` duplication, same mechanism as above:

- [method_trailing_semicolon_comment_svelte_prettier_divergence](../tests/fixtures/typescript/declarations/class/method_trailing_semicolon_comment_svelte_prettier_divergence/)
- [trailing_semicolon_comment_svelte_divergence](../tests/fixtures/typescript/types/type_members/trailing_semicolon_comment_svelte_divergence/)

Beyond acorn-typescript's per-parse duplication, **Svelte's own comment glue duplicates or drops comments at `<script>` and template boundaries**. tsv attaches each comment once, in its source region — the same anti-duplication stance as above. In every case below the distinct-comment set is identical (the comment is preserved on its source node and/or in the root `comments` array), `ast_diff` confirms semantic equivalence, and the formatter — which locates comments by position — is unaffected.

- **Module-script comment duplicated onto the instance script.** Svelte parses the `<script module>` and instance `<script>` against one shared `root.comments` array, and the instance parse's `add_comments` walk is not given a fresh queue, so every module-script comment (leading *or* trailing) is also shifted into the instance script's first statement (`instance.content.body[0].leadingComments`). tsv keeps each module comment only on the module body.
  - [module_comment_instance_duplication_svelte_divergence](../tests/fixtures/svelte/script/module_comment_instance_duplication_svelte_divergence/)

- **Block binding-pattern interior comment — node attachment + column offset.** Svelte parses the `{#each … as}` context and the `{#await … then}` / `{:then}` / `{:catch}` binding patterns with a separate acorn parse that (a) **attaches** an interior comment to its adjacent pattern node as `leadingComments` / `trailingComments`, and (b) for any such comment past the pattern's first line reports its `loc.column` **one too high** (an offset-translation slip in the context reparse — byte `start`/`end` are correct; the same context-reparse-loc family as the `each_as_stale_loc` correction above). tsv keeps each comment once in the root `comments` array, unattached, with the correct column. Distinct-comment set identical, `ast_diff` equivalent, formatter unaffected. These fixtures also drop the comment in prettier-plugin-svelte, so they carry the `_svelte_prettier_divergence` suffix — see [conformance_prettier.md §Svelte: destructuring binding-pattern comments](./conformance_prettier.md#svelte-destructuring-binding-pattern-comments).
  - [each/destructure_comment_svelte_prettier_divergence](../tests/fixtures/svelte/blocks/each/destructure_comment_svelte_prettier_divergence/)
  - [await/destructure_comment_svelte_prettier_divergence](../tests/fixtures/svelte/blocks/await/destructure_comment_svelte_prettier_divergence/)

- **Leading HTML comment duplicated onto the instance script.** A leading fragment HTML comment (`<!-- @component … -->`) before a `<script module>` + instance `<script>` pair is attached to *both* the module Program and the instance Program. tsv attaches it once, to the nearest (module) script Program; the comment is also a `Comment` node in the fragment in both parsers, so nothing is lost. (With no module script there is a single instance Program and tsv matches Svelte — the divergence needs a second script root to be copied onto.)
  - [leading_html_comment_instance_duplication_svelte_divergence](../tests/fixtures/svelte/script/leading_html_comment_instance_duplication_svelte_divergence/)

- **Template-expression comment before a parenthesized subexpression.** Svelte's `parse_expression_at` sets acorn's `preserveParens: true`, so a leading comment before a parenthesized subexpression attaches to the synthetic `ParenthesizedExpression`; Svelte's subsequent `remove_parens` discards that wrapper and its `leadingComments`, leaving the comment only in the root `comments` array. tsv (which has no `ParenthesizedExpression` node, matching Svelte's *final* shape) attaches it to the inner expression. This is template-only — a plain `<script>` parse does not set `preserveParens`, so the same comment attaches in both parsers there. The common real-world trigger is a JSDoc cast `/** @type {T} */ (expr)`.
  - [template_expr_paren_comment_svelte_divergence](../tests/fixtures/svelte/syntax/comments/template_expr_paren_comment_svelte_divergence/) — precedence parens, isolating the parser difference
  - [jsdoc_cast_template_svelte_prettier_divergence](../tests/fixtures/svelte/syntax/comments/jsdoc_cast_template_svelte_prettier_divergence/) — the JSDoc-cast trigger across template / attribute / directive positions; also a `_prettier_divergence` (prettier strips the cast there)


### Known Acorn-TypeScript Bugs (Not Corrections)

These are bugs in **upstream/standalone `acorn-typescript`** — the non-fork npm
package, distinct from the `@sveltejs/acorn-typescript@1.0.10` fork this project
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

Backslash doubling and unicode-escape duplication are inherited "for free" by extracting raw bytes (`source[span]`) into the public JSON value — Svelte's parser embeds those quirks in its span, so reproducing the bytes reproduces the quirks. No quirk-specific encoder runs.

**Selector-name half-decoding.** Svelte's `read_identifier` decodes a selector name only *half*-way: a **hex** escape (`\3A `, `\1F600`, with an optional single-whitespace terminator) decodes to its codepoint, but an **identity** escape (a backslash before a non-hex char — `\?`, `\@`, `:f\oo`) keeps the backslash. tsv's internal lexer fully decodes (the spec-canonical `<ident-token>` value, e.g. `:f\oo` → `foo`), so the public `name` is reconstructed half-decoded from the raw span by `raw_selector_name` for **every** selector kind — class/id/type, pseudo-class/element, and attribute. (For class/id/type and pseudo names the formatter already emitted the raw source from the span, so formatting was unaffected; **attribute** names additionally needed the formatter fixed — it had reconstructed the selector from the *decoded* `name`, so `[f\oo]` printed as `[foo]` and even `[\41 b]` as `[Ab]`, silently dropping escapes. The internal `Attribute` selector now carries a `name_span` (the name token within `[ns|name op 'value' flags]`); the printer emits it raw and convert half-decodes it, so escapes are preserved in output and the AST matches Svelte.) **Why match the half-form and not the spec:** the public AST's contract is byte-for-byte parity with Svelte's `parseCss` (tsv is a drop-in for it), so where Svelte's scan-based decode diverges from the CSS Syntax spec's full ident decode, tsv mirrors Svelte. Pinned by [css/selectors/escaped_names](../tests/fixtures/css/selectors/escaped_names/) (class/id/type identity escapes), [css/selectors/pseudo_escaped_identity](../tests/fixtures/css/selectors/pseudo_escaped_identity/) (pseudo identity escapes — `:f\oo` → `"f\\oo"`, never `"foo"`), and [css/selectors/attribute/escaped_identity](../tests/fixtures/css/selectors/attribute/escaped_identity/) (attribute names — both the AST half-decode and the formatter preserving the raw escape).

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
