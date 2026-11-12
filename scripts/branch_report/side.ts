/**
 * One side of a branch report: a ref resolved to a commit, its whole tree extracted from git
 * into its own directory under the cache (`<tree>-<slot>/src`), and one target directory beside
 * it. The two sides of a same-ref run are extracted and built apart on purpose: a difference
 * there is the build's own non-determinism, which a shared binary could never show.
 *
 * @module
 */

import { existsSync } from 'node:fs';
import { mkdir, readdir, rm, stat, utimes } from 'node:fs/promises';
import { resolve } from 'node:path';

import { type CachedTree, trees_to_evict } from '../perf_ab_verdict.ts';
import { extract_tree, git, HarnessError, kept_build_env, run, ROOT } from './deno_host.ts';

export type Slot = 'base' | 'cand';

export interface Side {
	slot: Slot;
	ref: string;
	commit: string;
	tree: string;
	/** The side's directory under the cache. */
	dir: string;
	/** Its extracted source tree. */
	src: string;
	/** Its Cargo target directory. */
	target: string;
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

/** Throw `MissingTool` unless `tool` can be spawned (looked up on `PATH`, a path otherwise). */
export async function require_tool(tool: string): Promise<void> {
	try {
		await new Deno.Command(tool, { args: ['--version'], stdout: 'null', stderr: 'null' }).output();
	} catch (err) {
		if (err instanceof Deno.errors.NotFound) throw new MissingTool(tool);
		throw err;
	}
}

export const SIDE_DIR_PATTERN = /^[0-9a-f]{40}-(base|cand)$/;

/** A side's first line of `rustc -vV` / `cargo -V`, run in its tree so its toolchain pin applies. */
async function tool_version(cmd: string, flag: string, cwd: string): Promise<string> {
	const result = await run(cmd, [flag], { cwd, env: kept_build_env(), clear_env: true });
	if (!result.success) throw new HarnessError(`${cmd} ${flag} in ${cwd} failed: ${result.stderr}`);
	return result.stdout.trim().replaceAll('\n', ' ');
}

/** Resolve `ref`, extract its tree into `<cache>/<tree>-<slot>/src` (kept if already there). */
export async function prepare_side(ref: string, slot: Slot, cache: string): Promise<Side> {
	const commit = await git(['rev-parse', '--verify', '--quiet', `${ref}^{commit}`]).catch(() => {
		throw new HarnessError(`not a commit: ${ref}`);
	});
	const tree = await git(['rev-parse', `${commit}^{tree}`]);
	const dir = resolve(cache, `${tree}-${slot}`);
	const src = resolve(dir, 'src');
	if (!existsSync(resolve(src, 'Cargo.toml'))) {
		await mkdir(dir, { recursive: true });
		console.error(`branch:report: ${slot} ${ref}: extracting tree ${tree.slice(0, 12)}`);
		await extract_tree(tree, [], src);
	}
	const [rustc, cargo] = await Promise.all([
		tool_version('rustc', '-vV', src),
		tool_version('cargo', '-V', src)
	]);
	return { slot, ref, commit, tree, dir, src, target: resolve(dir, 'target'), rustc, cargo };
}

/** How many side directories (`SIDE_DIR_PATTERN`) a cache keeps in all. */
export const KEEP_SIDES = 4;

/** Keep the sides this run uses and the most recently used others (`KEEP_SIDES` in all); delete
 * the rest. A side carries several gigabytes of build output, so a cache must not grow by run. */
export async function evict_sides(cache: string, sides: ReadonlyArray<Side>): Promise<void> {
	const in_use = new Set(sides.map((side) => `${side.tree}-${side.slot}`));
	const now = new Date();
	for (const name of in_use) await utimes(resolve(cache, name), now, now);
	const trees: Array<CachedTree> = [];
	for (const entry of await readdir(cache, { withFileTypes: true })) {
		if (!entry.isDirectory() || !SIDE_DIR_PATTERN.test(entry.name)) continue;
		trees.push({ name: entry.name, mtime_ms: (await stat(resolve(cache, entry.name))).mtimeMs });
	}
	for (const name of trees_to_evict(trees, in_use, KEEP_SIDES)) {
		console.error(`branch:report: evicting cached side ${name.slice(0, 16)}`);
		await rm(resolve(cache, name), { recursive: true, force: true });
	}
}

/** The env of every build a side runs: the kept toolchain variables, nothing else, so a caller's
 * `RUSTFLAGS` or `CARGO_PROFILE_*` cannot reach one side and not the other. */
export function side_env(side: Side, extra: Record<string, string> = {}): Record<string, string> {
	return { CARGO_TARGET_DIR: side.target, ...kept_build_env(), ...extra };
}

const NICE_PREFIX = ['nice', '-n', '19', 'ionice', '-c', '3'];

/** Run `argv` at idle priority in a side's tree under a cleared environment, recorded in `log`;
 * fails loudly on a nonzero exit. */
export async function run_build(
	side: Side,
	argv: ReadonlyArray<string>,
	log: CommandLog,
	options: { env?: Record<string, string>; cwd?: string; default_target_dir?: boolean } = {}
): Promise<void> {
	// a task that reads `target/` by name (the PGO pipeline, the bench's stamped builds) needs
	// cargo's default target dir, inside the extracted tree
	const env = options.default_target_dir
		? { ...kept_build_env(), ...options.env }
		: side_env(side, options.env);
	const cwd = options.cwd ?? side.src;
	const env_text = Object.entries(options.env ?? {})
		.map(([k, v]) => `${k}=${v}`)
		.join(' ');
	log.record(
		`[${side.slot} ${side.commit.slice(0, 10)}] (cd ${cwd}; ${env_text ? `${env_text} ` : ''}${argv.join(' ')})`
	);
	const [command, ...args] = [...NICE_PREFIX, ...argv];
	const output = await new Deno.Command(command!, {
		args,
		cwd,
		env,
		clearEnv: true,
		stdout: 'inherit',
		stderr: 'inherit'
	}).output();
	if (!output.success) {
		throw new HarnessError(`${argv.join(' ')} failed for ${side.slot} (exit ${output.code})`);
	}
}

/** The corpus's identity for the provenance table. */
export async function corpus_identity(
	corpus: string
): Promise<{ tree: string; dirty: boolean; relative_path: string }> {
	// asked of the corpus directory itself, so a repo root or a symlinked path keeps its identity
	const tree = await run('git', ['-C', corpus, 'rev-parse', 'HEAD:./'], { cwd: ROOT });
	const status = await run('git', ['-C', corpus, 'status', '--porcelain', '--', '.'], {
		cwd: ROOT
	});
	const prefix = await run('git', ['-C', corpus, 'rev-parse', '--show-prefix'], { cwd: ROOT });
	return {
		tree: tree.success ? tree.stdout.trim() : `(no git checkout at ${corpus})`,
		dirty: status.stdout.trim() !== '',
		relative_path: prefix.success ? prefix.stdout.trim() : corpus
	};
}
