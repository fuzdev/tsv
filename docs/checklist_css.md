# CSS Language Support

Comprehensive reference for CSS language features supported by tsv's parser and formatter.

## Coverage

Effectively all CSS features from stable W3C specifications are supported.
Early-draft features are covered under [Future Work](#future-work) — nearly all of them
already parse via generic handling; only two constructs are rejected outright.

**Scope & goals.** The north star is full **CSS-spec compliance**; the near-term,
enforced goal is **matching Svelte's `parseCss`** (the AST canonical). SCSS/Sass,
LESS, CSS Modules, PostCSS plugin syntax, YAML front-matter, and IE hacks are
**permanent non-goals** — out of scope, not unimplemented features. The parser
currently hard-fails on the first invalid construct; spec-style error recovery
(drop the bad rule, keep parsing) is future work toward spec compliance, not the
intended end state. See ./conformance_svelte.md (§CSS Parser Scope & Error Model).

**Spec References**:

- CSS specs: `../../csswg-drafts/`
- Machine-readable data: `../../webref/ed/css/`
- CSS Snapshot: `../../csswg-drafts/css-2026/` (the latest; prior years are siblings)

Sections below name the spec module but not its maturity level. Maturity moves, and a
hand-copied label rots silently — the CSS Working Group's [current
work](https://www.w3.org/Style/CSS/current-work) page is the live source, and each
module's `Overview.bs` carries its own `Status:` / `Work Status:`.

`css-properties-values-api` (the `@property` spec) is the one module cited below that
lives outside `csswg-drafts` — it belongs to the CSS Houdini Task Force
([w3c/css-houdini-drafts](https://github.com/w3c/css-houdini-drafts)). The webref data
covers it.

---

# Supported Features

## CSS Syntax Level 3

Foundation for all CSS parsing. Spec: `css-syntax-3`

### Tokenization

- Whitespace tokens — ASCII-only (tab/LF/FF/CR/space: css-syntax-3 §4.2's three, plus the `<CR>` and form feed §3.3's input preprocessing would have folded to a newline — tsv does not preprocess); non-ASCII Unicode whitespace (NBSP U+00A0, ideographic space, …) is value content, not a separator, so it is preserved inside a value token rather than collapsed
- Ident tokens (identifiers) — including unescaped non-ASCII code points (`#♥`, `#💩`; symbols and emoji ≥ U+00A0, matching Svelte's `>= 160` rule)
- Function tokens (`name(`)
- At-keyword tokens (`@name`)
- Hash tokens (`#name`, `#fff`)
- String tokens (single/double quoted)
- URL tokens (`url(...)`)
- Number tokens (integer, decimal, signed)
- Dimension tokens (number + unit)
- Percentage tokens
- Delim tokens (single characters)
- Colon, semicolon, comma tokens
- Block tokens (`{`, `}`, `[`, `]`, `(`, `)`)
- CDO/CDC tokens - SKIP: deprecated 1990s legacy, Svelte doesn't support

### Comments

- CSS comments (`/* comment */`)
- Multi-line comments
- Comments in **declaration values** — the value cannot become a doc (CSS value comments live
  outside the AST), so the printer re-emits its text; the comments keep their authored
  positions and everything between them still takes the value reader's rules, so a number, a
  hex color, a unit's case and a string's quote normalize exactly as they do without a comment
  (`printer/declarations.rs` → `value_normalization::normalize_value_with_comments`)
- Comments in selectors
- Comments in `:nth-*()` args — in every position: before the An+B term, **inside** it
  (`:nth-child(2n /* c */ + 1)`, either interior gap of the `['+' | '-'] <signless-integer>`
  tail), after it, around `of`, and after the `of` list. The term normalizes around the
  comment and the comment itself is opaque to the operator respacing, so its content is
  never rewritten
- Comments in **attribute selectors** — every juncture of the token-level
  `<attribute-selector>` production: after `[`, either side of the matcher, either side of
  the `i`/`s` flag, and before `]`. Spacing-safe there (the brackets bound the gap), so the
  comment is padded off its neighbours and glued to the brackets
- Comments between a selector's **sigil and the name it introduces** — `./* c */cls`,
  `:/* c */hover`, `::/* c */before`, `:/* c */:before`, `:/* c */not(.a)`. selectors-4
  forbids white space at exactly these junctures and a comment is no token, so it is
  admitted and stays **glued**; the pseudo-name case fold still runs, on the name only.
  Rejected by parseCss, and prettier's freeze diverges only on the fold, so a
  `_svelte_prettier_divergence`
- Comments splitting a **`<wq-name>` separator** (`svg/* c */|rect`, `[svg|/* c */attr]`) or
  an **`<attr-matcher>`** (`[attr~/* c */='value']`) — selectors-4 forbids white space
  between either production's components, and a comment is no token, so it is admitted and
  stays **glued** (a run included). Both are rejected by parseCss, so both are
  `_svelte_divergence`s
- Comments in `::slotted()` / `::part()` / unknown-pseudo args (leading/trailing gaps preserved; the interior positions — between `::part()` names, or `::slotted()` compound-internal — are rejected by parseCss but preserved + normalized by tsv, a `_svelte_prettier_divergence`)
- Comments in `:dir()` / `:lang()` / `::highlight()` identifier args (leading/trailing gaps preserved + normalized; parseCss accepts → a `_prettier_divergence`)
- Comments in a `@supports`/`@import` `selector()` argument — an argument that parses as a
  selector carries them through the selector printer, its list-comma seam included (a
  comment beside the comma keeps its side of it, `selector(.a /* c */, /* d */ .b)`; a
  `_prettier_divergence` on the spacing, like a rule's own list); one that doesn't is a
  `<general-enclosed>`, where no whitespace is inserted at all, so a glued comment stays
  glued on each side (a space would turn a compound into a descendant)
- Comments in declarations
- Comments in at-rules
- A prelude comment of a top-level at-rule whose previous sibling ends on the SAME line
  (`a{color:red}@supports /* c */ (display:grid){…}`, `@import 'a';@import /* c */ 'b';`)
  prints once, in its own prelude — the previous node's trailing claim stops at the next
  node's start
- Consecutive comments
- Nested comment closing (spec-compliant)
- Comments on the wire AST — `parseCss` hangs a flat, source-ordered `CSSComment[]` off both
  stylesheet roots (`StyleSheet` / `StyleSheetFile`), carrying a `position` offset on any
  comment it lifted out of a declaration `value` or an at-rule `prelude`. tsv emits the same
  set; the internal AST keeps those comments in three separate places, so the writer rebuilds
  the flat list (see [`crates/tsv_css/CLAUDE.md`](../crates/tsv_css/CLAUDE.md))
- `format-ignore` / `prettier-ignore` directive (`/* format-ignore */` emits the next rule/declaration verbatim — see [directives.md](./directives.md))

### Escapes

- Unicode escapes (1-6 hex digits: `\0001F4A9`)
- Backslash escapes (`\\`, `\"`, `\'`)
- Control character escapes (`\n`, `\t`)
- Escapes in identifiers (`.cl\61ss` → `.class`)
- Escapes preserved verbatim in at-rule preludes (`@keyframes \@mymove`, `\31 23` — serialized raw, not decoded)
- Escapes in strings
- Surrogate pair handling
- An escape's payload is content, not padding — a trailing escaped whitespace survives trimming and line-wrapping in every position (`width: 50px\ ;`, `url(x\ )`, `@layer a\ ;`, `.a\ , .b`), so the backslash never strands onto the following delimiter. Prettier corrupts these into forms that no longer parse; see [conformance_prettier_css.md §CSS: Values](conformance_prettier_css.md#css-values) and [§CSS: At-Rules](conformance_prettier_css.md#css-at-rules)
- A hex escape's optional whitespace terminator belongs to the escape and is absorbed exactly once (`\41 2px` is the single ident `A2px`, never `A` + `2px`)
- A paren that is *content* closes no function — one inside a quoted string (`fn('(', 0.1)`) or inside an escape (`fn(a\(b, 0.1)`) leaves the argument list balanced at the final unescaped `)`, so the value still parses as a function and normalizes (quote style, number form, colour case, and the list's own break). The four value scanners share that model, the leaf's function detector (`value/scan.rs::matching_close_paren`) included. Prettier's postcss counts them: it stops normalizing on `\(` and throws `Unbalanced parenthesis` on `\)` — [conformance_prettier_css.md §CSS: Values](conformance_prettier_css.md#css-values) and [conformance_prettier_ts.md §Prettier rejects valid input](conformance_prettier_ts.md#prettier-rejects-valid-input)
- A **bare** parenthesized group is a value group, not opaque text — `(1.50)`, `(a: 1.50)`, `(100vw - 1.50px)`, and one nested in a function (`calc((1.50px))`), parse their interiors, so their members take the same number / colour / string / unit rules and the same break shape as a named function's arguments (comma members one per line, space members filled). css-syntax-3 §"component value" calls it a **`()`-block** — a simple block whose associated token is the `(` — carrying the same "list of component values" a function does, which is prettier's model too (`parseValue` gives it a `function` node whose name is empty). The other two associated tokens stay opaque on both sides: `[1.50]` keeps its number, and a `{}`-block is the whole-value form the spec restricts to custom properties — [css/values/paren_group](../tests/fixtures/css/values/paren_group/), [css/values/paren_group_long](../tests/fixtures/css/values/paren_group_long/)
- A function's **name** is an ident sequence, escapes included, so an escape-spelled name is a real `<function-token>` and its arguments normalize like any other's (`\66 n(.10)` → `\66 n(0.1)`, `u\72 l("a.png")` → `u\72 l('a.png')`). The value parser's search for the opening `(` stays escape-blind and is sound because of the name rule rather than in spite of it: a `(` reached inside an escape leaves the name ending in a dangling `\`, which is not an ident sequence, so the value refuses and prints verbatim (`a\(b(c)` — which is also what prettier emits, by unbalancing). Prettier normalizes every escape spelling *except* the four whose raw payload byte drives its own tokenizer (`\(`, `\)`, `\"`, `\'`), so `a\28 b(.10)` normalizes there and `a\)b(.10)` does not — [conformance_prettier_css.md §CSS: Values](conformance_prettier_css.md#css-values)
- A function name's ident class is the **lexer's**, not `char::is_alphanumeric` — every code point at or above U+00A0, `parseCss`'s threshold. The value classifier re-reads a token the lexer already read, so a narrower class there refuses a name the lexer accepted and drops the whole value onto the verbatim `Identifier` path, silently switching off every normalization for that declaration: `a°(1.50)` kept its `1.50` where `aé(1.50)` normalized, and the same for `calc<NBSP>(1.50px)` and every look-alike space. Prettier reads those names as functions too, so each was a plain divergence — `css/values/functions/nonascii_symbol_name`, `css/values/boundary_nonascii_space_function`
- An at-rule **prelude**'s normalizers step escapes whole too, so an escape's payload is never read as the delimiter, hash, comment introducer or paren one of them is scanning for: `@supports (a: x\#FFF)` keeps its ident, `@supports (a: x\"y "z")` normalizes only the real string, `url(x\)1.50)` stays opaque past the escaped paren, and `@media (MIN-WIDTH: a\(b\)c)` is still a *simple* feature expression whose name lowercases. Prettier reads each payload byte as structure and corrupts a different way at each — [conformance_prettier_css.md §CSS: At-Rules](conformance_prettier_css.md#css-at-rules)
- A media-feature **name** is one ident sequence too, so it runs *through* every escape it carries — including one spelling the `:` that would otherwise look like the name→value separator (`@media (A\:B: 1px)` is the single name `A:B`). Two rules key on the whole name and a scanner stopping at the `\` got both wrong on the fragment after it: a plain name is a pre-defined keyword and so ASCII case-insensitive (css-values-4), while a custom-media name is an `<extension-name>` (css-extensions-1) with no canonical casing to fold to, so `@media (--MIN\-WIDTH: 1px)` half-lowercased. Case-sensitivity is a property of the **whole** name, asked once (`feature_name_preserves_case`, mirroring prettier's `maybeToLowerCase`), never per identifier run — `(--A B: 1px)` kept `--A` and folded `B` when it was asked per run. Prettier reads the escaped `:` as the separator and splits the ident instead (`(a\: B: 1px)`), a divergence — [conformance_prettier_css.md §CSS: At-Rules](conformance_prettier_css.md#css-at-rules)
- ⚠️ **An escape-spelled `url()` name in a prelude is NOT recognized, and that is parity rather than a gap.** `@supports (a: u\72 l(1.50))` normalizes to `1.5` on both sides, and `u\72 l(#FFF)` lowercases on both: prettier matches the name literally too, so teaching the prelude's emitting run the ident sequence's real extent would leave tsv the only one preserving them. The run must not be widened for a second reason — an ident sequence's real extent pulls the digits after a hex escape into the ident, and the number arm would then merge its leading zero onto it (`\41 2.50px` → `\41 20.5px`, the ident `A20`). The parser's own recognition, which *does* decode the escape (`printer::values::function_name_is`), answers a different question: an `@import` prelude the lexer tokenized, not this raw text
- A number **merges** with the run before it, so a prelude copies the pair verbatim: `x1.50` is the ident `x1` then `.50`, and giving the number its canonical leading zero handed the `0` to the ident (`x10` then `.5` — two different tokens). A signed number never merges, so `x+1.50` still normalizes. *Which* runs merge is one of the four things prettier's **two prelude readers** disagree about, so it is keyed on the reader rather than settled once: `parseValue` (`@supports`, `@import`) takes the word through to its end, so an ident, a `<hash-token>` and a number's own unit all absorb (`1a.50`, `#1.50`, `1.5.50` verbatim) — "its end" being postcss-values-parser's tokenizer plus `splitWord`'s gluing of consecutive word tokens, whose word-end set depends on the word's own first character (`x-1.50` is one word, `9-1.50` two), not a character class — while `parseMediaQuery` (`@media`) runs `adjustNumbers`' `(WORD_PART)?(NUMBER)(UNIT)?` regex, where only a word part absorbs and the number re-splits (`1a0.5`, `#1.5`, `1.50.5`, and `$1.50` → `$1.5`, since `$` heads no word part). The other two are the unit gate (the value reader has none, so `1.50abc` → `1.5abc`; the media reader keeps an unknown unit's whole match) the hex fold (`#FFF` → `#fff` on the value reader only, asked of the whole `#` word — `#FFF.5` is no colour and is preserved by both), and *where the media reader runs at all* (`adjustNumbers` prints only `media-type` and `media-value`; a `media-feature` name — the whole interior of a boolean or range form — takes the case fold and no number pass) — the first three pinned by [prelude_number_adjacent_number](../tests/fixtures/css/at_rules/prelude_number_adjacent_number/), [prelude_hash_adjacent_number](../tests/fixtures/css/at_rules/prelude_hash_adjacent_number/) and [import_prelude_value_path](../tests/fixtures/css/at_rules/import_prelude_value_path/). ⚠️ `@import` is on the **value** side of that split, not the media side its grammar suggests — prettier's `isModuleRuleName` routes it to `parseValue`, which is also why its media-feature *names* are not lowercased
- A hex escape's terminator is part of the escape at a function name's `(` too, so `c\41 (1px)` keeps its space: the ident sequence ends flush with the paren and the run is one `<function-token>`, the same restore a property name makes against its colon (`\41 : red`). Whether the author wrote the terminator is preserved either way — `(` is no hex digit, so `c\41(` and `c\41 (` are the same token. Prettier writes one in regardless, which is lossless on that pair but turns `c\41x(` (an escape ended by a literal, needing no terminator) into an ident plus a paren — [conformance_prettier_css.md §CSS: Values](conformance_prettier_css.md#css-values)
- Non-CSS whitespace is value content, not separator — an NBSP or U+3000 in a **value** survives verbatim, and so does one glued to a selector NAME (`.a<NBSP>, .b` keeps its class name: `read_identifier` reaches it first)
- …but the same code point at a selector **boundary** is a separator, because `parseCss` reaches it through `allow_whitespace()` (JS `\s`) instead — a selector-list start, each `,`, the compound break, a combinator's gaps, a pseudo-argument list's start and its `)`, the attribute selector's interior and tail, and a declaration's property→colon gap. It leaves the AST there and the printer re-emits the bytes at every one of those junctures, so nothing is lost either way (one position still drops it — the stylesheet's trailing whitespace — ratcheted in that test). Where css-syntax-3 reads the same run as identifier content — an attribute selector's tail, the property gap — the bytes are the claim: a bare value glued to a run stays bare (`[attr=value<NBSP>]`), a flag glued to a run stays glued, and ASCII whitespace beside a run keeps its presence (`css/selectors/attribute/tail_nonascii_space_prettier_divergence`, `css/declarations/property_nonascii_space_prettier_divergence`). Which reader wins is decided by ORDER, never by the class: [conformance_svelte.md §Boundary whitespace](conformance_svelte.md), pinned by [css_boundary_whitespace.rs](../tests/css_boundary_whitespace.rs)

---

## CSS Selectors Level 4

Spec: `selectors-4` (core features are REC via CSS2.1)

### Basic Selectors

- Type selectors (`div`, `span`)
- Universal selector (`*`)
- Class selectors (`.class`, chained `.foo.bar`)
- ID selectors (`#id`, with type `div#id`)
- Compound selectors (`div.class#id`)

### Combinators

- Descendant combinator (space)
- Child combinator (`>`)
- Adjacent sibling combinator (`+`)
- General sibling combinator (`~`)
- Column combinator (`||`) - _moved to Selectors Level 5 by the CSSWG_
- Leading combinator (`> .a`, `+ .a`) - accepted in every context; contextual invalidity deferred to diagnostics
- Consecutive combinators (`> > .a`, `+ ~ .d`) - preserved (parseCss collapses the run); see [conformance_svelte.md §CSS Corrections](conformance_svelte.md#css-corrections)

### Attribute Selectors

- Presence (`[disabled]`)
- Exact match (`[type="text"]`)
- Prefix match (`[href^="https"]`)
- Suffix match (`[href$=".pdf"]`)
- Substring match (`[title*="example"]`)
- Word match (`[class~="active"]`)
- Language match (`[lang|="en"]`)
- Case-insensitive flag (`[attr="value" i]`)
- Case-sensitive flag (`[attr="value" s]`)
- A non-ASCII boundary run in the tail (`[attr=value<NBSP>]`, `[attr=value<NBSP>i]`, `[attr='value' s<NBSP>]`) — the value and the flag end where `parseCss` ends them (`value` is `value`, `flags` is `i`/`s`), and the bytes are kept as authored (`tail_nonascii_space_prettier_divergence`); the wire's `value` is `read_attribute_value`'s trimmed one (`[attr=' value ']` → `value`)
- Namespace prefix (`[svg|href]`)
- Universal namespace (`[*|attr]`)
- No namespace (`[|attr]`)

### Namespace Selectors

- Qualified name (`svg|rect`)
- Universal namespace (`*|div`)
- No namespace (`|div`) - Svelte doesn't support (divergence)

### Structural Pseudo-Classes

- `:root`
- `:empty`
- `:first-child`, `:last-child`
- `:only-child`
- `:first-of-type`, `:last-of-type`
- `:only-of-type`
- `:nth-child(An+B)`
- `:nth-last-child(An+B)`
- `:nth-of-type(An+B)`
- `:nth-last-of-type(An+B)`
- `:nth-col(An+B)`, `:nth-last-col(An+B)` - Level 4, table columns
- `:nth-child(An+B of selector)` - Level 4
- Non-ASCII whitespace (`<NBSP>`, `<ZWNBSP>`, every `Zs`, `<LS>`, `<PS>`) at every An+B
  juncture — before the `)`, on either side of the tail's operator, around `of`, and after
  the `of` list — is the boundary run `parseCss` skips there (its `REGEX_NTH_OF` is a JS
  regex): the term parses as the `Nth` canonical reads and the character stays where it
  was written, flush against its neighbour, with the ASCII spacing regenerated around it
  (`2n<NBSP> + <NBSP>1`, `2n + 1<NBSP>)`, `of<NBSP>.class`). Both grammars behind the
  scanner take their own class: Svelte's JS `\s` for the `:is()`-style term, the parser's
  boundary class for the `:nth-*()` term. The ASCII half of the class — VT and FF included,
  as Svelte's `\s` has them — is regenerated instead: one space around the operator and
  `of`, none against the parens (prettier freezes those bytes; `_prettier_divergence`)

### Logical Pseudo-Classes

- `:is(selector-list)` - forgiving
- `:where(selector-list)` - forgiving, zero specificity
- `:not(selector-list)`
- `:has(relative-selector-list)` - relational

### User Action Pseudo-Classes

- `:hover`
- `:active`
- `:focus`
- `:focus-visible`
- `:focus-within`

### Input Pseudo-Classes

- `:enabled`, `:disabled`
- `:read-only`, `:read-write`
- `:placeholder-shown`
- `:default`
- `:checked`
- `:indeterminate`
- `:valid`, `:invalid`
- `:in-range`, `:out-of-range`
- `:required`, `:optional`

### Link/Location Pseudo-Classes

- `:link`, `:visited`
- `:any-link`
- `:local-link`
- `:target`
- `:target-within`
- `:scope`

### Directional/Language Pseudo-Classes

- `:dir(ltr)`, `:dir(rtl)`
- `:lang(en)`, `:lang(en-US)`

### Tree-Structural Pseudo-Classes

- `:defined`

### Modern Pseudo-Classes (Level 4+)

Parsed via generic pseudo-class handling.

- `:user-valid`, `:user-invalid`
- `:autofill`
- `:modal`, `:fullscreen`, `:popover-open`, `:open`
- `:playing`, `:paused`, `:seeking`, `:muted`
- `:host`, `:host()`, `:host-context()`
- `:current`, `:current()`, `:past`, `:future` (time-dimensional)
- `:blank` (empty-value, at-risk in spec)

### Pseudo-Elements (Standard)

- `::before`, `::after`
- `::first-line`, `::first-letter`
- `::marker`
- `::placeholder`
- `::selection`

### Pseudo-Elements (Shadow DOM)

Spec: `css-shadow-1` (CSS Shadow Module Level 1 — the module formerly named CSS Scoping Level 1, which also absorbed CSS Shadow Parts; `css-scoping-1` is now a redirect)

- `::slotted(selector)`
- `::part(name)`

### Modern Pseudo-Elements

Parsed via generic pseudo-element handling.

- `::highlight(name)`, `::spelling-error`, `::grammar-error`
- `::view-transition-*` (View Transitions API)
- `::file-selector-button`
- `::slider-thumb`, `::slider-track`
- `::backdrop` (top layer elements)
- `::search-text`, `::target-text` (text highlighting)

---

## CSS Values and Units

Specs: `css-values-3`, `css-values-4`

### Numbers and Dimensions

- Integers
- Decimals (with/without leading zero)
- Signed numbers (`+`, `-`)
- Percentages
- Scientific notation (`1e10`)

### Absolute Length Units

- `px`, `cm`, `mm`, `in`, `pt`, `pc`, `Q`

### Font-Relative Length Units

- `em`, `rem`, `ex`, `ch`
- `cap`, `ic`, `lh`, `rlh` (Level 4)
- Root-relative: `rex`, `rch`, `rcap`, `ric` (Level 4)

### Viewport Units

- `vw`, `vh`, `vmin`, `vmax`, and the logical `vi`, `vb` (Level 4)
- Small viewport: `svw`, `svh`, `svi`, `svb`, `svmin`, `svmax` (Level 4)
- Large viewport: `lvw`, `lvh`, `lvi`, `lvb`, `lvmin`, `lvmax` (Level 4)
- Dynamic viewport: `dvw`, `dvh`, `dvi`, `dvb`, `dvmin`, `dvmax` (Level 4)

### Container Query Units

Spec: `css-conditional-5` (container queries and their units moved here; `css-contain-3` is now a placeholder)

- `cqw`, `cqh`, `cqi`, `cqb`, `cqmin`, `cqmax`

### Other Units

- Angles: `deg`, `rad`, `turn`, `grad`
- Time: `s`, `ms`
- Frequency: `hz`, `khz`
- Resolution: `dpi`, `dppx`, `dpcm`, `x` (the `dppx` alias)
- Flex/grid fraction: `fr`
- Ratio: `16/9`, `4/3`

### Math Functions

- `calc()`
- `min()`, `max()`, `clamp()`

### Advanced Math Functions (Level 4)

- `round()`, `mod()`, `rem()`
- `abs()`, `sign()`
- Trigonometric: `sin()`, `cos()`, `tan()`, `asin()`, `acos()`, `atan()`, `atan2()`
- `sqrt()`, `pow()`, `hypot()`, `log()`, `exp()`

### URL Values

- `url()` with quoted string (quote normalized `"…"` → `'…'`)
- `url()` with unquoted string — opaque content preserved verbatim, incl. `?` query
  strings, `url(#anchor)`, and trailing/empty comma segments (`url(a,b,)`, `url(c,,d)`);
  surrounding whitespace trimmed
- Data URIs
- `@import url(<unquoted>)` (e.g. `@import url(a.css?x=1)`)
- `@namespace url(<unquoted>)` (e.g. `@namespace svg url(http://www.w3.org/2000/svg)`) —
  opaque content preserved verbatim, incl. the `://` colon

### unicode-range

- Single codepoint `U+26`, range `U+0-7F` / `U+0025-00FF`, wildcard `U+4??`

### Cascade Keywords

- `inherit`, `initial`, `unset`
- `revert` (Cascade Level 4)
- `revert-layer` (Cascade Level 5)

### Declaration Modifiers

- `!important`

---

## CSS Custom Properties

Spec: `css-variables-1`

- Custom property declaration (`--name: value`)
- Empty custom property (`--name:;`) — value grammar is `<declaration-value>?`;
  every spacing variant trims to the same empty value, normalized to a single
  space (`--name: ;`), a prettier divergence (see `conformance_prettier_css.md` →
  CSS: Values)
- Empty custom property with `!important` (`--name: !important;`) — also
  normalizes to a single space; prettier is non-convergent here (grows a space
  per pass), so it's guarded by a `.css` fixture (no prettier oracle)
- `var()` basic usage
- `var()` with fallback
- `var()` empty fallback (`var(--a,)`) — the trailing comma is significant
  (substitutes nothing when unset, unlike `var(--a)`) and is preserved; so is the
  same **closing** comma anywhere else in a declaration value (`transition: a,`,
  `rgb(1, 2, 3,)`, `--x: a,`), one rule rather than a `var()` carve-out, a
  prettier divergence (see `conformance_prettier_css.md` → CSS: Values, "Closing
  comma in a value"). An *escaped* comma (`var(--b, x\,)`) is content and closes nothing
- Nested fallbacks (`var(--a, var(--b, red))`)
- Composition with `calc()`

---

## CSS Colors

Specs: `css-color-3`, `css-color-4`, `css-color-5` (Level 5 is widely shipped)

### Named Colors

- Standard named colors (140+)
- `transparent`
- `currentColor`

### Hex Colors

- 3-digit (`#rgb`), 4-digit (`#rgba`)
- 6-digit (`#rrggbb`), 8-digit (`#rrggbbaa`)

### RGB/RGBA

- Legacy comma syntax (`rgb(255, 0, 0)`)
- Modern space syntax (`rgb(255 0 0)`)
- Alpha with slash (`rgb(255 0 0 / 50%)`)
- `none` keyword for missing components

### HSL/HSLA

- Legacy comma syntax
- Modern space syntax
- Alpha with slash
- `none` keyword

### Modern Color Functions (Level 4)

- `hwb()`
- `lab()`, `lch()`
- `oklab()`, `oklch()`
- `color()` with color spaces

### Color Level 5

- `color-mix(in colorspace, color1, color2)`
- Relative color syntax
- `light-dark()` function

---

## CSS At-Rules

> **Note**: At-rule preludes are parsed at three levels. **Structured**: `@supports` and
> `@container` (conditions, for line-width wrapping — with a raw fallback when the prelude
> isn't a valid condition, which fires on what the condition reader *cannot consume* rather
> than on validity as such, so a malformed prelude the reader consumes whole stays structured
> and owes its leftovers back — see the operator-binds-right rule in
> [`../crates/tsv_css/CLAUDE.md`](../crates/tsv_css/CLAUDE.md); a `selector()` argument parses
> one level deeper, as the selector the grammar says it is, and prints through the selector
> printer), `@import` (url/string +
> `layer()`/`supports()`/media, falling back to raw when it doesn't lead with a url/string —
> its `supports()` argument is a `<supports-condition>`, read and printed by the `@supports`
> machinery above), and `@scope` (forgiving selector lists).
> **Raw text**: `@media` — kept verbatim to preserve comments, with the printer locating
> `and`/`or` boundaries at print time for wrapping. **Raw text** likewise for everything else
> (`@keyframes`, `@layer`, `@page`, …), since they have no `property: value` / media-query
> grammar; `@namespace` is the exception that takes a normalizing path. In every case the
> public AST stays source-verbatim. Full structured parsing (range syntax, media features) may
> be added for tooling use cases (linting, type checking).

### Core At-Rules

- `@charset`
- `@import` (basic)
- `@import` with media-query condition (media-type-led `screen and (…)` or a bare `<media-condition>` `(max-width: 40px)`)
- `@import` with a comma-separated `<media-query-list>`, including an empty entry (`, screen`, `screen, , print`)
- `@namespace`
- `@media`
- `@page`
- `@font-face`
- `@keyframes`

### Conditional Rules

- `@media` with boolean logic (`and`, `or`, `not`)
- `@media` range syntax (`width >= 768px`)
- `@supports` (feature queries)
- `@supports selector()`

### Cascade Layers

- `@layer`
- `@import` with `layer()` condition
- `@import` with `supports()` condition

### Container Queries

Spec: `css-conditional-5` (which also adds `@when` / `@else`)

- `@container`
- `@container` with logical operators

### Custom Properties API

Spec: `css-properties-values-api`

- `@property` at-rule
- `syntax`, `inherits`, `initial-value` descriptors

### Anchor Positioning

Spec: `css-anchor-position-1` (Chromium shipped)

- `anchor()` function
- `anchor-size()` function
- `@position-try` at-rule

### Modern At-Rules (Widely Shipped)

- `@starting-style`
- `@scope`
- `@counter-style`
- `@font-feature-values`
- `@font-palette-values`
- `@color-profile`

### Experimental At-Rules

Parsed via generic at-rule handling.

- `@when`, `@else` (Conditional Rules Level 5)
- `@view-transition`
- Vendor-prefixed at-rules (`@-webkit-*`, `@-moz-*`) — except the `@keyframes` family
  (`@-webkit-keyframes`, `@-moz-keyframes`, `@-o-keyframes`, `@-ms-keyframes`), which
  gets the same structured keyframe-selector block parsing as bare `@keyframes`

---

## CSS Nesting

Spec: `css-nesting-1` (widely shipped)

- Nesting selector `&`
- `&` with combinators (`& > .child`)
- `&` with pseudo-classes (`&:hover`)
- `&` with pseudo-elements (`&::before`)
- Implicit nesting (`.parent { .child { } }`)
- Deep nesting (3+ levels)
- Nested at-rules (`@media`, `@supports` within rules)
- Conditional group at-rules nested in each other within a rule
  (`.s { @media … { @supports … { … } } }`) — nesting context propagates so the
  innermost block accepts bare declarations

---

## CSS Functions

### Gradients

Spec: `css-images-3`

- `linear-gradient()`
- `radial-gradient()`
- `repeating-linear-gradient()`
- `repeating-radial-gradient()`

### Images

Specs: `css-images-3`, `css-images-4`

- `conic-gradient()`, `repeating-conic-gradient()`
- `image-set()`
- `cross-fade()`

### 2D Transforms

Spec: `css-transforms-1`

- `translate()`, `translateX()`, `translateY()`
- `scale()`, `scaleX()`, `scaleY()`
- `rotate()`
- `skew()`, `skewX()`, `skewY()`
- `matrix()`

### 3D Transforms

Spec: `css-transforms-2`

- `translate3d()`, `translateZ()`
- `scale3d()`, `scaleZ()`
- `rotate3d()`, `rotateX()`, `rotateY()`, `rotateZ()`
- `perspective()`
- `matrix3d()`

### Individual Transform Properties

Spec: `css-transforms-2`

- `translate`, `scale`, `rotate` properties

### Filter Effects

Spec: `filter-effects-1`

- `blur()`, `brightness()`, `contrast()`
- `drop-shadow()`, `grayscale()`, `hue-rotate()`
- `invert()`, `opacity()`, `saturate()`, `sepia()`

### Shapes

Spec: `css-shapes-1`

- `circle()`, `ellipse()`, `polygon()`, `inset()`
- `path()` (SVG path syntax)

### Grid Functions

Spec: `css-grid-1`

- `minmax()`
- `repeat()`
- `fit-content()`
- Named grid lines (`[name]`)
- Multi-row string values on `grid` / `grid-template*` are **source-position-dependent** —
  the one place CSS formatting reads the author's line breaks. Consecutive string values
  written on different source lines wrap one-per-line; the same values written inline stay
  inline (`grid-template-areas: 'a a' 'b b'`). Matches prettier
  (`comma-separated-value-group.js`)

### Easing Functions

Spec: `css-easing-1`

- `linear` keyword
- `ease`, `ease-in`, `ease-out`, `ease-in-out` keywords
- `cubic-bezier()` function
- `steps()` function
- `step-start`, `step-end` keywords
- `steps()` positions: `jump-start`, `jump-end`, `jump-none`, `jump-both`, `start`, `end`

### Scroll-Driven Animations

Spec: `scroll-animations-1` (Chromium shipped)

- `scroll()` function
- `view()` function

### Environment Variables

Spec: `css-env-1` (widely supported)

- `env()` function
- Safe area insets

---

## Generic Property Handling

> **Architectural Note**: The parser uses generic declaration parsing. All CSS properties work automatically without explicit implementation.

- Standard properties parse correctly
- Shorthand properties parse correctly
- Unknown properties parse correctly (forward compatibility)

---

## Forward Compatibility

Features that parse correctly through generic handling.

- Unknown pseudo-classes parse correctly
- Unknown pseudo-elements parse correctly
- Unknown at-rules parse correctly
- Unknown functions parse correctly
- Unknown units parse as dimensions

---

# Future Work

## Not Parsed

The constructs tsv rejects outright:

- Reference combinator (`/ref/`, `selectors-5`) for IDREF-based relationships — no parse support
- A comment splitting a **column combinator** (`col |/* c */| td`) — rejected, because tsv
  lexes `||` as one token. css-syntax-3 gives U+007C no case in *consume a token*, so the
  spec reads it as two `<delim-token>`s that a comment may sit between. The **whitespace**
  form (`col | | td`) rejects too, and there that is the *correct* answer rather than a
  side effect: the column combinator lives in selectors-5, whose grammar
  (`<combinator> = '>' | '+' | '~' | [ '|' '|' ] | [ / <wq-name> / ]`) adds its own *white
  space is forbidden* rule — "Between the components of a `<combinator>`" — so only the
  comment form is a gap. Prettier is a usable oracle for the comment form only: its
  selector parser gives up on **any** comment and freezes the selector verbatim, so
  `col |/* c */| td` comes back byte-identical and stays so on a second pass. For the
  whitespace form it has none — it rewrites `col | | td` to `col|td`. Svelte's `parseCss`
  rejects both spellings, so closing the gap is a *shared* over-rejection with no fixture
  shape today. The **last** spec-valid selector position tsv rejects a
  comment in; every other one is listed under [Comments](#comments). An ident glued to `(`
  (`:not/* c */(`) is a single `<function-token>` and is correctly rejected, not a gap —
  and `#id` is a single `<hash-token>`, so it has no juncture to split either

## Parsed Generically, Not Modeled

Everything below **parses today** — the generic at-rule, pseudo-class/element, and
declaration-value paths accept it, format it, and round-trip it. What it lacks is
*structural* modeling, which is the same footing as `@view-transition` and the
[Modern Pseudo-Classes](#modern-pseudo-classes-level-4)
already listed under Supported.

For the value-level entries that is the intended end state rather than a gap:
a declaration value is a balanced token scan and
[property value validation is out of scope](#out-of-scope), so a new function needs
no per-function grammar. The list exists because these specs are still unstable —
if a construct ever earns dedicated modeling (a real AST shape, not just acceptance),
the spec settling is the gate.

| Spec | Constructs |
| --- | --- |
| CSS Mixins 1 (`css-mixins-1`) | `@function` (with its `result:` descriptor, parameters, and `returns` type), dashed-function calls (`--custom-fn()`), `@mixin`, `@apply`, `@contents` |
| CSS Grid 3 (`css-grid-3`) | `display: grid-lanes` / `inline-grid-lanes`, `flow-tolerance` — ordinary value keywords and a generic declaration. (This is the module formerly specced as `grid-template-*: masonry`; the CSSWG renamed the model to *grid lanes*, and the `masonry-*` anchors survive only as `oldids`.) |
| CSS Selectors 5 (`selectors-5`) | `:local-link(n)`, `:state(identifier)`, `:heading`, `:heading(level)` |
| CSS Conditional Rules 5 (`css-conditional-5`) | `@when`, `@else` |
| CSS Easing 2 (`css-easing-2`) | `linear()` with control points |
| CSS Values 5 (`css-values-5`) | `attr()` with type casting/fallbacks, `calc-size()`, `progress()`, `random()`, `first-valid()`, `if()`, `toggle()`, `sibling-count()`, `sibling-index()` |
| CSS Color 6 (`css-color-6`) | `color-layers()`, `contrast-color()` |
| CSS View Transitions 2 (`css-view-transitions-2`) | Cross-document transitions — no new syntax over the Level 1 `@view-transition` |
| CSS Conditional Values 1 (`css-conditional-values-1`) | `true` / `false` values, comparison operators in values, boolean logic in values |
| CSS Forms 1 (`css-forms-1`) | `appearance: base`, `::picker()`, `::field-text`, `::clear-icon` |

An unknown pseudo-class/element argument is not held opaque — `parse_unknown_args` first
tries the content as a complex selector list and only falls back to a paren-balanced raw
skip. So `:state(foo)`'s `foo` lands as a `TypeSelector` and `:heading(1)`'s `1` as an
`Nth`. That is not a divergence: `parseCss` produces the identical shape, and matching it
is the enforced goal.

---

# Out of Scope

These are outside the parser/formatter's responsibility:

- Preprocessor languages (Sass/SCSS, Less, Stylus) and PostCSS plugin syntax
- CSS Modules (stylesheet-mode `:global`, `composes`) and YAML front-matter. This is the
  **CSS Modules** `:global`, not Svelte's — Svelte's `:global(…)` selector and `:global`
  block are supported and get structured selector-list parsing (the same grammar as
  `:not()`); see [checklist_svelte.md](./checklist_svelte.md)
- IE / legacy-browser hacks (`*zoom`, `_width`, `+color`, `color: red\9`)
- CSS-in-JS patterns (styled-components, emotion)
- Houdini APIs (CSS Paint API, Layout API, Properties API runtime)
- Browser-specific rendering behavior
- Cascade/specificity calculation (runtime concern)
- Property value validation (accepted as-is)
- Vendor prefix expansion/removal

---

# Compatibility

Parse output matches Svelte's `parseCss` and formatting matches Prettier, except for the intentional divergences cataloged in [conformance_svelte.md](./conformance_svelte.md) and [conformance_prettier.md](./conformance_prettier.md).

## Intentional Differences (Spec-Compliant Improvements)

Places where tsv is more correct than Svelte's parser. The authoritative catalog —
each entry with its reasoning and fixture — is [conformance_svelte.md §CSS
Corrections](conformance_svelte.md#css-corrections); the classes are:

- An+B microsyntax — `of S` nesting, spec-valid negative forms (`-3`, `-2n`, `-n-3`),
  leading-`-n` forms, and terms split by a comment (`2n /* c */ + 1`), all of which
  `parseCss` rejects or mis-parses
- Comments as inter-token trivia — at combinator boundaries, glued inside a compound,
  between `::part()` names, and in `:nth-*()` argument positions
- Consecutive combinators (`> > .a`) — preserved rather than collapsed to the last
- Namespaces — attribute namespaces (`[svg|href]`, `[*|attr]`, `[|attr]`) and
  no-namespace selectors (`|div`, `|*`), neither of which Svelte supports
- Forgiving `:is()` / `:where()` — invalid items dropped, not a whole-parse failure
- `;` inside a balanced construct — a function value, a simple block, a `var()`
  fallback, or an `@supports` `<general-enclosed>` — is content, not a terminator
- Pseudo-element arguments (`::slotted()`, `::part()`) — internal parsing with
  Svelte-compatible public output. `parseCss` reads *every* pseudo-element argument as a
  selector list and emits it as `PseudoElementSelector.args`; tsv
  models `::part( <ident>+ )` per CSS Shadow Parts instead — an ident run, not selectors —
  and projects it onto that selector-list shape at the wire boundary, so the public output
  matches while the internal AST keeps the per-ident spans the printer needs
