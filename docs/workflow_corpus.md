# Corpus-Driven Formatting Conformance Workflow

> Systematic workflow for identifying and fixing formatting differences using corpus comparison

This doc covers the **formatting** comparison (`corpus:compare:format`, vs
prettier). The parser-side analogue — `corpus:compare:parse`, deep-diffing
parse ASTs against the canonical parsers — is documented in
../benches/js/CLAUDE.md §Parse Comparison; its diffs are triaged with the
fixture-first TDD flow rather than this file's hunk workflow.

Exit-code notes for `--all` runs: besides SAFETY (which always gates), the
tools fail on an empty scope, an all-errored / zero-compared run (systemic
sidecar/FFI failure), and on the **pinned per-language counts**
(`benches/js/lib/gate_counts.ts` — see [gate_counts.md](gate_counts.md)): exact pins on
`compared` and on the negative buckets (parse's tsv-failure count; format's
`unknown`/`partial` counts — a new one fails until triaged; fixing some means
re-pinning to record the win, which is exactly the loop this document drives), and a
minimum on `match`. All of them hold over the whole `gates` view, because every file in
it comes from a pinned checkout: the `../corpora` real-code snapshot plus the prettier
suites. A snapshot refresh is the one legitimate corpus move, re-pinned deliberately.

Repeat runs are cheap: prettier outputs are served from a content-addressed
cache (../benches/js/CLAUDE.md §Prettier-output cache), so the
one-file-at-a-time loop below re-formats only files whose content changed.

## First Step: Load Conformance Doc

**ALWAYS start by reading the conformance documentation:**

```bash
cat docs/conformance_prettier.md          # the frame: terminology, reason tags, decision framework
cat docs/conformance_prettier_css.md      # and the catalog for the language you're triaging
```

The frame's §Catalogs table indexes the per-language catalogs (`_css`, `_svelte`, `_ts`,
`_ts_comments`, `_ignore`); together they hold every intentional Prettier divergence with
rationale and fixture references.

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
cargo run -p tsv_debug compare ../corpora/collections/zzz/src/path/to/file.svelte

# Check if the divergence detector recognizes it
deno task corpus:compare:format:run ../corpora/collections/zzz --explain --exit-on-first
```

Read `docs/conformance_prettier.md` plus the catalog for the language you're triaging (its
§Catalogs table indexes them) and compare the diff against documented divergences.

**Step 3: Classify — one of three outcomes:**

---

**A) Already detected as known** → Move to next file

The divergence detector already identifies this pattern. Nothing to do.

---

**B) Known divergence but detector misses it** → Fix the detector

The diff matches a documented pattern in the `conformance_prettier*.md` family, but the detector in
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

The diff does NOT match any documented pattern in the `conformance_prettier*.md` family. This is a real formatting bug.

1. Read `./docs/fixture_naming.md` and check existing fixtures
2. Create a minimal fixture that demonstrates the issue
3. **GET USER APPROVAL ON THE FIXTURE** (see [Phase 4.3](#43--get-user-approval-) for details)
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

### Priority order

Work the categories in this order, not in the order the report prints them:

1. **SAFETY (content loss) — always first.** Dropped comments, lost selectors, changed
   values: the user loses code. One root cause routinely covers several files, so a
   SAFETY fix is also the highest-yield one.
2. **Errors (parse failures)** — usually a missing parser feature rather than a printer
   bug. Check whether several errors share one cause before fixing them singly, and
   expect the `unknown`/`partial` counts to **rise** as errors fall: a file that could
   not be parsed at all now reaches the formatter and can disagree with prettier.
3. **Unknown** (no hunk explained) — the standard fixture-first loop.
4. **Partial** (some hunks explained) — investigate the unexplained hunks only.

### Bulk triage — the one carve-out from ONE FILE AT A TIME

When a corpus *expansion* surfaces dozens or hundreds of files at once (adding a suite,
onboarding a repo), grouping comes before fixtures. The rule above governs
**investigation**; it does not mean one fixture per corpus file.

1. **Scan the SAFETY / unknown lists for repeated shapes** — the `Missing:` summaries
   cluster (e.g. many files losing comments around the same keyword pair).
2. **Group by root cause** — take 2–3 representative files per group and bisect them with
   `compare --content`. Same printer path ⇒ one root cause.
3. **One fixture per root cause**, capturing the minimal pattern — not one per file. After
   the fix, verify every file in the group resolved.
4. **Approve at the group level** — present "15 files lose comments between `extends` and
   `implements`; here is the fixture and the root cause", not fifteen separate gates.

This is what keeps a 150-file surface from becoming 150 fixtures when 10 cover it.

## Workflow Phases

```
DISCOVER → CATEGORIZE → CHECK FIXTURES → CREATE FIXTURE (if missing) → IMPLEMENT → VERIFY
                              ↓                     ↓
                       already covered?     ★ USER APPROVAL REQUIRED
                       skip to IMPLEMENT    (plan-mode approval counts)
