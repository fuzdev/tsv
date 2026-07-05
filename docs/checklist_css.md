# CSS Language Support

Comprehensive reference for CSS language features supported by tsv's parser and formatter.

## Coverage

Effectively all CSS features from stable W3C specifications are supported.
Early-draft features that tsv does not yet parse are listed under [Future Work](#future-work).

**Scope & goals.** The north star is full **CSS-spec compliance**; the near-term,
enforced goal is **matching Svelte's `parseCss`** (the AST canonical). SCSS/Sass,
LESS, CSS Modules, PostCSS plugin syntax, YAML front-matter, and IE hacks are
**permanent non-goals** — out of scope, not unimplemented features. The parser
currently hard-fails on the first invalid construct; spec-style error recovery
(drop the bad rule, keep parsing) is future work toward spec compliance, not the
intended end state. See ./conformance_svelte.md (§CSS Parser Scope & Error Model).

**Spec References**:

- CSS specs: `../../csswg-drafts/` (137 modules)
- Machine-readable data: `../../webref/ed/css/`
- CSS Snapshot 2025: `../../csswg-drafts/css-2025/`

---

# Supported Features

## CSS Syntax Level 3

Foundation for all CSS parsing. Spec: `css-syntax-3` (REC)

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
- Comments in `:nth-*()` args (An+B gaps, around `of`, after the `of` list; a comment inside the An+B text freezes it verbatim)
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

---

## CSS Selectors Level 4

Spec: `selectors-4` (CR - core features in REC via CSS2.1)

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

Spec: CSS Scoping Level 1

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

Specs: `css-values-3` (REC), `css-values-4` (CR)

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

### Viewport Units

- `vw`, `vh`, `vmin`, `vmax`
- Small viewport: `svw`, `svh`, `svmin`, `svmax` (Level 4)
- Large viewport: `lvw`, `lvh`, `lvmin`, `lvmax` (Level 4)
- Dynamic viewport: `dvw`, `dvh`, `dvmin`, `dvmax` (Level 4)
- Logical viewport: `vi`, `vb` (Level 4)

### Container Query Units

Spec: `css-contain-3` (CR)

- `cqw`, `cqh`, `cqi`, `cqb`, `cqmin`, `cqmax`

### Other Units

- Angles: `deg`, `rad`, `turn`, `grad`
- Time: `s`, `ms`
- Frequency: `hz`, `khz`
- Resolution: `dpi`, `dppx`, `dpcm`
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

Spec: `css-variables-1` (CR)

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

Specs: `css-color-3` (REC), `css-color-4` (CR), `css-color-5` (WD - widely shipped)

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

> **Note**: Condition at-rules (`@media`, `@supports`, `@container`) use partial structured parsing - outer structure (connectors, modifiers) is parsed while inner conditions remain as strings. Full structured parsing (range syntax, media features) may be added for tooling use cases (linting, type checking).

### Core At-Rules (REC)

- `@charset`
- `@import` (basic)
- `@import` with media-query condition (media-type-led `screen and (…)` or a bare `<media-condition>` `(max-width: 40px)`)
- `@namespace`
- `@media`
- `@page`
- `@font-face`
- `@keyframes`

### Conditional Rules (CR)

- `@media` with boolean logic (`and`, `or`, `not`)
- `@media` range syntax (`width >= 768px`)
- `@supports` (feature queries)
- `@supports selector()`

### Cascade Layers (CR)

- `@layer`
- `@import` with `layer()` condition
- `@import` with `supports()` condition

### Container Queries (CR)

Spec: `css-contain-3`

- `@container`
- `@container` with logical operators

### Custom Properties API (CR)

Spec: `css-properties-values-api-1`

- `@property` at-rule
- `syntax`, `inherits`, `initial-value` descriptors

### Anchor Positioning (CR)

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

- `@when` (Conditional Rules Level 5)
- `@view-transition`
- Vendor-prefixed at-rules (`@-webkit-*`, `@-moz-*`)

---

## CSS Nesting

Spec: `css-nesting-1` (CR - widely shipped)

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

Spec: `css-backgrounds-3` (CR)

- `linear-gradient()`
- `radial-gradient()`
- `repeating-linear-gradient()`
- `repeating-radial-gradient()`

### Images

Specs: `css-images-3` (CR), `css-images-4`

- `conic-gradient()`, `repeating-conic-gradient()`
- `image-set()`
- `cross-fade()`

### 2D Transforms

Spec: `css-transforms-1` (CR)

- `translate()`, `translateX()`, `translateY()`
- `scale()`, `scaleX()`, `scaleY()`
- `rotate()`
- `skew()`, `skewX()`, `skewY()`
- `matrix()`

### 3D Transforms

Spec: `css-transforms-1` (CR)

- `translate3d()`, `translateZ()`
- `scale3d()`, `scaleZ()`
- `rotate3d()`, `rotateX()`, `rotateY()`, `rotateZ()`
- `perspective()`
- `matrix3d()`

### Individual Transform Properties

Spec: `css-transforms-2` (CR)

- `translate`, `scale`, `rotate` properties

### Filter Effects

Spec: `filter-effects-1` (CR)

- `blur()`, `brightness()`, `contrast()`
- `drop-shadow()`, `grayscale()`, `hue-rotate()`
- `invert()`, `opacity()`, `saturate()`, `sepia()`

### Shapes

Spec: `css-shapes-1` (CR)

- `circle()`, `ellipse()`, `polygon()`, `inset()`
- `path()` (SVG path syntax)

### Grid Functions

Spec: `css-grid-1` (CR)

- `minmax()`
- `repeat()`
- `fit-content()`
- Named grid lines (`[name]`)

### Easing Functions

Spec: `css-easing-1` (CR)

- `linear` keyword
- `ease`, `ease-in`, `ease-out`, `ease-in-out` keywords
- `cubic-bezier()` function
- `steps()` function
- `step-start`, `step-end` keywords
- `steps()` positions: `jump-start`, `jump-end`, `jump-none`, `jump-both`, `start`, `end`

### Scroll-Driven Animations

Spec: `scroll-animations-1` (WD - Chromium shipped)

- `scroll()` function
- `view()` function

### Environment Variables

Spec: `css-env-1` (WD - widely supported)

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

Early-draft features tsv does not yet parse. All specs are Editor's Drafts (ED) with "Exploring" or "Revising" work status.

## CSS Mixins Level 1

Spec: `css-mixins-1` (ED - Exploring)

Chrome has announced intent to implement.

- `@function` at-rule
- Dashed-function syntax (`--custom-fn()`)
- `@return` inside functions
- `@mixin`, `@apply` (planned)

## CSS Grid Level 3 - Masonry

Spec: `css-grid-3` (ED - Revising)

Firefox has an experimental implementation.

- `grid-template-*: masonry`

## CSS Selectors Level 5

Spec: `selectors-5` (ED - Exploring)

- `:local-link(n)` functional form
- `:state(identifier)` for custom element states
- `:heading` pseudo-class (matches all heading elements)
- `:heading(level)` functional form (matches specific heading levels)
- Reference combinator (`/ref/`) for IDREF-based relationships

## CSS Conditional Rules Level 5

Spec: `css-conditional-5` (ED - Exploring)

- `@when` generalized conditional rule
- `@else` chained conditional rule

## CSS Easing Functions Level 2

Spec: `css-easing-2` (ED - Revising)

- `linear()` function with control points

## CSS Values and Units Level 5

Spec: `css-values-5` (ED - Exploring)

New functions for advanced value manipulation:

- `attr()` enhanced with type casting and fallbacks
- `calc-size()` for intrinsic size calculations
- `progress()` for progress interpolation
- `random()` for random values
- `first-valid()` for fallback chains
- `if()` for conditional values
- `toggle()` for cycling values
- `sibling-count()`, `sibling-index()` for sibling-based values

## CSS Color Level 6

Spec: `css-color-6` (ED - Exploring)

Note: `contrast-color()` already works via generic function parsing.

- `color-layers()`

## CSS View Transitions Level 2

Spec: `css-view-transitions-2` (ED - Exploring)

Note: `@view-transition` already works via generic at-rule parsing.

- Cross-document transitions

## CSS Conditional Values

Spec: `css-conditional-values-1` (UD - Exploring)

Very early stage - "Unofficial Draft" status.

- `true` and `false` CSS values
- Comparison operators in values
- Boolean logic in values

## CSS Forms Level 1

Spec: `css-forms-1` (ED - Exploring)

- `appearance: base`
- `::picker()` pseudo-element
- `::field-text`, `::clear-icon`

---

# Out of Scope

These are outside the parser/formatter's responsibility:

- Preprocessor languages (Sass/SCSS, Less, Stylus) and PostCSS plugin syntax
- CSS Modules (`:global`, `composes`) and YAML front-matter
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

Places where tsv is more correct than Svelte's parser:

- `:nth-child(An+B of selector)` - full complex selector list parsing
- Attribute namespace syntax (`[svg|href]`, `[*|attr]`, `[|attr]`)
- Pseudo-element arguments (`::slotted()`, `::part()`) - internal parsing with Svelte-compatible public output
- No-namespace selector syntax (`|div`, `|*`) - Svelte doesn't support
