/**
 * Guard the canonical-oracle version sync. Three checks split across **two
 * modes**, because they assert facts of different KINDS and so belong at
 * different cadences (exits 1 on any mismatch or missing pin):
 *
 * - `--pins` — half 1 below. A **repo fact**: whether the committed pin sites
 *   agree with each other. Nothing outside the repo can change the verdict, so
 *   it holds on a clean CI checkout, and its failure invalidates the fixture
 *   grading `cargo test` does — it belongs EARLY in `deno task check`, which is
 *   where the `pins:audit` task runs it.
 * - `--checkouts` — halves 2 and 3. **Environment facts**: whether this
 *   machine's sibling checkouts match what the pins say they are. Nothing in
 *   `deno task check` reads those checkouts, so a skew there cannot change any
 *   committed-tree verdict — making a `check` failure on it pure collateral
 *   damage (it halts the chain and leaves the real regressions unrun). The
 *   consumers are the conformance-cadence gates that DO read the checkouts, so
 *   the `pins:audit:checkouts` task runs it as `deno task conformance`'s
 *   preflight; `deno task doctor` reports both modes ahead of time.
 *
 * Passing neither flag runs both (what `doctor` does); passing both is the same.
 *
 * Its reach (`CheckCanonicalPinsDeps`, built by `scripts/roots/check_canonical_pins.ts`): the
 * checkout read-only; for `--checkouts`, the sibling checkouts read-only, each opened by its
 * path, so one that is a symlink to a directory elsewhere is followed, and `git`, when one is on
 * `PATH`, which reads each sibling checkout's ids. `--pins` gets neither.
 *
 * **1. Pin agreement** — the canonical versions must stay IDENTICAL, as exact
 * versions, across the three places that name them (the sync contract documented
 * in `crates/tsv_debug/sidecar/sidecar.ts` and `benches/js/package.json`
 * `//canonical-sync`):
 *
 *   1. `crates/tsv_debug/sidecar/package.json` `dependencies` — the sidecar's manifest,
 *      exactly the five canonical packages
 *   2. the root entry of `crates/tsv_debug/sidecar/package-lock.json` — what `npm ci`
 *      installs, and what the actor refuses an install that differs from
 *   3. benches/js/package.json `dependencies` — what the bench + conformance gates run
 *
 * and the lock installs each canonical package exactly once, at the top of its
 * flat tree (`node_modules/<name>`), at that version. No lock entry is nested
 * (`node_modules/a/node_modules/b`): a nested entry is a second copy of a package,
 * and one `acorn` is what lets the TypeScript plugin extend the parser it imports.
 *
 * Drift here silently grades fixtures and corpora against a different oracle
 * than the bench measures.
 *
 * The sidecar's install posture rides along: its `.npmrc` must equal the root's
 * byte for byte (npm reads only the prefix's own `.npmrc` for
 * `npm ci --prefix crates/tsv_debug/sidecar`, so the root's `ignore-scripts` and
 * `min-release-age` reach the sidecar only through a copy), and its `engines.node`
 * the root's.
 *
 * The prettier OPTIONS ride along, for the same reason and at the same cadence: a
 * version and an option are both pins on what the oracle emits, and the two oracles
 * spell their options in two files (`check_prettier_option_agreement`).
 *
 * The lock also carries the pins nothing else can: the oracle's own
 * transitive dependencies (`LOCKED_TRANSITIVE`). A version pinned by no literal
 * — `esrap`, which prints svelte's compiled output — can otherwise move under a
 * caret range with nothing in the repo changing.
 *
 * **2. Checkout alignment** — the graded-suite sibling checkouts must match the
 * pins they're graded against. The fixtures gates grade INPUTS from `../svelte`
 * / `../acorn-typescript` with the PINNED npm parser, and their
 * SANCTIONED/KNOWN_GAPS ledgers are path-keyed against those suites — a skewed
 * checkout silently grades different inputs than the oracle version defines
 * (and rots the ledgers). An ABSENT checkout is skipped with a note, so a
 * machine without the clones still passes; a PRESENT-but-mismatched one FAILS.
 * This is the HARD half of the alignment guard: the gates themselves only WARN
 * on skew (`lib/fixtures_gate.ts`) and catch it indirectly via their exact
 * `scanned` count pins, which a skew that happens not to move the count slips
 * past.
 *
 * `../prettier` is deliberately NOT gated: its fixture suites are
 * format-comparison inputs whose expected output is computed live per file by
 * the pinned npm prettier (no path-keyed ledger to rot), and the checkout
 * legitimately rides `-dev` versions — `deno task doctor` reports it instead.
 *
 * **3. Checkout drift** (warn-only) — a version string only bumps at
 * release, so upstream commits landing in between change a graded suite or
 * corpus with no version signal at all. That window is precisely how the count
 * pins went stale unnoticed. So each checkout's git object — its HEAD, or for the
 * `../corpora` snapshot the id of its `collections/` tree — is also compared
 * against the id `benches/js/lib/gate_counts.ts` records it was measured at
 * (`GATE_CHECKOUT_IDS`), and a move is reported. Deliberately a WARNING:
 * the count pins are the gate — this exists so that when one trips, "the corpus
 * moved" is distinguishable from "tsv regressed" at a glance instead of by
 * reverse-engineering. Absent / non-git checkouts are skipped. One check here
 * FAILS instead: an entry that also records a `commit` (the snapshot the PGO
 * training fetches by commit, since a tree cannot be fetched) must name a commit
 * whose subtree is the pinned one — a disagreement between the two recorded ids,
 * whatever the checkout's HEAD (a clone that lacks the commit is noted, not graded).
 */

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import { GATE_CHECKOUT_IDS } from '../benches/js/lib/gate_counts.ts';

