/**
 * `run_publish` over in-process fakes only: a checkout held in memory, fake program handles
 * that record each call's handle, argv, working directory, and stdin and answer from a small
 * model of git, npm, and the build, and recorded output. The whole dry run and the whole
 * `--wetrun` end to end, each refusal of the preflight, `--no-check` and `--no-git`, the order
 * of the publishes and what a republish error does, and the sentinel's life across a failure
 * at each step with the resume from each. Nothing here imports a host module, so nothing can
 * spawn, write, or reach the network; the entry is never run.
 *
 * @module
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';

import type { FileInfo, FsDir } from '@fuzdev/gro/fs.ts';
import { FsError } from '@fuzdev/gro/fs_error.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { describe, test } from 'vitest';

import { PUBLISH_PACKAGES, type PublishDeps, run_publish } from './publish.ts';

const ROOT = '/w';
const HEAD_SHA = 'a'.repeat(40);
const CORPORA = { root: '../corpora', collections: '../corpora/collections' };

const cargo_toml = (version: string): string =>
	'[workspace]\nmembers = ["crates/tsv_lang"]\n\n' +
	`[workspace.package]\nversion = "${version}"\n\n` +
	`[workspace.dependencies]\ntsv_lang = { path = "crates/tsv_lang", version = "=${version}" }\n`;

const CHANGELOG =
	'# tsv changelog\n\n## Unreleased\n<!-- bump: patch -->\n\n- fix: a thing\n\n## 0.8.1\n\n- old\n';

const SIDECAR_LOCK = JSON.stringify({
	packages: { '': { name: 'sidecar' }, 'node_modules/a': { version: '1.0.0' } }
});

/** Every path the preflight probes, present unless a test removes it. */
const ORACLES = [
	'benches/js/node_modules/typescript/lib/typescript.js',
	'benches/js/node_modules/@types/node/package.json',
	'../svelte/packages/svelte/tests',
	'../acorn-typescript/test',
	'../typescript/tests',
	'../test262/test',
	'../prettier/tests/format',
	'../prettier-plugin-svelte/test',
	'../corpora/collections',
	'benches/js/node_modules'
];

/** What a fake call answers. */
interface Answer {
	code?: number;
	stdout?: string;
	stderr?: string;
}

/** One recorded call or write, as `<handle> <method> <argv> @<cwd>[ stdin]`, or `<op> <path>`. */
type Event = string;

/** The model the fakes answer from, shared by every run over it so a resume sees what a failed
 * run left. */
interface World {
	files: Map<string, string>;
	exists: Set<string>;
	/** The tracked content of each release file at HEAD. */
	committed: Map<string, string>;
	head: string;
	head_subject: string;
	origin_main: string;
	/** Commits HEAD is behind origin/main. */
	behind: number;
	branch: string;
	local_tags: Map<string, string>;
	remote_tags: Map<string, string>;
	published: Set<string>;
	/** Status lines beyond the release files, such as a stray edit. */
	extra_status: Array<string>;
	events: Array<Event>;
	out: Array<string>;
	err: Array<string>;
	/** Answers that replace the model's for one call key (`<handle> <argv>`), each used once
	 * unless `sticky`. */
	overrides: Map<string, { answer: Answer; sticky: boolean; then?: (world: World) => void }>;
	/** Versions `node --version` and the others report. */
	versions: Record<string, string | null>;
	/** Programs that aren't there. */
	missing: Set<string>;
	commits: number;
	pushes: number;
	/** The version `build:packages` stamps its packages at, else Cargo.toml's. */
	build_version: string | null;
	/** The index of the `npm publish` npm refuses, counting from 0 across the world's life. */
	refuse_publish: number | null;
	publishes: number;
	/** What `npm config get dry-run` prints. */
	npm_dry_run: string;
	/** Whether a real `npm publish` exits 0 having published nothing, as one under npm's
	 * dry-run setting does. */
	noop_publish: boolean;
	/** How many `npm view` reads miss a version after its publish before the registry lists
	 * it, counted per version in `lag_left`. */
	lag: number;
	lag_left: Map<string, number>;
	/** The `scripts` `build:packages` stamps into the first package's manifest. */
	staged_scripts: unknown;
}

const RELEASE_FILES = ['Cargo.toml', 'Cargo.lock', 'CHANGELOG.md'];

const make_world = (version = '0.8.1'): World => {
	const files = new Map<string, string>([
		['Cargo.toml', cargo_toml(version)],
		['Cargo.lock', `lock ${version}\n`],
		['CHANGELOG.md', CHANGELOG],
		['crates/tsv_debug/sidecar/package-lock.json', SIDECAR_LOCK],
		['crates/tsv_debug/sidecar/node_modules/.package-lock.json', SIDECAR_LOCK],
		['crates/tsv_debug/sidecar/node_modules/a/package.json', '{"version":"1.0.0"}']
	]);
	return {
		files,
		exists: new Set(ORACLES),
		committed: new Map(RELEASE_FILES.map((path) => [path, files.get(path)!])),
		head: HEAD_SHA,
		head_subject: 'feat: the last change',
		origin_main: HEAD_SHA,
		behind: 0,
		branch: 'main',
		local_tags: new Map(),
		remote_tags: new Map(),
		published: new Set(),
		extra_status: [],
		events: [],
		out: [],
		err: [],
		overrides: new Map(),
		versions: { node: 'v24.14.1', 'wasm-pack': 'wasm-pack 0.15.0', bun: '1.3.0' },
		missing: new Set(),
		commits: 0,
		pushes: 0,
		build_version: null,
		refuse_publish: null,
		publishes: 0,
		npm_dry_run: 'false',
		noop_publish: false,
		lag: 0,
		lag_left: new Map(),
		staged_scripts: undefined
	};
};

const cargo_version = (world: World): string =>
	/version = "([^"]+)"/.exec(world.files.get('Cargo.toml')!)![1]!;

const status_lines = (world: World, paths?: ReadonlyArray<string>): Array<string> => [
	...RELEASE_FILES.filter(
		(path) =>
			(paths === undefined || paths.includes(path)) &&
			world.files.get(path) !== world.committed.get(path)
	).map((path) => ` M ${path}`),
	...(paths === undefined ? world.extra_status : [])
];

/** The model's answer to `git` run by the `local` handle. */
const git_local_answer = (world: World, args: ReadonlyArray<string>): Answer => {
	const line = args.join(' ');
	if (line === 'branch --show-current') return { stdout: `${world.branch}\n` };
	if (line === 'rev-parse HEAD') return { stdout: `${world.head}\n` };
	if (line === 'log -1 --format=%s') return { stdout: `${world.head_subject}\n` };
	if (line === 'rev-list --left-right --count origin/main...HEAD') {
		const ahead = world.head === world.origin_main ? 0 : 1;
		return { stdout: `${world.behind}\t${ahead}\n` };
	}
	if (line === 'status --porcelain' || line === 'status --porcelain --untracked-files=all') {
		const lines = status_lines(world);
		return { stdout: lines.length > 0 ? `${lines.join('\n')}\n` : '' };
	}
	if (args[0] === 'status' && args[1] === '--porcelain' && args[2] === '--') {
		const lines = status_lines(world, args.slice(3));
		return { stdout: lines.length > 0 ? `${lines.join('\n')}\n` : '' };
	}
	if (line === 'diff --binary HEAD') {
		return {
			stdout: RELEASE_FILES.filter((path) => world.files.get(path) !== world.committed.get(path))
				.map((path) => `${path}\n${world.files.get(path)}`)
				.join('\n')
		};
	}
	const rev = /^rev-parse -q --verify (.+)\^\{commit\}$/.exec(line);
	if (rev) {
		const name = rev[1]!;
		if (name === 'HEAD') return { stdout: `${world.head}\n` };
		const tag = /^refs\/tags\/(.+)$/.exec(name);
		const commit = tag ? world.local_tags.get(tag[1]!) : undefined;
		return commit === undefined ? { code: 1 } : { stdout: `${commit}\n` };
	}
	throw new Error(`git local: a call the model does not answer: ${line}`);
};

