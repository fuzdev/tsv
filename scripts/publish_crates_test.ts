import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import { FsError } from '@fuzdev/gro/fs_error.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { test } from 'vitest';

import {
	type PublishCratesDeps,
	release_checkout_refusals,
	run_publish_crates
} from './publish_crates.ts';
import { CRATES_IO_USER_AGENT, type PackageStateDeps } from './release/crates.ts';

const API = 'https://crates.io/api/v1/crates';
const SRC = '/home/u/.cargo/registry/src';

/** A workspace of two publishable crates (`tsv_b` depends on `tsv_a`, and is listed first)
 * and one that isn't. */
const FILES: Record<string, string> = {
	'Cargo.toml':
		'[workspace]\nmembers = ["crates/tsv_b", "crates/tsv_a", "crates/tsv_c"]\n\n' +
		'[workspace.package]\nversion = "0.8.0"\n',
	'crates/tsv_a/Cargo.toml':
		'[package]\nname = "tsv_a"\n\n[package.metadata.docs.rs]\nall-features = true\n',
	'crates/tsv_b/Cargo.toml':
		'[package]\nname = "tsv_b"\n\n[package.metadata.docs.rs]\nfeatures = ["x"]\n',
	'crates/tsv_c/Cargo.toml': '[package]\nname = "tsv_c"\npublish = false\n'
};

const METADATA = {
	packages: [
		{
			name: 'tsv_b',
			version: '0.8.0',
			publish: null,
			dependencies: [{ name: 'tsv_a', kind: null }]
		},
		{ name: 'tsv_a', version: '0.8.0', publish: null, dependencies: [] },
		{ name: 'tsv_c', version: '0.8.0', publish: [], dependencies: [{ name: 'tsv_a', kind: null }] }
	],
	target_directory: '/w/target'
};

interface Reply {
	code?: number;
	signal?: string;
	stdout?: string;
	stderr?: string;
	/** Whether the capture was cut short. */
	truncated?: boolean;
}

type Call =
	| { to: 'cargo' | 'git'; method: string; args: Array<string>; cwd: unknown }
	| { to: 'fetch'; url: string; agent: string | null }
	| { to: 'cleanup'; target_directory: string };

/** The answer a fake gives every call it isn't told about: a success. */
type Replies = (args: ReadonlyArray<string>) => Reply | undefined;

const fake_program = (to: 'cargo' | 'git', calls: Array<Call>, replies: Replies): SpawnProgram => {
	const answer = (method: string, args: ReadonlyArray<string>, options?: { cwd?: unknown }) => {
		calls.push({ to, method, args: [...args], cwd: options?.cwd });
		const reply = replies(args) ?? {};
		return reply.signal === undefined
			? { kind: 'exited' as const, code: reply.code ?? 0, reply }
			: { kind: 'signaled' as const, signal: reply.signal, reply };
	};
	return {
		name: to,
		path: `/usr/bin/${to}`,
		out: async (args: ReadonlyArray<string>, options?: { cwd?: unknown }) => {
			const { reply, ...result } = answer('out', args, options);
			return {
				...result,
				stdout: reply.stdout ?? '',
				stderr: reply.stderr ?? '',
				truncated: reply.truncated ?? false
			};
		},
		run: async (args: ReadonlyArray<string>, options?: { cwd?: unknown }) => {
			const { reply: _, ...result } = answer('run', args, options);
			return result;
		},
		out_bytes: () => Promise.reject(new Error('out_bytes is not a call publish:crates makes')),
		start: () => Promise.reject(new Error('start is a --wetrun call, which no test makes'))
	} as unknown as SpawnProgram;
};

const json =
	(body: unknown, status = 200) =>
	() =>
		new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

/** The registry: `tsv_a` has releases below 0.8.0, `tsv_b` is not on crates.io. */
const REGISTRY: Record<string, () => Response> = {
	[`${API}/tsv_a`]: json({
		versions: [
			{ num: '0.7.1', yanked: false },
			{ num: '0.7.0', yanked: false }
		]
	}),
	[`${API}/tsv_b`]: json({ errors: [] }, 404)
};

/** A fake cleanup: nothing to clear unless `state` says otherwise. */
const NOTHING_STALE: PackageStateDeps = {
	remove_package_dir: () => Promise.reject(new FsError('not_found', '/w/target/package')),
	registry_src: null
};

