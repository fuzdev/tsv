/**
 * The sibling checkouts over scratch trees with real symlinks: `open_sibling` follows a
 * checkout that is a symlink, relative, absolute, or a chain, and reads a dangling one, an
 * absent one, or a file as absent; `sibling_reads` never follows a symlink beneath the checkout
 * that is absolute or climbs out of it, even one landing back inside, and fails rather than
 * read it as missing. Then each tool that reads a sibling, driven through them, the same for a
 * linked checkout as for a directory: `audit:corpus`, `roundtrip:audit:prettier`,
 * `discovery:audit`, and `pins:audit:checkouts`.
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';
import { createServer } from 'node:net';
import { join, resolve } from 'node:path';
import { afterEach, describe, test, vi } from 'vitest';

import { fs, type FsDir, type FsReadDir } from '@fuzdev/gro/fs.ts';
import { spawn, type SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { is_spawn_error } from '@fuzdev/gro/spawn_error.ts';

import { run_corpus_audit } from '../../benches/js/corpus_audit.ts';
import { run_check_canonical_pins } from '../check_canonical_pins.ts';
import { run_discovery_audit } from '../discovery_audit.ts';
import { run_roundtrip_audit_prettier } from '../roundtrip_audit_prettier.ts';
import { link_out_message } from '../sibling_text.ts';
import {
	CHECKOUT_PATH,
	type CheckoutProgram,
	open_checkout_program,
	open_sibling,
	sibling_reads,
	to_cwd_refusal
} from './checkout.ts';

afterEach(() => {
	vi.restoreAllMocks();
});

/** What a run printed to `console.log`, `console.warn`, and `console.error`, in order. */
const capture_console = (): string[] => {
	const lines: string[] = [];
	const push = (...a: unknown[]) => void lines.push(a.join(' '));
	vi.spyOn(console, 'log').mockImplementation(push);
	vi.spyOn(console, 'warn').mockImplementation(push);
	vi.spyOn(console, 'error').mockImplementation(push);
	return lines;
};

/** The shapes a sibling checkout takes, each a `<shape>/<name>` beside a `<shape>/tsv`. */
const LINKED = ['relative', 'absolute', 'chain'] as const;
const ABSENT = ['dangling', 'absent', 'file'] as const;

/**
 * Plants `name` beside a `tsv` directory in `<shape>/` for each shape: a directory (`dir`);
 * a relative symlink leaving `<shape>/` for `elsewhere/<name>`, an absolute one, and a chain
 * (a relative link to an absolute one); a dangling absolute link, nothing, and a file. `fill`
 * writes the checkout's files beneath the directory it's given.
 */
const plant_siblings = async (
	root: FsDir,
	name: string,
	fill: (dir: string) => Promise<void>
): Promise<void> => {
	await fill(`elsewhere/${name}`);
	await fill(`dir/${name}`);
	for (const shape of ['dir', ...LINKED, ...ABSENT])
		await root.mkdir(`${shape}/tsv`, { recursive: true });
	await root.symlink(`relative/${name}`, `../elsewhere/${name}`);
	await root.symlink(`absolute/${name}`, join(root.path, `elsewhere/${name}`));
	await root.mkdir('hop', { recursive: true });
	await root.symlink(`hop/${name}`, join(root.path, `elsewhere/${name}`));
	await root.symlink(`chain/${name}`, `../hop/${name}`);
	await root.symlink(`dangling/${name}`, join(root.path, `no_such_${name}`));
	await root.write(`file/${name}`, 'not a checkout');
};

/** The absolute path a tool spelling `path` from `<shape>/tsv` names. */
const beside = (root: FsDir, shape: string, path: string): string =>
	resolve(root.path, shape, 'tsv', path);