/** The model's answer to `git` run by the `user` handle. */
const git_user_answer = (world: World, args: ReadonlyArray<string>): Answer => {
	const line = args.join(' ');
	if (line === 'fetch origin main') return {};
	if (args[0] === 'ls-remote') {
		const tag = /refs\/tags\/(.+)$/.exec(args[3]!)![1]!;
		const commit = world.remote_tags.get(tag);
		return commit === undefined
			? {}
			: {
					stdout: `${'t'.repeat(40)}\trefs/tags/${tag}\n${commit}\trefs/tags/${tag}^{}\n`
				};
	}
	if (args[0] === 'commit') {
		world.commits++;
		world.head = `c${world.commits}`.padEnd(40, '0');
		world.head_subject = args[2]!;
		for (const path of args.slice(4)) world.committed.set(path, world.files.get(path)!);
		return {};
	}
	if (args[0] === 'tag') {
		world.local_tags.set(args[3]!, world.head);
		return {};
	}
	throw new Error(`git user: a call the model does not answer: ${line}`);
};

/** The model's answer to an `npm` call run in `cwd`. */
const npm_answer = (world: World, args: ReadonlyArray<string>, cwd: string): Answer => {
	if (args[0] === 'run') {
		// `build:packages` stages every package; the other scripts are gates that pass
		if (args[1] === 'build:packages') build_packages(world);
		return {};
	}
	if (args[0] === 'whoami') return { stdout: 'owner\n' };
	if (args.join(' ') === 'config get dry-run') return { stdout: `${world.npm_dry_run}\n` };
	if (args[0] === 'view') {
		const spec = args[1]!;
		const version = spec.slice(spec.lastIndexOf('@') + 1);
		const lag = world.lag_left.get(spec) ?? 0;
		if (lag > 0) {
			world.lag_left.set(spec, lag - 1);
			return {};
		}
		return world.published.has(spec) ? { stdout: `${version}\n` } : {};
	}
	if (args[0] === 'publish') {
		const dir = cwd.slice(ROOT.length + 1);
		const { name, version } = JSON.parse(world.files.get(`${dir}/package.json`)!);
		if (args[1] === '--dry-run') {
			return world.published.has(`${name}@${version}`)
				? {
						code: 1,
						stderr:
							'npm notice tarball\nnpm error You cannot publish over the previously published versions: ' +
							version
					}
				: { stderr: 'npm notice tarball' };
		}
		if (world.publishes++ === world.refuse_publish) return { code: 1 };
		if (world.noop_publish) return {};
		world.published.add(`${name}@${version}`);
		world.lag_left.set(`${name}@${version}`, world.lag);
		return {};
	}
	throw new Error(`npm: a call the model does not answer: ${args.join(' ')}`);
};

/** `build:packages`: every package staged at the checkout's version, its wasm sized. */
const build_packages = (world: World): void => {
	const version = world.build_version ?? cargo_version(world);
	for (const [i, { label, dir }] of PUBLISH_PACKAGES.entries()) {
		const scripts = i === 0 ? world.staged_scripts : undefined;
		world.files.set(`${dir}/package.json`, JSON.stringify({ name: label, version, scripts }));
		world.files.set(`${dir}/tsv_wasm_bg.wasm`, 'w'.repeat(1000 * (i + 1)));
	}
};

const answer_of = (
	world: World,
	handle: string,
	method: 'out' | 'run',
	args: ReadonlyArray<string>,
	cwd: string
): Answer => {
	const key = `${handle} ${args.join(' ')}`;
	const override = world.overrides.get(key);
	if (override !== undefined) {
		if (!override.sticky) world.overrides.delete(key);
		override.then?.(world);
		return override.answer;
	}
	if (args[0] === '--version') {
		const version = world.versions[handle === 'wasm_pack' ? 'wasm-pack' : handle];
		return version === null || version === undefined ? { code: 1 } : { stdout: `${version}\n` };
	}
	if (handle === 'git_local') return git_local_answer(world, args);
	if (handle === 'git_user') {
		if (method === 'run') {
			world.pushes++;
			world.origin_main = world.head;
			for (const [tag, commit] of world.local_tags) world.remote_tags.set(tag, commit);
			return {};
		}
		return git_user_answer(world, args);
	}
	if (handle === 'npm') return npm_answer(world, args, cwd);
	if (handle === 'cargo') {
		world.files.set('Cargo.lock', `lock ${cargo_version(world)}\n`);
		return {};
	}
	return {};
};

/** A fake program handle named `handle`, recording each call into the world. */
const fake_program = (world: World, handle: string, name: string): SpawnProgram | null => {
	if (world.missing.has(handle)) return null;
	const record = (method: string, args: ReadonlyArray<string>, options?: object): string => {
		const { cwd, stdin } = (options ?? {}) as { cwd?: { path: string }; stdin?: string };
		if (cwd === undefined) throw new Error(`${handle} ${method} without a cwd`);
		world.events.push(
			`${handle} ${method} ${args.join(' ')} @${cwd.path}${stdin === undefined ? '' : ` ${stdin}`}`
		);
		return cwd.path;
	};
	return {
		name,
		path: `/usr/bin/${name}`,
		out: async (args: ReadonlyArray<string>, options?: object) => {
			const cwd = record('out', args, options);
			const answer = answer_of(world, handle, 'out', args, cwd);
			const code = answer.code ?? 0;
			return {
				kind: 'exited',
				code,
				stdout: answer.stdout ?? '',
				stderr: answer.stderr ?? '',
				truncated: false
			};
		},
		run: async (args: ReadonlyArray<string>, options?: object) => {
			const cwd = record('run', args, options);
			return { kind: 'exited', code: answer_of(world, handle, 'run', args, cwd).code ?? 0 };
		},
		out_bytes: async (args: ReadonlyArray<string>) => {
			world.events.push(`${handle} out_bytes ${args.join(' ')}`);
			const path = args[1]!.slice(ROOT.length + 1);
			// a stand-in for gzip's count: a tenth of the file
			const size = Math.ceil((world.files.get(path) ?? '').length / 10);
			return {
				kind: 'exited',
				code: 0,
				stdout: new Uint8Array(size),
				stderr: new Uint8Array(),
				truncated: false
			};
		},
		start: () => Promise.reject(new Error('not a call publish makes'))
	} as unknown as SpawnProgram;
};

/** A checkout handle at `path` over the world's files, recording each write and removal. */
const fake_dir = (world: World, path: string): FsDir => {
	const key = (p: string) => (path === ROOT ? p : `${path.slice(ROOT.length + 1)}/${p}`);
	const missing = (p: string) => new FsError('not_found', `${path}/${p}`);
	const read = (p: string): string => {
		const text = world.files.get(key(p));
		if (text === undefined) throw missing(p);
		return text;
	};
	const dir = {
		path,
		dir: (sub: string) => fake_dir(world, `${path}/${sub}`),
		read_text: async (p: string) => read(p),
		read_text_sync: (p: string) => read(p),
		read_text_if_exists: async (p: string) => world.files.get(key(p)) ?? null,
		exists_sync: (p: string) => world.files.has(key(p)),
		stat: async (p: string): Promise<FileInfo | null> => {
			const text = world.files.get(key(p));
			return text === undefined
				? null
				: { kind: 'file', size: text.length, mtime_ms: 0, mode: 0o644 };
		},
		write: async (p: string, data: string) => {
			world.events.push(`write ${key(p)} ${JSON.stringify(data.length > 40 ? '…' : data)}`);
			const override = world.overrides.get(`write ${key(p)}`);
			if (override !== undefined) {
				world.overrides.delete(`write ${key(p)}`);
				throw new FsError('io_error', `${path}/${p}`);
			}
			world.files.set(key(p), data);
		},
		remove: async (p: string, options?: { missing_ok?: boolean }) => {
			world.events.push(`remove ${key(p)}`);
			if (!world.files.has(key(p)) && !options?.missing_ok) throw missing(p);
			world.files.delete(key(p));
		}
	};
	return dir as unknown as FsDir;
};

const deps_of = (world: World): PublishDeps => ({
	checkout: fake_dir(world, ROOT),
	exists: async (path) => world.exists.has(path),
	corpora: CORPORA,
	programs: {
		node: fake_program(world, 'node', 'node'),
		node_test: (pkg_dir) => fake_program(world, `node_test[${pkg_dir}]`, 'node'),
		npm: fake_program(world, 'npm', 'npm'),
		git: {
			local: fake_program(world, 'git_local', 'git'),
			user: fake_program(world, 'git_user', 'git')
		},
		cargo: fake_program(world, 'cargo', 'cargo'),
		wasm_pack: fake_program(world, 'wasm_pack', 'wasm-pack'),
		bun: fake_program(world, 'bun', 'bun'),
		gzip: fake_program(world, 'gzip', 'gzip')
	},
	print: (line) => {
		world.out.push(line);
		world.events.push(`out ${line}`);
	},
	print_error: (line) => {
		world.err.push(line);
		world.events.push(`err ${line}`);
	},
	sleep: async (ms) => {
		world.events.push(`sleep ${ms}`);
	}
});

