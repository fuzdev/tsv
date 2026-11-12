/**
 * `npm run perf:ab -- <base-ref> [<cand-ref>]` — a parse / format performance A/B between two
 * trees, per language, plus the type strip's, that exits non-zero on a regression.
 *
 * ## What it measures
 *
 * Each side is built from a frozen snapshot of its tree (`git archive` of the build inputs —
 * the workspace manifests, the toolchain pin, `.cargo/`, `src/` and `crates/`; never `tests/`
 * or the docs — into `~/.cache/tsv/perf_ab/<inputs-key>/`, its own `CARGO_TARGET_DIR`, `--release
 * --locked`, at `nice` / `ionice` idle priority), and the measuring drivers are the same on both
 * sides: the current checkout's `crates/tsv_debug/examples/perf_ab.rs` (every language's parse
 * and format) and `perf_ab_strip.rs` (the type strip) are copied over each snapshot before it
 * builds, so only the measured libraries differ (the first file's module doc says why they are
 * examples, the second's why the strip has its own). A tree without the strip's entry point
 * (`STRIP_ENTRY_POINT`, older than the strip) builds the format driver alone, and a run with
 * such a side skips the strip rows, saying so on stderr and under the table. The candidate
 * defaults to the WORKING TREE — `HEAD` plus every uncommitted change, untracked non-ignored
 * files included — snapshotted through a throwaway
 * copy of the index (`git add -A` + `git write-tree` under `GIT_INDEX_FILE`), so neither the
 * real index nor any ref moves (the snapshot's blobs land in the object store as unreachable
 * loose objects, which `git gc` prunes); naming a second ref measures that ref instead. A
 * snapshot is keyed by its BUILD INPUTS — the sha256 of `git ls-tree -r -z <tree> --
 * <BUILD_INPUTS>`, the same list the archive takes, minus the overlaid drivers — so a change to
 * `tests/`, the docs, `scripts/` or a driver file reuses the built snapshot. A built driver
 * is keyed by the driver's content hash and by its build RECIPE (`build_command`, the
 * `BUILD_FIXED_ENV` variables and the `BUILD_ENV` names, hashed), so a re-run reuses both and a recipe change rebuilds both sides
 * rather than measuring an old-recipe binary against a new one. A run keeps the
 * `KEEP_TREES` most recently used snapshots and evicts the rest. The report prints each side's
 * binary sha256, so two readings of one build can be told apart from two builds.
 *
 * The builds run in a cleared environment, gro's base set plus `BUILD_ENV`: a caller's
 * `RUSTFLAGS`, `CARGO_PROFILE_*`, `RUSTUP_TOOLCHAIN` or `RUSTC_WRAPPER` would otherwise reach one
 * side's build and not the other's — a cached base built under flags the candidate never saw.
 * Clearing rather than hashing them into the cache key, because the gate measures the shipped
 * release profile and nothing else; a flag experiment is a manual `profile` A/B. The drivers and
 * `perf` run in a cleared environment too, gro's base set and nothing else (`DRIVER_PASS_ENV`:
 * the drivers read only their arguments), the same for both sides, so a delta holds while the
 * absolute numbers can sit a little off a run under a whole shell environment (its size moves
 * the process's initial stack). Each built driver runs through a program handle made beneath
 * the cache's directory handle once its binary is in place (`PerfAbCache.driver`), and `perf`
 * is given that handle's path.
 *
 * Over the `../corpora` snapshot's collections, `--rounds` rounds (even, so each side goes
 * first in half of them) interleave the two sides language by language; in each, every side
 * runs the driver three times under `perf stat -e instructions:u`: the full pipeline (whose
 * per-file medians of `--iterations` iterations, summed per language, are the wall numbers —
 * `tsv_debug profile`'s statistic), parse only, and zero iterations (the process's fixed cost:
 * start-up, the corpus walk, the reads). Parse instructions are parse-only minus
 * zero-iteration, format instructions full minus parse-only, both per iteration; the strip's
 * full run parses twice, so its instructions are full minus twice parse-only plus the fixed
 * cost (`phase_instructions`). The wall delta
 * is paired per round; while a row's verdict is within its standard error of flipping and the
 * cap could settle it, the run adds balanced pairs of rounds up to `MAX_ROUNDS`
 * (`perf_ab_verdict.ts`, which holds the rule).
 *
 * ## The rule
 *
 * A row FAILS when its paired wall delta exceeds `WALL_REGRESSION_PCT` — unless its
 * instructions moved by less than `INSTRUCTIONS_LAYOUT_PCT` either way, which reads the wall
 * move as code layout and passes the row as LAYOUT — or when its instructions rose by more
 * than `INSTRUCTIONS_REGRESSION_PCT`, whatever the wall. A rise between the two instruction
 * thresholds under a passing wall is a WARN. Wall-clock sits on run-to-run noise and on
 * layout, so it needs the instruction escape; the instruction count is deterministic (it
 * repeats from run to run where the wall wanders), so a large rise is added work even when the
 * wall happened to stay under its line, and more rounds would not change it — only a row whose
 * verdict the wall line still decides earns the extra rounds.
 *
 * A wall FAIL whose instructions sit inside the WARN band, its wall within a layout move of
 * them, still exits 1 but is marked SIGN-OFF: the instruction rise is the change's real cost,
 * which a person accepts or fixes. A re-run cannot clear it — the wall drifts between sittings
 * by more than the in-run spread, so a pass there would be a re-rolled wall, not a cheaper
 * change. The run also prints its LAYOUT READING: the widest wall move among the
 * flat-instruction rows, what code placement alone gave this pair of builds.
 *
 * ## Exit codes
 *
 * 0 every row passes; 1 a row regressed; 2 a harness error (bad arguments, a missing corpus, a
 * build or driver failure, a file set that differs between the sides or between runs, a
 * language with files on one side only, nothing to measure, a non-finite statistic). When
 * `perf` is missing or denied the run says so loudly and grades wall-clock alone — no layout
 * escape, so any wall move over the threshold fails, and no instruction gate — rather than
 * refusing: a strict wall verdict is still a verdict.
 *
 * Linux only (`perf`, `ionice`). Not in `check`: it builds two release trees.
 *
 * @module
 */

import { resolve } from 'node:path';

