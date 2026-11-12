/**
 * The pre-release conformance aggregate in ONE process (`deno task conformance`, whose last
 * step runs this driver's root, `benches/js/roots/conformance.ts`):
 * the three parse-conformance gates (svelte-fixtures, ts-fixtures, ts-repo), then
 * `corpus:compare:parse` twice — `--all`, and over `tests/fixtures` (`--fixtures`:
 * each fixture's parse-pinned documents, the inputs that reach the `loc` tolerance rows
 * real code rarely does, since the fixtures pin the span-only wire and nothing else
 * grades their `loc` against the oracle) — and `corpus:compare:format --all`, and
 * finally `render:audit` over the pinned checkouts (the one leg that runs as a
 * subprocess — see `run_render_audit`).
 *
 * One process means the canonical oracle modules (prettier, prettier-plugin-svelte,
 * svelte/compiler, acorn, @sveltejs/acorn-typescript — ~seconds of import each)
 * load ONCE via the module cache instead of once per leg, and one summary line
 * covers the whole run. Failure semantics match an `&&` chain exactly: a leg's non-zero
 * exit code is the run's, and stops the aggregate — fail-fast, no partial-green ambiguity.
 * Each leg still runs its own init/preflights (artifact freshness, node_modules) — those
 * are cheap; the module cache is the win.
 *
 * The task chains `pins:audit:checkouts` ahead of this driver: every leg below
 * grades inputs from the sibling checkouts against the PINNED npm oracle, so a
 * checkout whose version has drifted off its pin grades different inputs than the
 * oracle defines (and rots the path-keyed ledgers). That preflight is where a skew
 * blocks — `deno task check` deliberately no longer carries it, since nothing there
 * reads those checkouts.
 *
 * Takes NO arguments — per-leg options (subtree roots, --json, -v) live on the
 * standalone tasks (`conformance:svelte-fixtures`, `corpus:compare:*`, …), which
 * remain the scoped/triage entry points. The legs load tsv's N-API addon at the `corpus`
 * profile (panics caught, not aborted), and the task sets `PRETTIER_DEBUG=1` (the format
 * leg's prettier-plugin-svelte fallback surfaces as per-file errors — the same posture
 * as `corpus:compare:format:run`; the parse legs never format, so it's inert
 * for them).
 *
 * Runs under Node: `run_conformance` is a library function over the legs' deps and
 * `cargo` (`ConformanceDeps`), bound by its root.
 *
 * @module
 */

import type { SpawnResult } from '@fuzdev/gro/spawn.ts';

import { corpus_present_dirs_for_tiers, corpus_snapshot_dir } from './lib/corpus.ts';
import { type FixturesGateDeps, run_fixtures_gate } from './lib/fixtures_gate.ts';
import {
	type CorpusCompareFormatDeps,
	run_corpus_compare_format
} from './corpus_compare_format.ts';
import { type CorpusCompareParseDeps, run_corpus_compare_parse } from './corpus_compare_parse.ts';
import { SVELTE_FIXTURES_GATE } from './diagnostics/svelte_fixtures_compare.ts';
import { TS_FIXTURES_GATE } from './diagnostics/ts_fixtures_compare.ts';
import { run_ts_repo_compare, type TsRepoCompareDeps } from './diagnostics/ts_repo_compare.ts';

/** What `run_conformance` reaches: each leg's deps, and `cargo` for the render audit. */
export interface ConformanceDeps {
	fixtures: FixturesGateDeps;
	ts_repo: TsRepoCompareDeps;
	parse: CorpusCompareParseDeps;
	format: CorpusCompareFormatDeps;
	/** `cargo`, run in the checkout with stdio inherited. */
	cargo: { run(args: readonly string[]): Promise<SpawnResult> };
}

/** One leg: its name, for the banner, and its run, resolving to its exit code. */
export type ConformanceLeg = [name: string, run: () => Promise<number>];

