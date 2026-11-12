/**
 * The release assets over a fake file system, a fake `npm` (a canned `npm view`, an `npm pack`
 * that writes a tarball) and a fake `tar` (which writes the member it is asked for), with the
 * pause between registry attempts injected: the argv of every run and their order, the asset
 * names and bytes, the `SHA512SUMS` format, the retry count and message, each refusal, and
 * what a failure midway leaves behind. Every capability is a fake
 * (`scripts/no_package_closure_test.ts` holds this suite to that).
 */

import { deepStrictEqual, rejects, strictEqual } from 'node:assert';
import { join } from 'node:path';
import { describe, test } from 'vitest';

import {
	PACK_DIR_PREFIX,
	REGISTRY_ATTEMPTS,
	REGISTRY_RETRY_MS,
	type ReleaseAssetsDeps,
	run_release_assets,
	tar_extract_args
} from './release_assets.ts';

const CHECKOUT = join('/', 'repo');
const CWD = join('/', 'work');
const OUT = join('/', 'assets');
const PACK_DIR = join('/', 'tmp', `${PACK_DIR_PREFIX}abc123`);
const VERSION = '1.2.3';
const GNU = '@fuzdev/tsv-linux-x64-gnu';
const WIN = '@fuzdev/tsv-win32-x64';

/** SHA-512 of `abc` (FIPS 180-2, appendix C). */
const ABC_SHA512 =
	'ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a' +
	'2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f';
/** SHA-512 of the empty string. */
const EMPTY_SHA512 =
	'cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce' +
	'47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e';

const bytes = (text: string): Uint8Array<ArrayBuffer> => new TextEncoder().encode(text);
const text = (data: Uint8Array | undefined): string | undefined =>
	data === undefined ? undefined : new TextDecoder().decode(data);

interface Reply {
	code?: number;
	stdout?: string;
	stderr?: string;
}

/** A fake tarball: its members, each a file's bytes or a symlink's target. */
type Tarball = Map<string, { file: string } | { symlink: string }>;

interface FakeWorld {
	deps: ReleaseAssetsDeps;
	/** Every call, in order, as `[op, ...args]`. */
	calls: Array<Array<unknown>>;
	/** Each npm and tar run, in order, as `[program, ...args]`. */
	runs: Array<Array<string>>;
	files: Map<string, Uint8Array>;
	symlinks: Set<string>;
	sleeps: Array<number>;
	out: Array<string>;
	err: Array<string>;
}

/** The tarball each platform package packs to: its CLI binary (`abc` for gnu, empty for
 * win32) beside a README. */
const PACKAGES: Record<string, { filename: string; tarball: Tarball }> = {
	[GNU]: {
		filename: `fuzdev-tsv-linux-x64-gnu-${VERSION}.tgz`,
		tarball: new Map([
			['package/tsv', { file: 'abc' }],
			['package/README.md', { file: 'readme' }]
		])
	},
	[WIN]: {
		filename: `fuzdev-tsv-win32-x64-${VERSION}.tgz`,
		tarball: new Map([['package/tsv.exe', { file: '' }]])
	}
};

/**
 * A fake world: `npm view` answers `optional` as JSON, `npm pack` writes `PACKAGES`' tarball
 * into its `--pack-destination` and reports it, `tar` writes the member it names into `-C`,
 * and the checkout holds the NOTICE. `npm` and `tar` replies override per run.
 */
