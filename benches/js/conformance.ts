/**
 * The pre-release conformance aggregate in ONE process (`deno task conformance`):
 * the three parse-conformance gates (svelte-fixtures, ts-fixtures, ts-repo), then
 * `corpus:compare:parse --all` and `corpus:compare:format --all`, and finally
 * `render:audit` over the pinned checkouts (the one leg that runs as a subprocess
 * — see `run_render_audit`).
 *
 * One process means the canonical oracle modules (prettier, prettier-plugin-svelte,
 * svelte/compiler, acorn, @sveltejs/acorn-typescript — ~seconds of import each)
 * load ONCE via the module cache instead of once per leg, and one summary line
 * covers the whole run. Failure semantics match the old `&&` chain exactly: every
 * leg exits the process (`Deno.exit(1)`) on a finding, so a failing leg stops the
 * aggregate — fail-fast, no partial-green ambiguity. Each leg still runs its own
 * init/preflights (artifact freshness, node_modules) — those are cheap; the module
 * cache is the win.
 *
 * Takes NO arguments — per-leg options (subtree roots, --json, -v) live on the
 * standalone tasks (`conformance:svelte-fixtures`, `corpus:compare:*`, …), which
 * remain the scoped/triage entry points. The task sets `TSV_FFI_PROFILE=corpus`
 * (panics caught, not aborted) and `PRETTIER_DEBUG=1` (the format leg's
 * prettier-plugin-svelte fallback surfaces as per-file errors — the same posture
 * as `corpus:compare:format:run`; the parse legs never format, so it's inert
 * for them).
 */

import { spawnSync } from 'node:child_process';

import { corpus_present_dirs_for_tiers } from './lib/corpus.ts';
import { run_fixtures_gate } from './lib/fixtures_gate.ts';
import { run_corpus_compare_format } from './corpus_compare_format.ts';
import { run_corpus_compare_parse } from './corpus_compare_parse.ts';
import { SVELTE_FIXTURES_GATE } from './diagnostics/svelte_fixtures_compare.ts';
import { TS_FIXTURES_GATE } from './diagnostics/ts_fixtures_compare.ts';
import { run_ts_repo_compare } from './diagnostics/ts_repo_compare.ts';

if (Deno.args.length > 0) {
	console.error(
		'The conformance driver takes no arguments — use the per-leg tasks for scoped runs\n' +
			'(conformance:svelte-fixtures / conformance:ts-fixtures / conformance:ts-repo /\n' +
			' corpus:compare:parse / corpus:compare:format).',
	);
	Deno.exit(1);
}

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
 * Scoped by TIER — `framework` + `suite` (`../svelte` src and tests, svelte.dev,
 * kit), a pair no single view carries — rather than by view: those checkouts are
 * version-pinned by `pins:audit`, so a release verdict can't turn on a machine's
 * live `real` dev-repo working tree (which every `.svelte`-bearing view includes
 * — hence the tier selection). Absent
 * checkouts warn-skip, and a scope with no dirs at all fails — a leg that graded
 * nothing must not read as a pass.
 */
async function run_render_audit(): Promise<void> {
	const dirs = await corpus_present_dirs_for_tiers(['framework', 'suite'], console.error);
	if (dirs.length === 0) {
		throw new Error('no pinned corpus checkouts present — nothing to audit');
	}
	const { status } = spawnSync(
		'cargo',
		['run', '--profile', 'corpus', '-q', '-p', 'tsv_debug', '--', 'render_audit', '--gate', ...dirs],
		{ stdio: 'inherit' },
	);
	if (status !== 0) Deno.exit(status ?? 1);
}

const legs: [string, () => Promise<void>][] = [
	['conformance:svelte-fixtures', () => run_fixtures_gate(SVELTE_FIXTURES_GATE)],
	['conformance:ts-fixtures', () => run_fixtures_gate(TS_FIXTURES_GATE)],
	['conformance:ts-repo', () => run_ts_repo_compare([])],
	['corpus:compare:parse --all', () => run_corpus_compare_parse(['--all'])],
	['corpus:compare:format --all', () => run_corpus_compare_format(['--all'])],
	['render:audit (pinned checkouts)', run_render_audit],
];

const run_started = performance.now();
for (const [name, leg] of legs) {
	console.error(`\n════ ${name} ════`);
	const leg_started = performance.now();
	try {
		await leg();
	} catch (e) {
		// Legs normally exit the process themselves on findings; a thrown error is
		// an infrastructure failure (loader, sidecar, FFI) — report + fail the same.
		console.error(`\nFAIL: ${name} errored: ${e instanceof Error ? e.message : e}`);
		Deno.exit(1);
	}
	console.error(`──── ${name} OK (${((performance.now() - leg_started) / 1000).toFixed(1)}s)`);
}
console.error(
	`\n✓ conformance aggregate: all ${legs.length} legs green in ${
		((performance.now() - run_started) / 60_000).toFixed(1)
	}m`,
);
