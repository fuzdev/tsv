# Prettier Conformance: TypeScript

The TypeScript catalog of tsv's deliberate Prettier divergences, minus comments — the
largest category, which carries its own doc,
[conformance_prettier_ts_comments.md](./conformance_prettier_ts_comments.md). The
terminology, the `◆reason` tags, the prettier-bug index, and the decision framework that
governs every entry here live in [conformance_prettier.md](./conformance_prettier.md).

## TypeScript

- Empty statement blank lines — ◆design_choice — [empty_standalone](../tests/fixtures/typescript/statements/empty_standalone_prettier_divergence/)
- Return type generic union — ◆print_width — [return_type_generic_union_long](../tests/fixtures/typescript/declarations/function/return_type_generic_union_long_prettier_divergence/)
- Module path calls — ◆print_width — [path_calls_long](../tests/fixtures/typescript/modules/imports/path_calls_long_prettier_divergence/)
- Trailing member after object-arg call — ◆print_width — [trailing_member_expand_args_long](../tests/fixtures/typescript/expressions/calls/chained/trailing_member_expand_args_long_prettier_divergence/)
- Instantiation expression parens — ◆prettier_bug — [instantiation_parens](../tests/fixtures/typescript/typescript_specific/assertions/instantiation_parens_prettier_divergence/), [export_default_instantiation](../tests/fixtures/typescript/modules/exports/default_wrappable_leftmost_operators/instantiation_prettier_divergence/)
- Non-null parenthesized base — ◆design_choice — [non_null_paren_base_long](../tests/fixtures/typescript/expressions/member/non_null_paren_base_long_prettier_divergence/)
- Parenthesized binary member base — ◆design_choice ◆print_width — [paren_binary_base_long](../tests/fixtures/typescript/expressions/member/paren_binary_base_long_prettier_divergence/)
- Constrained infer extends-operand parens — ◆prettier_bug — [constrained_extends_parens](../tests/fixtures/typescript/types/infer/constrained_extends_parens_prettier_divergence/)
- Arrow type param trailing comma — ◆design_choice — [single_type_param](../tests/fixtures/typescript/expressions/arrow/generic/single_type_param_prettier_divergence/)
- Empty-object comment bracket spacing — ◆design_choice — [empty_block_comment](../tests/fixtures/typescript/expressions/objects/empty_block_comment_prettier_divergence/), [destructure empty_comment](../tests/fixtures/typescript/expressions/destructuring/empty_comment_prettier_divergence/), [enum empty_comment](../tests/fixtures/typescript/declarations/enum/body_empty_comment_prettier_divergence/), [literal_body_empty](../tests/fixtures/typescript/types/comments/literal_body_empty_prettier_divergence/), [union_empty_object_member](../tests/fixtures/typescript/types/union_empty_object_member_prettier_divergence/), [call_type_arg_empty_comment](../tests/fixtures/typescript/typescript_specific/generics/call_type_arg_empty_comment_prettier_divergence/)
- Optional rest parameter `?` — ◆design_choice — [rest_optional_param](../tests/fixtures/typescript/typescript_specific/rest_optional_param_prettier_divergence/)
- Comment in the name→`?` gap of an **unannotated, non-rest** optional parameter (`function fn(a /* c */?) {}`, and the arrow spelling) — ◆prettier_bug — prettier's TypeScript printer loses the comment and **throws** `Comment "c" was not printed`, so no oracle output exists; the fixture's `prettier_rejects.txt` pins the throw live, so a prettier release that fixes it fails the fixture rather than going unnoticed. tsv parses and keeps the comment before the marker. With an annotation, or on the *rest* spelling (the entry above, where prettier survives but strips the `?`), prettier does not crash — [optional_param_comment](../tests/fixtures/typescript/statements/function/optional_param_comment_prettier_divergence/)
- ES2015+ identifier property keys — ◆design_choice ◆spec_precedence — [property_key_es2015_ident](../tests/fixtures/typescript/expressions/objects/property_key_es2015_ident_prettier_divergence/)
- Non-null optional-chain bare strip — ◆design_choice — [optional_paren_non_null_bare](../tests/fixtures/typescript/expressions/chain/optional_paren_non_null_bare_prettier_divergence/)
- Class field key unquoting — ◆design_choice — [field_key_unquote](../tests/fixtures/typescript/declarations/class/field_key_unquote_prettier_divergence/)
- Member-chain wide-last-argument hug convergence — ◆prettier_bug — [last_arg_hug_convergence_long](../tests/fixtures/typescript/expressions/calls/chained/last_arg_hug_convergence_long_prettier_divergence/)
- Bodyless `declare global` — ◆prettier_bug — [global_shorthand](../tests/fixtures/typescript/declarations/namespace/global_shorthand_prettier_divergence/)

**Instantiation expression parens**: Prettier strips parentheses from ternary and binary expressions in `TSInstantiationExpression` (`(x ? y : z)<T>` → `x ? y : z<T>`), changing semantics. Without parens, `<T>` only applies to the last operand. tsv preserves parens to maintain the original meaning. Both formatters agree on preserving parens for assignment expressions (`(x = y)<T>`). A class-expression operand in `export default` position — `export default (class {}<T>)` — is the same bug but sharper: stripping the parens makes the leading `class {}` a class _declaration_, so Prettier's output re-parses to a `ClassDeclaration` plus a dangling `<T>;` statement (a different AST), while tsv keeps the parens (adjudicated by `export_default_needs_parens`; see [export_default_instantiation](../tests/fixtures/typescript/modules/exports/default_wrappable_leftmost_operators/instantiation_prettier_divergence/)).

**Constrained infer extends-operand parens**: An `infer X extends C` only ever appears in a conditional type's extends-type, so a trailing token always follows the constraint. When a _nested_ arrow's return abuts the enclosing `? :`, the parens TypeScript requires are the only thing keeping the parse unambiguous, and Prettier strips them, emitting output that **fails to re-parse** (acorn-typescript rejects it): `M extends (() => () => infer U extends string) ? …` → `M extends () => () => infer U extends string ? …` (Prettier's `needs-parentheses` rule only inspects the immediate return type). tsv keeps the parens, staying valid. Two related forms are preserved by both formatters: the _conditional-type_ infer constraint (`X extends infer U extends (A extends B ? C : D) ? …` — Prettier keeps these parens) and the single-arrow return (`M extends (() => infer U extends string) ? …` — Prettier's single-level rule covers it; see [constrained_extends_parens](../tests/fixtures/typescript/types/infer/constrained_extends_parens/), where tsv matches). A bare `<T extends (A extends B ? C : D)>` type-parameter declaration is unaffected: the `>` terminates it, so Prettier strips and tsv matches.