const fake_world = (
	options: {
		optional?: Record<string, string>;
		npm?: (args: Array<string>, attempt: number) => Reply | undefined;
		tar?: (args: Array<string>) => Reply | undefined;
		packages?: Record<string, { filename: string; tarball: Tarball }>;
	} = {}
): FakeWorld => {
	const calls: Array<Array<unknown>> = [];
	const runs: Array<Array<string>> = [];
	const files = new Map<string, Uint8Array>([
		[join(CHECKOUT, 'crates/tsv_html/NOTICE'), bytes('notice')]
	]);
	const symlinks = new Set<string>();
	const tarballs = new Map<string, Tarball>();
	const sleeps: Array<number> = [];
	const out: Array<string> = [];
	const err: Array<string> = [];
	const attempts = new Map<string, number>();
	const packages = options.packages ?? PACKAGES;
	const optional = options.optional ?? { [GNU]: VERSION, [WIN]: VERSION };
	const reply = ({ code = 0, stdout = '', stderr = '' }: Reply) => ({ code, stdout, stderr });
	return {
		calls,
		runs,
		files,
		symlinks,
		sleeps,
		out,
		err,
		deps: {
			checkout: CHECKOUT,
			cwd: CWD,
			npm: async (args) => {
				calls.push(['npm', ...args]);
				runs.push(['npm', ...args]);
				const key = args.join(' ');
				const attempt = (attempts.get(key) ?? 0) + 1;
				attempts.set(key, attempt);
				const override = options.npm?.(args, attempt);
				if (override) return reply(override);
				if (args[0] === 'view') return reply({ stdout: JSON.stringify(optional, null, 2) + '\n' });
				if (args[0] === 'pack') {
					const name = args[1]!.slice(0, args[1]!.lastIndexOf('@'));
					const packed = packages[name]!;
					tarballs.set(
						`${args[args.indexOf('--pack-destination') + 1]}/${packed.filename}`,
						packed.tarball
					);
					return reply({ stdout: JSON.stringify([{ filename: packed.filename }]) });
				}
				throw new Error(`unexpected npm ${key}`);
			},
			tar: async (args) => {
				calls.push(['tar', ...args]);
				runs.push(['tar', ...args]);
				const override = options.tar?.(args);
				if (override) return reply(override);
				const [, tarball, , dir, member] = args;
				const entry = tarballs.get(tarball!)?.get(member!);
				if (!entry) return reply({ code: 2, stderr: `tar: ${member}: Not found in archive\n` });
				const path = `${dir}/${member}`;
				if ('file' in entry) {
					files.set(path, bytes(entry.file));
					symlinks.delete(path);
				} else {
					symlinks.add(path);
					files.delete(path);
				}
				return reply({});
			},
			sleep: async (ms) => {
				calls.push(['sleep', ms]);
				sleeps.push(ms);
			},
			fs: {
				make_dir: async (path) => {
					calls.push(['make_dir', path]);
				},
				make_temp_dir: async (prefix) => {
					calls.push(['make_temp_dir', prefix]);
					return PACK_DIR;
				},
				remove_dir: async (path) => {
					calls.push(['remove_dir', path]);
					for (const key of [...files.keys()]) if (key.startsWith(`${path}/`)) files.delete(key);
					for (const key of [...symlinks]) if (key.startsWith(`${path}/`)) symlinks.delete(key);
				},
				copy_file: async (from, to) => {
					calls.push(['copy_file', from, to]);
					const data = files.get(from);
					if (data === undefined) throw new Error(`ENOENT: ${from}`);
					files.set(to, data);
				},
				is_regular_file: async (path) => {
					calls.push(['is_regular_file', path]);
					return files.has(path);
				},
				size: async (path) => {
					calls.push(['size', path]);
					const data = files.get(path);
					if (data === undefined) throw new Error(`ENOENT: ${path}`);
					// a MiB per byte, so the printed sizes are round
					return data.length * 1024 * 1024;
				},
				read_bytes: async (path) => {
					calls.push(['read_bytes', path]);
					const data = files.get(path);
					if (data === undefined) throw new Error(`ENOENT: ${path}`);
					return new Uint8Array(data);
				},
				write_text: async (path, content) => {
					calls.push(['write_text', path]);
					files.set(path, bytes(content));
				}
			},
			print: (line) => out.push(line),
			print_error: (line) => err.push(line)
		}
	};
};

const VIEW = ['npm', 'view', `@fuzdev/tsv@${VERSION}`, 'optionalDependencies', '--json'];
const pack = (name: string) => [
	'npm',
	'pack',
	`${name}@${VERSION}`,
	'--min-release-age=0',
	'--pack-destination',
	PACK_DIR,
	'--json'
];

/** The files under `dir`, by name, as text. */
const files_in = (world: FakeWorld, dir: string): Record<string, string | undefined> =>
	Object.fromEntries(
		[...world.files.keys()]
			.filter((path) => path.startsWith(`${dir}/`))
			.sort()
			.map((path) => [path.slice(dir.length + 1), text(world.files.get(path))])
	);