import type { FsDir, FsReadDir } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import type { FsPaths } from '@fuzdev/gro/fs_paths.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import { CORPORA_COLLECTIONS } from '../benches/js/lib/corpora.ts';
import { lines_of } from '../benches/js/lib/text_lines.ts';
import {
	BUILD_ENV,
	exit_code_of,
	extract_tree,
	type ExtractTreeDeps,
	format_ending,
	git_out,
	HarnessError,
	list_tree,
	sha256_hex
} from './git_snapshot.ts';
import {
	ab_row,
	type AbRow,
	build_inputs_of,
	build_recipe_text,
	CACHE_DIR_PATTERN,
	type CachedTree,
	DEFAULT_ROUNDS,
	file_set_mismatch,
	type FileSet,
	format_pct,
	INSTRUCTIONS_LAYOUT_PCT,
	INSTRUCTIONS_REGRESSION_PCT,
	lang_presence,
	type Lang,
	LANG_PHASES,
	LANGS,
	layout_reading,
	MAX_ROUNDS,
	noisy_rows,
	NOISY_SPREAD_PCT,
	overall_passes,
	parse_perf_stat_counter,
	parse_perf_stat_instructions,
	phase_instructions,
	render_table,
	row_is_ambiguous,
	row_is_settleable,
	type RowSamples,
	SIGN_OFF_EXCESS_PCT,
	sign_off_rows,
	strip_skip_reason,
	trees_to_evict,
	unsettled_rows,
	WALL_REGRESSION_PCT,
	wall_delta_error,
	wants_more_rounds
} from './perf_ab_verdict.ts';

/** The measuring drivers, each an example overlaid on every snapshot before it builds: `format`
 * measures every language's parse + format, `strip` the type strip (its own example, so a tree
 * older than the strip still builds the format driver — `perf_ab_strip.rs`'s module doc). */
const DRIVERS = {
	format: { path: 'crates/tsv_debug/examples/perf_ab.rs', example: 'perf_ab' },
	strip: { path: 'crates/tsv_debug/examples/perf_ab_strip.rs', example: 'perf_ab_strip' }
} as const;
type DriverKind = keyof typeof DRIVERS;
const DRIVER_KINDS: ReadonlyArray<DriverKind> = ['format', 'strip'];
const DRIVER_PATHS = DRIVER_KINDS.map((kind) => DRIVERS[kind].path);

function driver_of(lang: Lang): DriverKind {
	return lang === 'strip' ? 'strip' : 'format';
}

/** The pattern (`git grep -E`) whose match in a tree's `crates/tsv_ts/src` says the strip
 * driver can build against it (`strip_skip_reason`): the entry point's declaration, spelled
 * `strip_in<'s>(` while its output borrowed the source and `strip_in(` since, and both
 * answer the driver's one call. */
const STRIP_ENTRY_POINT = 'pub fn strip_in[<(]';

const DEFAULT_ITERATIONS = 10;

/** The top-level tree entries a driver build reads — what a snapshot extracts AND what its
 * cache key hashes (`build_inputs`), one list for both. `Cargo.toml`, `crates` and `src` (the
 * root package's library) are required; the rest are taken when the tree has them. */
const BUILD_INPUTS = [
	'.cargo',
	'Cargo.lock',
	'Cargo.toml',
	'clippy.toml',
	'crates',
	'rust-toolchain.toml',
	'src'
];
const REQUIRED_BUILD_INPUTS = ['Cargo.toml', 'crates', 'src'];

/** The variables a driver build sets itself, the same for every snapshot. `CARGO_TARGET_DIR` is
 * relative to the build's working directory (the snapshot's `src/`), so each snapshot builds into
 * its own `target/` beside it. Part of the build RECIPE (`build_recipe_text`). */
export const BUILD_FIXED_ENV: Readonly<Record<string, string>> = { CARGO_TARGET_DIR: '../target' };

/** The program a driver build starts, which runs cargo at idle priority (`build_args`). */
export const BUILD_PROGRAM = 'nice';

/** A driver build's arguments to `BUILD_PROGRAM`, run in the snapshot. With `BUILD_PROGRAM`,
 * `BUILD_FIXED_ENV` and `BUILD_ENV` it is the build RECIPE, whose hash names the binary
 * (`build_recipe_text`). */
export function build_args(example: string): Array<string> {
	return [
		'-n',
		'19',
		'ionice',
		'-c',
		'3',
		'cargo',
		'build',
		'--release',
		'--locked',
		'-p',
		'tsv_debug',
		'--example',
		example,
		'--message-format=json-render-diagnostics'
	];
}

/** The host variables a driver, and `perf` around it, is given beside gro's base environment:
 * none, as the drivers read only their arguments, and `perf` finds its own configuration
 * through `HOME`. The same for both sides, so the A/B delta holds. */
export const DRIVER_PASS_ENV: ReadonlyArray<string> = [];

/** A 1-minute load average above this before the run warns: something else holds a core. */
const LOAD_WARN = 1;
/** The run itself keeps one core busy, so the after-reading is allowed one more. */
const LOAD_WARN_AFTER = LOAD_WARN + 1;

/** Processes whose presence means a build or a corpus run competes for the machine. */
const HEAVY_PROCESSES = ['rustc', 'cargo', 'tsv_debug'];

interface Options {
	base_ref: string;
	cand_ref: string | null;
	rounds: number;
	iterations: number;
	corpus: string;
	/** The snapshot cache, or null for the default under `~/.cache/tsv/perf_ab`. */
	cache: string | null;
	json: boolean;
}

const USAGE = `usage: npm run perf:ab -- <base-ref> [<cand-ref>] [--rounds N] [--iterations N] [--corpus DIR] [--json]
  <cand-ref> defaults to the working tree (HEAD + uncommitted changes)
  --rounds N      interleaved base/cand rounds, even (default ${DEFAULT_ROUNDS}; an ambiguous row
                  extends the run by pairs up to ${MAX_ROUNDS})
  --iterations N  iterations per file per run (default ${DEFAULT_ITERATIONS})
  --corpus DIR    corpus directory (default ${CORPORA_COLLECTIONS})
  --json          print the result as JSON instead of the table`;

