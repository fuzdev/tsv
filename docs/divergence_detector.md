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

When a file also has a **safety violation** (real data loss vs prettier), reclassification
to `known_divergence` requires **`safety_vouched`**, which is stricter than
`all_explained`:

1. every hunk is explained (as above), **and**
2. every **char-risky** hunk — one whose own added/removed lines move the semantic
   character count (`hunk_alters_semantic_chars`, using the same folding/exclusion rules
   as the whole-file check) — is claimed by a pattern that declares
   `may_alter_char_frequency`.

The second condition exists because `all_explained` is a *set-cover over hunk indices*: it
cannot distinguish the hunk that carried the flagged characters from one that merely sits
in the same file. On `prettier/tests/format/html/tags/tags.html` the entire char delta comes
from three `<i … />` → `<i …></i>` expansions in one hunk, yet two unrelated
boundary-whitespace hunks were equally load-bearing for the downgrade — a change to the
pattern claiming *those* would have flipped the file into the gated `safety_violation`
bucket with no formatter change at all. Scoring each hunk on its own lines restores the
causal link: a whitespace-only hunk is never char-risky, so it can neither vouch nor, by
regressing, collapse the verdict.

`may_alter_char_frequency` is opt-in and defaults to `false`, so the gate **fails closed** —
a pattern that has not answered the question cannot excuse content loss, and a new pattern
is safe by omission. Declaring it is a promise that the pattern's own `detect` carries a
content-preservation proof; the current set is `bom_strip` (byte-exact BOM prefix test),
`self_closing_nonvoid` (matching tag names on both sides), `comment_preserved` (the comment
text must appear in ours), and `css_scss_directive_number` (identical non-numeric skeleton
*plus* equal numeric-token counts, so a number may be re-spelled but never dropped). A
pattern that legitimately changes char counts without declaring it surfaces loudly as a
SAFETY failure the first time it fires — which is the intended way to discover one.

See `corpus_compare_format.ts` and the safety section below.

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

# Audit: runs every pattern against every documented fixture's committed prettier
# forms and reports which are actually DETECTED, plus the `fixtures[]` listing
# drift as separate bookkeeping. Exits 1 on a genuine detection gap.
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
- `fixtures` - An **explicit assertion** that this pattern detects these
  `*_prettier_divergence` fixtures. **Enforced**: the behavioral fixture-coverage audit
  (`fixture_coverage_test.ts`, in `deno task check`) drives each pattern against the
  fixtures it lists and fails if one stops being detected, or if a listed path names no
  directory on disk.

**Coverage is computed, not declared.** `deno task divergence:audit` answers "is this
documented divergence detected?" by running `detect_divergences` — the same classifier the
corpus comparison uses — against each fixture's committed prettier forms
(`fixture_cases.ts`, shared with the test above — no build, no sidecar, ~0.2 s for the
whole set). Going through the classifier rather than looping `detect()` is deliberate: it
inherits both the per-pattern language filter and the three-level hunk coverage, so a
fixture with an unexplained hunk left over is reported **partial**, never folded in with
the fully-explained ones. A binary detected/undetected metric would re-introduce, one
level up, precisely the masking that hunk-aware detection exists to prevent.

It does **not** read the answer out of the `fixtures[]`
arrays: those are a hand-maintained mirror of a computable fact, and the two diverge badly
— most detected fixtures are simply unlisted. That drift is what produced every mislisting
and stale path the audit has had to repair, so listing gaps are now reported as
bookkeeping, below the detection headline, and only a genuine gap (a fixture pinning a
prettier form that no pattern explains) exits 1.

A fixture that pins **no** prettier form — no `output_prettier.*`, no
`prettier_variant_*`/`prettier_intermediate_*`/`divergent_variant_*`, and not an
unambiguous N10 case — is reported **ungradeable**: detection can't be asked of it, so it
counts as neither a success nor a gap. The report gives both rates (over all documented,
and over the gradeable subset) rather than folding it into either.

**Coverage is partial by design** — some documented
divergences have no detector, and a few deliberately never will: the `typescript/chain-expression`
optional-chain paren torture files mix a tsv-keeps/prettier-strips correctness divergence *and* its
reverse in one file, so any detector reaching them would have to claim the ours-side strip direction
— masking the paren-strip correctness family — and is left undetected. The "we preserve / Prettier
drops a comment" family (Svelte `expr_trailing`, `debug_comment`) is claimed by the dedicated
`comment_preserved` pattern, which keys on the *added*-comment direction — the direction
`comment_position`'s content guard cannot claim, since a dropped comment is absent from prettier's
output. Genuinely-uncovered divergences surface in the audit's uncovered list rather than being
falsely claimed by a pattern that cannot actually detect them.

