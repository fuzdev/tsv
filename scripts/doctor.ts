/**
 * `doctor` — one-pass, read-only setup check for tsv's diagnostic toolchain. The per-tool
 * preflights fail loud at use time; this answers "am I set up?" BEFORE a run: runtimes, the
 * canonical-oracle pin agreement, the sidecar's install (present + matching its lock), the
 * benches/js `node_modules` (installed + fresh), the sibling oracle checkouts (presence +
 * version skew vs the npm pins), the corpus entry lists, and which build artifacts exist.
 *
 * Exit semantics: exit 1 only for states that would MISLEAD a run or break the core toolchain
 * (canonical pin drift, checkout↔pin skew, stale node_modules, missing npm, cargo, or node, a
 * node below the engines floor the scripts and the sidecar state, or a missing sidecar install)
 * — mere absences (a checkout not cloned, an artifact not built) are ⚠ warnings: they fail loud
 * on their own at use time, and plenty of workflows don't need them. So a green doctor means
 * "nothing on this machine will lie to you", not "everything is installed". `--strict` promotes
 * warnings to failures — the "is this machine FULLY provisioned?" mode for a release box. One
 * exception: in the explicitly optional experimental-typechecker tier an ABSENCE reports as
 * · info, never ⚠, so it stays inert even under `--strict` — that oracle serves no release and
 * no ordinary dev flow, so a fully-provisioned release box legitimately lacks it. A BROKEN
 * checkout there (present, but its baselines missing) still warns: that is the misleading state
 * this tool exists to catch. So does a present checkout, in either tier, that can't say whether
 * it holds what a tool reads, a read failing or a path reaching through a symlink out of it: it
 * warns as what it is, never as a checkout not cloned. The lines that ask `git` for a
 * checkout's id (the `../corpora` snapshot, the harvest stamps) tell a present checkout git
 * can't read from one that isn't there the same way, by its handle.
 *
 * Everything it reads, runs, and loads comes through `DoctorDeps`, which its root
 * (`scripts/roots/doctor.ts`) binds: read-only handles on the checkout, its `target/`, and the
 * sibling checkouts, and one program handle per tool it probes, each run only for its version
 * (`node` also for the canonical pin audit, `git` for the checkouts' object ids and the corpus
 * snapshot's status). The bench modules it reports through (the `node_modules` probe, the
 * oracle pins, the corpus entry lists) are deps too: they read with `node:fs` because Bun's
 * bench runs them.
 *
 * @module
 */

import { is_fs_error } from '@fuzdev/gro/fs_error.ts';

import type { probe_node_modules } from '../benches/js/lib/check_node_modules.ts';
import { CORPORA_ROOT, CORPORA_TREE, CORPORA_URL } from '../benches/js/lib/corpora.ts';
import type {
	corpus_missing_entries,
	corpus_unplaced_collections
} from '../benches/js/lib/corpus.ts';
import { GATE_CHECKOUT_IDS } from '../benches/js/lib/gate_counts.ts';
import {
	checkout_hash,
	checkout_label,
	short_commit,
	type StampGit
} from '../benches/js/lib/harvest_stamp.ts';
import { type CheckoutInput, HARVEST_STAMPS } from '../benches/js/lib/harvest_stamp_table.ts';
import { native_library_filename } from '../benches/js/lib/runtime.ts';
import {
	COMPARE_PROFILE,
	napi_rebuild_task,
	napi_target_path
} from '../benches/js/lib/tsv_artifacts.ts';
import type { AllVersions, load_all_versions } from '../benches/js/lib/versions.ts';
import { BROKEN_CHECKOUT, link_out_message, UNREAD_CHECKOUT_REMEDY } from './sibling_text.ts';
import {
	SIDECAR_DIR,
	SIDECAR_INSTALL_COMMAND,
	SIDECAR_LOCK_PATH,
	sidecar_install_state
} from './sidecar_install.ts';

/** A program's captured run — a gro `SpawnOutput` is one. */
export type DoctorOutput =
	| {
			readonly kind: 'exited';
			readonly code: number;
			readonly stdout: string;
			readonly stderr: string;
	  }
	| {
			readonly kind: 'signaled' | 'timed_out' | 'aborted';
			readonly stdout: string;
			readonly stderr: string;
	  };

/** One program, its output captured — a gro `SpawnProgram` is one. */
export interface DoctorProgram {
	out(args: ReadonlyArray<string>): Promise<DoctorOutput>;
}

/** Each tool doctor probes, `null` when it isn't on `PATH`. */
export interface DoctorPrograms {
	deno: DoctorProgram | null;
	node: DoctorProgram | null;
	bun: DoctorProgram | null;
	cargo: DoctorProgram | null;
	wasm_pack: DoctorProgram | null;
	npm: DoctorProgram | null;
	git: DoctorProgram | null;
}

/** A directory's files, read by relative path — a gro `FsReadDir` is one. */
export interface DoctorDir {
	exists(path: string): Promise<boolean>;
	read_text(path: string): Promise<string>;
	read_text_sync(path: string): string;
	list(path?: string): AsyncIterable<{ readonly name: string; readonly kind: string }>;
}

/**
 * The sibling checkouts doctor reads, each opened beside the checkout by its name: every one
 * its checks read through a handle, and every one it asks `git` about, so a checkout git can't
 * read is told from one that isn't there.
 */
export const DOCTOR_SIBLINGS = [
	'svelte',
	'acorn-typescript',
	'typescript',
	'prettier',
	'prettier-plugin-svelte',
	'test262',
	'wpt',
	'typescript-go',
	'corpora'
] as const;