const make_deps = (
	options: {
		files?: Record<string, string>;
		cargo?: Replies | null;
		git?: Replies | null;
		registry?: Record<string, () => Response>;
		state?: () => Promise<PackageStateDeps>;
	} = {}
) => {
	const calls: Array<Call> = [];
	const out: Array<string> = [];
	const err: Array<string> = [];
	const files = options.files ?? FILES;
	const read = (path: string): string => {
		const text = files[path];
		if (text === undefined) throw new FsError('not_found', `/w/${path}`);
		return text;
	};
	const checkout = {
		path: '/w',
		read_text_sync: read,
		read_text: async (path: string) => read(path)
	} as unknown as FsReadDir;
	const cargo_replies: Replies = (args) =>
		args[0] === 'metadata'
			? { stdout: `${JSON.stringify(METADATA)}\n` }
			: args.join(' ') === 'semver-checks --version'
				? { stdout: 'cargo-semver-checks 0.51.0\n' }
				: undefined;
	const registry = options.registry ?? REGISTRY;
	const deps: PublishCratesDeps = {
		checkout,
		cargo:
			options.cargo === null
				? null
				: fake_program('cargo', calls, (args) => options.cargo?.(args) ?? cargo_replies(args)),
		git: options.git === null ? null : fake_program('git', calls, options.git ?? (() => undefined)),
		registry: {
			fetch: (url, init) => {
				calls.push({ to: 'fetch', url, agent: new Headers(init.headers).get('User-Agent') });
				const route = registry[url];
				return route
					? Promise.resolve(route())
					: Promise.reject(new TypeError('network unreachable'));
			}
		},
		registry_src_path: SRC,
		open_package_state: (target_directory) => {
			calls.push({ to: 'cleanup', target_directory });
			return options.state?.() ?? Promise.resolve(NOTHING_STALE);
		},
		print: (line) => out.push(line),
		print_error: (line) => err.push(line),
		write_error: () => Promise.reject(new Error('write_error is a --wetrun call')),
		sleep: () => Promise.reject(new Error('sleep is a --wetrun call')),
		now: () => 0
	};
	return { deps, calls, out, err, checkout };
};

const METADATA_ARGS = ['metadata', '--no-deps', '--format-version', '1', '--locked'];
const SEMVER_A = [
	'semver-checks',
	'--package',
	'tsv_a',
	'--baseline-version',
	'0.7.1',
	'--all-features'
];
const PUBLISH_DRY = ['publish', '--workspace', '--locked', '--dry-run'];

/** The calls, cwd checked to be the checkout and dropped. */
const calls_of = (setup: ReturnType<typeof make_deps>) =>
	setup.calls.map((call) => {
		if (call.to === 'cargo' || call.to === 'git') {
			strictEqual(
				call.cwd,
				setup.checkout,
				`${call.to} ${call.args.join(' ')} runs in the checkout`
			);
			return `${call.to} ${call.method} ${call.args.join(' ')}`;
		}
		if (call.to === 'fetch') {
			strictEqual(call.agent, CRATES_IO_USER_AGENT);
			return `fetch ${call.url}`;
		}
		if (call.to === 'cleanup') return `cleanup ${call.target_directory}`;
		throw new Error(`unknown call ${JSON.stringify(call)}`);
	});

test('a dry run: metadata, the registry, the gate, the cleanup, then cargo publish --dry-run', async () => {
	const setup = make_deps();
	strictEqual(await run_publish_crates(setup.deps, []), 0);
	deepStrictEqual(calls_of(setup), [
		`cargo out ${METADATA_ARGS.join(' ')}`,
		`fetch ${API}/tsv_a`,
		`fetch ${API}/tsv_b`,
		'cargo out semver-checks --version',
		`cargo run ${SEMVER_A.join(' ')}`,
		'cleanup /w/target',
		`cargo run ${PUBLISH_DRY.join(' ')}`
	]);
	deepStrictEqual(setup.out, [
		'=== publish:crates (dry run) ===',
		'  version 0.8.0; 2 crates, in dependency order (cargo orders its own uploads): tsv_a → tsv_b',
		'\n=== Registry (crates.io API) ===',
		'  tsv_a: 2 version(s) on crates.io; v0.8.0 missing',
		'  tsv_b: not on crates.io; v0.8.0 missing',
		'\n=== Public-API gate (cargo semver-checks, docs.rs feature sets) ===',
		'  cargo-semver-checks 0.51.0',
		`\n  cargo ${SEMVER_A.join(' ')}`,
		'\n=== Publish — dry run (cargo publish --workspace --dry-run) ===',
		'  no stale packaging state to clear',
		`  cargo ${PUBLISH_DRY.join(' ')}`,
		'\n=== Summary ===',
		'  publish: dry run OK (v0.8.0, 2 crates)',
		'  semver-checks: 1 graded, 0 failed'
	]);
	deepStrictEqual(setup.err, [
		'  ⚠ SKIP tsv_b: not on crates.io yet, so no published baseline — its public API is NOT graded',
		'  ⚠ semver-checks SKIPPED 1 of 2 crate(s): tsv_b — their public API was NOT graded'
	]);
});

