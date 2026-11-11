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
 * runs at release cadence over the sibling snapshot + working trees and takes minutes.
 *
 * Cost is ~0.17 s for ~2,350 files (pure Rust, no sidecar, population-only fast path — a
 * reparse plus the node census, read straight from the wire bytes), on a binary the preceding
 * `roundtrip:audit` leg has already built with the same profile and features — so this is
 * close to free in `check` and needs no new machinery.
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
 *
 * ## Reach
 *
 * The directory the checkout sits in, read-only (whether the suites are there), and `cargo`,
 * which runs the audit in the checkout with its output inherited
 * (`RoundtripAuditPrettierDeps`, built by `scripts/roots/roundtrip_audit_prettier.ts`).
 */

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

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

/** What `run_roundtrip_audit_prettier` reaches. */
export interface RoundtripAuditPrettierDeps {
	/** The checkout, cargo's working directory. */
	checkout: FsReadDir;
	/** The directory the checkout sits in, where `../prettier` is. */
	siblings: FsReadDir;
	/** `cargo`, its output inherited. */
	cargo: SpawnProgram;
}

/**
 * The leg.
 * @returns the audit's own exit code, `0` when no suite is present
 */
export async function run_roundtrip_audit_prettier(
	deps: RoundtripAuditPrettierDeps
): Promise<number> {
	const present: string[] = [];
	for (const suite of PRETTIER_ROUNDTRIP_SUITES) {
		if (await deps.siblings.exists(suite.slice('../'.length))) present.push(suite);
	}

	if (present.length === 0) {
		log(
			`⚠ roundtrip:audit:prettier NOT RUN — no ../prettier checkout (${PRETTIER_ROUNDTRIP_SUITES.length} suites skipped).`
		);
		log('  This leg is the only one in `check` that reaches non-format-stable inputs.');
		log('  Clone prettier beside this repo (see `deno task doctor`) to gate it locally.');
		return 0;
	}

	// Present-but-incomplete is a broken checkout, not a machine without one — say so, and
	// still audit what is there.
	for (const missing of PRETTIER_ROUNDTRIP_SUITES.filter((p) => !present.includes(p))) {
		log(`⚠ prettier suite missing from a present ../prettier checkout: ${missing}`);
	}

	// Same profile + features as the `roundtrip:audit` leg that precedes this one in `check`,
	// so the binary is already built and `cargo run` is a no-op re-check.
	const result = await deps.cargo.run(
		[
			'run',
			'--profile',
			'corpus',
			'-q',
			'-p',
			'tsv_debug',
			'--features',
			'_internal_audits',
			'--',
			'roundtrip_audit',
			'--gate',
			...present
		],
		{ cwd: deps.checkout }
	);

	return result.kind === 'exited' ? result.code : 1;
}
