/**
 * `deno task perf:ab <base-ref> [<cand-ref>]` — a parse / format performance A/B between two
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
 * The builds run in a cleared environment (`BUILD_ENV`): a caller's `RUSTFLAGS`,
 * `CARGO_PROFILE_*`, `RUSTUP_TOOLCHAIN` or `RUSTC_WRAPPER` would otherwise reach one side's
 * build and not the other's — a cached base built under flags the candidate never saw. Clearing
 * rather than hashing them into the cache key, because the gate measures the shipped release
 * profile and nothing else; a flag experiment is a manual `profile` A/B.
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
 * Linux only (`perf`, `ionice`). Not in `deno task check`: it builds two release trees.
 *
 * @module
 */

import { existsSync } from 'node:fs';
import { copyFile, mkdir, readdir, readFile, rename, rm, stat, utimes } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { CORPORA_COLLECTIONS } from '../benches/js/lib/corpora.ts';
import { lines_of } from '../benches/js/lib/text_lines.ts';
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

const ROOT = fileURLToPath(new URL('..', import.meta.url));
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

/** The text whose presence in a tree's `crates/tsv_ts/src` says the strip driver can build
 * against it (`strip_skip_reason`). */
const STRIP_ENTRY_POINT = 'pub fn strip_in<';

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

/** The variables a driver build keeps from the caller; everything else is cleared. Locating
 * the toolchain needs `PATH`, `HOME` and the rustup / cargo homes; `CARGO_BUILD_JOBS` only
 * schedules the build (release codegen is one unit, so the artifact does not depend on it). */
const BUILD_ENV = ['PATH', 'HOME', 'CARGO_HOME', 'RUSTUP_HOME', 'CARGO_BUILD_JOBS'];

/** The variables a driver build sets itself, the same for every snapshot. `CARGO_TARGET_DIR` is
 * relative to the build's working directory (the snapshot's `src/`), so each snapshot builds into
 * its own `target/` beside it. Part of the build RECIPE (`build_recipe_text`). */
const BUILD_FIXED_ENV: Readonly<Record<string, string>> = { CARGO_TARGET_DIR: '../target' };

/** A driver build's command line, run in the snapshot at idle priority. With `BUILD_FIXED_ENV`
 * and `BUILD_ENV` it is the build RECIPE, whose hash names the binary (`build_recipe_text`). */
function build_command(example: string): Array<string> {
	return [
		'nice',
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

/** A 1-minute load average above this before the run warns: something else holds a core. */
const LOAD_WARN = 1;
/** The run itself keeps one core busy, so the after-reading is allowed one more. */
const LOAD_WARN_AFTER = LOAD_WARN + 1;

/** Processes whose presence means a build or a corpus run competes for the machine. */
const HEAVY_PROCESSES = ['rustc', 'cargo', 'tsv_debug'];

class HarnessError extends Error {}

interface Options {
	base_ref: string;
	cand_ref: string | null;
	rounds: number;
	iterations: number;
	corpus: string;
	json: boolean;
}

const USAGE = `usage: deno task perf:ab <base-ref> [<cand-ref>] [--rounds N] [--iterations N] [--corpus DIR] [--json]
  <cand-ref> defaults to the working tree (HEAD + uncommitted changes)
  --rounds N      interleaved base/cand rounds, even (default ${DEFAULT_ROUNDS}; an ambiguous row
                  extends the run by pairs up to ${MAX_ROUNDS})
  --iterations N  iterations per file per run (default ${DEFAULT_ITERATIONS})
  --corpus DIR    corpus directory (default ${CORPORA_COLLECTIONS})
  --json          print the result as JSON instead of the table`;

function parse_options(args: ReadonlyArray<string>): Options {
	const positional: Array<string> = [];
	let rounds = DEFAULT_ROUNDS;
	let iterations = DEFAULT_ITERATIONS;
	let corpus = CORPORA_COLLECTIONS;
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
		} else if (arg === '--json') json = true;
		else if (arg === '--help' || arg === '-h') {
			console.log(USAGE);
			Deno.exit(0);
		} else if (arg.startsWith('-')) throw new HarnessError(`unknown flag ${arg}`);
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
		corpus: resolve(ROOT, corpus),
		json
	};
}

interface RunResult {
	success: boolean;
	code: number;
	stdout: string;
	stderr: string;
}