test('--no-semver-checks reads nothing from the registry and runs no gate', async () => {
	const setup = make_deps({ registry: {} });
	strictEqual(await run_publish_crates(setup.deps, ['--no-semver-checks']), 0);
	deepStrictEqual(calls_of(setup), [
		`cargo out ${METADATA_ARGS.join(' ')}`,
		'cleanup /w/target',
		`cargo run ${PUBLISH_DRY.join(' ')}`
	]);
	deepStrictEqual(setup.out, [
		'=== publish:crates (dry run) ===',
		'  version 0.8.0; 2 crates, in dependency order (cargo orders its own uploads): tsv_a → tsv_b',
		'\n=== Registry (crates.io API) — SKIPPED: a dry run with --no-semver-checks reads nothing from it ===',
		'\n=== Publish — dry run (cargo publish --workspace --dry-run) ===',
		'  no stale packaging state to clear',
		`  cargo ${PUBLISH_DRY.join(' ')}`,
		'\n=== Summary ===',
		'  publish: dry run OK (v0.8.0, 2 crates)',
		'  semver-checks: 0 graded, 0 failed'
	]);
	deepStrictEqual(setup.err, [
		'\n=== Public-API gate — SKIPPED (--no-semver-checks) ===',
		"  ⚠ cargo semver-checks did NOT run: no crate's public API was graded",
		'  ⚠ semver-checks SKIPPED 2 of 2 crate(s): tsv_a, tsv_b — their public API was NOT graded'
	]);
});

test('--allow-dirty, --bump, and a matching --expect-tag reach the argv they name', async () => {
	const setup = make_deps({
		registry: {
			...REGISTRY,
			[`${API}/tsv_a`]: json({ versions: [{ num: '0.8.0', yanked: false }] })
		}
	});
	const argv = ['--allow-dirty', '--bump', 'minor', '--expect-tag', 'v0.8.0'];
	strictEqual(await run_publish_crates(setup.deps, argv), 0);
	const runs = calls_of(setup).filter((call) => call.startsWith('cargo run'));
	deepStrictEqual(runs, [
		'cargo run semver-checks --package tsv_a --baseline-version 0.8.0 --all-features --release-type major',
		`cargo run ${PUBLISH_DRY.join(' ')} --allow-dirty`
	]);
	ok(setup.out.includes('  tsv_a: 1 version(s) on crates.io; v0.8.0 live'));
});

test('a crate failing the gate fails the run, after the dry run still packages', async () => {
	const setup = make_deps({
		cargo: (args) =>
			args[0] === 'semver-checks' && args[1] === '--package' ? { code: 1 } : undefined
	});
	strictEqual(await run_publish_crates(setup.deps, []), 1);
	ok(calls_of(setup).includes(`cargo run ${PUBLISH_DRY.join(' ')}`));
	deepStrictEqual(setup.out.slice(-3), [
		'\n=== Summary ===',
		'  publish: dry run OK (v0.8.0, 2 crates)',
		'  semver-checks: 1 graded, 1 failed (tsv_a)'
	]);
});

test('cargo publish --dry-run failing, or ended by a signal, fails the run with its code', async () => {
	for (const [reply, code] of [
		[{ code: 101 }, 101],
		[{ signal: 'SIGTERM' }, 143]
	] as const) {
		const setup = make_deps({ cargo: (args) => (args[0] === 'publish' ? reply : undefined) });
		strictEqual(await run_publish_crates(setup.deps, ['--no-semver-checks']), 1);
		deepStrictEqual(setup.err.slice(0, 3), [
			'\n=== Public-API gate — SKIPPED (--no-semver-checks) ===',
			"  ⚠ cargo semver-checks did NOT run: no crate's public API was graded",
			`  FAIL: cargo publish --dry-run exited ${code}`
		]);
		ok(setup.out.includes('  publish: FAILED (v0.8.0, 2 crates)'));
	}
});

