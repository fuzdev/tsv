/**
 * `run_publish_crates` with `--wetrun`, over in-process fakes only: fake `cargo` and `git`
 * program handles, a fake crates.io, a fake cleanup, a recorded stderr, and an injected
 * sleep and clock. Nothing here imports a host module, so nothing can spawn, write, or reach
 * the network; the entry is never run.
 *
 * @module
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import { FsError } from '@fuzdev/gro/fs_error.ts';
import type { SpawnProgram, SpawnResult } from '@fuzdev/gro/spawn.ts';
import { test } from 'vitest';

import { type PublishCratesDeps, run_publish_crates } from './publish_crates.ts';
import type { PackageStateDeps } from './release/crates.ts';

const API = 'https://crates.io/api/v1/crates';
const SRC = '/home/u/.cargo/registry/src';
const AT = 'a'.repeat(40);

const FILES: Record<string, string> = {
	'Cargo.toml':
		'[workspace]\nmembers = ["crates/tsv_b", "crates/tsv_a"]\n\n' +
		'[workspace.package]\nversion = "0.8.0"\n',
	'crates/tsv_a/Cargo.toml':
		'[package]\nname = "tsv_a"\n\n[package.metadata.docs.rs]\nall-features = true\n',
	'crates/tsv_b/Cargo.toml':
		'[package]\nname = "tsv_b"\n\n[package.metadata.docs.rs]\nfeatures = ["x"]\n'
};

const METADATA = {
	packages: [
		{
			name: 'tsv_b',
			version: '0.8.0',
			publish: null,
			dependencies: [{ name: 'tsv_a', kind: null }]
		},
		{ name: 'tsv_a', version: '0.8.0', publish: null, dependencies: [] }
	],
	target_directory: '/w/target'
};

/** One `cargo publish` attempt's answer: its stderr chunks, as bytes, and how it ended. */
interface Attempt {
	stderr: Array<Uint8Array>;
	result: SpawnResult;
}

type Versions = Array<{ num: string; yanked: boolean }> | null;

const encoder = new TextEncoder();
const bytes = (text: string) => encoder.encode(text);

const response = (versions: Versions | 'error'): Response =>
	versions === 'error'
		? new Response('{}', { status: 503 })
		: versions === null
			? new Response('{"errors":[]}', { status: 404 })
			: new Response(JSON.stringify({ versions }), { status: 200 });

