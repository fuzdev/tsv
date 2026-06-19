# test262 Conformance Workflow

> Systematic workflow for improving parser conformance against the ECMAScript test suite

## First Step: Load Conformance Doc

**ALWAYS start by reading the conformance documentation:**

```bash
cat docs/conformance_test262.md
```

This document contains:

- Current full-suite results (§Current Results — refresh after conformance work)
- Scope (what's tested vs skipped) and test directory priorities
- The command interface and frontmatter/execution design
- Design decisions (strict mode only)

The known-gap backlog is tracked with the project's planning notes, not in
the conformance doc.

**This workflow doc describes HOW to work. The conformance doc describes WHAT we're testing against.**

---

## Core Rule: ONE FAILURE CATEGORY AT A TIME

**Process failures by pattern. Never try to fix multiple unrelated issues at once.**

### Single-Category Workflow

```bash
# 1. Run test262 with filter to find a failure pattern
cargo run -p tsv_debug test262 language/expressions --verbose 2>&1 | head -100

# 2. Identify a pattern (e.g., "arrow function" or "class" related failures)
# 3. Create minimal reproduction
cargo run -p tsv_cli parse --content 'a /* comment */ => x' --parser typescript --pretty

# 4. Follow the fix workflow below
```

**For each failure pattern, one of three outcomes:**

**A) Out of scope** → Document and move on

- Sloppy-mode only features (we're strict-mode only)
- AnnexB web-compat features (lower priority)
- Runtime/resolution phase errors (we only test parsing)

**B) Early error detection missing** → Lower priority, track for later

- Duplicate parameter detection
- Invalid escape sequences
- These parse successfully but should fail

**C) Parser bug** → Create fixture, **★ GET APPROVAL ★**, implement fix

---

## Workflow Phases

```
DISCOVER → CATEGORIZE → CHECK FIXTURES → CREATE FIXTURE (if missing) → IMPLEMENT → VERIFY
                                  ↓                   ↓
                            Exists?              ★ USER APPROVAL
                            Skip to IMPLEMENT       REQUIRED
```

---

## Phase 1: Discover

### Run test262 with Filters

```bash
# Full suite (slow, ~50k tests)
cargo run -p tsv_debug test262

# Filter by directory (recommended for focused work)
cargo run -p tsv_debug test262 language/expressions
cargo run -p tsv_debug test262 language/statements
cargo run -p tsv_debug test262 language/types

# Only positive tests (should parse but don't)
cargo run -p tsv_debug test262 --positive-only --verbose

# Only negative tests (should fail but parse)
cargo run -p tsv_debug test262 --negative-only --verbose

# List tests without running
cargo run -p tsv_debug test262 language/expressions/arrow --list
```

### Interpret Results

