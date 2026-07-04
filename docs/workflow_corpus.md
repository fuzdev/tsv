# Corpus-Driven Formatting Conformance Workflow

> Systematic workflow for identifying and fixing formatting differences using corpus comparison

This doc covers the **formatting** comparison (`corpus:compare:format`, vs
prettier). The parser-side analogue — `corpus:compare:parse`, deep-diffing
parse ASTs against the canonical parsers — is documented in
../benches/js/CLAUDE.md §Parse Comparison; its diffs are triaged with the
fixture-first TDD flow rather than this file's hunk workflow.

## First Step: Load Conformance Doc

**ALWAYS start by reading the conformance documentation:**

```bash
cat docs/conformance_prettier.md
```

This document contains all intentional Prettier divergences with rationale and fixture references.

**This workflow doc describes HOW to work. The conformance doc describes WHERE we intentionally differ.**

---

## Core Rule: ONE FILE AT A TIME

**Process each corpus file individually. Never batch or parallelize.**

### Step-by-Step Process

**Step 1: Get the next differing file**

```bash
deno task corpus:compare:format:run --all --exit-on-first
```

**Step 2: Examine the diff and compare with conformance doc**

```bash
# Detailed diff for the file
cargo run -p tsv_debug compare ~/dev/zzz/src/path/to/file.svelte

# Check if the divergence detector recognizes it
deno task corpus:compare:format:run ~/dev/zzz --explain --exit-on-first
```

Read `docs/conformance_prettier.md` and compare the diff against documented divergences.

**Step 3: Classify — one of three outcomes:**

---

**A) Already detected as known** → Move to next file

The divergence detector already identifies this pattern. Nothing to do.

---

**B) Known divergence but detector misses it** → Fix the detector

The diff matches a documented pattern in `conformance_prettier.md`, but the detector in
`benches/js/lib/divergence/patterns.ts` doesn't recognize this variant.

1. Identify which existing pattern should match (e.g., `inline_content_hug`, `fill_101_boundary`)
2. Read the pattern's `detect()` function in `patterns.ts`
3. Understand why it fails to match this specific diff
4. Broaden the pattern (without overmatching)
5. Add a positive test case in `patterns_test.ts` matching the new variant
6. Verify: `deno task test:deno` (all tests pass)
7. Verify: `deno task corpus:compare:format:run --all --explain` (unknown count decreased)
8. Move to next file

---

**C) Genuine unknown difference (formatter bug)** → Create fixture, **GET APPROVAL**, implement fix

The diff does NOT match any documented pattern in `conformance_prettier.md`. This is a real formatting bug.