const make_wetrun = (options: {
	/** The registry's answers per crate, one list per read of the whole registry. */
	reads: Array<Record<string, Versions | 'error'>>;
	attempts?: Array<Attempt>;
	/** `git status --porcelain` per checkout assertion, in order; clean when absent. */
	statuses?: Array<string>;
	/** HEAD per checkout assertion, in order; the tag's commit when absent. */
	heads?: Array<string>;
	semver_code?: number;
	now?: number;
}) => {
	const log: Array<string> = [];
	const out: Array<string> = [];
	const err: Array<string> = [];
	const written: Array<Uint8Array> = [];
	const sleeps: Array<number> = [];
	const start_options: Array<unknown> = [];
	const checkout = {
		path: '/w',
		read_text_sync: (path: string) => {
			const text = FILES[path];
			if (text === undefined) throw new FsError('not_found', `/w/${path}`);
			return text;
		},
		read_text: async (path: string) => FILES[path]!
	} as unknown as FsReadDir;
	let read_index = -1;
	let fetches_this_read = 0;
	let assertion = 0;
	let attempt = 0;
	const cwd_is_checkout = (cwd: unknown) => strictEqual(cwd, checkout);
	const cargo = {
		name: 'cargo',
		path: '/usr/bin/cargo',
		out: async (args: ReadonlyArray<string>, o?: { cwd?: unknown }) => {
			cwd_is_checkout(o?.cwd);
			log.push(`cargo out ${args.join(' ')}`);
			const stdout =
				args[0] === 'metadata'
					? JSON.stringify(METADATA)
					: args[0] === 'semver-checks'
						? 'cargo-semver-checks 0.51.0'
						: '';
			return { kind: 'exited', code: 0, stdout, stderr: '', truncated: false };
		},
		run: async (args: ReadonlyArray<string>, o?: { cwd?: unknown }) => {
			cwd_is_checkout(o?.cwd);
			log.push(`cargo run ${args.join(' ')}`);
			return { kind: 'exited', code: args[0] === 'semver-checks' ? (options.semver_code ?? 0) : 0 };
		},
		out_bytes: () => Promise.reject(new Error('not a call publish:crates makes')),
		start: async (args: ReadonlyArray<string>, o?: { cwd?: unknown; stdio?: unknown }) => {
			cwd_is_checkout(o?.cwd);
			start_options.push(o?.stdio);
			log.push(`cargo start ${args.join(' ')}`);
			const answer = options.attempts?.[attempt++];
			if (answer === undefined) throw new Error('an attempt the test did not plan');
			return {
				pid: 1,
				stdin: null,
				stdout: null,
				stderr: (async function* () {
					for (const chunk of answer.stderr) yield chunk;
				})(),
				closed: Promise.resolve(answer.result),
				kill: () => false,
				[Symbol.asyncDispose]: async () => {}
			};
		}
	} as unknown as SpawnProgram;
	const git = {
		name: 'git',
		path: '/usr/bin/git',
		out: async (args: ReadonlyArray<string>, o?: { cwd?: unknown }) => {
			cwd_is_checkout(o?.cwd);
			log.push(`git out ${args.join(' ')}`);
			let stdout = '';
			if (args.join(' ') === 'rev-parse HEAD') stdout = options.heads?.[assertion] ?? AT;
			else if (args[0] === 'rev-parse') stdout = AT;
			else {
				stdout = options.statuses?.[assertion] ?? '';
				assertion++;
			}
			return { kind: 'exited', code: 0, stdout: `${stdout}\n`, stderr: '', truncated: false };
		},
		run: () => Promise.reject(new Error('git only captures here')),
		out_bytes: () => Promise.reject(new Error('git only captures here')),
		start: () => Promise.reject(new Error('git only captures here'))
	} as unknown as SpawnProgram;
	const crate_count = METADATA.packages.length;
	const deps: PublishCratesDeps = {
		checkout,
		cargo,
		git,
		registry: {
			fetch: async (url) => {
				if (fetches_this_read % crate_count === 0) read_index++;
				fetches_this_read++;
				log.push(`fetch ${url}`);
				const name = url.slice(API.length + 1);
				const read = options.reads[read_index];
				if (read === undefined) throw new Error(`registry read ${read_index} not planned`);
				return response(read[name] ?? null);
			}
		},
		registry_src_path: SRC,
		open_package_state: async (target_directory): Promise<PackageStateDeps> => {
			log.push(`cleanup ${target_directory}`);
			return {
				remove_package_dir: () => Promise.reject(new FsError('not_found', '/w/target/package')),
				registry_src: null
			};
		},
		print: (line) => out.push(line),
		print_error: (line) => err.push(line),
		write_error: async (chunk) => {
			written.push(chunk);
			return true;
		},
		sleep: async (ms) => {
			log.push(`sleep ${ms}`);
			sleeps.push(ms);
		},
		now: () => options.now ?? Date.parse('2026-11-11T20:00:00Z')
	};
	return { deps, log, out, err, written, sleeps, start_options };
};

const PREFLIGHT_GIT = [
	'git out rev-parse HEAD',
	'git out rev-parse --verify --quiet v0.8.0^{commit}',
	'git out status --porcelain'
];
const PUBLISH = 'cargo start publish --workspace --locked';
const OLD = [{ num: '0.7.1', yanked: false }];
const LIVE = [
	{ num: '0.8.0', yanked: false },
	{ num: '0.7.1', yanked: false }
];
const RETRY = 'Wed, 11 Nov 2026 20:10:00 GMT';
const LIMITED =
	'    Uploading tsv_b v0.8.0 (/w/crates/tsv_b)\n' +
	'error: failed to publish tsv_b v0.8.0 to registry at https://crates.io\n\n' +
	'Caused by:\n  the remote server responded with an error (status 429 Too Many Requests): ' +
	`You have published too many new crates in a short period of time. Please try again after ${RETRY} or email help@crates.io to have your limit increased.\n`;