/** One run over `world`, its events, output, and exit code read from the world after. */
const publish = async (world: World, argv: Array<string>) => {
	world.events.length = 0;
	world.out.length = 0;
	world.err.length = 0;
	const code = await run_publish(deps_of(world), argv);
	return {
		code,
		out: world.out.join('\n'),
		err: world.err.join('\n'),
		calls: world.events.filter((e) => !e.startsWith('out ') && !e.startsWith('err ')),
		events: [...world.events]
	};
};

/** Whether nothing was written, built, or published: no write, no build, no publish. */
const nothing_mutated = (calls: Array<string>): boolean =>
	!calls.some(
		(c) =>
			c.startsWith('write ') ||
			c.startsWith('remove ') ||
			c.startsWith('npm run run ') ||
			c.startsWith('cargo') ||
			c.startsWith('npm run publish') ||
			/^git_user out (commit|tag)|^git_user run/.test(c)
	);

const W = `@${ROOT}`;
const DIRS = PUBLISH_PACKAGES.map((p) => p.dir);

/** The preflight's calls on a clean checkout releasing `version` (fresh). */
const preflight_calls = (version: string): Array<string> => [
	`git_local out branch --show-current ${W}`,
	`git_local out rev-parse -q --verify HEAD^{commit} ${W}`,
	`git_user out fetch origin main ${W} inherit`,
	`git_local out rev-list --left-right --count origin/main...HEAD ${W}`,
	`git_local out status --porcelain ${W}`,
	`git_local out rev-parse -q --verify refs/tags/v${version}^{commit} ${W}`,
	`git_user out ls-remote --tags origin refs/tags/v${version} refs/tags/v${version}^{} ${W} inherit`,
	`npm out whoami ${W} inherit`,
	`npm out config get dry-run ${W} inherit`,
	`node out --version ${W}`,
	`wasm_pack out --version ${W}`,
	`bun out --version ${W}`,
	...PUBLISH_PACKAGES.map(({ label }) => `npm out view ${label}@${version} version ${W} inherit`)
];

/** Steps 3 to 6's calls, after the first fingerprint. */
const gate_calls = (options: { no_check?: boolean } = {}): Array<string> => [
	...(options.no_check
		? []
		: [
				`npm run run check ${W}`,
				`npm run run test:depth:napi ${W}`,
				`npm run run conformance:all ${W}`,
				`npm run run audit:corpus ${W}`
			]),
	`npm run run build:packages ${W}`,
	`npm run run validate:artifacts ${W}`,
	...DIRS.map((dir) => `node_test[${dir}] run --test scripts/test_npm.ts ${W}`),
	...(options.no_check ? [] : [`npm run run test:depth:wasm ${W}`]),
	`npm run run test:bun -- --require-bun format parse all ${W}`,
	`node run scripts/roots/build_napi_packages.ts --loader-only ${W}`,
	`npm run run typecheck:packages -- format parse all napi ${W}`
];

const FINGERPRINT = [
	`git_local out rev-parse HEAD ${W}`,
	`git_local out status --porcelain --untracked-files=all ${W}`,
	`git_local out diff --binary HEAD ${W}`
];

const GZIP = DIRS.map((dir) => `gzip out_bytes -c ${ROOT}/${dir}/tsv_wasm_bg.wasm`);

describe('the dry run', () => {
	test('runs every gate and build, publishes nothing, and writes nothing', async () => {
		const world = make_world();
		const run = await publish(world, ['--bump', 'patch']);
		strictEqual(run.code, 0, run.err);
		deepStrictEqual(run.calls, [
			...preflight_calls('0.8.2'),
			...FINGERPRINT,
			...gate_calls(),
			...FINGERPRINT,
			...DIRS.map((dir) => `npm out publish --dry-run --access public @${ROOT}/${dir} inherit`),
			...GZIP
		]);
		ok(nothing_mutated(run.calls.filter((c) => !c.startsWith('npm run run '))));
		strictEqual(world.files.get('Cargo.toml'), cargo_toml('0.8.1'));
		strictEqual(world.files.get('CHANGELOG.md'), CHANGELOG);
		strictEqual(world.files.has('.publish-in-progress'), false);
		strictEqual(world.published.size, 0);
		ok(run.out.includes('=== tsv publish (dry-run) ==='));
		ok(run.out.includes('  Version: 0.8.1 -> 0.8.2 (patch)'));
		ok(run.out.includes('  A wetrun bumps to v0.8.2'));
		ok(run.out.includes('  npm authenticated as: owner'));
		ok(run.out.includes('  PASS: @fuzdev/tsv-format-wasm = v0.8.1'));
		ok(run.out.includes('  PASS: @fuzdev/tsv-wasm packs cleanly'));
		ok(run.out.includes('npm notice tarball'));
		ok(run.out.includes('  @fuzdev/tsv-format-wasm@0.8.1 — wasm 1.0 KB (100 B gzipped wire)'));
		ok(run.out.endsWith('  Run with --wetrun --bump patch to publish v0.8.2.\n'), run.out);
		strictEqual(run.err, '');
	});

	test('a republish refusal from the dry-run publish passes a fresh release', async () => {
		const world = make_world();
		world.published = new Set(PUBLISH_PACKAGES.map(({ label }) => `${label}@0.8.1`));
		const run = await publish(world, ['--bump', 'patch']);
		strictEqual(run.code, 0, run.err);
		ok(
			run.out.includes(
				'  PASS: @fuzdev/tsv-parse-wasm packs cleanly (v0.8.1 already published — a real run publishes v0.8.2)'
			)
		);
	});

	test('any other dry-run publish failure fails, printing npm stdout to stderr', async () => {
		const world = make_world();
		world.overrides.set('npm publish --dry-run --access public', {
			answer: { code: 1, stdout: 'npm out', stderr: 'npm error E403 forbidden' },
			sticky: false
		});
		const run = await publish(world, ['--bump', 'patch']);
		strictEqual(run.code, 1);
		ok(run.out.includes('npm error E403 forbidden'));
		ok(run.err.endsWith('npm out\n  FAIL: npm publish --dry-run @fuzdev/tsv-format-wasm'), run.err);
		strictEqual(run.calls.filter((c) => c.startsWith('npm out publish')).length, 1);
		strictEqual(
			run.calls.some((c) => c.startsWith('gzip')),
			false
		);
	});

	test('a republish refusal fails a dry run of a resume', async () => {
		const world = make_world('0.8.2');
		world.files.set('.publish-in-progress', '0.8.2\n');
		world.published = new Set([`${PUBLISH_PACKAGES[0]!.label}@0.8.2`]);
		const run = await publish(world, []);
		strictEqual(run.code, 1);
		ok(run.out.includes('  Sentinel found at v0.8.2 — resuming that release'));
		ok(run.err.includes('  FAIL: npm publish --dry-run @fuzdev/tsv-format-wasm'));
	});

	test('the refusals a wetrun makes are warnings, and the run says a wetrun would fail', async () => {
		const world = make_world();
		world.branch = 'feature';
		world.extra_status = ['?? stray.txt'];
		const run = await publish(world, ['--bump', 'patch']);
		strictEqual(run.code, 0, run.err);
		ok(run.err.includes('  WARN: on branch "feature" — publish from main — a wetrun would FAIL'));
		ok(run.err.includes('  WARN: uncommitted changes: — a wetrun would FAIL\n    ?? stray.txt'));
		ok(
			run.out.endsWith(
				'\n  Dry-run complete for v0.8.1 — a wetrun would FAIL on the warnings above.\n'
			)
		);
		ok(run.calls.includes(`npm run run check ${W}`));
	});

	test('a resume dry run whose packages are all on npm says only git remains, exit 0', async () => {
		const world = make_world('0.8.2');
		world.files.set('.publish-in-progress', '0.8.2\n');
		world.committed.set('Cargo.toml', cargo_toml('0.8.1'));
		world.published = new Set(PUBLISH_PACKAGES.map(({ label }) => `${label}@0.8.2`));
		const run = await publish(world, []);
		strictEqual(run.code, 0, run.err);
		ok(run.out.includes('\n  Every package is already on npm at v0.8.2 — only git remains.'));
		ok(
			run.out.endsWith('  Dry-run complete — `npm run release -- --wetrun` finishes the release.\n')
		);
		strictEqual(
			run.calls.some((c) => c.startsWith('npm run run ')),
			false
		);
		strictEqual(
			run.calls.some((c) => c.startsWith('git_user out commit')),
			false
		);
	});

	test('the checkout moving under the gates warns a dry run', async () => {
		const world = make_world();
		world.overrides.set('npm run validate:artifacts', {
			answer: {},
			sticky: false,
			then: (w) => w.files.set('Cargo.toml', `${w.files.get('Cargo.toml')}# edited\n`)
		});
		const run = await publish(world, ['--bump', 'patch']);
		strictEqual(run.code, 0, run.err);
		ok(
			run.err.includes(
				'  WARN: the checkout changed while the gates ran — the built packages may match no commit'
			)
		);
		ok(run.out.includes('a wetrun would FAIL on the warnings above.'));
	});
});