/** The options `args` give, `'help'` for `--help`; relative paths resolve against `checkout`. */
function parse_options(args: ReadonlyArray<string>, checkout: string): Options | 'help' {
	const positional: Array<string> = [];
	let rounds = DEFAULT_ROUNDS;
	let iterations = DEFAULT_ITERATIONS;
	let corpus = CORPORA_COLLECTIONS;
	let cache: string | null = null;
	let json = false;
	const count = (flag: string, value: string | undefined): number => {
		const n = Number(value);
		if (!Number.isInteger(n) || n < 1) throw new HarnessError(`${flag} takes a positive integer`);
		return n;
	};
	for (let i = 0; i < args.length; i++) {
		const arg = args[i]!;
		if (arg === '--rounds') rounds = count(arg, args[++i]);
		else if (arg === '--iterations') iterations = count(arg, args[++i]);
		else if (arg === '--corpus') {
			const value = args[++i];
			if (!value) throw new HarnessError('--corpus takes a directory');
			corpus = value;
		} else if (arg === '--cache') {
			// internal, for `branch:report`, which keeps its runs beneath its own cache
			const value = args[++i];
			if (!value) throw new HarnessError('--cache takes a directory');
			cache = value;
		} else if (arg === '--json') json = true;
		else if (arg === '--help' || arg === '-h') return 'help';
		else if (arg.startsWith('-')) throw new HarnessError(`unknown flag ${arg}`);
		else positional.push(arg);
	}
	if (positional.length < 1 || positional.length > 2) {
		throw new HarnessError('expected <base-ref> and an optional <cand-ref>');
	}
	if (rounds % 2 !== 0) {
		throw new HarnessError(
			`--rounds ${rounds}: takes an even count, so each side goes first equally often`
		);
	}
	return {
		base_ref: positional[0]!,
		cand_ref: positional[1] ?? null,
		rounds,
		iterations,
		corpus: resolve(checkout, corpus),
		cache: cache === null ? null : resolve(checkout, cache),
		json
	};
}

/** One measured side: what it is called, the tree it builds, and that tree's build inputs. */
export interface Side {
	label: string;
	/** The commit, or null for a working-tree snapshot. */
	commit: string | null;
	tree: string;
	/** The snapshot's cache directory name: the sha256 of the build-inputs listing. */
	key: string;
	/** The `BUILD_INPUTS` entries the tree has — what its snapshot extracts. */
	inputs: Array<string>;
	/** Whether the tree has the strip driver's entry point (`STRIP_ENTRY_POINT`). */
	has_strip: boolean;
}

/** The snapshot cache a run builds into, and how it runs what it built there. */
export interface PerfAbCache {
	/** The cache's directory: the snapshots, the built drivers, the scratch index and `perf
	 * stat`'s output. */
	dir: FsDir;
	/** The built driver at `name`, a path beneath `dir`, as a program, made once its binary is
	 * in place. */
	driver: (name: string) => SpawnProgram;
}

/** What `run_perf_ab_command` reaches: the checkout, the paths it is given, the cache, and each
 * program it runs, every one built by the root with the environment it gets. */
export interface PerfAbDeps extends ExtractTreeDeps {
	/** The checkout: the drivers' sources, the working directory of `git` and of the drivers, and
	 * what a relative `--corpus` or `--cache` resolves against. */
	checkout: FsReadDir;
	/** Opens a path from outside: the corpus, and the index a linked worktree keeps outside the
	 * checkout. */
	fs_paths: FsPaths;
	/** `HOME`, beneath which the default cache sits. */
	home: string | undefined;
	/** Opens the cache at `path`, absolute, made when missing. */
	open_cache: (path: string) => Promise<PerfAbCache>;
	/** `git` with `GIT_INDEX_FILE` set to `index`, for the working-tree snapshot. */
	git_with_index: (index: string) => SpawnProgram;
	/** `BUILD_PROGRAM`, in the build environment (`BUILD_ENV`, `BUILD_FIXED_ENV`). */
	build: SpawnProgram;
	/** `perf`, or `null` when it is not installed. */
	perf: SpawnProgram | null;
	/** `pgrep`, or `null` when it is not installed. */
	pgrep: SpawnProgram | null;
	/** The 1-minute load average. */
	load_1m: () => number;
}

/** The cache key and the present build inputs of `tree`, from one recursive `git ls-tree`
 * listing of `BUILD_INPUTS` (the drivers left out: the overlay replaces them), and whether it
 * has the strip. */
async function build_inputs(
	deps: PerfAbDeps,
	tree: string
): Promise<{ key: string; inputs: Array<string>; has_strip: boolean }> {
	const listing = await list_tree(deps.git, deps.checkout, tree, BUILD_INPUTS);
	const { names, missing, key_text } = build_inputs_of(
		listing,
		REQUIRED_BUILD_INPUTS,
		DRIVER_PATHS
	);
	if (missing.length > 0) {
		throw new HarnessError(`tree ${tree} has no ${missing.join(', ')} — not a tsv workspace`);
	}
	return {
		key: await sha256_hex(new TextEncoder().encode(key_text)),
		inputs: names,
		has_strip: await tree_has_strip(deps.git, deps.checkout, tree)
	};
}

/** Whether `tree`'s `crates/tsv_ts/src` has `STRIP_ENTRY_POINT`, run in `cwd` (`git grep`:
 * exit 1 is "no", anything but 0 or 1 an error). */
export async function tree_has_strip(
	git: SpawnProgram,
	cwd: FsReadDir,
	tree: string
): Promise<boolean> {
	const args = ['grep', '-q', '-E', STRIP_ENTRY_POINT, tree, '--', 'crates/tsv_ts/src'];
	const result = await git.out(args, { cwd });
	const code = exit_code_of(result);
	if (code === 0) return true;
	if (code === 1) return false;
	throw new HarnessError(`git ${args.join(' ')} failed:\n${result.stderr.trim()}`);
}

async function resolve_ref(deps: PerfAbDeps, ref: string): Promise<Side> {
	const git = (args: ReadonlyArray<string>) => git_out(deps.git, deps.checkout, args);
	const commit = await git(['rev-parse', '--verify', '--quiet', `${ref}^{commit}`]).catch(() => {
		throw new HarnessError(`not a commit: ${ref}`);
	});
	const tree = await git(['rev-parse', `${commit}^{tree}`]);
	return {
		label: `${ref} (${commit.slice(0, 9)})`,
		commit,
		tree,
		...(await build_inputs(deps, tree))
	};
}

