import { deepStrictEqual, ok, rejects } from 'node:assert';

import { FsError } from '@fuzdev/gro/fs_error.ts';
import { test } from 'vitest';

import {
	cargo_publish_args,
	cargo_publish_rate_limit,
	clear_package_state,
	crate_version_state,
	CRATES_IO_USER_AGENT,
	crates_io_versions,
	crates_publish_order,
	crates_semver_plan,
	is_overlay_parent,
	is_overlay_unpack,
	type MetadataPackage,
	type PackageStateDeps,
	semver_checks_args,
	semver_checks_release_type,
	version_compare,
	wetrun_checkout_refusals
} from './crates.ts';

/** A fake registry: answers each URL from `routes`, recording the requests it gets. */
const fake_fetch = (routes: Record<string, () => Response>) => {
	const requests: Array<{ url: string; agent: string | null }> = [];
	return {
		requests,
		fetch: (url: string, init: RequestInit): Promise<Response> => {
			requests.push({ url, agent: new Headers(init.headers).get('User-Agent') });
			const route = routes[url];
			if (!route) return Promise.reject(new TypeError('network unreachable'));
			return Promise.resolve(route());
		}
	};
};

const json =
	(body: unknown, status = 200) =>
	() =>
		new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

const API = 'https://crates.io/api/v1/crates';

test('crates_io_versions reads a crate, a missing crate, and refuses everything else', async () => {
	const registry = fake_fetch({
		[`${API}/tsv_lang`]: json({
			crate: { name: 'tsv_lang' },
			versions: [
				{ num: '0.8.0', yanked: false, extra: 1 },
				{ num: '0.7.1', yanked: true }
			]
		}),
		[`${API}/tsv_ts`]: json({ errors: [{ detail: 'crate `tsv_ts` does not exist' }] }, 404),
		[`${API}/tsv_css`]: json({ errors: [] }, 500),
		[`${API}/tsv_html`]: () => new Response('<html>', { status: 200 }),
		[`${API}/tsv_ignore`]: json({ crate: {} })
	});
	deepStrictEqual(await crates_io_versions(registry, 'tsv_lang'), {
		ok: true,
		versions: [
			{ num: '0.8.0', yanked: false },
			{ num: '0.7.1', yanked: true }
		]
	});
	deepStrictEqual(await crates_io_versions(registry, 'tsv_ts'), { ok: true, versions: null });
	for (const name of ['tsv_css', 'tsv_html', 'tsv_ignore', 'tsv_cli']) {
		const answer = await crates_io_versions(registry, name);
		ok(!answer.ok, `${name} must be an error`);
		ok(answer.error.includes(`${API}/${name}`), answer.error);
	}
	ok(registry.requests.every((r) => r.agent === CRATES_IO_USER_AGENT));
});

test("crates_io_versions names a failed fetch's cause, which Node's fetch keeps off its message", async () => {
	// the shapes a failed fetch takes: a DNS failure, every address of a host refusing, an
	// aggregate with its own message, and a failure with no cause
	const refused = (address: string) => new Error(`connect ECONNREFUSED ${address}:443`);
	for (const [thrown, expected] of [
		[
			new TypeError('fetch failed', { cause: new Error('getaddrinfo EAI_AGAIN crates.io') }),
			'TypeError: fetch failed (cause: Error: getaddrinfo EAI_AGAIN crates.io)'
		],
		[
			new TypeError('fetch failed', {
				cause: new AggregateError([refused('::1'), refused('127.0.0.1')])
			}),
			'TypeError: fetch failed (cause: Error: connect ECONNREFUSED ::1:443; ' +
				'Error: connect ECONNREFUSED 127.0.0.1:443)'
		],
		[
			new TypeError('fetch failed', { cause: new AggregateError([], 'all refused') }),
			'TypeError: fetch failed (cause: AggregateError: all refused)'
		],
		[new TypeError('network unreachable'), 'TypeError: network unreachable']
	] as const) {
		const answer = await crates_io_versions({ fetch: () => Promise.reject(thrown) }, 'tsv_lang');
		deepStrictEqual(answer, { ok: false, error: `GET ${API}/tsv_lang failed: ${expected}` });
	}
});

