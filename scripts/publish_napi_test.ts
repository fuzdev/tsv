/**
 * The N-API publish over a fake file system and a fake `npm`: the argv of every `npm` run and
 * their order, the registry readings (republish, 404, a fatal error, a dry run's
 * already-published pass), each refusal of the staged set, the mode re-arm and its verify,
 * and what a failure midway leaves unpublished. Every capability is a fake, so the real
 * publish paths (`--provenance`, no `--dry-run`) are safe to drive here
 * (`scripts/no_package_closure_test.ts` holds this suite to that).
 */

import { deepStrictEqual, rejects, strictEqual } from 'node:assert';
import { join } from 'node:path';
import { describe, test } from 'vitest';

import { type PublishNapiDeps, run_publish_napi } from './publish_napi.ts';

const CHECKOUT = join('/', 'repo');
const PKG = 'crates/tsv_napi/pkg';
const at = (path: string): string => join(CHECKOUT, path);

const VERSION = '1.2.3';
const GNU = '@fuzdev/tsv-linux-x64-gnu';
const WIN = '@fuzdev/tsv-win32-x64';

interface Reply {
	code?: number;
	stdout?: string;
	stderr?: string;
}

interface FakeWorld {
	deps: PublishNapiDeps;
	/** Every call, in order, as `[op, ...args]`. */
	calls: Array<Array<unknown>>;
	/** Each npm run's argv, in order. */
	npm: Array<Array<string>>;
	modes: Map<string, number>;
	out: Array<string>;
	err: Array<string>;
}

/** The staged set: a loader pinning `optional` and, for each platform, its package.json and
 * both binaries, unless `files` says otherwise. */
const staged_files = (
	optional: Record<string, string> = { [GNU]: VERSION, [WIN]: VERSION },
	version = VERSION
): Map<string, string> => {
	const files = new Map<string, string>();
	files.set(
		at(`${PKG}/napi/package.json`),
		JSON.stringify({ name: '@fuzdev/tsv', version, optionalDependencies: optional })
	);
	for (const name of Object.keys(optional)) {
		const triple = name.replace('@fuzdev/tsv-', '');
		files.set(at(`${PKG}/${triple}/package.json`), JSON.stringify({ name, version }));
		files.set(at(`${PKG}/${triple}/tsv_napi.node`), 'addon');
		files.set(at(`${PKG}/${triple}/${triple.startsWith('win32-') ? 'tsv.exe' : 'tsv'}`), 'cli');
	}
	return files;
};

/** A fake world over `files`: `npm` answers by `replies` (a success with no output when it
 * says nothing), `set_mode` takes unless `resist_mode`, and every binary starts at 0o644. */
const fake_world = (
	options: {
		files?: Map<string, string>;
		replies?: (args: Array<string>) => Reply | undefined;
		platform?: string;
		resist_mode?: boolean;
	} = {}
): FakeWorld => {
	const files = options.files ?? staged_files();
	const calls: Array<Array<unknown>> = [];
	const npm: Array<Array<string>> = [];
	const modes = new Map<string, number>();
	const out: Array<string> = [];
	const err: Array<string> = [];
	return {
		calls,
		npm,
		modes,
		out,
		err,
		deps: {
			checkout: CHECKOUT,
			platform: options.platform ?? 'linux',
			read_text: async (path) => {
				calls.push(['read_text', path]);
				const text = files.get(path);
				if (text === undefined) throw new Error(`ENOENT: ${path}`);
				return text;
			},
			is_file: async (path) => {
				calls.push(['is_file', path]);
				return files.has(path);
			},
			set_mode: async (path, mode) => {
				calls.push(['set_mode', path, mode]);
				if (!options.resist_mode) modes.set(path, mode);
			},
			mode: async (path) => {
				calls.push(['mode', path]);
				return 0o100000 | (modes.get(path) ?? 0o644);
			},
			npm: async (args) => {
				calls.push(['npm', ...args]);
				npm.push(args);
				const reply = options.replies?.(args) ?? {};
				return { code: reply.code ?? 0, stdout: reply.stdout ?? '', stderr: reply.stderr ?? '' };
			},
			print: (line) => out.push(line),
			print_error: (line) => err.push(line)
		}
	};
};