/**
 * `render:audit` over the version-pinned checkouts — does `tsv format` change
 * what a Svelte component RENDERS?
 *
 * The odd one out among the legs: a Rust command, so it runs as a subprocess
 * rather than in this process's module cache (it drives its own `svelte compile`
 * sidecar). It belongs here rather than in the pure-Rust `audit:corpus` for
 * exactly that reason, and it is the only gate that sees this bug class —
 * `corpus:compare:format`'s SAFETY is char-frequency (blind: the characters only
 * MOVE), `roundtrip_audit`'s skeleton erases the whitespace that carries the
 * meaning, and `authoring_audit` asks about CONVERGENCE, never whether the fixed
 * point renders like the input. The fixture **R** rules gate the same property
 * over a curated corpus, which makes them a regression guard; real code is the
 * exposure.
 *
 * Scoped to the WHOLE `../corpora` snapshot plus the `suite` tier (`../svelte`'s
 * tests) — not to a view: the bench and gate views read the collections a tier places,
 * because every collection there re-pins every corpus count, and this leg pins
 * nothing, so it reads every collection the snapshot vendors as one root, placed or
 * not. Both inputs are version-pinned
 * (`pins:audit:checkouts`), so a release verdict can't turn on a machine's unpinned
 * `live` working trees, which only the `robustness` view holds. Absent checkouts
 * warn-skip, and a scope with no dirs at all fails — a leg that graded nothing must
 * not read as a pass.
 *
 * @returns cargo's exit code, `1` for a cargo a signal ended
 */
export async function run_render_audit(cargo: ConformanceDeps['cargo']): Promise<number> {
	const snapshot = corpus_snapshot_dir(console.error);
	const suites = await corpus_present_dirs_for_tiers(['suite'], console.error);
	const dirs = [...(snapshot === null ? [] : [snapshot]), ...suites];
	if (dirs.length === 0) {
		throw new Error('no pinned corpus checkouts present — nothing to audit');
	}
	// `--features _internal_audits` is not needed by `render_audit` itself — it is how this
	// leg shares the ONE `corpus` + `_internal_audits` build world every other audit uses,
	// instead of forcing a second full compile of the same crates under a
	// different feature set. The instrumentation it pulls in is inert unless armed.
	const result = await cargo.run([
		'run',
		'--profile',
		'corpus',
		'-q',
		'-p',
		'tsv_debug',
		'--features',
		'_internal_audits',
		'--',
		'render_audit',
		'--gate',
		...dirs
	]);
	return result.kind === 'exited' ? result.code : 1;
}

/** The aggregate's legs, in order. */
export function conformance_legs(deps: ConformanceDeps): ConformanceLeg[] {
	return [
		[
			'conformance:svelte-fixtures',
			() => run_fixtures_gate(deps.fixtures, SVELTE_FIXTURES_GATE, [])
		],
		['conformance:ts-fixtures', () => run_fixtures_gate(deps.fixtures, TS_FIXTURES_GATE, [])],
		['conformance:ts-repo', () => run_ts_repo_compare(deps.ts_repo, [])],
		['corpus:compare:parse --all', () => run_corpus_compare_parse(deps.parse, ['--all'])],
		[
			'corpus:compare:parse tests/fixtures --fixtures',
			() => run_corpus_compare_parse(deps.parse, ['tests/fixtures', '--fixtures'])
		],
		['corpus:compare:format --all', () => run_corpus_compare_format(deps.format, ['--all'])],
		['render:audit (pinned checkouts)', () => run_render_audit(deps.cargo)]
	];
}

/**
 * Run `legs` in order, each under its banner: the first non-zero exit code is the run's,
 * and a leg that throws (an infrastructure failure: loader, sidecar, addon) fails it with 1.
 * @returns the exit code
 */
export async function run_conformance_legs(legs: readonly ConformanceLeg[]): Promise<number> {
	const run_started = performance.now();
	for (const [name, leg] of legs) {
		console.error(`\n════ ${name} ════`);
		const leg_started = performance.now();
		let code: number;
		try {
			code = await leg();
		} catch (e) {
			console.error(`\nFAIL: ${name} errored: ${e instanceof Error ? e.message : e}`);
			return 1;
		}
		if (code !== 0) return code;
		console.error(`──── ${name} OK (${((performance.now() - leg_started) / 1000).toFixed(1)}s)`);
	}
	console.error(
		`\n✓ conformance aggregate: all ${legs.length} legs green in ${(
			(performance.now() - run_started) / 60_000
		).toFixed(1)}m`
	);
	return 0;
}

/**
 * The driver: no arguments, then every leg (`conformance_legs`).
 * @returns the exit code
 */
export function run_conformance(deps: ConformanceDeps, argv: readonly string[]): Promise<number> {
	if (argv.length > 0) {
		console.error(
			'The conformance driver takes no arguments — use the per-leg tasks for scoped runs\n' +
				'(conformance:svelte-fixtures / conformance:ts-fixtures / conformance:ts-repo /\n' +
				' corpus:compare:parse / corpus:compare:format).'
		);
		return Promise.resolve(1);
	}
	return run_conformance_legs(conformance_legs(deps));
}
