/**
 * The N-API staging over a fake file system: what each package holds, where every path
 * resolves, which binary gets its mode, and the exits a missing binary or a bad argument
 * gives.
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';
import { join } from 'node:path';
import { describe, test } from 'vitest';

import {
	type BuildNapiPackagesDeps,
	native_library_filename,
	platform_fields,
	run_build_napi_packages,
	SUPPORTED_TRIPLES
} from './build_napi_packages.ts';
import { ALL_FAMILIES, facade_files } from './npm_facade.ts';
import { THIRD_PARTY_NOTICE } from './npm_metadata.ts';

const CHECKOUT = join('/', 'repo');
const at = (path: string): string => join(CHECKOUT, path);

const CARGO_TOML = '[workspace.package]\nversion = "1.2.3"\n';

interface FakeWorld {
	deps: BuildNapiPackagesDeps;
	/** Every call, in order, as `[op, ...args]`. */
	calls: Array<Array<unknown>>;
	/** The files written or copied, by path: their text, or `copy of <from>`. */
	files: Map<string, string>;
	out: Array<string>;
	err: Array<string>;
}

/** A fake world: `Cargo.toml` reads `cargo_toml`, every copy succeeds but from a path in
 * `missing`, and a staged addon is 1.5 MB, any other file 2.5 MB. */
const fake_world = (
	options: {
		platform?: string;
		host_triple?: string;
		cargo_toml?: string;
		missing?: Array<string>;
	} = {}
): FakeWorld => {
	const calls: Array<Array<unknown>> = [];
	const files = new Map<string, string>();
	const out: Array<string> = [];
	const err: Array<string> = [];
	const missing = new Set(options.missing ?? []);
	return {
		calls,
		files,
		out,
		err,
		deps: {
			checkout: CHECKOUT,
			platform: options.platform ?? 'linux',
			host_triple: options.host_triple ?? 'linux-x64-gnu',
			fs: {
				read_text: async (path) => {
					calls.push(['read_text', path]);
					if (path !== at('Cargo.toml')) throw new Error(`unexpected read of ${path}`);
					return options.cargo_toml ?? CARGO_TOML;
				},
				write_text: async (path, text) => {
					calls.push(['write_text', path]);
					files.set(path, text);
				},
				make_dir: async (path) => {
					calls.push(['make_dir', path]);
				},
				copy: async (from, to) => {
					calls.push(['copy', from, to]);
					if (missing.has(from)) throw new Error(`ENOENT: no such file, copyfile '${from}'`);
					files.set(to, `copy of ${from}`);
				},
				set_times: async (path) => {
					calls.push(['set_times', path]);
				},
				set_mode: async (path, mode) => {
					calls.push(['set_mode', path, mode]);
				},
				size: async (path) => {
					calls.push(['size', path]);
					return path.endsWith('tsv_napi.node') ? 1_500_000 : 2_500_000;
				}
			},
			print: (line) => out.push(line),
			print_error: (line) => err.push(line)
		}
	};
};

const LOADER = 'crates/tsv_napi/pkg/napi';
const PLATFORM = 'crates/tsv_napi/pkg/linux-x64-gnu';
const FLAGGED = ['--artifact', '/built/libtsv_napi.so', '--cli-artifact', '/built/tsv'];

const json_at = (world: FakeWorld, path: string): Record<string, unknown> =>
	JSON.parse(world.files.get(at(path))!);

