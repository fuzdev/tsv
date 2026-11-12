/**
 * The real `node:*` bindings the credentialed release entries give their libraries, over temp
 * files and harmless programs (`sh`, `tar`, `node`; never `npm`): how a captured run ends and
 * what it gets, each file operation, `tar` taking one member out of a tarball the test builds,
 * each library's deps as its factory builds them, and each entry left as that one call.
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';
import { Buffer } from 'node:buffer';
import { lstat, mkdir, readFile, realpath, rm, stat, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import process from 'node:process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { afterEach, beforeEach, describe, test } from 'vitest';

import { tar_extract_args } from '../release_assets.ts';
import {
	create_publish_napi_deps,
	create_release_assets_deps,
	create_release_notes_deps,
	entry_checkout,
	node_capture,
	node_release_fs,
	RELEASE_PROGRAMS
} from './node_host.ts';

const CHECKOUT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

let dir: string;
beforeEach(async () => {
	dir = await node_release_fs.make_temp_dir('tsv_node_host_test_');
});
afterEach(async () => {
	await rm(dir, { recursive: true, force: true });
});

describe('node_capture', () => {
	test('captures both streams and the exit code', async () => {
		deepStrictEqual(
			await node_capture('sh', ['-c', 'printf " out\\n"; printf "err\\n" >&2; exit 3']),
			{
				code: 3,
				stdout: ' out\n',
				stderr: 'err\n'
			}
		);
	});

	test('a child ended by a signal exits 128 + its number', async () => {
		strictEqual((await node_capture('sh', ['-c', 'kill -TERM $$'])).code, 143);
		strictEqual((await node_capture('sh', ['-c', 'kill -KILL $$'])).code, 137);
	});

	test('a program not on PATH is a run that exited 127, naming it', async () => {
		deepStrictEqual(await node_capture('tsv_no_such_program', ['x']), {
			code: 127,
			stdout: '',
			stderr: 'tsv_no_such_program: command not found'
		});
	});

	test('decodes UTF-8 and drops a leading byte-order mark', async () => {
		strictEqual(
			(await node_capture('sh', ['-c', "printf '\\357\\273\\277caf\\303\\251'"])).stdout,
			'café'
		);
	});

	test('stdin is /dev/null', { skip: process.platform !== 'linux' }, async () => {
		strictEqual(
			(await node_capture('sh', ['-c', 'readlink /proc/self/fd/0'])).stdout,
			'/dev/null\n'
		);
	});

	test('runs in the working directory, or in `cwd`', async () => {
		strictEqual(
			(await node_capture('sh', ['-c', 'pwd -P'])).stdout,
			`${await realpath(process.cwd())}\n`
		);
		strictEqual(
			(await node_capture('sh', ['-c', 'pwd -P'], { cwd: dir })).stdout,
			`${await realpath(dir)}\n`
		);
	});

	test('the child inherits the whole environment', async () => {
		process.env.TSV_NODE_HOST_TEST = 'inherited';
		try {
			strictEqual(
				(await node_capture('sh', ['-c', 'printf %s "$TSV_NODE_HOST_TEST"'])).stdout,
				'inherited'
			);
		} finally {
			delete process.env.TSV_NODE_HOST_TEST;
		}
	});

	test('passes each argument as one word, through no shell', async () => {
		strictEqual(
			(await node_capture('sh', ['-c', 'printf "[%s]" "$@"', 'sh', 'a b', '$HOME', '*'])).stdout,
			'[a b][$HOME][*]'
		);
	});

	test('a program that is found but cannot start rejects, never reads as a run', async () => {
		const file = join(dir, 'not_executable');
		await writeFile(file, '#!/bin/sh\n', { mode: 0o644 });
		await rejects(node_capture(file, []), { code: 'EACCES' });
	});

	test('captures output past one pipe chunk, whole', async () => {
		const run = await node_capture('sh', ['-c', 'head -c 300000 /dev/zero | tr "\\0" x']);
		strictEqual(run.code, 0);
		strictEqual(run.stdout, 'x'.repeat(300000));
	});
});

describe('node_release_fs', () => {
	test('make_temp_dir makes a new directory beneath the temp dir, named by the prefix', async () => {
		ok(dir.startsWith(join(tmpdir(), 'tsv_node_host_test_')));
		ok((await stat(dir)).isDirectory());
		const other = await node_release_fs.make_temp_dir('tsv_node_host_test_');
		try {
			ok(other !== dir);
		} finally {
			await rm(other, { recursive: true });
		}
	});

	test('writes, reads and sizes a file', async () => {
		const path = join(dir, 'a.txt');
		await node_release_fs.write_text(path, 'héllo\n');
		strictEqual(await node_release_fs.read_text(path), 'héllo\n');
		deepStrictEqual([...(await node_release_fs.read_bytes(path))], [...Buffer.from('héllo\n')]);
		strictEqual(await node_release_fs.size(path), 7);
		await node_release_fs.write_text(path, 'x');
		strictEqual(await node_release_fs.read_text(path), 'x');
	});

	test('is_file follows a symlink; is_regular_file does not; neither throws', async () => {
		const file = join(dir, 'file');
		const link = join(dir, 'link');
		await writeFile(file, 'x');
		await symlink(file, link);
		await mkdir(join(dir, 'sub'));
		strictEqual(await node_release_fs.is_file(file), true);
		strictEqual(await node_release_fs.is_file(link), true);
		strictEqual(await node_release_fs.is_file(join(dir, 'sub')), false);
		strictEqual(await node_release_fs.is_file(join(dir, 'missing')), false);
		strictEqual(await node_release_fs.is_regular_file(file), true);
		strictEqual(await node_release_fs.is_regular_file(link), false);
		strictEqual(await node_release_fs.is_regular_file(join(dir, 'sub')), false);
		strictEqual(await node_release_fs.is_regular_file(join(dir, 'missing')), false);
	});

	test('set_mode and mode, and a copy that keeps the mode', async () => {
		const file = join(dir, 'bin');
		await writeFile(file, 'bytes', { mode: 0o644 });
		await node_release_fs.set_mode(file, 0o755);
		strictEqual((await node_release_fs.mode(file)) & 0o777, 0o755);
		await node_release_fs.copy_file(file, join(dir, 'copy'));
		strictEqual((await stat(join(dir, 'copy'))).mode & 0o777, 0o755);
		strictEqual(await node_release_fs.read_text(join(dir, 'copy')), 'bytes');
	});

	test('a copy replaces an existing file, as a re-run into one --out does', async () => {
		const from = join(dir, 'from');
		const to = join(dir, 'to');
		await writeFile(from, 'new');
		await writeFile(to, 'old');
		await node_release_fs.copy_file(from, to);
		strictEqual(await node_release_fs.read_text(to), 'new');
	});

	test('write_text creates the file at 0o666 less the umask', async () => {
		const path = join(dir, 'SHA512SUMS');
		await node_release_fs.write_text(path, 'x');
		strictEqual((await stat(path)).mode & 0o777, 0o666 & ~process.umask());
	});

	test('mode reads through a symlink to its target', async () => {
		const file = join(dir, 'bin');
		await writeFile(file, 'x', { mode: 0o755 });
		await symlink(file, join(dir, 'link'));
		strictEqual((await node_release_fs.mode(join(dir, 'link'))) & 0o777, 0o755);
	});

	test('make_dir makes parents and takes an existing one; remove_dir removes a tree', async () => {
		const nested = join(dir, 'a', 'b', 'c');
		await node_release_fs.make_dir(nested);
		await node_release_fs.make_dir(nested);
		await writeFile(join(nested, 'f'), 'x');
		await node_release_fs.remove_dir(join(dir, 'a'));
		await rejects(lstat(join(dir, 'a')), { code: 'ENOENT' });
		await rejects(node_release_fs.remove_dir(join(dir, 'a')), { code: 'ENOENT' });
	});
});

describe('tar_extract_args with the real tar', () => {
	/** A gzipped tarball at `dir/t.tgz` of the tree `build` makes under `dir/src`. */
	const make_tarball = async (build: (src: string) => Promise<void>, extra: Array<string> = []) => {
		const src = join(dir, 'src');
		await mkdir(join(src, 'package'), { recursive: true });
		await build(src);
		const made = await node_capture('tar', [
			'-czf',
			join(dir, 't.tgz'),
			...extra,
			'-C',
			src,
			'package'
		]);
		strictEqual(made.code, 0, made.stderr);
		return join(dir, 't.tgz');
	};

	test('takes the one member out, its bytes and mode, and nothing else', async () => {
		const tarball = await make_tarball(async (src) => {
			await writeFile(join(src, 'package', 'tsv'), 'binary', { mode: 0o755 });
			await writeFile(join(src, 'package', 'README.md'), 'readme');
		});
		const dest = join(dir, 'dest');
		await mkdir(dest);
		const run = await node_capture('tar', tar_extract_args(tarball, dest, 'package/tsv'));
		strictEqual(run.code, 0, run.stderr);
		strictEqual(await node_release_fs.read_text(join(dest, 'package', 'tsv')), 'binary');
		strictEqual((await stat(join(dest, 'package', 'tsv'))).mode & 0o111, 0o111);
		strictEqual(await node_release_fs.is_file(join(dest, 'package', 'README.md')), false);
	});

	test('a missing member fails the run', async () => {
		const tarball = await make_tarball(async (src) => {
			await writeFile(join(src, 'package', 'tsv.exe'), 'binary');
		});
		const run = await node_capture('tar', tar_extract_args(tarball, dir, 'package/tsv'));
		ok(run.code !== 0);
		ok(run.stderr.includes('package/tsv'), run.stderr);
	});

	test('a member that climbs out is never written by the one-member extraction', async () => {
		const tarball = await make_tarball(
			async (src) => {
				await writeFile(join(src, 'package', 'tsv'), 'binary');
				await writeFile(join(src, 'package', 'escape'), 'out');
			},
			['-P', '--transform', 's,^package/escape$,../escape,']
		);
		const dest = join(dir, 'dest');
		await mkdir(dest);
		const run = await node_capture('tar', tar_extract_args(tarball, dest, 'package/tsv'));
		strictEqual(run.code, 0, run.stderr);
		strictEqual(await node_release_fs.is_file(join(dir, 'escape')), false);
		strictEqual(await node_release_fs.read_text(join(dest, 'package', 'tsv')), 'binary');
	});

	test('a member that is a symlink extracts as one, which is_regular_file refuses', async () => {
		const tarball = await make_tarball(async (src) => {
			await symlink('/etc/hostname', join(src, 'package', 'tsv'));
		});
		const dest = join(dir, 'dest');
		await mkdir(dest);
		const run = await node_capture('tar', tar_extract_args(tarball, dest, 'package/tsv'));
		strictEqual(run.code, 0, run.stderr);
		strictEqual((await lstat(join(dest, 'package', 'tsv'))).isSymbolicLink(), true);
		strictEqual(await node_release_fs.is_regular_file(join(dest, 'package', 'tsv')), false);
	});
});