describe('flags', () => {
	test('an unknown argument exits 1 with the usage, before anything runs', async () => {
		const world = make_world();
		const run = await publish(world, ['--wetrun', '--no-gti']);
		strictEqual(run.code, 1);
		strictEqual(
			run.err,
			'FAIL: unknown argument(s): --no-gti\n' +
				'Usage: npm run release -- [--wetrun] [--bump patch|minor|major] [--no-check] [--no-git]'
		);
		deepStrictEqual(run.calls, []);
	});

	test('a bad or missing --bump level exits 1', async () => {
		for (const [argv, got] of [
			[['--bump', 'huge'], '"huge"'],
			[['--bump'], '""']
		] as const) {
			const run = await publish(make_world(), [...argv]);
			strictEqual(run.code, 1);
			strictEqual(run.err, `FAIL: --bump must be patch, minor, or major (got ${got})`);
			deepStrictEqual(run.calls, []);
		}
	});

	test('--no-check skips the check, depth, conformance, and corpus legs and the sidecar', async () => {
		const world = make_world();
		world.files.delete('crates/tsv_debug/sidecar/node_modules/.package-lock.json');
		world.versions.bun = null;
		const run = await publish(world, ['--wetrun', '--bump', 'patch', '--no-check']);
		strictEqual(run.code, 0, run.err);
		const gates = run.calls.filter(
			(c) => / run /.test(c) && !c.startsWith('npm run publish') && !c.startsWith('git')
		);
		deepStrictEqual(
			gates,
			gate_calls({ no_check: true }).filter((c) => !c.includes('test:bun'))
		);
		ok(run.out.includes('\n=== Step 3: Check — SKIPPED (--no-check) ==='));
		ok(run.err.includes('  WARN: bun not found; waived by --no-check — Step 6 skips the Bun leg'));
		ok(run.err.includes('  WARN: skipping npm run test:depth:wasm (--no-check)'));
		ok(run.err.includes('\n  ⚠ Step 3b conformance gates did NOT run (--no-check).'));
		ok(run.err.includes('\n  ⚠ Step 3c corpus robustness audit did NOT run (--no-check).'));
		ok(
			run.err.includes(
				'\n  ⚠ Step 6 parse-failure table under Bun did NOT run (bun not found; waived by --no-check).'
			)
		);
	});
});