describe('open_sibling', () => {
	test('follows a checkout that is a symlink and reads one that leads nowhere as absent', async () => {
		await using root = await fs.temp_dir('tsv_siblings_');
		await plant_siblings(root, 'prettier', async (dir) => {
			await root.mkdir(`${dir}/tests`, { recursive: true });
			await root.write(`${dir}/tests/a.js`, 'a;');
		});
		const real = await open_sibling(join(root.path, 'dir/prettier'));
		strictEqual(await real?.read_text('tests/a.js'), 'a;');
		for (const shape of LINKED) {
			const opened = await open_sibling(beside(root, shape, '../prettier'));
			strictEqual(opened?.path, join(root.path, 'elsewhere/prettier'), shape);
			strictEqual(await opened.read_text('tests/a.js'), 'a;', shape);
		}
		for (const shape of ABSENT) {
			strictEqual(await open_sibling(beside(root, shape, '../prettier')), null, shape);
		}
	});

	test('is read-only', async () => {
		await using root = await fs.temp_dir('tsv_siblings_');
		await root.mkdir('sibling');
		const opened = await open_sibling(join(root.path, 'sibling'));
		strictEqual('write' in opened!, false);
	});
});

describe('sibling_reads beneath the checkout', () => {
	test('follows a symlink that stays inside, and fails on one that leaves', async () => {
		await using root = await fs.temp_dir('tsv_siblings_');
		await root.mkdir('outside', { recursive: true });
		await root.write('outside/a.js', 'out;');
		await root.mkdir('sibling/real', { recursive: true });
		await root.write('sibling/real/a.js', 'in;');
		await root.symlink('sibling/inside', 'real');
		await root.symlink('sibling/out_relative', '../outside');
		await root.symlink('sibling/out_absolute', join(root.path, 'outside'));
		// absolute, or climbing out and back in by the checkout's name: never followed either
		await root.symlink('sibling/in_absolute', join(root.path, 'sibling/real'));
		await root.symlink('sibling/in_climbing', '../sibling/real');
		await root.symlink('sibling/dangling', 'no_such_entry');
		await root.write('sibling/a_file', 'x');
		const reads = sibling_reads(join(root.path, 'sibling'));

		ok(await reads.exists('inside/a.js'));
		strictEqual(await reads.read_text('inside/a.js'), 'in;');
		for (const path of [
			'out_relative/a.js',
			'out_absolute/a.js',
			'out_relative',
			'out_absolute',
			'in_absolute/a.js',
			'in_climbing/a.js'
		]) {
			// the words `doctor` prints for the same case, so the two never drift apart
			await rejects(reads.exists(path), {
				message: link_out_message(join(root.path, 'sibling'), path)
			});
		}
		await rejects(reads.read_text('out_relative/a.js'));
		await rejects(reads.read_text('out_absolute/a.js'));
		strictEqual(await reads.exists('dangling'), false);
		strictEqual(await reads.exists('a_file/x'), false);
		ok(await reads.exists('a_file'));
		ok(await reads.exists('.'));
	});

	test('answers a socket as present, as a presence probe of a path does', async () => {
		await using root = await fs.temp_dir('tsv_siblings_');
		await root.mkdir('sibling');
		const server = createServer();
		await new Promise<void>((done) => server.listen(join(root.path, 'sibling/sock'), done));
		try {
			ok(await sibling_reads(join(root.path, 'sibling')).exists('sock'));
		} finally {
			await new Promise((done) => server.close(done));
		}
	});

	test('reads an absent checkout as absent and fails its reads', async () => {
		const absent = sibling_reads('../tsv_no_such_sibling_checkout');
		strictEqual(await absent.open(), null);
		strictEqual(await absent.exists('tests'), false);
		await rejects(absent.read_text('x'));
		await rejects(async () => {
			for await (const _ of absent.list('.')) break;
		});
	});
});

/** Prettier's three round-trip suites, with a file in each but `typescript`. */
const fill_prettier = (root: FsDir) => async (dir: string) => {
	for (const suite of ['js', 'css']) {
		await root.mkdir(`${dir}/tests/format/${suite}`, { recursive: true });
		await root.write(`${dir}/tests/format/${suite}/a.${suite}`, 'a;');
	}
};