export type DoctorSibling = (typeof DOCTOR_SIBLINGS)[number];

/** The corpus loader's entry-list probes. */
export interface DoctorCorpus {
	corpus_missing_entries: typeof corpus_missing_entries;
	corpus_unplaced_collections: typeof corpus_unplaced_collections;
}

export interface DoctorDeps {
	/** The checkout, read-only. */
	checkout: DoctorDir;
	/** The checkout's `target/`, wherever a symlink puts it, or `null` when there is none. */
	target: DoctorDir | null;
	/** Each sibling checkout, or `null` when it can't be opened. */
	siblings: Readonly<Record<DoctorSibling, DoctorDir | null>>;
	programs: DoctorPrograms;
	probe_node_modules: typeof probe_node_modules;
	load_all_versions: typeof load_all_versions;
	/** Loads the corpus loader, whose failure to load is itself a finding. */
	load_corpus: () => Promise<DoctorCorpus>;
}

/** What `ci_pins` reads a pin of: Node alone, since no workflow installs Deno. */
type PinnedTool = 'node';

/** The line for a `deno` that is missing, or exits non-zero on `--version`: a warning, since
 * no `check` leg needs one (cargo's test that runs the `deno` on `PATH` skips without a working
 * one, printing a SKIPPED line). */
export const DENO_MISSING =
	'deno missing — check runs without it (cargo test skips a_node_that_runs_deno_is_refused, ' +
	"saying so), as CI does, but the bench's Deno row (bench:deno), its canonical size rows (deno bundle), " +
	"typecheck:js, and branch:report's bench leg (its Deno row, and every task of a side from before " +
	'the tasks moved) need it';

/** A `git` that never starts: a checkout id read through it is `null`, as with no checkout. */
const NO_GIT: StampGit = {
	out: () => Promise.reject(new Error('git missing'))
};

/** The bench's manifest, whose `engines.node` is the type-stripping floor its entries run by. */
const BENCH_PKG_PATH = 'benches/js/package.json';

/** The checkout's workflows, whose `<tool>-version:` pins `ci_pins` reads. */
const WORKFLOWS_DIR = '.github/workflows';

/**
 * The `engines.node` floors a Node on `PATH` is graded against, beside the bench's own
 * type-stripping floor (`BENCH_PKG_PATH`, graded apart since only the bench needs it), and what
 * each one is for.
 */
const NODE_FLOORS: ReadonlyArray<{ file: string; for: string }> = [
	{ file: 'package.json', for: 'the npm scripts on gro, this one among them' },
	{
		file: `${SIDECAR_DIR}/package.json`,
		for: 'the fixture oracle `cargo test` runs, which tsv_debug refuses to start below it'
	}
];

/** `>=major.minor[.patch]`, the floor form `engines.node` states, a missing patch `0`. */
const parse_node_floor = (range: string): [number, number, number] | null => {
	const m = /^>=\s*(\d+)\.(\d+)(?:\.(\d+))?$/.exec(range.trim());
	return m === null ? null : [Number(m[1]), Number(m[2]), Number(m[3] ?? 0)];
};

/** Whether `a` is at least `b`, part by part. */
const version_at_least = (
	a: readonly [number, number, number],
	b: readonly [number, number, number]
): boolean => {
	for (let i = 0; i < 3; i++) if (a[i] !== b[i]) return a[i]! > b[i]!;
	return true;
};

/**
 * Runs the check, printing each section, and returns the exit code: `1` when a check failed,
 * or under `--strict` when one warned, else `0`.
 * @param deps - what the checks read, run, and load
 * @param argv - the arguments after the script; `--strict` is the one it reads
 */