1. Read `./docs/fixture_naming.md` and check existing fixtures
2. Create a minimal fixture that demonstrates the issue
3. **GET USER APPROVAL ON THE FIXTURE** (see [Phase 4.6](#46--get-user-approval-) for details)
   - If working from an **approved plan** that specifies the fixture, approval is already satisfied — proceed directly
   - Otherwise, **STOP** and present the fixture for approval before continuing
4. Fix the code to make the fixture pass
5. Verify no regressions with `deno task fixtures:validate`
6. Verify corpus improvement: `deno task corpus:compare:format:run --all --explain`
7. Move to next file

---

### Why One File at a Time?

- **Focus**: Each diff is analyzed thoroughly
- **Traceability**: Clear cause-effect between diff and fix
- **No guesswork**: You see exactly what's different before deciding
- **Automatic categorization**: Known divergences are detected automatically

### Anti-Patterns (Inline)

- Don't try to fix multiple issues at once
- Don't skip the approval gate to "move faster" (plan-mode approval counts — interactive re-approval is redundant)
- Don't ignore "unknown" differences - investigate them
- **Don't implement fixes before getting fixture approval** - even if the fix is obvious

## Quick Reference

```bash
# 1. Discover: Find differences (builds FFI first)
deno task corpus:compare:format ~/dev/zzz

# 2. Isolate: Test minimal reproduction
cargo run -p tsv_debug compare --content '<tag>...</tag>' --parser svelte

# 3. Fixture: Create or find existing
find tests/fixtures -name "*pattern*"
deno task fixtures:validate pattern

# 4. After fixing: Verify
deno task fixtures:validate  # All fixtures pass
deno task corpus:compare:format ~/dev/zzz  # Match rate improved
```

## Workflow Phases

```
DISCOVER → CATEGORIZE → CHECK FIXTURES → CREATE FIXTURE (if missing) → IMPLEMENT → VERIFY
                                  ↓                   ↓
                            Exists?              ★ USER APPROVAL
                            Skip to IMPLEMENT       REQUIRED
                                                 (satisfied by plan approval
                                                  OR interactive approval)
```

**Phase 1: DISCOVER** - Run corpus:compare:format, identify patterns
**Phase 2: CATEGORIZE** - Default output shows all unexplained diffs; use `--explain` for pattern details
**Phase 3: CHECK FIXTURES** - Search for existing coverage
**Phase 4: CREATE FIXTURE** - Define target behavior (★ requires approval)
**Phase 5: IMPLEMENT** - Fix code to match fixtures
**Phase 6: VERIFY** - Confirm improvement, no regressions

---

## Phase 1: Discover

### Run Corpus Comparison

```bash
# Full comparison - builds FFI first (all languages)
deno task corpus:compare:format ~/dev/zzz

# Filtered by language
deno task corpus:compare:format ~/dev/zzz --filter svelte
deno task corpus:compare:format ~/dev/zzz --filter typescript
deno task corpus:compare:format ~/dev/zzz --filter css

# Verbose mode (see each file as processed)
deno task corpus:compare:format ~/dev/zzz --verbose

# Skip rebuild (if FFI already up-to-date)
deno task corpus:compare:format:run ~/dev/zzz

# Machine-readable: single JSON report on stdout (stats + safety/partial/unknown/
# error lists), human output on stderr. Combine with --all / --safety-only / --filter.
deno task corpus:compare:format:run --all --json 2>/dev/null
```

> **Staleness footgun**: `corpus:compare:format` rebuilds the FFI (`build:ffi:corpus`)
> before running; `corpus:compare:format:run` does **not**. Our formatted output
> (`ours`) comes from the compiled FFI, so after a Rust change you must run the
> rebuild variant (or `deno task build:ffi:corpus` yourself) before trusting the
> numbers — otherwise `:run` compares against a stale binary and a fix appears to
> have no effect. The safety/divergence logic itself is TypeScript and always
> live; only the formatter output is gated on the rebuild.

> **Safety is differential vs prettier**: the SAFETY count reports only data loss
> OUR output incurs _beyond_ what prettier does. Shared normalizations (redundant
> leading-`|` removal, number normalization, CSS keyword lowercasing) are not
> flagged even though they drop the source character count, because prettier
> performs them too. A flagged SAFETY file is genuine over-normalization or
> dropped content relative to prettier — see
> [divergence_detector.md](./divergence_detector.md#differential-against-prettier-false-positive-guard).

### Interpret Results

```
Results:
  svelte       N/N match (X%)    | N known | N partial | N unknown | N errors
  typescript   N/N match (X%)    | N known | N partial | N unknown
  css          N/N match (X%)    | N known
  total        N/N match (X%)    | N known | N partial | N unknown | N errors
```

**Metrics:**

- **match**: Our output exactly equals Prettier's
- **known**: All diff hunks explained by documented divergence patterns
- **partial**: Some hunks explained, some not (needs investigation)
- **unknown**: No hunks explained (needs investigation)
- **errors**: Parse or format errors (investigate separately)

### Examine Diffs

The default output shows unified diffs for all unexplained differences (prettier = expected, ours = actual). For partial files, only the unexplained hunks are shown. For unknown files, the full diff is shown.

---

## Phase 2: Categorize

### Triage All Unexplained Diffs

The default output shows every unexplained diff — partial file hunks and full unknown file diffs:

```bash
# All unexplained diffs at once (recommended starting point)
deno task corpus:compare:format --all

# Single project
deno task corpus:compare:format ~/dev/zzz

# Compact output without diffs
deno task corpus:compare:format --all --summary
```

This replaces manually running `cargo run -p tsv_debug compare <file>` on each unknown file.

### Check Intentional Divergences

**Before assuming a difference is a bug, verify it's not an intentional design choice.**

```bash
# Use --explain to also list known divergences with their patterns
deno task corpus:compare:format ~/dev/zzz --explain

# Check conformance_prettier.md for detailed rationale
cat docs/conformance_prettier.md

# Search for existing _prettier_divergence fixtures
find tests/fixtures -name "*prettier_divergence*" -type d
```

The default output shows unexplained diffs and which patterns explain the explained hunks. Focus on files classified as `unknown` or `partial` — those are where real bugs live.

**If the difference is detected as "known":** Not a bug. Move to the next file.

**If the difference is "unknown":** Likely a bug — proceed with fixture creation. Or it might be a NEW intentional divergence that needs documenting in `conformance_prettier.md` and a detector pattern added.

---

## Phase 3: Fixture Review

Before creating fixtures, check if the pattern is already covered.

### Search Existing Fixtures

```bash
# Find fixtures by keyword
ls tests/fixtures/svelte/elements/
ls tests/fixtures/typescript/expressions/
ls tests/fixtures/css/at_rules/

# Search fixture content
grep -r "pattern" tests/fixtures/

# Check for similar naming
find tests/fixtures -name "*keyword*" -type d
```

### Load Naming Conventions

**ALWAYS read before creating fixtures:**

```bash
cat docs/fixture_naming.md
```

Key reminders:

- Generic names: `Comp`, `text`, `prop`, `a`, `b`, `expr`
- Numeric suffixes only for multiples: `Comp1`, `Comp2` (not `Comp`, `Comp2`)
- `_long` suffix for width-based wrapping tests
- `_prettier_divergence` suffix for intentional differences

### Study Similar Fixtures

Find 2-3 similar fixtures and understand their structure:

```bash
# Example: studying element attribute fixtures
cat tests/fixtures/svelte/elements/*/input.svelte
```

Note:

- How many examples per fixture
- What edge cases are included
- Whether unformatted variants exist

---

## Phase 4: Fixture Creation

**CRITICAL: Create fixtures BEFORE changing code. Fixtures define target behavior.**

### 4.1 Isolate the Pattern

Extract a minimal reproduction from the corpus file:

```bash
# Compare specific content
cargo run -p tsv_debug compare --content '<div class="x" data-attr="y"></div>' --parser svelte
```

Reduce to the smallest case that shows the difference.

### 4.2 Create Fixture Directory

```bash
mkdir -p tests/fixtures/[language]/[category]/[pattern_name]
```

### 4.3 Create input.svelte

**Prefer `.svelte` over `.ts` or `.css`** - even for TypeScript/CSS-only patterns. The Svelte context (`<script>`/`<style>`) exercises the same code paths and has Prettier + Svelte plugin as a canonical source. Use `.ts` or `.css` only for file-level features that can't exist inside Svelte (e.g., hashbang at byte 0, BOM handling).

Write the **Prettier-formatted** version (the target):

```bash
# Get Prettier's output
cargo run -p tsv_debug format_prettier --content '<div class="x"></div>' --parser svelte

# Or format a file
cargo run -p tsv_debug format_prettier /tmp/test.svelte 2>/dev/null > tests/fixtures/.../input.svelte
```

**Verify input matches Prettier:**

```bash
cargo run -p tsv_debug format_prettier tests/fixtures/.../input.svelte 2>/dev/null > /tmp/p.svelte
diff tests/fixtures/.../input.svelte /tmp/p.svelte && echo "✓ MATCH"
```

### 4.4 Generate expected.json

```bash
deno task fixtures:update:parsed [pattern]
```

### 4.5 Validate Fixture Structure

```bash
# First, validate with prettier only (skip our formatter)
deno task fixtures:validate [pattern] --prettier-only

# Then check what fails with our formatter
deno task fixtures:validate [pattern]
```

The `--prettier-only` flag validates fixture structure without running our parser/formatter. Use this to verify:

- input.svelte matches prettier's output (idempotent)
- unformatted variants normalize correctly via prettier
- Fixture files are properly structured

At this point the fixture **should fail** when run without `--prettier-only` (because our formatter doesn't match yet). This is expected and correct - the failing test defines the target behavior.

**If the fixture passes immediately**, either:

- The issue was already fixed (verify with corpus comparison)
- The fixture doesn't capture the actual difference (re-examine the pattern)

### 4.6 ★ GET USER APPROVAL ★

**Do not proceed to implementation without user approval.**

There are two ways to satisfy this gate:

1. **Plan-mode approval**: If the user approved a plan that includes the fixture path, content, and fix strategy, approval is already satisfied. Proceed directly to Phase 5.
2. **Interactive approval**: If discovering issues during corpus comparison (no pre-approved plan), STOP and present the fixture to the user:
   - Show the fixture location and structure
   - Explain what behavior it tests
   - Wait for explicit approval before fixing code

This gate ensures:

- Fixture is in the correct location (category/naming)
- Fixture captures the intended behavior
- No wasted effort if the approach is wrong

---

## Phase 5: Implement

**User has approved the fixture. Now fix the code.**

### 5.1 Locate Relevant Code

Common locations:

- `crates/tsv_svelte/src/printer/` - Svelte formatting
- `crates/tsv_ts/src/printer/` - TypeScript formatting
- `crates/tsv_css/src/printer/` - CSS formatting

### 5.2 Make Changes

Fix the formatter to match Prettier's behavior (as defined by the fixture).

### 5.3 Verify Fix

```bash
# Run fixture validation
deno task fixtures:validate [pattern]

# Compare the specific file
cargo run -p tsv_debug compare tests/fixtures/.../input.svelte
```

### 5.4 Add Unformatted Variants

After the fix works, add normalization tests:

```bash
# Create unformatted_compact.svelte - minimal whitespace
# Create unformatted_spaces.svelte - excessive whitespace
```

Verify variants normalize:

```bash
cargo run -p tsv_debug format_prettier tests/fixtures/.../unformatted_compact.svelte 2>/dev/null > /tmp/c.svelte
diff tests/fixtures/.../input.svelte /tmp/c.svelte && echo "✓ compact normalizes"
```

---

## Phase 6: Verify

### Re-run Corpus Comparison

```bash
deno task corpus:compare:format ~/dev/zzz --filter [language]
```

Confirm:

- Match rate improved
- The specific files that were differing now match
- No regressions introduced

### Run Full Test Suite

```bash
deno task check
```

---

## Safety Check: Character Frequency Approach

The safety check uses **character frequency comparison**, run **differentially
against prettier** (`check_safety_vs_prettier`), to detect data loss:

1. Count all non-formatting characters in source, our output, and prettier's output
2. Per character, compute how many our output drops/adds vs source, and how many
   prettier drops/adds vs source
3. Report only the remainder our output incurs **beyond** prettier:
   `real = max(0, ours_delta − prettier_delta)`

Prettier is the source of truth, so a normalization prettier also performs
(redundant leading `|` removal, number normalization, CSS keyword lowercasing) is
not data loss even though it drops the source character count — the differential
cancels it. A violation survives only when our output drops/adds a semantic
character that prettier preserves.

**Excluded characters** (formatting may change these):

- Whitespace: space, tab, newline, carriage return
- Quotes: `'` `"` `` ` `` (style normalization)
- Parens: `( )` (optional in arrow params, grouping)
- Separators: `, ;` (trailing commas, ASI)

**Detected characters** (must be preserved):

- Letters: a-z, A-Z (identifiers, keywords, comments)
- Digits: 0-9 (numbers)
- Brackets: `[ ] { } < >` (arrays, objects, generics, JSX)
- Operators: `+ - * / % = ! & | ^ ~ ? : . @ # $ _` etc.

**Example violation output:** each char shows `real` count with its shared
context — the first labeled `(ours N, prettier M)`, the rest bare `(N, M)`:

```
content_lost: 7 beyond prettier — '|'×2 (ours 28, prettier 26), '/'×2 (2, 0), '*'×2 (2, 0), '4'×1 (1, 0)
```

The `'|'×2 (ours 28, prettier 26)` reads as: of the 28 pipes our output dropped,
26 are shared with prettier (a normalization, not loss) and only 2 are real. The
`(2, 0)` entries are fully real — prettier dropped none.

Safety violations fail the corpus check immediately — they are never skipped.

---

## Reference

### Corpus Compare Options

```bash
deno task corpus:compare:format --all [options]       # the gates corpus view (~6,000 files)
deno task corpus:compare:format <path> [options]      # Scans <path> recursively
deno task corpus:compare:format:run <path> [options]  # Skip FFI build (faster iteration)

Options:
  --all             Compare the gates corpus view (~6,000 files: real repos + the
                    prettier fixture suites — see benches/js/CLAUDE.md §Corpus)
  --filter <lang>   Only compare files of this language (svelte, typescript, css)
  --limit <n>       Limit to first n files per language
  --verbose         Show each file as it's processed
  --exit-on-first   Stop after finding the first mismatch or error (shows diff)
  --safety-only     Only check for safety violations (data loss)
  --explain         Show detected divergence patterns for each difference
  --summary         Compact output (no diffs, just file lists with brief descriptions)
  --strict          Fail on any difference (disable divergence detection)
  --audit-patterns  Per-pattern corpus coverage with sample diffs
```

### Divergence Audit

```bash
deno task divergence:audit        # Cross-reference patterns vs conformance_prettier.md
deno task divergence:audit --json # Machine-readable JSON output
```

### Debug Commands

```bash
# Compare single file/content
cargo run -p tsv_debug compare FILE
cargo run -p tsv_debug compare --content '<div>test</div>' --parser svelte

# Format with Prettier
cargo run -p tsv_debug format_prettier FILE

# Check line widths (for long fixtures)
cargo run -p tsv_debug line_width FILE --line N
```

### Key Documentation

- ./fixture_workflow.md - Complete fixture creation process
- ./fixture_naming.md - Naming conventions (ALWAYS read before creating fixtures)
- ./fixture_overview.md - Validation rules and patterns
- ./conformance_prettier.md - **Intentional Prettier divergences** (check before fixing a "bug")

---

## Anti-Patterns

### Never Do These

1. **Change code before fixtures exist**
   - Fixtures define correct behavior
   - Without a fixture, there's no specification

2. **Modify fixtures to make tests pass**
   - Fixtures are the source of truth
   - If tests fail, fix the code

3. **Create fixtures for buggy behavior**
   - `output_prettier.svelte` is for INTENTIONAL differences
   - Not for "our formatter is wrong here"

4. **Skip approval gates**
   - User approval ensures fixtures are correct
   - Catching errors early saves rework

5. **Create fixtures without checking existing ones**
   - May duplicate existing coverage
   - Miss established patterns

### Red Flags

- "Let me just fix this one thing" — Missing fixture first
- `output_prettier.svelte` in many fixtures — Not matching Prettier (bugs)
- Fixtures with domain-specific names — Not following naming conventions
- Tests passing after fixture changes — Modified fixture to hide bug
