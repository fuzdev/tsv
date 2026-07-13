/**
 * Guard the canonical-oracle version sync. Two halves, both pure file reads
 * (cheap enough for `deno task check` via the `pins:audit` task; exits 1 on
 * any mismatch or missing pin):
 *
 * **1. Pin agreement** — the canonical versions must stay IDENTICAL across four
 * places (the sync contract documented in `crates/tsv_debug/src/deno/sidecar.ts`
 * and `benches/js/package.json` `//canonical-sync`):
 *
 *   1. sidecar.ts `VERSIONS` object — what `tsv_debug check` reports
 *   2. sidecar.ts static `npm:` import specifiers — what the sidecar actually runs
 *   3. benches/js/package.json `dependencies` — what the bench + conformance gates run
 *   4. actor.rs `DENO_CONFIG` acorn import-map pin — the shared-acorn-instance pin
 *
 * Drift here silently grades fixtures and corpora against a different oracle
 * than the bench measures.
 *
 * **2. Checkout alignment** — the graded-suite sibling checkouts must match the
 * pins they're graded against. The fixtures gates grade INPUTS from `../svelte`
 * / `../acorn-typescript` with the PINNED npm parser, and their
 * SANCTIONED/KNOWN_GAPS ledgers are path-keyed against those suites — a skewed
 * checkout silently grades different inputs than the oracle version defines
 * (and rots the ledgers). An ABSENT checkout is skipped with a note, so clean
 * machines/CI still pass `deno task check`; a PRESENT-but-mismatched one FAILS.
 * `../prettier` is deliberately NOT gated: its fixture suites are
 * format-comparison inputs whose expected output is computed live per file by
 * the pinned npm prettier (no path-keyed ledger to rot), and the checkout
 * legitimately rides `-dev` versions — `deno task doctor` reports it instead.
 *
 * **3. Checkout COMMIT drift** (warn-only) — a version string only bumps at
 * release, so upstream commits landing in between change a graded suite or
 * corpus with no version signal at all. That window is precisely how the count
 * pins went stale unnoticed. So each checkout's HEAD is also compared against
 * the commit `benches/js/lib/gate_counts.ts` records it was measured at
 * (`GATE_CHECKOUT_COMMITS`), and a move is reported. Deliberately a WARNING:
 * the count pins are the gate — this exists so that when one trips, "the corpus
 * moved" is distinguishable from "tsv regressed" at a glance instead of by
 * reverse-engineering. Absent / non-git checkouts are skipped.
 */

import { GATE_CHECKOUT_COMMITS } from '../benches/js/lib/gate_counts.ts';

const CANONICAL_PACKAGES = [
	'prettier',
	'prettier-plugin-svelte',
	'svelte',
	'acorn',
	'@sveltejs/acorn-typescript',
] as const;

const SIDECAR_PATH = 'crates/tsv_debug/src/deno/sidecar.ts';
const ACTOR_PATH = 'crates/tsv_debug/src/deno/actor.rs';
const BENCH_PKG_PATH = 'benches/js/package.json';

const escape_regex = (s: string): string => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

const sidecar = Deno.readTextFileSync(SIDECAR_PATH);
const actor = Deno.readTextFileSync(ACTOR_PATH);
const bench_pkg = JSON.parse(Deno.readTextFileSync(BENCH_PKG_PATH)) as {
	dependencies?: Record<string, string>;
};
const bench_deps = bench_pkg.dependencies ?? {};

// The VERSIONS object body — sliced so a stray `name: 'x.y.z'` elsewhere can't match.
const versions_block = /const VERSIONS = \{([\s\S]*?)\} as const;/.exec(sidecar)?.[1];
if (!versions_block) {
	console.error(`FAIL: could not locate the VERSIONS object in ${SIDECAR_PATH}`);
	Deno.exit(1);
}

/** `name` → version per source, `null` when the source doesn't pin it. */
interface PinSources {
	versions_object: string | null;
	import_specifier: string | null;
	bench_package_json: string | null;
}

const failures: string[] = [];
const report: string[] = [];

