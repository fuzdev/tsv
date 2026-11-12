/**
 * `run_harvest_ts_repo` over fakes for `git`, the checkout's files, the `../typescript` tree,
 * and the `typescript` oracle: which bucket each case lands in, the bytes of the two lists and
 * the stamp, the stamp's freshness rule, and every refusal, each of which writes nothing.
 * Package-free, so it gates in `npm run test:node`.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { afterEach, beforeEach, test, vi } from 'vitest';

import {
	type HarvestTsRepoDeps,
	run_harvest_ts_repo,
	type TsRepoReads
} from './harvest_ts_repo.ts';
import { TS_REPO_CORPUS_PIN, TS_REPO_REJECTS_PIN } from './lib/gate_counts.ts';
import { fake_files, fake_id, fake_stamp_git, tab_json_strings } from './lib/harvest_fakes.ts';
import { HARVEST_STAMPS } from './lib/harvest_stamp_table.ts';
import type { TypeScriptModule } from './lib/tsc.ts';

const STAMP_PATH = HARVEST_STAMPS.ts_repo.path;
const FILES_PATH = 'benches/js/.cache/ts_repo_files.json';
const REJECTS_PATH = 'benches/js/.cache/ts_repo_rejects.json';
const COMMIT = fake_id('typescript');
const TSC_VERSION = '6.0.3';

let err: string[];
beforeEach(() => {
	err = [];
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});
afterEach(() => {
	vi.restoreAllMocks();
});

/** A tsc whose parser reads a verdict from the source: `REJECT` → a grammar diagnostic,
 * `BINARY` → TS1490, `GONE` → no diagnostics field at all; anything else parses clean. */
const fake_tsc: TypeScriptModule = {
	version: TSC_VERSION,
	ScriptTarget: { ESNext: 99 },
	ScriptKind: { TS: 3 },
	createSourceFile: (_name, source) => ({
		parseDiagnostics: source.includes('GONE')
			? undefined
			: source.includes('BINARY')
				? [{ code: 1490, messageText: 'binary' }]
				: source.includes('REJECT')
					? [{ code: 1005, messageText: "';' expected." }]
					: []
	}),
	flattenDiagnosticMessageText: (text) => String(text)
};

type Tree = Map<string, string | null>;

/** The `../typescript` tree: a path → its content (`null`, a file that can't be read). */
const typescript_tree = (options: { valid?: number; extra?: Record<string, string | null> }) => {
	const tree: Tree = new Map();
	const valid = options.valid ?? TS_REPO_CORPUS_PIN;
	// `typed1.ts` below is the last valid file
	for (let i = 0; i < valid - 1; i++) {
		const dir = i % 2 === 0 ? 'tests/cases/conformance/a' : 'tests/cases/compiler';
		// one capitalized name, so a code-unit sort and the locale's disagree
		tree.set(`${dir}/${i === 1 ? 'Valid' : 'valid'}${i}.ts`, 'let x = 1;');
	}
	for (let i = 0; i < TS_REPO_REJECTS_PIN; i++) {
		// every other reject has a TS1xxx baseline (one through a per-variant name)
		tree.set(`tests/cases/conformance/b/${i === 3 ? 'Reject' : 'reject'}${i}.ts`, 'let = REJECT');
		if (i % 2 === 0) {
			const variant = i === 0 ? '(target=es5)' : '';
			tree.set(`tests/baselines/reference/reject${i}${variant}.errors.txt`, 'error TS1005: x');
		}
	}
	// accepted by the parser, dropped by a TS1xxx baseline
	tree.set('tests/cases/compiler/checker1.ts', 'class A {}');
	tree.set('tests/baselines/reference/checker1.errors.txt', 'a.ts(1,1): error TS1206: x');
	// a baseline with only checker errors keeps its file
	tree.set('tests/cases/compiler/typed1.ts', 'let y = 1;');
	tree.set('tests/baselines/reference/typed1.errors.txt', 'error TS2322: x');
	tree.set('tests/baselines/reference/notes.txt', 'error TS1005: ignored, not an errors file');
	tree.set('tests/cases/compiler/binary1.ts', 'BINARY');
	tree.set(
		'tests/cases/compiler/multi1.ts',
		'// @filename: a.ts\nlet a;\n// @filename: b.ts\nlet b;'
	);
	tree.set('tests/cases/compiler/view.tsx', 'let v = <div/>;');
	tree.set('tests/cases/compiler/types.d.ts', 'declare const d: number;');
	tree.set('tests/cases/compiler/broken.ts', null);
	tree.set('tests/cases/compiler/node_modules/dep.ts', 'REJECT');
	for (const [path, content] of Object.entries(options.extra ?? {})) tree.set(path, content);
	return tree;
};

