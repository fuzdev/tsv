/**
 * One side of a branch report: a ref resolved to a commit, its whole tree extracted from git
 * into its own directory under the cache (`<tree>-<slot>/src`), and one target directory beside
 * it. The two sides of a same-ref run are extracted and built apart on purpose: a difference
 * there is the build's own non-determinism, which a shared binary could never show.
 *
 * Also what every leg reaches the machine through (`ReportHost`): the cache, the build program,
 * and each tool, all built by the root (`../roots/branch_report.ts`) with the environment it
 * gets.
 *
 * @module
 */

import type { FsDir, FsReadDir } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import type { FsPaths } from '@fuzdev/gro/fs_paths.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import { type CachedTree, trees_to_evict } from '../perf_ab_verdict.ts';
import {
	exit_code_of,
	extract_tree,
	format_ending,
	git_out,
	HarnessError
} from '../git_snapshot.ts';

export type Slot = 'base' | 'cand';

/** The programs the legs run by name, each found on `PATH` (`time` at `/usr/bin/time`), `null`
 * when the machine lacks it. `perf_ab` is `node` with what the `perf:ab` child passes to its
 * own builds and git (`TOOL_PASS_ENV`). */
export const TOOL_NAMES = [
	'bun',
	'cargo',
	'deno',
	'grep',
	'gzip',
	'heaptrack',
	'heaptrack_print',
	'hostname',
	'nm',
	'node',
	'npm',
	'nproc',
	'objdump',
	'perf',
	'perf_ab',
	'rustc',
	'size',
	'time',
	'uname',
	'wasm-pack'
] as const;
export type ToolName = (typeof TOOL_NAMES)[number];

/** Each tool's program, or `null` when it is not installed. */
export type ReportTools = Readonly<Record<ToolName, SpawnProgram | null>>;

/** The cache a run builds both sides in, and how it runs what it built there. */
export interface ReportCache {
	/** The cache's directory: the sides, the scratch files, the reports and the `perf:ab` cache. */
	dir: FsDir;
	/** The built program at `name`, a path beneath `dir`, as a program, made once it is built. */
	side_program: (name: string) => SpawnProgram;
}

/** What the legs reach: the checkout, the paths from outside, the cache, and every program,
 * each built by the root with the environment it gets. */
export interface ReportHost {
	/** The checkout: the working directory of `git` and of every tool, the source of `git
	 * archive`, and the probe scripts' tree. */
	checkout: FsReadDir;
	/** Opens a path from outside: the corpus and its files, `--aa`, `--out`. */
	fs_paths: FsPaths;
	cache: ReportCache;
	/** `nice` with the build environment (`BUILD_ENV`) and `env` beside it. */
	build: (env: Readonly<Record<string, string>>) => SpawnProgram;
	git: SpawnProgram;
	tar: SpawnProgram;
	tools: ReportTools;
	/** Names the partial and scratch directories, so two concurrent runs never share one. */
	pid: number;
}

export interface Side {
	slot: Slot;
	ref: string;
	commit: string;
	tree: string;
	/** The side's directory name beneath the cache, `<tree>-<slot>`. */
	name: string;
	/** The side's directory under the cache, absolute. */
	dir: string;
	/** Its extracted source tree, absolute. */
	src: string;
	/** Its Cargo target directory, absolute. */
	target: string;
	/** Handles on `dir` and `src`. */
	dir_handle: FsDir;
	src_handle: FsDir;
	rustc: string;
	cargo: string;
}

/** A leg's command log and how it runs a command through it. */
export class CommandLog {
	readonly lines: Array<string> = [];

	/** Record a command line as a person would re-run it (a shell's quoting is not attempted). */
	record(line: string): void {
		this.lines.push(line);
	}
}

/** Thrown for a command a leg needs and the machine lacks: a `missing_tool` leg, never `failed`. */
export class MissingTool extends HarnessError {
	readonly tool: string;

	constructor(tool: string) {
		super(`\`${tool}\` is not installed`);
		this.tool = tool;
	}
}

/** Thrown when a side lacks a file a leg reads: an `unavailable_on_ref` leg. */
export class UnavailableOnRef extends HarnessError {}

/** How a missing tool is named: `time` is the one at `/usr/bin/time`. */
const tool_label = (name: ToolName): string => (name === 'time' ? '/usr/bin/time' : name);

/** The program for `name`; throws `MissingTool` when the machine lacks it. */
export function require_tool(host: ReportHost, name: ToolName): SpawnProgram {
	const program = host.tools[name];
	if (program === null) throw new MissingTool(tool_label(name));
	return program;
}

/** The directory at `path`, absolute or relative to the root's working directory, its symlinks
 * followed; a directory outside the project is its own handle. */