describe('the loader package', () => {
	test('--loader-only stages the loader alone and exits 0', async () => {
		const world = fake_world();
		strictEqual(await run_build_napi_packages(world.deps, ['--loader-only']), 0);
		deepStrictEqual(world.out, [`Staged ${LOADER}: @fuzdev/tsv 1.2.3`]);
		deepStrictEqual(world.err, []);
		const touched = world.calls
			.flatMap(([, ...paths]) => paths)
			.filter((p) => typeof p === 'string');
		ok(!touched.some((p) => (p as string).includes('linux-x64-gnu')), 'no platform path touched');
	});

	test('every loader file is copied from the checkout and dated at the staging', async () => {
		const world = fake_world();
		await run_build_napi_packages(world.deps, ['--loader-only']);
		const expected: Array<[string, string]> = [
			['crates/tsv_napi/npm/index.js', 'index.js'],
			['crates/tsv_napi/npm/index.d.ts', 'index.d.ts'],
			['crates/tsv_napi/npm/platform.js', 'platform.js'],
			['crates/tsv_napi/npm/bin.js', 'bin.js'],
			['crates/tsv_napi/npm/README.md', 'README.md'],
			...facade_files(ALL_FAMILIES).map(({ source, published }): [string, string] => [
				source,
				published
			]),
			['crates/tsv_wasm/npm/cli.js', 'cli.js'],
			['LICENSE', 'LICENSE'],
			[THIRD_PARTY_NOTICE.source, THIRD_PARTY_NOTICE.published]
		];
		const staged = world.calls.filter(([op]) => op === 'copy' || op === 'set_times');
		deepStrictEqual(
			staged,
			expected.flatMap(([from, to]) => [
				['copy', at(from), at(`${LOADER}/${to}`)],
				['set_times', at(`${LOADER}/${to}`)]
			])
		);
		strictEqual(world.calls[1]![0], 'make_dir');
		strictEqual(world.calls[1]![1], at(LOADER));
	});

	test('its package.json pins every supported platform at the workspace version', async () => {
		const world = fake_world();
		await run_build_napi_packages(world.deps, ['--loader-only']);
		const pkg = json_at(world, `${LOADER}/package.json`);
		strictEqual(pkg.name, '@fuzdev/tsv');
		strictEqual(pkg.version, '1.2.3');
		deepStrictEqual(pkg.bin, { tsv: 'bin.js' });
		deepStrictEqual(
			pkg.optionalDependencies,
			Object.fromEntries(SUPPORTED_TRIPLES.map((t) => [`@fuzdev/tsv-${t}`, '1.2.3']))
		);
		ok(world.files.get(at(`${LOADER}/package.json`))!.endsWith('}\n'));
		ok(world.files.get(at(`${LOADER}/package.json`))!.includes('\n\t"name"'), 'tab-indented');
	});

	test('a Cargo.toml with no workspace version throws before staging anything', async () => {
		const world = fake_world({ cargo_toml: '[package]\nname = "x"\n' });
		await rejects(run_build_napi_packages(world.deps, ['--loader-only']), /workspace version/);
		deepStrictEqual(world.calls, [['read_text', at('Cargo.toml')]]);
	});
});