const MODE_FLAGS = ['--pins', '--checkouts'] as const;

type ModeFlag = (typeof MODE_FLAGS)[number];

const is_mode_flag = (a: string): a is ModeFlag => MODE_FLAGS.some((f) => f === a);

/** Which of the two modes a run takes. */
export interface PinsModes {
	run_pins: boolean;
	run_checkouts: boolean;
}

/**
 * The modes `args` select: `--pins`, `--checkouts`, both, or neither (= both, so a bare
 * invocation stays the full audit).
 * @returns the modes, or the usage error for an argument that is neither flag
 */
export function parse_pins_args(args: ReadonlyArray<string>): PinsModes | { error: string } {
	const unknown_args = args.filter((a) => !is_mode_flag(a));
	if (unknown_args.length > 0) {
		return {
			error:
				`FAIL: unknown argument(s) ${unknown_args.join(', ')} — expected ${MODE_FLAGS.join(' and/or ')} ` +
				'(neither = both modes)'
		};
	}
	const selected = args.filter(is_mode_flag);
	return {
		run_pins: selected.length === 0 || selected.includes('--pins'),
		run_checkouts: selected.length === 0 || selected.includes('--checkouts')
	};
}

/** What `run_check_canonical_pins` reaches. */
export interface CheckCanonicalPinsDeps {
	/** The checkout, where the pin sites are. */
	checkout: FsReadDir;
	/**
	 * Opens a sibling checkout by its path as the messages spell it (`../svelte`), read-only, or
	 * `null` when it is absent, for `--checkouts`. Unused by `--pins`.
	 */
	siblings: { open(path: string): Promise<FsReadDir | null> } | null;
	/**
	 * `git`, which reads each sibling checkout's ids for the drift half, or `null` when the run
	 * has none (`--pins`, or no git on `PATH`), which skips that half and says so.
	 */
	git: SpawnProgram | null;
}

const CANONICAL_PACKAGES = [
	'prettier',
	'prettier-plugin-svelte',
	'svelte',
	'acorn',
	'@sveltejs/acorn-typescript'
] as const;

