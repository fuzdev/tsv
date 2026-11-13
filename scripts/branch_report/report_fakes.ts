/**
 * Fakes for the branch report's suites: programs that record each call and answer from a
 * table, a `ReportHost` over a temp cache, and sides laid out beneath it without git. Plain
 * objects, no module mocking.
 *
 * @module
 */

import type { FsDir, FsReadDir } from '@fuzdev/gro/fs.ts';
import { create_fs_paths } from '@fuzdev/gro/fs_paths.ts';
import { fs } from '@fuzdev/gro/fs.ts';
import type {
	SpawnChild,
	SpawnOutput,
	SpawnOutputBytes,
	SpawnProgram,
	SpawnResult
} from '@fuzdev/gro/spawn.ts';

import { TOOL_NAMES, type ReportHost, type ReportTools, type Side, type Slot } from './side.ts';

/** One call a fake program saw. */
export interface FakeCall {
	program: string;
	args: Array<string>;
	cwd: string | null;
	/** The environment the program was made with (`build`'s), when it was. */
	env: Readonly<Record<string, string>> | null;
}

/** What a fake program answers a call with. */
export interface FakeAnswer {
	code?: number;
	stdout?: string;
	stderr?: string;
}

export type FakeRespond = (call: FakeCall) => FakeAnswer | void | Promise<FakeAnswer | void>;

/** A program named `name` at `path` that records each call into `calls` and answers with
 * `respond` (exit 0, no output, by default). */
export function fake_program(
	name: string,
	calls: Array<FakeCall>,
	respond: FakeRespond = () => undefined,
	options: { path?: string; env?: Readonly<Record<string, string>> } = {}
): SpawnProgram {
	const answer = async (args: ReadonlyArray<string>, cwd?: FsReadDir): Promise<FakeAnswer> => {
		const call: FakeCall = {
			program: name,
			args: [...args],
			cwd: cwd?.path ?? null,
			env: options.env ?? null
		};
		calls.push(call);
		return (await respond(call)) ?? {};
	};
	const result = (a: FakeAnswer): SpawnResult => ({ kind: 'exited', code: a.code ?? 0 });
	const out = async (
		args: ReadonlyArray<string>,
		o?: { cwd?: FsReadDir }
	): Promise<SpawnOutput> => {
		const a = await answer(args, o?.cwd);
		return { ...result(a), stdout: a.stdout ?? '', stderr: a.stderr ?? '', truncated: false };
	};
	return {
		name,
		path: options.path ?? `/fake/${name}`,
		run: async (args, o) => result(await answer(args, o?.cwd)),
		out,
		out_bytes: async (args, o): Promise<SpawnOutputBytes> => {
			const a = await answer(args, o?.cwd);
			const encoder = new TextEncoder();
			return {
				...result(a),
				stdout: encoder.encode(a.stdout ?? ''),
				stderr: encoder.encode(a.stderr ?? ''),
				truncated: false
			};
		},
		start: async (args, o): Promise<SpawnChild> => {
			const a = await answer(args, o?.cwd);
			const chunks = async function* (text: string | undefined): AsyncIterable<Uint8Array> {
				if (text) yield new TextEncoder().encode(text);
			};
			return {
				pid: 0,
				closed: Promise.resolve(result(a)),
				stdout: chunks(a.stdout),
				stderr: chunks(a.stderr),
				stdin: null,
				kill: () => false,
				[Symbol.asyncDispose]: async () => undefined
			};
		}
	};
}

/** Every tool absent. */
export const no_tools = (): Record<keyof ReportTools, SpawnProgram | null> =>
	Object.fromEntries(TOOL_NAMES.map((name) => [name, null])) as Record<
		keyof ReportTools,
		SpawnProgram | null
	>;

/** A host over `cache` and `checkout` whose programs all record into `calls`: the build answers
 * with `build`, each side program with `side`, and `tools` replaces the absent ones. */
export function fake_host(
	cache: FsDir,
	checkout: FsReadDir,
	calls: Array<FakeCall>,
	options: {
		build?: FakeRespond;
		side?: FakeRespond;
		tools?: Partial<Record<keyof ReportTools, SpawnProgram>>;
		/** Called with each name `side_program` is given, before it makes the program. */
		on_side_program?: (name: string) => void;
	} = {}
): ReportHost {
	return {
		checkout,
		fs_paths: create_fs_paths(fs),
		cache: {
			dir: cache,
			side_program: (name) => {
				options.on_side_program?.(name);
				return fake_program(name, calls, options.side, { path: `${cache.path}/${name}` });
			}
		},
		build: (env) => fake_program('nice', calls, options.build, { env }),
		git: fake_program('git', calls),
		tar: fake_program('tar', calls),
		tools: { ...no_tools(), ...options.tools },
		pid: 4242
	};
}

/** A side named `<tree>-<slot>` beneath `cache`, its tree holding `files`. */
export async function fake_side(
	cache: FsDir,
	slot: Slot,
	files: Readonly<Record<string, string>> = {},
	tree = 'a'.repeat(40)
): Promise<Side> {
	const name = `${tree}-${slot}`;
	await cache.mkdir(`${name}/src`, { recursive: true });
	const dir_handle = cache.dir(name);
	const src_handle = dir_handle.dir('src');
	for (const [path, text] of Object.entries(files)) {
		const parent = path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '';
		if (parent) await src_handle.mkdir(parent, { recursive: true });
		await src_handle.write(path, text);
	}
	const dir = `${cache.path}/${name}`;
	return {
		slot,
		ref: slot === 'base' ? 'main' : 'topic',
		commit: (slot === 'base' ? 'b' : 'c').repeat(40),
		tree,
		name,
		dir,
		src: `${dir}/src`,
		target: `${dir}/target`,
		dir_handle,
		src_handle,
		rustc: 'rustc 1.0.0',
		cargo: 'cargo 1.0.0'
	};
}
