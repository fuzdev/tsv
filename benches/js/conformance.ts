/**
 * The pre-release conformance aggregate in ONE process (`deno task conformance`):
 * the three parse-conformance gates (svelte-fixtures, ts-fixtures, ts-repo), then
 * `corpus:compare:parse --all` and `corpus:compare:format --all`.
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
 * The tsc_conformance roundtrip self-check — pure-Rust, reads tsgo's committed
 * `.errors.txt` baselines from `../typescript-go`. Shelled out (the only non-JS
 * leg) so it rides the same fail-fast + one summary as the parse gates. The
 * command prints its own report and exits non-zero on a pin mismatch or a
 * missing checkout, so mirror the other legs' "exit the process on a finding".
 */
async function run_tsc_roundtrip(): Promise<void> {
	const {code} = await new Deno.Command('cargo', {
		args: ['run', '-p', 'tsv_debug', '--quiet', 'tsc_conformance', 'roundtrip'],
		stdout: 'inherit',
		stderr: 'inherit',
	}).output();
	if (code !== 0) Deno.exit(1);
}

const legs: [string, () => Promise<void>][] = [
	['conformance:svelte-fixtures', () => run_fixtures_gate(SVELTE_FIXTURES_GATE)],
	['conformance:ts-fixtures', () => run_fixtures_gate(TS_FIXTURES_GATE)],
	['conformance:ts-repo', () => run_ts_repo_compare([])],
	['conformance:tsc-roundtrip', run_tsc_roundtrip],
	['corpus:compare:parse --all', () => run_corpus_compare_parse(['--all'])],
	['corpus:compare:format --all', () => run_corpus_compare_format(['--all'])],
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