test('crate_version_state: missing, live, and a yanked version still taken', () => {
	const versions = [
		{ num: '0.8.0', yanked: false },
		{ num: '0.7.1', yanked: true }
	];
	deepStrictEqual(crate_version_state(versions, '0.8.0'), 'live');
	deepStrictEqual(crate_version_state(versions, '0.7.1'), 'yanked');
	deepStrictEqual(crate_version_state(versions, '0.8.1'), 'missing');
	deepStrictEqual(crate_version_state(null, '0.8.0'), 'missing');
});

test('version_compare orders release versions numerically', () => {
	deepStrictEqual(Math.sign(version_compare('0.10.0', '0.9.9')!), 1);
	deepStrictEqual(version_compare('1.2.3', '1.2.3'), 0);
	deepStrictEqual(Math.sign(version_compare('0.7.1', '0.7.10')!), -1);
	deepStrictEqual(version_compare('0.7.1-rc.1', '0.7.1'), null);
});

const pkg = (
	name: string,
	deps: Array<[string, string | null]> = [],
	publish: Array<string> | null = null
): MetadataPackage => ({
	name,
	version: '0.7.1',
	publish,
	dependencies: deps.map(([dep, kind]) => ({ name: dep, kind, path: `crates/${dep}` }))
});

// the workspace's shape: members order, then the edges the publishable crates hold
const workspace = [
	pkg('tsv_lang'),
	pkg('tsv_arena', [['tsv_lang', null]], []),
	pkg('tsv_html'),
	pkg('tsv_ignore'),
	pkg('tsv_discover', [['tsv_ignore', null]]),
	pkg('tsv_ts', [
		['tsv_lang', null],
		['tsv_svelte', 'dev']
	]),
	pkg('tsv_css', [['tsv_lang', null]]),
	pkg('tsv_svelte', [
		['tsv_css', null],
		['tsv_html', null],
		['tsv_lang', null],
		['tsv_ts', null]
	]),
	pkg('tsv_cli', [
		['tsv_css', null],
		['tsv_discover', null],
		['tsv_ignore', null],
		['tsv_lang', null],
		['tsv_svelte', null],
		['tsv_ts', null],
		['bumpalo', null]
	])
];
const names = [
	'tsv_lang',
	'tsv_html',
	'tsv_ignore',
	'tsv_discover',
	'tsv_ts',
	'tsv_css',
	'tsv_svelte',
	'tsv_cli'
];

test('crates_publish_order: dependencies first, dev edges ignored, ties in member order', () => {
	deepStrictEqual(crates_publish_order(workspace, names), { ok: true, order: names });
	// listed dependents-first, the order still puts each dependency ahead
	const reversed = [...names].reverse();
	const answer = crates_publish_order(workspace, reversed);
	ok(answer.ok);
	for (const p of workspace.filter((p) => names.includes(p.name))) {
		for (const d of p.dependencies.filter((d) => d.kind !== 'dev' && names.includes(d.name))) {
			ok(answer.order.indexOf(d.name) < answer.order.indexOf(p.name), `${d.name} before ${p.name}`);
		}
	}
	// a build edge orders like a normal one
	const built = [pkg('a', [['b', 'build']]), pkg('b')];
	deepStrictEqual(crates_publish_order(built, ['a', 'b']), { ok: true, order: ['b', 'a'] });
});

test('crates_publish_order refuses a cycle and an unknown crate', () => {
	const cycle = [pkg('a', [['b', null]]), pkg('b', [['a', null]])];
	const answer = crates_publish_order(cycle, ['a', 'b']);
	ok(!answer.ok && answer.error.includes('cycle'));
	const unknown = crates_publish_order(workspace, ['tsv_nope']);
	ok(!unknown.ok && unknown.error.includes('tsv_nope'));
});