const exited = (code: number): SpawnResult => ({ kind: 'exited', code });

/** `text` cut into chunks at byte offsets, one cut inside the multi-byte `→`. */
const chunked = (text: string): Array<Uint8Array> => {
	const all = bytes(text);
	const cuts = [0, 7, 21, all.length - 20, all.length];
	return cuts.slice(1).map((end, i) => all.slice(cuts[i], end));
};

const concat = (chunks: Array<Uint8Array>): Uint8Array => {
	const all = new Uint8Array(chunks.reduce((n, c) => n + c.length, 0));
	let at = 0;
	for (const c of chunks) {
		all.set(c, at);
		at += c.length;
	}
	return all;
};

test('a --wetrun asserts the checkout, grades, clears, asserts again, then publishes for real', async () => {
	const stderr = chunked('   Packaging tsv_a → crates.io\n    Uploaded tsv_b');
	const w = make_wetrun({
		reads: [{ tsv_a: OLD, tsv_b: null }],
		attempts: [{ stderr, result: exited(0) }]
	});
	strictEqual(await run_publish_crates(w.deps, ['--wetrun']), 0);
	deepStrictEqual(w.log, [
		'cargo out metadata --no-deps --format-version 1 --locked',
		...PREFLIGHT_GIT,
		`fetch ${API}/tsv_a`,
		`fetch ${API}/tsv_b`,
		'cargo out semver-checks --version',
		'cargo run semver-checks --package tsv_a --baseline-version 0.7.1 --all-features',
		'cleanup /w/target',
		...PREFLIGHT_GIT,
		PUBLISH
	]);
	// stdout shown, stderr piped through, no stdin
	deepStrictEqual(w.start_options, [['ignore', 'inherit', 'pipe']]);
	// every byte cargo wrote reaches stderr, in order, unchanged, the final partial line included
	deepStrictEqual(concat(w.written), concat(stderr));
	strictEqual(w.written.length, stderr.length);
	deepStrictEqual(w.out, [
		'=== publish:crates (WETRUN — publishes to crates.io) ===',
		'  checkout is v0.8.0, clean',
		'  version 0.8.0; 2 crates, in dependency order (cargo orders its own uploads): tsv_a → tsv_b',
		'\n=== Registry (crates.io API) ===',
		'  tsv_a: 1 version(s) on crates.io; v0.8.0 missing',
		'  tsv_b: not on crates.io; v0.8.0 missing',
		'\n=== Public-API gate (cargo semver-checks, docs.rs feature sets) ===',
		'  cargo-semver-checks 0.51.0',
		'\n  cargo semver-checks --package tsv_a --baseline-version 0.7.1 --all-features',
		'\n=== Publish (cargo publish --workspace) ===',
		'  no stale packaging state to clear',
		'  checkout is v0.8.0, clean',
		'  cargo publish --workspace --locked',
		'  published tsv_a, tsv_b at v0.8.0',
		'\n=== Summary ===',
		'  publish: done (v0.8.0, 2 crates)',
		'  semver-checks: 1 graded, 0 failed'
	]);
	deepStrictEqual(w.sleeps, []);
});

test('a --wetrun excludes every crate already live, a yanked one included', async () => {
	for (const tsv_a of [LIVE, [{ num: '0.8.0', yanked: true }, ...OLD]]) {
		const w = make_wetrun({
			reads: [{ tsv_a, tsv_b: null }],
			attempts: [{ stderr: [], result: exited(0) }]
		});
		strictEqual(await run_publish_crates(w.deps, ['--wetrun', '--no-semver-checks']), 0);
		deepStrictEqual(
			w.log.filter((l) => l.startsWith('cargo start')),
			[`${PUBLISH} --exclude tsv_a`]
		);
		ok(w.out.includes('  published tsv_b at v0.8.0'), w.out.join('\n'));
	}
});