/** Snapshot the working tree as a git tree object through a throwaway copy of the index. */
async function resolve_worktree(deps: PerfAbDeps, cache: FsDir): Promise<Side> {
	const git = (args: ReadonlyArray<string>) => git_out(deps.git, deps.checkout, args);
	const index = resolve(deps.checkout.path, await git(['rev-parse', '--git-path', 'index']));
	const opened = await deps.fs_paths.open_path(index);
	const scratch = `index.${deps.pid}`;
	await cache.write(scratch, await opened.dir.read(opened.path));
	try {
		const scratch_git = deps.git_with_index(`${cache.path}/${scratch}`);
		await git_out(scratch_git, deps.checkout, ['add', '-A']);
		const tree = await git_out(scratch_git, deps.checkout, ['write-tree']);
		const head = await git(['rev-parse', 'HEAD']);
		const head_tree = await git(['rev-parse', 'HEAD^{tree}']);
		const label =
			tree === head_tree
				? `worktree (= HEAD ${head.slice(0, 9)})`
				: `worktree (HEAD ${head.slice(0, 9)} + changes)`;
		return { label, commit: null, tree, ...(await build_inputs(deps, tree)) };
	} finally {
		await cache.remove(scratch, { missing_ok: true });
	}
}

/** Keep the snapshots this run measures plus the most recently used others (`KEEP_TREES` in
 * all); delete the rest, and mark the measured ones used now. */
async function evict_cache(cache: FsDir, sides: ReadonlyArray<Side>): Promise<void> {
	const in_use = new Set(sides.map((side) => side.key));
	const now = Date.now();
	for (const tree of in_use) {
		if (await cache.exists(tree)) await cache.set_times(tree, { mtime_ms: now });
	}
	const trees: Array<CachedTree> = [];
	for await (const entry of cache.list()) {
		if (entry.kind !== 'directory' || !CACHE_DIR_PATTERN.test(entry.name)) continue;
		const info = await cache.stat(entry.name);
		if (info !== null) trees.push({ name: entry.name, mtime_ms: info.mtime_ms });
	}
	for (const name of trees_to_evict(trees, in_use)) {
		console.error(`perf:ab: evicting cached snapshot ${name.slice(0, 12)}`);
		await cache.remove(name, { recursive: true, missing_ok: true });
	}
}

/** One driver as the run builds it: its source, the source's hash and its recipe's hash. */
export interface DriverBuild {
	kind: DriverKind;
	source: Uint8Array;
	hash: string;
	recipe_hash: string;
}

/** The name, beneath the cache, of `driver`'s binary for `side`: content-addressed by the
 * side's build inputs, the driver's source and the build recipe, so a binary of that name is
 * never rebuilt. */
export function driver_binary_name(side_key: string, driver: DriverBuild): string {
	const { example } = DRIVERS[driver.kind];
	return `${side_key}/${example}-${driver.hash.slice(0, 16)}-${driver.recipe_hash.slice(0, 16)}`;
}

/** `path`, absolute, as a path beneath `dir`. */
export function path_beneath(dir: FsReadDir, path: string): string {
	const prefix = `${dir.path}/`;
	if (!path.startsWith(prefix)) throw new HarnessError(`${path} is not beneath ${dir.path}`);
	return path.slice(prefix.length);
}

/** What `ensure_driver` runs: the snapshot's extraction and the build. */
export interface EnsureDriverDeps extends ExtractTreeDeps {
	/** `BUILD_PROGRAM`, in the build environment. */
	build: SpawnProgram;
}

/** Build (or reuse) `driver` for `side`; returns the binary's path beneath `cache`. */
export async function ensure_driver(
	deps: EnsureDriverDeps,
	cache: FsDir,
	side: Side,
	driver: DriverBuild
): Promise<string> {
	const { path: driver_path, example } = DRIVERS[driver.kind];
	const binary = driver_binary_name(side.key, driver);
	if (await cache.exists(binary)) {
		console.error(`perf:ab: ${side.label}: reusing ${cache.path}/${binary}`);
		return binary;
	}
	await cache.mkdir(side.key, { recursive: true });
	const base = cache.dir(side.key);
	if (!(await base.exists('src'))) {
		console.error(
			`perf:ab: ${side.label}: snapshotting tree ${side.tree.slice(0, 12)} (build inputs ${side.key.slice(0, 12)})`
		);
		await extract_tree(deps, side.tree, side.inputs, base, 'src');
	}
	const src = base.dir('src');
	const overlay_dir = driver_path.slice(0, driver_path.lastIndexOf('/'));
	await src.mkdir(overlay_dir, { recursive: true });
	await src.write(driver_path, driver.source);

	console.error(`perf:ab: ${side.label}: building the ${example} driver (release, idle priority)`);
	const child = await deps.build.start(build_args(example), {
		cwd: src,
		stdio: ['ignore', 'pipe', 'inherit']
	});
	let executable: string | null = null;
	for await (const line of lines_of(child.stdout!)) {
		let message: unknown;
		try {
			message = JSON.parse(line);
		} catch {
			continue;
		}
		const {
			reason,
			target,
			executable: path
		} = message as {
			reason?: unknown;
			target?: { kind?: unknown; name?: unknown };
			executable?: unknown;
		};
		if (
			reason === 'compiler-artifact' &&
			target?.name === example &&
			Array.isArray(target.kind) &&
			target.kind.includes('example') &&
			typeof path === 'string'
		) {
			executable = path;
		}
	}
	const status = await child.closed;
	const code = exit_code_of(status);
	if (code !== 0 || executable === null) {
		throw new HarnessError(
			`building the ${example} driver for ${side.label} failed (${code === 0 ? 'no executable reported' : `cargo ${format_ending(status)}`}). ` +
				`The driver compiles against each tree's language-crate entry points; a tree older than ` +
				`those entry points cannot be measured with it.`
		);
	}
	// Copied out of the target dir so a later build for another driver hash cannot replace it,
	// under a temporary name renamed into place so an interrupted copy is never reused.
	const partial = `${binary}.partial.${deps.pid}`;
	await cache.copy(path_beneath(cache, executable), partial);
	await cache.set_mode(partial, 0o755);
	await cache.rename(partial, binary);
	return binary;
}

