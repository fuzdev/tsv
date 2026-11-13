/**
 * `run_css_over_acceptance` over fakes for git, the checkout's files, the corpus, and the
 * engines: the profile's rows and `--json` report, the reject pin and the stamp it gates, the
 * pin leg's partial-corpus and fresh-stamp skips, and the oracle row that must read zero.
 * Package-free, so it gates in `npm run test:node`.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { afterEach, beforeEach, test, vi } from 'vitest';

import { CSS_REJECTS_PIN, GATE_CHECKOUT_IDS } from '../lib/gate_counts.ts';
import { measured_id } from '../lib/harvest_fakes.ts';
import { HARVEST_STAMPS } from '../lib/harvest_stamp_table.ts';
import { TsvRejection } from '../lib/divergence/panic_errors.ts';
import type { SourceFile, TsvImplementation } from '../lib/types.ts';
import { raise_wasm_trap } from '../lib/wasm_trap_fakes.ts';
import {
	type CssOverAcceptanceDeps,
	type CssOverAcceptanceImplementations,
	run_css_over_acceptance
} from './css_over_acceptance.ts';

const STAMP_PATH = HARVEST_STAMPS['css-rejects'].path;

let err: string[];
beforeEach(() => {
	err = [];
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});
afterEach(() => {
	vi.restoreAllMocks();
});

/** `rejects` files the oracle refuses (every eighth also holds `lenient`), then good ones. */
const corpus = (rejects = CSS_REJECTS_PIN): SourceFile[] =>
	[
		...Array.from({ length: rejects }, (_, i) => `bad${i % 8 === 0 ? ' lenient' : ''}`),
		'good',
		'good'
	].map(
		(content, i) =>
			({ path: `c/${i}.css`, content, language: 'css', bytes: content.length }) as SourceFile
	);
const LENIENT = corpus().filter((f) => f.content.includes('lenient')).length;

/** An engine that refuses a source holding `bad`, unless it also holds a word in `allows`. */
const fake_impl = (disposed: string[], name: string, ...allows: string[]): TsvImplementation =>
	({
		supports_parse_language: (language: string) => language === 'css',
		parse: (source: string) => {
			if (source.includes('bad') && !allows.some((w) => source.includes(w))) throw new Error('no');
			return {};
		},
		dispose: () => void disposed.push(name)
	}) as unknown as TsvImplementation;

interface FakeOptions {
	files?: SourceFile[];
	missing?: string[];
	git_fails?: boolean;
	/** The commit `git` reports per checkout, over a stand-in spelled from its name. */
	commits?: Record<string, string>;
	impls?: (disposed: string[]) => CssOverAcceptanceImplementations;
	oracle_fails?: boolean;
}

const fake_world = () => ({
	stored: new Map<string, string>(),
	writes: [] as string[],
	loads: 0,
	inits: 0,
	oracle_inits: 0,
	disposed: [] as string[],
	out: ''
});

const fake_deps = (world: ReturnType<typeof fake_world>, options: FakeOptions = {}) => {
	const deps: CssOverAcceptanceDeps = {
		git: {
			out: async (args) =>
				options.git_fails
					? { kind: 'exited', code: 128, stdout: '' }
					: {
							kind: 'exited',
							code: 0,
							stdout: `${options.commits?.[args[1]] ?? args[1].slice(3).padEnd(40, '0')}\n`
						}
		},
		files: {
			exists: async (path) => world.stored.has(path),
			read_text: async (path) => {
				if (path === STAMP_PATH) {
					const text = world.stored.get(path);
					if (text === undefined) throw new Error('not_found');
					return text;
				}
				return `// ${path}`;
			},
			write: async (path, data) => {
				world.writes.push(path);
				world.stored.set(path, data);
			},
			mkdir: async () => {
				throw new Error('mkdir is never needed');
			}
		},
		corpus: {
			missing_entries: async () => ({ missing: options.missing ?? [], optional_missing: [] }),
			view_paths: async () => ['../prettier/tests/format/css', 'benches/js/.cache/wpt_css'],
			load: async () => {
				world.loads++;
				return options.files ?? corpus();
			}
		},
		init_oracle: async () => {
			world.oracle_inits++;
			if (options.oracle_fails) throw new Error('svelte missing');
			return fake_impl(world.disposed, 'oracle');
		},
		init_implementations: async () => {
			world.inits++;
			return (
				options.impls?.(world.disposed) ?? {
					canonical: fake_impl(world.disposed, 'canonical'),
					native: fake_impl(world.disposed, 'native', 'lenient'),
					wasm: undefined,
					postcss: fake_impl(world.disposed, 'postcss', 'bad')
				}
			);
		},
		stdout: { write: async (data) => void (world.out += data) }
	};
	return deps;
};