describe('a linked ../prettier reads as a directory, a dangling one as none', () => {
	test('audit:corpus', async () => {
		await using root = await fs.temp_dir('tsv_siblings_');
		await plant_siblings(root, 'prettier', fill_prettier(root));
		const audit = async (shape: string) => {
			const lines = capture_console();
			const calls: Array<ReadonlyArray<string>> = [];
			const code = await run_corpus_audit({
				seeds: async () => ({ dirs: ['../corpora'], live_files: [] }),
				prettier: sibling_reads(beside(root, shape, '../prettier')),
				cargo: { run: async (args) => (calls.push(args), { kind: 'exited', code: 0 }) }
			});
			vi.restoreAllMocks();
			return { code, lines, calls };
		};

		const present = await audit('dir');
		strictEqual(present.code, 0);
		deepStrictEqual(present.lines.slice(0, 1), [
			'  ⚠ prettier suite missing, skipped: ../prettier/tests/format/typescript'
		]);
		ok(
			present.calls.some(
				(args) =>
					args.includes('../prettier/tests/format/js') &&
					args.includes('../prettier/tests/format/css')
			)
		);
		for (const shape of LINKED) deepStrictEqual(await audit(shape), present, shape);

		const absent = await audit('absent');
		strictEqual(absent.code, 0);
		deepStrictEqual(absent.lines.slice(0, 3), [
			'  ⚠ prettier suite missing, skipped: ../prettier/tests/format/typescript',
			'  ⚠ prettier suite missing, skipped: ../prettier/tests/format/js',
			'  ⚠ prettier suite missing, skipped: ../prettier/tests/format/css'
		]);
		ok(!absent.calls.some((args) => args.some((arg) => arg.startsWith('../prettier/'))));
		for (const shape of ['dangling', 'file']) deepStrictEqual(await audit(shape), absent, shape);
	});

	test('roundtrip:audit:prettier', async () => {
		await using root = await fs.temp_dir('tsv_siblings_');
		await plant_siblings(root, 'prettier', fill_prettier(root));
		const checkout = { path: '/dev/tsv' } as unknown as FsReadDir;
		const audit = async (shape: string) => {
			const lines = capture_console();
			const calls: Array<ReadonlyArray<string>> = [];
			const cargo = {
				run: async (args: ReadonlyArray<string>) => (calls.push(args), { kind: 'exited', code: 3 })
			} as unknown as SpawnProgram;
			const code = await run_roundtrip_audit_prettier({
				checkout,
				prettier: sibling_reads(beside(root, shape, '../prettier')),
				cargo
			});
			vi.restoreAllMocks();
			return { code, lines, calls };
		};

		const present = await audit('dir');
		// the audit's own exit code
		strictEqual(present.code, 3);
		deepStrictEqual(present.lines, [
			'⚠ prettier suite missing from a present ../prettier checkout: ../prettier/tests/format/typescript'
		]);
		deepStrictEqual(
			present.calls.map((args) => args.slice(-2)),
			[['../prettier/tests/format/js', '../prettier/tests/format/css']]
		);
		for (const shape of LINKED) deepStrictEqual(await audit(shape), present, shape);

		// the message and exit code of a machine without the checkout
		const not_run = {
			code: 0,
			lines: [
				'⚠ roundtrip:audit:prettier NOT RUN — no ../prettier checkout (3 suites skipped).',
				'  This leg is the only one in `check` that reaches non-format-stable inputs.',
				'  Clone prettier beside this repo (see `npm run doctor`) to gate it locally.'
			],
			calls: []
		};
		for (const shape of ABSENT) deepStrictEqual(await audit(shape), not_run, shape);
	});

	test('roundtrip:audit:prettier fails on a suite reached through a symlink out of ../prettier, never passing it as missing', async () => {
		await using root = await fs.temp_dir('tsv_siblings_');
		await fill_prettier(root)('elsewhere');
		await root.mkdir('sibling/tests/format', { recursive: true });
		await root.symlink('sibling/tests/format/js', '../../../elsewhere/tests/format/js');
		await root.mkdir('sibling/tests/format/css', { recursive: true });
		const lines = capture_console();
		const calls: Array<ReadonlyArray<string>> = [];
		await rejects(
			run_roundtrip_audit_prettier({
				checkout: { path: '/dev/tsv' } as unknown as FsReadDir,
				prettier: sibling_reads(join(root.path, 'sibling')),
				cargo: {
					run: async (args: ReadonlyArray<string>) => (
						calls.push(args),
						{ kind: 'exited', code: 0 }
					)
				} as unknown as SpawnProgram
			}),
			/tests\/format\/js reaches through a symlink that is absolute or climbs out of /
		);
		deepStrictEqual(lines, []);
		deepStrictEqual(calls, []);
	});
});