test('a missing cargo fails at the first call that needs it', async () => {
	const setup = make_deps({ cargo: null });
	strictEqual(await run_publish_crates(setup.deps, []), 1);
	deepStrictEqual(setup.calls, []);
	deepStrictEqual(setup.out, ['=== publish:crates (dry run) ===']);
	deepStrictEqual(setup.err, ['FAIL: cargo metadata --locked failed:\ncargo: command not found']);
});

test('the preflight refusals each exit 1 before anything publishes', async () => {
	const cases: Array<[Parameters<typeof make_deps>[0], Array<string>, string, number]> = [
		[{}, ['--bump', 'huge'], 'FAIL: --bump must be patch, minor or major, not "huge"', 0],
		[{}, ['--wait'], 'FAIL: --wait is for a --wetrun; a dry run hits no rate limit', 0],
		[
			{
				cargo: (args) =>
					args[0] === 'metadata' ? { code: 101, stderr: '  error: lock\n' } : undefined
			},
			[],
			'FAIL: cargo metadata --locked failed:\nerror: lock',
			1
		],
		[
			{
				files: {
					...FILES,
					'crates/tsv_c/Cargo.toml': '[package]\nname = "tsv_c"\n\n[package.metadata.docs.rs]\n'
				}
			},
			[],
			"FAIL: cargo's publishable set and docsrs_crates.ts's disagree on tsv_c",
			1
		],
		[
			{ files: { ...FILES, 'Cargo.toml': FILES['Cargo.toml']!.replace('0.8.0', '0.9.0') } },
			[],
			'FAIL: every publishable crate takes the workspace version 0.9.0: tsv_b is 0.8.0, tsv_a is 0.8.0',
			1
		],
		[
			{ files: { ...FILES, 'Cargo.toml': FILES['Cargo.toml']!.replace('version = "0.8.0"', '') } },
			[],
			'FAIL: no [workspace.package] version in Cargo.toml',
			1
		],
		[
			{},
			['--expect-tag', 'v0.7.9'],
			'FAIL: the workspace version v0.8.0 does not match the tag v0.7.9 — the checkout and the tag disagree',
			1
		],
		[
			{
				cargo: (args) => (args.join(' ') === 'semver-checks --version' ? { code: 101 } : undefined)
			},
			[],
			'FAIL: cargo semver-checks is not installed (`cargo install --locked cargo-semver-checks`); --no-semver-checks skips the gate',
			2
		]
	];
	for (const [options, argv, message, calls] of cases) {
		const setup = make_deps(options);
		strictEqual(await run_publish_crates(setup.deps, argv), 1, message);
		deepStrictEqual(setup.err, [message]);
		ok(!calls_of(setup).some((call) => call.startsWith('cargo run publish')), message);
		ok(!calls_of(setup).includes('cleanup /w/target'), message);
		strictEqual(setup.calls.filter((call) => call.to === 'cargo').length, calls, message);
	}
});

test('a flag parseArgs refuses throws before anything runs', async () => {
	for (const argv of [['--nope'], ['--', '--allow-dirty'], ['stray']]) {
		const setup = make_deps();
		await rejects(run_publish_crates(setup.deps, argv), TypeError);
		deepStrictEqual(setup.calls, []);
		deepStrictEqual(setup.out, []);
	}
});

test('a registry lookup that fails is fatal: no gate, no cleanup, no publish', async () => {
	for (const [registry, error] of [
		[
			{ [`${API}/tsv_b`]: REGISTRY[`${API}/tsv_b`]! },
			`GET ${API}/tsv_a failed: TypeError: network unreachable`
		],
		[{ ...REGISTRY, [`${API}/tsv_b`]: json({}, 500) }, `GET ${API}/tsv_b answered 500: {}`]
	] as const) {
		const setup = make_deps({ registry });
		strictEqual(await run_publish_crates(setup.deps, []), 1);
		deepStrictEqual(setup.err, [
			`FAIL: ${error}\n  no publish or gate decision is made without it`
		]);
		deepStrictEqual(
			calls_of(setup).filter((call) => !call.startsWith('fetch')),
			[`cargo out ${METADATA_ARGS.join(' ')}`]
		);
	}
});