const SIDECAR_DIR = 'crates/tsv_debug/sidecar';
const SIDECAR_PATH = `${SIDECAR_DIR}/sidecar.ts`;
const SIDECAR_PKG_PATH = `${SIDECAR_DIR}/package.json`;
const SIDECAR_LOCK_PATH = `${SIDECAR_DIR}/package-lock.json`;
const SIDECAR_NPMRC_PATH = `${SIDECAR_DIR}/.npmrc`;
const BENCH_PKG_PATH = 'benches/js/package.json';
const ROOT_PKG_PATH = 'package.json';
const ROOT_NPMRC_PATH = '.npmrc';

/** An exact version: a range in a pin would let `npm install` resolve something else. */
const EXACT_VERSION = /^\d+\.\d+\.\d+$/;

/** The fields of a `package.json` the audit reads. */
interface PackageJson {
	dependencies?: Record<string, string>;
	engines?: { node?: string };
}

/** The fields of an npm `package-lock.json` the audit reads. */
interface PackageLock {
	packages?: Record<string, { version?: string; dependencies?: Record<string, string> }>;
}

/**
 * Transitive npm packages the sidecar lock pins that NO manifest names — the
 * oracle's own dependencies, which float on THEIR declared ranges.
 *
 * `esrap` is why this exists. It PRINTS the JS that svelte's `compile()`
 * returns, so it is the effective oracle for every compile fixture, yet
 * Svelte depends on it by a caret range. Before the lockfile the
 * compile oracle's output could change with no version in this repo changing and
 * no pin site able to see it, which is exactly what happened: esrap 2.3.1
 * stopped dropping a string-literal specifier's `as` alias and silently staled
 * five committed fixtures while `deno task check` stayed green.
 *
 * Bumping one of these is a deliberate oracle move: regenerate the lock
 * (`deno task pins:lock`), re-validate the compile fixtures
 * (`deno task compile:fixtures:validate` — the only check that grades the
 * committed expectations against a LIVE oracle), and update the version here in
 * the same change.
 */
const LOCKED_TRANSITIVE: Record<string, string> = {
	esrap: '2.4.0'
};

/** `name` → version per source, `null` when the source doesn't pin it. */
interface PinSources {
	sidecar_package_json: string | null;
	sidecar_lock_root: string | null;
	bench_package_json: string | null;
}

/**
 * Keys the sidecar's prettier call carries that are STRUCTURAL rather than layout
 * pins — the plugin list and the per-call parser/filepath routing, which the bench
 * spells its own way. Everything else in that call is a pin and must agree.
 *
 * A new key on either side lands in `failures` until it is either mirrored or
 * added here, which is the point: the agreement is maintained by a deliberate act,
 * not by whoever edits one file remembering the other.
 */
const PRETTIER_STRUCTURAL_KEYS = new Set(['plugins', 'parser', 'filepath']);

/** `key: value` literal pairs in an object-literal body, values kept verbatim. */
function option_pairs(body: string): Map<string, string> {
	const pairs = new Map<string, string>();
	for (const m of body.matchAll(/^\s*([a-zA-Z_$][\w$]*)\s*:\s*(.+?),?\s*$/gm)) {
		pairs.set(m[1], m[2].trim().replace(/,$/, ''));
	}
	return pairs;
}

const CHECKOUT_ALIGNMENT: {
	pkg_json: string;
	npm_package: (typeof CANONICAL_PACKAGES)[number];
}[] = [
	{ pkg_json: '../svelte/packages/svelte/package.json', npm_package: 'svelte' },
	{ pkg_json: '../acorn-typescript/package.json', npm_package: '@sveltejs/acorn-typescript' }
];

const BENCH_CANONICAL_PATH = 'benches/js/lib/canonical.ts';