/** A program whose stdout is `stdout`, ending with `code`, recording each call's cwd. */
const fake_started = (
	respond: (args: ReadonlyArray<string>) => { stdout: string; code: number }
) => {
	const calls: Array<{ args: ReadonlyArray<string>; cwd: string }> = [];
	const program = {
		start: async (args: ReadonlyArray<string>, options: { cwd: FsReadDir }) => {
			calls.push({ args, cwd: options.cwd.path });
			const { stdout, code } = respond(args);
			return {
				stdout: (async function* () {
					yield new TextEncoder().encode(stdout);
				})(),
				closed: Promise.resolve({ kind: 'exited', code })
			};
		}
	} as unknown as SpawnProgram;
	return { program, calls };
};

test('discovery:audit runs git in a linked ../corpora as in a directory, and not at all for a dangling one', async () => {
	await using root = await fs.temp_dir('tsv_siblings_');
	await plant_siblings(root, 'corpora', async (dir) => {
		await root.mkdir(`${dir}/collections`, { recursive: true });
	});
	const audit = async (shape: string) => {
		const lines = capture_console();
		const git = fake_started((args) =>
			args[0] === 'ls-files' ? { stdout: 'collections/a.ts\0', code: 0 } : { stdout: '', code: 0 }
		);
		const tsv = fake_started(() => ({ stdout: '../corpora/collections/a.ts\n', code: 0 }));
		const checkout = { path: join(root.path, shape, 'tsv') } as unknown as FsReadDir;
		const code = await run_discovery_audit({
			checkout,
			corpora: sibling_reads(beside(root, shape, '../corpora')),
			git: git.program,
			tsv: tsv.program
		});
		vi.restoreAllMocks();
		return { code, lines, git: git.calls, tsv: tsv.calls.map((call) => call.args) };
	};

	const present = await audit('dir');
	strictEqual(present.code, 0);
	deepStrictEqual(
		present.git.map((call) => call.cwd),
		[join(root.path, 'dir/corpora'), join(root.path, 'dir/corpora')]
	);
	for (const shape of LINKED) {
		const linked = await audit(shape);
		deepStrictEqual(
			linked.git.map((call) => call.cwd),
			[join(root.path, 'elsewhere/corpora'), join(root.path, 'elsewhere/corpora')]
		);
		deepStrictEqual({ ...linked, git: [] }, { ...present, git: [] }, shape);
	}

	for (const shape of ABSENT) {
		deepStrictEqual(
			await audit(shape),
			{
				code: 0,
				lines: [
					'⚠ discovery:audit NOT RUN — no ../corpora checkout.',
					"  This leg checks tsv's discovery against the real-code snapshot's committed file list.",
					'  Clone it beside this repo to gate it locally: git clone https://github.com/fuzdev/corpora ../corpora'
				],
				git: [],
				tsv: []
			},
			shape
		);
	}
});