export async function open_dir_path(fs_paths: FsPaths, path: string): Promise<FsDir> {
	const opened = await fs_paths.open_dir_path(path);
	if (opened === null) throw new HarnessError(`no directory at ${path}`);
	return opened.path === '' ? opened.dir : opened.dir.dir(opened.path);
}

/** Whether something is at `path`, absolute, its symlinks followed. */
export async function path_exists(fs_paths: FsPaths, path: string): Promise<boolean> {
	try {
		const opened = await fs_paths.open_path(path);
		return await opened.dir.exists(opened.path);
	} catch (err) {
		if (is_fs_error(err, 'not_a_directory')) return false;
		throw err;
	}
}

/** The absolute path of `name`, a path beneath the cache. */
export const cache_path = (host: ReportHost, name: string): string =>
	`${host.cache.dir.path}/${name}`;

export const SIDE_DIR_PATTERN = /^[0-9a-f]{40}-(base|cand)$/;

/** A side's first line of `rustc -vV` / `cargo -V`, run in its tree so its toolchain pin applies. */
async function tool_version(program: SpawnProgram, flag: string, cwd: FsReadDir): Promise<string> {
	const result = await program.out([flag], { cwd });
	if (exit_code_of(result) !== 0) {
		throw new HarnessError(`${program.name} ${flag} in ${cwd.path} failed: ${result.stderr}`);
	}
	return result.stdout.trim().replaceAll('\n', ' ');
}

/** Resolve `ref`, extract its tree into `<cache>/<tree>-<slot>/src` (kept if already there). */
export async function prepare_side(host: ReportHost, ref: string, slot: Slot): Promise<Side> {
	const commit = await git_out(host.git, host.checkout, [
		'rev-parse',
		'--verify',
		'--quiet',
		`${ref}^{commit}`
	]).catch(() => {
		throw new HarnessError(`not a commit: ${ref}`);
	});
	const tree = await git_out(host.git, host.checkout, ['rev-parse', `${commit}^{tree}`]);
	const cache = host.cache.dir;
	const name = `${tree}-${slot}`;
	if (!(await cache.exists(`${name}/src/Cargo.toml`))) {
		await cache.mkdir(name, { recursive: true });
		console.error(`branch:report: ${slot} ${ref}: extracting tree ${tree.slice(0, 12)}`);
		await extract_tree(host, tree, [], cache.dir(name), 'src');
	}
	const dir_handle = cache.dir(name);
	const src_handle = dir_handle.dir('src');
	const [rustc, cargo] = await Promise.all([
		tool_version(require_tool(host, 'rustc'), '-vV', src_handle),
		tool_version(require_tool(host, 'cargo'), '-V', src_handle)
	]);
	const dir = cache_path(host, name);
	return {
		slot,
		ref,
		commit,
		tree,
		name,
		dir,
		src: `${dir}/src`,
		target: `${dir}/target`,
		dir_handle,
		src_handle,
		rustc,
		cargo
	};
}

/** How many side directories (`SIDE_DIR_PATTERN`) a cache keeps in all. */
export const KEEP_SIDES = 4;

/** Keep the sides this run uses and the most recently used others (`KEEP_SIDES` in all); delete
 * the rest. A side carries several gigabytes of build output, so a cache must not grow by run. */
export async function evict_sides(cache: FsDir, sides: ReadonlyArray<Side>): Promise<void> {
	const in_use = new Set(sides.map((side) => side.name));
	const now = Date.now();
	for (const name of in_use) await cache.set_times(name, { mtime_ms: now });
	const trees: Array<CachedTree> = [];
	for await (const entry of cache.list()) {
		if (entry.kind !== 'directory' || !SIDE_DIR_PATTERN.test(entry.name)) continue;
		trees.push({ name: entry.name, mtime_ms: (await cache.stat(entry.name))!.mtime_ms });
	}
	for (const name of trees_to_evict(trees, in_use, KEEP_SIDES)) {
		console.error(`branch:report: evicting cached side ${name.slice(0, 16)}`);
		await cache.remove(name, { recursive: true, missing_ok: true });
	}
}

/** `nice`'s own arguments before the command it runs at idle priority. */
export const NICE_ARGS = ['-n', '19', 'ionice', '-c', '3'];

/** What `run_build` takes beside the command. */
export interface RunBuildOptions {
	/** Variables beside the build environment (`BUILD_ENV`), over `CARGO_TARGET_DIR`. */
	env?: Readonly<Record<string, string>>;
	/** Where it runs; the side's tree by default. */
	cwd?: FsReadDir;
	/** Leave `CARGO_TARGET_DIR` unset: a task that reads `target/` by name (the PGO pipeline,
	 * the bench's stamped builds) needs cargo's default target dir, inside the extracted tree. */
	default_target_dir?: boolean;
}

/**
 * Run `argv` at idle priority in a side's tree, through `nice` in the build environment, its
 * output on this process's, recorded in `log`; fails loudly on anything but exit 0. Every side
 * builds in the same environment, so a caller's `RUSTFLAGS` or `CARGO_PROFILE_*` cannot reach
 * one side and not the other.
 */