describe('the preflight', () => {
	/** A wetrun that must refuse with `message` and leave everything as it was. */
	const refuses = async (world: World, message: string, argv = ['--wetrun', '--bump', 'patch']) => {
		const run = await publish(world, argv);
		strictEqual(run.code, 1, run.out);
		ok(run.err.includes(message), `${message}\nnot in\n${run.err}`);
		ok(nothing_mutated(run.calls), run.calls.join('\n'));
		strictEqual(world.files.get('Cargo.toml'), cargo_toml(cargo_version(world)));
		return run;
	};

	test('a wetrun names every refusal at once, then refuses', async () => {
		const world = make_world();
		world.branch = '';
		world.behind = 2;
		world.extra_status = [' M src/x.rs'];
		world.local_tags.set('v0.8.2', 'b'.repeat(40));
		world.remote_tags.set('v0.8.2', 'b'.repeat(40));
		world.overrides.set('npm whoami', {
			answer: { code: 1, stderr: 'npm error code ENEEDAUTH\nmore' },
			sticky: false
		});
		const run = await refuses(world, 'Refusing to publish');
		deepStrictEqual(run.err.split('\n'), [
			'  FAIL: HEAD is detached — publish from main',
			'  FAIL: HEAD is 0 ahead / 2 behind origin/main — release from exactly origin/main',
			'  FAIL: uncommitted changes:',
			'     M src/x.rs',
			'  FAIL: tag v0.8.2 already exists locally',
			'  FAIL: tag v0.8.2 already exists on origin',
			'  FAIL: not logged in to npm (run `npm login` first): npm error code ENEEDAUTH',
			'  Refusing to publish — nothing was changed, built, or published.'
		]);
	});

	test('the plan problems and an unmovable pin refuse', async () => {
		await refuses(make_world(), '--bump', ['--wetrun']);
		const world = make_world();
		world.files.set(
			'Cargo.toml',
			`${cargo_toml('0.8.1')}tsv_x = { path = "crates/tsv_x", version = "0.8.1" }\n`
		);
		const run = await publish(world, ['--wetrun', '--bump', 'patch']);
		strictEqual(run.code, 1);
		ok(run.err.includes('is not an exact "=x.y.z" pin the bump can move'), run.err);
		ok(nothing_mutated(run.calls));
	});

	test('no workspace version exits 1 before git', async () => {
		const world = make_world();
		world.files.set('Cargo.toml', '[workspace]\n');
		const run = await publish(world, ['--wetrun', '--bump', 'patch']);
		strictEqual(run.code, 1);
		strictEqual(run.err, '  FAIL: could not find [workspace.package] version in Cargo.toml');
		deepStrictEqual(run.calls, []);
	});

	test('each git failure refuses, naming it', async () => {
		for (const [key, answer, message] of [
			[
				'git_local branch --show-current',
				{ code: 128 },
				'git branch failed — is this a git repository?'
			],
			['git_local rev-parse -q --verify HEAD^{commit}', { code: 128 }, 'git rev-parse HEAD failed'],
			[
				'git_user fetch origin main',
				{ code: 128, stderr: 'fatal: unreachable' },
				'git fetch origin main failed: fatal: unreachable — cannot verify sync with origin'
			],
			[
				'git_local rev-list --left-right --count origin/main...HEAD',
				{ stdout: 'garbage' },
				'could not compare HEAD with origin/main:  — cannot verify sync with origin'
			],
			['git_local status --porcelain', { code: 128, stderr: 'boom' }, 'git status failed: boom'],
			[
				'git_user ls-remote --tags origin refs/tags/v0.8.2 refs/tags/v0.8.2^{}',
				{ code: 2, stderr: 'denied' },
				'git ls-remote origin failed: denied'
			]
		] as const) {
			const world = make_world();
			world.overrides.set(key, { answer, sticky: true });
			await refuses(world, `  FAIL: ${message}`);
		}
	});

	test('a missing git refuses through each git question', async () => {
		const world = make_world();
		world.missing.add('git_local');
		world.missing.add('git_user');
		const run = await refuses(world, 'git branch failed — is this a git repository?');
		ok(run.err.includes('git fetch origin main failed: git: command not found'));
	});

	test('a missing or too old node, a missing wasm-pack, and a missing bun exit 1', async () => {
		for (const [version, message] of [
			[null, '  FAIL: node not found — required to test the built packages'],
			['v22.17.0', '  FAIL: node v22.17.0 — Step 6 needs >= 22.18'],
			['v23.5.0', '  FAIL: node v23.5.0 — Step 6 needs >= 22.18'],
			['nonsense', '  FAIL: node nonsense — Step 6 needs >= 22.18']
		] as const) {
			const world = make_world();
			world.versions.node = version;
			await refuses(world, message, ['--bump', 'patch']);
		}
		for (const version of ['v22.18.0', 'v23.6.0']) {
			const world = make_world();
			world.versions.node = version;
			strictEqual((await publish(world, ['--bump', 'patch'])).code, 0);
		}
		const world = make_world();
		world.missing.add('node');
		await refuses(world, '  FAIL: node not found');
		const wasm = make_world();
		wasm.versions['wasm-pack'] = null;
		await refuses(wasm, '  FAIL: wasm-pack not found — required to build the packages', [
			'--bump',
			'patch'
		]);
		const bun = make_world();
		bun.versions.bun = null;
		await refuses(bun, '  FAIL: bun not found — required to run the parse-failure table under Bun');
	});

	test('a dry run without bun warns, skips the Bun leg, and re-warns at the end', async () => {
		const world = make_world();
		world.versions.bun = null;
		const run = await publish(world, ['--bump', 'patch']);
		strictEqual(run.code, 0);
		ok(
			run.err.includes(
				'  WARN: bun not found — Step 6 skips the Bun leg; a --wetrun would FAIL here'
			)
		);
		ok(run.err.includes('  WARN: skipping npm run test:bun — bun not found'));
		strictEqual(
			run.calls.some((c) => c.includes('test:bun')),
			false
		);
		ok(
			run.err.endsWith(
				'    Run `npm run test:bun -- format parse all` with bun installed for this release.'
			)
		);
	});

	test('a missing typecheck install and a missing or stale sidecar exit 1, even dry', async () => {
		const ts = make_world();
		ts.exists.delete('benches/js/node_modules/@types/node/package.json');
		await refuses(ts, '  FAIL: benches/js/node_modules lacks @types/node — required', [
			'--bump',
			'patch'
		]);
		const missing = make_world();
		missing.files.delete('crates/tsv_debug/sidecar/node_modules/.package-lock.json');
		await refuses(missing, "  FAIL: the sidecar's packages are not installed", ['--bump', 'patch']);
		const stale = make_world();
		stale.files.set('crates/tsv_debug/sidecar/node_modules/a/package.json', '{"version":"2.0.0"}');
		await refuses(
			stale,
			"  FAIL: the sidecar's packages differ from its lock — required by Step 3's fixture oracle; run `npm ci --prefix crates/tsv_debug/sidecar`",
			['--bump', 'patch']
		);
	});

	test('a failed registry lookup and a version already on npm refuse a dry run too', async () => {
		const world = make_world();
		world.overrides.set('npm view @fuzdev/tsv-parse-wasm@0.8.2 version', {
			answer: { code: 1, stderr: 'npm error ETIMEDOUT' },
			sticky: false
		});
		const run = await refuses(world, 'Refusing to run', ['--bump', 'patch']);
		ok(
			run.err.includes('  FAIL: npm view @fuzdev/tsv-parse-wasm@0.8.2 failed:\nnpm error ETIMEDOUT')
		);
		const live = make_world();
		live.published.add('@fuzdev/tsv-wasm@0.8.2');
		await refuses(live, '  FAIL: @fuzdev/tsv-wasm@0.8.2 is already on npm', ['--bump', 'patch']);
		const e404 = make_world();
		e404.overrides.set('npm view @fuzdev/tsv-wasm@0.8.2 version', {
			answer: { code: 1, stderr: 'npm error code E404' },
			sticky: false
		});
		strictEqual((await publish(e404, ['--bump', 'patch'])).code, 0);
	});

	test('each probed path is required: missing, it fails the run that needs it', async () => {
		for (const path of ORACLES) {
			const world = make_world();
			world.exists.delete(path);
			const run = await publish(world, ['--wetrun', '--bump', 'patch']);
			strictEqual(run.code, 1, path);
			strictEqual(
				run.calls.some((c) => c.startsWith(`npm run run build:packages`)),
				false,
				path
			);
		}
	});

	test('a missing oracle fails a wetrun at Step 3b, after the bump, and warns a dry run', async () => {
		const world = make_world();
		world.exists.delete('../test262/test');
		const run = await publish(world, ['--wetrun', '--bump', 'patch']);
		strictEqual(run.code, 1);
		ok(
			run.err.includes(
				'  FAIL: missing ../test262 checkout — the conformance gates cannot run. Clone/install'
			)
		);
		strictEqual(world.files.get('.publish-in-progress'), '0.8.2\n');
		const dry = make_world();
		dry.exists.delete('../corpora/collections');
		const dry_run = await publish(dry, ['--bump', 'patch']);
		strictEqual(dry_run.code, 0);
		ok(
			dry_run.err.includes(
				'  WARN: skipping — missing ../corpora checkout (the real-code snapshot). A --wetrun would FAIL here'
			)
		);
		ok(
			dry_run.err.includes(
				'  WARN: skipping — missing ../corpora checkout (the real-code snapshot — the corpus floor).'
			)
		);
		strictEqual(
			dry_run.calls.some((c) => c.includes('conformance:all') || c.includes('audit:corpus')),
			false
		);
	});
});

/** The calls of a fresh wetrun from Step 2 on, its version `version`. */
const wetrun_tail = (version: string): Array<string> => [
	`write Cargo.toml "…"`,
	`write .publish-in-progress "${version}\\n"`,
	`cargo out update --workspace --offline ${W}`,
	`write CHANGELOG.md "…"`,
	...FINGERPRINT,
	...gate_calls(),
	...FINGERPRINT,
	...PUBLISH_PACKAGES.flatMap(({ label, dir }) => [
		`npm out view ${label}@${version} version ${W} inherit`,
		`npm run publish --access public @${ROOT}/${dir}`,
		`npm out view ${label}@${version} version ${W} inherit`
	]),
	`git_local out status --porcelain -- Cargo.toml Cargo.lock CHANGELOG.md ${W}`,
	`git_user out commit -m chore: publish v${version} -- Cargo.toml Cargo.lock CHANGELOG.md ${W} inherit`,
	`git_local out rev-parse -q --verify refs/tags/v${version}^{commit} ${W}`,
	`git_user out tag -m v${version} v${version} ${W} inherit`,
	`git_user run push --follow-tags origin main ${W}`,
	'remove .publish-in-progress',
	...GZIP
];