/** A built driver: its binary's path beneath the cache, and the program that runs it. */
export interface BuiltDriver {
	name: string;
	program: SpawnProgram;
}

/** One side's built drivers; `strip` is null when the strip rows are skipped. */
export type Binaries = Record<DriverKind, BuiltDriver | null>;

/** Build (or reuse) `side`'s drivers, `strip` only when `with_strip`, each made a program
 * through the cache once its binary is in place. */
export async function build_side(
	deps: EnsureDriverDeps,
	cache: PerfAbCache,
	side: Side,
	drivers: Readonly<Record<DriverKind, DriverBuild>>,
	with_strip: boolean
): Promise<Binaries> {
	const built = async (driver: DriverBuild): Promise<BuiltDriver> => {
		const name = await ensure_driver(deps, cache.dir, side, driver);
		return { name, program: cache.driver(name) };
	};
	return {
		format: await built(drivers.format),
		strip: with_strip ? await built(drivers.strip) : null
	};
}

/** Whether `perf stat -e instructions:u` works here; the reason when it does not. */
async function probe_perf(perf: SpawnProgram | null): Promise<string | null> {
	if (perf === null) return '`perf` is not installed';
	const result = await perf.out(['stat', '-x,', '-e', 'instructions:u', '--', 'true']);
	if (exit_code_of(result) !== 0) {
		return `perf stat ${format_ending(result)}: ${result.stderr.trim()}`;
	}
	if (parse_perf_stat_instructions(result.stderr) === null) {
		return `perf stat counted no instructions:u (${result.stderr.trim() || 'no output'})`;
	}
	return null;
}

interface DriverOutput extends FileSet {
	iterations: number;
	parse_us: number;
	/** The wall of the work after the parse — the format driver's `format_us`, the strip
	 * driver's `strip_us`. */
	work_us: number;
	/** The formatted (or stripped) output's bytes, summed over the files (zero under
	 * `--parse-only`). */
	output_bytes: number;
	/** Retired user instructions for the whole process, or null without `perf`. */
	instructions: number | null;
	/** User cycles for the whole process, or null when `perf` did not count them. */
	cycles: number | null;
}

type DriverMode = 'full' | 'parse' | 'fixed';

/** What a driver run needs besides the driver: `perf`, where both run, and where `perf` writes. */
interface DriverRunner {
	checkout: FsReadDir;
	perf: SpawnProgram | null;
	cache: FsDir;
	pid: number;
	options: Options;
}

async function run_driver(
	runner: DriverRunner,
	driver: SpawnProgram,
	lang: Lang,
	mode: DriverMode,
	use_perf: boolean
): Promise<DriverOutput> {
	const { options, cache } = runner;
	const binary = driver.path;
	const iterations = mode === 'fixed' ? 0 : options.iterations;
	const driver_args =
		lang === 'strip'
			? [options.corpus, '--iterations', String(iterations)]
			: [options.corpus, '--lang', lang, '--iterations', String(iterations)];
	if (mode === 'parse') driver_args.push('--parse-only');
	const stat_file = `perf_stat.${runner.pid}.csv`;
	const result =
		use_perf && runner.perf !== null
			? await runner.perf.out(
					[
						'stat',
						'-x,',
						'-e',
						'instructions:u,cycles:u',
						'-o',
						`${cache.path}/${stat_file}`,
						'--',
						binary,
						...driver_args
					],
					{ cwd: runner.checkout }
				)
			: await driver.out(driver_args, { cwd: runner.checkout });
	if (exit_code_of(result) !== 0) {
		throw new HarnessError(
			`driver ${binary} ${driver_args.join(' ')} ${format_ending(result)}:\n${result.stderr.trim()}`
		);
	}
	let instructions: number | null = null;
	let cycles: number | null = null;
	if (use_perf) {
		const stat_text = await cache.read_text(stat_file);
		await cache.remove(stat_file, { missing_ok: true });
		instructions = parse_perf_stat_instructions(stat_text);
		if (instructions === null) throw new HarnessError(`perf stat counted nothing:\n${stat_text}`);
		cycles = parse_perf_stat_counter(stat_text, 'cycles');
	}
	let printed: Record<string, unknown>;
	try {
		printed = JSON.parse(result.stdout);
	} catch {
		throw new HarnessError(`driver ${binary} printed no JSON:\n${result.stdout}`);
	}
	const number_of = (field: string): number => {
		const value = printed[field];
		if (typeof value !== 'number') {
			throw new HarnessError(`driver ${binary} printed no number \`${field}\`:\n${result.stdout}`);
		}
		return value;
	};
	return {
		files: number_of('files'),
		bytes: number_of('bytes'),
		errors: number_of('errors'),
		iterations: number_of('iterations'),
		parse_us: number_of('parse_us'),
		work_us: number_of(lang === 'strip' ? 'strip_us' : 'format_us'),
		output_bytes: number_of('output_bytes'),
		instructions,
		cycles
	};
}

/** The names among `HEAVY_PROCESSES` running right now (one `pgrep` each; empty when `pgrep`
 * itself is missing). */
async function heavy_processes(pgrep: SpawnProgram | null): Promise<Array<string>> {
	const running: Array<string> = [];
	if (pgrep === null) return running;
	for (const name of HEAVY_PROCESSES) {
		try {
			if (exit_code_of(await pgrep.out(['-x', name])) === 0) running.push(name);
		} catch {
			return running;
		}
	}
	return running;
}

/** Print a load reading plus the heavy processes running now, warning over `threshold`. */
async function report_load(
	pgrep: SpawnProgram | null,
	when: 'before' | 'after',
	load: number,
	threshold: number
): Promise<void> {
	const heavy = await heavy_processes(pgrep);
	console.error(
		`perf:ab: 1-minute load ${when}: ${load.toFixed(2)}` +
			(heavy.length > 0 ? ` (running: ${heavy.join(', ')})` : '')
	);
	if (load > threshold || heavy.length > 0) {
		console.error(
			`perf:ab: ⚠️  the machine is not quiet ${when} the run — wall-clock medians may be skewed`
		);
	}
}