async function run(
	cmd: string,
	args: ReadonlyArray<string>,
	options: { cwd?: string; env?: Record<string, string> } = {}
): Promise<RunResult> {
	const output = await new Deno.Command(cmd, {
		args: [...args],
		cwd: options.cwd ?? ROOT,
		env: options.env,
		stdout: 'piped',
		stderr: 'piped'
	}).output();
	const decoder = new TextDecoder();
	return {
		success: output.success,
		code: output.code,
		stdout: decoder.decode(output.stdout),
		stderr: decoder.decode(output.stderr)
	};
}

async function git(args: ReadonlyArray<string>, env?: Record<string, string>): Promise<string> {
	const result = await run('git', args, { env });
	if (!result.success) {
		throw new HarnessError(`git ${args.join(' ')} failed:\n${result.stderr.trim()}`);
	}
	return result.stdout.trim();
}

/** One measured side: what it is called, the tree it builds, and that tree's build inputs. */
interface Side {
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

/** The recursive, NUL-separated `git ls-tree -r -z` listing of `paths` in `tree`, run in `cwd`
 * (untrimmed: a raw path may end in whitespace). */
export async function list_tree(
	tree: string,
	paths: ReadonlyArray<string>,
	cwd: string = ROOT
): Promise<string> {
	const args = ['ls-tree', '-r', '-z', tree, '--', ...paths];
	const result = await run('git', args, { cwd });
	if (!result.success) {
		throw new HarnessError(`git ${args.join(' ')} failed:\n${result.stderr.trim()}`);
	}
	return result.stdout;
}

/** The cache key and the present build inputs of `tree`, from one recursive `git ls-tree`
 * listing of `BUILD_INPUTS` (the drivers left out: the overlay replaces them), and whether it
 * has the strip. */
async function build_inputs(
	tree: string
): Promise<{ key: string; inputs: Array<string>; has_strip: boolean }> {
	const listing = await list_tree(tree, BUILD_INPUTS);
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
		has_strip: await tree_has_strip(tree)
	};
}

/** Whether `tree`'s `crates/tsv_ts/src` has `STRIP_ENTRY_POINT` (`git grep`: exit 1 is "no",
 * anything but 0 or 1 an error). */
async function tree_has_strip(tree: string): Promise<boolean> {
	const args = ['grep', '-q', '-F', STRIP_ENTRY_POINT, tree, '--', 'crates/tsv_ts/src'];
	const result = await run('git', args);
	if (result.code === 0) return true;
	if (result.code === 1) return false;
	throw new HarnessError(`git ${args.join(' ')} failed:\n${result.stderr.trim()}`);
}

async function resolve_ref(ref: string): Promise<Side> {
	const commit = await git(['rev-parse', '--verify', '--quiet', `${ref}^{commit}`]).catch(() => {
		throw new HarnessError(`not a commit: ${ref}`);
	});
	const tree = await git(['rev-parse', `${commit}^{tree}`]);
	return { label: `${ref} (${commit.slice(0, 9)})`, commit, tree, ...(await build_inputs(tree)) };
}

/** Snapshot the working tree as a git tree object through a throwaway copy of the index. */
async function resolve_worktree(cache: string): Promise<Side> {
	const index = resolve(ROOT, await git(['rev-parse', '--git-path', 'index']));
	const scratch = resolve(cache, `index.${Deno.pid}`);
	await copyFile(index, scratch);
	try {
		const env = { GIT_INDEX_FILE: scratch };
		await git(['add', '-A'], env);
		const tree = await git(['write-tree'], env);
		const head = await git(['rev-parse', 'HEAD']);
		const head_tree = await git(['rev-parse', 'HEAD^{tree}']);
		const label =
			tree === head_tree
				? `worktree (= HEAD ${head.slice(0, 9)})`
				: `worktree (HEAD ${head.slice(0, 9)} + changes)`;
		return { label, commit: null, tree, ...(await build_inputs(tree)) };
	} finally {
		await rm(scratch, { force: true });
	}
}

/** Keep the snapshots this run measures plus the most recently used others (`KEEP_TREES` in
 * all); delete the rest, and mark the measured ones used now. */
async function evict_cache(cache: string, sides: ReadonlyArray<Side>): Promise<void> {
	const in_use = new Set(sides.map((side) => side.key));
	const now = new Date();
	for (const tree of in_use) {
		const dir = resolve(cache, tree);
		if (existsSync(dir)) await utimes(dir, now, now);
	}
	const trees: Array<CachedTree> = [];
	for (const entry of await readdir(cache, { withFileTypes: true })) {
		if (!entry.isDirectory() || !CACHE_DIR_PATTERN.test(entry.name)) continue;
		trees.push({ name: entry.name, mtime_ms: (await stat(resolve(cache, entry.name))).mtimeMs });
	}
	for (const name of trees_to_evict(trees, in_use)) {
		console.error(`perf:ab: evicting cached snapshot ${name.slice(0, 12)}`);
		await rm(resolve(cache, name), { recursive: true, force: true });
	}
}

