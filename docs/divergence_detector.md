# Divergence Detector

> Programmatic detection of known formatting divergences

The divergence detector is integrated into `corpus_compare_format.ts` and automatically identifies known differences documented in `conformance_prettier.md`.

## Architecture

```
benches/js/
├── corpus_compare_format.ts          # Uses divergence detection
├── divergence_audit.ts        # Cross-references patterns vs docs
├── lib/
│   ├── corpus.ts              # Corpus loading
│   ├── diff.ts                # Diff utilities + DiffHunk extraction
│   ├── canonical.ts           # Prettier wrapper
│   ├── ffi.ts                 # Native FFI
│   │
│   └── divergence/            # Divergence detection module
│       ├── mod.ts             # Main exports
│       ├── safety.ts          # Safety check (differential char-frequency vs prettier)
│       ├── patterns.ts        # Hunk-aware pattern detectors (PATTERNS), with traceability
│       ├── expected_errors.ts # Expected-error fixtures (parse-rejection cases)
│       └── validation.ts      # Audit: cross-ref patterns vs conformance_prettier.md
```

## Hunk-Aware Detection

Pattern detection operates at the **diff hunk** level, not the whole file. Each diff between Prettier and our formatter is split into contiguous groups of changes (hunks). Patterns must explain specific hunks, not just match global file properties.

This prevents masking: if a file has two hunks and only one is explained by a known pattern, the file is classified as `partial` instead of being hidden as `known`.

### Classification

- **`all_explained`** → `known_divergence`: Every hunk explained by at least one pattern
- **`partial`** → `partial_divergence`: Some hunks explained, some not (needs investigation)
- **`none_explained`** → `unknown_diff`: No hunks explained by any pattern

When a file also has a **safety violation** (real data loss vs prettier), it is only
reclassified to `known_divergence` if every hunk is explained — and the patterns that
explain it carry their own content guards (e.g. `comment_position` requires the comment
to exist as a whole line in both outputs), so a dropped-content hunk cannot be silently
absorbed. See `corpus_compare_format.ts` and the safety section below.

### DiffHunk Type

```typescript
interface DiffHunk {
	index: number; // 0-based hunk index
	lines: DiffLine[]; // All diff lines in this hunk (incl. context)
	ours_range: { start: number; end: number } | null; // Line range in our output (null if only removals)
	prettier_range: { start: number; end: number } | null; // Line range in prettier output (null if only additions)
	added_lines: string[]; // Lines we added (ours-only)
	removed_lines: string[]; // Lines we removed (prettier-only)
}
```

## Commands

```bash
# Compare corpus against Prettier (uses divergence detection)
deno task corpus:compare:format ~/dev/some-project
deno task corpus:compare:format ~/dev/some-project --explain         # Show which patterns matched
deno task corpus:compare:format --all --audit-patterns               # Per-pattern coverage with samples

# Audit: static metadata check — cross-references each pattern's `fixtures` list
# against the conformance doc. Does NOT run patterns (reports documented-vs-claimed
# coverage only); the behavioral test below is what actually runs them.
deno task divergence:audit        # Human-readable report
deno task divergence:audit --json # Machine-readable JSON

# Pattern tests: synthetic positive/negative (overmatch-rejection) unit tests PLUS
# a behavioral fixture-coverage audit that drives each detector against its own
# committed fixtures (input == ours, output_prettier == prettier) and fails if a
# pattern stops claiming a hunk in a fixture it lists. Runs read-only.
deno task test:deno
```

## Traceability

Every pattern in `patterns.ts` includes:

- `conformance_sections` - Which sections of `conformance_prettier.md` it covers
- `fixtures` - Which `*_prettier_divergence` fixtures it detects. This is **enforced**:
  the behavioral fixture-coverage audit (`fixture_coverage_test.ts`) drives each pattern
  against the fixtures it lists and fails if one stops being detected — closing the gap
  where the static audit (below) could report a fixture as "covered" while the detector
  had silently drifted away from it.

`deno task divergence:audit` cross-references the `fixtures` lists against
`conformance_prettier.md` and reports how many documented divergences are claimed by some
pattern versus the uncovered remainder. **Coverage is partial by design** — some documented
divergences have no detector, and a few deliberately never will: the `typescript/chain-expression`
optional-chain paren torture files mix a tsv-keeps/prettier-strips correctness divergence *and* its
reverse in one file, so any detector reaching them would have to claim the ours-side strip direction
— masking the paren-strip correctness family — and is left undetected. The "we preserve / Prettier
drops a comment" family (Svelte `expr_trailing`, `debug_comment`) is claimed by the dedicated
`comment_preserved` pattern, which keys on the *added*-comment direction — the direction
`comment_position`'s content guard cannot claim, since a dropped comment is absent from prettier's
output. Genuinely-uncovered divergences surface in the audit's uncovered list rather than being
falsely claimed by a pattern that cannot actually detect them.

## Pattern Registry

The patterns live in the `PATTERNS` array in `patterns.ts`, ordered from **most specific
to most broad** so each hunk gets the most precise explanation possible — a narrow
language/feature pattern claims a hunk before the broad `fill_101_boundary` /
`comment_position` fallbacks ever see it. Rough tiers, specific → broad:

1. **Language-specific narrow patterns** — BOM stripping, self-closing normalization,
   empty-statement removal, …