function binary_for(binaries: Binaries, lang: Lang): SpawnProgram {
	const binary = binaries[driver_of(lang)];
	if (binary === null) throw new HarnessError(`${lang}: no ${driver_of(lang)} driver was built`);
	return binary.program;
}

interface SideSamples {
	parse: RowSamples;
	/** The second phase's samples: format, or the strip's walk + blank. */
	work: RowSamples;
	/** Cycles per iteration per round, fixed cost removed, for `parse` then the work (`phase_instructions`
	 * over the cycle counts); empty when a run counted no cycles. */
	parse_cycles: Array<number>;
	work_cycles: Array<number>;
	/** The first full run's file set; every later full run must match it. */
	file_set: FileSet | null;
	/** The first full run's formatted bytes; every later full run must match it. */
	output_bytes: number | null;
}

function empty_samples(): SideSamples {
	return {
		parse: { wall_us: [], instructions: [] },
		work: { wall_us: [], instructions: [] },
		parse_cycles: [],
		work_cycles: [],
		file_set: null,
		output_bytes: null
	};
}

/** Measure one round of `langs`, `first` side first. */
async function run_round(
	round: number,
	langs: ReadonlyArray<Lang>,
	binaries: ReadonlyArray<Binaries>,
	sides: ReadonlyArray<Side>,
	samples: ReadonlyArray<Record<Lang, SideSamples>>,
	runner: DriverRunner,
	use_perf: boolean
): Promise<void> {
	const { options } = runner;
	const order = round % 2 === 0 ? [0, 1] : [1, 0];
	for (const lang of langs) {
		console.error(`perf:ab: round ${round + 1} ${lang}`);
		for (const i of order) {
			const binary = binary_for(binaries[i]!, lang);
			const full = await run_driver(runner, binary, lang, 'full', use_perf);
			const s = samples[i]![lang];
			if (s.file_set === null) {
				s.file_set = { files: full.files, bytes: full.bytes, errors: full.errors };
				s.output_bytes = full.output_bytes;
			} else {
				const mismatch = file_set_mismatch(s.file_set, full);
				if (mismatch !== null) {
					throw new HarnessError(
						`${sides[i]!.label} ${lang}: one run measured ${mismatch} — the corpus moved`
					);
				}
				if (full.output_bytes !== s.output_bytes) {
					throw new HarnessError(
						`${sides[i]!.label} ${lang}: the output changed between runs ` +
							`(${s.output_bytes} bytes, then ${full.output_bytes}) — a nondeterministic ${lang === 'strip' ? 'strip' : 'format'}`
					);
				}
			}
			s.parse.wall_us.push(full.parse_us);
			s.work.wall_us.push(full.work_us);
			if (use_perf) {
				const parse = await run_driver(runner, binary, lang, 'parse', use_perf);
				const fixed = await run_driver(runner, binary, lang, 'fixed', use_perf);
				const [parse_instructions, work_instructions] = phase_instructions(
					lang,
					{ full: full.instructions!, parse: parse.instructions!, fixed: fixed.instructions! },
					options.iterations
				);
				s.parse.instructions.push(parse_instructions);
				s.work.instructions.push(work_instructions);
				if (full.cycles !== null && parse.cycles !== null && fixed.cycles !== null) {
					const [parse_cycles, work_cycles] = phase_instructions(
						lang,
						{ full: full.cycles, parse: parse.cycles, fixed: fixed.cycles },
						options.iterations
					);
					s.parse_cycles.push(parse_cycles);
					s.work_cycles.push(work_cycles);
				}
			}
		}
	}
}

/** Each row's median cycles per iteration on each side, keyed `<lang> <phase>`, for the JSON
 * output only: the verdict reads wall and instructions, never cycles. */
function cycles_by_row(
	langs: ReadonlyArray<Lang>,
	samples: ReadonlyArray<Record<Lang, SideSamples>>
): Record<string, { base: number; cand: number }> {
	const median = (xs: ReadonlyArray<number>): number => {
		const sorted = [...xs].sort((a, b) => a - b);
		const mid = Math.floor(sorted.length / 2);
		return sorted.length % 2 ? sorted[mid]! : (sorted[mid - 1]! + sorted[mid]!) / 2;
	};
	const out: Record<string, { base: number; cand: number }> = {};
	for (const lang of langs) {
		const [parse_phase, work_phase] = LANG_PHASES[lang];
		for (const [phase, key] of [
			[parse_phase, 'parse_cycles'],
			[work_phase, 'work_cycles']
		] as const) {
			const b = samples[0]![lang][key];
			const c = samples[1]![lang][key];
			if (b.length > 0 && c.length > 0)
				out[`${lang} ${phase}`] = { base: median(b), cand: median(c) };
		}
	}
	return out;
}

function grade(
	langs: ReadonlyArray<Lang>,
	samples: ReadonlyArray<Record<Lang, SideSamples>>
): Array<AbRow> {
	const rows: Array<AbRow> = [];
	for (const lang of langs) {
		const [b, c] = [samples[0]![lang], samples[1]![lang]];
		const [parse_phase, work_phase] = LANG_PHASES[lang];
		for (const [phase, key] of [
			[parse_phase, 'parse'],
			[work_phase, 'work']
		] as const) {
			try {
				rows.push(ab_row(lang, phase, b[key], c[key]));
			} catch (err) {
				throw new HarnessError(`${lang} ${phase}: ${err instanceof Error ? err.message : err}`);
			}
		}
	}
	return rows;
}

/** Whether something is at `path`, absolute, its symlinks followed. */
async function path_exists(fs_paths: FsPaths, path: string): Promise<boolean> {
	try {
		const opened = await fs_paths.open_path(path);
		return await opened.dir.exists(opened.path);
	} catch (err) {
		if (is_fs_error(err, 'not_a_directory')) return false;
		throw err;
	}
}

/**
 * Runs `perf:ab` with `args`, the arguments after the script, printing the report on stdout
 * and the progress on stderr.
 * @returns the exit code: 0 every row passes, 1 a row regressed, 2 a harness error
 */
