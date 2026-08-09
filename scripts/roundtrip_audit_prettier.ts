/**
 * The format→reparse round-trip audit over the pinned **Prettier format suites**, as a
 * `deno task check` leg.
 *
 * ## Why a second scope of the same audit
 *
 * `check`'s own `roundtrip:audit` walks `tests/fixtures`, which is format-stable by
 * construction — so the fixture tree can never *contain* the input shape that triggers a
 * valid→unreparseable regression, and the leg is a tripwire there rather than a detector.
 * The prettier suites are the opposite corpus: delimiter-dense, adversarial, and mostly
 * NOT tsv-formatted, which is where output the parser rejects actually surfaces.
 *
 * The class is not hypothetical. A statement-head paren strip turned `for ((let) of foo);`
 * into `for (let of foo);` — output tsv's own parser rejects — and it sat behind a green
 * `deno task check` for a whole PR. Three prettier-suite files caught it on the first run;
 * nothing in `tests/fixtures` could. The same sweep is a leg of `audit:corpus`, but that
 * runs at release cadence over machine-dependent dev repos and takes minutes.
 *
 * Cost is ~0.1 s for ~2,350 files (pure Rust, no sidecar, reparse-only fast path), on a
 * binary the preceding `roundtrip:audit` leg has already built with the same profile and
 * features — so this is close to free in `check` and needs no new machinery.
 *
 * ## Why absence is a warning, not a failure
 *
 * `deno task check` is the committed-tree gate: it must run on a bare checkout, and the CI
 * `check` job has no sibling checkouts at all. So `../prettier` is read **opportunistically**
 * — present, and the leg gates; absent, and it prints a loud NOT RUN line and exits 0. The
 * suites are version-pinned reading references (`deno task doctor` reports the checkout),
 * and this audit is an invariant check rather than a count-pinned gate, so a finding fails
 * wherever it occurs and a smaller corpus cannot silently soften a verdict — the only thing
 * absence costs is coverage, stated on stdout rather than assumed.
 *
 * A *partial* checkout is different and says so: if `../prettier` exists but a listed suite
 * does not, that is a broken checkout rather than a machine without one.
 */

import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';

/**
 * The Prettier format suites the round-trip audit walks — the pinned, adversarial half of
 * `audit:corpus`'s corpus, shared with it so the two scopes cannot drift into meaning
 * different things by the same name.
 *
 * Lives here rather than in `benches/js/lib/corpus.ts` (where the corpus entries are
 * declared) because this module runs inside `deno task check`, which must not import the
 * bench tree — that resolves npm bare specifiers out of `benches/js/node_modules`, which
 * `check` neither installs nor needs.
 *
 * `../prettier/tests/format/html` is deliberately absent (as it is from `audit:corpus`):
 * tsv formats no HTML. `../prettier-plugin-svelte/test` is absent for a duller reason —
 * its Svelte samples are named `.html`, an extension the Rust-side walk does not treat as
 * Svelte, so pointing this leg at it reaches 2 files.
 */
export const PRETTIER_ROUNDTRIP_SUITES = [
	'../prettier/tests/format/typescript',
	'../prettier/tests/format/js',
	'../prettier/tests/format/css'
];

const log = (...args: unknown[]) => console.error(...args);

/** The leg. Exits the process with the audit's own status. */
function run(): never {
	const present = PRETTIER_ROUNDTRIP_SUITES.filter((p) => existsSync(p));

	if (present.length === 0) {
		log(
			`⚠ roundtrip:audit:prettier NOT RUN — no ../prettier checkout (${PRETTIER_ROUNDTRIP_SUITES.length} suites skipped).`
		);
		log('  This leg is the only one in `check` that reaches non-format-stable inputs.');
		log('  Clone prettier beside this repo (see `deno task doctor`) to gate it locally.');
		process.exit(0);
	}

	// Present-but-incomplete is a broken checkout, not a machine without one — say so, and
	// still audit what is there.
	for (const missing of PRETTIER_ROUNDTRIP_SUITES.filter((p) => !present.includes(p))) {
		log(`⚠ prettier suite missing from a present ../prettier checkout: ${missing}`);
	}

	// Same profile + features as the `roundtrip:audit` leg that precedes this one in `check`,
	// so the binary is already built and `cargo run` is a no-op re-check.
	const { status } = spawnSync(
		'cargo',
		[
			'run',
			'--profile',
			'corpus',
			'-q',
			'-p',
			'tsv_debug',
			'--features',
			'audits',
			'--',
			'roundtrip_audit',
			'--gate',
			...present
		],
		{ stdio: 'inherit' }
	);

	process.exit(status ?? 1);
}

// Guarded because `benches/js/corpus_audit.ts` imports the suite list above — running the
// audit (and exiting) as an import side effect would take that driver down with it.
if (import.meta.main) run();