describe('the deps each entry hands its library', () => {
	/** Runs `run` with `PATH` naming only an empty directory, so no program is found. */
	const with_empty_path = async <T>(run: () => Promise<T>): Promise<T> => {
		const path = process.env.PATH;
		const empty = join(dir, 'empty_path');
		await mkdir(empty);
		process.env.PATH = empty;
		try {
			return await run();
		} finally {
			process.env.PATH = path;
		}
	};

	test('entry_checkout is two directories above the entry', () => {
		strictEqual(entry_checkout(pathToFileURL(join(dir, 'scripts', 'roots', 'x.ts')).href), dir);
		strictEqual(
			entry_checkout(pathToFileURL(join(CHECKOUT, 'scripts', 'roots', 'publish_napi.ts')).href),
			CHECKOUT
		);
	});

	test('the programs are npm and tar, found on PATH', async () => {
		deepStrictEqual(RELEASE_PROGRAMS, { npm: 'npm', tar: 'tar' });
		const publish = create_publish_napi_deps(dir);
		const assets = create_release_assets_deps(dir);
		const runs = await with_empty_path(() =>
			Promise.all([publish.npm(['x']), assets.npm(['x']), assets.tar(['x'])])
		);
		deepStrictEqual(
			runs.map((run) => [run.code, run.stderr]),
			[
				[127, 'npm: command not found'],
				[127, 'npm: command not found'],
				[127, 'tar: command not found']
			]
		);
	});

	test("publish_napi's: Node's platform, the file operations, npm in the checkout", async () => {
		const deps = create_publish_napi_deps(dir, { npm: 'sh' });
		strictEqual(deps.checkout, dir);
		strictEqual(deps.platform, process.platform);
		strictEqual(deps.read_text, node_release_fs.read_text);
		strictEqual(deps.is_file, node_release_fs.is_file);
		strictEqual(deps.set_mode, node_release_fs.set_mode);
		strictEqual(deps.mode, node_release_fs.mode);
		const run = await deps.npm(['-c', 'pwd -P; printf "[%s]" "$@"', 'sh', 'a b']);
		deepStrictEqual(run, { code: 0, stdout: `${await realpath(dir)}\n[a b]`, stderr: '' });
	});

	test("release_notes': the checkout and its text read", () => {
		const deps = create_release_notes_deps(dir);
		strictEqual(deps.checkout, dir);
		strictEqual(deps.read_text, node_release_fs.read_text);
	});

	test("release_assets': the working directory, npm and tar run in it, each its own", async () => {
		const deps = create_release_assets_deps(dir, { npm: 'sh', tar: 'tsv_no_such_tar' });
		strictEqual(deps.checkout, dir);
		strictEqual(deps.cwd, process.cwd());
		strictEqual((await deps.npm(['-c', 'pwd -P'])).stdout, `${await realpath(process.cwd())}\n`);
		strictEqual((await deps.tar(['x'])).stderr, 'tsv_no_such_tar: command not found');
		const tar_deps = create_release_assets_deps(dir, { npm: 'tsv_no_such_npm', tar: 'sh' });
		strictEqual(
			(await tar_deps.tar(['-c', 'pwd -P'])).stdout,
			`${await realpath(process.cwd())}\n`
		);
		strictEqual((await tar_deps.npm(['x'])).stderr, 'tsv_no_such_npm: command not found');
	});

	test("release_assets': the file operations, is_regular_file not following a symlink", () => {
		deepStrictEqual(create_release_assets_deps(dir).fs, {
			make_dir: node_release_fs.make_dir,
			make_temp_dir: node_release_fs.make_temp_dir,
			remove_dir: node_release_fs.remove_dir,
			copy_file: node_release_fs.copy_file,
			is_regular_file: node_release_fs.is_regular_file,
			size: node_release_fs.size,
			read_bytes: node_release_fs.read_bytes,
			write_text: node_release_fs.write_text
		});
	});

	test("release_assets' pause waits", async () => {
		const start = performance.now();
		await create_release_assets_deps(dir).sleep(60);
		ok(performance.now() - start >= 50);
	});

	test('print writes a line to stdout and print_error to stderr, for each', async () => {
		const host = pathToFileURL(join(CHECKOUT, 'scripts', 'release', 'node_host.ts')).href;
		const run = await node_capture(process.execPath, [
			'--input-type=module',
			'-e',
			`import * as host from ${JSON.stringify(host)};
for (const create of [host.create_publish_napi_deps, host.create_release_notes_deps, host.create_release_assets_deps]) {
	const deps = create('/');
	deps.print('out %s');
	deps.print_error('err %s');
}`
		]);
		deepStrictEqual(run, {
			code: 0,
			stdout: 'out %s\n'.repeat(3),
			stderr: 'err %s\n'.repeat(3)
		});
	});
});

describe('each entry', () => {
	/** The entry's code past its module doc and imports, its whitespace collapsed. */
	const code_of = async (entry: string): Promise<string> =>
		(await readFile(join(CHECKOUT, 'scripts', 'roots', entry), 'utf8'))
			.replace(/^\/\*\*[\s\S]*?\*\/\n/, '')
			.replace(/^import [^;]*;\n/gm, '')
			.replace(/\s+/g, ' ')
			.trim();

	// what neither `tsc` nor a closure row sees: the exit code set from the one call, over the
	// entry's own checkout and its arguments
	const entries: Array<[entry: string, run: string, create: string]> = [
		['publish_napi.ts', 'run_publish_napi', 'create_publish_napi_deps'],
		['release_notes.ts', 'run_release_notes', 'create_release_notes_deps'],
		['release_assets.ts', 'run_release_assets', 'create_release_assets_deps']
	];
	for (const [entry, run, create] of entries) {
		test(`${entry} sets the exit code from its one call`, async () => {
			strictEqual(
				await code_of(entry),
				`process.exitCode = await ${run}( ${create}(entry_checkout(import.meta.url)), process.argv.slice(2) );`
			);
		});
	}
});
