/**
 * Svelte-fixtures parse-conformance gate — the drop-in-parser analog of test262
 * (JS) and the WPT harness (CSS), run against Svelte's own compiler test suite
 * (`../svelte/packages/svelte/tests`).
 *
 * Oracle = the LIVE modern Svelte parser (`svelte/compiler` `parse(src,
 * {modern:true})`), NOT the committed fixture artifacts. Why: `output.json` in
 * `parser-legacy` is the *legacy* AST (tsv targets the modern parser);
 * `compiler-errors/_config.js` encodes *compiler* verdicts (often analysis-stage,
 * post-parse), not parse-stage ones; `css` ships compiled CSS. The modern parser
 * is the only correct oracle for a drop-in *modern-parser* replacement — and it
 * makes the two "trap" partitions resolve for free: `loose-*` inputs throw under
 * the non-loose oracle (→ parity), and analysis-stage `compiler-errors` parse
 * fine on both sides (→ never miscounted as a tsv bug).
 *
 * Scope: the canonical `.svelte` INPUTS across `tests/` (`input.svelte` /
 * `main.svelte` / `index.svelte`), skipping generated `_`-prefixed artifacts,
 * `output.svelte` dups, and the `migrate/` tree (Svelte-4 migrator inputs, not
 * modern-parse targets). `.svelte.js`/`.ts`/`.css` are out of scope here — the
 * TS/CSS parsers have their own corpora (test262, wpt, ts-fixtures).
 *
 * The shared engine (`lib/fixtures_gate.ts`) does the rest — two comparisons per
 * input: VERDICT parity (the enforced gate; over-rejections must be `SANCTIONED`
 * or a tracked `KNOWN_GAP`, else exit 1) and AST-shape (report-only; the
 * adversarial tree exposes edge divergences to triage into the shared
 * `DOCUMENTED_MATCHERS` or fix as writer bugs). Periodic (non-`check`) gate — needs
 * the FFI + the `svelte/compiler` oracle. Strict about setup: a missing `../svelte`
 * checkout (0 scanned) FAILS — publish Step 3b's preflight probe is the tolerance
 * point. Full-suite runs freshness-check the ledgers, enforce the exact pinned
 * counts (`lib/gate_counts.ts`), and warn on checkout↔npm-pin version skew.
 *
 * Run (from the repo root):
 *   deno task conformance:svelte-fixtures             # builds corpus FFI, then runs
 *   deno task conformance:svelte-fixtures:run         # skip rebuild (freshness-guarded)
 *   deno task conformance:svelte-fixtures:run --json 2>/dev/null > report.json
 *   deno task conformance:svelte-fixtures:run ../svelte/packages/svelte/tests/parser-modern
 */

import { type FixturesGateConfig, run_fixtures_gate } from '../lib/fixtures_gate.ts';
import { SVELTE_FIXTURES_PINS } from '../lib/gate_counts.ts';
import { type KnownGap, SVELTE_FIXTURE_SANCTIONS } from '../lib/parse_sanctions.ts';

/**
 * Over-rejections where tsv is WRONG — genuine drop-in parse gaps, tracked so the
 * gate is green at baseline and only regressions (a NEW, untracked over-rejection)
 * fail it. This set must only SHRINK: when a gap is fixed, delete its entry (the
 * input then parses → parity). Full triage lives in internal notes
 * (TODO_PARSE_COVERAGE.md §"Svelte parse over-rejections vs `svelte/tests`").
 */
const KNOWN_GAPS: KnownGap[] = [
	// Empty: every tracked Svelte-parser over-rejection is fixed. A drop-in gap here is one
	// where svelte's PARSER accepts an input (its VALIDATOR — a stage tsv doesn't run —
	// rejects) but tsv over-rejects at parse; each gets a fixtures-first fix and its entry is
	// deleted once the input parses. The last entry (top-level leading combinator, e.g.
	// `> span {}`) closed when tsv's CSS selector parser began accepting a leading combinator
	// in every context, matching parseCss — see `crates/tsv_css/src/parser/selectors.rs`
	// `parse_complex_selector` and docs/conformance_svelte.md §CSS Parser Scope & Error Model.
	// The freshness check fails a stale entry, so this list stays honest.
];

/** The gate's config — exported for the `conformance.ts` single-process driver. */
export const SVELTE_FIXTURES_GATE: FixturesGateConfig = {
	title: 'Svelte-fixtures',
	language: 'svelte',
	default_root: '../svelte/packages/svelte/tests',
	// Excludes `output.svelte`, `*.svelte.js`.
	input_basenames: new Set(['input.svelte', 'main.svelte', 'index.svelte']),
	input_noun: '.svelte inputs',
	prune_dir: (name) =>
		name.startsWith('_') || // generated artifacts (_output/_expected/_actual)
		name === 'node_modules' ||
		name === '.svelte-kit' ||
		// Svelte-4 → 5 migrator INPUTS: intentionally legacy/weird, not modern-parse
		// targets. Out of scope for a modern-parser conformance gate.
		name === 'migrate',
	sanctioned: SVELTE_FIXTURE_SANCTIONS,
	sanctioned_note: 'tsv correctly stricter',
	known_gaps: KNOWN_GAPS,
	oracle_name: 'the modern Svelte parser',
	oracle_pin: {
		checkout_package_json: '../svelte/packages/svelte/package.json',
		npm_package: 'svelte',
	},
	pins: SVELTE_FIXTURES_PINS,
};

if (import.meta.main) {
	await run_fixtures_gate(SVELTE_FIXTURES_GATE);
}
