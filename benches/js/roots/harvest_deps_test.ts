/**
 * The harvests' shared root deps over a scratch directory: `cache_files` writes beneath
 * `benches/js/.cache` alone (the directory itself made through the checkout) and reads
 * anywhere in it; `sibling_reads` answers an absent checkout as absent and follows a checkout
 * that is a symlink, as `audit:corpus` reads `../prettier` through it.
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';
import { join } from 'node:path';
import { test, vi } from 'vitest';

import { fs } from '@fuzdev/gro/fs.ts';

import { run_corpus_audit } from '../corpus_audit.ts';
import { cache_files, HARVEST_CACHE_DIR, sibling_reads } from './harvest_deps.ts';

test('cache_files writes beneath the cache directory and nowhere else', async () => {
	await using dir = await fs.temp_dir('tsv_harvest_deps_');
	await dir.write('package.json', '{}');
	const files = cache_files(dir);
	await files.mkdir(HARVEST_CACHE_DIR, { recursive: true });
	await files.mkdir(`${HARVEST_CACHE_DIR}/wpt_css`, { recursive: true });
	await files.write(`${HARVEST_CACHE_DIR}/a.stamp.json`, 'stamped');
	strictEqual(await dir.read_text('benches/js/.cache/a.stamp.json'), 'stamped');
	strictEqual(await files.read_text('package.json'), '{}');
	ok(await files.exists('benches/js/.cache/wpt_css'));
	for (const path of [
		'package.json',
		'benches/js/.cachex/a',
		// one character past the directory's name: a prefix test without the `/` reads it as `wpt_css/a`
		'benches/js/.cachexwpt_css/a',
		'benches/js/.cache/../x',
		`${HARVEST_CACHE_DIR}`
	]) {
		await rejects(files.write(path, 'x'));
	}
	await rejects(files.mkdir('benches/js/other', { recursive: true }));
	strictEqual(await dir.exists('benches/js/other'), false);
	strictEqual(await dir.exists('benches/js/x'), false);
	strictEqual(await dir.exists('benches/js/.cache/wpt_css/a'), false);
	strictEqual(await dir.read_text('package.json'), '{}');
});

test('cache_files makes the cache directory, with its parents, for a directory beneath it', async () => {
	await using dir = await fs.temp_dir('tsv_harvest_deps_');
	const files = cache_files(dir);
	await files.mkdir(`${HARVEST_CACHE_DIR}/svelte_styles`, { recursive: true });
	strictEqual((await dir.stat('benches/js/.cache/svelte_styles'))?.kind, 'directory');
	await files.write(`${HARVEST_CACHE_DIR}/svelte_styles/a.css`, 'a');
	strictEqual(await dir.read_text('benches/js/.cache/svelte_styles/a.css'), 'a');
});

test('sibling_reads reads an absent checkout as absent and fails its reads', async () => {
	const absent = sibling_reads('../tsv_no_such_sibling_checkout');
	strictEqual(await absent.exists('tests'), false);
	await rejects(absent.read_text('x'));
});

test('audit:corpus reads ../prettier through a symlink as through a directory', async () => {
	await using dir = await fs.temp_dir('tsv_harvest_deps_');
	// the suites live outside the directory the checkout sits in, which a link to them leaves
	for (const root of ['elsewhere/prettier', 'parent_dir/prettier']) {
		await dir.mkdir(`${root}/tests/format/js`, { recursive: true });
		await dir.write(`${root}/tests/format/js/a.js`, 'a;');
		await dir.mkdir(`${root}/tests/format/css`, { recursive: true });
	}
	await dir.mkdir('parent_relative', { recursive: true });
	await dir.symlink('parent_relative/prettier', '../elsewhere/prettier');
	await dir.mkdir('parent_absolute', { recursive: true });
	await dir.symlink('parent_absolute/prettier', join(dir.path, 'elsewhere/prettier'));
	await dir.mkdir('parent_dangling', { recursive: true });
	await dir.symlink('parent_dangling/prettier', join(dir.path, 'no_such_prettier'));
	await dir.mkdir('parent_absent', { recursive: true });

	const audit = async (parent: string) => {
		const err: string[] = [];
		const calls: Array<ReadonlyArray<string>> = [];
		const spy = vi
			.spyOn(console, 'error')
			.mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
		try {
			const code = await run_corpus_audit({
				seeds: async () => ({ dirs: ['../corpora'], live_files: [] }),
				prettier: sibling_reads(join(dir.path, parent, 'prettier')),
				cargo: { run: async (args) => (calls.push(args), { kind: 'exited', code: 0 }) }
			});
			return { code, err, calls };
		} finally {
			spy.mockRestore();
		}
	};

	const present = await audit('parent_dir');
	strictEqual(present.code, 0);
	deepStrictEqual(present.err.slice(0, 1), [
		'  ⚠ prettier suite missing, skipped: ../prettier/tests/format/typescript'
	]);
	ok(
		present.calls.some(
			(args) =>
				args.includes('../prettier/tests/format/js') &&
				args.includes('../prettier/tests/format/css')
		)
	);
	deepStrictEqual(await audit('parent_relative'), present);
	deepStrictEqual(await audit('parent_absolute'), present);

	const absent = await audit('parent_absent');
	strictEqual(absent.code, 0);
	deepStrictEqual(absent.err.slice(0, 3), [
		'  ⚠ prettier suite missing, skipped: ../prettier/tests/format/typescript',
		'  ⚠ prettier suite missing, skipped: ../prettier/tests/format/js',
		'  ⚠ prettier suite missing, skipped: ../prettier/tests/format/css'
	]);
	ok(!absent.calls.some((args) => args.some((arg) => arg.startsWith('../prettier/'))));
	deepStrictEqual(await audit('parent_dangling'), absent);
});