**Module path calls**: Prettier special-cases `require`/`import` identifiers:

- `require(string)`: Prettier keeps on one line regardless of length; tsv wraps at print width
- `require.resolve.paths(string)`: Prettier breaks at `.paths` chain; tsv expands call arguments
- `import.meta.resolve(string)`: Prettier breaks at `.resolve` chain; tsv expands call arguments

tsv treats these like any other function call—no special-casing for module path identifiers, so print width is respected uniformly.

**Trailing member after a call with an object argument**: When `X.f({…}).member` exceeds print width — the call's arguments fit inline, but appending the trailing member overflows — Prettier keeps the arguments on one line and breaks the member onto its own indented line (`.member`), while tsv expands the object argument and keeps the member on the closing brace (`}).member`). This is the same stance as Module path calls above: tsv wraps a call's arguments the same way regardless of a trailing member, rather than special-casing the chain. Because Prettier preserves a multiline object, it keeps tsv's expanded form stable too, so the two only diverge from compact or Prettier-authored source — the Prettier form is pinned as a `prettier_variant` (Prettier keeps it stable; tsv normalizes it back to `input`). Fixture: [trailing_member_expand_args_long](../tests/fixtures/typescript/expressions/calls/chained/trailing_member_expand_args_long_prettier_divergence/).

**Non-null parenthesized base**: For a non-null assertion on a parenthesized base whose inner call breaks its arguments (`(await call(...))!.member`), Prettier hugs the inner call (`(await call(\n...\n))!.member`) — yet it _hangs_ the outer parens for the same base without the `!` (`(await call(...)).member`, see [paren_base_trailing_long](../tests/fixtures/typescript/expressions/member/paren_base_trailing_long/), where tsv matches Prettier). tsv lays out the parenthesized base the same way regardless of a trailing non-null assertion, keeping the two forms visually consistent. Content is identical (ASTs match); only the parenthesized-base layout differs. The rule is scoped to the **call / `await`** base family it is stated over: a parenthesized **ternary** base is not covered and follows Prettier exactly, including the part that *does* key on the `!` — Prettier's `breakClosingParen` (`print/ternary.js`) fires only when the member access is the ternary's direct parent, so `(c⏎? a⏎: b⏎).prop` drops its `)` to its own line while `(c⏎? a⏎: b)!.prop` keeps it welded to the last arm, the `!` being the parent in between. See [paren_ternary_operand_positions](../tests/fixtures/typescript/expressions/paren_ternary_operand_positions/).