test('the profile grades each available row over the oracle rejects and stamps the pin', async () => {
	const world = fake_world();
	strictEqual(await run_css_over_acceptance(fake_deps(world), ['--json']), 0);
	strictEqual(world.oracle_inits, 0);
	deepStrictEqual(JSON.parse(world.out), {
		corpus: 'conformance/css',
		scanned: CSS_REJECTS_PIN + 2,
		files: CSS_REJECTS_PIN,
		oracle_agrees: true,
		rows: [
			{ name: 'svelte/compiler', accepted: 0, total: CSS_REJECTS_PIN, rate: 0 },
			{ name: 'tsv', accepted: LENIENT, total: CSS_REJECTS_PIN, rate: LENIENT / CSS_REJECTS_PIN },
			{ name: 'postcss', accepted: CSS_REJECTS_PIN, total: CSS_REJECTS_PIN, rate: 1 }
		]
	});
	deepStrictEqual(world.writes, [STAMP_PATH]);
	const stamp = JSON.parse(world.stored.get(STAMP_PATH)!);
	strictEqual(stamp.harvest, 'css-rejects');
	strictEqual(stamp.svelte_commit, `svelte`.padEnd(40, '0'));
	strictEqual(stamp.rejects_pin, CSS_REJECTS_PIN);
	deepStrictEqual(world.disposed.toSorted(), ['canonical', 'native', 'postcss']);
});

test('the human report ranks the rows, with samples under --verbose', async () => {
	const world = fake_world();
	strictEqual(await run_css_over_acceptance(fake_deps(world), ['-v']), 0);
	strictEqual(world.out, '');
	const rows = err.filter((l) => /^ {2}\S.*\d+\/\d+/.test(l));
	deepStrictEqual(
		rows.map((l) => l.trim().split(/\s+/)[0]),
		['svelte/compiler', 'tsv', 'postcss']
	);
	ok(err.includes('      c/0.css'));
	ok(err.includes('  ⚠ skipping tsv-wasm — not available on this machine'));
});

test('a reject count off the pin fails and stamps nothing', async () => {
	const world = fake_world();
	const deps = fake_deps(world, { files: corpus(CSS_REJECTS_PIN - 1) });
	strictEqual(await run_css_over_acceptance(deps, ['--json']), 1);
	deepStrictEqual(world.writes, []);
	strictEqual(world.out, '');
	ok(err.some((l) => l.startsWith(`FAIL: the oracle rejects ${CSS_REJECTS_PIN - 1} of`)));
});

test('a reject count off the pin with every checkout at its measured commit names the re-pin', async () => {
	const world = fake_world();
	const commits = Object.fromEntries(
		['../svelte', '../prettier', '../wpt'].map((c) => [c, measured_id(c)])
	);
	const deps = fake_deps(world, { files: corpus(CSS_REJECTS_PIN - 1), commits });
	strictEqual(await run_css_over_acceptance(deps, ['--pin-only']), 1);
	strictEqual(
		err.at(-1),
		`FAIL: the oracle rejects ${CSS_REJECTS_PIN - 1} of ${CSS_REJECTS_PIN + 1} CSS files ≠ pinned ` +
			`${CSS_REJECTS_PIN}. Either a pinned input moved (../prettier's or ../svelte's checkout, the ` +
			"wpt-css harvest) or svelte's parseCss changed what it accepts — re-pin CSS_REJECTS_PIN in " +
			'lib/gate_counts.ts deliberately, after checking which.'
	);
});

