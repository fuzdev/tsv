/**
 * `run_wpt_css_harvest` over fakes for `git`, the checkout's files, the source tree, and the
 * output directory: the walk's order and the first-path-wins rule it decides, every block
 * skip, the files and the stats written, the pin that clears the output, the stamp's
 * freshness rule, and the refusals, `--out`'s among them. Package-free, so it gates in
 * `npm run test:node`.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { afterEach, beforeEach, describe, test, vi } from 'vitest';

import { GATE_CHECKOUT_IDS, WPT_CSS_HARVEST_PIN } from '../lib/gate_counts.ts';
import {
	fake_files,
	fake_id,
	fake_stamp_git,
	measured_id,
	SKEW_TAIL
} from '../lib/harvest_fakes.ts';
import { HARVEST_STAMPS } from '../lib/harvest_stamp_table.ts';
import {
	run_wpt_css_harvest,
	to_wpt_out,
	walk_breadth_first,
	WPT_OUT_DEFAULT,
	type WptCssHarvestDeps,
	type WptSource
} from './wpt_css_harvest.ts';

const STAMP_PATH = HARVEST_STAMPS['wpt-css'].path;
const OUT = 'benches/js/.cache/wpt_css';
const COMMIT = fake_id('wpt');

let err: string[];
beforeEach(() => {
	err = [];
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});
afterEach(() => {
	vi.restoreAllMocks();
});

/** A source tree from path → content, listed in reverse insertion order (never sorted), with
 * `links` as symlink entries. */
const fake_source = (
	tree: Record<string, string>,
	links: string[] = [],
	path = '/dev/wpt/css'
): WptSource => {
	const paths = Object.keys(tree);
	return {
		path,
		read_text: async (path) => {
			const text = tree[path];
			if (text === undefined) throw new Error(`not found: ${path}`);
			return text;
		},
		list: async function* (dir) {
			const prefix = dir === '' ? '' : `${dir}/`;
			const seen = new Set<string>();
			for (const path of [...paths, ...links].reverse()) {
				if (!path.startsWith(prefix)) continue;
				const [name, ...rest] = path.slice(prefix.length).split('/');
				if (seen.has(name!)) continue;
				seen.add(name!);
				const kind = links.includes(`${prefix}${name}`)
					? 'symlink'
					: rest.length > 0
						? 'directory'
						: 'file';
				yield { name: name!, kind };
			}
		}
	};
};

const SMALL_TREE: Record<string, string> = {
	'z/deep/a.html': '<style>.dup{}</style>',
	'b/x.html': '<style>.dup{}</style><style>.b{}</style>',
	'a.html':
		'<style>.a{}</style><script>let s = "<style>.in_script{}</style>";</script>' +
		'<style type="text/plain">.typed{}</style><style TYPE=\'TEXT/CSS\'>.css{}</style>' +
		'<style> \n </style>',
	'B.html': '<style>.upper{}</style>',
	'tmpl.sub.html': '<style>{{x}}</style>',
	'notes.txt': '<style>.txt{}</style>'
};

const fake_deps = (
	options: {
		tree?: Record<string, string>;
		links?: string[];
		missing?: boolean;
		/** Why `open_out` refuses the output, when it does. */
		out_refused?: string;
		/** The real paths the source and the output resolve to. */
		source_path?: string;
		out_path?: (path: string) => string;
		files?: Record<string, string>;
		commit?: string | null;
	} = {}
) => {
	const files = fake_files(options.files);
	const out_log: string[] = [];
	const out_files = new Map<string, string>();
	let stdout = '';
	const opened: string[] = [];
	const deps: WptCssHarvestDeps = {
		git: fake_stamp_git(options.commit === null ? {} : { '../wpt/css': options.commit ?? COMMIT }),
		files,
		open_source: async (path) => {
			opened.push(`source ${path}`);
			return options.missing
				? null
				: fake_source(options.tree ?? SMALL_TREE, options.links, options.source_path);
		},
		open_out: async (path) => {
			opened.push(`out ${path}`);
			if (options.out_refused !== undefined) return { refused: options.out_refused };
			return {
				path: options.out_path?.(path) ?? `/dev/tsv/${path}`,
				remove: async () => {
					out_log.push('remove');
					out_files.clear();
				},
				mkdir: async (beneath) => void out_log.push(`mkdir ${beneath}`),
				write: async (beneath, data) => {
					out_log.push(`write ${beneath}`);
					out_files.set(beneath, data);
				}
			};
		},
		stdout: { write: async (data) => void (stdout += data) }
	};
	return { deps, files, out_log, out_files, opened, stdout: () => stdout };
};