export const run_doctor = async (
	deps: DoctorDeps,
	argv: ReadonlyArray<string>
): Promise<number> => {
	const { checkout, target, siblings, programs } = deps;
	const stamp_git: StampGit = programs.git ?? NO_GIT;

	let warnings = 0;
	let errors = 0;

	const ok = (text: string): void => {
		console.log(`  ✓ ${text}`);
	};
	const warn = (text: string): void => {
		warnings++;
		console.log(`  ⚠ ${text}`);
	};
	const fail = (text: string): void => {
		errors++;
		console.log(`  ✗ ${text}`);
	};
	const info = (text: string): void => {
		console.log(`  · ${text}`);
	};
	const section = (title: string): void => {
		console.log(`\n${title}`);
	};

	/** First line of `program --version`, or null when the program is missing or failing. */
	const run_version = async (program: DoctorProgram | null): Promise<string | null> => {
		if (program === null) return null;
		try {
			const out = await program.out(['--version']);
			if (out.kind !== 'exited' || out.code !== 0) return null;
			return out.stdout.trim().split('\n')[0]!;
		} catch {
			return null;
		}
	};

	/** Whether `git status` reports any change under `subpath` of the checkout at `repo`, without
	 * the index refresh a plain `status` writes into that repository. */
	const git_dirty = async (repo: string, subpath: string): Promise<boolean> => {
		try {
			const out = await stamp_git.out([
				'--no-optional-locks',
				'-C',
				repo,
				'status',
				'--porcelain',
				'--',
				subpath
			]);
			return out.kind === 'exited' && out.code === 0 && out.stdout.length > 0;
		} catch {
			return false;
		}
	};

	/** Whether the build artifact `path` exists beneath `dir`, false for any failure to tell, as
	 * for no `dir`: the section listing them is informational. */
	const exists_in = async (dir: DoctorDir | null, path: string): Promise<boolean> => {
		if (dir === null) return false;
		try {
			return await dir.exists(path);
		} catch {
			return false;
		}
	};

	/**
	 * Whether the sibling checkout `name` has `path`: `false` when the checkout is absent, or
	 * `path` is missing or passes through a file. `null` when a present checkout can't tell,
	 * warned as what it is, so the caller says nothing more: `path` reaching through a symlink
	 * that is absolute or climbs out of the checkout, which no tool follows, or a read that fails.
	 */
	const sibling_has = async (name: DoctorSibling, path: string): Promise<boolean | null> => {
		const dir = siblings[name];
		if (dir === null) return false;
		try {
			return await dir.exists(path);
		} catch (error) {
			if (is_fs_error(error, 'not_a_directory')) return false;
			warn(
				is_fs_error(error, 'outside_root')
					? link_out_message(`../${name}`, path)
					: `../${name} is present but whether ${path} is in it can't be read (${
							error instanceof Error ? error.message.split('\n')[0] : String(error)
						}) — ${BROKEN_CHECKOUT}`
			);
			return null;
		}
	};

	/**
	 * The id `checkout_hash` reads for `input`, or why it has none: `absent` when the checkout
	 * (or the directory beneath it the input names) isn't there, `unread` when it is and git
	 * can't read it (not a git checkout, a corrupt one, the subtree gone, git refusing the
	 * repository, or no git on `PATH`).
	 */
	const checkout_state = async (
		input: CheckoutInput
	): Promise<{ id: string } | 'absent' | 'unread'> => {
		const id = await checkout_hash(stamp_git, input);
		if (id !== null) return { id };
		const [name, ...beneath] = (typeof input === 'string' ? input : input.path)
			.slice('../'.length)
			.split('/');
		const dir = siblings[name as DoctorSibling] ?? null;
		if (dir === null) return 'absent';
		if (beneath.length === 0) return 'unread';
		try {
			return (await dir.exists(beneath.join('/'))) ? 'unread' : 'absent';
		} catch (error) {
			// a path beneath it that can't be told is the checkout's to answer for, not git's
			return is_fs_error(error, 'not_a_directory') ? 'absent' : 'unread';
		}
	};

	/** Why git read no id in a present checkout, as the lines naming one say it. */
	const unread_why = programs.git === null ? 'no git on PATH to read it' : "git can't read it";

	/**
	 * The `version` of the `package.json` at `path` in the present sibling `name`, `null` when it
	 * states none, or `undefined` when the file can't be read, warned as what it is (missing,
	 * unparseable, unreadable, or reached through a symlink out of the checkout), so the caller
	 * says nothing more: a checkout whose version file is broken is never graded as matching.
	 */
	const read_pkg_version = async (
		name: DoctorSibling,
		path: string
	): Promise<string | null | undefined> => {
		const dir = siblings[name];
		if (dir === null) return null;
		try {
			const pkg = JSON.parse(await dir.read_text(path)) as { version?: string };
			return pkg.version ?? null;
		} catch (error) {
			warn(
				is_fs_error(error, 'outside_root')
					? link_out_message(`../${name}`, path)
					: `../${name} is present but ${path} can't be read (${
							error instanceof Error ? error.message.split('\n')[0] : String(error)
						}) — ${BROKEN_CHECKOUT}`
			);
			return undefined;
		}
	};

	/**
	 * Every `<tool>-version:` pin under `.github/workflows/`, as `file -> version` pairs (one
	 * per distinct version a file carries) — empty when the directory can't be read, and when no
	 * workflow pins the tool at all. EVERY workflow, not just `check.yml`: `release.yml` installs
	 * the runtimes it needs for the tag-triggered publish, and a pin that drifts there is graded
	 * by nothing else.
	 *
	 * READ from the workflows rather than mirrored into a constant here, unlike `WASM_PACK_PIN`
	 * below: that one pins a calibration this repo owns (the wasm size bounds), so a local
	 * constant IS its source of truth, while node is pinned in CI and nowhere else — a
	 * mirror would just be a second place to forget. The drift this catches is one-directional
	 * and quiet: an upgraded local runtime leaves CI on the old one, every gate stays green on
	 * both, and the machine that produced the committed bench reports is no longer the machine CI
	 * verifies.
	 */
	const ci_pins = async (
		tool: PinnedTool
	): Promise<ReadonlyArray<{ file: string; version: string }>> => {
		const pins: Array<{ file: string; version: string }> = [];
		try {
			const files: string[] = [];
			for await (const entry of checkout.list(WORKFLOWS_DIR)) {
				if (entry.kind === 'file' && /\.ya?ml$/.test(entry.name)) files.push(entry.name);
			}
			files.sort();
			for (const file of files) {
				const yml = await checkout.read_text(`${WORKFLOWS_DIR}/${file}`);
				// `node-version: '24.14.1'` is quoted, and the pattern reads it bare too. EVERY
				// occurrence, not the first: a workflow sets each pin once per job, so a
				// bump that reached one job and not the others is its own drift — and
				// reading only the first would report the machine aligned while a job trails.
				const all = [...yml.matchAll(new RegExp(`${tool}-version:\\s*'?([\\d.]+)'?`, 'g'))];
				for (const version of new Set(all.map((m) => m[1]!))) pins.push({ file, version });
			}
		} catch {
			return [];
		}
		return pins;
	};

	/** Warn when the installed version differs from what CI pins — see `ci_pins`. A tool no
	 * workflow pins is graded against nothing, which an info line says. */
	const check_ci_pin = async (tool: PinnedTool, installed: string): Promise<void> => {
		const pins = await ci_pins(tool);
		if (pins.length === 0) {
			info(
				`${tool} ${installed} — no workflow pins a ${tool}-version, so nothing grades it against CI`
			);
			return;
		}
		const versions = [...new Set(pins.map((p) => p.version))];
		if (versions.length > 1) {
			warn(
				`${tool} — the workflows pin more than one version ` +
					`(${pins.map((p) => `${p.file}: ${p.version}`).join(', ')}); ` +
					`CI disagrees with itself about which ${tool} verifies this repo`
			);
			return;
		}
		// Exact, not `includes` as the wasm-pack check below does: both versions here are
		// already bare (`24.14.1`, node's `v` stripped by the caller), so a substring
		// test would only buy the false pass where `24.14.1` matches an installed `24.14.10`.
		if (installed === versions[0]) return;
		const files = [...new Set(pins.map((p) => p.file))].join(' + ');
		warn(
			`${tool} ${installed} — differs from the ${versions[0]} that ${files} install; ` +
				`bump the workflows (${tool}-version) so CI verifies what you develop against`
		);
	};

	/** Grades the Node on `PATH` against each floor in `NODE_FLOORS`, one line per floor. */
	const check_node_floors = async (node_version: string): Promise<void> => {
		const m = /^v(\d+)\.(\d+)\.(\d+)/.exec(node_version);
		// an unparseable version already warned above, against the type-stripping floor
		if (m === null) return;
		const version: [number, number, number] = [Number(m[1]), Number(m[2]), Number(m[3])];
		const by_range = new Map<string, Array<{ file: string; for: string }>>();
		for (const floor of NODE_FLOORS) {
			let range: unknown;
			try {
				const pkg = JSON.parse(await checkout.read_text(floor.file)) as {
					engines?: { node?: unknown };
				};
				range = pkg.engines?.node;
			} catch (e) {
				fail(
					`cannot read engines.node from ${floor.file} (${e instanceof Error ? e.message.split('\n')[0] : e}) — ` +
						'the Node floor it states is ungraded'
				);
				continue;
			}
			if (typeof range !== 'string' || parse_node_floor(range) === null) {
				fail(
					`${floor.file} states no engines.node floor of the form >=major.minor[.patch] — ` +
						'the Node floor it states is ungraded'
				);
				continue;
			}
			by_range.set(range, [...(by_range.get(range) ?? []), floor]);
		}
		for (const [range, floors] of by_range) {
			const files = floors.map((f) => f.file).join(' + ');
			if (version_at_least(version, parse_node_floor(range)!)) {
				ok(`node ${node_version} meets engines.node ${range} (${files})`);
			} else {
				fail(
					`node ${node_version} — below engines.node ${range} (${files}): ` +
						floors.map((f) => f.for).join('; ')
				);
			}
		}
	};

	/**
	 * Grades the Node on `PATH` against the bench's own floor, `BENCH_PKG_PATH`'s `engines.node`,
	 * native TS type-stripping, which the harness entries run by: a warning, since only the bench
	 * needs it.
	 */
	const check_bench_node_floor = async (node_version: string): Promise<void> => {
		let range: unknown;
		try {
			const pkg = JSON.parse(await checkout.read_text(BENCH_PKG_PATH)) as {
				engines?: { node?: unknown };
			};
			range = pkg.engines?.node;
		} catch (e) {
			warn(
				`cannot read engines.node from ${BENCH_PKG_PATH} (${e instanceof Error ? e.message.split('\n')[0] : e}) — ` +
					"the bench's Node floor is ungraded"
			);
			return;
		}
		const floor = typeof range === 'string' ? parse_node_floor(range) : null;
		if (floor === null) {
			warn(
				`${BENCH_PKG_PATH} states no engines.node floor of the form >=major.minor[.patch] — ` +
					"the bench's Node floor is ungraded"
			);
			return;
		}
		const stated = (range as string).replace(/^>=\s*/, '');
		const m = /^v(\d+)\.(\d+)\.(\d+)/.exec(node_version);
		if (m !== null && version_at_least([Number(m[1]), Number(m[2]), Number(m[3])], floor)) {
			ok(`node ${node_version} (≥ ${stated})`);
		} else {
			warn(
				`node ${node_version} — < ${stated} lacks native TS type-stripping; the harness entries fail to parse`
			);
		}
	};

	const strict = argv.includes('--strict');

	console.log(
		`tsv doctor — diagnostic-toolchain setup check (read-only${strict ? ', --strict: warnings fail' : ''})`
	);

	// --- Runtimes -----------------------------------------------------------------

	section('Runtimes');
	// the `deno` on `PATH`: `check` runs no Deno task, so a missing one fails nothing `check` runs
	// (cargo test's one Deno test skips without it, saying so); the bench's Deno row and what is
	// typechecked or bundled under Deno need it. No workflow installs Deno, so no pin grades it.
	const deno_line = await run_version(programs.deno);
	if (deno_line === null) {
		warn(DENO_MISSING);
	} else {
		const deno_version = /^deno (\d+\.\d+\.\d+\S*)/.exec(deno_line)?.[1];
		if (deno_version === undefined) {
			warn(`deno — \`deno --version\` printed "${deno_line}", no Deno version`);
		} else {
			ok(`deno ${deno_version}`);
		}
	}

	const node_version = await run_version(programs.node);
	if (node_version === null) {
		fail(
			"node missing — cargo test's fixture oracle (tsv_debug's sidecar) runs on it, and bench:node, " +
				'bench:install, test:npm, and the publish artifact tests need it'
		);
	} else {
		await check_bench_node_floor(node_version);
		await check_ci_pin('node', node_version.replace(/^v/, ''));
		await check_node_floors(node_version);
	}

	const bun_version = await run_version(programs.bun);
	if (bun_version === null) {
		warn(
			"bun missing — bench:bun unavailable, test:bun and the package suites' Bun legs warn-skip, and a publish --wetrun fails without it (--no-check waives)"
		);
	} else ok(`bun ${bun_version}`);

	const cargo_version = await run_version(programs.cargo);
	if (cargo_version === null) fail('cargo missing — nothing Rust builds without it');
	else ok(cargo_version);

	// The one version the repo otherwise records only in CI (`check.yml` pins the
	// same): the published wasm sizes are produced by wasm-pack's bundled wasm-opt,
	// and `validate_artifacts.ts`'s ±8% bounds are calibrated against this version —
	// a different wasm-pack can move the bytes into failing bounds at publish time.
	const WASM_PACK_PIN = '0.15.0';
	const wasm_pack_version = await run_version(programs.wasm_pack);
	if (wasm_pack_version === null) {
		warn(
			'wasm-pack missing — WASM builds (build:wasm:*, bench, publish) unavailable: cargo install wasm-pack'
		);
	} else if (!wasm_pack_version.includes(WASM_PACK_PIN)) {
		warn(
			`${wasm_pack_version} — differs from the pinned ${WASM_PACK_PIN} (check.yml + the ` +
				`validate_artifacts.ts size-bound calibration); sizes may drift into failing bounds at publish`
		);
	} else ok(`${wasm_pack_version} (matches pin)`);

	const npm_version = await run_version(programs.npm);
	if (npm_version === null) {
		fail(
			'npm missing — `npm run` runs the package.json scripts (`check` among them) and bench:install'
		);
	} else ok(`npm ${npm_version}`);

	// --- Canonical pins -----------------------------------------------------------

	section('Canonical oracle pins');
	// No mode flag = BOTH halves — the pin agreement `npm run check` gates plus the
	// checkout alignment/drift `npm run conformance` gates. Doctor is the one place
	// that reports them together, which is what makes an env skew visible before a
	// conformance run rather than during one. It runs under Node, as `pins:audit` does; its
	// commit-drift half shells out to `git rev-parse` and says so when no git is on `PATH`.
	if (programs.node === null) {
		fail(
			'node missing — the canonical pin audit (scripts/roots/check_canonical_pins.ts) runs on it'
		);
	} else {
		let pins: DoctorOutput | null;
		try {
			pins = await programs.node.out(['scripts/roots/check_canonical_pins.ts']);
		} catch (e) {
			pins = null;
			fail(
				`cannot run the canonical pin audit (${e instanceof Error ? e.message.split('\n')[0] : e})`
			);
		}
		if (pins !== null) {
			const success = pins.kind === 'exited' && pins.code === 0;
			const pins_out = (success ? pins.stdout : pins.stderr).trim();
			if (success) ok(pins_out);
			else fail(pins_out.split('\n').join('\n    '));
		}
	}

	// --- Harness deps ---------------------------------------------------------------

	section('Harness deps (benches/js/node_modules)');
	// Both this probe and `load_all_versions` below THROW when `benches/js/package.json`
	// can't be read or parsed — the right posture for a run that would otherwise measure
	// under placeholder labels, and the wrong one HERE: doctor's whole job is to report a
	// broken setup, so a broken pins file must be a `✗` line, not an unhandled rejection
	// that kills the remaining sections and the verdict. Same guard as the corpus probe.
	try {
		const nm = await deps.probe_node_modules();
		if (nm.status === 'ok') {
			ok(
				'installed, and every exact pin (plus each `force_installed` pin) matches the installed version'
			);
		} else if (nm.status === 'missing') warn(nm.message);
		else fail(`${nm.message} — reports would label OLD installed versions with the new pins`);
	} catch (e) {
		fail(
			`cannot read benches/js/package.json (${e instanceof Error ? e.message.split('\n')[0] : e}) — ` +
				'the pins are unreadable, so nothing downstream can be graded against them'
		);
	}

	// --- Oracle checkouts -----------------------------------------------------------

	section('Oracle checkouts (conformance gates / publish Step 3b)');
	// Guarded for the same reason as the probe above: this throws on an unreadable pins
	// file, which is a state to REPORT here rather than to die on. `null` then — the
	// checkouts are still worth listing, but no skew can be graded against a pin nothing
	// could read, so the per-checkout lines say that instead of claiming a match.
	let versions: AllVersions | null = null;
	try {
		versions = await deps.load_all_versions();
	} catch (e) {
		fail(
			`cannot read the oracle pins (${e instanceof Error ? e.message.split('\n')[0] : e}) — ` +
				'checkout-vs-pin skew is ungraded below'
		);
	}
	const pin_suffix = versions ? ', matches the pin' : '; pin unreadable';

	const svelte_tests = await sibling_has('svelte', 'packages/svelte/tests');
	if (svelte_tests) {
		const v = await read_pkg_version('svelte', 'packages/svelte/package.json');
		if (v === undefined) {
			// warned as what it is
		} else if (versions && v !== null && v !== versions.canonical.svelte) {
			warn(
				`../svelte checkout is v${v} but the svelte oracle is pinned v${versions.canonical.svelte} — ` +
					'suite inputs and the grading parser disagree (align the checkout or bump the pins deliberately)'
			);
		} else ok(`../svelte checkout (v${v ?? '?'}${pin_suffix})`);
	} else if (svelte_tests === false)
		warn('../svelte checkout missing — conformance:svelte-fixtures + the corpus suites need it');

	const acorn_ts_tests = await sibling_has('acorn-typescript', 'test');
	if (acorn_ts_tests) {
		const v = await read_pkg_version('acorn-typescript', 'package.json');
		if (v === undefined) {
			// warned as what it is
		} else if (versions && v !== null && v !== versions.canonical['@sveltejs/acorn-typescript']) {
			warn(
				`../acorn-typescript checkout is v${v} but the oracle is pinned ` +
					`v${versions.canonical['@sveltejs/acorn-typescript']} — suite inputs and the grading parser disagree`
			);
		} else ok(`../acorn-typescript checkout (v${v ?? '?'}${pin_suffix})`);
	} else if (acorn_ts_tests === false)
		warn('../acorn-typescript checkout missing — conformance:ts-fixtures needs it');

	const ts_baselines = await sibling_has('typescript', 'tests/baselines/reference');
	if (ts_baselines) {
		ok('../typescript checkout (baselines present)');
	} else if (ts_baselines === false) {
		if (siblings.typescript !== null) {
			warn(
				'../typescript present but tests/baselines/reference missing — conformance:ts-repo will FAIL (partial checkout)'
			);
		} else
			warn('../typescript checkout missing — conformance:ts-repo + bench:harvest:ts-repo need it');
	}

	// Informational (NOT gated by pins:audit — see its docstring): the prettier
	// checkout is a reading reference + corpus-suite source whose oracle output is
	// computed live per file, and it legitimately rides `-dev` versions.
	if (siblings.prettier !== null) {
		const v = await read_pkg_version('prettier', 'package.json');
		if (v === undefined) {
			// warned as what it is
		} else if (versions && v !== null && v.replace(/-dev$/, '') !== versions.canonical.prettier) {
			warn(
				`../prettier checkout is v${v} vs pinned prettier v${versions.canonical.prettier} — its fixture ` +
					'suites (corpus inputs) come from a different version than the live oracle (informational)'
			);
		} else ok(`../prettier checkout (v${v ?? '?'})`);
	} else
		warn(
			'../prettier checkout missing — the corpus prettier suites + layout-reference reading need it'
		);

	const test262 = await sibling_has('test262', 'test');
	if (test262) ok('../test262 checkout (conformance:test262 release gate)');
	else if (test262 === false)
		warn(
			'../test262 checkout missing — conformance:test262 (the release gate, publish Step 3b) + bench:harvest:test262 need it'
		);

	const wpt_css = await sibling_has('wpt', 'css');
	if (wpt_css) ok('../wpt/css checkout (sparse)');
	else if (wpt_css === false)
		warn('../wpt/css checkout missing — bench:harvest:wpt unavailable (manual-cadence tool)');

	// The real-code corpus snapshot: every `benched`/`gated` corpus entry reads one of
	// its collections, and the corpus count pins were measured at the `collections/` tree
	// id `GATE_CHECKOUT_IDS` records — so a checkout whose collections are at any
	// other id grades a different corpus than the pins describe (a refresh re-pins;
	// anything else is skew). The tree, not the commit: the snapshot repo's tooling
	// commits move HEAD without moving a corpus byte.
	const corpora_pin = GATE_CHECKOUT_IDS[CORPORA_ROOT]!.hash;
	const corpora_state = await checkout_state({ path: CORPORA_ROOT, tree: CORPORA_TREE });
	const corpora_head = typeof corpora_state === 'string' ? null : corpora_state.id;
	if (corpora_state === 'absent') {
		warn(
			`${CORPORA_ROOT} checkout missing — the bench + corpus:compare gates read the real-code snapshot ` +
				`(git clone ${CORPORA_URL} ${CORPORA_ROOT})`
		);
	} else if (corpora_head === null) {
		warn(
			programs.git === null
				? `${CORPORA_ROOT} is present, but no git is on PATH to read its ${CORPORA_TREE}/ tree id — the snapshot is ungraded`
				: `${CORPORA_ROOT} is present but git can't read its ${CORPORA_TREE}/ tree id (not a git checkout, one git refuses, a corrupt or unreadable one, or the snapshot gone) — the snapshot is ungraded; ${UNREAD_CHECKOUT_REMEDY}`
		);
	} else if (!corpora_head.startsWith(corpora_pin)) {
		warn(
			`${CORPORA_ROOT}:${CORPORA_TREE} is at ${short_commit(corpora_head)} but the corpus pins were measured at ${corpora_pin} — ` +
				'a snapshot refresh re-runs the corpus gates and re-pins; otherwise check out the pinned snapshot'
		);
	} else if (await git_dirty(CORPORA_ROOT, CORPORA_TREE)) {
		// The gates read the working tree, so bytes the pinned commit doesn't hold would
		// be graded under its name. Nothing in tsv writes there; a stray `tsv format` on a
		// collection is the way this happens.
		warn(
			`${CORPORA_ROOT} is at pinned ${corpora_pin} but ${CORPORA_TREE}/ has local modifications — ` +
				`the corpus gates would grade bytes that commit does not hold (git -C ${CORPORA_ROOT} checkout -- ${CORPORA_TREE})`
		);
	} else ok(`${CORPORA_ROOT} snapshot (${CORPORA_TREE}/ at pinned ${corpora_pin}, clean)`);

	// --- Optional: the experimental typechecker's oracle -----------------------------
	//
	// `../typescript-go` feeds ONLY the on-demand `conformance:tsc-roundtrip` /
	// `conformance:tsc-check` tasks for the experimental `tsv_check` crate — not
	// `npm run check`, not the release gates. A machine that never touches the
	// typechecker needs none of it, so every finding here is a warning at most.
	// Git-SHA-pinned (not npm-versioned), so it isn't in pins:audit either — its tsgo
	// commit is pinned by the Rust count-pins. See docs/typechecker.md.

	section('Optional — experimental typechecker oracle (on-demand tsc_conformance only)');

	info('tsv_check is outside the workspace build — the conformance:tsc-* tasks do not run');

	const tsgo_baselines = await sibling_has(
		'typescript-go',
		'testdata/baselines/reference/submodule'
	);
	if (tsgo_baselines) {
		ok('../typescript-go checkout (tsgo baselines present)');
	} else if (tsgo_baselines === false) {
		if (siblings['typescript-go'] !== null) {
			warn(
				'../typescript-go present but testdata/baselines/reference/submodule missing — conformance:tsc-roundtrip would FAIL (partial checkout)'
			);
		} else
			info('../typescript-go checkout absent — needed only by the on-demand tsc_conformance tasks');
	}

	// The tsc-check task additionally sweeps the corpus INPUTS + bundled libs (unlike
	// roundtrip, which reads only the committed baselines). The corpus is the
	// often-unmaterialized _submodules/TypeScript submodule.
	if (siblings['typescript-go'] !== null) {
		const tsgo_cases = await sibling_has('typescript-go', '_submodules/TypeScript/tests/cases');
		if (tsgo_cases) {
			ok('../typescript-go corpus inputs (_submodules/TypeScript materialized)');
		} else if (tsgo_cases === false) {
			info(
				'../typescript-go corpus inputs absent — only conformance:tsc-check needs them (git submodule update --init in ../typescript-go)'
			);
		}
		const tsgo_libs = await sibling_has('typescript-go', 'internal/bundled/libs');
		if (tsgo_libs) {
			ok('../typescript-go bundled libs present');
		} else if (tsgo_libs === false) {
			info(
				'../typescript-go bundled libs (internal/bundled/libs) absent — only conformance:tsc-check needs them'
			);
		}
	}

	// --- Harvest stamps -------------------------------------------------------------
	//
	// Each stamped grade records the checkout object(s) it ran against — a HEAD commit,
	// or for the corpora snapshot its `collections/` tree id. A stamp whose recorded id
	// no longer matches the checkout means the pin it graded describes the PREVIOUS
	// corpus: `bench:pins:suites` (a `conformance` preflight) re-derives the suite pins
	// and `bench:harvest:svelte-styles` the styles one, but nothing in `check` does, so
	// name it here ahead of a release run. Only the checkout inputs are compared — the
	// oracle-version and pin inputs are `pins:audit`'s.
	//
	// Graded PER CHECKOUT, never all-or-nothing on the first absent one. Two reasons,
	// both of which the earlier short-circuit got wrong: an absent checkout would hide
	// a MOVED sibling in the same stamp (the two reject pins carry three checkouts
	// apiece), and this doctor cannot know what the task will do about the absence —
	// `css:over-acceptance:pin` reads the wpt CACHE, not `../wpt`, so it grades happily
	// with that checkout gone. So report the fact observed here, and leave the task's
	// own `--if-present` verdict to the task.

	section(
		'Harvest stamps (bench:pins:suites — the pin-freshness preflight — + bench:harvest:svelte-styles)'
	);
	for (const [name, { path, task, checkouts }] of Object.entries(HARVEST_STAMPS)) {
		const heads = await Promise.all(
			Object.entries(checkouts).map(async ([key, input]) => {
				const state = await checkout_state(input);
				return {
					key,
					repo: checkout_label(input),
					state,
					head: typeof state === 'string' ? null : state.id
				};
			})
		);
		const absent = heads.filter((h) => h.state === 'absent');
		const unread = heads.filter((h) => h.state === 'unread');
		const gradable = heads.filter((h) => h.head !== null);
		// Named on every line below, so a ✓ never reads as "all three agree" when one
		// of the three was never compared, and a present checkout git can't read never as
		// one not checked out.
		const asides = [
			...(absent.length === 0 ? [] : [`${absent.map((h) => h.repo).join(', ')} not checked out`]),
			...(unread.length === 0
				? []
				: [`${unread.map((h) => h.repo).join(', ')} present, ${unread_why}`])
		];
		const aside = asides.length === 0 ? '' : ` (${asides.join('; ')})`;
		if (gradable.length === 0) {
			info(`${name}: no checkout to grade against${aside}`);
			continue;
		}
		let recorded: Record<string, unknown>;
		try {
			recorded = JSON.parse(await checkout.read_text(path)) as Record<string, unknown>;
		} catch {
			warn(`${name}: never stamped (no ${path}) — run npm run ${task}${aside}`);
			continue;
		}
		// A key the stamp does not carry at all is reported as such rather than as a
		// moved commit: that is what an input ADDED since the stamp was written looks
		// like, and "stamped ?" reads as a corrupt SHA.
		const moved = gradable
			.map((h) => ({ ...h, recorded: recorded[h.key] }))
			.filter((h) => h.recorded !== h.head);
		if (moved.length > 0) {
			warn(
				`${name}: stamp is stale for ${moved
					.map((h) =>
						typeof h.recorded === 'string'
							? `${h.repo} (stamped ${short_commit(h.recorded)}, now ${short_commit(h.head!)})`
							: `${h.repo} (no \`${h.key}\` recorded — the stamp predates that input)`
					)
					.join(', ')} — run npm run ${task}${aside}`
			);
		} else ok(`${name}: stamp matches ${gradable.map((h) => h.repo).join(' + ')}${aside}`);
	}

	// --- Corpus entries -------------------------------------------------------------

	section('Corpus entries (gates + conformance views)');
	try {
		const { corpus_missing_entries, corpus_unplaced_collections } = await deps.load_corpus();
		for (const view of ['gates', 'conformance'] as const) {
			const { missing, optional_missing, total } = await corpus_missing_entries(view);
			if (missing.length > 0) {
				warn(
					`${view} view: ${missing.length}/${total} entries missing — ` +
						`corpus:compare/bench on this view fail fast:\n      ${missing.join('\n      ')}`
				);
			} else
				ok(
					`${view} view: all ${total - optional_missing.length}/${total} required entries present`
				);
			for (const o of optional_missing)
				info(`${view} view: optional entry absent (fail-open, disclosed): ${o}`);
		}
		// A collection the snapshot vendors that `COLLECTIONS` does not place is in no bench
		// or gate view — vendored ahead of its triage, read only by the whole-snapshot sweeps.
		// Listed so it is a standing question rather than a forgotten one.
		const unplaced = await corpus_unplaced_collections();
		if (unplaced.length > 0) {
			info(
				`${unplaced.length} snapshot collection(s) not placed (whole-snapshot sweeps only): ` +
					unplaced.join(', ')
			);
		}
	} catch (e) {
		// The entry list is derived from the snapshot's manifest, so this is also where a
		// checkout whose manifest this reader can't take (or a placement table naming a
		// collection the manifest dropped) surfaces — the message names which.
		warn(`cannot load the corpus entry list: ${e instanceof Error ? e.message.split('\n')[0] : e}`);
	}

	// --- Build artifacts (informational) --------------------------------------------

	section('Build artifacts (built on demand — absence is normal before a run)');
	const ffi_lib = native_library_filename('tsv_ffi');
	const napi_lib = native_library_filename('tsv_napi');
	// each in the checkout, or beneath `target/` in the target handle
	const artifacts: Array<[path: string, task: string]> = [
		[`.pgo/bench/${ffi_lib}`, 'npm run build:bench:pgo (bench:deno, profile-guided)'],
		[`.pgo/bench/${napi_lib}`, 'npm run build:bench:pgo (bench:node/bun, profile-guided)'],
		[
			`target/${napi_target_path(COMPARE_PROFILE)}`,
			`${napi_rebuild_task(COMPARE_PROFILE)} (conformance gates, over-acceptance)`
		],
		[`target/${napi_target_path('napi')}`, `${napi_rebuild_task('napi')} (test:napi)`],
		['crates/tsv_wasm/pkg/all/deno', 'npm run build:wasm:all:deno (bench:deno)'],
		[
			'crates/tsv_wasm/pkg/all/nodejs',
			'npm run build:wasm:all:nodejs (bench:node/bun, over-acceptance)'
		],
		['crates/tsv_wasm/pkg/all/npm', 'npm run build:npm:all (publish)']
	];
	for (const [path, task] of artifacts) {
		const present = path.startsWith('target/')
			? await exists_in(target, path.slice('target/'.length))
			: await exists_in(checkout, path);
		if (present) ok(path);
		else info(`${path} absent — ${task}`);
	}
	info(
		'freshness (artifact vs source mtimes) is enforced at run time by lib/check_artifact_freshness.ts' +
			' (bench/corpus) and scripts/check_staged_freshness.ts (the staged npm/napi packages)'
	);

	// --- Sidecar --------------------------------------------------------------------

	// The fixture oracle `cargo test` and the tsv_debug fixture/compare tooling run. Its actor
	// refuses an install that isn't its lock's, so this is the same check, ahead of a run.
	section('Sidecar (the fixture oracle, run by tsv_debug)');
	const read_sidecar_file = (path: string): string | null => {
		const full = `${SIDECAR_DIR}/${path}`;
		// a missing file is an install's absence; any other failure to read it is thrown
		try {
			return checkout.read_text_sync(full);
		} catch (e) {
			if (is_fs_error(e, 'not_found')) return null;
			throw e;
		}
	};
	try {
		const state = sidecar_install_state(
			await checkout.read_text(SIDECAR_LOCK_PATH),
			read_sidecar_file
		);
		if (state === 'current') ok(`${SIDECAR_DIR} installed as its lock says`);
		else {
			fail(
				`${SIDECAR_DIR} ${state === 'missing' ? 'is not installed' : 'differs from its lock'} — ` +
					`cargo test and the fixture tooling refuse to run it; run \`${SIDECAR_INSTALL_COMMAND}\``
			);
		}
	} catch (e) {
		fail(
			`cannot check ${SIDECAR_DIR}'s install (${e instanceof Error ? e.message.split('\n')[0] : e})`
		);
	}
	info('the Node that answers: `cargo run -p tsv_debug check`');

	// --- Verdict --------------------------------------------------------------------

	console.log('');
	if (errors > 0) {
		console.log(
			`✗ doctor: ${errors} error(s), ${warnings} warning(s) — the ✗ items above would mislead a run; fix them first.`
		);
		return 1;
	}
	if (warnings > 0) {
		if (strict) {
			console.log(
				`✗ doctor --strict: ${warnings} warning(s) — this machine is not fully provisioned.`
			);
			return 1;
		}
		console.log(
			`⚠ doctor: ${warnings} warning(s) — nothing misleading; the ⚠ tools fail loud on their own if used.`
		);
	} else {
		console.log('✓ doctor: everything present and consistent.');
	}
	return 0;
};