2. **CSS patterns** — at-rule spacing/wrapping, selectors, comments, SCSS directives, …
3. **Feature patterns** — template-literal width, single-specifier import,
   member-expression call, return-type generic union, …
4. **Svelte element/block patterns** — `inline_content_hug`, `short_expr_100`,
   `block_expression_logical`, `comment_preserved`, …
5. **Broad fallbacks, run last** — `css_value_wrap`, `fill_101_boundary`,
   `comment_position`

`patterns.ts` is the source of truth for the full list and each pattern's
`conformance_sections` / `fixtures`; `deno task divergence:audit` prints the live
per-pattern fixture counts and overall coverage. Each pattern's `detect()` returns
`hunk_indices` identifying which specific hunks it explains.

## Safety Checks

`check_safety_vs_prettier()` runs before divergence detection to catch **bugs**
(data loss).

Uses **character frequency comparison**: counts semantic characters (letters,
digits, brackets, operators) in source vs formatted output. If source has more
of any character, content was lost.

Excluded from counting (formatters legitimately change these):

- Whitespace: space, tab, newline
- Quotes: `' " \`` (style normalization)
- Parens: `( )` (optional in arrow params)
- Separators: `, ;` (trailing commas, ASI)

Safety violations fail the corpus check immediately - they are never skipped.

### Differential against prettier (false-positive guard)

The check is **differential**, not absolute. Comparing OUR output's character
deltas directly against source over-reports: every place we legitimately
normalize the way prettier does (removing a redundant leading `|` in
`type A = | C` → `type A = C`, number normalization, CSS keyword lowercasing)
drops the source character count and would be flagged as data loss even though
we match prettier.

Prettier is the source of truth, so any transformation prettier _also_ performs
is not data loss. `check_safety_vs_prettier(source, ours, prettier)` computes
per-character deltas for **both** outputs against source and reports only the
remainder our output incurs beyond prettier:

```
real_lost[c]  = max(0, ours_lost[c]  − prettier_lost[c])
real_added[c] = max(0, ours_added[c] − prettier_added[c])
```

A violation survives only when our output drops/adds a character that prettier
preserves. This subsumes the older `ours !== prettier` guard (when
`ours === prettier`, every delta matches prettier's and the real set is empty)
and — unlike that all-or-nothing guard — correctly isolates a real loss in a
file that _also_ contains an unrelated shared normalization.

The algorithm lives only in `lib/divergence/safety.ts`; it runs in-process on
strings already in the Deno heap (source, FFI output, prettier output), so it
never shells out — per-file cost is dominated by the prettier format call.

## Overmatching Audit

A pattern that matches a hunk it shouldn't will mark a real bug (or real data loss) as
`known`. Two layers guard against this:

- **Behavioral + unit tests** (`deno task test:deno`): synthetic positive cases,
  overmatch-**rejection** negative cases (a bug-shaped diff must return `null`), and the
  fixture-coverage audit. New patterns must add a negative for the false-positive scenario.
- **Corpus spot-check**: per-pattern report of what each pattern claims, with sample diffs.

```bash
deno task corpus:compare:format --all --audit-patterns   # what each pattern claims, with samples
deno task test:deno                          # unit + behavioral tests
```

**Audit workflow:**

1. Run `deno task corpus:compare:format --all --audit-patterns` to see what each pattern claims
2. For each pattern, review the sample diffs shown
3. If a claimed diff doesn't match the documented divergence:
   - The pattern's detection logic is too broad — tighten it (prefer reusing the shared
     `long_line_rewrapped` helper, which bundles the ours-side re-wrap guard)
   - Add a negative test case in `patterns_test.ts` for the false-positive scenario
4. Run `deno task test:deno` to verify no regressions

## Adding New Patterns

1. Document the divergence in `docs/conformance_prettier.md`
2. Create fixture in `tests/fixtures/.../..._prettier_divergence/`
3. Add pattern to `benches/js/lib/divergence/patterns.ts`:

```typescript
const new_pattern: DivergencePattern = {
	id: 'pattern_id',
	description: 'What this pattern detects',
	languages: ['svelte', 'typescript'], // or ['css']
	conformance_sections: ['Section Name'], // From conformance_prettier.md
	fixtures: ['path/to/fixture_prettier_divergence'], // Relative to tests/fixtures/
	detect(ctx) {
		// Use find_matching_hunks to identify which hunks this pattern explains.
		// For long-line "prettier kept it wide, we re-wrapped" divergences, prefer
		// the shared long_line_rewrapped helper — it bundles the ours-side guard.
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			// Check hunk.added_lines, hunk.removed_lines, or range-based lookups.
			return; /* condition */
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'pattern_id',
				confidence: 'likely', // 'certain' | 'likely' | 'possible'
				hunk_indices,
				reason: 'Human-readable explanation',
			};
		}
		return null;
	},
};
```

4. Add positive + negative (overmatch-rejection) tests to
   `benches/js/lib/divergence/patterns_test.ts`. Only list a fixture in `fixtures` if the
   pattern genuinely detects it — the behavioral fixture-coverage audit fails otherwise, so
   don't list a divergence the pattern structurally can't claim (reassign it to the pattern
   that does, or leave it honestly uncovered).
5. Add to `PATTERNS` array (respect tier ordering: specific before broad)
6. Run `deno task test:deno` to verify tests pass
7. Run `deno task divergence:audit` to verify coverage