test('the walk is breadth first, each directory in code-point order, never through a symlink', async () => {
	const seen: string[] = [];
	for await (const entry of walk_breadth_first(
		fake_source(
			{
				'z/q/1.html': '',
				'a/2.html': '',
				'_x.html': '',
				'B/3.html': '',
				'é.html': '',
				'a/link/behind.html': ''
			},
			['a/link']
		)
	)) {
		seen.push(`${entry.kind[0]} ${entry.path}`);
	}
	deepStrictEqual(seen, [
		'd B',
		'f _x.html',
		'd a',
		'd z',
		'f é.html',
		'f B/3.html',
		'f a/2.html',
		's a/link',
		'd z/q',
		'f z/q/1.html'
	]);
});

test('the walk orders names by code point, not by UTF-16 unit', async () => {
	const seen: string[] = [];
	for await (const entry of walk_breadth_first(
		fake_source({ '\u{1F600}.html': '', '\u{FF5E}.html': '', '\u{E000}.html': '' })
	)) {
		seen.push(entry.path);
	}
	deepStrictEqual(seen, ['\u{E000}.html', '\u{FF5E}.html', '\u{1F600}.html']);
});

test('a custom source into the default output is never stamped, however fresh', async () => {
	const run = fake_deps({
		files: {
			[STAMP_PATH]: `{\n\t"harvest": "wpt-css",\n\t"source_commit": "${COMMIT}",\n\t"pin": ${WPT_CSS_HARVEST_PIN}\n}\n`,
			[`${OUT}/a.css`]: ''
		}
	});
	run.deps.git = fake_stamp_git({ '../elsewhere': COMMIT });
	strictEqual(await run_wpt_css_harvest(run.deps, ['../elsewhere']), 0);
	strictEqual(run.out_log[0], 'remove');
	deepStrictEqual(run.files.log, []);
});

test('a custom source harvests every kept block, first path in walk order wins, unstamped', async () => {
	const run = fake_deps();
	strictEqual(
		await run_wpt_css_harvest(run.deps, ['../elsewhere', '--out', 'benches/js/.cache/x']),
		0
	);
	deepStrictEqual(run.opened, ['source ../elsewhere', 'out benches/js/.cache/x']);
	deepStrictEqual(run.out_log, [
		'remove',
		'mkdir ',
		'mkdir ',
		'write B__0.css',
		'mkdir ',
		'write a__0.css',
		'mkdir ',
		'write a__3.css',
		'mkdir b',
		'write b/x__0.css',
		'mkdir b',
		'write b/x__1.css'
	]);
	strictEqual(run.out_files.get('a__3.css'), '.css{}');
	strictEqual(run.out_files.get('b/x__0.css'), '.dup{}');
	deepStrictEqual(run.files.log, []);
	strictEqual(
		run.stdout(),
		'{\n\t"html_files_scanned": 4,\n\t"sub_html_skipped": 1,\n\t"files_with_blocks": 3,' +
			'\n\t"blocks_written": 5,\n\t"blocks_skipped_in_script": 1,\n\t"blocks_skipped_type": 1,' +
			'\n\t"blocks_skipped_empty": 1,\n\t"blocks_skipped_duplicate": 1,\n\t"bytes_written": 28\n}\n'
	);
	strictEqual(err.at(-1), 'done: 5 blocks from 3 files → benches/js/.cache/x');
});

/** A default-source tree holding exactly `blocks` distinct blocks. */
const pinned_tree = (blocks = WPT_CSS_HARVEST_PIN): Record<string, string> =>
	Object.fromEntries(
		Array.from({ length: blocks }, (_, i) => [`d${i % 7}/t${i}.html`, `<style>.c${i}{}</style>`])
	);