```

| Phase | What it settles | Command that drives it |
| --- | --- | --- |
| [1 DISCOVER](#phase-1-discover) | what differs | `deno task corpus:compare:format --all` |
| [2 CATEGORIZE](#phase-2-categorize) | known vs partial vs unknown | `deno task corpus:compare:format --all --explain` |
| [3 CHECK FIXTURES](#phase-3-fixture-review) | is the pattern already pinned? | `find tests/fixtures -name '*pattern*' -type d` |
| [4 CREATE FIXTURE](#phase-4-fixture-creation) | ★ the target behavior | `cargo run -p tsv_debug fixture_init …` |
| [5 IMPLEMENT](#phase-5-implement) | the fix | `deno task fixtures:validate <pattern>` |
| [6 VERIFY](#phase-6-verify) | improvement, no regressions | `deno task corpus:compare:format --all` |

---

## Phase 1: Discover

### Run Corpus Comparison

```bash
# Full comparison - builds FFI first (all languages)
deno task corpus:compare:format ../corpora/collections/zzz

# Filtered by language
deno task corpus:compare:format ../corpora/collections/zzz --filter svelte
deno task corpus:compare:format ../corpora/collections/zzz --filter typescript
deno task corpus:compare:format ../corpora/collections/zzz --filter css

# Verbose mode (see each file as processed)
deno task corpus:compare:format ../corpora/collections/zzz --verbose

# Skip rebuild (if FFI already up-to-date)
deno task corpus:compare:format:run ../corpora/collections/zzz

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
deno task corpus:compare:format ../corpora/collections/zzz

# Compact output without diffs
deno task corpus:compare:format --all --summary
```

This replaces manually running `cargo run -p tsv_debug compare <file>` on each unknown file.

### Check Intentional Divergences

**Before assuming a difference is a bug, verify it's not an intentional design choice.**

```bash
# Use --explain to also list known divergences with their patterns
deno task corpus:compare:format ../corpora/collections/zzz --explain

# Check the conformance docs for detailed rationale (frame + the language's catalog)
cat docs/conformance_prettier.md docs/conformance_prettier_svelte.md