for (const name of CANONICAL_PACKAGES) {
	const escaped = escape_regex(name);
	const pins: PinSources = {
		versions_object:
			new RegExp(`(?:'${escaped}'|${escaped}): '([^']+)'`).exec(versions_block)?.[1] ?? null,
		import_specifier: new RegExp(`npm:${escaped}@(\\d+\\.\\d+\\.\\d+)`).exec(sidecar)?.[1] ?? null,
		bench_package_json: bench_deps[name] ?? null,
	};

	const entries = Object.entries(pins) as [keyof PinSources, string | null][];
	const missing = entries.filter(([, v]) => v === null).map(([k]) => k);
	if (missing.length > 0) {
		failures.push(`${name}: missing pin in ${missing.join(', ')}`);
		continue;
	}
	const distinct = new Set(entries.map(([, v]) => v));
	if (distinct.size > 1) {
		failures.push(
			`${name}: pins disagree — ${entries.map(([k, v]) => `${k}=${v}`).join(', ')}`,
		);
		continue;
	}
	report.push(`${name}@${pins.versions_object}`);

	// The actor.rs import map pins acorn (so the ts plugin extends the same
	// acorn instance) — it must ride along with the acorn pin.
	if (name === 'acorn') {
		const actor_pin = /npm:acorn@(\d+\.\d+\.\d+)/.exec(actor)?.[1] ?? null;
		if (actor_pin === null) {
			failures.push(`acorn: missing npm:acorn@x.y.z import-map pin in ${ACTOR_PATH}`);
		} else if (actor_pin !== pins.versions_object) {
			failures.push(
				`acorn: ${ACTOR_PATH} import-map pin ${actor_pin} disagrees with ${pins.versions_object}`,
			);
		}
	}
}

// --- Checkout alignment (see the header docstring, half 2) ---------------------

const CHECKOUT_ALIGNMENT: { pkg_json: string; npm_package: (typeof CANONICAL_PACKAGES)[number] }[] = [
	{ pkg_json: '../svelte/packages/svelte/package.json', npm_package: 'svelte' },
	{ pkg_json: '../acorn-typescript/package.json', npm_package: '@sveltejs/acorn-typescript' },
];
const checkout_notes: string[] = [];
for (const { pkg_json, npm_package } of CHECKOUT_ALIGNMENT) {
	let checkout_version: string | undefined;
	try {
		checkout_version = (JSON.parse(Deno.readTextFileSync(pkg_json)) as { version?: string })
			.version;
	} catch {
		checkout_notes.push(`${npm_package} checkout absent — alignment not checked`);
		continue;
	}
	const pinned = bench_deps[npm_package];
	if (checkout_version !== pinned) {
		failures.push(
			`${npm_package}: checkout ${pkg_json} is v${checkout_version ?? '?'} but the oracle pin is v${pinned} — ` +
				'align the checkout to the pinned tag, or bump the canonical pins deliberately (a fixture re-baseline)',
		);
	}
}

// --- Checkout commit drift (see the header docstring, half 3) -------------------

/** Resolve a checkout's HEAD, or `null` when it is absent / not a git repo. */
const head_commit = (repo: string): string | null => {
	try {
		const { success, stdout } = new Deno.Command('git', {
			args: ['-C', repo, 'rev-parse', 'HEAD'],
			stdout: 'piped',
			stderr: 'null',
		}).outputSync();
		return success ? new TextDecoder().decode(stdout).trim() : null;
	} catch {
		return null;
	}
};

const drifted: string[] = [];
for (const [repo, { commit, pins }] of Object.entries(GATE_CHECKOUT_COMMITS)) {
	const head = head_commit(repo);
	if (head === null) {
		checkout_notes.push(`${repo} absent — commit drift not checked`);
		continue;
	}
	// The recorded commits are abbreviated, so compare on the prefix.
	if (!head.startsWith(commit)) {
		drifted.push(`${repo}: measured at ${commit}, now at ${head.slice(0, commit.length)} — pins: ${pins}`);
	}
}

if (failures.length > 0) {
	console.error('FAIL: canonical version sync broken:');
	for (const f of failures) console.error(`  · ${f}`);
	console.error(
		`  Pin sites: ${SIDECAR_PATH} (VERSIONS + imports), ${BENCH_PKG_PATH}, ${ACTOR_PATH} — edit in lockstep.\n` +
			'  ⚠ Bumping a canonical pin re-baselines the fixture corpus — see benches/js/CLAUDE.md §"Canonical baseline is coupled".',
	);
	Deno.exit(1);
}
if (drifted.length > 0) {
	console.warn('⚠ pins:audit — checkout(s) moved since the gate counts were measured:');
	for (const d of drifted) console.warn(`  · ${d}`);
	console.warn(
		'  The count pins are the gate; this is the diagnosis. If one trips, suspect corpus movement\n' +
			'  before a tsv regression — and re-record the commit in GATE_CHECKOUT_COMMITS when you re-pin.',
	);
}
console.log(
	`pins:audit OK — canonical pins agree across sidecar VERSIONS/imports, benches/js/package.json, actor.rs ` +
		`(${report.join(', ')}); checkouts aligned${checkout_notes.length > 0 ? ` (${checkout_notes.join('; ')})` : ''}`,
);
