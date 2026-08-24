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
 * **1. Pin agreement** — the canonical versions must stay IDENTICAL across five
 * places (the sync contract documented in `crates/tsv_debug/src/deno/sidecar.ts`
 * and `benches/js/package.json` `//canonical-sync`):
 *
 *   1. sidecar.ts `VERSIONS` object — what `tsv_debug check` reports
 *   2. sidecar.ts static `npm:` import specifiers — what the sidecar actually runs
 *   3. benches/js/package.json `dependencies` — what the bench + conformance gates run
 *   4. actor.rs `deno_config` acorn import-map pin — the shared-acorn-instance pin
 *   5. the sidecar lockfile `crates/tsv_debug/src/deno/deno.lock` — what deno
 *      ACTUALLY resolves, so a literal the lock disagrees with is a lie
 *
 * Drift here silently grades fixtures and corpora against a different oracle
 * than the bench measures.
 *
 * The prettier OPTIONS ride along, for the same reason and at the same cadence: a
 * version and an option are both pins on what the oracle emits, and the two oracles
 * spell their options in two files (`check_prettier_option_agreement`).
 *
 * The lockfile also carries the pins nothing else can: the oracle's own
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

const MODE_FLAGS = ['--pins', '--checkouts'] as const;

type ModeFlag = (typeof MODE_FLAGS)[number];

const is_mode_flag = (a: string): a is ModeFlag => MODE_FLAGS.some((f) => f === a);

const unknown_args = Deno.args.filter((a) => !is_mode_flag(a));
if (unknown_args.length > 0) {
	console.error(
		`FAIL: unknown argument(s) ${unknown_args.join(', ')} — expected ${MODE_FLAGS.join(' and/or ')} ` +
			'(neither = both modes)'
	);
	Deno.exit(1);
}
// Neither flag = both modes, so a bare invocation stays the full audit.
const selected = Deno.args.filter(is_mode_flag);
const run_pins = selected.length === 0 || selected.includes('--pins');
const run_checkouts = selected.length === 0 || selected.includes('--checkouts');

const CANONICAL_PACKAGES = [
	'prettier',
	'prettier-plugin-svelte',
	'svelte',
	'acorn',
	'@sveltejs/acorn-typescript'
] as const;

const SIDECAR_PATH = 'crates/tsv_debug/src/deno/sidecar.ts';
const ACTOR_PATH = 'crates/tsv_debug/src/deno/actor.rs';
const BENCH_PKG_PATH = 'benches/js/package.json';
const SIDECAR_LOCK_PATH = 'crates/tsv_debug/src/deno/deno.lock';

/**
 * Transitive npm packages the sidecar lockfile pins that NO literal pin site
 * names — the oracle's own dependencies, which float on THEIR declared ranges.
 *
 * `esrap` is why this exists. It PRINTS the JS that svelte's `compile()`
 * returns, so it is the effective oracle for every compile fixture, yet
 * `svelte@5.56.9` depends on it as `^2.2.12` — a caret. Before the lockfile the
 * compile oracle's output could change with no version in this repo changing and
 * no pin site able to see it, which is exactly what happened: esrap 2.3.1
 * stopped dropping a string-literal specifier's `as` alias and silently staled
 * five committed fixtures while `deno task check` stayed green.
 *
 * Bumping one of these is a deliberate oracle move: regenerate the lockfile
 * (`deno task pins:lock`), re-validate the compile fixtures
 * (`deno task compile:fixtures:validate` — the only check that grades the
 * committed expectations against a LIVE oracle), and update the version here in
 * the same change.
 */
const LOCKED_TRANSITIVE: Record<string, string> = {
	esrap: '2.3.2'
};

const escape_regex = (s: string): string => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

const sidecar = Deno.readTextFileSync(SIDECAR_PATH);
const actor = Deno.readTextFileSync(ACTOR_PATH);
const bench_pkg = JSON.parse(Deno.readTextFileSync(BENCH_PKG_PATH)) as {
	dependencies?: Record<string, string>;
};
const bench_deps = bench_pkg.dependencies ?? {};
const lock_npm =
	(JSON.parse(Deno.readTextFileSync(SIDECAR_LOCK_PATH)) as { npm?: Record<string, unknown> }).npm ??
	{};