describe('a collection', () => {
	test("each npm pack waives the release-age window the checkout's .npmrc sets", async () => {
		const world = fake_world();
		strictEqual(await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]), 0);
		const packs = world.runs.filter((run) => run[1] === 'pack');
		strictEqual(packs.length, 2);
		for (const run of packs) strictEqual(run.includes('--min-release-age=0'), true, run.join(' '));
	});

	test('views the loader, then packs and extracts each platform in order, then sums', async () => {
		const world = fake_world();
		strictEqual(await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]), 0);
		deepStrictEqual(world.runs, [
			VIEW,
			pack(GNU),
			[
				'tar',
				'-xzf',
				`${PACK_DIR}/fuzdev-tsv-linux-x64-gnu-${VERSION}.tgz`,
				'-C',
				PACK_DIR,
				'package/tsv'
			],
			pack(WIN),
			[
				'tar',
				'-xzf',
				`${PACK_DIR}/fuzdev-tsv-win32-x64-${VERSION}.tgz`,
				'-C',
				PACK_DIR,
				'package/tsv.exe'
			]
		]);
		deepStrictEqual(files_in(world, OUT), {
			NOTICE: 'notice',
			SHA512SUMS: `${ABC_SHA512}  tsv-linux-x64-gnu\n${EMPTY_SHA512}  tsv-win32-x64.exe\n`,
			'tsv-linux-x64-gnu': 'abc',
			'tsv-win32-x64.exe': ''
		});
		deepStrictEqual(world.out, [
			`  tsv-linux-x64-gnu  (${GNU}@${VERSION}, 3.00 MB)`,
			`  tsv-win32-x64.exe  (${WIN}@${VERSION}, 0.00 MB)`,
			'  SHA512SUMS  (2 assets)',
			'  NOTICE  (crates/tsv_html/NOTICE)'
		]);
		deepStrictEqual(world.err, []);
		deepStrictEqual(world.sleeps, []);
	});

	test('makes --out, then one temp dir, and removes the temp dir before the sums', async () => {
		const world = fake_world();
		await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]);
		const ops = world.calls
			.map(([op]) => op)
			.filter(
				(op) =>
					op === 'make_dir' || op === 'make_temp_dir' || op === 'remove_dir' || op === 'write_text'
			);
		deepStrictEqual(ops, ['make_dir', 'make_temp_dir', 'remove_dir', 'write_text']);
		deepStrictEqual(
			world.calls.find(([op]) => op === 'make_dir'),
			['make_dir', OUT]
		);
		deepStrictEqual(
			world.calls.find(([op]) => op === 'make_temp_dir'),
			['make_temp_dir', PACK_DIR_PREFIX]
		);
		deepStrictEqual(
			world.calls.find(([op]) => op === 'remove_dir'),
			['remove_dir', PACK_DIR]
		);
		deepStrictEqual(files_in(world, PACK_DIR), {});
	});

	test('copies each binary and the NOTICE, the NOTICE from the checkout', async () => {
		const world = fake_world();
		await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]);
		deepStrictEqual(
			world.calls.filter(([op]) => op === 'copy_file'),
			[
				['copy_file', `${PACK_DIR}/package/tsv`, join(OUT, 'tsv-linux-x64-gnu')],
				['copy_file', `${PACK_DIR}/package/tsv.exe`, join(OUT, 'tsv-win32-x64.exe')],
				['copy_file', join(CHECKOUT, 'crates/tsv_html/NOTICE'), join(OUT, 'NOTICE')]
			]
		);
	});

	test('a relative --out is read against the working directory; the tag may drop its v', async () => {
		const world = fake_world();
		strictEqual(await run_release_assets(world.deps, [VERSION, '--out', 'rel/assets']), 0);
		deepStrictEqual(
			world.calls.find(([op]) => op === 'make_dir'),
			['make_dir', join(CWD, 'rel/assets')]
		);
		strictEqual(text(world.files.get(join(CWD, 'rel/assets/SHA512SUMS')))?.endsWith('\n'), true);
	});
});

