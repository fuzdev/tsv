/**
 * TypeScript-fixtures parse-conformance gate — the drop-in-parser analog of
 * test262 (JS) and the Svelte-fixtures harness, run against acorn-typescript's
 * OWN test suite (`../acorn-typescript/test`, ~200 adversarial `input.ts`
 * fixtures: arrow-type params, class/decorator edge cases, import attributes,
 * escaped keywords, …). tsv is a drop-in replacement for acorn + acorn-typescript,
 * so that parser's own regression corpus is the natural TS edge-case oracle — the
 * shape real-world code (`corpus:compare:parse`) can't reach, because ordinary
 * code doesn't exercise these edge cases.
 *
 * Oracle = the LIVE `@sveltejs/acorn-typescript` parser (pinned in
 * benches/js/package.json / sidecar.ts), NOT the committed `expected.json`
 * artifacts — same reason the Svelte gate uses the live modern parser: a committed
 * artifact can drift from the pinned version that defines fixture correctness, and
 * the live parser is exactly what `corpus:compare:parse` already diffs against, so
 * the two stay consistent by construction.
 *
 * Scope: every `input.ts` under the suite root (the `*.test.ts` / `utils.ts`
 * harness files are excluded by basename). `.tsx`/JSX fixtures parse as ordinary
 * `.ts` here — tsv and acorn (module mode, no JSX plugin) both reject them, so
 * they land in `parity`.
 *
 * The shared engine (`lib/fixtures_gate.ts`) does the rest: verdict parity GATES
 * (over-rejections must be a `TS_FIXTURE_SANCTIONS` sanction or a tracked
 * `KNOWN_GAPS` entry, else exit 1), AST-shape is a report-only triage surface.
 * Periodic (non-`check`) gate — needs the FFI + the acorn-typescript oracle
 * (node_modules) + the `../acorn-typescript/test` checkout. Strict about setup:
 * a missing root (0 scanned) FAILS — the tolerance point for machines without
 * the checkout is publish Step 3b's preflight probe, which skips the whole
 * aggregate (warn on dry-run, blocking on --wetrun). Full-suite runs also
 * freshness-check the ledgers (a sanction/known-gap entry matching nothing
 * fails), enforce the exact pinned counts (`lib/gate_counts.ts`), and warn on
 * version skew between the checkout and the pinned npm oracle.
 *
 * Run (from the repo root):
 *   deno task conformance:ts-fixtures                # builds corpus FFI, then runs
 *   deno task conformance:ts-fixtures:run            # skip rebuild (freshness-guarded)
 *   deno task conformance:ts-fixtures:run --json 2>/dev/null > report.json
 *   deno task conformance:ts-fixtures:run ../acorn-typescript/test/class_accessor
 */

import { type FixturesGateConfig, run_fixtures_gate } from '../lib/fixtures_gate.ts';
import { TS_FIXTURES_PINS } from '../lib/gate_counts.ts';
import { type KnownGap, TS_FIXTURE_SANCTIONS } from '../lib/parse_sanctions.ts';

/**
 * Over-rejections where tsv is WRONG — genuine drop-in parse gaps, tracked so the
 * gate is green at baseline and only regressions (a NEW, untracked over-rejection)
 * fail it. This set must only SHRINK: when a gap is fixed, delete its entry (the
 * input then parses → parity). Full triage lives in the grimoire lore
 * (TODO_PARSE_COVERAGE.md §"Productionized: conformance:ts-fixtures").
 */
const KNOWN_GAPS: KnownGap[] = [];

/** The gate's config — exported for the `conformance.ts` single-process driver. */
export const TS_FIXTURES_GATE: FixturesGateConfig = {
	title: 'TypeScript-fixtures',
	language: 'typescript',
	default_root: '../acorn-typescript/test',
	// Excludes `*.test.ts`, `utils.ts`, `run_test262.js`.
	input_basenames: new Set(['input.ts']),
	input_noun: '.ts inputs',
	prune_dir: (name) => name.startsWith('_') || name === 'node_modules',
	sanctioned: TS_FIXTURE_SANCTIONS,
	sanctioned_note: 'deliberate; see TS_FIXTURE_SANCTIONS',
	known_gaps: KNOWN_GAPS,
	oracle_name: 'acorn-typescript',
	oracle_pin: {
		checkout_package_json: '../acorn-typescript/package.json',
		npm_package: '@sveltejs/acorn-typescript',
	},
	pins: TS_FIXTURES_PINS,
};

if (import.meta.main) {
	await run_fixtures_gate(TS_FIXTURES_GATE);
}
