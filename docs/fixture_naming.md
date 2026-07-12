# Fixture Content Naming Conventions

**Fixture naming should clarify what's being tested.**

Use **descriptive names** when they explain the test case (escape sequences, edge cases, spec compliance). Use **generic names** for structural/formatting tests where semantics don't matter.

Consult this when creating new fixtures.

**Terminology**: a `prettier_variant_*` file is a form prettier keeps stable but our formatter normalizes to input; a `variant_*` file is a form both formatters keep stable that is distinct from input; a `divergent_variant_*` file is a form prettier keeps stable but our formatter rewrites to a *third* stable form (distinct from both the form and input). See ./conformance_prettier.md for the full catalog.

---

## Core Principle

**Descriptive when helpful, generic otherwise:**

✅ **Use descriptive names** when they clarify what's being tested:

- Edge case tests: `const newline = 'line\nbreak'` (testing newline escapes)
- Spec compliance: `content: '\10FFFF'` (max unicode codepoint)
- Entity tests: `<div>basic named entities</div>` (explains what entities)

✅ **Use generic names** for structural/formatting tests:

- Layout tests: `<Comp><div>block1</div></Comp>` (testing nesting/whitespace)
- Whitespace normalization: `<span>text1</span> <span>text2</span>` (testing spacing)
- Component formatting: `<Comp1 prop1 /><Comp2 prop2 />` (testing component layout)

**Rule of thumb:** If the name helps understand WHAT edge case is tested, use it. If the test is about HOW things are formatted, use generic names.

---

## Guidelines

Each category — ✅ use / ❌ avoid:

- **Component Names** — ✅ `Comp`, `Comp1`, `Comp2`, `Inner`, `Outer` ❌ `Button`, `Modal`, `UserProfile`
- **Text Content** — ✅ `text`, `text1`, `text2`, `block1`, `inline1` ❌ `Click here`, `Hello world`
  - ✅ Preserve word count: `text1 text2` for multi-word
  - ✅ Descriptive OK for edge cases: `basic named entities`
- **Component Props** — ✅ `prop`, `prop1`, `prop2`, `"value"`, `"value1"` ❌ `primary`, `disabled`, `isActive`
- **HTML Attributes** — ✅ `data-attr`, `data-attr1`, `"value"`, `"value1"` ❌ `disabled`, `checked`, `type`, `class`
- **Expressions** — ✅ `expr`, `expr1`, `expr2` (structural) ❌ `count`, `userName`, `isLoggedIn`
  - ✅ Descriptive for edge cases: `newline`, `unicode`
- **JS Variables** — ✅ `a`, `b`, `c`, `x`, `y` (terse) or `expr`, `value` ❌ `myVariable`, `userData`, `isEnabled`
- **JS Functions** — ✅ `fn`, `fn1`, `fn2` or descriptive: `single`, `withConstraint` ❌ `fetchData`, `handleClick`
- **Type Names** — ✅ `A`, `B`, `C`, `T`, `U` (single letter) ❌ `User`, `Props`, `MyType`
- **Module Paths** — ✅ `'./a'`, `'./b'`, `'./types'`, `'./mod'` ❌ `'./UserTypes'`, `'@app/utils'`
- **CSS Classes** — ✅ `.class`, `.class1`, `.class2` ❌ `.my-class`, `.component-name`
- **CSS IDs** — ✅ `#id`, `#id1`, `#id2` ❌ `#myId`, `#component-id`
- **CSS Values** — ✅ `red`, `blue`, `100px`, `bold` ❌ `primary-color`, `brand-blue`
- **Snippet Names** — ✅ `fn`, `fn1`, `fn2` ❌ `header`, `greeting`, `renderItem`
- **Iteration Vars** — ✅ `item`, `items`, `key` ❌ `user`, `product`, `todoItem`
- **Promises** — ✅ `promise` ❌ `fetchData`, `loadUser`
- **Conditions** — ✅ `cond`, `a`, `b` ❌ `isLoggedIn`, `hasPermission`

**Terse variable names**: Single-letter names (`a`, `b`, `c`) are encouraged when variable semantics don't matter. Use descriptive names (`value`, `expr`, `result`) only when they clarify what's being tested.

---

## TypeScript Declaration Naming

Use **descriptive structural names** that indicate what's being tested. All TypeScript declaration fixtures require `<script lang="ts">` (not plain `<script>`).