test('the default run writes the stamp once the pin holds', async () => {
	const run = fake_deps({ tree: pinned_tree(), files: { [STAMP_PATH]: 'stale' } });
	strictEqual(await run_wpt_css_harvest(run.deps, ['--if-present']), 0);
	strictEqual(run.out_files.size, WPT_CSS_HARVEST_PIN);
	deepStrictEqual(run.files.log, [`write ${STAMP_PATH}`]);
	strictEqual(
		run.files.files.get(STAMP_PATH),
		`{\n\t"harvest": "wpt-css",\n\t"source_commit": "${COMMIT}",\n\t"pin": ${WPT_CSS_HARVEST_PIN}\n}\n`
	);
	ok(err.includes(`scanned 5000 files, 5000 blocks…`));
});

test('a block count off the pin removes the output and fails, unstamped', async () => {
	const run = fake_deps({
		tree: pinned_tree(WPT_CSS_HARVEST_PIN - 1),
		commit: measured_id('../wpt')
	});
	strictEqual(await run_wpt_css_harvest(run.deps, ['--if-present']), 1);
	strictEqual(run.out_log.at(-1), 'remove');
	strictEqual(run.out_files.size, 0);
	deepStrictEqual(run.files.log, []);
	strictEqual(run.stdout(), '');
	strictEqual(
		err.at(-1),
		`FAIL: pinned count mismatch — harvested ${WPT_CSS_HARVEST_PIN - 1} blocks ≠ pinned ${WPT_CSS_HARVEST_PIN}. ` +
			`Removing ${OUT}; if the move is deliberate (wpt pull), re-pin in lib/gate_counts.ts.`
	);
});

test('a block count off the pin in a checkout off its measured commit says so, not the re-pin', async () => {
	const run = fake_deps({ tree: pinned_tree(WPT_CSS_HARVEST_PIN - 1) });
	strictEqual(await run_wpt_css_harvest(run.deps, ['--if-present']), 1);
	strictEqual(run.out_log.at(-1), 'remove');
	strictEqual(
		err.at(-1),
		`FAIL: pinned count mismatch — harvested ${WPT_CSS_HARVEST_PIN - 1} blocks ≠ pinned ${WPT_CSS_HARVEST_PIN}. ` +
			`Removing ${OUT}; ../wpt is at ${COMMIT.slice(0, 9)}, not ` +
			`${GATE_CHECKOUT_IDS['../wpt']!.hash}${SKEW_TAIL}`
	);
});

test('a fresh stamp skips with its line; --force and a custom --out harvest past it', async () => {
	const fresh = {
		[STAMP_PATH]: `{\n\t"harvest": "wpt-css",\n\t"source_commit": "${COMMIT}",\n\t"pin": ${WPT_CSS_HARVEST_PIN}\n}\n`,
		[`${OUT}/a.css`]: '',
		'benches/js/.cache/y/a.css': ''
	};
	const skipped = fake_deps({ files: fresh });
	strictEqual(await run_wpt_css_harvest(skipped.deps, []), 0);
	deepStrictEqual(skipped.out_log, []);
	deepStrictEqual(err, [
		`wpt harvest up to date (../wpt at ${COMMIT.slice(0, 9)}, pin ${WPT_CSS_HARVEST_PIN}) — skipping; --force to re-harvest.`
	]);
	const forced = fake_deps({ files: fresh, tree: pinned_tree() });
	strictEqual(await run_wpt_css_harvest(forced.deps, ['--force']), 0);
	strictEqual(forced.out_files.size, WPT_CSS_HARVEST_PIN);
	const custom = fake_deps({ files: fresh });
	strictEqual(await run_wpt_css_harvest(custom.deps, ['--out', 'benches/js/.cache/y']), 1);
	strictEqual(custom.out_log[0], 'remove');
});

test('an absent output directory reads stale under a matching stamp', async () => {
	const files = {
		[STAMP_PATH]: `{\n\t"harvest": "wpt-css",\n\t"source_commit": "${COMMIT}",\n\t"pin": ${WPT_CSS_HARVEST_PIN}\n}\n`
	};
	const run = fake_deps({ files, tree: pinned_tree() });
	strictEqual(await run_wpt_css_harvest(run.deps, []), 0);
	strictEqual(run.out_log[0], 'remove');
});