/** The registry's 404 for a version it doesn't hold. */
const E404: Reply = { code: 1, stderr: 'npm error code E404\nnpm error 404 Not Found - GET …\n' };

const not_on_registry = (args: Array<string>): Reply | undefined =>
	args[0] === 'view' ? E404 : undefined;

describe('a real publish', () => {
	test('views then publishes each package, platforms first and the loader last', async () => {
		const world = fake_world({ replies: not_on_registry });
		strictEqual(
			await run_publish_napi(world.deps, ['--provenance', '--expect-tag', `v${VERSION}`]),
			0
		);
		deepStrictEqual(world.npm, [
			['view', `${GNU}@${VERSION}`, 'version'],
			['publish', `${PKG}/linux-x64-gnu`, '--access', 'public', '--provenance'],
			['view', `${WIN}@${VERSION}`, 'version'],
			['publish', `${PKG}/win32-x64`, '--access', 'public', '--provenance'],
			['view', `@fuzdev/tsv@${VERSION}`, 'version'],
			['publish', `${PKG}/napi`, '--access', 'public', '--provenance']
		]);
		deepStrictEqual(world.out, [
			`npm publish ${PKG}/linux-x64-gnu --access public --provenance`,
			`npm publish ${PKG}/win32-x64 --access public --provenance`,
			`npm publish ${PKG}/napi --access public --provenance`,
			`published 3, skipped 0 (of 3) at v${VERSION}`
		]);
		deepStrictEqual(world.err, []);
	});

	test('without --provenance the publish argv carries neither flag', async () => {
		const world = fake_world({ replies: not_on_registry });
		strictEqual(await run_publish_napi(world.deps, []), 0);
		deepStrictEqual(
			world.npm.filter((args) => args[0] === 'publish').map((args) => args.slice(2)),
			[
				['--access', 'public'],
				['--access', 'public'],
				['--access', 'public']
			]
		);
	});

	test('a version already on the registry is skipped, read from trimmed stdout', async () => {
		const world = fake_world({
			replies: (args) =>
				args[0] === 'view'
					? args[1]!.startsWith(GNU)
						? { stdout: ` ${VERSION} \n` }
						: E404
					: undefined
		});
		strictEqual(await run_publish_napi(world.deps, ['--provenance']), 0);
		deepStrictEqual(
			world.npm.map((args) => args.slice(0, 2)),
			[
				['view', `${GNU}@${VERSION}`],
				['view', `${WIN}@${VERSION}`],
				['publish', `${PKG}/win32-x64`],
				['view', `@fuzdev/tsv@${VERSION}`],
				['publish', `${PKG}/napi`]
			]
		);
		deepStrictEqual(world.out, [
			`skip: ${GNU}@${VERSION} already on the registry`,
			`npm publish ${PKG}/win32-x64 --access public --provenance`,
			`npm publish ${PKG}/napi --access public --provenance`,
			`published 2, skipped 1 (of 3) at v${VERSION}`
		]);
	});

	test('a re-run over a fully published set publishes nothing', async () => {
		const world = fake_world({
			replies: (args) => (args[0] === 'view' ? { stdout: VERSION } : undefined)
		});
		strictEqual(await run_publish_napi(world.deps, ['--provenance']), 0);
		deepStrictEqual(
			world.npm.map((args) => args[0]),
			['view', 'view', 'view']
		);
		strictEqual(world.out.at(-1), `published 0, skipped 3 (of 3) at v${VERSION}`);
	});

	test('an exit-0 view with other stdout reads as not published', async () => {
		const world = fake_world({
			replies: (args) => (args[0] === 'view' ? { stdout: '1.2.2' } : undefined)
		});
		strictEqual(await run_publish_napi(world.deps, []), 0);
		strictEqual(world.out.at(-1), `published 3, skipped 0 (of 3) at v${VERSION}`);
	});

	test('a bare 404 without the E404 code also reads as not published', async () => {
		const world = fake_world({
			replies: (args) => (args[0] === 'view' ? { code: 1, stderr: '404 Not Found' } : undefined)
		});
		strictEqual(await run_publish_napi(world.deps, []), 0);
		strictEqual(world.out.at(-1), `published 3, skipped 0 (of 3) at v${VERSION}`);
	});

	test('a view error that is no 404 is fatal, before anything publishes', async () => {
		const world = fake_world({
			replies: (args) =>
				args[0] === 'view' ? { code: 1, stderr: '  npm error code E401\n' } : undefined
		});
		strictEqual(await run_publish_napi(world.deps, ['--provenance']), 1);
		deepStrictEqual(world.npm, [['view', `${GNU}@${VERSION}`, 'version']]);
		deepStrictEqual(world.err, [
			`FAIL: npm view ${GNU}@${VERSION} errored (not a 404):`,
			'npm error code E401'
		]);
		deepStrictEqual(world.out, []);
	});

	test('a fatal view with an empty stderr prints its stdout', async () => {
		const world = fake_world({
			replies: (args) => (args[0] === 'view' ? { code: 1, stdout: 'offline\n' } : undefined)
		});
		strictEqual(await run_publish_napi(world.deps, []), 1);
		strictEqual(world.err.at(-1), 'offline');
	});

	test('a failed publish midway stops there, so the loader never goes live', async () => {
		const world = fake_world({
			replies: (args) =>
				args[0] === 'view'
					? E404
					: args[1] === `${PKG}/win32-x64`
						? { code: 1, stderr: 'npm error code E403\n' }
						: undefined
		});
		strictEqual(await run_publish_napi(world.deps, ['--provenance']), 1);
		deepStrictEqual(
			world.npm.map((args) => args.slice(0, 2)),
			[
				['view', `${GNU}@${VERSION}`],
				['publish', `${PKG}/linux-x64-gnu`],
				['view', `${WIN}@${VERSION}`],
				['publish', `${PKG}/win32-x64`]
			]
		);
		deepStrictEqual(world.err, [
			`FAIL: npm publish ${WIN}@${VERSION} exited 1:`,
			'npm error code E403'
		]);
	});

	test('a real publish reads an already-published refusal as a failure', async () => {
		const world = fake_world({
			replies: (args) =>
				args[0] === 'view'
					? E404
					: {
							code: 1,
							stderr: 'You cannot publish over the previously published versions: 1.2.3.'
						}
		});
		strictEqual(await run_publish_napi(world.deps, []), 1);
		strictEqual(world.err[0], `FAIL: npm publish ${GNU}@${VERSION} exited 1:`);
	});
});

