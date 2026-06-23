# Fixture Creation Workflow

> **For agents**: Follow this script when creating fixtures for new features.

---

## Golden Rules

**NEVER modify a correct fixture to make tests pass.** Fixtures define correct behavior (prettier's output). If a test fails, fix the implementation — not the fixture. A failing test is doing its job: revealing a bug.

**No code changes without a failing fixture first.** Even "obvious" fixes need a fixture proving the divergence exists. The fixture is the proof and the regression guard. Exception: pure comment/doc updates that don't change behavior.

## TDD Steps

These steps match CLAUDE.md's numbered list. Each is expanded in a section below.

```
0. LOAD CONTEXT — read this file + fixture_naming.md, study 2-3 existing fixtures
1. CREATE FIXTURE — fixture_init (formats through prettier + generates expected.json)
2. REVIEW — verify generic names, edge cases, AST structure, handle divergence
3. SEE IT FAIL — deno task fixtures:validate <pattern>
4. ★ APPROVAL GATE — show input + failing diff, wait for user approval
5. IMPLEMENT — fix parser/formatter to make the fixture pass
6. VALIDATE — deno task fixtures:validate <pattern>, add unformatted_* variants
```

**Approval Gate**: Never proceed past step 4 without explicit user approval. Present the fixture content and failing diff, then wait for "lgtm" or feedback. **If the user gives feedback requiring fixture changes, redo steps 1-3 and return to step 4 — the gate resets every time the fixture changes.** Variants (step 6) don't need separate approval unless complex (divergence fixtures).

**Failing tests are the normal starting point.** When you create a fixture for a feature that doesn't exist yet, tests will fail. That's the workflow — the fixture defines the target, implementation catches up.

---

## Step 0: Load Context

Pick ONE item from the todo list. State: **"Working on: [item name]"**

**Read these docs first** (don't skip):

- ./fixture_naming.md — **REQUIRED before every fixture.** Actually open and read it — generic naming rules, `long` fixture conventions, variant naming, boundary testing
- ./conformance_prettier.md — **REQUIRED only before a `_prettier_divergence` fixture** (skip for ordinary fixtures). Read §Comment Position Philosophy + the §Comment relocation catalog; the divergence must be sanctioned and cataloged there (see [Step 2.3](#23-divergence-handling))
- This file's [Golden Rules](#golden-rules) — fixture-first discipline

**Always load `fixture_naming.md`** — skipping it leads to domain-specific names, missing boundary tests, and fixture rework; conventions are easy to forget. Add `conformance_prettier.md` whenever the fixture will be a divergence.

Then find 2-3 similar existing fixtures and READ them. Note: how many examples (usually 3-6), naming conventions (`expr`, `cond`, `a`, `b` — see ./fixture_naming.md), edge cases, and `unformatted_*` variants.

Use `--prettier-only` during fixture design to validate structure without running our parser/formatter:

```bash
deno task fixtures:validate --prettier-only <pattern>
```

---

## Step 1: Create Fixture

### 1.1 Create Directory and Draft

```bash
mkdir -p tests/fixtures/typescript/[category]/[name]
```

**Directory location:**

- **Feature fixtures** in feature-specific directories (e.g., `calls/chained/`, `types/conditional/`)
- **Comment fixtures** with the feature they test, using `*_comment` suffix (e.g., `calls/chained/trailing_arg_comment`). Only `syntax/comments/` for universal comment rules (basic syntax, blank lines, cross-cutting edge cases)

**Choose input file type:**

- `input.svelte` (preferred) - Tests code embedded in Svelte `<script>`/`<style>` context
- `input.ts` (rare) - Only when a feature can't be tested in `.svelte`: byte-0 file-level features (hashbang, BOM) or context-dependent formatting (e.g. the `<T>` vs `<T,>` arrow type-parameter trailing comma). TS-only _syntax_ (`import =`, `export =`, types, decorators, `declare`) is **not** a reason — it formats identically in `<script lang="ts">`. See [fixture_overview.md §Input File Types](./fixture_overview.md#input-file-types)
- `input.css` (rare) - Only for file-level CSS features at byte position 0 (e.g., BOM)
- `input.svelte.ts` (runes) - Svelte rune modules (`$state`, `$derived`, etc.)

⚠️ **Prefer `.svelte`** - it's the only path with an external canonical source for CSS. See [fixture_overview.md](./fixture_overview.md#why-svelte-is-the-default-canonical-source) for details.

⚠️ **Prefer plain block comments over JSDoc** — When testing comment-related formatting (e.g., comment placement, blank line detection), use plain `/* comment */` instead of `/** @type {T} */` unless the fixture specifically tests JSDoc cast behavior. A JSDoc cast triggers paren *preservation* — a dedicated code path whose prettier-oracle behavior differs by parser backend (oxc-ts strips, babel keeps) — which can obscure the real formatting issue being tested. Plain block comments exercise the same comment-placement paths without the cast's preservation semantics.

Write input content with:

- Multiple related patterns in ONE fixture (consolidate)
- Generic names: `a`, `b`, `cond`, `expr`, `obj`, `arr`, `i`, `n`
- Content does NOT need to be prettier-formatted — `fixture_init` handles that

### 1.2 Create with `fixture_init`

Formats through prettier + generates `expected.json` automatically. Input is guaranteed correctly formatted by construction.

```bash
cargo run -p tsv_debug fixture_init tests/fixtures/.../name --content '<script>code</script>'
cargo run -p tsv_debug fixture_init tests/fixtures/.../name --stdin << 'EOF'
<script>multiline code (any formatting is fine)</script>
EOF
cargo run -p tsv_debug fixture_init tests/fixtures/.../name   # reformat existing input
```

Options: `--parser typescript|css|svelte-ts` (default: svelte; `ts` and `svelte.ts` are accepted aliases), `--force` (overwrite existing).

After running, **read the generated `input.svelte`** to verify structure. For `long` fixtures, **check the line widths in the output** — do not estimate widths manually.

**⚠️ `long` fixtures MUST include BOTH boundary cases:** a line at exactly 100 chars (stays inline) AND a line at exactly 101 chars (breaks/wraps). This catches off-by-one errors. Iterate with `--force` until both widths are exact — adjust variable name length to hit the boundary precisely.

### 1.3 Manual Alternative

For divergence fixtures needing fine-grained control (writing `output_prettier.*`, `prettier_variant_*.*`, `prettier_intermediate_*.*`, `prettier_intermediate_to_variant_*.*`, etc.), use `format_prettier` directly:

```bash
cargo run -p tsv_debug format_prettier .../input.svelte --no-line-widths 2>/dev/null > /tmp/p.svelte && cp /tmp/p.svelte .../input.svelte
deno task fixtures:update:parsed <pattern>  # generate expected.json separately
```

### 1.4 Quick Parse Check

```bash
cargo run -p tsv_debug canonical_parse tests/fixtures/.../input.svelte 2>/dev/null | head -30
```

Verify no parse errors (detailed AST verification in step 2.2). If prettier passes but canonical_parse fails, you have invalid JS that prettier silently accepts:

Error → cause — fix:

- `'return' outside of function` → `return` in script block — Wrap in function or use `break`
- `'await' outside async function` → `await` in script block — Use `{#await}` block or wrap in `async function`
- `'break'/'continue' outside loop` → Control flow outside loop — Add enclosing loop context
- `Unexpected token` → Invalid JS syntax — Check syntax in browser console

---

## Step 2: Review

### 2.1 Edge Case Checklist

Review your fixture against these checklists. **If gaps found**: add to `input.svelte`, rerun `fixture_init` to reformat (step 1.2), and repeat.

**Comments describe formatting, not bugs.** Fixture comments should explain what the correct output IS (e.g., "array expands to multi-line"), never how our formatter differs. Fixtures define correct behavior — they are not bug reports.

**For major features**: Create a feature-specific checklist in the TODO document with comprehensive edge cases.

**For comprehensive feature matrices**: See ./checklist_css.md, ./checklist_svelte.md, ./checklist_typescript.md.

**For Statements** (`if`, `for`, `while`, `switch`, `try`):

- Empty body — `if (cond) {}`, `for (;;) {}`, `while (cond) {}`
- No braces — `if (cond) expr;`, `for (...) expr;`, `while (cond) expr;`
- Nested — `if (a) { if (b) {} }`, `for (...) { for (...) {} }`
- Chained — `if (a) {} else if (b) {} else {}`
- With expressions — `if (a && b)`, `for (let i = 0, j = 0; ...)`

**For Imports/Exports**:

- Basic — `import x from 'y'`, `export const x = 1`
- Named — `import {a, b} from 'y'`, `export {a, b}`
- Renamed — `import {a as b}`, `export {a as b}`
- Combined — `import x, {a, b} from 'y'`, `import x, * as ns from 'y'`
- Re-exports — `export * from 'y'`, `export * as ns from 'y'`, `export {a} from 'y'`
- Default — `export default x`, `export default function fn() {}`
- Empty braces — `import {} from 'y'`, `export {}`
- Trailing comma — `import {a, b,} from 'y'`, `export {a, b,}`
- Type imports — `import type {A}`, `import type * as T`, `import {type A, b}`
- Attributes — `import x from 'y' with {type: 'json'}`

**Notes**:

- Use unique variable names across all imports (JS doesn't allow duplicate identifiers)
- Trailing commas are normalized by prettier (removed), so they belong in `unformatted_*.svelte`, not `input.svelte`

**For Expressions**:

- Precedence — `a + b * c`, `a || b && c`
- Parenthesized — `(a + b) * c`
- Nested — `fn(fn(fn(x)))`, `a.b.c.d`
- With types — `x as T`, `fn(): T => ...`

**For CSS**:

- Empty — `div {}`, `@media screen {}`
- Nested — `div { span {} }`
- Multiple values — `margin: 1px 2px 3px 4px;`
- Functions — `calc()`, `var()`, `url()`
- Comments — `/* before */ selector`, `property: /* mid */ value`
- Long lines — See `long` naming below

**Width-based wrapping (`long` naming)**:

When testing line-width wrapping behavior, use `long` in the directory name:

- Subdirectory: `feature/long/` (e.g., `calls/long/`) - when feature is the parent
- Suffix: `feature_long/` (e.g., `gradient_long/`) - when describing what's long
- Content must exceed 100 chars to trigger wrapping
- Use generic data: `rgba(0, 0, 0, 0.8)`, `'f0000000'` (not realistic values)
- Add comments explaining what wraps vs what doesn't

**⚠️ Always test the exact 100/101 boundary.** Include both a case that fits at exactly 100 chars and one that exceeds at 101. This catches off-by-one errors and documents the precise breakpoint. Test at multiple indent levels if the feature appears nested (each tab adds 2 visual chars).

**Do not estimate line widths manually — they are often wrong** (tabs count as 2 visual chars, emoji/unicode vary, and off-by-one errors are common). `fixture_init` shows line widths automatically: lines at 90+ chars are listed with markers for exactly/over 100, and `_long` directories warn if nothing is near the boundary. Use `--force` to iterate until widths are correct:

```bash
# Iterate: adjust content, rerun, check widths in output
cargo run -p tsv_debug fixture_init tests/fixtures/.../name --force
# For specific line detail:
cargo run -p tsv_debug line_width input.svelte --line 5
```

**Simplify content.** Strip the reproduction to the minimum that triggers the divergence — use simple string literals (`'aaa...'`) and generic names instead of complex expressions or domain data. The fixture should isolate the formatting behavior, not the content.

See [fixture_naming.md](./fixture_naming.md#line-wrapping-tests-long--_long) for full conventions.

### 2.2 Parser Verification

Verify expected node types, no `Error`/`Unknown` nodes, correct structure:

```bash
cargo run -p tsv_cli parse tests/fixtures/.../input.svelte --pretty | head -80       # our parser
cargo run -p tsv_debug canonical_parse tests/fixtures/.../input.svelte | head -80  # canonical
```

### 2.3 Divergence Handling

**The spec wins; adopting prettier's output is the default tie-breaker.** When the spec defines canonical behavior, follow the spec — even if prettier's output is itself valid CSS. Otherwise adopt prettier's output. Diverge only for a spec-defined canonical form prettier doesn't emit, documented prettier bugs, spec violations, or comment repositioning — never for preference. When prettier moves a comment to a different syntactic position, preserve the user's placement (see [conformance_prettier.md Comment Position Philosophy](./conformance_prettier.md#comment-position-philosophy)). See [fixture_overview.md Decision Framework](./fixture_overview.md#decision-framework).

**Creating a divergence fixture** (rare):

0. **Load [conformance_prettier.md](./conformance_prettier.md) FIRST** — REQUIRED before any divergence. Read §Comment Position Philosophy and the §Comment relocation catalog to (a) confirm the divergence is sanctioned (not a bug you're papering over) and (b) check whether it's already cataloged. If a sibling rule exists, match its shape; if new, add a catalog entry + one-line note there as part of this change. The README and conformance entry must agree.
1. Create directory with `_prettier_divergence` suffix
2. Add `README.md` explaining why tsv differs — match the concise style of sibling READMEs (prettier form ↔ tsv form ↔ reason + conformance link)
3. Document with: `output_prettier.*`, `prettier_variant_*.*`, `variant_*.*`, `unformatted_ours_*.*`, `prettier_intermediate_*.*`, or `prettier_intermediate_to_variant_*.*` — see [fixture_naming.md](./fixture_naming.md#prettier-divergence-file-naming) for details
4. Use `deno task fixtures:audit <pattern>` to investigate novel prettier outputs

`deno task fixtures:update:formatted` may also auto-generate an `audit_signature.txt` next to `output_prettier.*` when prettier requires multiple passes on it. Treat it as a sibling of `output_prettier.*` — never edit by hand; regenerate with the same command. See ./fixture_overview.md (rule F4).

If prettier **never converges** on the input (each pass keeps changing the output — no fixed point, so no `output_prettier.*` is possible), add a `prettier_nonconvergent.txt` marker + README instead of the claim files above; the validator live-verifies the non-convergence. Rare — one in-tree case. See ./fixture_overview.md (rules F5/S18).

If prettier **throws** on the input (a parse rejection or a printer crash — also no `output_prettier.*` possible), add a `prettier_rejects.txt` marker + README instead. The marker's trimmed content is the position-stripped expected-error substring; the validator live-verifies that prettier still errors with that message (rules F6/S19). The input must be valid by tsv's parse oracle (Svelte / acorn-typescript) and idempotent under tsv. Hand-author it — `fixture_init` runs prettier, which throws — then `deno task fixtures:update:parsed` for `expected.json`. See ./fixture_overview.md (rules F6/S19) and the catalog of in-tree cases in ./conformance_prettier.md §"Prettier rejects valid input".

---

## Steps 3-4: See It Fail + Approval Gate

Run validation to confirm the fixture fails as expected:

```bash
deno task fixtures:validate <pattern>
```

**⚠️ APPROVAL GATE — STOP HERE.** Show the user:

- The input file content
- The failing diff
- Your proposed fix approach

Wait for explicit approval before writing ANY implementation code. This is a hard stop — not a suggestion.

**The gate resets on rework.** If the user gives feedback that requires changing the fixture (naming, structure, cases, etc.), redo steps 1-3 and return here for approval again. Every version of the fixture must pass through this gate before implementation begins.

---

## Step 5: Implement

Fix parser/formatter errors so the fixture passes validation:

```bash
deno task fixtures:validate [pattern]
```

If errors exist:

1. Fix the implementation (parser, printer, converter)
2. **Never modify fixtures to work around bugs** — fix the code
3. If a feature genuinely can't be implemented yet, document in TODO and defer the fixture

---

## Step 6: Validate + Variants

### 6.1 Validate

```bash
deno task fixtures:validate [pattern]
```

All checks must pass.

### 6.2 Add Variants

Variants stress-test normalization — unusual formatting that should collapse to the canonical input. Extension must match input file (`input.svelte` → `unformatted_*.svelte`, etc.).

- `unformatted_compact` → Minimal whitespace — `if(cond){expr;}`
- `unformatted_spaces` → Excessive whitespace — `if  (  cond  )  {  expr  ;  }`

**Preserve blank lines between statements** — prettier preserves them, so compact variants must too. Without matching blank lines, the variant won't normalize to input.

```svelte
<!-- input.svelte -->
<script>
	if (cond) {
		expr;
	}

	if (a) {
		expr;
	} else {
		expr;
	}
</script>

<!-- unformatted_compact.svelte — note the blank line is preserved -->
<script>
if(cond){expr;}

if(a){expr;}else{expr;}
</script>
```

In `_prettier_divergence` directories: use `unformatted_ours_*.*` instead (normalizes with our formatter only).

### 6.3 Verify Variants Normalize

```bash
# Both must produce output identical to input.svelte
cargo run -p tsv_debug format_prettier .../unformatted_compact.svelte 2>/dev/null > /tmp/c.svelte
cargo run -p tsv_debug format_prettier .../unformatted_spaces.svelte 2>/dev/null > /tmp/s.svelte

diff .../input.svelte /tmp/c.svelte && echo "✓ compact normalizes"
diff .../input.svelte /tmp/s.svelte && echo "✓ spaces normalizes"
```

**If a variant doesn't normalize**: It may be a `prettier_variant_*.*` (prettier stable, ours normalizes to input) or `variant_*.*` (both formatters keep stable). Use `deno task fixtures:audit <pattern>` to investigate. See [fixture_overview.md](./fixture_overview.md#unformatted-variant-doesnt-normalize-prettier-variant-discovery).

---

### 6.4 Mark Complete

Update the relevant TODO document if applicable. State: **"Completed: [item] - N fixtures in [paths]"**

---

## Pre-Implementation Fixtures

When the feature doesn't exist yet, `fixture_init` still works — prettier doesn't need our parser. Follow the normal steps but skip "our parser" checks in step 2 (they'll fail) and skip variants in step 6:

```bash
deno task fixtures:validate --prettier-only [pattern]  # skips our formatter, validates prettier + canonical parser
```

Formatter errors like `InvalidSyntax` are expected until implementation. The approval gate (step 4) still applies. To regenerate `expected.json` after upstream parser changes: `deno task fixtures:update:parsed [pattern]`

---

## Worked Example: Creating `statements/if/basic`

```bash
# Step 0: LOAD CONTEXT
# "Working on: if/else statements"
# Read fixture_workflow.md + fixture_naming.md (done)
cat tests/fixtures/typescript/declarations/function/*/input.svelte  # study existing

# Step 1: CREATE FIXTURE (content does NOT need to be formatted)
cargo run -p tsv_debug fixture_init tests/fixtures/typescript/statements/if/basic --stdin << 'EOF'
<script>
if(cond){expr;}

if(a){expr;}else{expr;}

if(a){expr;}else if(b){expr;}else{expr;}
</script>
EOF
# ✓ input.svelte (prettier-formatted)
# ✓ expected.json

# Step 2: REVIEW
cat tests/fixtures/typescript/statements/if/basic/input.svelte  # verify generic names, structure
# Checklist: empty body? no braces? nested? ← Found gap: no "if without braces"
cargo run -p tsv_cli parse tests/fixtures/typescript/statements/if/basic/input.svelte --pretty | head -30

# Step 3: SEE IT FAIL
deno task fixtures:validate statements/if

# Step 4: ★ APPROVAL GATE - present input + failing diff to user, wait for "lgtm"

# Step 5: IMPLEMENT - fix any parser/formatter errors
deno task fixtures:validate statements/if  # should pass after implementation

# Step 6: VALIDATE + VARIANTS
# Note: blank lines between statements must be preserved!
cat > tests/fixtures/typescript/statements/if/basic/unformatted_compact.svelte << 'EOF'
<script>
if(cond){expr;}

if(a){expr;}else{expr;}

if(a){expr;}else if(b){expr;}else{expr;}
</script>
EOF

cargo run -p tsv_debug format_prettier tests/fixtures/typescript/statements/if/basic/unformatted_compact.svelte 2>/dev/null > /tmp/c.svelte
diff tests/fixtures/typescript/statements/if/basic/input.svelte /tmp/c.svelte && echo "✓ normalizes"

deno task fixtures:validate statements/if  # all checks pass
# "Completed: if/else statements - 3 fixtures in statements/if/"
```

---

## Quick Reference

```bash
# Create a new fixture (formats through prettier automatically)
cargo run -p tsv_debug fixture_init tests/fixtures/.../name --content '<script>code</script>'
cargo run -p tsv_debug fixture_init tests/fixtures/.../name --stdin << 'EOF'
<script>multiline code</script>
EOF
cargo run -p tsv_debug fixture_init tests/fixtures/.../name  # reformat existing input file

# Compare our output vs prettier (most useful for quick testing)
cargo run -p tsv_debug compare FILE
cargo run -p tsv_debug compare --content "<script>CODE</script>" --parser svelte
cargo run -p tsv_debug compare --content "const x = 1" --parser typescript

# Verify fixture matches prettier (manual check)
cargo run -p tsv_debug format_prettier FILE 2>/dev/null > /tmp/p.svelte && diff FILE /tmp/p.svelte && echo "✓ MATCH"

# Check our parser output
cargo run -p tsv_cli parse FILE --pretty | head -50

# Check canonical parser (Svelte/acorn for .svelte, acorn+typescript for .ts)
cargo run -p tsv_debug canonical_parse FILE 2>/dev/null | head -50

# Measure line width (for long fixtures)
cargo run -p tsv_debug line_width FILE --line N

# Validate fixtures
deno task fixtures:validate [pattern]

# Count fixtures (all input types)
find tests/fixtures -name "input.svelte" -o -name "input.ts" -o -name "input.css" | wc -l
```

---

## Using Fixtures to Track Bugs

Add the failing case to `input.svelte` (prettier's canonical form), run validation (it will fail), and document with a TODO. The failing fixture is a regression test that passes once the bug is fixed.

```bash
cargo run -p tsv_debug compare --content "<script>import {} from 'x';</script>" --parser svelte
```

**Keep failing tests.** Never remove test cases to make tests pass — fix the code instead.

- Formatter bug (output differs) — Keep — fix the formatter
- Parser bug (parse error on valid code) — Keep — fix the parser
- Feature reveals OTHER unrelated bugs — Keep — fix those bugs too
- Parser genuinely can't parse (not impl) — Document in TODO, defer fixture until parser works

Only defer when the parser can't parse the syntax yet (use `--prettier-only` until ready).

---

## Invalid Syntax Fixtures

`input_invalid_<description>.<ext>` files test that parsers correctly **reject** invalid syntax. Both our parser and the canonical parser must reject the file.

```bash
cat > tests/fixtures/.../input_invalid_await_const.svelte << 'EOF'
<script lang="ts">
	const await = 'a';
</script>
EOF
cargo run -p tsv_debug canonical_parse tests/fixtures/.../input_invalid_await_const.svelte  # must fail
deno task fixtures:validate [pattern]
```

Rules: one syntax error per file, minimal content, no `expected.json`, descriptive suffixes (`_const`, `_param`, `_destructure`, `_label`). If our parser accepts but canonical rejects, our parser is too permissive (bug).

---

## Common Mistakes

Mistake → symptom — fix:

- Domain-specific names → `describe`, `Logger`, `UserProfile` in fixture — Use generic names per ./fixture_naming.md
- Code change before fixture → Implementation attempted without proof of divergence — Stop, create fixture, get approval, then implement
- Skipping naming doc → Fixture requires redo after review — Always read ./fixture_naming.md in step 0
- Too many test cases → >6 unrelated patterns in one fixture — Focus on 3-6 related patterns per fixture
- Missing blank lines in variants → `unformatted_compact` doesn't normalize to input — Preserve blank lines between logical groups

## See Also

- ./fixture_overview.md - Validation rules, divergence patterns, troubleshooting
- ./fixture_naming.md - Naming conventions, `long` fixture details