describe('the registry window', () => {
	test('a failed read is retried after the pause, loudly, until it succeeds', async () => {
		const world = fake_world({
			npm: (args, attempt) =>
				args[0] === 'view' && attempt < 3
					? { code: 1, stderr: ' npm error code E404 \n' }
					: undefined
		});
		strictEqual(await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]), 0);
		deepStrictEqual(world.sleeps, [REGISTRY_RETRY_MS, REGISTRY_RETRY_MS]);
		deepStrictEqual(world.err, [
			`  npm view @fuzdev/tsv@${VERSION} optionalDependencies --json failed (attempt 1/30) — retrying in 20s`,
			`  npm view @fuzdev/tsv@${VERSION} optionalDependencies --json failed (attempt 2/30) — retrying in 20s`
		]);
		deepStrictEqual(world.runs.slice(0, 4), [VIEW, VIEW, VIEW, pack(GNU)]);
	});

	test('thirty attempts twenty seconds apart, then a failure naming the last error', async () => {
		strictEqual(REGISTRY_ATTEMPTS, 30);
		strictEqual(REGISTRY_RETRY_MS, 20_000);
		const world = fake_world({
			npm: () => ({ code: 1, stderr: '\nnpm error code E404\n' })
		});
		strictEqual(await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]), 1);
		strictEqual(world.runs.length, 30);
		strictEqual(world.sleeps.length, 29);
		strictEqual(world.err.length, 30);
		strictEqual(
			world.err.at(-2),
			`  npm view @fuzdev/tsv@${VERSION} optionalDependencies --json failed (attempt 29/30) — retrying in 20s`
		);
		strictEqual(
			world.err.at(-1),
			`FAIL: npm view @fuzdev/tsv@${VERSION} optionalDependencies --json failed after 30 attempts:\nnpm error code E404`
		);
		deepStrictEqual(
			world.calls.filter(([op]) => op === 'make_dir' || op === 'make_temp_dir'),
			[]
		);
	});

	test('npm pack is retried the same way', async () => {
		const world = fake_world({
			npm: (args, attempt) => (args[0] === 'pack' && attempt === 1 ? { code: 1 } : undefined)
		});
		strictEqual(await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]), 0);
		deepStrictEqual(world.runs.slice(0, 3), [VIEW, pack(GNU), pack(GNU)]);
		deepStrictEqual(world.sleeps, [REGISTRY_RETRY_MS, REGISTRY_RETRY_MS]);
	});

	test('tar is not retried', async () => {
		const world = fake_world({ tar: () => ({ code: 2, stderr: 'tar: broken\n' }) });
		strictEqual(await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]), 1);
		deepStrictEqual(world.sleeps, []);
		deepStrictEqual(world.err, [
			`FAIL: tar -xzf ${PACK_DIR}/fuzdev-tsv-linux-x64-gnu-${VERSION}.tgz -C ${PACK_DIR} package/tsv failed:\ntar: broken`
		]);
		deepStrictEqual(world.calls.at(-1), ['remove_dir', PACK_DIR]);
	});
});