describe('a dry run', () => {
	test('views nothing and packs every package with --dry-run', async () => {
		const world = fake_world({ replies: () => ({ code: 0 }) });
		strictEqual(await run_publish_napi(world.deps, ['--dry-run']), 0);
		deepStrictEqual(world.npm, [
			['publish', `${PKG}/linux-x64-gnu`, '--access', 'public', '--dry-run'],
			['publish', `${PKG}/win32-x64`, '--access', 'public', '--dry-run'],
			['publish', `${PKG}/napi`, '--access', 'public', '--dry-run']
		]);
		strictEqual(
			world.out.at(-1),
			`DRY-RUN: 3 of 3 packed cleanly at v${VERSION} (3 would publish, 0 already on the registry)`
		);
	});

	test('reads npm refusing to publish over a live version as a pass', async () => {
		const world = fake_world({
			replies: (args) =>
				args[1] === `${PKG}/napi`
					? undefined
					: {
							code: 1,
							stderr: 'npm error You cannot publish over the previously published versions: 1.2.3.'
						}
		});
		strictEqual(await run_publish_napi(world.deps, ['--dry-run', '--provenance']), 0);
		deepStrictEqual(world.out, [
			`npm publish ${PKG}/linux-x64-gnu --access public --provenance --dry-run`,
			`PASS: ${GNU} packs cleanly (v${VERSION} already on the registry)`,
			`npm publish ${PKG}/win32-x64 --access public --provenance --dry-run`,
			`PASS: ${WIN} packs cleanly (v${VERSION} already on the registry)`,
			`npm publish ${PKG}/napi --access public --provenance --dry-run`,
			`DRY-RUN: 3 of 3 packed cleanly at v${VERSION} (1 would publish, 2 already on the registry)`
		]);
	});

	test("npm's other previously-published refusal still fails the rehearsal", async () => {
		const world = fake_world({
			replies: () => ({
				code: 1,
				stderr:
					'Cannot implicitly apply the "latest" tag because previously published version 2.0.0 is higher'
			})
		});
		strictEqual(await run_publish_napi(world.deps, ['--dry-run']), 1);
		strictEqual(world.npm.length, 1);
		strictEqual(world.err[0], `FAIL: npm publish ${GNU}@${VERSION} exited 1:`);
	});
});