Example output (numbers illustrative — current full-suite results are in
[conformance_test262.md §Current Results](./conformance_test262.md#current-results)):

```
Results:
  Positive tests: 41000 passed, 1100 failed
  Negative tests: 1400 passed, 3000 failed
  Skipped:        2600 (sloppy mode: 2500, runtime: 50, resolution: 50)

Pass rate: 42400/46500 (91.2%)
```

**Focus areas:**

- **Positive failed**: Parser rejects valid syntax (higher priority)
- **Negative failed**: Parser accepts invalid syntax (early error detection)

---

## Phase 2: Categorize

### Priority Matrix

- Positive failed, many tests — **High** — Fix immediately - blocking valid code
- Positive failed, few tests — Medium — Fix when addressing related code
- Negative failed, early error — Lower — Track for later - code runs but shouldn't
- Sloppy-mode required — Skip — Out of scope (strict-mode only design)
- AnnexB features — Lower — Web-compat, not essential

### Common Failure Patterns

**Positive failures (should parse but don't):**

- Comments in unusual positions (between tokens)
- ASI edge cases
- Unicode identifiers
- `new function(){}` expressions

**Negative failures (should fail but parse):**

- Duplicate parameters
- Rest parameter with initializer `[...x = y]`
- Escaped reserved words `\u0061wait`
- Strict mode early errors

### Tracking Template

When you find a new pattern, document it:

```markdown
### [Pattern Name]

**Test filter:** `cargo run -p tsv_debug test262 language/path/pattern`
**Tests affected:** ~N tests
**Example test:** test/language/expressions/example.js
**Type:** positive-failed | negative-failed
**Priority:** high | medium | lower
**Related fixture:** tests/fixtures/typescript/... OR "none"
**Status:** discovered | fixture-created | implemented | verified
```

---

## Phase 3: Fixture Review

Before creating fixtures, check if the pattern is already covered.

### Search Existing Fixtures

```bash
# TypeScript expression fixtures
ls tests/fixtures/typescript/expressions/

# Search by keyword
find tests/fixtures -name "*arrow*" -type d
grep -r "pattern" tests/fixtures/typescript/
```

### Load Naming Conventions

**ALWAYS read before creating fixtures:**

```bash
cat docs/fixture_naming.md
```

---

## Phase 4: Fixture Creation

**CRITICAL: Create fixtures BEFORE changing code.**

### 4.1 Create Minimal Reproduction

Extract the failing pattern from test262:

```bash
# Read the test file
cat ../test262/test/language/expressions/arrow/example.js

# Test with our parser
cargo run -p tsv_cli parse --content 'a /* comment */ => x' --parser typescript --pretty

# Compare with canonical (acorn)
cargo run -p tsv_debug canonical_parse --content 'a /* comment */ => x' --parser typescript
```

### 4.2 Create Fixture

```bash
mkdir -p tests/fixtures/typescript/[category]/[pattern_name]
```

### 4.3 Write input.ts (or input.svelte)

**Prefer `.svelte` for most cases** (exercises same parser, has canonical source). Use `.ts` only for file-level features that can't exist inside `<script>` (e.g., hashbang at byte 0).

```svelte
<script lang="ts">
// Minimal reproduction of the pattern
const f = a /* comment */ => x;
</script>
```

### 4.4 Generate expected.json

```bash
deno task fixtures:update:parsed [pattern]
```

### 4.5 Validate Structure

```bash
# First, verify fixture structure (skip our parser/formatter)
deno task fixtures:validate [pattern] --prettier-only

# Then see our parser fail
deno task fixtures:validate [pattern]
```

The fixture **should fail** at this point. This is correct - it defines the target behavior.

### 4.6 ★ GET USER APPROVAL ★

**STOP HERE. Do not proceed without approval.**

Present to user:

- Fixture location and content
- Which test262 tests this addresses
- What parser behavior needs to change

---

## Phase 5: Implement

**User has approved. Now fix the parser.**

### 5.1 Locate Relevant Code

```
crates/tsv_ts/src/parser/    # TypeScript parser
crates/tsv_ts/src/lexer/     # Tokenization
crates/tsv_lang/src/         # Shared utilities
```

### 5.2 Make Changes

Fix the parser to handle the pattern correctly.

### 5.3 Verify Fix

```bash
# Fixture passes
deno task fixtures:validate [pattern]

# Test262 tests pass
cargo run -p tsv_debug test262 [filter] --verbose
```

---

## Phase 6: Verify

### Re-run test262

```bash
cargo run -p tsv_debug test262
```

Confirm:

- Pass rate improved
- Specific test category passes
- No regressions

Then refresh [conformance_test262.md §Current Results](./conformance_test262.md#current-results)
with the new counts.

### Run Full Test Suite

```bash
deno task check
```

---

## Quick Reference

```bash
# Discover failures
cargo run -p tsv_debug test262 --positive-only --verbose 2>&1 | head -50

# Reproduce specific test
cat ../test262/test/language/path/to/test.js
cargo run -p tsv_cli parse --content '...' --parser typescript

# Compare with canonical parser
cargo run -p tsv_debug canonical_parse --content '...' --parser typescript

# Validate fixture
deno task fixtures:validate pattern

# Full validation
deno task check
```

---

## Anti-Patterns

### Never Do These

1. **Fix parser before fixture exists**
   - Fixtures define correct behavior
   - Without a fixture, no specification

2. **Modify expected.json to make tests pass**
   - expected.json comes from canonical parser
   - If tests fail, fix our parser

3. **Try to fix multiple unrelated failures at once**
   - Each pattern needs focused attention
   - Mixing fixes makes debugging harder

4. **Skip the approval gate**
   - User approval ensures fixture is correct
   - Catching errors early saves rework

5. **Add AnnexB/sloppy features without discussion**
   - We're strict-mode only by design
   - Adding sloppy mode is a major scope change

### Red Flags

- "Let me just fix this one thing" — Missing fixture first
- Many unrelated changes in one PR — Not focused on single pattern
- Pass rate dropped — Regression introduced
- Modified expected.json — Hiding parser bug