✅ use / ❌ avoid:

- **Class/Interface** — ✅ `Single`, `Multiple`, `WithConstraint`, `Extends` ❌ `Foo`, `MyClass`, `UserService`
- **Type Parameters** — ✅ `T`, `U`, `V` (single letter) ❌ `TData`, `TItem`, `Type`
- **Heritage (extends)** — ✅ `Base`, `Single` (reuse class names) ❌ `AbstractBase`, `SuperClass`
- **Heritage (implements)** — ✅ `Contract` ❌ `IService`, `Serializable`
- **Properties** — ✅ `a`, `b`, `value`, `prop` ❌ `name`, `id`, `data`
- **Functions** — ✅ `single`, `multiple`, `withConstraint`, `fn`, `fn1` ❌ `identity`, `fetchData`
- **Exported types** — ✅ `A`, `B`, `C` (single letter) ❌ `User`, `Props`, `State`
- **Module specifiers** — ✅ `'./a'`, `'./b'`, `'./types'` ❌ `'./UserTypes'`, `'@app/utils'`

**Examples**:

```typescript
// Good - structural names describe the test case
class Single<T> {
	value: T;
}
class Multiple<T, U> {
	a: T;
	b: U;
}
class WithConstraint<T extends object> {
	items: T[];
}
class Extends<T> extends Single<T> {}
class Implements<T> implements Contract<T> {}

// Good - generic function names
function single<T>(x: T): T {
	return x;
}
function withConstraint<T extends object>(x: T): T {
	return x;
}
async function asyncGeneric<T>(): Promise<T> {}

// Good - single letter for imports/exports
export type { A } from './a';
export type { B, C, D } from './b';
import type { E, F } from './c';

// Bad - semantic names obscure what's being tested
class UserProfile extends BaseEntity {}
function fetchData<T>(): Promise<T> {}
export type { Props, User } from './UserTypes';
```

**For `long` fixtures** (testing line-width wrapping), use artificially long but obviously generic names:

```typescript
// Good - obviously artificial long names for wrapping tests
interface VeryLong<
	T extends VeryLongTypeName | AnotherLongTypeName,
	U extends ExtraLongConstraintType | MoreTypes,
> extends Base1<T>, Base2<T> {
	prop: string;
}

// Short contrast case - clearly named
interface Short<T extends string | number> extends Base<T> {
	prop: T;
}

// Bad - domain-specific names in long fixtures
interface AreaConfig<ES extends ExprRef | SignalRef>
	extends MarkConfig<ES>, PointOverlayMixins<ES> {}
```

---

## CSS Value Tests: Use ONE Rule

When testing CSS values/properties, use a **SINGLE rule** with multiple properties.
Only create multiple rules when testing selector interactions, cascading, or rule ordering.

**✅ Good** - Testing multiple values in ONE rule:

```css
div {
	width: calc(100% - 2rem);
	height: calc(50vh + 10px);
	margin: calc(1em * 2);
}
```

**❌ Bad** - Unnecessary multiple rules:

```css
.class1 {
	width: calc(100% - 2rem);
}
.class2 {
	height: calc(50vh + 10px);
}
.class3 {
	margin: calc(1em * 2);
}
```

---

## Naming Rules

1. **Use numeric suffixes only when there are multiples**:
   - Single item: `prop`, `text`, `Comp` (no number)
   - Multiple items: `prop1`, `prop2` / `text1`, `text2` / `Comp1`, `Comp2` (ALL numbered starting from 1)
   - ⚠️ When there are multiples, don't mix numbered and unnumbered: ❌ `Comp` and `Comp2` → ✅ `Comp1` and `Comp2`

2. **HTML elements use `data-attr`**: Valid HTML custom attributes with `data-` prefix

3. **Components use `prop`**: Clear distinction from HTML attributes

4. **Text content numbered by context**:
   - Block elements: `<div>block1</div>`, `<div>block2</div>`
   - Inline elements: `<span>inline1</span>`, `<span>inline2</span>`
   - Plain text nodes: `text` (single), `text1`, `text2`, `text3` (multiple)
   - Space-separated text: `text1 text2` (explicit spacing)
   - **Multi-word preservation**: `"text content"` → `text1 text2`, not `text` (preserves word count for unformatted variants)