test('crates_semver_plan skips, never passes, without a published baseline', () => {
	const none = crates_semver_plan(null, '0.8.0', null);
	ok(none.kind === 'skip' && none.reason.includes('no published baseline'));
	const yanked = crates_semver_plan([{ num: '0.7.1', yanked: true }], '0.8.0', 'minor');
	ok(yanked.kind === 'skip' && yanked.reason.includes('no published baseline'));
	const above = crates_semver_plan([{ num: '0.9.0', yanked: false }], '0.8.0', null);
	ok(above.kind === 'skip' && above.reason.includes('above'));
});

test('crates_semver_plan: a new version grades against the highest release below it', () => {
	const versions = [
		{ num: '0.7.0', yanked: false },
		{ num: '0.7.10', yanked: false },
		{ num: '0.7.11', yanked: true },
		{ num: '0.7.9', yanked: false },
		{ num: '0.8.0-rc.1', yanked: false }
	];
	deepStrictEqual(crates_semver_plan(versions, '0.8.0', null), {
		kind: 'check',
		baseline: '0.7.10',
		release_type: null
	});
	// the version numbers carry the release type; a --bump changes nothing here
	deepStrictEqual(crates_semver_plan(versions, '0.8.0', 'patch'), {
		kind: 'check',
		baseline: '0.7.10',
		release_type: null
	});
});

test('crates_semver_plan: an already-live version grades only at a named bump', () => {
	const versions = [{ num: '0.8.0', yanked: false }];
	const plan = crates_semver_plan(versions, '0.8.0', null);
	ok(plan.kind === 'skip' && plan.reason.includes('already on crates.io'));
	// at 0.x the next minor is a breaking release, so the tool grades it as major
	deepStrictEqual(crates_semver_plan(versions, '0.8.0', 'minor'), {
		kind: 'check',
		baseline: '0.8.0',
		release_type: 'major'
	});
	deepStrictEqual(crates_semver_plan([{ num: '1.2.0', yanked: false }], '1.2.0', 'minor'), {
		kind: 'check',
		baseline: '1.2.0',
		release_type: 'minor'
	});
});

test('semver_checks_release_type reads a bump as cargo does before 1.0', () => {
	deepStrictEqual(semver_checks_release_type('0.8.0', 'major'), 'major');
	deepStrictEqual(semver_checks_release_type('0.8.0', 'minor'), 'major');
	deepStrictEqual(semver_checks_release_type('0.8.0', 'patch'), 'minor');
	for (const bump of ['major', 'minor', 'patch'] as const) {
		deepStrictEqual(semver_checks_release_type('0.0.3', bump), 'major');
		deepStrictEqual(semver_checks_release_type('1.4.2', bump), bump);
	}
});

test('semver_checks_args states the docs.rs feature set explicitly', () => {
	const plan = { kind: 'check', baseline: '0.7.1', release_type: null } as const;
	const plain = { features: [], all_features: false, no_default_features: false };
	deepStrictEqual(semver_checks_args('tsv_cli', plain, plan), [
		'semver-checks',
		'--package',
		'tsv_cli',
		'--baseline-version',
		'0.7.1',
		'--default-features'
	]);
	deepStrictEqual(
		semver_checks_args(
			'tsv_ts',
			{ ...plain, features: ['locations'] },
			{ ...plan, release_type: 'minor' }
		),
		[
			'semver-checks',
			'--package',
			'tsv_ts',
			'--baseline-version',
			'0.7.1',
			'--default-features',
			'--features',
			'locations',
			'--release-type',
			'minor'
		]
	);
	ok(
		semver_checks_args(
			'x',
			{ ...plain, no_default_features: true, features: ['a'] },
			plan
		).includes('--only-explicit-features')
	);
	ok(semver_checks_args('x', { ...plain, all_features: true }, plan).includes('--all-features'));
	// never the tool's own all-but-a-few heuristic: some explicit mode is always named
	for (const set of [
		plain,
		{ ...plain, no_default_features: true },
		{ ...plain, all_features: true }
	]) {
		const args = semver_checks_args('x', set, plan);
		ok(
			['--default-features', '--only-explicit-features', '--all-features'].some((f) =>
				args.includes(f)
			)
		);
	}
});