async function sha256_hex(bytes: Uint8Array<ArrayBuffer>): Promise<string> {
	const digest = await crypto.subtle.digest('SHA-256', bytes);
	return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, '0')).join('');
}

/** Extract `side`'s build inputs into `dir` with `git archive | tar -x`, via a per-process
 * partial directory so an interrupted extraction is never mistaken for a snapshot and two
 * concurrent runs never share one. */
async function extract_tree(side: Side, dir: string): Promise<void> {
	const { tree, inputs: paths } = side;
	const partial = `${dir}.partial.${Deno.pid}`;
	await rm(partial, { recursive: true, force: true });
	await mkdir(partial, { recursive: true });
	const archive = new Deno.Command('git', {
		args: ['archive', '--format=tar', tree, '--', ...paths],
		cwd: ROOT,
		stdout: 'piped',
		stderr: 'piped'
	}).spawn();
	const tar = new Deno.Command('tar', {
		args: ['-x', '-C', partial],
		stdin: 'piped',
		stdout: 'null',
		stderr: 'piped'
	}).spawn();
	await archive.stdout.pipeTo(tar.stdin);
	const [archive_out, tar_out] = await Promise.all([archive.output(), tar.output()]);
	if (!archive_out.success || !tar_out.success) {
		await rm(partial, { recursive: true, force: true });
		const stderr = new TextDecoder().decode(
			archive_out.success ? tar_out.stderr : archive_out.stderr
		);
		throw new HarnessError(`extracting tree ${tree} failed:\n${stderr.trim()}`);
	}
	try {
		await rename(partial, dir);
	} catch (err) {
		// a concurrent run finished the same extraction first; its copy is the same tree
		await rm(partial, { recursive: true, force: true });
		if (!existsSync(dir)) throw err;
	}
}

/** `BUILD_FIXED_ENV` plus the caller's values for `BUILD_ENV`, the whole environment of a driver
 * build. */
function build_env(): Record<string, string> {
	const env: Record<string, string> = { ...BUILD_FIXED_ENV };
	for (const name of BUILD_ENV) {
		const value = Deno.env.get(name);
		if (value !== undefined) env[name] = value;
	}
	return env;
}

/** One driver as the run builds it: its source, the source's hash and its recipe's hash. */
interface DriverBuild {
	kind: DriverKind;
	source: Uint8Array;
	hash: string;
	recipe_hash: string;
}