test('a reject count off the pin names each checkout off its measured commit instead', async () => {
	const world = fake_world();
	const commits = { '../svelte': 'a'.repeat(40), '../prettier': measured_id('../prettier') };
	const deps = fake_deps(world, { files: corpus(CSS_REJECTS_PIN - 1), commits });
	strictEqual(await run_css_over_acceptance(deps, ['--pin-only']), 1);
	strictEqual(
		err.at(-1),
		`FAIL: the oracle rejects ${CSS_REJECTS_PIN - 1} of ${CSS_REJECTS_PIN + 1} CSS files ≠ pinned ` +
			`${CSS_REJECTS_PIN}. ../svelte is at aaaaaaaaa, not ${GATE_CHECKOUT_IDS['../svelte']!.hash}; ` +
			`../wpt is at ${'wpt'.padEnd(9, '0')}, not ${GATE_CHECKOUT_IDS['../wpt']!.hash}, the commits ` +
			'the pin was measured at (GATE_CHECKOUT_IDS in lib/gate_counts.ts): move the checkouts there ' +
			'and re-run before any re-pin. `npm run pins:audit:checkouts` says which way each moved; a ' +
			'deliberate pull re-pins and re-records the id together.'
	);
	// ../prettier's suites are read too, so it alone off its commit is named
	const prettier_moved = Object.fromEntries(
		['../svelte', '../wpt'].map((c) => [c, measured_id(c)])
	);
	strictEqual(
		await run_css_over_acceptance(
			fake_deps(fake_world(), { files: corpus(CSS_REJECTS_PIN - 1), commits: prettier_moved }),
			['--pin-only']
		),
		1
	);
	strictEqual(
		err.at(-1),
		`FAIL: the oracle rejects ${CSS_REJECTS_PIN - 1} of ${CSS_REJECTS_PIN + 1} CSS files ≠ pinned ` +
			`${CSS_REJECTS_PIN}. ../prettier is at ${'prettier'.padEnd(9, '0')}, not ` +
			`${GATE_CHECKOUT_IDS['../prettier']!.hash}, the commit the pin was measured at ` +
			'(GATE_CHECKOUT_IDS in lib/gate_counts.ts): move the checkout there and re-run before any ' +
			're-pin. `npm run pins:audit:checkouts` says which way it moved; a deliberate pull re-pins ' +
			'and re-records the id together.'
	);
});

test('an oracle row that accepts its own rejects fails the run, after stamping the pin', async () => {
	const world = fake_world();
	let calls = 0;
	const deps = fake_deps(world, {
		impls: (disposed) => ({
			// refuses every `bad` source while the reject list is built, then accepts one
			canonical: {
				...fake_impl(disposed, 'canonical'),
				parse: (source: string) => {
					if (source.includes('bad') && calls++ !== CSS_REJECTS_PIN) throw new Error('no');
					return {};
				}
			} as TsvImplementation
		})
	});
	strictEqual(await run_css_over_acceptance(deps, ['--json']), 1);
	strictEqual(JSON.parse(world.out).oracle_agrees, false);
	ok(err.some((l) => l.startsWith('FAIL: the svelte/compiler row accepted 1 of its own rejects')));
});

test('the pin leg skips a partial corpus under --if-present and fails it otherwise', async () => {
	const world = fake_world();
	const deps = fake_deps(world, { missing: ['benches/js/.cache/wpt_css'] });
	strictEqual(await run_css_over_acceptance(deps, ['--pin-only', '--if-present']), 0);
	strictEqual(await run_css_over_acceptance(deps, ['--pin-only']), 1);
	strictEqual(world.loads, 0);
	strictEqual(world.oracle_inits, 0);
	ok(err.some((l) => l.startsWith('  ⚠ css-rejects pin: the conformance CSS corpus is partial')));
	ok(err.some((l) => l.startsWith('FAIL: css-rejects pin: the conformance CSS corpus is partial')));
});

test('the pin leg loads the oracle alone, and skips once the stamp is fresh unless --force', async () => {
	const world = fake_world();
	const deps = fake_deps(world);
	strictEqual(await run_css_over_acceptance(deps, ['--pin-only']), 0);
	strictEqual(world.inits, 0);
	strictEqual(world.oracle_inits, 1);
	deepStrictEqual(world.disposed, ['oracle']);
	ok(err.some((l) => l.endsWith('— matches CSS_REJECTS_PIN (stamped).')));

	strictEqual(await run_css_over_acceptance(deps, ['--pin-only']), 0);
	strictEqual(world.loads, 1);
	ok(err.some((l) => l.startsWith('css-rejects pin up to date')));

	strictEqual(await run_css_over_acceptance(deps, ['--pin-only', '--force']), 0);
	strictEqual(world.loads, 2);
	deepStrictEqual(world.writes, [STAMP_PATH, STAMP_PATH]);
});

test('with no commit to record the pin leg never reads its stamp fresh, nor writes one', async () => {
	const world = fake_world();
	world.stored.set(STAMP_PATH, '{}');
	const deps = fake_deps(world, { git_fails: true });
	strictEqual(await run_css_over_acceptance(deps, ['--pin-only']), 0);
	strictEqual(await run_css_over_acceptance(deps, ['--pin-only']), 0);
	strictEqual(world.loads, 2);
	deepStrictEqual(world.writes, []);
});