test('cargo_publish_args excludes the live crates', () => {
	deepStrictEqual(cargo_publish_args({ dry_run: true, allow_dirty: false, exclude: [] }), [
		'publish',
		'--workspace',
		'--locked',
		'--dry-run'
	]);
	deepStrictEqual(
		cargo_publish_args({ dry_run: false, allow_dirty: false, exclude: ['tsv_lang', 'tsv_html'] }),
		['publish', '--workspace', '--locked', '--exclude', 'tsv_lang', '--exclude', 'tsv_html']
	);
});

test('cargo_publish_rate_limit reads the 429 and its retry time', () => {
	const stderr =
		'error: failed to publish to registry at https://crates.io\n\nCaused by:\n' +
		'  the remote server responded with an error (status 429 Too Many Requests): You have ' +
		'published too many new crates in a short period of time. Please try again after ' +
		'Thu, 29 Oct 2026 12:34:56 GMT or email help@crates.io to have your limit increased.';
	const limited = cargo_publish_rate_limit(stderr);
	ok(limited.limited);
	deepStrictEqual(limited.retry_at?.toISOString(), '2026-10-29T12:34:56.000Z');
	const no_time = cargo_publish_rate_limit('(status 429 Too Many Requests): slow down');
	deepStrictEqual(no_time, { limited: true, retry_at: null });
	deepStrictEqual(cargo_publish_rate_limit('error: failed to verify package tarball (429 files)'), {
		limited: false
	});
	deepStrictEqual(
		cargo_publish_rate_limit('the remote server responded with an error (status 403 Forbidden)'),
		{
			limited: false
		}
	);
});

test('is_overlay_unpack names only this release overlay unpacks', () => {
	const crates = ['tsv_lang', 'tsv_ts'];
	ok(is_overlay_unpack('-088a2fcb40cb7e05', 'tsv_lang-0.7.1', crates, '0.7.1'));
	ok(!is_overlay_unpack('-088a2fcb40cb7e05', 'tsv_lang-0.7.0', crates, '0.7.1'));
	ok(!is_overlay_unpack('-088a2fcb40cb7e05', 'tsv_cli-0.7.1', crates, '0.7.1'));
	ok(!is_overlay_unpack('index.crates.io-1949cf8c6b5b557f', 'tsv_lang-0.7.1', crates, '0.7.1'));
	ok(!is_overlay_unpack('-088a2fcb40cb7e05', 'tsv_lang-0.7.1-x', crates, '0.7.1'));
});

const SRC = '/home/u/.cargo/registry/src';
const A = `${SRC}/-088a2fcb40cb7e05`;
const B = `${SRC}/-1ce78c6a834fdbe2`;
const PACKAGE = '/t/package';

/**
 * A fake of the cleanup's two directories: a set of directory paths (`PACKAGE`, and `SRC` with
 * what is beneath it), with the removals recorded. `registry_src` is `null` without `SRC`, as
 * the root opens it. `fail` throws an error of its own from an op on a path.
 */