/** `tree` as the checkout's read handle, recording each read. */
const tree_reads = (tree: Tree, present = true): TsRepoReads & { reads: string[] } => {
	const reads: string[] = [];
	const is_dir = (path: string) => [...tree.keys()].some((k) => k.startsWith(`${path}/`));
	return {
		reads,
		exists: async (path) => present && (tree.has(path) || is_dir(path)),
		read_text: async (path) => {
			reads.push(path);
			const content = tree.get(path);
			if (content === undefined || content === null) throw new Error(`cannot read ${path}`);
			return content;
		},
		list: async function* (path) {
			if (!is_dir(path)) throw new Error(`no directory ${path}`);
			const seen = new Set<string>();
			// newest first, so the walk's own sort decides the order
			for (const key of [...tree.keys()].reverse()) {
				if (!key.startsWith(`${path}/`)) continue;
				const [name, ...rest] = key.slice(path.length + 1).split('/');
				if (seen.has(name!)) continue;
				seen.add(name!);
				yield { name: name!, kind: rest.length > 0 ? 'directory' : 'file' };
			}
		}
	};
};

const stamp_text = (commit: string, version = TSC_VERSION): string =>
	`{\n\t"harvest": "ts_repo",\n\t"source_commit": "${commit}",\n\t"tsc_version": "${version}",` +
	`\n\t"corpus_pin": ${TS_REPO_CORPUS_PIN},\n\t"rejects_pin": ${TS_REPO_REJECTS_PIN}\n}\n`;

const fake_deps = (
	options: {
		tree?: Tree;
		present?: boolean;
		files?: Record<string, string>;
		commit?: string | null;
		tsc?: TypeScriptModule;
	} = {}
) => {
	const files = fake_files(options.files);
	const typescript = tree_reads(options.tree ?? typescript_tree({}), options.present);
	let tsc_loads = 0;
	const deps: HarvestTsRepoDeps = {
		git: fake_stamp_git(
			options.commit === null ? {} : { '../typescript': options.commit ?? COMMIT }
		),
		files,
		typescript,
		load_typescript: async () => {
			tsc_loads++;
			return options.tsc ?? fake_tsc;
		}
	};
	return { deps, files, typescript, tsc_loads: () => tsc_loads };
};

test('each case lands in its bucket, and the lists and the stamp are written byte for byte', async () => {
	const { deps, files } = fake_deps({ files: { [STAMP_PATH]: stamp_text(COMMIT, '5.9.0') } });
	strictEqual(await run_harvest_ts_repo(deps, []), 0);
	deepStrictEqual(files.log, [
		'mkdir benches/js/.cache',
		`write ${FILES_PATH}`,
		`write ${REJECTS_PATH}`,
		`write ${STAMP_PATH}`
	]);
	const valid = JSON.parse(files.files.get(FILES_PATH)!) as string[];
	const rejects = JSON.parse(files.files.get(REJECTS_PATH)!) as string[];
	strictEqual(files.files.get(FILES_PATH), tab_json_strings(valid) + '\n');
	strictEqual(files.files.get(REJECTS_PATH), tab_json_strings(rejects) + '\n');
	strictEqual(valid.length, TS_REPO_CORPUS_PIN);
	strictEqual(rejects.length, TS_REPO_REJECTS_PIN);
	// paths are spelled from the checkout, sorted by locale; a checker-only baseline keeps its file
	ok(valid.includes('../typescript/tests/cases/compiler/typed1.ts'));
	ok(!valid.includes('../typescript/tests/cases/compiler/checker1.ts'));
	deepStrictEqual(
		valid,
		valid.toSorted((a, b) => a.localeCompare(b))
	);
	strictEqual(rejects[0], '../typescript/tests/cases/conformance/b/reject0.ts');
	deepStrictEqual(
		rejects,
		rejects.toSorted((a, b) => a.localeCompare(b))
	);
	strictEqual(files.files.get(STAMP_PATH), stamp_text(COMMIT));
	strictEqual(
		err.at(-1),
		`ts-repo harvest (tsc ${TSC_VERSION}): ${TS_REPO_CORPUS_PIN} valid → ${FILES_PATH}; ` +
			`${TS_REPO_REJECTS_PIN} tsc-parser rejects → ${REJECTS_PATH}. ` +
			`Of ${TS_REPO_CORPUS_PIN + TS_REPO_REJECTS_PIN + 2} single-file .ts scanned: 1 dropped by a TS1xxx baseline ` +
			`(tsc's parser accepted, its grammar checks did not), ${TS_REPO_REJECTS_PIN / 2 - 0.5} rejected ` +
			'with no TS1xxx baseline (the other direction), 1 non-UTF-8. ' +
			'Skipped before grading: 1 @filename multi-file, 1 .tsx, 1 .d.ts, 1 unreadable.'
	);
});

