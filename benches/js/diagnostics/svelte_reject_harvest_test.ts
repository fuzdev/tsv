/**
 * `run_svelte_reject_harvest` and `run_prettier_jsx_harvest` over fakes for `git`, the
 * checkout's files, the corpus, and the oracle: the list and the stamp they write (bytes
 * spelled out), the stamp's freshness rule, the oracle and corpus refusals under and without
 * `--if-present`, and the pin, which writes nothing when it moves. Package-free, so it gates
 * in `npm run test:node`.
 */

import { createHash } from 'node:crypto';
import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';
import { afterEach, beforeEach, describe, test, vi } from 'vitest';

import { PRETTIER_JSX_PIN, SVELTE_REJECTS_PIN } from '../lib/gate_counts.ts';
import { fake_files, fake_id, fake_stamp_git, tab_json_strings } from '../lib/harvest_fakes.ts';
import { HARVEST_STAMPS } from '../lib/harvest_stamp_table.ts';
import type { SourceFile, TsvImplementation } from '../lib/types.ts';
import { load_all_versions } from '../lib/versions.ts';
import { type PrettierJsxHarvestDeps, run_prettier_jsx_harvest } from './prettier_jsx_harvest.ts';
import {
	type HarvestCorpus,
	run_svelte_reject_harvest,
	type SvelteRejectHarvestDeps
} from './svelte_reject_harvest.ts';

const REJECTS_CACHE = 'benches/js/.cache/svelte_parse_rejects.json';
const JSX_CACHE = 'benches/js/.cache/prettier_jsx_files.json';
const ENTRIES = ['../svelte/packages/svelte/tests', '../prettier/tests/format/js'];
/** The filter modules a conformance stamp fingerprints, with stand-in sources. */
const FILTER_SOURCES = {
	'benches/js/lib/prettier_fixtures.ts': '// prettier fixtures',
	'benches/js/lib/corpus.ts': '// corpus'
};
const FILTERS = (() => {
	const hash = createHash('sha1');
	for (const [path, text] of Object.entries(FILTER_SOURCES)) {
		hash.update(path).update('\0').update(text).update('\0');
	}
	return hash.digest('hex').slice(0, 16);
})();
const IDS = {
	'../svelte': fake_id('svelte'),
	'../prettier': fake_id('prettier'),
	'../prettier-plugin-svelte': fake_id('plugin')
};
const versions = await load_all_versions();

let err: string[];
beforeEach(() => {
	err = [];
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});
afterEach(() => {
	vi.restoreAllMocks();
});

const source = (path: string, content: string): SourceFile =>
	({ path, content, language: 'svelte', bytes: content.length }) as SourceFile;

/** A corpus that answers `files`, or `null` (an absent entry under `--if-present`), recording
 * each load's options. */
const fake_corpus = (files: SourceFile[] | null | Error) => {
	const loads: Array<Parameters<HarvestCorpus['load']>> = [];
	const corpus: HarvestCorpus = {
		view_paths: async () => ENTRIES,
		load: async (...args) => {
			loads.push(args);
			if (files instanceof Error) throw files;
			return files;
		}
	};
	return { corpus, loads };
};