describe('the wetrun', () => {
	test('bumps, gates, builds, publishes in order, commits, tags, pushes, and clears the sentinel', async () => {
		const world = make_world();
		const run = await publish(world, ['--wetrun', '--bump', 'patch']);
		strictEqual(run.code, 0, run.err);
		deepStrictEqual(run.calls, [...preflight_calls('0.8.2'), ...wetrun_tail('0.8.2')]);
		strictEqual(world.files.get('Cargo.toml'), cargo_toml('0.8.2'));
		strictEqual(world.files.get('Cargo.lock'), 'lock 0.8.2\n');
		strictEqual(
			world.files.get('CHANGELOG.md'),
			'# tsv changelog\n\n## Unreleased\n<!-- bump: patch -->\n\n## 0.8.2\n\n- fix: a thing\n\n## 0.8.1\n\n- old\n'
		);
		deepStrictEqual(
			[...world.published],
			PUBLISH_PACKAGES.map(({ label }) => `${label}@0.8.2`)
		);
		strictEqual(world.head_subject, 'chore: publish v0.8.2');
		strictEqual(world.local_tags.get('v0.8.2'), world.head);
		strictEqual(world.remote_tags.get('v0.8.2'), world.head);
		strictEqual(world.origin_main, world.head);
		strictEqual(world.files.has('.publish-in-progress'), false);
		ok(run.out.includes('  Version bumped (patch): 0.8.1 -> 0.8.2'));
		ok(
			run.out.includes(
				'  Stamped CHANGELOG.md: ## Unreleased -> ## 0.8.2; seeded fresh ## Unreleased (bump: patch)'
			)
		);
		ok(run.out.includes('  Committed "chore: publish v0.8.2"'));
		ok(run.out.includes('  Tagged v0.8.2'));
		ok(
			run.out.includes('  Published @fuzdev/tsv-wasm@0.8.2 — wasm 3.0 KB (300 B gzipped wire)'),
			run.out
		);
		ok(run.out.includes('\n  The pushed v0.8.2 tag triggers release.yml —'));
		ok(run.out.endsWith('  requires (its `crates` job is a dry run).\n'), run.out);
		strictEqual(run.err, '');
	});

	test('a stale sentinel is removed before the bump', async () => {
		const world = make_world();
		world.files.set('.publish-in-progress', '0.7.0\n');
		const run = await publish(world, ['--wetrun', '--bump', 'patch']);
		strictEqual(run.code, 0, run.err);
		ok(run.err.includes('  WARN: stale sentinel (v0.7.0) — a fresh wetrun removes it'));
		const i = run.calls.indexOf('remove .publish-in-progress');
		strictEqual(run.calls[i + 1], 'write Cargo.toml "…"');
	});

	test('--no-git prints the finalize instead of running it, then clears the sentinel', async () => {
		const world = make_world();
		const run = await publish(world, ['--wetrun', '--bump', 'patch', '--no-git']);
		strictEqual(run.code, 0, run.err);
		strictEqual(
			run.calls.some((c) => /^git_user (out (commit|tag)|run)/.test(c)),
			false
		);
		ok(
			run.out.includes(
				'\n=== Step 8: Git finalize — SKIPPED (--no-git) ===\n' +
					'  Finalize manually (only the release files; annotated tag — --follow-tags\n' +
					'  ignores lightweight tags):\n' +
					'    git add Cargo.toml Cargo.lock CHANGELOG.md\n' +
					'    git commit -m "chore: publish v0.8.2"\n' +
					'    git tag -m v0.8.2 v0.8.2\n' +
					'    git push --follow-tags origin main'
			)
		);
		ok(run.out.includes('\n  The N-API set does NOT publish until the v0.8.2 tag is pushed —'));
		ok(run.calls.includes('remove .publish-in-progress'));
		strictEqual(world.files.has('.publish-in-progress'), false);
	});

	test('a missing npm refuses a wetrun at the preflight, before anything is bumped', async () => {
		const world = make_world();
		world.missing.add('npm');
		const run = await publish(world, ['--wetrun', '--bump', 'patch']);
		strictEqual(run.code, 1);
		ok(
			run.err.includes(
				'  FAIL: not logged in to npm (run `npm login` first): npm: command not found'
			),
			run.err
		);
		ok(
			run.err.endsWith('  Refusing to publish — nothing was changed, built, or published.'),
			run.err
		);
		ok(nothing_mutated(run.calls));
	});

	test('a missing gzip fails the summary after the release is done', async () => {
		const world = make_world();
		world.missing.add('gzip');
		await publish(world, ['--wetrun', '--bump', 'patch']).then(
			() => ok(false, 'the summary should throw'),
			(error: Error) => strictEqual(error.message, 'gzip is not on PATH')
		);
		strictEqual(world.files.has('.publish-in-progress'), false);
		strictEqual(world.pushes, 1);
	});
});

describe('a failure at each step, and the resume', () => {
	/** Each failure point: the override that fails it, and what the failed run printed. */
	const points: Array<{
		at: string;
		fail: (world: World) => void;
		message: string;
		published_before?: number;
		committed_before?: boolean;
	}> = [
		{
			at: 'the lockfile update',
			fail: (w) =>
				w.overrides.set('cargo update --workspace --offline', {
					answer: { code: 101, stderr: 'error: offline' },
					sticky: false
				}),
			message: '  FAIL: cargo update --workspace for Cargo.lock failed:\nerror: offline'
		},
		...[
			'check',
			'test:depth:napi',
			'conformance:all',
			'audit:corpus',
			'validate:artifacts',
			'test:depth:wasm',
			'test:bun -- --require-bun format parse all',
			'typecheck:packages -- format parse all napi'
		].map((task) => ({
			at: `npm run ${task}`,
			fail: (w: World) =>
				w.overrides.set(`npm run ${task}`, { answer: { code: 1 }, sticky: false }),
			message: `  FAIL: npm run ${task.split(' ')[0]} (exit code 1)`
		})),
		{
			at: 'the build',
			fail: (w) =>
				w.overrides.set('npm run build:packages', {
					answer: { code: 2 },
					sticky: false
				}),
			message: '  FAIL: npm run build:packages (exit code 2)'
		},
		{
			at: 'Step 5',
			fail: (w) => {
				w.build_version = '0.0.1';
			},
			message: '  FAIL: @fuzdev/tsv-format-wasm built at v0.0.1, expected v0.8.2'
		},
		...DIRS.map((dir, i) => ({
			at: `the artifact tests of ${dir}`,
			fail: (w: World) =>
				w.overrides.set(`node_test[${dir}] --test scripts/test_npm.ts`, {
					answer: { code: 1 },
					sticky: false
				}),
			message: `  FAIL: artifact tests for ${PUBLISH_PACKAGES[i]!.label} (exit code 1)`
		})),
		{
			at: 'the loader staging',
			fail: (w) =>
				w.overrides.set('node scripts/roots/build_napi_packages.ts --loader-only', {
					answer: { code: 1 },
					sticky: false
				}),
			message: '  FAIL: stage the @fuzdev/tsv loader (exit code 1)'
		},
		{
			at: 'the second fingerprint',
			fail: (w) =>
				w.overrides.set('npm run validate:artifacts', {
					answer: {},
					sticky: false,
					then: (world) => world.extra_status.push('?? new.txt')
				}),
			message:
				'  FAIL: the checkout changed while the gates ran — the built packages may match no commit; refusing to publish'
		},
		{
			at: "Step 7's registry read",
			fail: (w) =>
				w.overrides.set('npm view @fuzdev/tsv-format-wasm@0.8.2 version', {
					answer: {},
					sticky: false,
					// the preflight's read of it passes; Step 7's is the one that fails
					then: (world) =>
						world.overrides.set('npm view @fuzdev/tsv-format-wasm@0.8.2 version', {
							answer: { code: 1, stderr: 'npm error ECONNRESET' },
							sticky: false
						})
				}),
			message: '  FAIL: npm view @fuzdev/tsv-format-wasm@0.8.2 failed:\nnpm error ECONNRESET'
		},
		...PUBLISH_PACKAGES.map(({ label }, i) => ({
			at: `the publish of ${label}`,
			// the publishes before this one land; this one is refused
			fail: (w: World) => {
				w.refuse_publish = i;
			},
			message: `  FAIL: npm publish ${label} (exit code 1)`,
			published_before: i
		})),
		{
			at: 'the commit',
			fail: (w) =>
				w.overrides.set(
					'git_user commit -m chore: publish v0.8.2 -- Cargo.toml Cargo.lock CHANGELOG.md',
					{
						answer: { code: 1, stderr: 'gpg failed to sign the data' },
						sticky: false
					}
				),
			message: '  FAIL: git commit failed: gpg failed to sign the data',
			published_before: 3
		},
		{
			at: 'the tag',
			fail: (w) =>
				w.overrides.set('git_user tag -m v0.8.2 v0.8.2', {
					answer: { code: 1, stderr: 'no pinentry' },
					sticky: false
				}),
			message: '  FAIL: git tag failed: no pinentry',
			published_before: 3,
			committed_before: true
		},
		{
			at: 'the push',
			fail: (w) =>
				w.overrides.set('git_user push --follow-tags origin main', {
					answer: { code: 128 },
					sticky: false
				}),
			message: '  FAIL: git push --follow-tags origin main failed',
			published_before: 3,
			committed_before: true
		}
	];

	for (const point of points) {
		test(`at ${point.at}: the failed run leaves the bump and the sentinel, and a resume finishes`, async () => {
			const world = make_world();
			point.fail(world);
			const failed = await publish(world, ['--wetrun', '--bump', 'patch']);
			strictEqual(failed.code, 1, failed.out);
			ok(failed.err.includes(point.message), `${point.message}\nnot in\n${failed.err}`);
			const git_step = point.published_before === 3;
			ok(
				failed.err.includes(
					git_step
						? 'The sentinel survives this step, so `npm run release -- --wetrun` resumes from here'
						: 'The bump is in place — fix the cause and run `npm run release -- --wetrun` to resume.'
				),
				failed.err
			);
			// what the failed run left
			strictEqual(world.files.get('.publish-in-progress'), '0.8.2\n');
			strictEqual(world.files.get('Cargo.toml'), cargo_toml('0.8.2'));
			strictEqual(
				world.files.get('CHANGELOG.md')!.includes('## 0.8.2\n'),
				point.at !== 'the lockfile update'
			);
			strictEqual(world.published.size, point.published_before ?? 0);
			strictEqual(world.commits, point.committed_before ? 1 : 0);
			strictEqual(world.pushes, 0);

			world.extra_status = [];
			world.build_version = null;
			world.overrides = new Map();
			world.refuse_publish = null;
			const resumed = await publish(world, ['--wetrun', '--bump', 'minor']);
			strictEqual(resumed.code, 0, resumed.err);
			ok(resumed.out.includes('  Sentinel found at v0.8.2 — resuming that release'));
			ok(resumed.err.startsWith('  WARN: '), resumed.err);
			deepStrictEqual(
				[...world.published].sort(),
				PUBLISH_PACKAGES.map(({ label }) => `${label}@0.8.2`).sort()
			);
			strictEqual(
				resumed.calls.some((c) => c.startsWith('write Cargo.toml')),
				false
			);
			strictEqual(world.commits, 1);
			strictEqual(world.pushes, 1);
			strictEqual(world.local_tags.get('v0.8.2'), world.head);
			strictEqual(world.remote_tags.get('v0.8.2'), world.head);
			strictEqual(world.files.has('.publish-in-progress'), false);
			strictEqual(world.files.get('Cargo.toml'), cargo_toml('0.8.2'));
			strictEqual(world.committed.get('CHANGELOG.md'), world.files.get('CHANGELOG.md'));
			// nothing published twice: each package's publish ran once across both runs
			const publishes = [...failed.calls, ...resumed.calls].filter((c) =>
				c.startsWith('npm run publish')
			);
			strictEqual(
				publishes.length -
					(point.published_before !== undefined && point.published_before < 3 ? 1 : 0),
				3
			);
			if (git_step) {
				// every package was on npm, so the resume went straight to the finalize
				ok(resumed.out.includes('  Every package is already on npm at v0.8.2 — only git remains.'));
				strictEqual(
					resumed.calls.some((c) => c.startsWith('npm run run ')),
					false
				);
			}
		});
	}

	test('a failure before the sentinel is written leaves nothing to resume', async () => {
		const world = make_world();
		world.overrides.set('write Cargo.toml', { answer: {}, sticky: false });
		await publish(world, ['--wetrun', '--bump', 'patch']).then(
			() => ok(false, 'the write should throw'),
			(error: FsError) => strictEqual(error.kind, 'io_error')
		);
		strictEqual(world.files.has('.publish-in-progress'), false);
		strictEqual(world.files.get('Cargo.toml'), cargo_toml('0.8.1'));
	});

	test('a resume refuses a stray change outside the release files', async () => {
		const world = make_world('0.8.2');
		world.committed.set('Cargo.toml', cargo_toml('0.8.1'));
		world.files.set('.publish-in-progress', '0.8.2\n');
		world.extra_status = [' M crates/tsv_lang/src/lib.rs'];
		const run = await publish(world, ['--wetrun']);
		strictEqual(run.code, 1);
		ok(
			run.err.includes(
				'  FAIL: uncommitted changes outside the release files (Cargo.toml, Cargo.lock, CHANGELOG.md):\n     M crates/tsv_lang/src/lib.rs'
			),
			run.err
		);
	});

	test("a resume refuses a tag on another commit, and a commit ahead that isn't its own", async () => {
		const tagged = make_world('0.8.2');
		tagged.files.set('.publish-in-progress', '0.8.2\n');
		tagged.remote_tags.set('v0.8.2', 'b'.repeat(40));
		const run = await publish(tagged, ['--wetrun']);
		strictEqual(run.code, 1);
		ok(run.err.includes(`  FAIL: tag v0.8.2 exists on origin on ${'b'.repeat(40)}, not on HEAD`));
		const ahead = make_world('0.8.2');
		ahead.files.set('.publish-in-progress', '0.8.2\n');
		ahead.head = 'd'.repeat(40);
		ahead.head_subject = 'feat: something else';
		const ahead_run = await publish(ahead, ['--wetrun']);
		strictEqual(ahead_run.code, 1);
		ok(
			ahead_run.err.includes(
				'  FAIL: HEAD is 1 ahead / 0 behind origin/main — release from exactly origin/main'
			)
		);
		ok(ahead_run.calls.includes(`git_local out log -1 --format=%s ${W}`));
	});
});