/**
 * The prettier OPTIONS must stay identical across the two oracles, exactly as the
 * versions above must.
 *
 * A version and an option are the same kind of pin — both decide what the oracle
 * emits — but only the version had a guard. The bench's `PRETTIER_OPTIONS`
 * (`benches/js/lib/canonical.ts`) grades every `corpus:compare:format` verdict and
 * is the denominator of every published speedup; the sidecar's inline copy
 * (`crates/tsv_debug/sidecar/sidecar.ts`) regenerates every fixture's
 * `output_prettier.*`. `canonical.ts` has said "keep these two option sets
 * identical" since it was written, with nothing checking it.
 *
 * Note what this does NOT cover, and what does: whether either set still LANDS in
 * the tool is a behavioral question a text comparison can't answer — two identical
 * option sets can both be ignored after an upstream rename. `canonical.ts`'s `init`
 * probe answers it for the bench (`benches/js/lib/format_config_probe.ts`); the
 * sidecar's answer is `deno task fixtures:validate`, whose F2 arm re-formats
 * through the live oracle. This check is the repo-fact half: that the two sets are
 * the SAME set.
 */
function check_prettier_option_agreement(
	bench_src: string,
	sidecar: string,
	failures: string[],
	report: string[]
): void {
	const bench_body = /const PRETTIER_OPTIONS = \{([\s\S]*?)\} as const;/.exec(bench_src)?.[1];
	const sidecar_body = /return await prettier\.format\(content, \{([\s\S]*?)\}\);/.exec(
		sidecar
	)?.[1];
	if (bench_body === undefined || sidecar_body === undefined) {
		failures.push(
			`prettier options: could not locate the option literal in ${
				bench_body === undefined ? BENCH_CANONICAL_PATH : SIDECAR_PATH
			} — this check reads source text, so a refactor of that call site must update it`
		);
		return;
	}

	const bench = option_pairs(bench_body);
	const sidecar_opts = new Map(
		[...option_pairs(sidecar_body)].filter(([k]) => !PRETTIER_STRUCTURAL_KEYS.has(k))
	);

	// A pin missing from one side is drift as surely as a pin whose value differs —
	// the union is what makes a one-sided ADDITION fail rather than pass unnoticed.
	for (const key of new Set([...bench.keys(), ...sidecar_opts.keys()])) {
		const a = bench.get(key);
		const b = sidecar_opts.get(key);
		if (a === b) continue;
		failures.push(
			`prettier option '${key}': ${BENCH_CANONICAL_PATH} has ${a ?? '(absent)'}, ` +
				`${SIDECAR_PATH} has ${b ?? '(absent)'} — the bench oracle and the fixture oracle ` +
				`would format differently, so fixtures and corpus verdicts would grade against ` +
				`different prettiers`
		);
	}
	report.push(`prettier options agree (${[...bench].map(([k, v]) => `${k}=${v}`).join(', ')})`);
}

/**
 * A path into a sibling checkout as the messages spell it (`../svelte/packages/svelte/...`):
 * the checkout's path (`../svelte`) and the path beneath it.
 */
const split_sibling_path = (path: string): { checkout: string; beneath: string } => {
	const slash = path.indexOf('/', '../'.length);
	if (!path.startsWith('../') || slash === -1) throw new Error(`not a sibling path: ${path}`);
	return { checkout: path.slice(0, slash), beneath: path.slice(slash + 1) };
};

/**
 * Resolve a git object of a sibling checkout — `HEAD` (the commit) or `HEAD:<tree>` (a
 * subtree's id) — or `null` when it is absent / not a git repo.
 */
const rev_parse = async (
	git: SpawnProgram,
	siblings: NonNullable<CheckCanonicalPinsDeps['siblings']>,
	repo: string,
	rev: string
): Promise<string | null> => {
	const cwd = await siblings.open(repo);
	if (cwd === null) return null;
	try {
		const result = await git.out(['rev-parse', rev], { cwd });
		return result.kind === 'exited' && result.code === 0 ? result.stdout.trim() : null;
	} catch {
		return null;
	}
};

/**
 * The audit in the modes `modes` names (module doc).
 * @returns the exit code: `1` on any mismatch or missing pin, else `0`
 */