test('a --wetrun with every crate live publishes nothing and clears nothing', async () => {
	const w = make_wetrun({ reads: [{ tsv_a: LIVE, tsv_b: LIVE }] });
	strictEqual(await run_publish_crates(w.deps, ['--wetrun']), 0);
	ok(!w.log.some((l) => l.startsWith('cleanup') || l.startsWith('cargo start')));
	strictEqual(w.log.filter((l) => l === 'git out status --porcelain').length, 1);
	ok(w.out.includes('  every crate is live at v0.8.0 — nothing to publish'));
	ok(w.out.includes('  publish: done (v0.8.0, 2 crates)'));
});

test('a failed publish that is no rate limit stops at once with its code', async () => {
	for (const [result, code] of [
		[exited(101), 101],
		[{ kind: 'signaled', signal: 'SIGKILL' } as const, 137]
	] as const) {
		const w = make_wetrun({
			reads: [{ tsv_a: OLD, tsv_b: null }],
			attempts: [{ stderr: [bytes('error: 500 Internal Server Error\n')], result }]
		});
		strictEqual(await run_publish_crates(w.deps, ['--wetrun', '--wait']), 1);
		strictEqual(w.log.filter((l) => l.startsWith('cargo start')).length, 1);
		deepStrictEqual(w.sleeps, []);
		ok(w.err.includes(`  FAIL: cargo publish exited ${code}`), w.err.join('\n'));
		ok(w.out.includes('  publish: FAILED (v0.8.0, 2 crates)'));
	}
});

test('a rate limit without --wait stops, naming the retry time read from the tee', async () => {
	const w = make_wetrun({
		reads: [{ tsv_a: OLD, tsv_b: null }],
		attempts: [{ stderr: chunked(LIMITED), result: exited(101) }]
	});
	strictEqual(await run_publish_crates(w.deps, ['--wetrun', '--no-semver-checks']), 1);
	deepStrictEqual(w.sleeps, []);
	ok(
		w.err.includes(
			`  STOPPED at crates.io's rate limit. Re-run \`npm run publish:crates -- --wetrun\` after ${RETRY}: ` +
				'it publishes only the crates not yet live (or pass --wait to sleep through it).'
		),
		w.err.join('\n')
	);
	deepStrictEqual(concat(w.written), bytes(LIMITED));
});

test('a rate limit naming no time stops even with --wait', async () => {
	const w = make_wetrun({
		reads: [{ tsv_a: OLD, tsv_b: null }],
		attempts: [{ stderr: [bytes('error: status 429 Too Many Requests\n')], result: exited(101) }]
	});
	strictEqual(await run_publish_crates(w.deps, ['--wetrun', '--wait', '--no-semver-checks']), 1);
	deepStrictEqual(w.sleeps, []);
	ok(
		w.err.some((l) => l.includes('after the time crates.io names above:')),
		w.err.join('\n')
	);
});

test('--wait sleeps to the retry time plus 30 s, re-reads the registry, asserts, and goes on', async () => {
	const now = Date.parse(RETRY) - 5 * 60_000;
	const w = make_wetrun({
		now,
		reads: [
			{ tsv_a: OLD, tsv_b: null },
			{ tsv_a: LIVE, tsv_b: null }
		],
		attempts: [
			{ stderr: chunked(LIMITED), result: exited(101) },
			{ stderr: [], result: exited(0) }
		]
	});
	strictEqual(await run_publish_crates(w.deps, ['--wetrun', '--wait', '--no-semver-checks']), 0);
	deepStrictEqual(w.sleeps, [5 * 60_000 + 30_000]);
	const after_preflight = w.log.slice(w.log.indexOf(`fetch ${API}/tsv_b`) + 1);
	deepStrictEqual(after_preflight, [
		'cleanup /w/target',
		...PREFLIGHT_GIT,
		PUBLISH,
		`sleep ${5 * 60_000 + 30_000}`,
		`fetch ${API}/tsv_a`,
		`fetch ${API}/tsv_b`,
		'cleanup /w/target',
		...PREFLIGHT_GIT,
		`${PUBLISH} --exclude tsv_a`
	]);
	ok(
		w.out.includes(
			`  rate limited — waiting until ${RETRY} (+30 s), then going on from the live set`
		)
	);
	ok(w.out.includes('  published tsv_b at v0.8.0'));
});