describe('svelte rejects', () => {
	const STAMP_PATH = HARVEST_STAMPS['svelte-rejects'].path;
	const stamp_text = (svelte = IDS['../svelte']): string =>
		`{\n\t"harvest": "svelte-rejects",\n\t"svelte_commit": "${svelte}",` +
		`\n\t"prettier_commit": "${IDS['../prettier']}",` +
		`\n\t"prettier_plugin_svelte_commit": "${IDS['../prettier-plugin-svelte']}",` +
		`\n\t"svelte_oracle": "${versions.canonical.svelte}",\n\t"rejects_pin": ${SVELTE_REJECTS_PIN},` +
		`\n\t"conformance_entries": "${ENTRIES.join(' ')}",\n\t"filters": "${FILTERS}"\n}\n`;

	/** `rejects` files holding `bad` (named in reverse), then two good ones. */
	const corpus_files = (rejects = SVELTE_REJECTS_PIN): SourceFile[] => [
		...Array.from({ length: rejects }, (_, i) => source(`s/${rejects - i}.svelte`, 'bad')),
		source('s/good1.svelte', 'ok'),
		source('s/good2.svelte', 'ok')
	];

	const oracle: TsvImplementation = {
		parse: (content: string) => {
			if (content.includes('bad')) throw new Error('no');
			return {};
		}
	} as unknown as TsvImplementation;

	const fake_deps = (
		options: {
			files?: Record<string, string>;
			corpus?: SourceFile[] | null | Error;
			oracle_fails?: boolean;
			ids?: Record<string, string>;
		} = {}
	) => {
		const files = fake_files({ ...FILTER_SOURCES, ...options.files });
		const { corpus, loads } = fake_corpus(
			options.corpus === undefined ? corpus_files() : options.corpus
		);
		let inits = 0;
		const deps: SvelteRejectHarvestDeps = {
			git: fake_stamp_git(options.ids ?? IDS),
			files,
			corpus,
			init_oracle: async (canonical) => {
				inits++;
				deepStrictEqual(canonical, versions.canonical);
				if (options.oracle_fails) throw new Error('svelte missing');
				return oracle;
			}
		};
		return { deps, files, loads, inits: () => inits };
	};

	test('a stale stamp grades the unfiltered Svelte corpus and writes the list and the stamp', async () => {
		const run = fake_deps({ files: { [STAMP_PATH]: stamp_text(fake_id('old')) } });
		strictEqual(await run_svelte_reject_harvest(run.deps, ['--if-present']), 0);
		strictEqual(run.loads.length, 1);
		const [view, language, options] = run.loads[0]!;
		deepStrictEqual(
			[view, language, options.if_present, options.apply_exclusion_caches],
			['conformance', 'svelte', true, false]
		);
		deepStrictEqual(run.files.log, [
			'mkdir benches/js/.cache',
			`write ${REJECTS_CACHE}`,
			`write ${STAMP_PATH}`
		]);
		const expected = corpus_files()
			.slice(0, SVELTE_REJECTS_PIN)
			.map((f) => f.path)
			.sort();
		strictEqual(run.files.files.get(REJECTS_CACHE), tab_json_strings(expected) + '\n');
		strictEqual(run.files.files.get(STAMP_PATH), stamp_text());
		strictEqual(
			err.at(-1),
			`svelte_reject_harvest: ${SVELTE_REJECTS_PIN}/${SVELTE_REJECTS_PIN + 2} Svelte files rejected by ` +
				`svelte/compiler → ${REJECTS_CACHE}`
		);
	});

	test('a fresh stamp skips with its line, loading nothing; --force grades past it', async () => {
		const fresh = { [STAMP_PATH]: stamp_text(), [REJECTS_CACHE]: '[]' };
		const skipped = fake_deps({ files: fresh });
		strictEqual(await run_svelte_reject_harvest(skipped.deps, []), 0);
		strictEqual(skipped.inits(), 0);
		strictEqual(skipped.loads.length, 0);
		deepStrictEqual(skipped.files.log, []);
		deepStrictEqual(err, [
			`svelte-rejects harvest up to date (../svelte at ${IDS['../svelte'].slice(0, 9)}, ` +
				`oracle svelte@${versions.canonical.svelte}, pin ${SVELTE_REJECTS_PIN}) — skipping; --force to re-harvest.`
		]);
		const forced = fake_deps({ files: fresh });
		strictEqual(await run_svelte_reject_harvest(forced.deps, ['--force']), 0);
		ok(forced.files.log.includes(`write ${STAMP_PATH}`));
	});

	test('an oracle that fails to load warn-skips under --if-present and throws otherwise', async () => {
		const skipped = fake_deps({ oracle_fails: true });
		strictEqual(await run_svelte_reject_harvest(skipped.deps, ['--if-present']), 0);
		deepStrictEqual(err, [
			'  ⚠ svelte_reject_harvest: could not init svelte/compiler (svelte missing) — skipping ' +
				'(run `deno task bench:install`)'
		]);
		const strict = fake_deps({ oracle_fails: true });
		await rejects(run_svelte_reject_harvest(strict.deps, []), {
			message: 'svelte_reject_harvest: could not init svelte/compiler (svelte missing)'
		});
		strictEqual(strict.loads.length, 0);
		deepStrictEqual(strict.files.log, []);
	});

	test("the loader's refusal skips (null) or propagates, writing nothing", async () => {
		const skipped = fake_deps({ corpus: null });
		strictEqual(await run_svelte_reject_harvest(skipped.deps, ['--if-present']), 0);
		deepStrictEqual(skipped.files.log, []);
		const strict = fake_deps({ corpus: new Error('Missing corpus entry') });
		await rejects(run_svelte_reject_harvest(strict.deps, []), { message: 'Missing corpus entry' });
		deepStrictEqual(strict.files.log, []);
	});

	test('a reject count off the pin fails, writing nothing', async () => {
		const run = fake_deps({ corpus: corpus_files(SVELTE_REJECTS_PIN + 1) });
		strictEqual(await run_svelte_reject_harvest(run.deps, ['--if-present']), 1);
		deepStrictEqual(run.files.log, []);
		ok(
			err
				.at(-1)!
				.startsWith(`FAIL: pinned count mismatch — ${SVELTE_REJECTS_PIN + 1} rejects ≠ pinned`)
		);
	});

	test('no ../svelte commit grades past any stamp and writes the list unstamped', async () => {
		const ids = { ...IDS } as Record<string, string>;
		delete ids['../svelte'];
		const run = fake_deps({ ids, files: { [STAMP_PATH]: stamp_text(), [REJECTS_CACHE]: '[]' } });
		strictEqual(await run_svelte_reject_harvest(run.deps, []), 0);
		deepStrictEqual(run.files.log, ['mkdir benches/js/.cache', `write ${REJECTS_CACHE}`]);
	});
});