describe('flags spelled with a value, a suffix, or twice', () => {
	test('a flag spelled with a value or a suffix, and a positional, are unknown', async () => {
		for (const arg of ['--wetrun=false', '--wetrun=true', '--wetrunx', 'patch', '--bump=patch']) {
			const world = make_world();
			const run = await publish(world, [arg]);
			strictEqual(run.code, 1, arg);
			strictEqual(
				run.err,
				`FAIL: unknown argument(s): ${arg}\n` +
					'Usage: npm run release -- [--wetrun] [--bump patch|minor|major] [--no-check] [--no-git]',
				arg
			);
			deepStrictEqual(run.calls, [], arg);
		}
	});

	test('a flag given twice: the last --bump wins, a repeated --wetrun is one', async () => {
		const run = await publish(make_world(), ['--bump', 'minor', '--bump', 'patch']);
		ok(run.out.includes('  Version: 0.8.1 -> 0.8.2 (patch)'), run.out);
		const twice = await publish(make_world(), ['--wetrun', '--wetrun']);
		ok(twice.out.startsWith('\n=== tsv publish (wetrun) ==='), twice.out);
	});
});

describe('a resume whose changelog lost its section', () => {
	test('fails resumable at the stamp, after the lockfile, writing no changelog', async () => {
		const world = make_world();
		world.files.set('.publish-in-progress', '0.8.1\n');
		world.files.set('CHANGELOG.md', '# tsv changelog\n\n## 0.8.0\n\n- old\n');
		world.committed.set('CHANGELOG.md', world.files.get('CHANGELOG.md')!);
		const run = await publish(world, ['--wetrun']);
		strictEqual(run.code, 1, run.err);
		ok(
			run.err.endsWith(
				'  FAIL: no "## Unreleased" or "## 0.8.1" in CHANGELOG.md\n' +
					'  The bump is in place — fix the cause and run `npm run release -- --wetrun` to resume.'
			),
			run.err
		);
		ok(!run.calls.some((c) => c.startsWith('write CHANGELOG.md')), run.calls.join('\n'));
		ok(!run.calls.some((c) => / run /.test(c)), run.calls.join('\n'));
		strictEqual(world.files.get('.publish-in-progress'), '0.8.1\n');
	});
});

describe("npm's dry-run setting", () => {
	test('anything but false refuses a wetrun before the bump and warns a dry run', async () => {
		for (const value of ['true', '1', 'yes', 'FALSE', '0']) {
			const world = make_world();
			world.npm_dry_run = value;
			const run = await publish(world, ['--wetrun', '--bump', 'patch']);
			strictEqual(run.code, 1, value);
			ok(
				run.err.includes(
					`  FAIL: npm's dry-run setting is ${JSON.stringify(value)} (\`npm config get dry-run\`), ` +
						'so each `npm publish` would only pretend — unset `npm_config_dry_run`'
				),
				run.err
			);
			ok(run.err.endsWith('  Refusing to publish — nothing was changed, built, or published.'));
			ok(nothing_mutated(run.calls), run.calls.join('\n'));
		}
		const world = make_world();
		world.npm_dry_run = 'true';
		const dry = await publish(world, ['--bump', 'patch']);
		strictEqual(dry.code, 0, dry.err);
		ok(dry.err.includes(`  WARN: npm's dry-run setting is "true"`), dry.err);
		ok(dry.out.includes('a wetrun would FAIL on the warnings above'), dry.out);
	});

	test('a failed read of it refuses a wetrun', async () => {
		const world = make_world();
		world.overrides.set('npm config get dry-run', {
			answer: { code: 1, stderr: 'npm error config broken\nmore' },
			sticky: false
		});
		const run = await publish(world, ['--wetrun', '--bump', 'patch']);
		strictEqual(run.code, 1);
		ok(
			run.err.includes('  FAIL: npm config get dry-run failed: npm error config broken\n'),
			run.err
		);
		ok(nothing_mutated(run.calls));
	});
});