test('--wait past a retry time already gone sleeps only the 30 s', async () => {
	const w = make_wetrun({
		now: Date.parse(RETRY) + 60_000,
		reads: [
			{ tsv_a: OLD, tsv_b: null },
			{ tsv_a: OLD, tsv_b: null }
		],
		attempts: [
			{ stderr: [bytes(LIMITED)], result: exited(101) },
			{ stderr: [], result: exited(0) }
		]
	});
	strictEqual(await run_publish_crates(w.deps, ['--wetrun', '--wait', '--no-semver-checks']), 0);
	deepStrictEqual(w.sleeps, [30_000]);
});

test('a checkout that moved during the --wait sleep is refused before the next publish', async () => {
	for (const moved of [
		{ heads: [AT, AT, 'b'.repeat(40)] },
		{ statuses: ['', '', ' M Cargo.toml'] }
	]) {
		const w = make_wetrun({
			...moved,
			reads: [
				{ tsv_a: OLD, tsv_b: null },
				{ tsv_a: OLD, tsv_b: null }
			],
			attempts: [{ stderr: [bytes(LIMITED)], result: exited(101) }]
		});
		strictEqual(await run_publish_crates(w.deps, ['--wetrun', '--wait', '--no-semver-checks']), 1);
		strictEqual(w.log.filter((l) => l.startsWith('cargo start')).length, 1);
		ok(
			w.err.some((l) => l.startsWith('FAIL: refusing to publish:\n  ')),
			w.err.join('\n')
		);
	}
});

test('a registry read that fails after the --wait sleep is fatal', async () => {
	const w = make_wetrun({
		reads: [
			{ tsv_a: OLD, tsv_b: null },
			{ tsv_a: 'error', tsv_b: null }
		],
		attempts: [{ stderr: [bytes(LIMITED)], result: exited(101) }]
	});
	strictEqual(await run_publish_crates(w.deps, ['--wetrun', '--wait', '--no-semver-checks']), 1);
	strictEqual(w.log.filter((l) => l.startsWith('cargo start')).length, 1);
	ok(
		w.err.some((l) => l.startsWith(`FAIL: GET ${API}/tsv_a answered 503`)),
		w.err.join('\n')
	);
});

test('a --wetrun refuses before the registry: a dirty tree, or --allow-dirty', async () => {
	const dirty = make_wetrun({ reads: [], statuses: ['?? stray.rs'] });
	strictEqual(await run_publish_crates(dirty.deps, ['--wetrun']), 1);
	deepStrictEqual(dirty.log, [
		'cargo out metadata --no-deps --format-version 1 --locked',
		...PREFLIGHT_GIT
	]);
	ok(dirty.err[0]!.startsWith('FAIL: refusing to publish:\n  the tree is not clean (1 path(s)'));
	const flag = make_wetrun({ reads: [] });
	strictEqual(await run_publish_crates(flag.deps, ['--wetrun', '--allow-dirty']), 1);
	deepStrictEqual(flag.log, []);
	deepStrictEqual(flag.err, [
		'FAIL: --allow-dirty is for a dry run; a publish packages a clean tree'
	]);
});

test('a --wetrun whose gate fails publishes nothing and clears nothing', async () => {
	const w = make_wetrun({ reads: [{ tsv_a: OLD, tsv_b: null }], semver_code: 1 });
	strictEqual(await run_publish_crates(w.deps, ['--wetrun']), 1);
	ok(!w.log.some((l) => l.startsWith('cleanup') || l.startsWith('cargo start')));
	ok(w.err.includes('\n=== Publish — REFUSED: the public-API gate failed ==='));
});