test('a fresh stamp skips with its line; a moved tsc version or --force re-grades', async () => {
	const fresh = { [STAMP_PATH]: stamp_text(COMMIT), [FILES_PATH]: '[]', [REJECTS_PATH]: '[]' };
	const skipped = fake_deps({ files: fresh });
	strictEqual(await run_harvest_ts_repo(skipped.deps, []), 0);
	deepStrictEqual(skipped.files.log, ['mkdir benches/js/.cache']);
	deepStrictEqual(skipped.typescript.reads, []);
	deepStrictEqual(err, [
		`ts-repo harvest up to date (../typescript at ${COMMIT.slice(0, 9)}, tsc ${TSC_VERSION}, ` +
			`pins ${TS_REPO_CORPUS_PIN}/${TS_REPO_REJECTS_PIN}) — skipping; --force to re-grade.`
	]);

	const bumped = fake_deps({ files: fresh, tsc: { ...fake_tsc, version: '6.1.0' } });
	strictEqual(await run_harvest_ts_repo(bumped.deps, []), 0);
	ok(bumped.files.log.includes(`write ${FILES_PATH}`));

	const forced = fake_deps({ files: fresh });
	strictEqual(await run_harvest_ts_repo(forced.deps, ['--force']), 0);
	ok(forced.files.log.includes(`write ${STAMP_PATH}`));
});

test('a missing checkout warn-skips under --if-present and fails otherwise, touching nothing', async () => {
	for (const [argv, code, line] of [
		[
			['--if-present'],
			0,
			'typescript checkout not found at ../typescript — skipping harvest (--if-present)'
		],
		[[], 1, 'typescript checkout not found: ../typescript']
	] as const) {
		err = [];
		const run = fake_deps({ present: false });
		strictEqual(await run_harvest_ts_repo(run.deps, argv), code);
		deepStrictEqual(err, [line]);
		deepStrictEqual(run.files.log, []);
		strictEqual(run.tsc_loads(), 0);
	}
});

test('missing baselines fail, writing nothing', async () => {
	const tree = typescript_tree({});
	for (const key of [...tree.keys()]) if (key.startsWith('tests/baselines/')) tree.delete(key);
	const { deps, files } = fake_deps({ tree });
	strictEqual(await run_harvest_ts_repo(deps, []), 1);
	deepStrictEqual(files.log, ['mkdir benches/js/.cache']);
	ok(
		err
			.at(-1)!
			.startsWith(
				'FAIL: cannot read ../typescript/tests/baselines/reference (no directory tests/baselines/reference) — '
			)
	);
});

test('a tsc with no parse-diagnostics field refuses to write', async () => {
	const tree = typescript_tree({ extra: { 'tests/cases/compiler/a0.ts': 'GONE' } });
	const { deps, files } = fake_deps({ tree });
	strictEqual(await run_harvest_ts_repo(deps, []), 1);
	deepStrictEqual(files.log, ['mkdir benches/js/.cache']);
	ok(err.at(-1)!.startsWith('FAIL: tsc SourceFile has no `parseDiagnostics`'));
});

test('a count off either pin writes nothing and names both', async () => {
	const tree = typescript_tree({ valid: TS_REPO_CORPUS_PIN + 1 });
	tree.delete('tests/cases/conformance/b/reject1.ts');
	const { deps, files } = fake_deps({ tree });
	strictEqual(await run_harvest_ts_repo(deps, []), 1);
	deepStrictEqual(files.log, ['mkdir benches/js/.cache']);
	strictEqual(
		err.at(-1),
		`FAIL: pinned count mismatch — valid ${TS_REPO_CORPUS_PIN + 1} ≠ pinned ${TS_REPO_CORPUS_PIN}, ` +
			`rejects ${TS_REPO_REJECTS_PIN - 1} ≠ pinned ${TS_REPO_REJECTS_PIN}; caches not written. ` +
			'If the move is deliberate (checkout pull, tsc bump, grading change), re-pin in lib/gate_counts.ts.'
	);
});

test('a checkout with no commit writes the lists but no stamp', async () => {
	const { deps, files } = fake_deps({ commit: null });
	strictEqual(await run_harvest_ts_repo(deps, []), 0);
	deepStrictEqual(files.log, [
		'mkdir benches/js/.cache',
		`write ${FILES_PATH}`,
		`write ${REJECTS_PATH}`
	]);
});
