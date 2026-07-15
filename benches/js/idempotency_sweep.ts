/**
 * F1 (idempotency) sweep over the **real-code** corpus.
 *
 * Drives every file of the `perf` view (the `real` tier — the sibling dev repos +
 * upstream framework source) through `tsv_debug fuzz --iterations 0`: the fuzzer's
 * pristine pass, which asserts the three invariants on each seed **as authored**,
 * with no mutation. The load-bearing one here is **F1** — `format(format(x)) ==
 * format(x)`, tsv's core "input always formats to itself" invariant — alongside
 * no-panic and structural reparse.
 *
 * Why this isn't in `deno task check`: the corpus is sibling checkouts (legitimately
 * machine-dependent) and the sweep is minutes, not seconds. `check`'s `fuzz:audit`
 * covers `tests/fixtures`; this covers real code, which is a different risk surface —
 * a formatter can be idempotent on every curated fixture and still reflow a real
 * component on the second pass. Run it at conformance/release cadence, or after any
 * printer change.
 *
 * Builds with `--profile corpus` (release + `panic = "unwind"`), because the fuzzer
 * drives each file under `catch_unwind` — a `panic = "abort"` release build would
 * take the process down instead of reporting the panic.
 */

import { spawnSync } from 'node:child_process';

import { corpus_present_dirs } from './lib/corpus.ts';

const dirs = await corpus_present_dirs('perf', (...args) => console.error(...args));

if (dirs.length === 0) {
	console.error('Error: no corpus directories present — nothing to sweep.');
	process.exit(1);
}

console.error(`idempotency sweep over ${dirs.length} corpus directories\n`);

const { status } = spawnSync(
	'cargo',
	['run', '--profile', 'corpus', '-q', '-p', 'tsv_debug', '--', 'fuzz', '--iterations', '0', ...dirs],
	{ stdio: 'inherit' },
);

process.exit(status ?? 1);