test('discovery:audit fails on a ../corpora whose snapshot is a symlink out of it, never reading it as no checkout', async () => {
	await using root = await fs.temp_dir('tsv_siblings_');
	await root.mkdir('elsewhere/collections', { recursive: true });
	await root.mkdir('corpora', { recursive: true });
	await root.symlink('corpora/collections', '../elsewhere/collections');
	const lines = capture_console();
	const git = fake_started(() => ({ stdout: '', code: 0 }));
	const tsv = fake_started(() => ({ stdout: '', code: 0 }));
	await rejects(
		run_discovery_audit({
			checkout: { path: join(root.path, 'tsv') } as unknown as FsReadDir,
			corpora: sibling_reads(join(root.path, 'corpora')),
			git: git.program,
			tsv: tsv.program
		}),
		/collections reaches through a symlink that is absolute or climbs out of /
	);
	deepStrictEqual(lines, []);
	deepStrictEqual(git.calls, []);
	deepStrictEqual(tsv.calls, []);
});

test('pins:audit:checkouts reads a linked ../svelte as a directory, and a dangling one as absent', async () => {
	await using root = await fs.temp_dir('tsv_siblings_');
	await plant_siblings(root, 'svelte', async (dir) => {
		await root.mkdir(`${dir}/packages/svelte`, { recursive: true });
		await root.write(`${dir}/packages/svelte/package.json`, JSON.stringify({ version: '5.0.1' }));
	});
	const checkout = {
		read_text: async (path: string) => {
			if (path !== 'benches/js/package.json') throw new Error(`not_found: ${path}`);
			return JSON.stringify({ dependencies: { svelte: '5.0.0' } });
		}
	} as unknown as FsReadDir;
	const audit = async (shape: string) => {
		const lines = capture_console();
		const asked: Array<{ args: ReadonlyArray<string>; cwd: string }> = [];
		const git = {
			out: async (args: ReadonlyArray<string>, options: { cwd: FsReadDir }) => {
				asked.push({ args, cwd: options.cwd.path });
				return { kind: 'exited', code: 128, stdout: '', stderr: '', truncated: false };
			}
		} as unknown as SpawnProgram;
		const code = await run_check_canonical_pins(
			{
				checkout,
				siblings: { open: (path) => open_sibling(beside(root, shape, path)) },
				git
			},
			{ run_pins: false, run_checkouts: true }
		);
		vi.restoreAllMocks();
		return { code, lines, asked };
	};

	const present = await audit('dir');
	// the checkout's version was read: it disagrees with the pin
	strictEqual(present.code, 1);
	ok(
		present.lines.includes(
			'  · svelte: checkout ../svelte/packages/svelte/package.json is v5.0.1 but the oracle pin is v5.0.0 — ' +
				'align the checkout to the pinned tag, or bump the canonical pins deliberately (a fixture re-baseline)'
		),
		present.lines.join('\n')
	);
	deepStrictEqual(present.asked, [
		{ args: ['rev-parse', 'HEAD'], cwd: join(root.path, 'dir/svelte') }
	]);
	for (const shape of LINKED) {
		const linked = await audit(shape);
		deepStrictEqual(linked.asked, [
			{ args: ['rev-parse', 'HEAD'], cwd: join(root.path, 'elsewhere/svelte') }
		]);
		deepStrictEqual({ ...linked, asked: [] }, { ...present, asked: [] }, shape);
	}

	for (const shape of ABSENT) {
		const absent = await audit(shape);
		strictEqual(absent.code, 0, shape);
		deepStrictEqual(absent.asked, [], shape);
		strictEqual(absent.lines.length, 1, shape);
		ok(absent.lines[0]!.includes('svelte checkout absent — alignment not checked'), shape);
		ok(absent.lines[0]!.includes('../svelte absent — drift not checked'), shape);
	}

	// a checkout whose version file is reached through a symlink out of it: git still runs in
	// the checkout, the version file is never read, and the run fails rather than reading the
	// checkout as absent
	await root.mkdir('inner_out/tsv', { recursive: true });
	await root.mkdir('inner_out/svelte', { recursive: true });
	await root.symlink('inner_out/svelte/packages', '../../elsewhere/svelte/packages');
	const inner_out = await audit('inner_out');
	strictEqual(inner_out.code, 1);
	deepStrictEqual(inner_out.asked, [
		{ args: ['rev-parse', 'HEAD'], cwd: join(root.path, 'inner_out/svelte') }
	]);
	ok(
		inner_out.lines.includes(
			'  · svelte: ../svelte/packages/svelte/package.json reaches through a symlink that is absolute or climbs out of its checkout, which is never followed — make it relative within the checkout, or link the checkout itself instead'
		),
		inner_out.lines.join('\n')
	);
	ok(!inner_out.lines.some((line) => line.includes('svelte checkout absent')));
});

