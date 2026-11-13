/**
 * `run_if_stale` over a temp checkout and fake programs: the usage errors, a missing, stale,
 * or fresh target, the links under a crate's `src/`, `TSV_BUILD_FORCE`, the command's exit passed
 * on, and a command it may not run.
 */

import { deepStrictEqual, rejects, strictEqual } from 'node:assert';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram, SpawnResult } from '@fuzdev/gro/spawn.ts';
import { test } from 'vitest';

import { CORE_CRATES, WASM_CRATES } from '../benches/js/lib/crate_lists.ts';
import { run_if_stale, type RunIfStaleDeps } from './run_if_stale.ts';

const TARGET = 'crates/tsv_wasm/pkg/format/deno/tsv_wasm_bg.wasm';
const COMMAND = ['wasm-pack', 'build', 'crates/tsv_wasm', '--out-dir', 'pkg/format/deno'];
const ARGV = ['--target', TARGET, '--', ...COMMAND];

/** Seconds apart, so an mtime comparison never ties. */
const T_SOURCE = Date.UTC(2026, 0, 1);
const T_TARGET = T_SOURCE + 60_000;
const T_LATER = T_TARGET + 60_000;

/** A checkout with every crate the bundle reads, the workspace files, and `package.json`, all
 * dated `T_SOURCE`, and the target dated `T_TARGET` unless `target` is false. */
const make_checkout = async (target = true): Promise<FsDir & AsyncDisposable> => {
	const root = await fs.temp_dir('tsv_run_if_stale_test_');
	const write = async (path: string, ms = T_SOURCE): Promise<void> => {
		const dir = path.split('/').slice(0, -1).join('/');
		if (dir !== '') await root.mkdir(dir, { recursive: true });
		await root.write(path, path);
		await root.set_times(path, { mtime_ms: ms });
	};
	for (const crate of [...CORE_CRATES, ...WASM_CRATES, 'tsv_cli']) {
		await write(`crates/${crate}/Cargo.toml`);
		await write(`crates/${crate}/src/lib.rs`);
	}
	for (const file of [
		'Cargo.toml',
		'Cargo.lock',
		'.cargo/config.toml',
		'package.json',
		'deno.json'
	])
		await write(file);
	if (target) await write(TARGET, T_TARGET);
	return root;
};

const touch = (root: FsDir, path: string, ms = T_LATER): Promise<void> =>
	root.set_times(path, { mtime_ms: ms });

interface Call {
	args: ReadonlyArray<string>;
	cwd: string | undefined;
}

/** A program that records each `run` and ends with `result`; its other ops throw. */
const fake_program = (name: string, result: SpawnResult = { kind: 'exited', code: 0 }) => {
	const calls: Array<Call> = [];
	const unused = (): never => {
		throw new Error(`${name}: only run is faked`);
	};
	const program: SpawnProgram = {
		name,
		path: `/fake/${name}`,
		run: async (args, options) => {
			calls.push({ args, cwd: options?.cwd?.path });
			return result;
		},
		out: unused,
		out_bytes: unused,
		start: unused
	};
	return { program, calls };
};

/** The deps over `checkout`, with what `run_if_stale` printed. */
const make_deps = (
	checkout: FsDir,
	programs: RunIfStaleDeps['programs'],
	force = false
): { deps: RunIfStaleDeps; out: Array<string>; err: Array<string> } => {
	const out: Array<string> = [];
	const err: Array<string> = [];
	return {
		deps: {
			checkout: checkout.readonly(),
			force,
			programs,
			print: (line) => out.push(line),
			print_error: (line) => err.push(line)
		},
		out,
		err
	};
};

const USAGE = 'usage: run_if_stale.ts --target <artifact-path> -- <command> [args...]';

test('a usage error exits 2 and runs nothing', async () => {
	await using checkout = await make_checkout();
	const { program, calls } = fake_program('wasm-pack');
	for (const argv of [
		[],
		['--', ...COMMAND],
		['--target', TARGET],
		['--target', '--', ...COMMAND],
		['--', '--target', TARGET]
	]) {
		const { deps, out, err } = make_deps(checkout, new Map([['wasm-pack', program]]));
		strictEqual(await run_if_stale(deps, argv), 2, argv.join(' '));
		deepStrictEqual(err, [USAGE]);
		deepStrictEqual(out, []);
	}
	const { deps, err } = make_deps(checkout, new Map([['wasm-pack', program]]));
	strictEqual(await run_if_stale(deps, ['--target', TARGET, '--']), 2);
	deepStrictEqual(err, ['run_if_stale.ts: no command after --']);
	deepStrictEqual(calls, []);
});