/** Build (or reuse) `driver` for `side`; returns the binary's path. */
async function ensure_driver(side: Side, cache: string, driver: DriverBuild): Promise<string> {
	const { path: driver_path, example } = DRIVERS[driver.kind];
	const base = resolve(cache, side.key);
	const binary = resolve(
		base,
		`${example}-${driver.hash.slice(0, 16)}-${driver.recipe_hash.slice(0, 16)}`
	);
	if (existsSync(binary)) {
		console.error(`perf:ab: ${side.label}: reusing ${binary}`);
		return binary;
	}
	const src = resolve(base, 'src');
	if (!existsSync(src)) {
		console.error(
			`perf:ab: ${side.label}: snapshotting tree ${side.tree.slice(0, 12)} (build inputs ${side.key.slice(0, 12)})`
		);
		await extract_tree(side, src);
	}
	const overlay = resolve(src, driver_path);
	await mkdir(dirname(overlay), { recursive: true });
	await Deno.writeFile(overlay, driver.source);

	console.error(`perf:ab: ${side.label}: building the ${example} driver (release, idle priority)`);
	const [command, ...args] = build_command(example);
	const child = new Deno.Command(command!, {
		args,
		cwd: src,
		clearEnv: true,
		env: build_env(),
		stdout: 'piped',
		stderr: 'inherit'
	}).spawn();
	let executable: string | null = null;
	for await (const line of lines_of(child.stdout)) {
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
	const status = await child.status;
	if (!status.success || executable === null) {
		throw new HarnessError(
			`building the ${example} driver for ${side.label} failed (${status.success ? 'no executable reported' : `cargo exited ${status.code}`}). ` +
				`The driver compiles against each tree's language-crate entry points; a tree older than ` +
				`those entry points cannot be measured with it.`
		);
	}
	// Copied out of the target dir so a later build for another driver hash cannot replace it,
	// under a temporary name renamed into place so an interrupted copy is never reused.
	const partial = `${binary}.partial.${Deno.pid}`;
	await copyFile(executable, partial);
	await Deno.chmod(partial, 0o755);
	await rename(partial, binary);
	return binary;
}

/** Whether `perf stat -e instructions:u` works here; the reason when it does not. */
async function probe_perf(): Promise<string | null> {
	try {
		const result = await run('perf', ['stat', '-x,', '-e', 'instructions:u', '--', 'true']);
		if (!result.success) return `perf stat exited ${result.code}: ${result.stderr.trim()}`;
		if (parse_perf_stat_instructions(result.stderr) === null) {
			return `perf stat counted no instructions:u (${result.stderr.trim() || 'no output'})`;
		}
		return null;
	} catch (err) {
		if (err instanceof Deno.errors.NotFound) return '`perf` is not installed';
		throw err;
	}
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
}

type DriverMode = 'full' | 'parse' | 'fixed';

async function run_driver(
	binary: string,
	cache: string,
	options: Options,
	lang: Lang,
	mode: DriverMode,
	use_perf: boolean
): Promise<DriverOutput> {
	const iterations = mode === 'fixed' ? 0 : options.iterations;
	const driver_args =
		lang === 'strip'
			? [options.corpus, '--iterations', String(iterations)]
			: [options.corpus, '--lang', lang, '--iterations', String(iterations)];
	if (mode === 'parse') driver_args.push('--parse-only');
	const stat_file = resolve(cache, `perf_stat.${Deno.pid}.csv`);
	const result = use_perf
		? await run('perf', [
				'stat',
				'-x,',
				'-e',
				'instructions:u',
				'-o',
				stat_file,
				'--',
				binary,
				...driver_args
			])
		: await run(binary, driver_args);
	if (!result.success) {
		throw new HarnessError(
			`driver ${binary} ${driver_args.join(' ')} exited ${result.code}:\n${result.stderr.trim()}`
		);
	}
	let instructions: number | null = null;
	if (use_perf) {
		const stat_text = await readFile(stat_file, 'utf8');
		await rm(stat_file, { force: true });
		instructions = parse_perf_stat_instructions(stat_text);
		if (instructions === null) throw new HarnessError(`perf stat counted nothing:\n${stat_text}`);
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
		instructions
	};
}

function load_1m(): number {
	return Deno.loadavg()[0]!;
}

/** The names among `HEAVY_PROCESSES` running right now (one `pgrep` each; empty when `pgrep`
 * itself is missing). */
async function heavy_processes(): Promise<Array<string>> {
	const running: Array<string> = [];
	for (const name of HEAVY_PROCESSES) {
		try {
			if ((await run('pgrep', ['-x', name])).success) running.push(name);
		} catch {
			return running;
		}
	}
	return running;
}

/** Print a load reading plus the heavy processes running now, warning over `threshold`. */
async function report_load(
	when: 'before' | 'after',
	load: number,
	threshold: number
): Promise<void> {
	const heavy = await heavy_processes();
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

/** One side's built drivers; `strip` is null when the strip rows are skipped. */
type Binaries = Record<DriverKind, string | null>;

function binary_for(binaries: Binaries, lang: Lang): string {
	const binary = binaries[driver_of(lang)];
	if (binary === null) throw new HarnessError(`${lang}: no ${driver_of(lang)} driver was built`);
	return binary;
}

interface SideSamples {
	parse: RowSamples;
	/** The second phase's samples: format, or the strip's walk + blank. */
	work: RowSamples;
	/** The first full run's file set; every later full run must match it. */
	file_set: FileSet | null;
	/** The first full run's formatted bytes; every later full run must match it. */
	output_bytes: number | null;
}

function empty_samples(): SideSamples {
	return {
		parse: { wall_us: [], instructions: [] },
		work: { wall_us: [], instructions: [] },
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
	cache: string,
	options: Options,
	use_perf: boolean
): Promise<void> {
	const order = round % 2 === 0 ? [0, 1] : [1, 0];
	for (const lang of langs) {
		console.error(`perf:ab: round ${round + 1} ${lang}`);
		for (const i of order) {
			const binary = binary_for(binaries[i]!, lang);
			const full = await run_driver(binary, cache, options, lang, 'full', use_perf);
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
				const parse = await run_driver(binary, cache, options, lang, 'parse', use_perf);
				const fixed = await run_driver(binary, cache, options, lang, 'fixed', use_perf);
				const [parse_instructions, work_instructions] = phase_instructions(
					lang,
					{ full: full.instructions!, parse: parse.instructions!, fixed: fixed.instructions! },
					options.iterations
				);
				s.parse.instructions.push(parse_instructions);
				s.work.instructions.push(work_instructions);
			}
		}
	}
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

async function main(): Promise<number> {
	const options = parse_options(Deno.args);
	if (!existsSync(options.corpus)) {
		throw new HarnessError(
			`corpus not found: ${options.corpus} — check out fuzdev/corpora beside this repo, or pass --corpus`
		);
	}
	const drivers = {} as Record<DriverKind, DriverBuild>;
	for (const kind of DRIVER_KINDS) {
		const driver_path = resolve(ROOT, DRIVERS[kind].path);
		if (!existsSync(driver_path)) throw new HarnessError(`driver missing: ${driver_path}`);
		const source = await Deno.readFile(driver_path);
		drivers[kind] = {
			kind,
			source,
			hash: await sha256_hex(source),
			recipe_hash: await sha256_hex(
				new TextEncoder().encode(
					build_recipe_text(build_command(DRIVERS[kind].example), BUILD_FIXED_ENV, BUILD_ENV)
				)
			)
		};
	}

	const home = Deno.env.get('HOME');
	if (!home) throw new HarnessError('HOME is not set');
	const cache = resolve(home, '.cache/tsv/perf_ab');
	await mkdir(cache, { recursive: true });

	const load_before = load_1m();
	const perf_problem = await probe_perf();
	const use_perf = perf_problem === null;
	if (!use_perf) {
		console.error(
			`perf:ab: ⚠️ ⚠️  NO INSTRUCTION COUNTS — ${perf_problem}.\n` +
				`perf:ab: ⚠️ ⚠️  Grading wall-clock ALONE: no layout escape and no instruction gate, so ` +
				`any row over ${WALL_REGRESSION_PCT}% fails and added work under it passes. Install \`perf\` (Debian: linux-perf) and allow ` +
				`kernel.perf_event_paranoid <= 2 for the full verdict.`
		);
	}

	const base = await resolve_ref(options.base_ref);
	const cand =
		options.cand_ref === null ? await resolve_worktree(cache) : await resolve_ref(options.cand_ref);
	const sides = [base, cand] as const;
	await evict_cache(cache, sides);
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
		binaries.push({
			format: await ensure_driver(side, cache, drivers.format),
			strip: strip_skip === null ? await ensure_driver(side, cache, drivers.strip) : null
		});
	}
	const sha256_of = async (binary: string | null): Promise<string | null> =>
		binary === null ? null : sha256_hex(await Deno.readFile(binary));
	const binary_sha256 = await Promise.all(
		binaries.map(async (b) => ({
			format: await sha256_of(b.format),
			strip: await sha256_of(b.strip)
		}))
	);

	// Which languages the corpus holds, per side, from a zero-iteration run (no timing).
	const langs: Array<Lang> = [];
	const absent: Array<Lang> = [];
	for (const lang of LANGS) {
		if (lang === 'strip' && strip_skip !== null) continue;
		const [b, c] = await Promise.all(
			binaries.map((b) => run_driver(binary_for(b, lang), cache, options, lang, 'fixed', false))
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
	await report_load('before', load_before, LOAD_WARN);
	const samples: Array<Record<Lang, SideSamples>> = sides.map(() => ({
		ts: empty_samples(),
		svelte: empty_samples(),
		css: empty_samples(),
		strip: empty_samples()
	}));
	let rounds = 0;
	while (rounds < options.rounds) {
		await run_round(rounds++, langs, binaries, sides, samples, cache, options, use_perf);
	}
	let rows = grade(langs, samples);
	while (wants_more_rounds(rows, rounds)) {
		const ambiguous = rows
			.filter((row) => row_is_ambiguous(row) && row_is_settleable(row))
			.map((row) => `${row.lang} ${row.phase}`);
		console.error(
			`perf:ab: ${ambiguous.join(', ')} within its error of the ${WALL_REGRESSION_PCT}% line — two more rounds`
		);
		await run_round(rounds++, langs, binaries, sides, samples, cache, options, use_perf);
		await run_round(rounds++, langs, binaries, sides, samples, cache, options, use_perf);
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
	const load_after = load_1m();
	await report_load('after', load_after, LOAD_WARN_AFTER);

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

if (import.meta.main) {
	try {
		Deno.exit(await main());
	} catch (err) {
		if (err instanceof HarnessError) {
			console.error(`perf:ab: error: ${err.message}`);
			if (err.message.startsWith('expected') || err.message.startsWith('unknown flag')) {
				console.error(USAGE);
			}
			Deno.exit(2);
		}
		console.error(err);
		Deno.exit(2);
	}
}