describe('open_checkout_program', () => {
	/** Plants an executable script at `path` beneath `root` that prints how it was started. */
	const plant_program = async (root: FsDir, path: string): Promise<void> => {
		await root.mkdir(path.slice(0, path.lastIndexOf('/')), { recursive: true });
		await root.write(path, '#!/bin/sh\necho "ran $0 $*"\n');
		await root.set_mode(path, 0o755);
	};

	test('makes and runs a program beneath a checkout that is not the working directory', async () => {
		await using root = await fs.temp_dir('tsv_checkout_program_');
		ok(!join(fs.project.path, '/').startsWith(join(root.path, '/')));
		await plant_program(root, 'checkout/target/corpus/tsv');
		await plant_program(root, 'checkout/node_modules/typescript/bin/tsc');
		await root.mkdir('checkout/node_modules/.bin');
		await root.symlink('checkout/node_modules/.bin/tsc', '../typescript/bin/tsc');
		const checkout_program = await open_checkout_program(
			await fs.open_dir(join(root.path, 'checkout'))
		);
		const tsv = await checkout_program('target/corpus/tsv').out(['a']);
		strictEqual(tsv.stdout, `ran ${join(root.path, 'checkout/target/corpus/tsv')} a\n`);
		// a relative symlink that stays inside the checkout is followed
		const tsc = await checkout_program('node_modules/.bin/tsc').out(['-p', 'x']);
		strictEqual(tsc.stdout, `ran ${join(root.path, 'checkout/node_modules/.bin/tsc')} -p x\n`);
	});

	/** Whether making `path` throws `SpawnError` `not_granted`. */
	const is_refused = (checkout_program: CheckoutProgram, path: string): boolean => {
		try {
			checkout_program(path);
			return false;
		} catch (error) {
			ok(is_spawn_error(error, 'not_granted'), String(error));
			return true;
		}
	};

	test('follows a target and a node_modules that are absolute symlinks out of the checkout', async () => {
		await using root = await fs.temp_dir('tsv_checkout_program_');
		await plant_program(root, 'elsewhere/corpus/tsv_debug');
		await plant_program(root, 'shared/typescript/bin/tsc');
		await root.mkdir('shared/.bin');
		await root.symlink('shared/.bin/tsc', '../typescript/bin/tsc');
		await root.mkdir('checkout');
		await root.symlink('checkout/target', join(root.path, 'elsewhere'));
		await root.symlink('checkout/node_modules', join(root.path, 'shared'));
		const checkout_program = await open_checkout_program(
			await fs.open_dir(join(root.path, 'checkout'))
		);
		const run = await checkout_program('target/corpus/tsv_debug').out([]);
		strictEqual(run.kind, 'exited');
		ok(run.stdout.startsWith('ran ') && run.stdout.endsWith('/corpus/tsv_debug \n'), run.stdout);
		const tsc = await checkout_program('node_modules/.bin/tsc').out(['-p', 'x']);
		strictEqual(tsc.stdout, `ran ${join(root.path, 'shared/.bin/tsc')} -p x\n`);
		// any other top-level directory linked absolutely is refused, as is a name that only
		// begins like a linkable one
		await plant_program(root, 'elsewhere/bin/x');
		await root.symlink('checkout/bin', join(root.path, 'elsewhere/bin'));
		await root.symlink('checkout/target_old', join(root.path, 'elsewhere'));
		ok(is_refused(checkout_program, 'bin/x'));
		ok(is_refused(checkout_program, 'target_old/corpus/tsv_debug'));
	});

	test('refuses a binary that is itself an absolute symlink, beneath a linked target too', async () => {
		await using root = await fs.temp_dir('tsv_checkout_program_');
		await plant_program(root, 'store/tsv');
		await root.mkdir('checkout/target/corpus', { recursive: true });
		await root.symlink('checkout/target/corpus/tsv', join(root.path, 'store/tsv'));
		// a relative one that stays inside target runs
		await plant_program(root, 'checkout/target/store/tsv_debug');
		await root.symlink('checkout/target/corpus/tsv_debug', '../store/tsv_debug');
		const checkout_program = await open_checkout_program(
			await fs.open_dir(join(root.path, 'checkout'))
		);
		ok(is_refused(checkout_program, 'target/corpus/tsv'));
		strictEqual((await checkout_program('target/corpus/tsv_debug').out([])).kind, 'exited');
	});

	test('a missing binary is not_found, naming where it looked; a missing target is never made', async () => {
		await using root = await fs.temp_dir('tsv_checkout_program_');
		await root.mkdir('checkout');
		const checkout = await fs.open_dir(join(root.path, 'checkout'));
		const assert_not_found = async (message: string): Promise<void> => {
			const checkout_program = await open_checkout_program(checkout);
			try {
				checkout_program('target/corpus/tsv');
				ok(false, `${message}: made`);
			} catch (error) {
				ok(is_spawn_error(error, 'not_found'), String(error));
				strictEqual(error.message, message);
			}
		};
		// no target: looked for beneath the checkout, which gains none
		await assert_not_found(`not_found: no program target/corpus/tsv beneath ${checkout.path}`);
		strictEqual(await root.exists('checkout/target'), false);
		// a target that is a file reads as none
		await root.write('checkout/target', '');
		await assert_not_found(`not_found: no program target/corpus/tsv beneath ${checkout.path}`);
		// a target without the binary: looked for beneath target
		await root.remove('checkout/target');
		await root.mkdir('checkout/target/corpus', { recursive: true });
		await assert_not_found(
			`not_found: no program corpus/tsv beneath ${join(checkout.path, 'target')}`
		);
	});
});

