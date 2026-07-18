/**
 * Corpus-scale content-loss / robustness audit — the standing gate over **real code**.
 *
 * `deno task check` gates `tests/fixtures`, which is format-stable by construction, so it
 * structurally cannot catch a content-loss / panic / non-idempotency bug in real code — every
 * such bug this cycle was found by a corpus audit or a wide fuzz seed, never by `check`. This
 * driver runs the pure-Rust content-loss audits over the `perf` corpus view (the live dev repos +
 * the pinned framework source) plus the version-pinned Prettier format suites, so a robustness
 * regression fails loudly instead of shipping in the VS Code extension's format-on-save.
 *
 * Legs (all pure Rust, no sidecar; one `--profile corpus` binary — release + `panic = "unwind"`
 * so a formatter panic is caught and reported, not a process kill):
 *
 * - `roundtrip_audit --gate` — output must reparse to the same document (delimiter/structure
 *   corruption the char-frequency SAFETY check is blind to). All dirs.
 * - `comment_audit` — every parsed comment emitted exactly once (dropped / double-printed). All
 *   dirs. Needs the `comment_check` feature (folded into the `audits` umbrella).
 * - `binding_audit --gate` — a glued comment must bind the same subtree after formatting
 *   (cast/annotation re-binding invisible to every other gate). Real code only: the Prettier
 *   suites carry a handful of known adversarial philosophy HARDs (plain comments tsv preserves
 *   in place where prettier relocates), run report-only below so they don't fail the gate.
 * - `authoring_audit` — every render-equivalent authoring of a Svelte document reaches ONE tsv
 *   fixed point (boundary-whitespace idempotency). Real code only (Svelte).
 * - `fuzz --iterations 0` — the pristine F1 sweep: no panic + `format(format(x)) == format(x)` +
 *   structural reparse on every file as authored (the same leg as `idempotency:sweep`). Real code.
 *
 * NOT here: `corpus:compare:format --all` SAFETY (content loss vs prettier) needs the FFI + prettier
 * sidecar and already gates every file in `conformance:all` (publish Step 3b) — this driver is the
 * pure-Rust half. Together they are the "did we actually look over real code" bar.
 *
 * Absent corpus dirs are skipped with a warning (sibling checkouts are legitimately
 * machine-dependent), matching `idempotency:sweep`. A run that finds NO corpus at all fails. The
 * audits themselves are invariant checks (content loss / panic / non-idempotency are real bugs on
 * any machine), so unlike the count-pinned `corpus:compare:*` gates they need no reproducible-subset
 * split — a finding fails wherever it occurs.
 */

import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';

import { corpus_present_dirs } from './lib/corpus.ts';

const log = (...args: unknown[]) => console.error(...args);

// Real code: the live dev repos + the pinned framework source (the `perf` view).
const real_dirs = await corpus_present_dirs('perf', log);

// The version-pinned Prettier format suites (adversarial edge cases). Present-only.
const prettier_suites = [
	'../prettier/tests/format/typescript',
	'../prettier/tests/format/js',
	'../prettier/tests/format/css',
].filter((p) => {
	if (existsSync(p)) return true;
	log(`  ⚠ prettier suite missing, skipped: ${p}`);
	return false;
});

if (real_dirs.length === 0) {
	log('Error: no corpus directories present — nothing to audit.');
	process.exit(1);
}

const all_dirs = [...real_dirs, ...prettier_suites];

// Build the audit binary once (`audits` umbrella = swallow_check + comment_check), then invoke it
// per leg via `cargo run` (a no-op re-check once built) so only `cargo` needs `--allow-run`.
log('building --profile corpus tsv_debug (--features audits) …');
const build = spawnSync(
	'cargo',
	['build', '--profile', 'corpus', '-q', '-p', 'tsv_debug', '--features', 'audits'],
	{ stdio: 'inherit' },
);
if (build.status !== 0) {
	log('Error: build failed.');
	process.exit(build.status ?? 1);
}
const cargo_run = ['run', '--profile', 'corpus', '-q', '-p', 'tsv_debug', '--features', 'audits', '--'];

interface Leg {
	name: string;
	args: string[];
	dirs: string[];
	/** A gating leg fails the run on a non-zero exit; a report-only leg is informational. */
	gating: boolean;
	note?: string;
}

const legs: Leg[] = [
	{ name: 'roundtrip_audit --gate', args: ['roundtrip_audit', '--gate'], dirs: all_dirs, gating: true },
	{ name: 'comment_audit', args: ['comment_audit'], dirs: all_dirs, gating: true },
	{ name: 'binding_audit --gate (real code)', args: ['binding_audit', '--gate'], dirs: real_dirs, gating: true },
	{
		name: 'binding_audit --gate (prettier suites)',
		args: ['binding_audit', '--gate'],
		dirs: prettier_suites,
		gating: false,
		note: 'report-only: a few known adversarial philosophy HARDs (plain comments tsv preserves in place)',
	},
	{ name: 'authoring_audit', args: ['authoring_audit'], dirs: real_dirs, gating: true },
	{ name: 'fuzz --iterations 0 (F1 sweep)', args: ['fuzz', '--iterations', '0'], dirs: real_dirs, gating: true },
];

let failed = false;
for (const leg of legs) {
	if (leg.dirs.length === 0) {
		log(`\n─ ${leg.name}: SKIPPED (no dirs present)`);
		continue;
	}
	log(`\n─ ${leg.name}${leg.gating ? '' : ' [report-only]'}${leg.note ? ` — ${leg.note}` : ''}`);
	const { status } = spawnSync('cargo', [...cargo_run, ...leg.args, ...leg.dirs], {
		stdio: 'inherit',
	});
	if (status !== 0) {
		if (leg.gating) {
			failed = true;
			log(`  ✗ ${leg.name} FAILED (exit ${status})`);
		} else {
			log(`  (report-only leg exited ${status} — not gating)`);
		}
	}
}

if (failed) {
	log('\n✗ corpus audit found a robustness regression over real code.');
	process.exit(1);
}
log('\n✓ corpus audit clean — no content-loss / panic / non-idempotency over real code.');