test('a missing source warn-skips under --if-present and fails otherwise, touching nothing', async () => {
	for (const [argv, code, line] of [
		[
			['--if-present'],
			0,
			'wpt source root not found at ../wpt/css — skipping harvest (--if-present)'
		],
		[[], 1, 'wpt source root not found: ../wpt/css'],
		[['--out', 'benches/js/.cache/x', 'nowhere'], 1, 'wpt source root not found: nowhere']
	] as const) {
		err = [];
		const run = fake_deps({ missing: true });
		strictEqual(await run_wpt_css_harvest(run.deps, argv), code);
		deepStrictEqual(err, [line]);
		strictEqual(run.opened.length, 1);
		deepStrictEqual(run.out_log, []);
		deepStrictEqual(run.files.log, []);
	}
});

describe('to_wpt_out', () => {
	test('accepts the default and any directory strictly beneath the cache, normalized', () => {
		for (const [out, path] of [
			[WPT_OUT_DEFAULT, OUT],
			['benches/js/.cache/wpt_css/', OUT],
			['./benches/js/.cache/x/../wpt_css', OUT],
			['benches/js/.cache/x', 'benches/js/.cache/x'],
			['benches/js/.cache/wpt_css_old/a', 'benches/js/.cache/wpt_css_old/a'],
			['benches/js/.cache/prettier_x', 'benches/js/.cache/prettier_x']
		] as const) {
			deepStrictEqual(to_wpt_out(out, '../wpt/css'), { ok: true, path });
		}
	});

	test('refuses a path outside the cache, the cache itself, or one spelled absolute', () => {
		for (const out of [
			'.git',
			'crates',
			'benches',
			'benches/js',
			'.',
			'',
			'..',
			'../wpt_out',
			'benches/js/.cache',
			'benches/js/.cache/',
			'benches/js/.cache/.',
			'benches/js/.cache/..',
			'benches/js/.cache/../x',
			'benches/js/.cache/x/../../.cache',
			'benches/js/.cachex/a',
			'benches/js/.cachexwpt_css/a',
			'/home/x/tsv/benches/js/.cache/wpt_css',
			'--force'
		]) {
			const check = to_wpt_out(out, '../wpt/css');
			ok(!check.ok, out);
		}
		const why = (out: string) => {
			const check = to_wpt_out(out, '../wpt/css');
			return check.ok ? null : check.message.split(' refused: ')[1]!.split(';')[0];
		};
		strictEqual(why('benches/js/.cache/'), "that is the cache itself, every harvest's");
		strictEqual(why('/home/x/tsv/benches/js/.cache/wpt_css'), 'spell it relative to the checkout');
		strictEqual(why('benches/js/.cache/../x'), 'not beneath benches/js/.cache');
		deepStrictEqual(to_wpt_out(undefined, '../wpt/css'), {
			ok: false,
			message: '--out needs a path'
		});
		deepStrictEqual(to_wpt_out('crates', '../wpt/css'), {
			ok: false,
			message:
				'--out crates refused: not beneath benches/js/.cache; it must be a directory strictly beneath benches/js/.cache/, which the harvest clears'
		});
	});

	test("refuses another harvest's cache or stamp, wpt's own stamp, and the source", () => {
		for (const out of [
			'benches/js/.cache/wpt_css.stamp.json',
			'benches/js/.cache/test262.stamp.json',
			'benches/js/.cache/test262_files.json',
			'benches/js/.cache/test262_manifest.json',
			'benches/js/.cache/ts_repo_files.json',
			'benches/js/.cache/ts_repo_rejects.json',
			'benches/js/.cache/svelte_parse_rejects.json',
			'benches/js/.cache/prettier_jsx_files.json',
			'benches/js/.cache/css_rejects.stamp.json',
			'benches/js/.cache/svelte_styles',
			'benches/js/.cache/svelte_styles/x'
		]) {
			const check = to_wpt_out(out, '../wpt/css');
			ok(!check.ok, out);
		}
		for (const source of [
			'benches/js/.cache/x',
			'benches/js/.cache/x/css',
			'benches/js/.cache',
			'./benches/js/.cache/x/',
			'benches',
			'.',
			'./',
			'..',
			'../..'
		]) {
			ok(!to_wpt_out('benches/js/.cache/x', source).ok, source);
		}
		for (const source of ['benches/js/.cache/xy', '../wpt', '..wpt', '/elsewhere/css']) {
			ok(to_wpt_out('benches/js/.cache/x', source).ok, source);
		}
	});
});