/** `name` → version per source, `null` when the source doesn't pin it. */
interface PinSources {
	versions_object: string | null;
	import_specifier: string | null;
	bench_package_json: string | null;
}

const failures: string[] = [];
const report: string[] = [];
const checkout_notes: string[] = [];
const drifted: string[] = [];

/**
 * Versions the sidecar lockfile resolves for `name`.
 *
 * Lock keys are `name@version`, optionally suffixed with the peer resolution
 * that disambiguates a duplicate (`prettier-plugin-svelte@4.1.1_prettier@3.9.6_svelte@5.56.9`),
 * and a scoped name carries its own leading `@`. Returns every match so the
 * caller can distinguish absent (0) from a genuinely duplicated package (>1) —
 * two copies of the oracle's printer in one tree would make "the pinned version"
 * meaningless rather than merely wrong.
 */
const locked_versions = (name: string): string[] => {
	const prefix = `${name}@`;
	return Object.keys(lock_npm)
		.filter((key) => key.startsWith(prefix))
		.map((key) => key.slice(prefix.length).split('_')[0]);
};

/** Assert the lockfile resolves `name` to exactly `expected`, attributing the source. */
const assert_locked = (name: string, expected: string, source: string): void => {
	const found = locked_versions(name);
	if (found.length === 0) {
		failures.push(
			`${name}: absent from ${SIDECAR_LOCK_PATH} — the sidecar does not resolve it; regenerate the lockfile`
		);
	} else if (found.length > 1) {
		failures.push(
			`${name}: ${SIDECAR_LOCK_PATH} resolves ${found.length} copies (${found.join(', ')}) — ` +
				'the sidecar would run more than one, so no single version is "the" oracle'
		);
	} else if (found[0] !== expected) {
		failures.push(
			`${name}: ${SIDECAR_LOCK_PATH} resolves ${found[0]} but ${source} says ${expected} — ` +
				'the sidecar runs the lockfile, so regenerate it or fix the pin'
		);
	}
};

// --- Pin agreement (see the header docstring, half 1) --------------------------