export async function run_build(
	host: ReportHost,
	side: Side,
	argv: ReadonlyArray<string>,
	log: CommandLog,
	options: RunBuildOptions = {}
): Promise<void> {
	const env = options.default_target_dir
		? { ...options.env }
		: { CARGO_TARGET_DIR: side.target, ...options.env };
	const cwd = options.cwd ?? side.src_handle;
	const env_text = Object.entries(options.env ?? {})
		.map(([k, v]) => `${k}=${v}`)
		.join(' ');
	log.record(
		`[${side.slot} ${side.commit.slice(0, 10)}] (cd ${cwd.path}; ${env_text ? `${env_text} ` : ''}${argv.join(' ')})`
	);
	const child = await host.build(env).start([...NICE_ARGS, ...argv], {
		cwd,
		stdio: ['ignore', 'inherit', 'inherit']
	});
	const result = await child.closed;
	if (exit_code_of(result) !== 0) {
		const ending = result.kind === 'exited' ? `exit ${result.code}` : format_ending(result);
		throw new HarnessError(`${argv.join(' ')} failed for ${side.slot} (${ending})`);
	}
}

/** A side's own task as its tree runs it: the runner and the whole command. */
export interface SideTask {
	runner: 'deno' | 'npm';
	argv: Array<string>;
}

/** The task names `text`, a `deno.json` or `package.json`, declares under `key`. */
function declares_task(
	text: string | null,
	key: 'tasks' | 'scripts',
	file: string
): (name: string) => boolean {
	if (text === null) return () => false;
	let parsed: unknown;
	try {
		parsed = JSON.parse(text);
	} catch (err) {
		throw new HarnessError(
			`${file} is not JSON: ${err instanceof Error ? err.message : String(err)}`
		);
	}
	const table = (parsed as Record<string, unknown> | null)?.[key];
	return (name) => typeof table === 'object' && table !== null && Object.hasOwn(table, name);
}

/**
 * How `side`'s tree runs its task `name` with `args`: `deno task` when its `deno.json` declares
 * the task (a tree from before the tasks moved, or a task whose body runs Deno), else `npm run`
 * when its `package.json` does, the arguments after `--`. Deno resolves a name the same way, so
 * the choice is the one `deno task <name>` makes, without Deno where the task needs none.
 * @throws `UnavailableOnRef` - when neither file declares it: the leg reports it not run
 */
export async function side_task(
	side: Side,
	name: string,
	args: ReadonlyArray<string> = []
): Promise<SideTask> {
	const src = side.src_handle;
	const deno_json = await src.read_text_if_exists('deno.json');
	if (declares_task(deno_json, 'tasks', `${side.src}/deno.json`)(name)) {
		return { runner: 'deno', argv: ['deno', 'task', name, ...args] };
	}
	const package_json = await src.read_text_if_exists('package.json');
	if (declares_task(package_json, 'scripts', `${side.src}/package.json`)(name)) {
		return {
			runner: 'npm',
			argv: ['npm', 'run', name, ...(args.length > 0 ? ['--', ...args] : [])]
		};
	}
	throw new UnavailableOnRef(
		`${side.slot} ${side.ref} has no task ${name} (neither its deno.json nor its package.json declares it)`
	);
}

/** Run `side`'s task `name` (`side_task`) through `run_build`, its runner required. */
export async function run_side_task(
	host: ReportHost,
	side: Side,
	name: string,
	args: ReadonlyArray<string>,
	log: CommandLog,
	options: RunBuildOptions = {}
): Promise<void> {
	const task = await side_task(side, name, args);
	require_tool(host, task.runner);
	await run_build(host, side, task.argv, log, options);
}

/** The corpus's identity for the provenance table. */
export async function corpus_identity(
	git: SpawnProgram,
	checkout: FsReadDir,
	corpus: string
): Promise<{ tree: string; dirty: boolean; relative_path: string }> {
	// asked of the corpus directory itself, so a repo root or a symlinked path keeps its identity;
	// `--no-optional-locks`, so `status` never refreshes (writes) the corpus repo's index
	const ask = (args: ReadonlyArray<string>) =>
		git.out(['--no-optional-locks', '-C', corpus, ...args], { cwd: checkout });
	const tree = await ask(['rev-parse', 'HEAD:./']);
	const status = await ask(['status', '--porcelain', '--', '.']);
	const prefix = await ask(['rev-parse', '--show-prefix']);
	return {
		tree: exit_code_of(tree) === 0 ? tree.stdout.trim() : `(no git checkout at ${corpus})`,
		dirty: status.stdout.trim() !== '',
		relative_path: exit_code_of(prefix) === 0 ? prefix.stdout.trim() : corpus
	};
}