**⚠️ CRITICAL: Preserve Each Text Node's Word Count**

When multiple text nodes exist, preserve EACH node's word count independently:

```
❌ text<span>inline</span>more text
   → text1<span>inline1</span>text2
   (Lost "more text" which is 2 words!)

✅ text<span>inline</span>more text
   → text1<span>inline1</span>text2 text3
   (Preserved all word counts: 1, 1, and 2)
```

5. **Attribute values use `"value"` pattern**:
   - String values: `prop="value"`, `prop1="value1"`, `prop2="value2"`
   - Not text content patterns: ❌ `prop="text"`, `prop="block1"`

6. **CSS uses simple, valid values**:
   - **Selectors**: Use `div`, `span` for value tests; `.class`, `#id` for selector tests
   - **One rule for value tests**: See [CSS Value Tests](#css-value-tests-use-one-rule) above
   - **Duplicate properties allowed**: `div { margin: 1rem; margin: 1rem 2rem; }` (tests multiple values)
   - Colors: `red`, `blue`, `#f00` | Lengths: `100px`, `1rem` | Keywords: `bold`, `none`

7. **Update ALL variants together**: When changing the input file, update ALL variant files with the same standardized names

8. **Use descriptive names for edge case tests**: When testing edge cases (escapes, entities, unicode, spec compliance):
   - **TypeScript variables**: `const newline = 'line\nbreak'` (explains the test case)
   - **Text content**: `<div>basic named entities</div>` (documents what's being tested)
   - **Test data**: Preserve escape sequences, entities, unicode characters exactly as written
   - **Still generic**: Markup structure uses generic names (`.class1`, `data-attr`, `Comp`)

9. **Use comments when testing multiple cases in one file**: Comments help organize and clarify distinct test cases within a single fixture. When a fixture tests several variations of a feature, use comments to label each case:
   ```typescript
   // Numeric literals
   type Num = 1;
   type Hex = 0xff;

   // String literals
   type Str = 'a';
   ```
   - **When in doubt, add a comment** - clarity is more valuable than minimalism
   - Comments are especially helpful in `long` fixtures to explain what exceeds print width
   - **Describe the formatting, not our bugs.** Comments should say what the correct output IS (e.g., "array expands to multi-line"), not how our formatter differs (e.g., ~~"we break after ="~~). Fixtures define correct behavior — they shouldn't reference our implementation's shortcomings.

---

## Rationale

- **Clarity first**: Names should clarify what's being tested
- **Generic when possible**: Prevents domain coupling in structural tests
- **Minimal fixtures**: One rule for CSS value tests (see [above](#css-value-tests-use-one-rule))
- **Consistent patterns**: Enable duplicate detection and validation
- **Valid syntax**: `data-` attributes are valid HTML5, CSS values must be syntactically valid

---

## Variant File Naming

### Extension Matching

Variant files must match the input file extension:

- `input.svelte` → `unformatted_*.svelte`
- `input.svelte.ts` → `unformatted_*.svelte.ts`
- `input.ts` → `unformatted_*.ts`
- `input.css` → `unformatted_*.css`

### Choosing Between `unformatted_*`, `unformatted_ours_*`, `prettier_variant_*`, and `variant_*`

⚠️ **Key Distinction** - Directory and normalization behavior determine which pattern to use:

**Prettier-stable forms** (prettier preserves idempotently):

- Ours normalizes to input → `prettier_variant_*`
- Ours keeps stable (not input) → `variant_*`
- Ours rewrites to a third stable form (not input, not the form) → `divergent_variant_*`
- Run `deno task fixtures:audit <pattern>` to classify

**Unformatted variants** (normalization tests) — behavior is identical across every
extension (`.svelte`, `.svelte.ts`, `.ts`, `.css`; the extension just matches the input
and selects the prettier parser):

- **`unformatted_*.*`** — normalizes to input with BOTH prettier AND our formatter.
  Regular directories; in `_prettier_divergence` directories allowed only without
  `output_prettier.*` (S9 — input must be prettier-stable for prettier to normalize to it)
- **`unformatted_ours_*.*`** — normalizes to input with our formatter, NOT with prettier
  (the claim is verified). ONLY in `_prettier_divergence` directories (S8)
- **`unformatted_prettier_*.*`** — prettier normalizes it to `output_prettier.*` (which
  must exist); our formatter validation is not applied. ONLY in `_prettier_divergence`
  directories

### Standard Variant Names

Both patterns follow the same content conventions:

Variant name — purpose (example):

- `unformatted_compact` — All content on one line, minimal whitespace (`<div><span>text</span></div>`)
- `unformatted_spaces` — Excessive spaces AND newlines that collapse (`if  (\n\tcond  )` → `if (cond)`)
- `unformatted_newlines` — Excessive blank lines (Multiple `\n\n\n` between elements)
- `unformatted_no_self_closing` — Void elements without `/>` (`<br>`, `<img>`, `<hr>`)
- `unformatted_tabs` — Mixed tabs and spaces (`\t` + spaces in indentation)
- `unformatted_tag_split` — Tags broken across lines (hug mode) (`<div\n><span\n>text</span\n></div\n>`)
- `unformatted_mixed_spacing` — Chaotic mix of tabs, spaces, breaks (Combination of above)
- `unformatted_excessive_blank_lines` — 3+ consecutive blank lines (`\n\n\n\n`)
- `unformatted_with_closing_tag` — Self-closing as regular tag (`<Comp></Comp>` vs `<Comp />`)

**Examples**:

- Regular directory: `unformatted_compact.svelte`, `unformatted_spaces.svelte`
- `_prettier_divergence` directory: `unformatted_ours_compact.svelte`, `unformatted_ours_spaces.svelte`

---

## Prettier Divergence File Naming

### Directory Suffix

Directories documenting prettier divergence MUST end with `_prettier_divergence`:

- `container_spacing_prettier_divergence/`
- `scope_complex_prettier_divergence/`
- `media_boolean_spacing_prettier_divergence/`

### File Naming Patterns

**For documenting prettier's quirky outputs** (`prettier_variant_*.*`):

The extension must match the input file (`.svelte`, `.ts`, `.css`, `.svelte.ts`).

Quirk name — description (example):

- `prettier_variant_compact` — No spaces (compact) (`@container (min-width:700px)`)
- `prettier_variant_spaces` — Extra spaces preserved (`@layer base,  components`)
- `prettier_variant_parens_spaces` — Spaces inside parentheses (`@scope ( .card )`)
- `prettier_variant_comma_spaces` — Extra spaces after commas (`@scope (.card,  .panel)`)
- `prettier_variant_to_spaces` — Extra spaces around 'to' keyword (`@scope (.card)  to  (.ignore)`)
- `prettier_variant_minus_space` — Space before minus in nth notation (`:nth-child(2n - 1)`)
- `prettier_variant_minus_compact` — No space around minus (`:nth-child(2n-1)`)
- `prettier_variant_missing_space` — Missing required space (`@media screen and(min-width:768px)`)
- `prettier_variant_bom` — BOM preserved (for BOM fixtures) (File starts with UTF-8 BOM)

**For testing our normalization** (`unformatted_ours_*.*`):

- Use `unformatted_ours_*` naming in `_prettier_divergence` directories
- Extension must match input file (`.svelte`, `.ts`, `.css`, `.svelte.ts`)
- Our formatter must normalize these to input (N5)
- Prettier must NOT normalize these to input (N6 verifies the `_ours` designation)

**Naming convention**:

- Prefix: `prettier_variant_`
- Suffix: Describes WHAT is quirky (not just "variant1")
- Be specific: `parens_spaces` not just `spaces`
- Match pattern consistently across fixtures

**For documenting prettier's unstable intermediate output** (`prettier_intermediate_*.*` / `prettier_intermediate_to_variant_*.*`):

Capture prettier's actual first-pass output (not hand-written) from the same-suffix
`unformatted_ours_*` source — the shared suffix links them (e.g.
`unformatted_ours_expanded` → `prettier_intermediate_expanded`). Use the
`_to_variant_` form when prettier's second pass lands on a documented
`variant_*`/`prettier_variant_*` sibling instead of `input.*`. Validation: rules
N7/N7b in ./fixture_overview.md.

**Example:**

```
trailing_member_computed_comment_prettier_divergence/
├── input.svelte                        # const f = items.filter((x) => x)[0]; // comment
├── unformatted_ours_expanded.svelte    # Comment before [0] on separate line
├── prettier_intermediate_expanded.svelte  # [// comment\n0] (prettier's unstable form)
└── README.md
```

**For documenting dual-stable and divergent-variant forms** (`variant_*.*` / `divergent_variant_*.*`):

All three prettier-stable-form kinds are prettier-idempotent and differ only in what
our formatter does — `prettier_variant_*`: ours → `input`; `variant_*`: ours keeps it
(dual-stable); `divergent_variant_*`: ours → a distinct third stable form (three stable
forms coexist). Suffixes describe the form, e.g. `variant_compact`, `variant_wrapped`,
`divergent_variant_own_line` (heritage keyword own-line form), `divergent_variant_spaces`
(a blank-line form). Validation: rules N9/N11 + C3b/C3c in ./fixture_overview.md;
README.md required.

**No-oracle marker files** — three fixed filenames (not variant patterns), all rare;
their claim semantics and coexistence rules are ./fixture_overview.md rules
F5/S18, F6/S19, F7/S20:

- `prettier_nonconvergent.txt` — prettier never reaches a fixed point on the input;
  content is free-form prose. Use only when `deno task fixtures:update:formatted`
  cannot converge.
- `prettier_rejects.txt` — prettier throws on the input (parse rejection or printer
  crash); the trimmed content is the position-stripped expected-error substring
  (matched with `contains`; all prose lives in README.md). The input must be valid by
  tsv's parse oracle. Hand-author it (`fixture_init` runs prettier, which throws),
  then `deno task fixtures:update:parsed` for `expected.json`.
- `tsv_rejects.txt` — **tsv** rejects an input the canonical parser accepts (neither
  `input_invalid_*`, which needs both to reject, nor a plain fixture, which needs tsv
  to parse). Lives in a `_svelte_divergence` dir with README + `expected_svelte.json`;
  trimmed content is the expected **tsv**-error substring. Hand-author `input.*`
  (prettier is not consulted), then `deno task fixtures:update:parsed` generates
  `expected_svelte.json` (failing loudly if the canonical parser rejects — the
  divergence would be dead). Catalog in ./conformance_svelte.md §TypeScript Corrections.

---

## Line Wrapping Tests (`long` / `_long`)

### Directory Naming Patterns

Two patterns are used for fixtures testing width-based wrapping behavior:

**Pattern 1: `long` subdirectory** (preferred for feature categories with a single long variant)

```
expressions/calls/long/           # Long call expressions
types/conditional/long/           # Long conditional types
modules/imports/long/             # Long import statements
```

**Pattern 2: `*_long` suffix** (when the feature name needs the suffix)

```
css/values/functions/gradient_long/    # Long gradients
svelte/components/attrs_long/          # Long component attributes
typescript/types/aliases/generics_long/ # Long generic type aliases
```

**Choosing between patterns:**

- Use `long/` subdirectory when the parent directory is the feature (e.g., `calls/long/`)
- Use `*_long` suffix when the fixture name describes what's long (e.g., `generics_long/`)
- Both patterns are equivalent in purpose - choose based on directory structure

**⚠️ Naming Standardization:**

Avoid older patterns like `_wrapping` or `_wrapped`:

✅ use / ❌ avoid:

- ✅ `function_gradient_long` ❌ `function_gradient_wrapping`
- ✅ `element_native_attrs_long` ❌ `element_native_wrapped`
- ✅ `component_attrs_long` ❌ `component_single_long_attr`

**Rationale:** `long` describes the _condition_ (content exceeds print width). `_wrapped` describes the _result_, and `_wrapping` is redundant.

### Purpose

The `long` naming indicates:

- **Content exceeds print width** (typically 100 characters)
- **Tests width-based wrapping behavior** specific to that CSS/Svelte feature
- **Whether wrapping occurs** depends on the feature, not the suffix

### Key Principle: Generic Data

**ALWAYS use generic, nonsense-but-valid data in `long` fixtures** to reduce visual noise and focus on wrapping behavior:

✅ **Good examples:**

```css
/* Gradient with generic colors/values */
background: linear-gradient(0deg, rgba(0, 0, 0, 0.8) 0%, rgba(1, 1, 1, 0.8) 50%);

/* Font family with generic names */
font-family: 'f0000000', 'f1111111', 'f2222222', 'f3333333';

/* Transform with zero values */
transform: translateX(0px) translateY(0px) rotate(0deg) scale(0);
```

❌ **Bad examples:**

```css
/* Realistic data creates visual noise */
background: linear-gradient(to bottom right, rgba(255, 255, 255, 0.8) 0%);
font-family: 'Helvetica Neue', 'Segoe UI', 'Arial Unicode MS';
transform: translateX(100px) translateY(200px) rotate(45deg);
```

### Data Patterns

**Moderate repetition**:

- Use enough zeros/ones to be obviously generic/long
- Not visually overwhelming
- Examples: `'f0000000'`, `rgba(0, 0, 0, 0.8)`, `0.0000000001`

**JS/TypeScript padding patterns** for reaching exact line-width boundaries:

Pattern → example — use case:

- **Letter repetition** → `AAAA...`, `yyyy...` — Class names, variable suffixes
- **Underscore padding** → `unknown___________` — Fine-tuning to exact char count
- **Trailing letters** → `abcdef`, `abcdefg` — +1 char increments for 100→101
- **Number padding** → `190000`, `1900000` — Numeric literals
- **Descriptive camelCase** → `variableNameThatPadsToExactly...` — Self-documenting long names
- **Generic + suffix** → `condA`, `argument1`, `class1` — Multiple similar items

✅ **Good** - obviously artificial padding:

```typescript
// Letter repetition for class/variable names
class AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA<T, U> {}
const long = aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ? x : y;

// Underscore padding for exact boundaries
class Inline<T extends Record<string, unknown___________>> {}

// Descriptive camelCase padding
<input bind:value={variableNameThatPadsToExactlySeventySixChars} />
catch (e: A | B | LongGenericTypeNameForTestingBoundary) {}

// Generic base + suffix for multiple items
if (condA && condB && condC && condD && condJJJJJJJ) {}
fn(argument1, argument2, argument3);
.class1 > .class2 > .class3[data-attr1][data-attr2abcdef] {}
```

❌ **Bad** - domain-specific names that imply real semantics:

```typescript
// These look like real error properties, not test padding
catch ({message, stack, errno, syscall, cause}: Error) {}

// Realistic data obscures what's being tested
const user = fetchUserData(userId, sessionToken);
```

**Color formats** (use whatever's clearest):

- Hex for simple colors: `#000`, `#111`, `#222`
- RGB for functions: `rgb(0, 0, 0)`, `rgb(1, 1, 1)`
- Use whatever makes the test clearest

### Required Comments

**EVERY `long` fixture MUST have comments explaining:**

1. ✅ What exceeds print width
2. ✅ What wraps vs what doesn't wrap
3. ✅ Why (if non-obvious)

**Examples:**

```css
/* Short gradient - doesn't wrap (under 100 chars) */
.short {
	background: linear-gradient(0deg, #000, #111, #222);
}

/* Long gradient - wraps arguments */
.long {
	background: linear-gradient(
		0deg,
		rgba(0, 0, 0, 0.8) 0%,
		rgba(1, 1, 1, 0.8) 50%
	);
}

/* Media queries DON'T wrap - even when exceeding 100 chars (this is 183 chars) */
@media (min-width: 768px) and (max-width: 1024px) {}
```

### Wrapping Behavior Examples

**Features that wrap when long:**

- Gradient function arguments: `linear-gradient(...)`, `radial-gradient(...)`
- Polygon function arguments: `polygon(...)`
- Selector lists with combinators: `.class1 > .class2 > .class3`
- Nested selector lists: `:where(.a, .b, .c)`
- Comma-separated value lists: `font-family: 'a', 'b', 'c'` (prettier divergence)
- Space-separated value lists: `transform: fn1() fn2() fn3()` (prettier divergence)
- Single-argument functions (non-`url`): `fn(<long-token>)` — the lone arg wraps onto its own line, just like a multi-arg list (matches prettier)

**Features that DON'T wrap when long:**

- Media query conditions: `@media (condition1) and (condition2) and (condition3)`
- Transform function arguments: `matrix(1, 2, 3, 4, 5, 6)` stays inline
- Filter function arguments: `drop-shadow(0 0 0 rgba(...))` stays inline
- `url()` content: `url('very/long/path/...')` stays inline — opaque, never wrapped (the lone exception to single-arg wrapping)

### HTML/Svelte Element Attribute Wrapping

**Prettier is indent-aware.** The effective line width = indent + content. Wrapping occurs when effective width exceeds 100.

**Key behaviors vary by element type:**

All inline at ≤100 effective width; at >100 effective:

- Self-closing (components, void) — full multiline
- Block (`<div>`, `<p>`, etc.) — full multiline
- Inline (`<span>`, `<a>`, etc.) — hug mode* or full multiline

*Hug mode: attrs stay on one line, only `>` moves to new line. Used when attr line (without `>`) fits at column 0 but total exceeds with indent.

**Example:**

```svelte
<!-- 98 + 2 (indent) = 100 effective - no wrap -->
<div>
	<span class="x" data-attr="...98 chars total..."></span>
</div>

<!-- 99 + 2 (indent) = 101 effective - WRAPS -->
<div>
	<span class="x" data-attr="...99 chars total..."
	></span>
</div>
```

**Testing nested elements:**

Test at multiple indent levels to verify indent-aware wrapping:

- Indent Level 0 — Tabs 0, Visual Width 0, Content to hit 101: 101 chars
- Indent Level 1 — Tabs 1, Visual Width 2, Content to hit 101: 99 chars
- Indent Level 2 — Tabs 2, Visual Width 4, Content to hit 101: 97 chars
- Indent Level 3 — Tabs 3, Visual Width 6, Content to hit 101: 95 chars

**Verification:**

**Do not estimate line widths manually** — they are often wrong (tabs = 2 visual chars, off-by-one errors are common). `fixture_init` shows line widths automatically. Use `--force` to iterate until widths are correct. For specific lines:

```bash
cargo run -p tsv_debug line_width FILE --line 5   # specific line with preview
cargo run -p tsv_debug compare FILE               # compare with prettier
```

### Consolidation Strategy

When multiple `long` fixtures test the same feature:

- **Merge into one** with multiple test cases
- Example: `function_gradient_wrapping` + `function_gradient_wrapping_long` → `function_gradient_long`
- Keep all test cases, add clear comments for each

**Example of consolidated fixture:**

```css
div {
	/* Short case - doesn't wrap */
	background: linear-gradient(0deg, #000, #111);
	/* Long case - wraps arguments */
	background: linear-gradient(
		0deg,
		rgba(0, 0, 0, 0.8) 0%,
		rgba(1, 1, 1, 0.8) 50%
	);
	/* Edge case - nested functions */
	background: linear-gradient(0deg, color-mix(in srgb, #000, #111));
}
```

**Note:** Use `div {}` with multiple properties and comments, not multiple class selectors. See [CSS Value Tests](#css-value-tests-use-one-rule).

---

## Invalid Syntax File Naming (`input_invalid_*`)

Files testing parser rejection use `input_invalid_<description>.<ext>`.

### Pattern

```
input_invalid_<what>_<where>.svelte
```

- `<what>` - The keyword/construct being misused
- `<where>` - The position/context where it's invalid (optional)

### Examples

- `input_invalid_await_const.svelte` — `const await = ...` (await as variable name)
- `input_invalid_await_param.svelte` — `function fn(await)` (await as parameter)
- `input_invalid_await_destructure_array.svelte` — `const [await] = ...` (await in array pattern)
- `input_invalid_yield_shorthand.svelte` — `{yield}` (yield as shorthand property)
- `input_invalid_let_label.svelte` — `let: for(...)` (let as label)

### Naming Guidelines

Category → pattern — example:

- Declaration → `_const`, `_let`, `_var` — `input_invalid_yield_const.svelte`
- Parameter → `_param`, `_param_arrow` — `input_invalid_await_param.svelte`
- Destructuring → `_destructure_array`, `_destructure_shorthand` — `input_invalid_yield_destructure_array.svelte`
- Property → `_shorthand`, `_method` — `input_invalid_yield_shorthand.svelte`
- Control flow → `_label` — `input_invalid_let_label.svelte`

### Best Practices

- **Descriptive names** - Should indicate what's invalid without reading the file
- **One error per file** - Don't combine multiple invalid constructs
- **Consistent suffixes** - Use the same suffix pattern across similar tests

---

## See Also

- ./fixture_workflow.md - Step-by-step fixture creation process
- ./fixture_overview.md - Validation rules, troubleshooting, divergence patterns
- ./conformance_prettier.md - Full prettier quirk catalog