describe('to_cwd_refusal', () => {
	test('says nothing from the checkout, and names the directory to run from anywhere else', () => {
		strictEqual(to_cwd_refusal('/dev/tsv', '/dev/tsv'), null);
		for (const cwd of ['/', '/dev/tsv/scripts', '/dev/other_tsv']) {
			strictEqual(
				to_cwd_refusal('/dev/tsv', cwd),
				`✗ run this from the checkout's root, /dev/tsv, not ${cwd}: ` +
					'the corpus and the paths it is given resolve against the working directory'
			);
		}
	});

	test('exit_unless_checkout_cwd ends a process started elsewhere with exit 1, printing it first', async () => {
		await using elsewhere = await fs.temp_dir('tsv_checkout_cwd_');
		const script =
			`import { exit_unless_checkout_cwd, open_checkout } from ${JSON.stringify(import.meta.resolve('./checkout.ts'))};\n` +
			"exit_unless_checkout_cwd(await open_checkout());\nconsole.log('ran on');\n";
		const node = spawn.program('node');
		const run = async (cwd: FsReadDir) => {
			const out = await node.out(['--input-type=module', '-e', script], { cwd });
			return [out.kind === 'exited' ? out.code : out.signal, out.stdout, out.stderr];
		};
		deepStrictEqual(await run(elsewhere), [
			1,
			'',
			`${to_cwd_refusal(CHECKOUT_PATH, elsewhere.path)}\n`
		]);
		deepStrictEqual(await run(await fs.open_dir(CHECKOUT_PATH)), [0, 'ran on\n', '']);
	});
});