test('the cleanup reports what it removed, and a cleanup that fails stops the run', async () => {
	let unpacked = true;
	const removed = make_deps({
		state: async () => ({
			remove_package_dir: async () => {},
			registry_src: {
				list: (path?: string) =>
					(async function* () {
						if (path === undefined) yield { name: '-088a2fcb40cb7e05', kind: 'directory' as const };
						else if (unpacked) yield { name: 'tsv_a-0.8.0', kind: 'directory' as const };
					})(),
				stat: async () => ({ kind: 'directory' as const, size: 0, mtime_ms: 0, mode: 0o755 }),
				remove: async (path: string) => {
					if (path.endsWith('/tsv_a-0.8.0')) unpacked = false;
				}
			}
		})
	});
	strictEqual(await run_publish_crates(removed.deps, ['--no-semver-checks']), 0);
	ok(
		removed.out.includes(
			`  cleared stale packaging state: target/package, 1 overlay unpack(s) of v0.8.0 and 1 emptied overlay dir(s) under ${SRC}`
		),
		removed.out.join('\n')
	);
	for (const failure of [
		new Error('outside the grant'),
		new FsError('permission_denied', '/w/target/package')
	]) {
		const setup = make_deps({
			state: async () => ({ remove_package_dir: () => Promise.reject(failure), registry_src: null })
		});
		strictEqual(await run_publish_crates(setup.deps, ['--no-semver-checks']), 1);
		deepStrictEqual(setup.err.slice(-1), [
			`FAIL: could not clear the stale packaging state (/w/target/package, ${SRC}): ${String(failure)}` +
				'\n  a target directory or cargo home outside its grant is refused, not skipped'
		]);
		ok(!calls_of(setup).some((call) => call.startsWith('cargo run publish')));
	}
	// opening the directories failing is the same failure
	const unopened = make_deps({ state: () => Promise.reject(new FsError('not_a_directory', SRC)) });
	strictEqual(await run_publish_crates(unopened.deps, ['--no-semver-checks']), 1);
	ok(unopened.err.at(-1)!.startsWith('FAIL: could not clear the stale packaging state'));
});

test('release_checkout_refusals reads HEAD, the tag, and git status in the checkout', async () => {
	const at = 'a'.repeat(40);
	const clean = make_deps({
		git: (args) => (args[0] === 'rev-parse' ? { stdout: `${at}\n` } : { stdout: '' })
	});
	deepStrictEqual(await release_checkout_refusals(clean.deps, '0.8.0'), { ok: true, refusals: [] });
	deepStrictEqual(calls_of(clean), [
		'git out rev-parse HEAD',
		'git out rev-parse --verify --quiet v0.8.0^{commit}',
		'git out status --porcelain'
	]);
	const moved = make_deps({
		git: (args) =>
			args[1] === 'HEAD'
				? { stdout: 'b'.repeat(40) }
				: args[0] === 'rev-parse'
					? { code: 1 }
					: { stdout: ' M Cargo.toml\n?? stray.rs\n' }
	});
	const answer = await release_checkout_refusals(moved.deps, '0.8.0');
	ok(answer.ok);
	strictEqual(answer.refusals.length, 2);
	ok(answer.refusals[0]!.includes('no local tag v0.8.0'), answer.refusals[0]);
	ok(answer.refusals[1]!.endsWith(':\n     M Cargo.toml\n    ?? stray.rs'), answer.refusals[1]);
	const failed = make_deps({
		git: (args) => (args[0] === 'status' ? { code: 128, stderr: 'fatal: no repo\n' } : undefined)
	});
	deepStrictEqual(await release_checkout_refusals(failed.deps, '0.8.0'), {
		ok: false,
		error: 'git status failed:\nfatal: no repo'
	});
	const missing = make_deps({ git: null });
	deepStrictEqual(await release_checkout_refusals(missing.deps, '0.8.0'), {
		ok: false,
		error: 'git status failed:\ngit: command not found'
	});
	// a status cut short is no answer: read as it stands, it could miss the dirty lines
	const truncated = make_deps({
		git: (args) =>
			args[0] === 'rev-parse'
				? { stdout: `${at}\n` }
				: { stdout: '', stderr: 'cut', truncated: true }
	});
	deepStrictEqual(await release_checkout_refusals(truncated.deps, '0.8.0'), {
		ok: false,
		error: 'git status failed:\ncut'
	});
});