test('an oracle that cannot load skips under --if-present and fails otherwise', async () => {
	const world = fake_world();
	const deps = fake_deps(world, { oracle_fails: true });
	strictEqual(await run_css_over_acceptance(deps, ['--pin-only', '--if-present']), 0);
	strictEqual(await run_css_over_acceptance(deps, ['--pin-only']), 1);
	strictEqual(world.loads, 0);
	ok(
		err.includes(
			'FAIL: css-rejects pin: could not init svelte/compiler (svelte missing) — run `npm run bench:install`'
		)
	);
});

/**
 * `fake_impl`, but failing on a source holding `crash` as `failure` says: the panic the
 * `corpus` addon throws, a WASM build's trap, or a returned rejection reading `panic: `.
 */
const crashing_impl = (
	disposed: string[],
	name: string,
	failure: 'panic' | 'trap' | 'panic_text',
	...allows: string[]
): TsvImplementation => {
	const impl = fake_impl(disposed, name, ...allows);
	return {
		...impl,
		parse: (source: string, language: 'css') => {
			if (source.includes('crash')) {
				if (failure === 'trap') raise_wasm_trap();
				throw failure === 'panic'
					? new Error('panic: boom\n  at a frame')
					: new TsvRejection('panic: boom\n  at a frame');
			}
			return impl.parse(source, language);
		}
	} as TsvImplementation;
};

/** The pinned corpus with its second and third rejects marked to crash a crashing engine. */
const crash_corpus = (): SourceFile[] =>
	corpus().map((f, i) => (i === 1 || i === 2 ? { ...f, content: `${f.content} crash` } : f));

test('a crash in a tsv row is neither an accept nor a rejection: listed with its engine, it fails the run', async () => {
	const world = fake_world();
	const deps = fake_deps(world, {
		files: crash_corpus(),
		impls: (disposed) => ({
			canonical: fake_impl(disposed, 'canonical'),
			native: crashing_impl(disposed, 'native', 'panic', 'lenient'),
			wasm: crashing_impl(disposed, 'wasm', 'trap', 'lenient')
		})
	});
	strictEqual(await run_css_over_acceptance(deps, ['--json']), 1);
	const graded = CSS_REJECTS_PIN - 2;
	// the report is still out, whole, with the crashed files in neither count
	deepStrictEqual(JSON.parse(world.out).rows, [
		{ name: 'svelte/compiler', accepted: 0, total: CSS_REJECTS_PIN, rate: 0 },
		{ name: 'tsv', accepted: LENIENT, total: graded, rate: LENIENT / graded },
		{ name: 'tsv-wasm', accepted: LENIENT, total: graded, rate: LENIENT / graded }
	]);
	deepStrictEqual(err, [
		'FAIL: 4 panic(s) — tsv crashed on real code (a native panic the corpus profile caught, which a shipped artifact aborts the host on, or a WASM trap):',
		'  tsv: c/1.css: panic: boom',
		'  tsv: c/2.css: panic: boom',
		'  tsv-wasm: c/1.css: RuntimeError: unreachable',
		'  tsv-wasm: c/2.css: RuntimeError: unreachable'
	]);
	deepStrictEqual(world.disposed.toSorted(), ['canonical', 'native', 'wasm']);
});

test("another engine's crash text stays a rejection: only tsv's rows are gated", async () => {
	const world = fake_world();
	const deps = fake_deps(world, {
		files: crash_corpus(),
		impls: (disposed) => ({
			canonical: fake_impl(disposed, 'canonical'),
			postcss: crashing_impl(disposed, 'postcss', 'panic', 'bad')
		})
	});
	strictEqual(await run_css_over_acceptance(deps, ['--json']), 0);
	deepStrictEqual(JSON.parse(world.out).rows[1], {
		name: 'postcss',
		accepted: CSS_REJECTS_PIN - 2,
		total: CSS_REJECTS_PIN,
		rate: (CSS_REJECTS_PIN - 2) / CSS_REJECTS_PIN
	});
	ok(!err.some((l) => l.includes('panic')));
});

test('a tsv rejection whose text reads `panic: ` is a rejection: the engine returned it', async () => {
	const world = fake_world();
	const deps = fake_deps(world, {
		files: crash_corpus(),
		impls: (disposed) => ({
			canonical: fake_impl(disposed, 'canonical'),
			native: crashing_impl(disposed, 'native', 'panic_text', 'lenient')
		})
	});
	strictEqual(await run_css_over_acceptance(deps, ['--json']), 0);
	deepStrictEqual(JSON.parse(world.out).rows[1], {
		name: 'tsv',
		accepted: LENIENT,
		total: CSS_REJECTS_PIN,
		rate: LENIENT / CSS_REJECTS_PIN
	});
	deepStrictEqual(err, []);
});