describe('the platform package', () => {
	test('stages the binaries the flags name, as given, and the files beside them', async () => {
		const world = fake_world();
		strictEqual(
			await run_build_napi_packages(world.deps, ['--triple', 'linux-x64-gnu', ...FLAGGED]),
			0
		);
		deepStrictEqual(world.err, []);
		strictEqual(world.files.get(at(`${PLATFORM}/tsv_napi.node`)), 'copy of /built/libtsv_napi.so');
		strictEqual(world.files.get(at(`${PLATFORM}/tsv`)), 'copy of /built/tsv');
		strictEqual(world.files.get(at(`${PLATFORM}/LICENSE`)), `copy of ${at('LICENSE')}`);
		strictEqual(
			world.files.get(at(`${PLATFORM}/${THIRD_PARTY_NOTICE.published}`)),
			`copy of ${at(THIRD_PARTY_NOTICE.source)}`
		);
		ok(world.files.get(at(`${PLATFORM}/README.md`))!.startsWith('# @fuzdev/tsv-linux-x64-gnu\n'));
		deepStrictEqual(world.out, [
			`Staged ${LOADER}: @fuzdev/tsv 1.2.3`,
			`Staged ${PLATFORM}: @fuzdev/tsv-linux-x64-gnu 1.2.3 (tsv_napi.node 1.50 MB, tsv 2.50 MB)`
		]);
	});

	test('its package.json selects the platform', async () => {
		const world = fake_world();
		await run_build_napi_packages(world.deps, ['--triple', 'linux-x64-musl', ...FLAGGED]);
		const pkg = json_at(world, 'crates/tsv_napi/pkg/linux-x64-musl/package.json');
		strictEqual(pkg.name, '@fuzdev/tsv-linux-x64-musl');
		strictEqual(pkg.version, '1.2.3');
		deepStrictEqual(pkg.os, ['linux']);
		deepStrictEqual(pkg.cpu, ['x64']);
		deepStrictEqual(pkg.libc, ['musl']);
		deepStrictEqual(pkg.files, ['tsv_napi.node', 'tsv', 'README.md', 'LICENSE', 'NOTICE']);
	});

	test('off Windows the CLI binary is made executable, after its copy', async () => {
		const world = fake_world({ platform: 'darwin' });
		await run_build_napi_packages(world.deps, ['--triple', 'darwin-arm64', ...FLAGGED]);
		const cli = at('crates/tsv_napi/pkg/darwin-arm64/tsv');
		deepStrictEqual(
			world.calls.filter(([op]) => op === 'set_mode'),
			[['set_mode', cli, 0o755]]
		);
		const copied = world.calls.findIndex(([op, , to]) => op === 'copy' && to === cli);
		const moded = world.calls.findIndex(([op]) => op === 'set_mode');
		ok(copied >= 0 && moded > copied);
	});

	test("on Windows no mode is set, and the defaults are cargo's Windows names", async () => {
		const world = fake_world({ platform: 'win32', host_triple: 'win32-x64' });
		strictEqual(await run_build_napi_packages(world.deps, []), 0);
		deepStrictEqual(
			world.calls.filter(([op]) => op === 'set_mode'),
			[]
		);
		const dir = 'crates/tsv_napi/pkg/win32-x64';
		strictEqual(
			world.files.get(at(`${dir}/tsv_napi.node`)),
			`copy of ${at('target/napi/tsv_napi.dll')}`
		);
		strictEqual(world.files.get(at(`${dir}/tsv.exe`)), `copy of ${at('target/release/tsv.exe')}`);
		deepStrictEqual(json_at(world, `${dir}/package.json`).os, ['win32']);
	});

	test('the default binaries are the checkout builds of the host, the triple the host one', async () => {
		const world = fake_world({ platform: 'darwin', host_triple: 'darwin-x64' });
		await run_build_napi_packages(world.deps, []);
		const dir = 'crates/tsv_napi/pkg/darwin-x64';
		strictEqual(
			world.files.get(at(`${dir}/tsv_napi.node`)),
			`copy of ${at('target/napi/libtsv_napi.dylib')}`
		);
		strictEqual(world.files.get(at(`${dir}/tsv`)), `copy of ${at('target/release/tsv')}`);
	});

	test('a relative flagged binary is read as given, not against the checkout', async () => {
		const world = fake_world();
		await run_build_napi_packages(world.deps, [
			'--artifact',
			'built/addon.so',
			'--cli-artifact',
			'built/tsv'
		]);
		strictEqual(world.files.get(at(`${PLATFORM}/tsv_napi.node`)), 'copy of built/addon.so');
		strictEqual(world.files.get(at(`${PLATFORM}/tsv`)), 'copy of built/tsv');
	});

	test('every path written is beneath the checkout', async () => {
		const world = fake_world();
		await run_build_napi_packages(world.deps, FLAGGED);
		const written = world.calls
			.filter(([op]) => op !== 'copy' && op !== 'read_text')
			.map(([, path]) => path as string);
		const copied_to = world.calls.filter(([op]) => op === 'copy').map(([, , to]) => to as string);
		for (const path of [...written, ...copied_to]) {
			ok(path.startsWith(at('crates/tsv_napi/pkg') + '/'), path);
		}
	});

	test('a missing addon fails 1, naming its path and build, before the CLI is staged', async () => {
		const world = fake_world({ missing: [at('target/napi/libtsv_napi.so')] });
		strictEqual(await run_build_napi_packages(world.deps, []), 1);
		deepStrictEqual(world.err, [
			"FAIL: cannot read the built cdylib at target/napi/libtsv_napi.so — run 'npm run build:napi' first",
			`Error: ENOENT: no such file, copyfile '${at('target/napi/libtsv_napi.so')}'`
		]);
		ok(!world.calls.some(([op, from]) => op === 'copy' && from === at('target/release/tsv')));
		deepStrictEqual(world.out, [`Staged ${LOADER}: @fuzdev/tsv 1.2.3`]);
	});

	test('a missing CLI binary fails 1, naming the path the flag gave', async () => {
		const world = fake_world({ missing: ['/built/tsv'] });
		strictEqual(await run_build_napi_packages(world.deps, FLAGGED), 1);
		strictEqual(
			world.err[0],
			"FAIL: cannot read the built CLI binary at /built/tsv — run 'cargo build -p tsv_cli --release' first"
		);
		deepStrictEqual(
			world.calls.filter(([op]) => op === 'set_mode'),
			[]
		);
	});

	test('a triple outside the supported list is refused before anything is read', async () => {
		for (const triple of [
			'bogus',
			'linux-riscv64-gnu',
			'linux-arm64-musl',
			'win32-arm64',
			'LINUX-X64-GNU',
			'linux-x64-gnu/'
		]) {
			const world = fake_world();
			await rejects(
				run_build_napi_packages(world.deps, ['--triple', triple, ...FLAGGED]),
				new RegExp(`^Error: unsupported triple '${triple}' \\(supported: linux-x64-gnu, `)
			);
			deepStrictEqual(world.calls, []);
		}
	});

	test('a triple that climbs out of pkg/ is refused before anything is read', async () => {
		for (const triple of ['../escape', '../../../tmp/x', 'linux-x64-gnu/../../escape']) {
			const world = fake_world();
			await rejects(
				run_build_napi_packages(world.deps, ['--triple', triple, ...FLAGGED]),
				/unsupported triple/
			);
			deepStrictEqual(world.calls, []);
		}
	});

	test('an unsupported host triple is refused when no --triple is given', async () => {
		const world = fake_world({ host_triple: 'freebsd-x64' });
		await rejects(run_build_napi_packages(world.deps, []), /unsupported triple 'freebsd-x64'/);
		deepStrictEqual(world.calls, []);
	});

	test('--loader-only stages on a host whose triple is unsupported', async () => {
		const world = fake_world({ host_triple: 'freebsd-x64' });
		strictEqual(await run_build_napi_packages(world.deps, ['--loader-only']), 0);
		deepStrictEqual(world.out, [`Staged ${LOADER}: @fuzdev/tsv 1.2.3`]);
	});
});

test('a flag parseArgs refuses throws before anything is read', async () => {
	for (const argv of [['--bogus'], ['extra'], ['--triple']]) {
		const world = fake_world();
		await rejects(run_build_napi_packages(world.deps, argv), TypeError);
		deepStrictEqual(world.calls, []);
	}
});

test('platform_fields maps a triple to npm selection fields', () => {
	deepStrictEqual(platform_fields('linux-x64-gnu'), {
		os: ['linux'],
		cpu: ['x64'],
		libc: ['glibc']
	});
	deepStrictEqual(platform_fields('linux-arm64-musl'), {
		os: ['linux'],
		cpu: ['arm64'],
		libc: ['musl']
	});
	deepStrictEqual(platform_fields('win32-x64'), { os: ['win32'], cpu: ['x64'] });
});

test("native_library_filename is cargo's cdylib name per platform", () => {
	strictEqual(native_library_filename('linux'), 'libtsv_napi.so');
	strictEqual(native_library_filename('darwin'), 'libtsv_napi.dylib');
	strictEqual(native_library_filename('win32'), 'tsv_napi.dll');
});