## Pending work

The audit's report **is** the work-list — these numbers move as detectors are widened,
so read them live (`deno task divergence:audit`) rather than trusting the counts here.
Four buckets, in rough priority order:

1. **Undetected (~64)** — a documented divergence pinning a prettier form that no
   pattern explains at all. The headline gap. Some are deliberate and will stay
   (see the two families named above); the rest want a detector or a reassignment.
2. **Partial (~25)** — a pattern explains the divergence but leaves an adjacent hunk
   unclaimed. Not a mystery, and quieter than it sounds: typically the diff splits one
   logical change across hunk boundaries (a dangling `) {` line), or the detector claims
   *some* instances of a repeated divergence but not all — `css/selectors/combinators/
   column_prettier_divergence` pins four identical `||` combinators and the pattern
   claims three. **These are worth closing, and demonstrably**: the corpus classifies by
   the same rule, so a partial fixture is a real file landing in the pinned `partial`
   bucket instead of `known`. Widening `comment_position` to split prettier's *merged*
   trailing-line-comment form (`a // c1 // c2`) closed 6 fixture partials and moved one
   real corpus file (`js/for-of/comments.js`) partial→known, ratcheting
   `CORPUS_FORMAT_PARTIAL_PIN` down.

   The dominant remaining shape was **indentation**, and it turned out not to be
   leftover reflow at all: for those fixtures the indent shift *is* the whole
   divergence — the sanctioned [§Uniform Forced-Continuation
   Indent](conformance_prettier.md#uniform-forced-continuation-indent) rule, where a
   line comment forces a construct's tail onto a continuation line that tsv indents one
   level and prettier keeps flush. `forced_continuation_indent` covers it across the
   sites that section enumerates (annotation `:`, module headers, prefix type
   operators, the before-`:` key gap). It stays safe to apply broadly because it is
   keyed on the construct head *above* the hunk carrying the comment that forced the
   break: an ordinary indentation defect has no such comment and is never claimed, so
   the detector cannot mask the tsv defect class it most resembles.
   Of the remainder, the **11 that some pattern also LISTS** are ratcheted by
   `KNOWN_PARTIAL` in `fixture_coverage_test.ts` — a listed fixture going partial fails
   the gate, and an entry that stops firing fails too, so the list mirrors the live set
   and can only shrink.
3. **Ungradeable (~15)** — the fixture pins no prettier form at all, so detection is
   unanswerable. Not a detector gap: either the fixture gains a witness file, or it stays
   honestly unmeasurable.
4. **Explained but unlisted (~291)** — pure bookkeeping. The detector sees them; no
   `fixtures[]` array says so. Listing one buys an explicit per-pattern assertion (the
   gated test) at the cost of a hand-maintained entry that can drift, so this is
   deliberately **not** a backlog to burn down — list a fixture when you want that
   specific assertion pinned, not to make a number go up.

Note that a pattern detecting no *documented* fixture is not thereby dead —
`empty_statement_removal` detects none yet fires on 3 corpus files, so check
`--audit-patterns` (the corpus-side view) before concluding. `block_multiline_attrs_hug`
was the one pattern dead by every measure (0 corpus files, 0 committed fixture pairs, 0
documented) and has been deleted.

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
`conformance_sections` / `fixtures`; `deno task divergence:audit` prints, per pattern,
both the `listed` count (its `fixtures[]` entries) and the measured `detects` count, plus
overall detection. A pattern with `detects 0` explains no *documented* divergence — which
does not by itself make it dead, since it may fire only on corpus code
(`--audit-patterns` is the corpus-side view). Each pattern's `detect()` returns
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

**Reading a violation:** each char shows its `real` count with shared context —
the first labeled `(ours N, prettier M)`, the rest bare `(N, M)`:

```
content_lost: 7 beyond prettier — '|'×2 (ours 28, prettier 26), '/'×2 (2, 0), '*'×2 (2, 0), '4'×1 (1, 0)
```

The `'|'×2 (ours 28, prettier 26)` reads as: of the 28 pipes our output dropped,
26 are shared with prettier (a normalization, not loss) and only 2 are real. The
`(2, 0)` entries are fully real — prettier dropped none.

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