describe('refusals', () => {
	test('a loader naming no platforms fails before anything is made', async () => {
		for (const stdout of ['', '{}']) {
			const world = fake_world({ npm: (args) => (args[0] === 'view' ? { stdout } : undefined) });
			strictEqual(await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]), 1);
			deepStrictEqual(world.err, [
				`FAIL: @fuzdev/tsv@${VERSION} names no optionalDependencies — is the published loader the staged one?`
			]);
			deepStrictEqual(
				world.calls.filter(([op]) => op === 'make_dir'),
				[]
			);
		}
	});

	test('a pin off the version is refused, the temp dir removed', async () => {
		const world = fake_world({ optional: { [GNU]: VERSION, [WIN]: '1.2.2' } });
		strictEqual(await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]), 1);
		deepStrictEqual(world.err, [
			`FAIL: @fuzdev/tsv@${VERSION} pins ${WIN}@1.2.2 — versions must move in lockstep`
		]);
		deepStrictEqual(world.calls.at(-1), ['remove_dir', PACK_DIR]);
	});

	test('a name that is no platform package, or whose triple holds a path, is refused', async () => {
		for (const name of [
			'left-pad',
			'@fuzdev/tsv-',
			'@fuzdev/tsv-../../x',
			'@fuzdev/tsv-a/b',
			'@fuzdev/tsv-Linux'
		]) {
			const world = fake_world({ optional: { [name]: VERSION } });
			strictEqual(await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]), 1);
			deepStrictEqual(world.err, [
				`FAIL: @fuzdev/tsv@${VERSION} names ${name}, which is not a @fuzdev/tsv-<triple> platform package`
			]);
			deepStrictEqual(
				world.runs.filter(([, op]) => op === 'pack'),
				[]
			);
		}
	});

	test('a pack that reports no tarball, or one that is no file name, is refused', async () => {
		for (const [stdout, message] of [
			['[]', `FAIL: npm pack ${GNU}@${VERSION} reported no tarball`],
			['[{}]', `FAIL: npm pack ${GNU}@${VERSION} reported no tarball`],
			[
				'[{"filename": "../x.tgz"}]',
				`FAIL: npm pack ${GNU}@${VERSION} reported ../x.tgz, which is no file name`
			],
			[
				'[{"filename": ".."}]',
				`FAIL: npm pack ${GNU}@${VERSION} reported .., which is no file name`
			],
			[
				'[{"filename": "a\\\\b.tgz"}]',
				`FAIL: npm pack ${GNU}@${VERSION} reported a\\b.tgz, which is no file name`
			]
		] as const) {
			const world = fake_world({ npm: (args) => (args[0] === 'pack' ? { stdout } : undefined) });
			strictEqual(await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]), 1);
			deepStrictEqual(world.err, [message]);
			deepStrictEqual(
				world.runs.filter(([program]) => program === 'tar'),
				[]
			);
		}
	});

	test('a binary the tarball holds as a symlink is never copied', async () => {
		const world = fake_world({
			packages: {
				...PACKAGES,
				[GNU]: {
					filename: PACKAGES[GNU]!.filename,
					tarball: new Map([['package/tsv', { symlink: '/etc/passwd' }]])
				}
			}
		});
		strictEqual(await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]), 1);
		deepStrictEqual(world.err, [`FAIL: package/tsv in ${GNU}@${VERSION} is not a regular file`]);
		deepStrictEqual(
			world.calls.filter(([op]) => op === 'copy_file'),
			[]
		);
	});

	test('a failure midway keeps the assets already written, writes no sums, removes the temp dir', async () => {
		const world = fake_world({
			npm: (args) =>
				args[0] === 'pack' && args[1]!.startsWith(WIN) ? { code: 1, stderr: 'E500' } : undefined
		});
		strictEqual(await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]), 1);
		deepStrictEqual(files_in(world, OUT), { 'tsv-linux-x64-gnu': 'abc' });
		strictEqual(
			world.err.at(-1),
			`FAIL: npm pack ${WIN}@${VERSION} --min-release-age=0 --pack-destination ${PACK_DIR} --json failed after 30 attempts:\nE500`
		);
		deepStrictEqual(world.calls.at(-1), ['remove_dir', PACK_DIR]);
	});

	test('npm output that is no JSON fails the run', async () => {
		const world = fake_world({
			npm: (args) => (args[0] === 'view' ? { stdout: 'not json' } : undefined)
		});
		strictEqual(await run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT]), 1);
		strictEqual(world.err.length, 1);
		strictEqual(world.err[0]!.startsWith('FAIL: '), true);
	});
});

describe('arguments', () => {
	test('a missing tag or --out is a usage error, run nothing', async () => {
		for (const argv of [[], [`v${VERSION}`], ['--out', OUT], [`v${VERSION}`, '--out', '']]) {
			const world = fake_world();
			strictEqual(await run_release_assets(world.deps, argv), 1);
			deepStrictEqual(world.err, ['usage: release_assets.ts v<version> --out <dir>']);
			deepStrictEqual(world.calls, []);
		}
	});

	test('a tag that is no version fails, run nothing', async () => {
		const world = fake_world();
		strictEqual(await run_release_assets(world.deps, ['latest', '--out', OUT]), 1);
		deepStrictEqual(world.err, ['FAIL: "latest" is not a v<major>.<minor>.<patch> version']);
		deepStrictEqual(world.calls, []);
	});

	test('an unknown flag throws, run nothing', async () => {
		const world = fake_world();
		await rejects(
			run_release_assets(world.deps, [`v${VERSION}`, '--out', OUT, '--dry-run']),
			TypeError
		);
		deepStrictEqual(world.calls, []);
	});

	test("tar's arguments take the one member into the directory", () => {
		deepStrictEqual(tar_extract_args('/t/x.tgz', '/t', 'package/tsv'), [
			'-xzf',
			'/t/x.tgz',
			'-C',
			'/t',
			'package/tsv'
		]);
	});
});