if (run_pins) {
	// The VERSIONS object body — sliced so a stray `name: 'x.y.z'` elsewhere can't match.
	const versions_block = /const VERSIONS = \{([\s\S]*?)\} as const;/.exec(sidecar)?.[1];
	if (!versions_block) {
		console.error(`FAIL: could not locate the VERSIONS object in ${SIDECAR_PATH}`);
		Deno.exit(1);
	}

	for (const name of CANONICAL_PACKAGES) {
		const escaped = escape_regex(name);
		const pins: PinSources = {
			versions_object:
				new RegExp(`(?:'${escaped}'|${escaped}): '([^']+)'`).exec(versions_block)?.[1] ?? null,
			import_specifier:
				new RegExp(`npm:${escaped}@(\\d+\\.\\d+\\.\\d+)`).exec(sidecar)?.[1] ?? null,
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
			failures.push(`${name}: pins disagree — ${entries.map(([k, v]) => `${k}=${v}`).join(', ')}`);
			continue;
		}
		const pinned = versions[0];
		report.push(`${name}@${pinned}`);

		// The actor.rs import map pins acorn (so the ts plugin extends the same
		// acorn instance) — it must ride along with the acorn pin.
		if (name === 'acorn') {
			const actor_pin = /npm:acorn@(\d+\.\d+\.\d+)/.exec(actor)?.[1] ?? null;
			if (actor_pin === null) {
				failures.push(`acorn: missing npm:acorn@x.y.z import-map pin in ${ACTOR_PATH}`);
			} else if (actor_pin !== pinned) {
				failures.push(`acorn: ${ACTOR_PATH} import-map pin ${actor_pin} disagrees with ${pinned}`);
			}
		}

		// The sidecar RESOLVES against its lockfile, so a literal pin the lock
		// disagrees with is a lie: the sidecar would run one version while every
		// pin site claims another. Checked per package rather than once, so the
		// failure names which one drifted.
		assert_locked(name, pinned, 'the canonical pin');
	}

	// The oracle's own transitive dependencies — pinned ONLY by the lockfile.
	for (const [name, expected] of Object.entries(LOCKED_TRANSITIVE)) {
		assert_locked(name, expected, 'LOCKED_TRANSITIVE in scripts/check_canonical_pins.ts');
	}
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

/**
 * The prettier OPTIONS must stay identical across the two oracles, exactly as the
 * versions above must.
 *
 * A version and an option are the same kind of pin — both decide what the oracle
 * emits — but only the version had a guard. The bench's `PRETTIER_OPTIONS`
 * (`benches/js/lib/canonical.ts`) grades every `corpus:compare:format` verdict and
 * is the denominator of every published speedup; the sidecar's inline copy
 * (`crates/tsv_debug/src/deno/sidecar.ts`) regenerates every fixture's
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
function check_prettier_option_agreement(): void {
	const BENCH_CANONICAL_PATH = 'benches/js/lib/canonical.ts';
	const bench_src = Deno.readTextFileSync(BENCH_CANONICAL_PATH);

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

// Invoked here rather than inside the `run_pins` block above: that block runs before
// this declaration, and `PRETTIER_STRUCTURAL_KEYS` is a `const` — a call from there
// hits the temporal dead zone, where the version checks' hoisted helpers do not.
if (run_pins) check_prettier_option_agreement();

// --- Checkout alignment + commit drift (header docstring, halves 2 and 3) ------

const CHECKOUT_ALIGNMENT: {
	pkg_json: string;
	npm_package: (typeof CANONICAL_PACKAGES)[number];
}[] = [
	{ pkg_json: '../svelte/packages/svelte/package.json', npm_package: 'svelte' },
	{ pkg_json: '../acorn-typescript/package.json', npm_package: '@sveltejs/acorn-typescript' }
];

/** Resolve a checkout's HEAD, or `null` when it is absent / not a git repo. */
const head_commit = (repo: string): string | null => {
	try {
		const { success, stdout } = new Deno.Command('git', {
			args: ['-C', repo, 'rev-parse', 'HEAD'],
			stdout: 'piped',
			stderr: 'null'
		}).outputSync();
		return success ? new TextDecoder().decode(stdout).trim() : null;
	} catch {
		return null;
	}
};

if (run_checkouts) {
	// Without `--allow-run=git` every `head_commit` throws and the drift half reads
	// as "all checkouts absent" — silently inert, which is exactly how it sat
	// unnoticed in `doctor`'s invocation. Say so instead of reporting nothing.
	const can_run_git = Deno.permissions.querySync({ name: 'run', command: 'git' }).state;
	if (can_run_git !== 'granted') {
		checkout_notes.push('commit drift not checked — this invocation lacks --allow-run=git');
	}

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
					'align the checkout to the pinned tag, or bump the canonical pins deliberately (a fixture re-baseline)'
			);
		}
	}

	// Skipped wholesale rather than per-repo: without the permission every
	// `head_commit` returns `null`, which would read as "every checkout absent".
	if (can_run_git === 'granted') {
		for (const [repo, { commit, pins }] of Object.entries(GATE_CHECKOUT_COMMITS)) {
			const head = head_commit(repo);
			if (head === null) {
				checkout_notes.push(`${repo} absent — commit drift not checked`);
				continue;
			}
			// The recorded commits are abbreviated, so compare on the prefix.
			if (!head.startsWith(commit)) {
				drifted.push(
					`${repo}: measured at ${commit}, now at ${head.slice(0, commit.length)} — pins: ${pins.join(', ')}`
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
			`  Pin sites: ${SIDECAR_PATH} (VERSIONS + imports), ${BENCH_PKG_PATH}, ${ACTOR_PATH} — edit in lockstep.`
		);
	}
	console.error(
		'  ⚠ Bumping a canonical pin re-baselines the fixture corpus — see docs/benchmarks.md §"Canonical baseline is coupled".'
	);
	Deno.exit(1);
}
if (drifted.length > 0) {
	console.warn(`⚠ ${task} — checkout(s) moved since the gate counts were measured:`);
	for (const d of drifted) console.warn(`  · ${d}`);
	console.warn(
		'  The count pins are the gate; this is the diagnosis. If one trips, suspect corpus movement\n' +
			'  before a tsv regression — and re-record the commit in GATE_CHECKOUT_COMMITS when you re-pin.'
	);
}
const done: string[] = [];
if (run_pins) {
	done.push(
		'canonical pins agree across sidecar VERSIONS/imports, benches/js/package.json, actor.rs, deno.lock ' +
			`(${report.join(', ')})`
	);
	// Named separately: these are pinned ONLY by the lockfile, so a green line
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