test('a refused --out exits 2 with one line before opening anything', async () => {
	for (const argv of [['--out', '.git'], ['--out', 'benches/js/.cache'], ['--out']]) {
		err = [];
		const run = fake_deps();
		strictEqual(await run_wpt_css_harvest(run.deps, argv), 2);
		strictEqual(err.length, 1);
		ok(err[0]!.startsWith('wpt_css_harvest: --out '), err[0]);
		deepStrictEqual(run.opened, []);
		deepStrictEqual(run.out_log, []);
		deepStrictEqual(run.files.log, []);
	}
});

test('an --out that leaves the cache through a symlink exits 2 before any removal', async () => {
	const run = fake_deps({
		out_refused: 'it reaches out of benches/js/.cache through a symlink'
	});
	strictEqual(await run_wpt_css_harvest(run.deps, ['--out', 'benches/js/.cache/link']), 2);
	deepStrictEqual(err, [
		'wpt_css_harvest: --out benches/js/.cache/link refused: it reaches out of benches/js/.cache through a symlink'
	]);
	deepStrictEqual(run.opened, ['source ../wpt/css', 'out benches/js/.cache/link']);
	deepStrictEqual(run.out_log, []);
	deepStrictEqual(run.files.log, []);
});

test('the output opens at the path to_wpt_out normalized, not as argv spells it', async () => {
	const run = fake_deps({
		out_refused: 'it reaches out of benches/js/.cache through a symlink'
	});
	strictEqual(await run_wpt_css_harvest(run.deps, ['--out', './benches/js/.cache/x/../link/']), 2);
	deepStrictEqual(run.opened, ['source ../wpt/css', 'out benches/js/.cache/link']);
});

test('an --out overlapping the source on real paths exits 2 before any removal, however either is spelled', async () => {
	const cases: Array<[argv: string[], source_path: string]> = [
		// the checkout, spelled through its parent: the output lies inside it
		[['../tsv', '--out', 'benches/js/.cache/x'], '/dev/tsv'],
		// spelled absolute
		[['/dev/tsv', '--out', 'benches/js/.cache/x'], '/dev/tsv'],
		// the source beneath the output, which the removal would delete before the walk
		[
			['/dev/tsv/benches/js/.cache/x/css', '--out', 'benches/js/.cache/x'],
			'/dev/tsv/benches/js/.cache/x/css'
		],
		// the output itself, reached through a symlink
		[['../wpt_link', '--out', 'benches/js/.cache/x'], '/dev/tsv/benches/js/.cache/x'],
		// the root holds everything
		[['/', '--out', 'benches/js/.cache/x'], '/']
	];
	for (const [argv, source_path] of cases) {
		err = [];
		const run = fake_deps({ source_path });
		strictEqual(await run_wpt_css_harvest(run.deps, argv), 2, argv.join(' '));
		deepStrictEqual(err, [
			`wpt_css_harvest: --out benches/js/.cache/x refused: it overlaps the source, ${argv[0]}, at ${source_path}`
		]);
		deepStrictEqual(run.opened, [`source ${argv[0]}`, 'out benches/js/.cache/x']);
		deepStrictEqual(run.out_log, []);
		deepStrictEqual(run.files.log, []);
		// refused before git reads the source's commit
		deepStrictEqual((run.deps.git as ReturnType<typeof fake_stamp_git>).calls, []);
	}
	// beside it, apart: harvested
	for (const source_path of [
		'/dev/tsv/benches/js/.cache/xy',
		'/dev/wpt',
		'/dev/tsv/benches/js/.cache/y'
	]) {
		err = [];
		const run = fake_deps({ source_path });
		strictEqual(
			await run_wpt_css_harvest(run.deps, ['/elsewhere', '--out', 'benches/js/.cache/x']),
			0,
			source_path
		);
		strictEqual(run.out_log[0], 'remove');
	}
});