describe('prettier JSX', () => {
	const STAMP_PATH = HARVEST_STAMPS['prettier-jsx'].path;
	const stamp_text = (prettier = IDS['../prettier']): string =>
		`{\n\t"harvest": "prettier-jsx",\n\t"prettier_commit": "${prettier}",` +
		`\n\t"prettier_oracle": "${versions.canonical.prettier}",\n\t"jsx_pin": ${PRETTIER_JSX_PIN},` +
		`\n\t"conformance_entries": "${ENTRIES.join(' ')}",\n\t"filters": "${FILTERS}"\n}\n`;

	/** `jsx` files babel reads as JSX (named in reverse), one it refuses, one plain. */
	const corpus_files = (jsx = PRETTIER_JSX_PIN): SourceFile[] => [
		...Array.from({ length: jsx }, (_, i) => source(`p/${jsx - i}.js`, '<div/>')),
		source('p/refused.js', 'REFUSE'),
		source('p/plain.js', 'let a;')
	];

	/** A prettier whose babel reads `<` as JSX and refuses `REFUSE`. */
	const prettier = {
		__debug: {
			parse: async (text: string, options: { parser: string }) => {
				strictEqual(options.parser, 'babel');
				if (text.includes('REFUSE')) throw new Error('babel refuses');
				return {
					ast: {
						type: 'File',
						program: {
							type: 'Program',
							body: text.includes('<')
								? [{ type: 'ExpressionStatement', expression: { type: 'JSXElement' } }]
								: []
						}
					}
				};
			}
		}
	};

	const fake_deps = (
		options: {
			files?: Record<string, string>;
			corpus?: SourceFile[] | null;
			import_fails?: boolean;
		} = {}
	) => {
		const files = fake_files({ ...FILTER_SOURCES, ...options.files });
		const { corpus, loads } = fake_corpus(
			options.corpus === undefined ? corpus_files() : options.corpus
		);
		let imports = 0;
		const deps: PrettierJsxHarvestDeps = {
			git: fake_stamp_git(IDS),
			files,
			corpus,
			import_prettier: async () => {
				imports++;
				if (options.import_fails) throw new Error('Cannot find package');
				return prettier;
			}
		};
		return { deps, files, loads, imports: () => imports };
	};

	test('a stale stamp reads the JS suite alone and writes the list and the stamp', async () => {
		const run = fake_deps();
		strictEqual(await run_prettier_jsx_harvest(run.deps, ['--if-present']), 0);
		const [view, language, options] = run.loads[0]!;
		deepStrictEqual(
			[view, language, options.apply_exclusion_caches],
			['conformance', 'typescript', false]
		);
		strictEqual(options.only?.('../prettier/tests/format/js'), true);
		strictEqual(options.only?.('../prettier/tests/format/typescript'), false);
		const expected = corpus_files()
			.slice(0, PRETTIER_JSX_PIN)
			.map((f) => f.path)
			.sort();
		strictEqual(run.files.files.get(JSX_CACHE), tab_json_strings(expected) + '\n');
		strictEqual(run.files.files.get(STAMP_PATH), stamp_text());
		strictEqual(
			err.at(-1),
			`prettier_jsx_harvest: ${PRETTIER_JSX_PIN}/${PRETTIER_JSX_PIN + 2} Prettier JS fixtures read as JSX by ` +
				`prettier's babel parser → ${JSX_CACHE}`
		);
	});

	test('a fresh stamp skips with its line, importing nothing', async () => {
		const run = fake_deps({ files: { [STAMP_PATH]: stamp_text(), [JSX_CACHE]: '[]' } });
		strictEqual(await run_prettier_jsx_harvest(run.deps, []), 0);
		strictEqual(run.imports(), 0);
		deepStrictEqual(err, [
			`prettier-jsx harvest up to date (../prettier at ${IDS['../prettier'].slice(0, 9)}, ` +
				`oracle prettier@${versions.canonical.prettier}, pin ${PRETTIER_JSX_PIN}) — skipping; --force to re-harvest.`
		]);
	});

	test('a failed import warn-skips under --if-present and throws otherwise', async () => {
		const skipped = fake_deps({ import_fails: true });
		strictEqual(await run_prettier_jsx_harvest(skipped.deps, ['--if-present']), 0);
		deepStrictEqual(err, [
			'  ⚠ prettier_jsx_harvest: could not import prettier (Cannot find package) — skipping ' +
				'(run `deno task bench:install`)'
		]);
		await rejects(run_prettier_jsx_harvest(fake_deps({ import_fails: true }).deps, []), {
			message: 'prettier_jsx_harvest: could not import prettier (Cannot find package)'
		});
	});

	test('an absent entry skips; a count off the pin fails; neither writes', async () => {
		const skipped = fake_deps({ corpus: null });
		strictEqual(await run_prettier_jsx_harvest(skipped.deps, ['--if-present']), 0);
		deepStrictEqual(skipped.files.log, []);
		const moved = fake_deps({ corpus: corpus_files(PRETTIER_JSX_PIN - 1) });
		strictEqual(await run_prettier_jsx_harvest(moved.deps, ['--if-present']), 1);
		deepStrictEqual(moved.files.log, []);
	});
});