export async function run_check_canonical_pins(
	deps: CheckCanonicalPinsDeps,
	{ run_pins, run_checkouts }: PinsModes
): Promise<number> {
	const read_json = async <T>(path: string): Promise<T> =>
		JSON.parse(await deps.checkout.read_text(path)) as T;
	const bench_deps = (await read_json<PackageJson>(BENCH_PKG_PATH)).dependencies ?? {};

	const failures: string[] = [];
	const report: string[] = [];
	const checkout_notes: string[] = [];
	const drifted: string[] = [];

	if (run_pins) {
		const sidecar = await deps.checkout.read_text(SIDECAR_PATH);
		const sidecar_pkg = await read_json<PackageJson>(SIDECAR_PKG_PATH);
		const sidecar_deps = sidecar_pkg.dependencies ?? {};
		const lock_packages = (await read_json<PackageLock>(SIDECAR_LOCK_PATH)).packages ?? {};
		const lock_root_deps = lock_packages['']?.dependencies ?? {};

		/** Lock keys installing `name`: `node_modules/<name>`, at the top or nested. */
		const lock_keys = (name: string): string[] =>
			Object.keys(lock_packages).filter(
				(key) => key === `node_modules/${name}` || key.endsWith(`/node_modules/${name}`)
			);

		/**
		 * Assert the lock installs `name` exactly once, at the top of the tree, at `expected`,
		 * attributing the source. Two copies of the oracle's printer in one tree would make
		 * "the pinned version" meaningless rather than merely wrong.
		 */
		const assert_locked = (name: string, expected: string, source: string): void => {
			const keys = lock_keys(name);
			if (keys.length === 0) {
				failures.push(
					`${name}: absent from ${SIDECAR_LOCK_PATH} — the sidecar does not install it; regenerate the lock`
				);
			} else if (keys.length > 1) {
				failures.push(
					`${name}: ${SIDECAR_LOCK_PATH} installs ${keys.length} copies (${keys.join(', ')}) — ` +
						'the sidecar would run more than one, so no single version is "the" oracle'
				);
			} else if (keys[0] !== `node_modules/${name}`) {
				failures.push(
					`${name}: ${SIDECAR_LOCK_PATH} installs it nested at ${keys[0]}, not at the top of the tree`
				);
			} else if (lock_packages[keys[0]]?.version !== expected) {
				failures.push(
					`${name}: ${SIDECAR_LOCK_PATH} installs ${lock_packages[keys[0]]?.version ?? '?'} but ${source} says ${expected} — ` +
						'the sidecar runs the lock, so regenerate it or fix the pin'
				);
			}
		};

		// The sidecar's manifest names the oracle and nothing else: a sixth dependency would
		// be an input to the fixtures that no pin site tracks.
		for (const name of Object.keys(sidecar_deps)) {
			if (!CANONICAL_PACKAGES.some((c) => c === name)) {
				failures.push(
					`${name}: ${SIDECAR_PKG_PATH} depends on it, but it is not a canonical package`
				);
			}
		}

		// The lock's root entry names what the manifest does and nothing else: `npm ci` refuses
		// a lock out of sync with `package.json`, so a stray name there is a broken install.
		for (const name of Object.keys(lock_root_deps)) {
			if (!(name in sidecar_deps)) {
				failures.push(
					`${name}: the root entry of ${SIDECAR_LOCK_PATH} depends on it, but ${SIDECAR_PKG_PATH} doesn't — run \`deno task pins:lock\``
				);
			}
		}

		for (const name of CANONICAL_PACKAGES) {
			const pins: PinSources = {
				sidecar_package_json: sidecar_deps[name] ?? null,
				sidecar_lock_root: lock_root_deps[name] ?? null,
				bench_package_json: bench_deps[name] ?? null
			};

			const entries = Object.entries(pins) as [keyof PinSources, string | null][];
			const missing = entries.filter(([, v]) => v === null).map(([k]) => k);
			if (missing.length > 0) {
				failures.push(`${name}: missing pin in ${missing.join(', ')}`);
				continue;
			}
			// Non-null by the check above — asserted once here so the rest of the loop
			// reads as plain strings rather than re-testing a field that cannot be null.
			const versions = entries.map(([, v]) => v as string);
			const distinct = new Set(versions);
			if (distinct.size > 1) {
				failures.push(
					`${name}: pins disagree — ${entries.map(([k, v]) => `${k}=${v}`).join(', ')}`
				);
				continue;
			}
			const pinned = versions[0];
			if (!EXACT_VERSION.test(pinned)) {
				failures.push(`${name}: pinned as ${pinned}, not an exact version`);
				continue;
			}
			report.push(`${name}@${pinned}`);

			// The sidecar RUNS what the lock installs, so a manifest pin the lock disagrees
			// with is a lie: the sidecar would run one version while every pin site claims
			// another. Checked per package rather than once, so the failure names which one
			// drifted.
			assert_locked(name, pinned, 'the canonical pin');
		}

		// The oracle's own transitive dependencies — pinned ONLY by the lock.
		for (const [name, expected] of Object.entries(LOCKED_TRANSITIVE)) {
			assert_locked(name, expected, 'LOCKED_TRANSITIVE in scripts/check_canonical_pins.ts');
		}

		// One copy of every package, the canonical ones and all beneath them: a nested entry
		// is a second copy, which the per-name checks above see only for the names they ask.
		const nested = Object.keys(lock_packages).filter((key) =>
			/node_modules\/.*\/node_modules\//.test(key)
		);
		if (nested.length > 0) {
			failures.push(
				`${SIDECAR_LOCK_PATH} nests ${nested.join(', ')} — a second copy of a package; ` +
					'the sidecar tree must be flat (one acorn for the parser and its TypeScript plugin)'
			);
		}

		const npmrc = await deps.checkout.read_text(SIDECAR_NPMRC_PATH);
		if (npmrc !== (await deps.checkout.read_text(ROOT_NPMRC_PATH))) {
			failures.push(
				`${SIDECAR_NPMRC_PATH} differs from ${ROOT_NPMRC_PATH} — npm reads only the prefix's own ` +
					".npmrc for the sidecar install, so it must be a byte-for-byte copy of the root's"
			);
		}
		const root_engine = (await read_json<PackageJson>(ROOT_PKG_PATH)).engines?.node;
		if (sidecar_pkg.engines?.node !== root_engine) {
			failures.push(
				`${SIDECAR_PKG_PATH} engines.node is ${sidecar_pkg.engines?.node ?? '(absent)'}, ` +
					`${ROOT_PKG_PATH}'s is ${root_engine ?? '(absent)'} — keep the two floors equal`
			);
		}

		check_prettier_option_agreement(
			await deps.checkout.read_text(BENCH_CANONICAL_PATH),
			sidecar,
			failures,
			report
		);
	}

	if (run_checkouts) {
		const { siblings } = deps;
		if (siblings === null)
			throw new Error('--checkouts reads the sibling checkouts, and none were given');
		// Without git every `rev_parse` would read as "all checkouts absent" — silently
		// inert, which is exactly how it once sat unnoticed in `doctor`'s invocation. Say so
		// instead of reporting nothing.
		const { git } = deps;
		if (git === null) {
			checkout_notes.push('checkout drift not checked — git is not on PATH');
		}

		for (const { pkg_json, npm_package } of CHECKOUT_ALIGNMENT) {
			let checkout_version: string | undefined;
			try {
				const { checkout, beneath } = split_sibling_path(pkg_json);
				const dir = await siblings.open(checkout);
				if (dir === null) throw new Error(`no checkout at ${checkout}`);
				checkout_version = (JSON.parse(await dir.read_text(beneath)) as { version?: string })
					.version;
			} catch (error) {
				if (is_fs_error(error, 'outside_root')) {
					failures.push(
						`${npm_package}: ${pkg_json} reaches through a symlink that is absolute or climbs out of its checkout, which is never followed — make it relative within the checkout, or link the checkout itself instead`
					);
				} else {
					checkout_notes.push(`${npm_package} checkout absent — alignment not checked`);
				}
				continue;
			}
			const pinned = bench_deps[npm_package];
			if (checkout_version !== pinned) {
				failures.push(
					`${npm_package}: checkout ${pkg_json} is v${checkout_version ?? '?'} but the oracle pin is v${pinned} — ` +
						'align the checkout to the pinned tag, or bump the canonical pins deliberately (a fixture re-baseline)'
				);
			}
		}

		// Skipped wholesale rather than per-repo: without git every `rev_parse` would return
		// `null`, which would read as "every checkout absent".
		if (git !== null) {
			for (const [repo, { hash, tree, pins, commit }] of Object.entries(GATE_CHECKOUT_IDS)) {
				// A `tree` pin compares that subtree's id at HEAD, so a commit that touches
				// nothing under it is not a move (the corpora snapshot's tooling commits).
				const what = tree === undefined ? repo : `${repo}:${tree}`;
				const current = await rev_parse(
					git,
					siblings,
					repo,
					tree === undefined ? 'HEAD' : `HEAD:${tree}`
				);
				if (current === null) {
					checkout_notes.push(`${repo} absent — drift not checked`);
					continue;
				}
				// A recorded commit is what the PGO training fetches, so its tree must BE the pin —
				// a fact about the two recorded ids, not about this checkout, hence a failure
				if (commit !== undefined && tree !== undefined) {
					const commit_tree = await rev_parse(git, siblings, repo, `${commit}:${tree}`);
					if (commit_tree === null) {
						checkout_notes.push(`${repo} lacks the recorded commit ${commit.slice(0, 12)}`);
					} else if (!commit_tree.startsWith(hash)) {
						failures.push(
							`${what}: the recorded commit ${commit.slice(0, 12)} holds ${tree} ${commit_tree.slice(0, hash.length)}, ` +
								`not the pinned ${hash} — re-record GATE_CHECKOUT_IDS' commit with its tree`
						);
					}
				}
				// The recorded ids are abbreviated, so compare on the prefix.
				if (!current.startsWith(hash)) {
					drifted.push(
						`${what}: measured at ${hash}, now at ${current.slice(0, hash.length)} — pins: ${pins.join(', ')}`
					);
				}
			}
		}
	}

	/** Names the mode that ran, so a message points at the command to re-run. */
	const task =
		run_pins && run_checkouts ? 'canonical pins' : run_pins ? 'pins:audit' : 'pins:audit:checkouts';

	if (failures.length > 0) {
		console.error('FAIL: canonical version sync broken:');
		for (const f of failures) console.error(`  · ${f}`);
		if (run_pins) {
			console.error(
				`  Pin sites: ${SIDECAR_PKG_PATH} and its lock (deno task pins:lock), ${BENCH_PKG_PATH} — edit in lockstep.`
			);
		}
		console.error(
			'  ⚠ Bumping a canonical pin re-baselines the fixture corpus — see docs/benchmarks.md §"Canonical baseline is coupled".'
		);
		return 1;
	}
	if (drifted.length > 0) {
		console.warn(`⚠ ${task} — checkout(s) moved since the gate counts were measured:`);
		for (const d of drifted) console.warn(`  · ${d}`);
		console.warn(
			'  The count pins are the gate; this is the diagnosis. If one trips, suspect corpus movement\n' +
				'  before a tsv regression — and re-record the id in GATE_CHECKOUT_IDS when you re-pin.'
		);
	}
	const done: string[] = [];
	if (run_pins) {
		done.push(
			`canonical pins agree across ${SIDECAR_PKG_PATH}, its lock, and ${BENCH_PKG_PATH}, ` +
				`one copy of each, flat (${report.join(', ')})`
		);
		// Named separately: these are pinned ONLY by the lock, so a green line
		// that didn't say so would leave the oracle's printer looking unguarded.
		done.push(
			`lockfile-only pins hold (${Object.entries(LOCKED_TRANSITIVE)
				.map(([n, v]) => `${n}@${v}`)
				.join(', ')})`
		);
	}
	if (run_checkouts) {
		done.push(
			`checkouts aligned${checkout_notes.length > 0 ? ` (${checkout_notes.join('; ')})` : ''}`
		);
	}
	console.log(`${task} OK — ${done.join('; ')}`);
	return 0;
}