const fake_state = (
	paths: Array<string>,
	options: {
		symlinks?: Array<string>;
		fail?: (op: 'remove' | 'list', path: string) => Error | null;
	} = {}
) => {
	const entries = new Set(paths);
	const removals: Array<{ path: string; recursive: boolean }> = [];
	const children = (dir: string) =>
		[...entries]
			.filter((p) => p.startsWith(`${dir}/`) && !p.slice(dir.length + 1).includes('/'))
			.map((p) => p.slice(dir.length + 1));
	const at = (name?: string) => (name === undefined ? SRC : `${SRC}/${name}`);
	const remove = async (path: string, recursive: boolean): Promise<void> => {
		const failure = options.fail?.('remove', path);
		if (failure) throw failure;
		if (!entries.has(path)) throw new FsError('not_found', path);
		if (!recursive && children(path).length > 0) throw new FsError('not_empty', path);
		removals.push({ path, recursive });
		for (const p of [...entries]) if (p === path || p.startsWith(`${path}/`)) entries.delete(p);
	};
	const deps: PackageStateDeps = {
		remove_package_dir: () => remove(PACKAGE, true),
		registry_src: entries.has(SRC)
			? {
					list: (name) =>
						(async function* () {
							const dir = at(name);
							const failure = options.fail?.('list', dir);
							if (failure) throw failure;
							if (!entries.has(dir)) throw new FsError('not_found', dir);
							for (const child of children(dir)) yield { name: child, kind: 'directory' as const };
						})(),
					stat: async (name, stat_options) => {
						const path = at(name);
						if (!entries.has(path)) return null;
						const link = stat_options?.follow === false && options.symlinks?.includes(path);
						return { kind: link ? 'symlink' : 'directory', size: 0, mtime_ms: 0, mode: 0o755 };
					},
					remove: (name, remove_options) => remove(at(name), remove_options?.recursive === true)
				}
			: null
	};
	return { entries, removals, deps };
};

const options = (names: Array<string> = ['tsv_lang']) => ({
	package_dir: PACKAGE,
	registry_src: SRC,
	names,
	version: '0.7.1'
});

test('clear_package_state removes the package dir, this release unpacks, and emptied overlay dirs', async () => {
	const fs = fake_state([
		'/t',
		PACKAGE,
		`${PACKAGE}/x`,
		SRC,
		A,
		`${A}/tsv_lang-0.7.1`,
		`${A}/tsv_lang-0.7.1/src`,
		`${A}/tsv_ts-0.7.1`,
		B,
		`${B}/tsv_lang-0.7.1`,
		`${B}/tsv_lang-0.7.0`,
		`${SRC}/index.crates.io-1949cf8c6b5b557f`,
		`${SRC}/index.crates.io-1949cf8c6b5b557f/tsv_lang-0.7.1`
	]);
	const removed = await clear_package_state(fs.deps, options(['tsv_lang', 'tsv_ts']));
	deepStrictEqual(removed, [
		PACKAGE,
		`${A}/tsv_lang-0.7.1`,
		`${A}/tsv_ts-0.7.1`,
		A,
		`${B}/tsv_lang-0.7.1`
	]);
	// the emptied parent goes by a NON-recursive remove; a parent still holding another
	// version, and a real registry's unpack, stay
	deepStrictEqual(fs.removals, [
		{ path: PACKAGE, recursive: true },
		{ path: `${A}/tsv_lang-0.7.1`, recursive: true },
		{ path: `${A}/tsv_ts-0.7.1`, recursive: true },
		{ path: A, recursive: false },
		{ path: `${B}/tsv_lang-0.7.1`, recursive: true }
	]);
	ok(fs.entries.has(B) && fs.entries.has(`${B}/tsv_lang-0.7.0`));
	ok(fs.entries.has(`${SRC}/index.crates.io-1949cf8c6b5b557f/tsv_lang-0.7.1`));
});

test('clear_package_state leaves an overlay dir it removed nothing from, and tolerates absence', async () => {
	const C = `${SRC}/-f15641a0afa3a4f9`;
	const fs = fake_state([SRC, C]);
	deepStrictEqual(await clear_package_state(fs.deps, options()), []);
	ok(fs.entries.has(C), 'an empty overlay dir this run did not empty stays');
	deepStrictEqual(fs.removals, []);
	// no package dir and no registry/src (a fresh runner) is no error
	deepStrictEqual(await clear_package_state(fake_state([]).deps, options()), []);
});