export async function run_perf_ab_command(
	deps: PerfAbDeps,
	args: ReadonlyArray<string>
): Promise<number> {
	try {
		return await perf_ab(deps, args);
	} catch (err) {
		if (err instanceof HarnessError) {
			console.error(`perf:ab: error: ${err.message}`);
			if (err.message.startsWith('expected') || err.message.startsWith('unknown flag')) {
				console.error(USAGE);
			}
			return 2;
		}
		console.error(err);
		return 2;
	}
}

async function perf_ab(deps: PerfAbDeps, args: ReadonlyArray<string>): Promise<number> {
	const options = parse_options(args, deps.checkout.path);
	if (options === 'help') {
		console.log(USAGE);
		return 0;
	}
	if (!(await path_exists(deps.fs_paths, options.corpus))) {
		throw new HarnessError(
			`corpus not found: ${options.corpus} — check out fuzdev/corpora beside this repo, or pass --corpus`
		);
	}
	const drivers = {} as Record<DriverKind, DriverBuild>;
	for (const kind of DRIVER_KINDS) {
		const source = await deps.checkout.read_if_exists(DRIVERS[kind].path);
		if (source === null) {
			throw new HarnessError(`driver missing: ${resolve(deps.checkout.path, DRIVERS[kind].path)}`);
		}
		drivers[kind] = {
			kind,
			source,
			hash: await sha256_hex(source),
			recipe_hash: await sha256_hex(
				new TextEncoder().encode(
					build_recipe_text(
						[BUILD_PROGRAM, ...build_args(DRIVERS[kind].example)],
						BUILD_FIXED_ENV,
						BUILD_ENV
					)
				)
			)
		};
	}

	const home = deps.home;
	if (!home) throw new HarnessError('HOME is not set');
	const cache = await deps.open_cache(options.cache ?? resolve(home, '.cache/tsv/perf_ab'));

	const load_before = deps.load_1m();
	const perf_problem = await probe_perf(deps.perf);
	const use_perf = perf_problem === null;
	if (!use_perf) {
		console.error(
			`perf:ab: ⚠️ ⚠️  NO INSTRUCTION COUNTS — ${perf_problem}.\n` +
				`perf:ab: ⚠️ ⚠️  Grading wall-clock ALONE: no layout escape and no instruction gate, so ` +
				`any row over ${WALL_REGRESSION_PCT}% fails and added work under it passes. Install \`perf\` (Debian: linux-perf) and allow ` +
				`kernel.perf_event_paranoid <= 2 for the full verdict.`
		);
	}

	const base = await resolve_ref(deps, options.base_ref);
	const cand =
		options.cand_ref === null
			? await resolve_worktree(deps, cache.dir)
			: await resolve_ref(deps, options.cand_ref);
	const sides = [base, cand] as const;
	await evict_cache(cache.dir, sides);
	if (base.key === cand.key && base.tree !== cand.tree) {
		console.error(
			`perf:ab: the sides differ only outside the build inputs — one binary, measured against itself (an A/A)`
		);
	}
	const strip_skip = strip_skip_reason(base.has_strip, cand.has_strip);
	const strip_skip_line =
		strip_skip === null
			? null
			: `perf:ab: ⚠️ ⚠️  STRIP ROWS SKIPPED — ${strip_skip}; the strip driver cannot build ` +
				`against such a tree, so this run grades the format rows alone`;
	if (strip_skip_line !== null) console.error(strip_skip_line);
	const binaries: Array<Binaries> = [];
	for (const side of sides) {
		binaries.push(await build_side(deps, cache, side, drivers, strip_skip === null));
	}
	const sha256_of = async (binary: BuiltDriver | null): Promise<string | null> =>
		binary === null ? null : sha256_hex(await cache.dir.read(binary.name));
	const binary_sha256 = await Promise.all(
		binaries.map(async (b) => ({
			format: await sha256_of(b.format),
			strip: await sha256_of(b.strip)
		}))
	);
	const runner: DriverRunner = {
		checkout: deps.checkout,
		perf: deps.perf,
		cache: cache.dir,
		pid: deps.pid,
		options
	};

	// Which languages the corpus holds, per side, from a zero-iteration run (no timing).
	const langs: Array<Lang> = [];
	const absent: Array<Lang> = [];
	for (const lang of LANGS) {
		if (lang === 'strip' && strip_skip !== null) continue;
		const [b, c] = await Promise.all(
			binaries.map((b) => run_driver(runner, binary_for(b, lang), lang, 'fixed', false))
		);
		const presence = lang_presence(b!.files, c!.files);
		if (presence === 'one_sided') {
			throw new HarnessError(
				`${lang}: ${b!.files} files on the base side, ${c!.files} on the candidate — the sides read different corpora`
			);
		}
		(presence === 'absent' ? absent : langs).push(lang);
	}
	if (langs.length === 0)
		throw new HarnessError(`no ts, svelte or css files under ${options.corpus}`);

	for (const lang of absent) {
		console.error(`perf:ab: ${lang}: no files in the corpus — not measured, no rows`);
	}

	// Read before building, so this run's own build is not what it reports; the heavy-process
	// check runs now, when that build has exited.
	await report_load(deps.pgrep, 'before', load_before, LOAD_WARN);
	const samples: Array<Record<Lang, SideSamples>> = sides.map(() => ({
		ts: empty_samples(),
		svelte: empty_samples(),
		css: empty_samples(),
		strip: empty_samples()
	}));
	let rounds = 0;
	while (rounds < options.rounds) {
		await run_round(rounds++, langs, binaries, sides, samples, runner, use_perf);
	}
	let rows = grade(langs, samples);
	while (wants_more_rounds(rows, rounds)) {
		const ambiguous = rows
			.filter((row) => row_is_ambiguous(row) && row_is_settleable(row))
			.map((row) => `${row.lang} ${row.phase}`);
		console.error(
			`perf:ab: ${ambiguous.join(', ')} within its error of the ${WALL_REGRESSION_PCT}% line — two more rounds`
		);
		await run_round(rounds++, langs, binaries, sides, samples, runner, use_perf);
		await run_round(rounds++, langs, binaries, sides, samples, runner, use_perf);
		rows = grade(langs, samples);
	}
	const unsettled = unsettled_rows(rows);
	for (const row of unsettled) {
		console.error(
			`perf:ab: ${row.lang} ${row.phase}: ${format_pct(row.wall_pct)} sits within its error ` +
				`(±${wall_delta_error(row).toFixed(3)} points) of the ${WALL_REGRESSION_PCT}% line, ` +
				`which rounds up to the ${MAX_ROUNDS}-round cap could not narrow enough — graded on the point estimate`
		);
	}
	const load_after = deps.load_1m();
	await report_load(deps.pgrep, 'after', load_after, LOAD_WARN_AFTER);

	for (const lang of langs) {
		const [b, c] = [samples[0]![lang].file_set!, samples[1]![lang].file_set!];
		const mismatch = file_set_mismatch(b, c);
		if (mismatch !== null) {
			throw new HarnessError(
				`${lang}: the sides measured different file sets (base, then cand: ${mismatch}) — ` +
					`a medians sum over different files is not an A/B`
			);
		}
		if (b.errors > 0) {
			console.error(
				`perf:ab: ⚠️  ${lang}: ${b.errors} files failed to parse on both sides (left out)`
			);
		}
	}
	const noisy = noisy_rows(rows);
	for (const row of noisy) {
		console.error(
			`perf:ab: ⚠️  ${row.lang} ${row.phase}: a side's round spread exceeds ${NOISY_SPREAD_PCT}% ` +
				`(base ±${row.base_wall_spread_pct.toFixed(2)}%, cand ±${row.cand_wall_spread_pct.toFixed(2)}%) — ` +
				`the machine was noisy; re-run it quiet before trusting the wall column`
		);
	}
	const passes = overall_passes(rows);
	const sign_offs = sign_off_rows(rows);
	const layout = layout_reading(rows);
	const layout_line =
		layout === null
			? null
			: `perf:ab: layout reading: ${layout.lang} ${layout.phase} moved ${format_pct(layout.wall_pct)} ` +
				`on flat instructions (${format_pct(layout.instructions_pct!)}) — the wall move code ` +
				`placement alone gave these two builds`;
	const sign_off_line = (row: AbRow): string =>
		`perf:ab: ${row.lang} ${row.phase}: FAIL on wall (${format_pct(row.wall_pct)}) with instructions ` +
		`inside the WARN band (${format_pct(row.instructions_pct!)}), the wall ` +
		`${(row.wall_pct - row.instructions_pct!).toFixed(3)} points beyond them — SIGN-OFF: the ` +
		`instruction rise is the real cost; accept it or fix it. A re-run re-rolls only the wall, so ` +
		`it cannot clear this (docs/performance.md §Gating a change against a base)`;

	if (options.json) {
		if (layout_line !== null) console.error(layout_line);
		for (const row of sign_offs) console.error(sign_off_line(row));
		console.log(
			JSON.stringify(
				{
					base: { ...base, binary_sha256: binary_sha256[0] },
					cand: { ...cand, binary_sha256: binary_sha256[1] },
					strip_skipped: strip_skip,
					corpus: options.corpus,
					rounds,
					iterations: options.iterations,
					instructions: use_perf,
					thresholds: {
						wall_pct: WALL_REGRESSION_PCT,
						instructions_layout_pct: INSTRUCTIONS_LAYOUT_PCT,
						instructions_regression_pct: INSTRUCTIONS_REGRESSION_PCT,
						sign_off_excess_pct: SIGN_OFF_EXCESS_PCT,
						noisy_spread_pct: NOISY_SPREAD_PCT
					},
					load_before,
					load_after,
					files: Object.fromEntries(langs.map((lang) => [lang, samples[0]![lang].file_set])),
					absent,
					noisy: noisy.map((row) => `${row.lang} ${row.phase}`),
					unsettled: unsettled.map((row) => `${row.lang} ${row.phase}`),
					sign_off: sign_offs.map((row) => `${row.lang} ${row.phase}`),
					layout_reading:
						layout === null
							? null
							: { row: `${layout.lang} ${layout.phase}`, wall_pct: layout.wall_pct },
					cycles: cycles_by_row(langs, samples),
					rows,
					passes
				},
				null,
				'\t'
			)
		);
	} else {
		const sha_text = (sha: { format: string | null; strip: string | null }): string =>
			`binary sha256 ${sha.format}` + (sha.strip === null ? '' : `, strip ${sha.strip}`);
		console.log(
			`base: ${base.label}, ${sha_text(binary_sha256[0]!)}\n` +
				`cand: ${cand.label}, ${sha_text(binary_sha256[1]!)}`
		);
		console.log(
			`corpus: ${options.corpus} (${langs.map((lang) => `${lang} ${samples[0]![lang].file_set!.files}`).join(', ')} files` +
				(absent.length > 0 ? `; no ${absent.join(', ')} files` : '') +
				')'
		);
		console.log(
			`${rounds} interleaved rounds${rounds > options.rounds ? ` (${options.rounds} + ${rounds - options.rounds} for an ambiguous row)` : ''} × ` +
				`--iterations ${options.iterations}. wall = median across rounds, ± = its median absolute deviation; ` +
				`Δ wall = median of the per-round paired deltas, ± = their median absolute deviation (points). ` +
				`instr = instructions:u per iteration, fixed cost removed.\n`
		);
		console.log(render_table(rows));
		console.log(
			`\nrule: Δ wall > ${WALL_REGRESSION_PCT}% fails unless |Δ instr| < ${INSTRUCTIONS_LAYOUT_PCT}% ` +
				`(LAYOUT); Δ instr > ${INSTRUCTIONS_REGRESSION_PCT}% fails whatever the wall; ` +
				`Δ instr ≥ ${INSTRUCTIONS_LAYOUT_PCT}% under a passing wall warns; a wall FAIL with Δ instr ` +
				`in that band and Δ wall − Δ instr ≤ ${SIGN_OFF_EXCESS_PCT} points is a SIGN-OFF` +
				(use_perf ? '' : ' — NO INSTRUCTION COUNTS THIS RUN: wall alone')
		);
		if (layout_line !== null) console.log(layout_line);
		for (const row of sign_offs) console.log(sign_off_line(row));
		if (strip_skip_line !== null) console.log(strip_skip_line);
		console.log(passes ? 'perf:ab: PASS' : 'perf:ab: FAIL — a row regressed');
	}
	return passes ? 0 : 1;
}