test('a missing target builds, in the checkout, with the command as given', async () => {
	await using checkout = await make_checkout(false);
	const { program, calls } = fake_program('wasm-pack');
	const { deps, out, err } = make_deps(checkout, new Map([['wasm-pack', program]]));
	strictEqual(await run_if_stale(deps, ARGV), 0);
	deepStrictEqual(calls, [{ args: COMMAND.slice(1), cwd: checkout.path }]);
	deepStrictEqual([out, err], [[], []]);
});

test('a target newer than every source skips, saying so', async () => {
	await using checkout = await make_checkout();
	const { program, calls } = fake_program('wasm-pack');
	const { deps, out, err } = make_deps(checkout, new Map([['wasm-pack', program]]));
	strictEqual(await run_if_stale(deps, ARGV), 0);
	deepStrictEqual(calls, []);
	deepStrictEqual(out, [
		`run_if_stale: ${TARGET} is fresh — skipping \`wasm-pack\` (TSV_BUILD_FORCE=1 to force)`
	]);
	deepStrictEqual(err, []);
});

test('a fresh skip needs no program', async () => {
	await using checkout = await make_checkout();
	const { deps } = make_deps(checkout, new Map([['wasm-pack', null]]));
	strictEqual(await run_if_stale(deps, ARGV), 0);
});

test('each input newer than the target builds', async () => {
	for (const input of [
		'crates/tsv_lang/src/lib.rs',
		'crates/tsv_svelte/Cargo.toml',
		'crates/tsv_wasm/src/lib.rs',
		'crates/tsv_discover/src/lib.rs',
		'Cargo.toml',
		'Cargo.lock',
		'.cargo/config.toml',
		'package.json'
	]) {
		await using checkout = await make_checkout();
		await touch(checkout, input);
		const { program, calls } = fake_program('wasm-pack');
		const { deps, out } = make_deps(checkout, new Map([['wasm-pack', program]]));
		strictEqual(await run_if_stale(deps, ARGV), 0, input);
		strictEqual(calls.length, 1, input);
		deepStrictEqual(out, [], input);
	}
});

test("a build.rs and any file under src/ count, the crate's other files don't", async () => {
	await using checkout = await make_checkout();
	await checkout.write('crates/tsv_wasm/README.md', 'x');
	await touch(checkout, 'crates/tsv_wasm/README.md');
	await checkout.mkdir('crates/tsv_wasm/src/nested', { recursive: true });
	const fresh = fake_program('wasm-pack');
	strictEqual(
		await run_if_stale(make_deps(checkout, new Map([['wasm-pack', fresh.program]])).deps, ARGV),
		0
	);
	strictEqual(fresh.calls.length, 0);
	for (const input of ['crates/tsv_wasm/build.rs', 'crates/tsv_wasm/src/nested/table.txt']) {
		await checkout.write(input, 'x');
		await touch(checkout, input);
		const stale = fake_program('wasm-pack');
		strictEqual(
			await run_if_stale(make_deps(checkout, new Map([['wasm-pack', stale.program]])).deps, ARGV),
			0
		);
		strictEqual(stale.calls.length, 1, input);
		await touch(checkout, input, T_SOURCE);
	}
});

test('a link under src/ is dated by what it points at, and a directory is not a source', async () => {
	await using checkout = await make_checkout();
	await checkout.mkdir('crates/tsv_wasm/src/nested', { recursive: true });
	await checkout.symlink('crates/tsv_wasm/src/linked', 'nested');
	await checkout.symlink('crates/tsv_wasm/src/dangling.rs', 'gone.rs');
	await touch(checkout, 'crates/tsv_wasm/src/nested');
	const fresh = fake_program('wasm-pack');
	strictEqual(
		await run_if_stale(make_deps(checkout, new Map([['wasm-pack', fresh.program]])).deps, ARGV),
		0
	);
	strictEqual(fresh.calls.length, 0);
	await checkout.write('crates/tsv_wasm/table.txt', 'x');
	await touch(checkout, 'crates/tsv_wasm/table.txt');
	await checkout.symlink('crates/tsv_wasm/src/table.txt', '../table.txt');
	const stale = fake_program('wasm-pack');
	strictEqual(
		await run_if_stale(make_deps(checkout, new Map([['wasm-pack', stale.program]])).deps, ARGV),
		0
	);
	strictEqual(stale.calls.length, 1);
});