test('clear_package_state fails, never skips, on any error but a missing entry', async () => {
	const denied = (path: string) => new FsError('permission_denied', path);
	// the package dir, present or absent: a refusal answers before existence does
	for (const paths of [[PACKAGE], []]) {
		const fs = fake_state(paths, { fail: (op, path) => (path === PACKAGE ? denied(path) : null) });
		await rejects(clear_package_state(fs.deps, options()), (error) => {
			ok(error instanceof FsError && error.kind === 'permission_denied' && error.path === PACKAGE);
			return true;
		});
	}
	// a refusal that isn't an `FsError` at all, as the root's for a directory outside its grant
	const refused = new Error('outside the grant');
	await rejects(
		clear_package_state(fake_state([], { fail: () => refused }).deps, options()),
		(error) => error === refused
	);
	// an overlay unpack's removal, after the package dir went
	const unpack = fake_state([PACKAGE, SRC, A, `${A}/tsv_lang-0.7.1`], {
		fail: (op, path) => (op === 'remove' && path === `${A}/tsv_lang-0.7.1` ? denied(path) : null)
	});
	await rejects(clear_package_state(unpack.deps, options()), (error) => {
		ok(error instanceof FsError && error.kind === 'permission_denied');
		return true;
	});
	deepStrictEqual(unpack.removals, [{ path: PACKAGE, recursive: true }]);
	ok(unpack.entries.has(`${A}/tsv_lang-0.7.1`));
	// a listing of registry/src or of an overlay dir
	for (const dir of [SRC, A]) {
		const listing = fake_state([SRC, A, `${A}/tsv_lang-0.7.1`], {
			fail: (op, path) => (op === 'list' && path === dir ? denied(path) : null)
		});
		await rejects(clear_package_state(listing.deps, options()), (error) => {
			ok(error instanceof FsError && error.kind === 'permission_denied' && error.path === dir);
			return true;
		});
		ok(listing.entries.has(`${A}/tsv_lang-0.7.1`));
	}
});

test('is_overlay_parent names only a -<16 hex> overlay dir', () => {
	ok(is_overlay_parent('-088a2fcb40cb7e05'));
	ok(!is_overlay_parent('index.crates.io-1949cf8c6b5b557f'));
	ok(!is_overlay_parent('-088a2fcb40cb7e0'));
	ok(!is_overlay_parent('-088A2FCB40CB7E05'));
});

test('wetrun_checkout_refusals: only a clean checkout of the version tag publishes', () => {
	const at = 'a'.repeat(40);
	deepStrictEqual(
		wetrun_checkout_refusals({ version: '0.8.0', head: at, tag_commit: at, status_lines: [] }),
		[]
	);
	const off = wetrun_checkout_refusals({
		version: '0.8.0',
		head: 'b'.repeat(40),
		tag_commit: at,
		status_lines: []
	});
	deepStrictEqual(off.length, 1);
	ok(off[0]!.includes('is not v0.8.0'), off[0]);
	const no_tag = wetrun_checkout_refusals({
		version: '0.8.0',
		head: at,
		tag_commit: null,
		status_lines: []
	});
	ok(no_tag.length === 1 && no_tag[0]!.includes('no local tag v0.8.0'), no_tag[0]);
	const dirty = wetrun_checkout_refusals({
		version: '0.8.0',
		head: at,
		tag_commit: at,
		status_lines: [' M Cargo.toml', '?? stray.rs']
	});
	ok(dirty.length === 1 && dirty[0]!.includes('?? stray.rs'), dirty[0]);
	deepStrictEqual(
		wetrun_checkout_refusals({ version: '0.8.0', head: null, tag_commit: at, status_lines: ['x'] })
			.length,
		2
	);
});

test('clear_package_state never follows a symlinked overlay parent', async () => {
	const L = `${SRC}/-dddddddddddddddd`;
	const fs = fake_state([SRC, L, `${L}/tsv_lang-0.7.1`], { symlinks: [L] });
	deepStrictEqual(await clear_package_state(fs.deps, options()), []);
	ok(fs.entries.has(`${L}/tsv_lang-0.7.1`));
	deepStrictEqual(fs.removals, []);
});