**Parenthesized binary member base**: For a parenthesized binary expression used as a member-access base, long enough that the parens must break onto their own lines and whose left operand is itself a parenthesized binary (`((a && b) || c).toString()`), Prettier breaks the parens **and** splits the operand chain onto separate lines. tsv takes only the break the width demands — the parens break, the operand chain stays flat while it fits (`(\n\t(a && b) || c\n).toString()`) — per [§Print Width Philosophy](./conformance_prettier.md#print-width-philosophy). The shape is uniform along both axes Prettier varies on: **operator family** (arithmetic, logical `&&`/`||`, and nullish `??` bases are laid out identically) and **what follows the base** (a plain `.member`, a non-null `!.member`, and an optional `?.member` all lay out identically, where Prettier hangs the parens for `.`/`?.` and welds `)!.member` onto the last operand for `!` — the same stance as [§Non-null parenthesized base](#typescript), extended from call/await bases to binary ones). That uniformity is the substance of the rule: the alternative — giving logical and nullish bases a *third* layout that welds the closing `).member` onto the last operand (`((a && b) ||\n\tc).toString()`) — matches neither tsv's own arithmetic shape nor Prettier's, so it is a shape with no constituency. A **flat** chain (`a && b && c`, no parenthesized operand) is outside the divergence: with no nested operand to hold together both formatters break every operand. Content is identical (ASTs match); only the operand-chain layout differs. See [paren_binary_base_long](../tests/fixtures/typescript/expressions/member/paren_binary_base_long_prettier_divergence/). A parenthesized **call** or `as`-cast base is a different case, where tsv matches Prettier — see [paren_base_trailing_long](../tests/fixtures/typescript/expressions/member/paren_base_trailing_long/).

**Expand-first tail at print width**: The expand-first hug (Prettier's `shouldExpandFirstArg` — `fn(() => {…}, tail)`) renders the tail argument inline past the callback's closing brace, so that closing line is the one print width measures. Prettier emits the layout unconditionally and never breaks inside the tail, letting the line run however far it must; tsv takes the break the width demands, per [§Print Width Philosophy](./conformance_prettier.md#print-width-philosophy) — at 100 both hold the tail flat, at 101 tsv breaks the tail's operator and indents the continuation. Nothing but a width measurement separates the two forms: both are idempotent, and Prettier keeps its flat form stable while normalizing tsv's broken one back to it, so the divergence is one of normalization. The rule is uniform across the three argument printers — plain call, `new`, and member chain. A tail carrying its own **forced** break is outside it: the hug is refused and every argument breaks out, where both formatters print the same continuation indent ([expand_first_tail_arg_breaks](../tests/fixtures/typescript/expressions/calls/expand_first_tail_arg_breaks/)). See [expand_first_binary_tail_long](../tests/fixtures/typescript/expressions/calls/expand_first_binary_tail_long_prettier_divergence/).

**Non-null assertion in optional-chain parens**: A non-null `!` on a **bare** parenthesized optional chain — one with no trailing non-optional access — has redundant parens: the `!` is TypeScript-only and applies to the whole chain regardless (`(a?.b)!` ≡ `a?.b!`), so tsv strips them (`(a?.b)!` → `a?.b!`, and the `!`-inside form `(a?.b!)` → `a?.b!`). Prettier keeps the parens on `(a?.b)!` (it strips only `(a?.b!)`). tsv matches Biome. When a **non-optional access follows** (`(a?.b)!.c`), the parens are required — they seal the chain so `.c` isn't short-circuited (`(a?.b)!.c` vs `a?.b!.c`) — and there both formatters keep the parens and preserve the author's `!` placement (inside or outside: `(a?.b!).c` and `(a?.b)!.c` each stay as written), so that case is not a divergence. Content is identical (ASTs match); only the redundant parens differ. Fixtures: [optional_paren_non_null_bare](../tests/fixtures/typescript/expressions/chain/optional_paren_non_null_bare_prettier_divergence/) (the strip), [optional_paren_non_null_boundary](../tests/fixtures/typescript/expressions/chain/optional_paren_non_null_boundary/) and [optional_paren_non_null_inside](../tests/fixtures/typescript/expressions/chain/optional_paren_non_null_inside/) (required parens, both preserved).

**Return type generic union**: Prettier has special handling for `null` and `void` in union types within generic return types. When the second union member is `null` or `void`: (1) function declarations and class methods allow lines to exceed print width instead of breaking inside `<>`, (2) arrow functions break the assignment (`const fn =`) instead of breaking inside the return type. tsv breaks consistently inside the return type generic at the print width boundary regardless of type keyword.

**Arrow type param trailing comma**: For a generic arrow with a **single type param that has no constraint** (`<T>`, default-only `<T = string>`, or `const`-modified `<const T>`), Prettier forces a trailing comma — `<T,>` — via `shouldForceTrailingComma` (`language-js/print/type-parameters.js`). It does so to keep the output valid as TSX, where a bare `<T>` is ambiguous with a JSX element; the guard fires whenever the file is not known to end in `.ts`, which is always the case for a Svelte `<script>` body (prettier-plugin-svelte hands it to prettier without a `.ts` filepath). tsv has no JSX — it never emits TSX, and Svelte's own parser accepts bare `<T>` in every TS position (`<script>`, template `{...}`, `{@const}`) — so the disambiguation is vestigial and tsv emits the bare canonical form. Multi-param (`<T, U>`), constrained (`<T extends X>`), and empty (`<>`) type params are unaffected; prettier never forces the comma for those and tsv matches. The accepted tradeoff: in a mixed-tool repo prettier rewrites `<T>` back to `<T,>`, so the two ping-pong on this construct (reviewed and accepted — bare `<T>` is correct for a non-JSX formatter). Fixtures: [single_type_param](../tests/fixtures/typescript/expressions/arrow/generic/single_type_param_prettier_divergence/), [const_type_param_arrow](../tests/fixtures/typescript/typescript_specific/generics/const_type_param_arrow_prettier_divergence/), and — on async generic arrows — [async_generic/minimal](../tests/fixtures/typescript/expressions/arrow/async_generic/minimal_prettier_divergence/), [async_generic/forms](../tests/fixtures/typescript/expressions/arrow/async_generic/forms_prettier_divergence/) (optional-param, object-`as`-body, and a type-vs-value-position contrast that pins the comma to value position) and [curried_typed_callback](../tests/fixtures/typescript/expressions/arrow/curried_typed_callback_prettier_divergence/). The comment-relocation fixture [arrow_type_params_paren_comment](../tests/fixtures/typescript/declarations/function/arrow_type_params_paren_comment_prettier_divergence/) also exercises it.

**Empty-object comment bracket spacing**: An empty object whose sole body content is an interior comment keeps its bracket spacing in tsv — `{ /* c */ }` — where Prettier (since 3.9.5) tightens it to `{/* c */}`. The padding is the only difference; the comment itself stays exactly where the author wrote it in both formatters. tsv applies bracket spacing uniformly: any brace body kept on one line gets the ` … ` padding, a comment-only body included (a comment is content), so it is not special-cased on emptiness — a truly empty `{}`, with no content to space, stays tight in both. Bracket spacing is hardcoded in tsv, so this is a fixed design choice, not a configurable gap. The rule holds across every brace position, each pinned by a fixture: an object literal ([empty_block_comment](../tests/fixtures/typescript/expressions/objects/empty_block_comment_prettier_divergence/)), a destructuring pattern ([empty_comment](../tests/fixtures/typescript/expressions/destructuring/empty_comment_prettier_divergence/)), an enum body ([empty_comment](../tests/fixtures/typescript/declarations/enum/body_empty_comment_prettier_divergence/)), a type-alias body ([literal_body_empty](../tests/fixtures/typescript/types/comments/literal_body_empty_prettier_divergence/) — a line-comment body breaks multiline in both, no divergence there), a union/intersection member ([union_empty_object_member](../tests/fixtures/typescript/types/union_empty_object_member_prettier_divergence/) — a bare-`{}` member with no comment agrees in both and lives in the sibling non-divergence fixture), and a call/`new` type argument ([call_type_arg_empty_comment](../tests/fixtures/typescript/typescript_specific/generics/call_type_arg_empty_comment_prettier_divergence/)). All six route through the same builder (`build_empty_braces_inline_with_comments_doc`), so the padding is decided in one place. The type-argument case also pins a convergence: Prettier ≤3.9.4 broke a comment-bearing curly type argument's whole `<…>` list onto its own indented lines while tsv hugged it (`fn<{ … }>()`); Prettier 3.9.5 now hugs like tsv, so the only remaining difference there is the same block-comment bracket spacing (a line-comment body hugs in both).

**Optional rest parameter `?`**: A rest parameter written with an optional `?` marker (`(...a?)`, in a value signature, an interface call / construct signature, or a function type) is invalid TypeScript — tsc reports **TS1047** "a rest parameter cannot be optional". But that is a *deferred grammar-check* error, not a parse rejection: tsc's own parser stores the `?` on the parameter node regardless of the `...` and reports TS1047 later during checking (`checker.ts` `checkGrammarParameterList`), exactly like the already-deferred **TS1051** (`set x(a?)`). Per tsv's permissive-parser stance it accepts the syntax and preserves the token; acorn-typescript's AST likewise carries `optional: true` on the `RestElement` (never on `argument`). Prettier instead **strips** the `?` on every rest parameter (`(...a?)` → `(...a)`), silently deleting a token the source wrote. tsv preserves the author's `?`; plain rest (`...b`) is unaffected. A comment in the binding→`?` gap (`(...a /* c */?)`) stays before the marker in both formatters — the rest parameter takes the same comment landings the plain identifier parameter does — so the dropped `?` stays the only difference; prettier is non-idempotent on its own output there, pinned by the fixture's `audit_signature.txt`. Fixture: [rest_optional_param](../tests/fixtures/typescript/typescript_specific/rest_optional_param_prettier_divergence/).

**ES2015+ identifier property keys**: A property key that is a valid identifier renders unquoted (`{ 𐊧: 1 }`, not `{ '𐊧': 1 }`). tsv's identifier test uses the Unicode `ID_Start`/`ID_Continue` sets the ECMAScript grammar names (ecma262 §12.7 — `IdentifierName :: IdentifierStart`, `IdentifierStart :: UnicodeIDStart`; a well-formed `IdentifierName` is a `LiteralPropertyName` per §13.2.5), so it unquotes every key valid in **ES2015+**. Prettier unquotes only keys valid under **ES5** (a frozen legacy table), so it keeps an astral letter like `𐊧` (U+102A7 CARIAN LETTER, a valid `ID_Start` absent from ES5's table) quoted. The rule is position-scoped and never over-unquotes: object-literal keys, type-literal members, and interface members unquote; a key that is not a valid identifier (`'0a'`) stays quoted. tsv matches Biome. Fixture: [property_key_es2015_ident](../tests/fixtures/typescript/expressions/objects/property_key_es2015_ident_prettier_divergence/).

**Member-chain wide-last-argument hug convergence**: A member chain whose last call's single argument is **object-rooted** — a bare object literal, or an arrow whose grammar-parenthesized expression body is one — authored flat, too wide to fit on **any** chain line, while the chain's head fits. Prettier is **non-idempotent** here: pass 1's flat argument carries no forced break, so the chain's one-line measurement reads the whole flat content, overflows, and the chain expands, the argument breaking inside it; pass 2 re-reads that multiline object as authored-expanded (printObject's newline-after-`{` rule), truncates its fit measurement at the forced break, and collapses back to the flat chain with the argument hugging (`expr.fn1().map((item) => ({⏎…⏎}))`). Pass 3 holds that form, so prettier settles rather than cycling — the divergence is one of convergence **speed**, and tsv prints prettier's own settled pass-2 form in **one** pass, a single authoring-independent fixed point, in every position the chain can sit (initializer, call argument, property value, Svelte template expression). The window is exact and gated: when the argument **does** fit flat on the expanded chain's continuation line, the broken chain keeping it flat is the shared stable form and tsv prints that (the fixture pins the exact 100/101 boundary); when an **earlier** call in the chain takes a function argument, prettier's `lastGroupWillBreakAndOtherCallsHaveFunctionArguments` refusal makes the expanded chain its settled form, and tsv matches — that refusal is a chain-level force-expand in tsv too, covering the object, array and arrow-body kinds alike ([expand_last_arg_earlier_callback](../tests/fixtures/typescript/expressions/calls/chained/expand_last_arg_earlier_callback/), a non-divergence). Two neighbouring kinds are deliberately outside the window: a flat-authored **array** argument has no authored-multiline re-read rule, so prettier is stable at the broken chain and tsv matches; a **`new`/call wrapper** around an object reaches its settled form in two passes only via the inner object — a deeper discriminator tsv does not model, so tsv prints prettier's pass-1 output there and shares its two-pass convergence, agreeing with prettier at every pass ([last_arg_wrapped_object](../tests/fixtures/typescript/expressions/calls/chained/last_arg_wrapped_object/) pins the settled form both keep). Fixture: [last_arg_hug_convergence_long](../tests/fixtures/typescript/expressions/calls/chained/last_arg_hug_convergence_long_prettier_divergence/).

**Bodyless `declare global`**: `declare global;` (or closed by ASI) is one bodyless
`TSModuleDeclaration` in tsv — `global: true`, no `body`, acorn's shape and the shape tsv
already emits for the production's string-literal arm (`declare module 'a';`). Prettier prints
`declare;⏎global;`, two identifier expression statements, so its output **re-parses to a
different AST**.

Both oracles' module-declaration production admits the form: acorn's
`tsParseAmbientExternalModuleDeclaration` and tsc's `parseAmbientExternalModuleDeclaration` each
take `global` as the name and then `if (braceL) body else semicolon()`, the same branch the string
arm takes. Only tsc's *statement-level routing* diverts — `isDeclaration`'s `GlobalKeyword` case
requires `{`, an identifier or `export` after `global`, so a `;` sends the whole thing down the
expression-statement path. Prettier is not an independent witness: its `typescript` parser is tsc's.

Following that reading is not available to tsv, for the reason
[§tsv rejects what prettier formats](#tsv-rejects-what-prettier-formats) gives one construct over:
the split needs a semicolon between `declare` and `global`, two words on **one line** with no
`LineTerminator` between them, and no ASI rule may insert one. There tsv *rejects*, because acorn
rejects too and no reading is left; here acorn supplies one, so tsv takes it rather than refusing
to format the file. Without `declare`, a bodyless `global` stays an ordinary expression statement
in all three — the bare arm is a declaration head only when `{` follows it.

Behind `export` the shorthand is rejected by all three (`export` is left with nothing to attach
to), while the **bodied** `export global {}` / `export declare global {}` are a Svelte divergence
in the other direction — tsc and prettier take them, acorn's `export declare` allowlist omits
`global` alone. See [global_export](../tests/fixtures/typescript/declarations/namespace/global_export_svelte_divergence/)
and [conformance_svelte.md §TypeScript Corrections](./conformance_svelte.md#typescript-corrections).

**Class field key unquoting**: tsv applies that same "unquote a valid-identifier key" rule at **every** non-computed key position — object properties, type-literal / interface members, and every class member: method, accessor, static, and **field**. Prettier unquotes class method/accessor keys but leaves class *field* keys quoted (`'foo'() {}` → `foo() {}`, yet `'x' = 1` stays quoted), so under prettier an object property `{ 'x': 1 }` → `{ x: 1 }` while the class field `'x' = 1` does not — an inconsistency tsv removes. Unquoting is always meaning-preserving here: a valid-identifier key names the same member either way, and a string-keyed `'constructor'() {}` *is* the class's real constructor, so unquoting it to `constructor() {}` changes nothing. Non-identifier (`'0a'`, `'x-y'`) and escape-bearing keys stay quoted, and numeric keys are untouched. Fixtures: [field_key_unquote](../tests/fixtures/typescript/declarations/class/field_key_unquote_prettier_divergence/) (the field divergence — tsv unquotes where prettier keeps quoted), [member_key_unquote](../tests/fixtures/typescript/declarations/class/member_key_unquote/) (methods / accessors / static / `constructor`, a non-divergence where tsv matches prettier).

### Import-phase proposals

The **source-phase imports** and **import defer** proposals (`import source x
from 'mod'` / `import.source('mod')`, `import defer * as ns from 'mod'` /
`import.defer('mod')`) are a tsv-native parser divergence — acorn rejects them, so
they are **not** in the "Prettier rejects valid input" set above (that set is keyed
on acorn *accepting* the input). Prettier diverges one way:

- **`import source` — printer throws.** Prettier's `typescript` parser reads
  `source` as a binding name and throws (`'=' expected`). tsv parses and keeps the
  statement stable.

⚠️ **A second divergence used to be listed here and is GONE — `import defer` phase
drop.** Prettier once formatted `import defer * as ns from 'mod'` to `import * as ns
from 'mod'`, deleting the phase keyword and changing the import's semantics. At the
pinned prettier (3.9.6) it preserves the phase exactly, so the entry was a standing
false claim and is deleted rather than reworded. Two lessons the deletion is worth
keeping for: the entry was **documented-only by deliberate choice** — a live check was
declined as too costly — and a documented-only claim about an external oracle has no
gate, so it rots silently; and it rotted *behind* the "none of these can be fixtures"
belief below, which is what kept the fixture that would have caught it from existing.

The dynamic `import.source(…)` / `import.defer(…)` forms have no divergence —
prettier formats them identically to tsv. ⚠️ **"None of these can be fixtures" was
also wrong** and is corrected in
[conformance_svelte.md §Import-phase proposals](./conformance_svelte.md#import-phase-proposals):
a canonical-parser *rejection* is representable (`expected_ours.json` +
`expected_svelte.json` holding the parse-failure marker), and
[phase_keyword_comment](../tests/fixtures/typescript/modules/imports/phase_keyword_comment_svelte_prettier_divergence/)
is one. The remaining printer round-trips stay in `tests/import_phase.rs` and the
parser in the test262 suite. The `import source` throw is live-pinned by
[source_phase](../tests/fixtures/typescript/modules/imports/source_phase_svelte_prettier_divergence/),
whose `prettier_rejects.txt` carries the expected-error substring while
`expected_svelte.json` carries acorn's rejection — both oracles failing in one fixture.
See
[conformance_svelte.md §Import-phase proposals](./conformance_svelte.md#import-phase-proposals)
and [conformance_test262.md](./conformance_test262.md). **Upstream candidate**:
prettier import-phase support — promote to fixtures once it lands.

## Prettier rejects valid input

This input is **valid** by tsv's parse oracle (Svelte / acorn-typescript / `parseCss`) and our formatter keeps it stable, but prettier's parser/printer **throws** on it (the `typescript` parser for the TS/Svelte cases, postcss for the lone CSS one) — so there is no `output_prettier.*` oracle. The fixture carries a `prettier_rejects.txt` marker pinning the exact error; rule F6 live-verifies that prettier still rejects the input (failing loudly if the bug is fixed upstream or the error morphs). It reproduces in plain prettier (`parser: 'typescript'`, zero Svelte) and is fine under `babel-ts`; prettier-plugin-svelte routes `lang="ts"` formatting through the real `typescript` parser rather than `babel-ts`, so it surfaces there too.

- Optional chain to private field (`x?.#a`) — `An optional chain cannot contain private identifiers.` — [private_fields_optional_chain](../tests/fixtures/typescript/declarations/class/private_fields_optional_chain_prettier_divergence/)
- `<<` type-argument split in a class-extends clause (`class extends fn<<T>(v: T) => void> {}`) — `',' expected.` — [shift_left_class_extends](../tests/fixtures/typescript/expressions/binary/shift_left_class_extends_prettier_divergence/)
- `<<` split opening a type assertion (`<<T>() => R>x`) — `Expression expected.` — [shift_left_type_assertion](../tests/fixtures/typescript/expressions/binary/shift_left_type_assertion_prettier_divergence/)
- `<<` split in a `typeof` query's type arguments (`typeof f<<T>() => void>`) — `';' expected.` — [shift_left_typeof_query](../tests/fixtures/typescript/expressions/binary/shift_left_typeof_query_prettier_divergence/)
- `using`/`await using` cast (`using as T`, `(await using) satisfies T`) — `',' expected.` — [using/cast](../tests/fixtures/typescript/typescript_specific/using/cast_prettier_divergence/)
- Bare definite-assignment class property (`b!;` — no type annotation, no initializer) — `Declarations with definite assignment assertions must also have type annotations.` (TS1264; acorn-typescript defers the early error, tsv follows) — [property_definite_no_init](../tests/fixtures/typescript/statements/class/property_definite_no_init_prettier_divergence/)
- Ambient generator signature (`declare function* g(): Iterator<number>;`) — `Generators are not allowed in an ambient context.` (TS1221; a *checker* grammar error, not a parse error — see below) — [declare/function/generator](../tests/fixtures/typescript/typescript_specific/declare/function/generator_prettier_divergence/)
- Bodiless generator signature in a namespace body (`declare namespace N { function* g(): void; }`, and the plain-`namespace` spelling) — `A function signature cannot be declared as a generator.` (TS1221 / TS1222) — [namespace/generator_signature](../tests/fixtures/typescript/declarations/namespace/generator_signature_prettier_divergence/)
- Ambient `async` signature (`declare async function f(): Promise<void>;`) — `'async' modifier cannot be used in an ambient context.` (TS1040; **also** a Svelte divergence, acorn rejecting the bare form) — [declare/function/async](../tests/fixtures/typescript/typescript_specific/declare/function/async_svelte_prettier_divergence/), and with a comment in each gap of the head — the `declare`→`async` one being reachable from no other construct — [declare/function/async_keyword_comment](../tests/fixtures/typescript/typescript_specific/declare/function/async_keyword_comment_svelte_prettier_divergence/)
- `@supports (margin: 0))` — unbalanced-paren `@supports` prelude; prettier's CSS parser (postcss, not `typescript`) throws — `Unbalanced parenthesis` — [supports_unbalanced_paren](../tests/fixtures/css/at_rules/supports_unbalanced_paren_prettier_divergence/)
- `url(a\)b)` — unquoted `url()` with an escaped `)`; per CSS Syntax 3 §4.3.6 a url-token ends at the first *unescaped* `)`, so this is valid (parseCss accepts), but prettier's postcss miscounts the escaped `)` and throws — `Unbalanced parenthesis` — [url_escaped_paren](../tests/fixtures/css/values/functions/url_escaped_paren_prettier_divergence/)
- Own-line format-ignore directive before an **empty** class body (`class Aaa⏎// prettier-ignore⏎{}`) — prettier's own every-comment-printed assertion fires, because its empty-body path emits no member for the relocated directive to lead — `Comment "prettier-ignore" was not printed` — [body_prettier_ignore_empty](../tests/fixtures/typescript/class/body_prettier_ignore_empty_prettier_divergence/)

**Optional chain to private field**: `x?.#a` is valid modern JS (ecma262 `OptionalChain : ?. PrivateIdentifier`, from the private-fields-in-`in` era). typescript-estree rejects it; tsv keeps it stable. The comprehensive (prettier-formattable) private-field cases live in [private_fields](../tests/fixtures/typescript/declarations/class/private_fields/).

**`<<` type-argument splits tsc never makes**: a `<<` token can split into `<` `<`
when the first type argument is a generic function type (the sole `<`-initial
type) — acorn-typescript splits it wherever type arguments are legal, and tsv
follows acorn. tsc splits it only in some positions, so prettier's `typescript`
parser throws on the rest:

- **class-extends clause** (`class extends fn<<T>(v: T) => void> {}`): a heritage
  clause takes a `LeftHandSideExpression`, so an actual left-shift is impossible
  there and the split is unambiguous.
- **type assertion** (`<<T>() => R>x`): a statement cannot begin with an
  operand-less `<<`.
- **`typeof` query type arguments** (`type Y = typeof f<<T>() => void>`): a type
  position has no shift operator.

The positions where tsc does split — call, `new`, optional call, bare
instantiation, and type references — are prettier-formattable and covered as
ordinary fixtures in
[shift_left_vs_type_args](../tests/fixtures/typescript/expressions/binary/shift_left_vs_type_args/).

**Ambient generator / `async` signatures — a checker rule read as a parse rule**:
`declare function* g()`, a bodiless `function*` in a namespace body, and `declare
async function f()` are all barred by TypeScript (TS1221, TS1222, TS1040), but every
one of those is raised by tsc's **checker** — `checkGrammarFunctionLikeDeclaration`
calling `grammarErrorOnNode` — not by its parser, which builds the signature with
`asteriskToken` / `[DeclareKeyword, AsyncKeyword]` set and reports an **empty**
`parseDiagnostics`. So they are ambient-context early errors of exactly the family
tsv already defers (`declare` member bodies, initializers, decorators), and the
error code being a TS1xxx is not evidence to the contrary — the TS1xxx range spans
both parser and checker grammar diagnostics. typescript-estree promotes them to
parse failures, which is why prettier throws.

Deferring is also what makes tsv **self-consistent**: the same bodiless `function*`
signature already parsed in a `declare namespace`, a `declare global` and an overload
set, so rejecting it under a top-level `declare` was a hole in one position rather
than a rule. The one member of the family prettier *accepts* is a `declare class`
generator **method**, an ordinary fixture —
[declare/class/generator_members](../tests/fixtures/typescript/typescript_specific/declare/class/generator_members/)
— which is what shows the divergence tracks the bodiless *function signature*, not
generators in ambient contexts as such.

`async` keeps its `[no LineTerminator here]` throughout: `declare async⏎function
f(): void;` is not one ambient signature (tsc's modifier lookahead bails on the break
too), and the rule is enforced *before* the ambient reading is committed to.

**`using`/`await using` cast**: `using as T` / `(await using) satisfies T` is a
cast of the identifier `using` in acorn-typescript (and so in tsv); tsc instead
commits to a `using` *declaration* whose binding is named `as`/`satisfies` and
throws when no `=` follows. The reverse form (`using as = r;`) parses in tsc
and is rejected by acorn and tsv. Every other identifier-shaped word after
`using` is a binding attempt in both parsers — those cases are ordinary
`_svelte_divergence` fixtures (acorn has no `using` declarations at all); only
the cast keywords diverge from tsc, in tsv's favor of the drop-in oracle.

## tsv rejects what prettier formats

The reverse of the section above, and rarer: prettier parses and prints the input,
and tsv **refuses** it. The bar is high — tsv is first a formatter, so "prettier
formats it" is normally the accept test — and it is cleared here only because
accepting would require tsv's **grammar** to leave ECMAScript, which no amount of
oracle agreement buys.

**A `declare` head followed on the SAME line by a non-declaration word** —
`declare async⏎function f(): Promise<void>;`, `declare abstract⏎class B {}`,
`declare bar⏎function f(): void;`, `declare async⏎class B {}`. Prettier prints each
as three statements (`declare;` / `<word>;` / the declaration); tsv rejects all four
— [declare/line_break](../tests/fixtures/typescript/typescript_specific/declare/line_break/),
whose `input.svelte` pins the accepted side of the same boundary.

That split needs a semicolon between `declare` and the word after it, and
**ECMAScript does not insert one there**. All three ASI conditions
([§sec-rules-of-automatic-semicolon-insertion](https://tc39.es/ecma262/#sec-rules-of-automatic-semicolon-insertion))
require something absent here: a `LineTerminator` before the offending token, an
offending `}`, or the do-while `)`. The two words are on one line, so no production
admits the second and no semicolon may be inserted.

Prettier is not an independent witness to the contrary: its `typescript` parser is
typescript-estree, which wraps **tsc's own parser**, so prettier and tsc are one
engine here rather than two agreeing ones. And tsc grants the leniency to `declare`
**alone** — `abstract`, `async`, `public`, `export` and `readonly` all reject the
identical shape (`abstract bar⏎class B {}` is TS1434), while `declare bar⏎function`
is accepted with no syntactic diagnostic and both words resolved as ordinary
identifier references. A rule that applies to one modifier and not the five beside
it, with nothing to distinguish them, is an oracle slip rather than a judgement.
**acorn rejects all four**, so the `input_invalid_*` form pins the rejection on both
parsers.

This is the line: tsv freely defers *static-semantic early errors* to a diagnostics
layer — that is what makes it permissive, and what the ambient generator / `async`
entries above rest on. Inserting a semicolon the grammar forbids is not a deferral
but a change to the productions themselves, and it would leak past TypeScript
entirely: `declare` is an ordinary identifier in plain JS, so the same leniency would
make tsv accept `declare foo` unseparated in a `.js` file.

**`export default interface` split from its name** — `export default interface⏎A {}`.
Prettier welds the two lines into one same-line interface declaration, deleting the
line terminator; tsv rejects —
[exports/default_interface](../tests/fixtures/typescript/modules/exports/default_interface/),
whose `input.svelte` pins the accepted same-line form.

`interface` is a contextual keyword, and TypeScript's own rule is that it heads a
declaration only when its name follows on the same line (`isDeclaration` →
`nextTokenIsIdentifierOnSameLine`). tsc applies that rule at every sibling gap —
bare `interface⏎A {}` is TS1434, `export interface⏎A {}` is TS1128, `declare
interface⏎A {}` is TS1434 — and skips it at exactly one, the `export default`
route, which reads `parseInterfaceDeclaration` directly with no lookahead. Prettier
is again not an independent witness: its `typescript` parser *is* tsc's, so the two
are one engine. A rule spelled at three gaps and dropped at the fourth, with nothing
to distinguish them, is an oracle slip rather than a judgement — the same call made
for the `await`→`using` gap in
[conformance_svelte.md §TypeScript Corrections](./conformance_svelte.md#typescript-corrections).
**acorn rejects too**, so the `input_invalid_*` form pins the rejection on both
parsers.

Unlike the `declare` entry above, there is no reading to fall back on: `interface`
as the default-exported expression leaves `A {}`, where no line terminator separates
`A` from the `{`, so ASI cannot split it either. The choice is reject or weld, and
welding is the one thing the whole `[no LineTerminator here]` family exists to
prevent.

The rule generalizes past `declare`'s own gap to the heads that carry one of their
own: a `declare namespace`/`module` name must be on the keyword's line (tsc's
`nextTokenIsIdentifierOrStringLiteralOnSameLine`; `declare namespace⏎N {}` is TS1434),
and `abstract` must reach its `class` on one line. tsv and acorn reject **every**
spelling of those, and so does prettier wherever the leftover is not a statement
(`declare namespace⏎N {}`, `declare module⏎M {}`, `declare module⏎'a' {}` — tsc throws
`Unexpected keyword or identifier`), so those are ordinary `input_invalid_*` pins.

⚠️ Where the leftover *is* ASI-terminable, prettier prints it and the gap joins this
section rather than leaving it: `declare module⏎'a';` becomes `declare;` `module;`
`('a');`, `declare namespace⏎N;` becomes `declare;` `namespace;` `N;`, and the same
holds when a comment carries the break (`declare module // c⏎'a';`). That is the
identical slip one construct out — the words `declare` and `module` are on **one line**,
so the semicolon between them is one no ASI rule may insert — and acorn rejects with
tsv, so these ride as `input_invalid_*` files in
[declare/line_break](../tests/fixtures/typescript/typescript_specific/declare/line_break/)
beside the same-line-word cases above. The `[no LineTerminator here]` gate reads a
comment's own newlines, so a *multi-line* block comment in the gap is the break, not an
exemption from it.

What does **not** carry the restriction, and stays accepted everywhere: `declare
const⏎enum E {}`, `declare global⏎{}`, and a single-line comment wherever a break is
allowed.

**Behind `export` the same gap is not a divergence at all** — prettier and tsv both
reject `export declare⏎class B {}` (and every other head), because `export` is then
left with nothing to attach to. There the odd one out is **acorn**, which welds
across the break; that case is
[declare/export_line_break](../tests/fixtures/typescript/typescript_specific/declare/export_line_break_svelte_divergence/),
catalogued in
[conformance_svelte.md §TypeScript Corrections](./conformance_svelte.md#typescript-corrections).
So the two directions meet at one rule with different dissenters: without `export`,
prettier accepts a split ECMAScript forbids; with it, acorn accepts a weld tsc calls
fatal.

The boundary is exactly the line break, and tsv agrees with prettier on both sides of
it. A break **after** `declare` is ordinary ASI and parses as two statements in both
(`declare⏎function f(): void;`, `declare⏎async function f(): Promise<void>;` — the
fixture's `unformatted_asi` variant, which both formatters normalize to `input`).
Same-line spellings stay one ambient declaration in both (`declare abstract class B
{}`). Only the mixed form — same-line word, break before the declaration — diverges.

## TypeScript: Template Literals

For **value-position** interpolations `${...}`, tsv follows Prettier's heuristic (`template-literal.js` `printTemplateExpression`): a `${...}` is kept **on one line** unless its *source* already spans multiple lines, or the expression would render with a newline anyway (a nested function / block body). When neither holds the expression is **atomized** — rendered flat, unable to break — so the interpolation stays inline even past print width, exactly as Prettier does. Only when the interpolation *does* span lines does tsv break it, and then by expression type:

- **Qualifying types** (Identifier, MemberExpression, ConditionalExpression, BinaryExpression, SequenceExpression, TSAsExpression, TSSatisfiesExpression): softline wrapping at `${`/`}` boundaries — the group breaks when the line exceeds print width.
- **Non-qualifying types** (CallExpression, chains, ArrowFunction, etc.): no softlines at `${`/`}` — the expression breaks internally while `${`/`}` stays hugged.

Both paths match Prettier for the common value-position case — a bare `${prop}`, `${obj.field}`, a binary operand, a ternary consequent, all past print width with compact source, covered by the non-divergence [interpolation_boundary](../tests/fixtures/typescript/expressions/literals/template/interpolation_boundary/) and [interpolation_expression_inline_long](../tests/fixtures/typescript/expressions/literals/template/interpolation_expression_inline_long/) fixtures. A non-null-asserted member chain (`obj.a.b!`) qualifies exactly like the bare member (Prettier's `stripChainElementWrappers` peels the `!` before the member test; tsv mirrors it), covered by the non-divergence [interpolation_non_null_newline](../tests/fixtures/typescript/expressions/literals/template/interpolation_non_null_newline/) fixture. Two narrow value-position layout edges remain, a deliberate width break for template-literal **types**, and a deliberate carve-out for **jest-each** table alignment:

- Nested-deep chain / ternary — [interpolation_nested_deep_long](../tests/fixtures/typescript/expressions/literals/template/interpolation_nested_deep_long_prettier_divergence/)
- Nested template — [interpolation_nested_template](../tests/fixtures/typescript/expressions/literals/template/interpolation_nested_template_prettier_divergence/)
- Template literal type — [template_literal_type_long](../tests/fixtures/typescript/types/template_literal_type_long_prettier_divergence/)
- Template literal type (multibyte width) — [template_literal_type_multibyte_long](../tests/fixtures/typescript/types/template_literal_type_multibyte_long_prettier_divergence/)
- Type with conditional — [template_literal_type_conditional_long](../tests/fixtures/typescript/types/template_literal_type_conditional_long_prettier_divergence/)
- jest-each table alignment — [jest_each_table](../tests/fixtures/typescript/expressions/literals/template/jest_each_table_prettier_divergence/)
- Embedded-language tagged/decorator template kept verbatim — [embedded_language_verbatim](../tests/fixtures/typescript/expressions/literals/template/embedded_language_verbatim_prettier_divergence/)

**Atomization**: Prettier pre-renders each template expression at `printWidth: Infinity` (`template-literal.js`); if the result is single-line it replaces the expression doc with that atomic string, so the interpolation can never break. tsv reproduces this without re-rendering: when the interpolation source carries no newline and the expression doc has no forced break (`will_break` — a doc breaks at infinite width iff it holds a hardline / forced break), tsv strips the expression's lines (`atomize`) so `${...}` stays flat. Simple interpolations like `${prop}` or `${obj.field}` therefore stay inline in both formatters.

Flattening a `conditional_group` **collapses it to its least-expanded state**, because that is the state prettier's re-render at `printWidth: Infinity` would select. The expanded states are dead once every line is flattened, and keeping them is not merely redundant but wrong: render finds that no state fits at the *real* width, falls back to the most-expanded one, and emits its already-flattened separators as literal spaces (`xs.map( (i) => fn(i) )`) — or, where that state's separator was a `softline`, deletes a required one (`(i) =>fn(i)`). This is why the atomization guarantee holds for a last-argument **hug** (an arrow, object, or array argument), not just for simple interpolations — pinned by [interpolation_hug_arg_inline_long](../tests/fixtures/typescript/expressions/literals/template/interpolation_hug_arg_inline_long/), whose contrast case shows a newline *inside* the interpolation still breaking normally.

**Multiline indent**: For code-generation templates with indented, multi-line-source content, tsv applies Prettier's `addAlignmentToDoc` (ceiling division to match Prettier's useTabs rounding) and **matches Prettier** — single-level indent alignment is non-divergent, covered by the non-divergence fixture [interpolation_multiline_indent_long](../tests/fixtures/typescript/expressions/literals/template/interpolation_multiline_indent_long/). The residual divergence appears only in a **deeply nested** template: a member chain / ternary whose multi-line source overflows print width *at its nested visual position* breaks in tsv but stays inline in Prettier at **101–109 chars** (whose alignment reset measures the overflow as fitting — inconsistent with Prettier's own wrapping of the same overflow at normal nesting). An interpolation with no newline stays on one line in both regardless of width (atomized), so the overflow only reaches this divergence when the source spans lines.

**Nested template**: When a template expression contains an array literal wrapping a long inner template, tsv breaks the array to respect print width while Prettier keeps it inline.

**Template-literal types**: tsv treats print width as a hard limit for a `${...}` inside a template literal **type** — breaking after `${` with `}` on its own line — whereas Prettier keeps type interpolations inline regardless of length. Unlike the value-position case above (which matches Prettier), this is a deliberate tsv choice, covered by the three `template_literal_type_*` fixtures listed above.

**jest-each tables**: Prettier special-cases `` describe.each` `` / `` test.each` `` / `` it.each` `` tagged templates (`template-literal.js` `isJestEachTemplateLiteral`) — it parses the `` `…` `` body as a `|`-separated table and **re-aligns** it, padding every cell to its column's widest entry. tsv applies no jest-specific magic: a tagged template's body is preserved verbatim like any other template's quasi text (the `${...}` interpolations still format normally), so the table keeps the author's spacing. Deliberate — tsv treats every tagged template uniformly rather than detecting a testing framework by tag name. See [jest_each_table](../tests/fixtures/typescript/expressions/literals/template/jest_each_table_prettier_divergence/).

**Embedded languages kept verbatim**: the same uniform-tagged-template stance as jest-each, generalized. Prettier's `embeddedLanguageFormatting` reformats a tagged (or decorator) template's body when it recognizes the language by tag name — `` html`…` `` (collapsing embedded HTML whitespace), `` css`…` `` (expanding embedded CSS onto its own indented lines), `` graphql`…` ``, and more. tsv does not *yet* embed a sub-formatter for the languages inside string templates: the quasi text is opaque source, kept exactly as authored, while `${...}` interpolations still format normally. So tsv preserves the compact `` html`<div>  {{label}}  </div>` `` / `` css`.a{color:red}` `` bodies prettier reflows. This is current, transitional behavior — embedded-language formatting for tsv's own languages (CSS, Svelte/HTML), including prettier's comment-tagged form, is a planned feature; until it lands tsv keeps these bodies verbatim (the lossless interim stance). See [embedded_language_verbatim](../tests/fixtures/typescript/expressions/literals/template/embedded_language_verbatim_prettier_divergence/).