describe('the registry confirms each publish', () => {
	const confirm_reads = (label: string, reads: number): Array<string> =>
		Array.from({ length: reads }, (_, i) => [
			...(i > 0 ? ['sleep 10000'] : []),
			`npm out view ${label}@0.8.2 version ${W} inherit`
		]).flat();

	test('a publish that exits 0 with the version absent stops before git, resumably', async () => {
		const world = make_world();
		world.noop_publish = true;
		const failed = await publish(world, ['--wetrun', '--bump', 'patch']);
		strictEqual(failed.code, 1);
		const label = PUBLISH_PACKAGES[0]!.label;
		const at = failed.calls.indexOf(`npm run publish --access public @${ROOT}/${DIRS[0]}`);
		// the reads, then nothing: no other publish, no git, no sentinel removal
		deepStrictEqual(failed.calls.slice(at + 1), confirm_reads(label, 31));
		ok(
			failed.out.includes(
				`  ${label}@0.8.2 is not on the registry yet — reading again in 10s (1/31)\n` +
					`  ${label}@0.8.2 is not on the registry yet — reading again in 10s (2/31)`
			),
			failed.out
		);
		ok(failed.out.includes('reading again in 10s (30/31)'));
		ok(!failed.out.includes('(31/31)'));
		ok(!failed.out.includes(`  Published ${label}`), failed.out);
		ok(
			failed.err.endsWith(
				`  FAIL: npm publish ${label} exited 0, but the registry does not list ${label}@0.8.2 after 5 minutes\n` +
					'  Either the publish did not happen (npm ran it as a dry run) or the registry is behind.\n' +
					'  Nothing was committed, tagged, or pushed. A resume skips it once the registry lists it\n' +
					'  and publishes it while it is absent (a registry still behind then refuses that second\n' +
					'  publish, and the run stops here again).\n' +
					'  The bump is in place — fix the cause and run `npm run release -- --wetrun` to resume.'
			),
			failed.err
		);
		strictEqual(world.files.get('.publish-in-progress'), '0.8.2\n');
		strictEqual(world.commits, 0);
		strictEqual(world.local_tags.size, 0);
		strictEqual(world.pushes, 0);

		world.noop_publish = false;
		const resumed = await publish(world, ['--wetrun']);
		strictEqual(resumed.code, 0, resumed.err);
		deepStrictEqual(
			[...world.published].sort(),
			PUBLISH_PACKAGES.map((p) => `${p.label}@0.8.2`).sort()
		);
		strictEqual(world.commits, 1);
		strictEqual(world.pushes, 1);
		strictEqual(world.files.has('.publish-in-progress'), false);
	});

	test('a resume whose publish is still a no-op stops before git again', async () => {
		const world = make_world();
		world.noop_publish = true;
		strictEqual((await publish(world, ['--wetrun', '--bump', 'patch'])).code, 1);
		// the cause is one the preflight cannot see (an npmrc only the staged directory reads)
		const resumed = await publish(world, ['--wetrun']);
		strictEqual(resumed.code, 1);
		const label = PUBLISH_PACKAGES[0]!.label;
		ok(
			resumed.err.includes(`  FAIL: npm publish ${label} exited 0, but the registry does not list`),
			resumed.err
		);
		strictEqual(world.commits, 0);
		strictEqual(world.local_tags.size, 0);
		strictEqual(world.pushes, 0);
		strictEqual(world.files.get('.publish-in-progress'), '0.8.2\n');
	});

	test('lag that clears within the wait proceeds, printing each wait', async () => {
		const world = make_world();
		world.lag = 3;
		const run = await publish(world, ['--wetrun', '--bump', 'patch']);
		strictEqual(run.code, 0, run.err);
		for (const [i, { label }] of PUBLISH_PACKAGES.entries()) {
			const at = run.calls.indexOf(`npm run publish --access public @${ROOT}/${DIRS[i]}`);
			deepStrictEqual(run.calls.slice(at + 1, at + 8), confirm_reads(label, 4));
			ok(run.out.includes(`reading again in 10s (3/31)\n  Published ${label}@0.8.2`), run.out);
		}
		strictEqual(run.calls.filter((c) => c === 'sleep 10000').length, 9);
		strictEqual(world.pushes, 1);
		strictEqual(run.err, '');
	});

	test('a failed read while confirming is retried', async () => {
		const world = make_world();
		const label = PUBLISH_PACKAGES[0]!.label;
		const view = `npm view ${label}@0.8.2 version`;
		// the publish lands, and the confirmation's first read fails
		world.overrides.set(`npm publish --access public`, {
			answer: {},
			sticky: false,
			then: (w) => {
				w.published.add(`${label}@0.8.2`);
				w.overrides.set(view, {
					answer: { code: 1, stderr: 'npm error ECONNRESET' },
					sticky: false
				});
			}
		});
		const run = await publish(world, ['--wetrun', '--bump', 'patch']);
		strictEqual(run.code, 0, run.err);
		ok(
			run.out.includes(
				`  npm view ${label}@0.8.2 failed — reading again in 10s (1/31)\n  Published ${label}@0.8.2`
			),
			run.out
		);
		strictEqual(world.pushes, 1);
	});

	test('a read that fails to the end names its error', async () => {
		const world = make_world();
		const label = PUBLISH_PACKAGES[0]!.label;
		world.overrides.set(`npm publish --access public`, {
			answer: {},
			sticky: false,
			then: (w) =>
				w.overrides.set(`npm view ${label}@0.8.2 version`, {
					answer: { code: 1, stderr: 'npm error ECONNRESET' },
					sticky: true
				})
		});
		const run = await publish(world, ['--wetrun', '--bump', 'patch']);
		strictEqual(run.code, 1);
		ok(run.out.includes(`  npm view ${label}@0.8.2 failed — reading again in 10s (30/31)`));
		ok(
			run.err.includes(
				`after 5 minutes\nnpm view ${label}@0.8.2 failed:\nnpm error ECONNRESET\n  Either`
			),
			run.err
		);
		strictEqual(world.commits, 0);
		strictEqual(world.files.get('.publish-in-progress'), '0.8.2\n');
	});

	test('a version that goes live after the wait gave up is skipped on resume, not republished', async () => {
		const world = make_world();
		world.lag = 31;
		const failed = await publish(world, ['--wetrun', '--bump', 'patch']);
		strictEqual(failed.code, 1);
		const label = PUBLISH_PACKAGES[0]!.label;
		ok(
			failed.err.includes(`  FAIL: npm publish ${label} exited 0, but the registry does not list`)
		);
		// the registry was behind: the version landed, and lists it now
		deepStrictEqual([...world.published], [`${label}@0.8.2`]);
		strictEqual(world.lag_left.get(`${label}@0.8.2`), 0);
		world.lag = 0;
		const resumed = await publish(world, ['--wetrun']);
		strictEqual(resumed.code, 0, resumed.err);
		ok(resumed.out.includes(`  SKIP: ${label}@0.8.2 already published`), resumed.out);
		strictEqual(
			resumed.calls.includes(`npm run publish --access public @${ROOT}/${DIRS[0]}`),
			false
		);
		strictEqual(world.publishes, 3);
		strictEqual(world.pushes, 1);
		strictEqual(world.files.has('.publish-in-progress'), false);
	});
});

describe('the staged manifests', () => {
	test('a staged package with a scripts key is refused at Step 5, before any publish', async () => {
		for (const [scripts, named] of [
			[{ prepack: 'x', postpublish: 'y' }, ' (prepack, postpublish)'],
			[{}, ''],
			['x', '']
		] as const) {
			const world = make_world();
			world.staged_scripts = scripts;
			const run = await publish(world, ['--wetrun', '--bump', 'patch']);
			strictEqual(run.code, 1);
			ok(
				run.err.includes(
					`  FAIL: @fuzdev/tsv-format-wasm (${DIRS[0]}/package.json) has a \`scripts\` key${named}, ` +
						"which `npm publish` would run with the owner's credentials — a staged package carries none"
				),
				run.err
			);
			strictEqual(
				run.calls.some(
					(c) => c.startsWith('npm run publish') || c.startsWith('npm run run validate')
				),
				false
			);
			strictEqual(world.files.get('.publish-in-progress'), '0.8.2\n');
		}
		const world = make_world();
		world.staged_scripts = { prepare: 'x' };
		const dry = await publish(world, ['--bump', 'patch']);
		strictEqual(dry.code, 1);
		ok(dry.err.includes('has a `scripts` key (prepare)'), dry.err);
		ok(!dry.calls.some((c) => c.startsWith('npm out publish')));
	});
});
