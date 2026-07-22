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

- Whitespace tokens — ASCII-only (tab/LF/FF/CR/space, css-syntax-3 §4.2); non-ASCII Unicode whitespace (NBSP U+00A0, ideographic space, …) is value content, not a separator, so it is preserved inside a value token rather than collapsed
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
- Comments in selectors
- Comments in `:nth-*()` args — before or after the An+B term, around `of`, and after the
  `of` list. A trailing comment inside the parens freezes the An+B text verbatim (`2n+1`
  keeps its authored spacing instead of normalizing to `2n + 1`). A comment *splitting* the
  An+B term (`:nth-child(2n /* c */ + 1)`) is not supported — see [Future Work](#not-parsed)
- Comments in `::slotted()` / `::part()` / unknown-pseudo args (leading/trailing gaps preserved; the interior positions — between `::part()` names, or `::slotted()` compound-internal — are rejected by parseCss but preserved + normalized by tsv, a `_svelte_prettier_divergence`)
- Comments in `:dir()` / `:lang()` / `::highlight()` identifier args (leading/trailing gaps preserved + normalized; parseCss accepts → a `_prettier_divergence`)
- Comments in declarations
- Comments in at-rules
- Consecutive comments
- Nested comment closing (spec-compliant)
- `format-ignore` / `prettier-ignore` directive (`/* format-ignore */` emits the next rule/declaration verbatim — see [directives.md](./directives.md))

### Escapes

- Unicode escapes (1-6 hex digits: `\0001F4A9`)
- Backslash escapes (`\\`, `\"`, `\'`)
- Control character escapes (`\n`, `\t`)
- Escapes in identifiers (`.cl\61ss` → `.class`)
- Escapes preserved verbatim in at-rule preludes (`@keyframes \@mymove`, `\31 23` — serialized raw, not decoded)
- Escapes in strings
- Surrogate pair handling
- An escape's payload is content, not padding — a trailing escaped whitespace survives trimming and line-wrapping in every position (`width: 50px\ ;`, `url(x\ )`, `@layer a\ ;`, `.a\ , .b`), so the backslash never strands onto the following delimiter. Prettier corrupts these into forms that no longer parse; see [conformance_prettier.md §CSS: Values](conformance_prettier.md#css-values) and [§CSS: At-Rules](conformance_prettier.md#css-at-rules)
- A hex escape's optional whitespace terminator belongs to the escape and is absorbed exactly once (`\41 2px` is the single ident `A2px`, never `A` + `2px`)
- Non-CSS whitespace is value content, not separator — an NBSP or U+3000 in a value or selector survives verbatim (`.a<NBSP>, .b` keeps its class name)

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
- Column combinator (`||`) - _at risk in spec_
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
  space (`--name: ;`), a prettier divergence (see `conformance_prettier.md` →
  CSS: Values)
- Empty custom property with `!important` (`--name: !important;`) — also
  normalizes to a single space; prettier is non-convergent here (grows a space
  per pass), so it's guarded by a `.css` fixture (no prettier oracle)
- `var()` basic usage
- `var()` with fallback
- `var()` empty fallback (`var(--a,)`) — the trailing comma is significant
  (substitutes nothing when unset, unlike `var(--a)`) and is preserved; other
  functions (`rgb(0,0,0,)`) drop a trailing empty arg, matching prettier
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
> isn't a valid condition), `@import` (url/string + `layer()`/`supports()`/media, falling back
> to raw when it doesn't lead with a url/string), and `@scope` (forgiving selector lists).
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
- A comment splitting an An+B term (`:nth-child(2n /* c */ + 1)`) — rejected, matching
  `parseCss`. Per css-syntax-3 §4 comments are removed at tokenization and produce no token,
  so the spec accepts this (prettier does too); the An+B microsyntax reads frozen source text
  and so can't see through the comment. The surrounding positions are supported — see
  [Comments](#comments)

## Parsed Generically, Not Modeled

Everything below **parses today** — the generic at-rule, pseudo-class/element, and
declaration-value paths accept it, format it, and round-trip it. What it lacks is
*structural* modeling, which is the same footing as `@view-transition`,
`contrast-color()`, and the [Modern Pseudo-Classes](#modern-pseudo-classes-level-4)
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
  and leading-`-n` forms that `parseCss` rejects or mis-parses
- Comments as inter-token trivia — at combinator boundaries, glued inside a compound,
  between `::part()` names, and in `:nth-*()` argument positions
- Consecutive combinators (`> > .a`) — preserved rather than collapsed to the last
- Namespaces — attribute namespaces (`[svg|href]`, `[*|attr]`, `[|attr]`) and
  no-namespace selectors (`|div`, `|*`), neither of which Svelte supports
- Forgiving `:is()` / `:where()` — invalid items dropped, not a whole-parse failure
- `;` inside a balanced construct — a function value, a simple block, a `var()`
  fallback, or an `@supports` `<general-enclosed>` — is content, not a terminator
- Pseudo-element arguments (`::slotted()`, `::part()`) — internal parsing with
  Svelte-compatible public output