# Search for existing _prettier_divergence fixtures
find tests/fixtures -name "*prettier_divergence*" -type d
```

The default output shows unexplained diffs and which patterns explain the explained hunks. Focus on files classified as `unknown` or `partial` — those are where real bugs live.

**If the difference is detected as "known":** Not a bug. Move to the next file.

**If the difference is "unknown":** Likely a bug — proceed with fixture creation. Or it might be a NEW intentional divergence that needs documenting in the language's `conformance_prettier*.md` catalog and a detector pattern added.

---

### Common Root Causes

Recurring shapes worth checking before reading the printer end to end. Each names the
*symptom* first, because that is what the diff hands you.

- **A comment is dropped at one gap.** A printer path builds its children's docs without
  ever asking what sits between two span positions — typically a `build_*_doc(x)` call
  reached straight from a delimiter or keyword, with no gap lookup for the region between
  them. The gap emitters, the three lookup axes and the ownership hazards are stated in
  [comments.md](./comments.md); read it before adding an emitter, since which lookup the
  site needs is the part that is easy to get backwards.
- **A blank line appears (or a comment moves) near stripped grouping parens.** When the
  parser strips `(expr)` to `expr`, the expression's span excludes the `(` and `)`, so the
  source between adjacent items holds bytes no node claims and a naive scan reads them as
  an author blank. Not JSDoc-specific — any stripped grouping paren does it. Step over the
  shell on the side you are scanning (`skip_stripped_open_paren` for the opening gap, the
  `find_comma_pos` / `find_comma_after` family for the closing one) rather than measuring
  from the node's own span.
- **It diverges only when nested.** Break propagation or the indent context differs at
  depth; the construct itself is fine. Compare the doc trees, not the outputs.
- **It works for one variant and not its sibling.** Two paths print the same construct and
  only one carries the fix. The frequent sub-shape is an **early-return optimization**
  (expand-last-arg, single-argument hugging, function composition) firing ahead of the
  general path that would have handled the comment. Bisect by varying the last argument's
  type, the argument count, or the expression context until the path flips.
- **The parser silently skips tokens.** An `is_*_start()` predicate doesn't recognize a
  valid token, so the parser falls through to its skip-unexpected-token recovery and the
  content vanishes — usually with no error, because recovery succeeded. Diagnose by
  diffing `cargo run -p tsv_cli parse <file> --pretty` against
  `cargo run -p tsv_debug canonical_parse <file>`: the missing nodes name the dropped
  region.
- **The parser uses the wrong grammar production.** A strict variant is called where a
  permissive one is needed (nested CSS rules need relative selectors, which admit a
  leading combinator; complex selectors forbid it). The permissive function often already
  exists for a sibling context and only needs reusing.
- **The disambiguation heuristic is too shallow.** A 1–2 token lookahead cannot separate
  constructs whose difference sits far ahead — `span:hover { }` and `filter:blur(5px);`
  both open `Identifier Colon`, and only the `{` versus `;` decides. Scan forward to a
  definitive delimiter, skipping balanced parenthesized groups (a temporary lexer, no AST
  allocation). Then check the *other* callers: the same decision usually has to be made in
  at-rule block parsing too.
- **A keyword means different things in two languages.** `as` is both a TS type assertion
  and the `{#each}` binding separator, so a partial parser that disables it at the top
  level works inside parens and fails outside them. Try the restricted parse, detect the
  ambiguity, then re-parse with the keyword enabled — and use `canonical_parse` to see
  which interpretation is correct.

---

## Phase 3: Fixture Review

Before creating a fixture, check whether the pattern is already covered:

```bash
find tests/fixtures -name "*keyword*" -type d   # by name
grep -r "pattern" tests/fixtures/               # by content
```

Then read 2–3 fixtures in the target category and match their shape — how many examples,
which edge cases, which `unformatted_*` variants. The conventions themselves (generic
names, the `_long` boundary rule, the divergence suffixes) live in
[fixture_naming.md](./fixture_naming.md), which is required reading before every fixture
and is not restated here.

---

## Phase 4: Fixture Creation

**CRITICAL: Create fixtures BEFORE changing code. Fixtures define target behavior.**

### 4.1 Isolate the Pattern

Extract a minimal reproduction from the corpus file:

```bash
# Compare specific content
cargo run -p tsv_debug compare --content '<div class="x" data-attr="y"></div>' --parser svelte
```

Reduce to the smallest case that shows the difference. Bisect in this order — each step
narrows *what the trigger is*, not just how much text surrounds it:

1. Start from the corpus file's full diff and identify the affected construct.
2. Extract that construct into a `--content` snippet.
3. Simplify: drop surrounding nesting, control flow, function bodies.
4. **Vary the construct's parent** — assignment vs declaration, chain vs standalone,
   statement vs expression. A trigger that survives simplification but dies on a parent
   change is a *path* bug, not a construct bug.
5. Test the boundary: which context works and which fails.

Worked example — a dropped comment in an assignment:

```bash
# 1. The corpus file shows `node = (node.parentNode)` losing its cast comment
cargo run -p tsv_debug compare /path/to/Sidebar.svelte

# 2. Extract the pattern
cargo run -p tsv_debug compare --content '<script>
a = /** @type {T} */ (expr);
</script>' --parser svelte  # FAILS — comment lost

# 3. Vary the parent: the declaration form
cargo run -p tsv_debug compare --content '<script>
let a = /** @type {T} */ (expr);
</script>' --parser svelte  # WORKS — so the trigger is the assignment path, not the cast

# 4. Widen within that path
cargo run -p tsv_debug compare --content '<script>
a += /* comment */ expr;
</script>' --parser svelte  # FAILS — every compound assignment too
```

For a SAFETY violation the corpus output's `Missing:` text names the lost content
directly, which is the bisection's starting point rather than its result.

### 4.2 Create the Fixture

[fixture_workflow.md](./fixture_workflow.md) owns the mechanics. For an ordinary fixture
`fixture_init` writes a prettier-formatted `input.svelte` **and** `expected.json` in one
step, so don't hand-roll the directory, the prettier round-trip, or a separate
`fixtures:update:parsed`:

```bash
cargo run -p tsv_debug fixture_init tests/fixtures/<lang>/<category>/<name> --content '<code>'
deno task fixtures:validate <name> --prettier-only   # structure only, skips our formatter
deno task fixtures:validate <name>                   # should FAIL — that failure is the spec
```

Prefer `.svelte` even for TypeScript- or CSS-only patterns
([why](./fixture_overview.md#why-svelte-is-the-default-canonical-source)).

⚠️ If the diff turns out to be a **new sanctioned divergence** rather than a bug, the
fixture is built by hand instead — `fixture_init` formats the input *through prettier*,
which overwrites exactly the form the divergence exists to claim. See
[fixture_workflow.md §1.3](./fixture_workflow.md#13-manual-alternative), and catalog the
divergence in the language's `conformance_prettier*.md` before adding a detector pattern.

Two readings of that second run are corpus-specific:

- **It fails as expected** — good; the failing fixture defines the target behavior.
- **It passes immediately** — either the bug was already fixed (re-check against the
  corpus comparison) or the fixture doesn't capture the difference. The usual cause of the
  latter is a reduction that dropped the trigger along with the context; go back to §4.1
  and re-bisect from the last failing snippet.

### 4.3 ★ GET USER APPROVAL ★

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

**Test every path the construct has.** If it prints through more than one path — chain vs
non-chain, a different parent context, an early-return optimization — exercise each. A fix
in one path routinely reveals the identical bug in its sibling, and the fixture only
covers the path it happens to reach.

### 5.3 Verify Fix

```bash
# Run fixture validation
deno task fixtures:validate [pattern]

# Compare the specific file
cargo run -p tsv_debug compare tests/fixtures/.../input.svelte
```

### 5.4 Add Unformatted Variants

Once the fix works, add the normalization variants — both formatters must reduce each to
`input` byte-for-byte, and the `_compact` / `_spaces` pair is direction-graded by
`variants:audit`. Rules and naming:
[fixture_workflow.md §6.2](./fixture_workflow.md#62-add-variants),
[fixture_naming.md](./fixture_naming.md#standard-variant-names).

---

## Phase 6: Verify

### Re-run Corpus Comparison

```bash
deno task corpus:compare:format ../corpora/collections/zzz --filter [language]
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

## Safety Check

The safety check is a **differential character-frequency comparison against
prettier** (`check_safety_vs_prettier`): it reports only the semantic-character
loss/addition our output incurs **beyond** prettier
(`real = max(0, ours_delta − prettier_delta)`), so shared normalizations cancel.
Safety violations fail the corpus check immediately — they are never skipped.
Full algorithm, character sets, and how to read a violation:
[divergence_detector.md §Safety Checks](./divergence_detector.md#safety-checks).

---

## Reference

### Corpus Compare Options

```bash
deno task corpus:compare:format --all [options]       # the gates corpus view (~6,200 files)
deno task corpus:compare:format <path> [options]      # Scans <path> recursively
deno task corpus:compare:format:run <path> [options]  # Skip FFI build (faster iteration)

Options:
  --all             Compare the gates corpus view (~6,200 files: the ../corpora snapshot + the
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
deno task divergence:audit        # Cross-reference patterns vs conformance_prettier*.md
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
- ./conformance_prettier.md - **Intentional Prettier divergences** — the frame; its §Catalogs table indexes the per-language catalogs (check before fixing a "bug")

---

## Anti-Patterns

### Never Do These

1. **Change code before a fixture exists** — without one there is no specification, only
   an opinion about the diff.
2. **Modify a fixture to make a test pass** — fixtures are the source of truth; a failure
   means the code is wrong.
3. **Create a fixture for buggy behavior** — `output_prettier.*` records an INTENTIONAL
   difference, never "our formatter is wrong here".
4. **Skip the approval gate** — including "the fix is obvious". Plan-mode approval counts;
   interactive re-approval on top of it is redundant, but nothing else substitutes.
5. **Create a fixture without checking existing ones** — duplicates coverage and misses
   the established pattern for the category.
6. **Fix several issues at once** — the corpus loop's traceability comes from one diff at
   a time (see [Bulk triage](#bulk-triage--the-one-carve-out-from-one-file-at-a-time) for
   the one carve-out, which groups *investigation*, not fixes).
7. **Skip an "unknown" difference** — an unexplained diff is either a bug or a missing
   catalog entry, and both are work.

### Red Flags

- "Let me just fix this one thing" — Missing fixture first
- `output_prettier.svelte` in many fixtures — Not matching Prettier (bugs)
- Fixtures with domain-specific names — Not following naming conventions
- Tests passing after fixture changes — Modified fixture to hide bug