test("a source the checkout can't follow fails, naming it, and runs nothing", async () => {
	await using checkout = await make_checkout();
	await checkout.symlink('crates/tsv_wasm/src/outside.rs', '/outside.rs');
	const { program, calls } = fake_program('wasm-pack');
	const { deps, out } = make_deps(checkout, new Map([['wasm-pack', program]]));
	await rejects(run_if_stale(deps, ARGV), (error: Error) =>
		error.message.startsWith("can't read crates/tsv_wasm/src/outside.rs to date what it builds: ")
	);
	deepStrictEqual([calls, out], [[], []]);
});

test('package.json newer than the target builds, by any margin', async () => {
	await using checkout = await make_checkout();
	// the sources a millisecond older than the target, package.json a millisecond newer
	await touch(checkout, 'crates/tsv_wasm/src/lib.rs', T_TARGET - 1);
	await touch(checkout, 'package.json', T_TARGET + 1);
	const { program, calls } = fake_program('wasm-pack');
	strictEqual(
		await run_if_stale(make_deps(checkout, new Map([['wasm-pack', program]])).deps, ARGV),
		0
	);
	strictEqual(calls.length, 1);
});

test("an edit outside the bundle's inputs skips", async () => {
	await using checkout = await make_checkout();
	for (const other of ['crates/tsv_cli/src/lib.rs', 'deno.json']) await touch(checkout, other);
	const { program, calls } = fake_program('wasm-pack');
	strictEqual(
		await run_if_stale(make_deps(checkout, new Map([['wasm-pack', program]])).deps, ARGV),
		0
	);
	deepStrictEqual(calls, []);
});

test('a source dated with the target builds', async () => {
	await using checkout = await make_checkout();
	await touch(checkout, 'crates/tsv_lang/src/lib.rs', T_TARGET);
	const { program, calls } = fake_program('wasm-pack');
	strictEqual(
		await run_if_stale(make_deps(checkout, new Map([['wasm-pack', program]])).deps, ARGV),
		0
	);
	strictEqual(calls.length, 1);
});

test('force builds a fresh target', async () => {
	await using checkout = await make_checkout();
	const { program, calls } = fake_program('wasm-pack');
	const { deps, out } = make_deps(checkout, new Map([['wasm-pack', program]]), true);
	strictEqual(await run_if_stale(deps, ARGV), 0);
	strictEqual(calls.length, 1);
	deepStrictEqual(out, []);
});

test("the command's exit is passed on, 128 + N for signal N", async () => {
	await using checkout = await make_checkout(false);
	for (const [result, code] of [
		[{ kind: 'exited', code: 3 }, 3],
		[{ kind: 'exited', code: 0 }, 0],
		[{ kind: 'signaled', signal: 'SIGTERM' }, 143],
		[{ kind: 'signaled', signal: 'SIGKILL' }, 137]
	] as const) {
		const { program } = fake_program('wasm-pack', result);
		const { deps } = make_deps(checkout, new Map([['wasm-pack', program]]));
		strictEqual(await run_if_stale(deps, ARGV), code);
	}
});

test('a command that is not granted, or not on PATH, exits 1 and runs nothing', async () => {
	await using checkout = await make_checkout(false);
	const { program, calls } = fake_program('wasm-pack');
	const granted = make_deps(checkout, new Map([['wasm-pack', program]]));
	strictEqual(await run_if_stale(granted.deps, ['--target', TARGET, '--', 'cargo', 'build']), 1);
	deepStrictEqual(granted.err, [
		'run_if_stale.ts: `cargo` is not a command this script runs (wasm-pack)'
	]);
	deepStrictEqual(calls, []);
	const missing = make_deps(checkout, new Map([['wasm-pack', null]]));
	strictEqual(await run_if_stale(missing.deps, ARGV), 1);
	deepStrictEqual(missing.err, ['run_if_stale.ts: `wasm-pack` is not on PATH']);
});