describe('the staged set', () => {
	test('re-arms the posix CLI binary to 0o755 and verifies it, never the Windows one', async () => {
		const world = fake_world({ replies: not_on_registry });
		strictEqual(await run_publish_napi(world.deps, []), 0);
		const binary = at(`${PKG}/linux-x64-gnu/tsv`);
		deepStrictEqual(
			world.calls.filter(([op]) => op === 'set_mode' || op === 'mode'),
			[
				['set_mode', binary, 0o755],
				['mode', binary]
			]
		);
	});

	test('the mode is set before any npm run', async () => {
		const world = fake_world({ replies: not_on_registry });
		await run_publish_napi(world.deps, []);
		const first_npm = world.calls.findIndex(([op]) => op === 'npm');
		const last_mode = world.calls.findLastIndex(([op]) => op === 'mode');
		strictEqual(last_mode < first_npm, true);
	});

	test('a Windows host sets no mode', async () => {
		const world = fake_world({ replies: not_on_registry, platform: 'win32' });
		strictEqual(await run_publish_napi(world.deps, []), 0);
		deepStrictEqual(
			world.calls.filter(([op]) => op === 'set_mode' || op === 'mode'),
			[]
		);
	});

	test('a mode that does not take refuses the set, and nothing runs npm', async () => {
		const world = fake_world({ replies: not_on_registry, resist_mode: true });
		strictEqual(await run_publish_napi(world.deps, []), 1);
		deepStrictEqual(world.err, [
			`NOT EXECUTABLE: ${PKG}/linux-x64-gnu/tsv — chmod 755 did not take`,
			'FAIL: refusing to publish a partial set'
		]);
		deepStrictEqual(world.npm, []);
	});

	test('a missing binary refuses the partial set, naming each missing file', async () => {
		const files = staged_files();
		files.delete(at(`${PKG}/linux-x64-gnu/tsv_napi.node`));
		files.delete(at(`${PKG}/win32-x64/tsv.exe`));
		const world = fake_world({ files, replies: not_on_registry });
		strictEqual(await run_publish_napi(world.deps, ['--provenance']), 1);
		deepStrictEqual(world.err, [
			`MISSING: ${PKG}/linux-x64-gnu/tsv_napi.node (${GNU}) — did its matrix job fail?`,
			`MISSING: ${PKG}/win32-x64/tsv.exe (${WIN}) — did its matrix job fail?`,
			'FAIL: refusing to publish a partial set'
		]);
		deepStrictEqual(world.npm, []);
		deepStrictEqual(
			world.calls.filter(([op]) => op === 'set_mode'),
			[]
		);
	});

	test('a platform staged at another version, under another name, or pinned elsewhere is refused', async () => {
		for (const [name, version, pin] of [
			[GNU, '1.2.2', VERSION],
			['@fuzdev/tsv-other', VERSION, VERSION],
			[GNU, VERSION, '1.2.2']
		] as const) {
			const files = staged_files({ [GNU]: pin });
			files.set(at(`${PKG}/linux-x64-gnu/package.json`), JSON.stringify({ name, version }));
			const world = fake_world({ files, replies: not_on_registry });
			strictEqual(await run_publish_napi(world.deps, []), 1);
			deepStrictEqual(world.err, [
				`MISMATCH: ${PKG}/linux-x64-gnu stages ${name}@${version}, loader pins ${GNU}@${pin}, ` +
					`loader version ${VERSION}`,
				'FAIL: refusing to publish a partial set'
			]);
			deepStrictEqual(world.npm, []);
		}
	});

	test('a tag the staged version disagrees with is refused before the set is read', async () => {
		const world = fake_world({ replies: not_on_registry });
		strictEqual(await run_publish_napi(world.deps, ['--expect-tag', 'v1.2.4']), 1);
		deepStrictEqual(world.err, [
			`FAIL: staged version v${VERSION} does not match the triggering tag v1.2.4 — ` +
				'the checkout and the staged artifacts disagree'
		]);
		deepStrictEqual(world.calls, [['read_text', at(`${PKG}/napi/package.json`)]]);
	});

	test('an unreadable loader package.json fails, naming it and the error', async () => {
		const world = fake_world({ files: new Map() });
		strictEqual(await run_publish_napi(world.deps, []), 1);
		deepStrictEqual(world.err, [
			`FAIL: cannot read ${PKG}/napi/package.json — is the set staged?`,
			`Error: ENOENT: ${at(`${PKG}/napi/package.json`)}`
		]);
	});

	test('a platform package.json that does not parse fails at once', async () => {
		const files = staged_files();
		files.set(at(`${PKG}/win32-x64/package.json`), '{');
		const world = fake_world({ files, replies: not_on_registry });
		strictEqual(await run_publish_napi(world.deps, []), 1);
		strictEqual(
			world.err[0],
			`FAIL: cannot read ${PKG}/win32-x64/package.json — is the set staged?`
		);
		strictEqual(world.err.length, 2);
		deepStrictEqual(world.npm, []);
	});

	test('a loader with no platforms is refused', async () => {
		for (const optional of [undefined, {}]) {
			const files = new Map([
				[
					at(`${PKG}/napi/package.json`),
					JSON.stringify({ name: '@fuzdev/tsv', version: VERSION, optionalDependencies: optional })
				]
			]);
			const world = fake_world({ files });
			strictEqual(await run_publish_napi(world.deps, []), 1);
			deepStrictEqual(world.err, [
				'FAIL: loader package.json has no optionalDependencies — stale staging?'
			]);
		}
	});

	test('a platform name that climbs out of the staging is refused before its mode is set', async () => {
		const name = '../../../escape';
		const files = staged_files({ [name]: VERSION });
		const world = fake_world({ files, replies: not_on_registry });
		strictEqual(files.has(at(`${PKG}/${name}/tsv`)), true);
		await rejects(
			run_publish_napi(world.deps, []),
			/refusing to set a mode outside crates\/tsv_napi\/pkg/
		);
		deepStrictEqual(
			world.calls.filter(([op]) => op === 'set_mode'),
			[]
		);
		deepStrictEqual(world.npm, []);
	});

	test('every path it reads is beneath the checkout', async () => {
		const world = fake_world({ replies: not_on_registry });
		await run_publish_napi(world.deps, []);
		const paths = world.calls.filter(([op]) => op !== 'npm').map(([, path]) => String(path));
		strictEqual(paths.length > 0, true);
		deepStrictEqual(
			paths.filter((path) => !path.startsWith(`${at(PKG)}/`)),
			[]
		);
	});
});

describe('arguments', () => {
	test('an unknown flag or a positional throws before anything is read', async () => {
		for (const argv of [['--wetrun'], ['v1.2.3'], ['--', '--dry-run']]) {
			const world = fake_world();
			await rejects(run_publish_napi(world.deps, argv), TypeError);
			deepStrictEqual(world.calls, []);
		}
	});
});
