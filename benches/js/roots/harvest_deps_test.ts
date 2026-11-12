/**
 * The harvests' shared root deps over a scratch directory: `cache_files` writes beneath
 * `benches/js/.cache` alone (the directory itself made through the checkout) and reads
 * anywhere in it; `open_cache_out` refuses an output with a symlink on its path beneath the
 * cache or one leaving it, and `bench:harvest:wpt`'s `--out`, through it, an output that
 * overlaps the source on real paths, each before anything is removed.
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';
import { basename, join, resolve } from 'node:path';
import { describe, test, vi } from 'vitest';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';

import { run_wpt_css_harvest } from '../diagnostics/wpt_css_harvest.ts';
import { cache_files, HARVEST_CACHE_DIR, open_cache_out } from './harvest_deps.ts';

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

/** A scratch checkout whose cache holds another harvest's files, `svelte_styles/a.css`. */
const plant_cache = async (dir: FsDir): Promise<void> => {
	await dir.mkdir(`${HARVEST_CACHE_DIR}/svelte_styles`, { recursive: true });
	await dir.write(`${HARVEST_CACHE_DIR}/svelte_styles/a.css`, 'a{}');
	await dir.mkdir('outside/sub', { recursive: true });
	await dir.write('outside/sub/b.css', 'b{}');
};

describe('open_cache_out', () => {
	test('refuses a path with a symlink for a directory beneath the cache, wherever it leads', async () => {
		await using dir = await fs.temp_dir('tsv_harvest_deps_');
		await plant_cache(dir);
		await dir.symlink(`${HARVEST_CACHE_DIR}/inner`, 'svelte_styles');
		await dir.mkdir(`${HARVEST_CACHE_DIR}/a`, { recursive: true });
		await dir.symlink(`${HARVEST_CACHE_DIR}/a/b`, '../svelte_styles');
		await dir.symlink(`${HARVEST_CACHE_DIR}/out_relative`, '../../../outside');
		await dir.symlink(`${HARVEST_CACHE_DIR}/out_absolute`, join(dir.path, 'outside'));
		for (const [path, link] of [
			['inner/sub', 'inner'],
			['inner/a.css', 'inner'],
			['a/b/sub', 'a/b'],
			['a/b/x/y', 'a/b'],
			['out_relative/sub', 'out_relative'],
			['out_absolute/sub', 'out_absolute']
		]) {
			deepStrictEqual(await open_cache_out(dir, `${HARVEST_CACHE_DIR}/${path}`), {
				refused: `${HARVEST_CACHE_DIR}/${link} is a symlink`
			});
		}
		strictEqual(await dir.read_text(`${HARVEST_CACHE_DIR}/svelte_styles/a.css`), 'a{}');
		strictEqual(await dir.read_text('outside/sub/b.css'), 'b{}');
	});

	test('refuses a final symlink that leaves the cache, and removes one inside as the link', async () => {
		await using dir = await fs.temp_dir('tsv_harvest_deps_');
		await plant_cache(dir);
		await dir.symlink(`${HARVEST_CACHE_DIR}/out`, join(dir.path, 'outside'));
		deepStrictEqual(await open_cache_out(dir, `${HARVEST_CACHE_DIR}/out`), {
			refused: `it reaches out of ${HARVEST_CACHE_DIR} through a symlink`
		});
		await dir.symlink(`${HARVEST_CACHE_DIR}/final`, 'svelte_styles');
		const out = await open_cache_out(dir, `${HARVEST_CACHE_DIR}/final`);
		ok(!('refused' in out));
		await out.remove();
		strictEqual(await dir.stat(`${HARVEST_CACHE_DIR}/final`, { follow: false }), null);
		strictEqual(await dir.read_text(`${HARVEST_CACHE_DIR}/svelte_styles/a.css`), 'a{}');
	});

	test('removes through a handle on the cache, so a directory swapped for a symlink out of it after the check is never followed', async () => {
		await using dir = await fs.temp_dir('tsv_harvest_deps_');
		await plant_cache(dir);
		await dir.mkdir(`${HARVEST_CACHE_DIR}/x`, { recursive: true });
		const out = await open_cache_out(dir, `${HARVEST_CACHE_DIR}/x/sub`);
		ok(!('refused' in out));
		// a relative link that leaves the cache and stays in the checkout
		await dir.remove(`${HARVEST_CACHE_DIR}/x`, { recursive: true });
		await dir.symlink(`${HARVEST_CACHE_DIR}/x`, '../../../outside');
		await rejects(out.remove(), (error) => is_fs_error(error, 'outside_root'));
		strictEqual(await dir.read_text('outside/sub/b.css'), 'b{}');
	});

	test('opens a path beneath a cache that may not exist yet, at its real path', async () => {
		await using dir = await fs.temp_dir('tsv_harvest_deps_');
		const fresh = await open_cache_out(dir, `${HARVEST_CACHE_DIR}/x/y`);
		ok(!('refused' in fresh));
		strictEqual(fresh.path, join(dir.path, HARVEST_CACHE_DIR, 'x/y'));
		await fresh.remove();
		await fresh.mkdir('');
		await fresh.write('a.css', 'a{}');
		strictEqual(await dir.read_text(`${HARVEST_CACHE_DIR}/x/y/a.css`), 'a{}');

		// a cache that is itself a symlink inside the checkout resolves to where it leads
		await dir.remove(HARVEST_CACHE_DIR, { recursive: true });
		await dir.mkdir('benches/real_cache', { recursive: true });
		strictEqual(await dir.symlink(HARVEST_CACHE_DIR, '../real_cache'), 'created');
		const linked = await open_cache_out(dir, `${HARVEST_CACHE_DIR}/z`);
		ok(!('refused' in linked));
		strictEqual(linked.path, join(dir.path, 'benches/real_cache/z'));
	});
});

describe("bench:harvest:wpt's --out through the root's deps, over a scratch checkout", () => {
	/** Runs the harvest's argument handling as its root binds it, the checkout `dir`. */
	const run = async (dir: FsDir, argv: string[]) => {
		const err: string[] = [];
		const spy = vi
			.spyOn(console, 'error')
			.mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
		const writes: string[] = [];
		try {
			const code = await run_wpt_css_harvest(
				{
					git: { out: () => Promise.reject(new Error('git is never reached')) },
					files: {
						...cache_files(dir),
						write: async (path) => void writes.push(path),
						mkdir: async (path) => void writes.push(path)
					},
					open_source: async (path) => (await fs.open_dir(resolve(dir.path, path))).readonly(),
					open_out: (path) => open_cache_out(dir, path),
					stdout: { write: async () => undefined }
				},
				argv
			);
			return { code, err, writes };
		} finally {
			spy.mockRestore();
		}
	};

	test('a directory symlink beneath the cache is refused before anything is removed', async () => {
		await using dir = await fs.temp_dir('tsv_harvest_deps_');
		await plant_cache(dir);
		await dir.mkdir('wpt/css', { recursive: true });
		await dir.mkdir(`${HARVEST_CACHE_DIR}/svelte_styles/sub`, { recursive: true });
		await dir.symlink(`${HARVEST_CACHE_DIR}/inner`, 'svelte_styles');
		deepStrictEqual(await run(dir, ['wpt/css', '--out', `${HARVEST_CACHE_DIR}/inner/sub`]), {
			code: 2,
			err: [
				`wpt_css_harvest: --out ${HARVEST_CACHE_DIR}/inner/sub refused: ${HARVEST_CACHE_DIR}/inner is a symlink`
			],
			writes: []
		});
		strictEqual((await dir.stat(`${HARVEST_CACHE_DIR}/svelte_styles/sub`))?.kind, 'directory');
		strictEqual(await dir.read_text(`${HARVEST_CACHE_DIR}/svelte_styles/a.css`), 'a{}');
	});

	test('a source that holds the output or lies in it is refused however it is spelled', async () => {
		await using dir = await fs.temp_dir('tsv_harvest_deps_');
		await plant_cache(dir);
		await dir.mkdir(`${HARVEST_CACHE_DIR}/x/css`, { recursive: true });
		await dir.write(`${HARVEST_CACHE_DIR}/x/css/a.html`, '<style>a{}</style>');
		await dir.symlink('wpt_link', `${HARVEST_CACHE_DIR}/x`);
		const name = basename(dir.path);
		for (const [source, real] of [
			[`../${name}`, dir.path],
			[dir.path, dir.path],
			[join(dir.path, HARVEST_CACHE_DIR, 'x/css'), join(dir.path, HARVEST_CACHE_DIR, 'x/css')],
			['wpt_link', join(dir.path, HARVEST_CACHE_DIR, 'x')]
		]) {
			deepStrictEqual(
				await run(dir, [source!, '--out', `${HARVEST_CACHE_DIR}/x`]),
				{
					code: 2,
					err: [
						`wpt_css_harvest: --out ${HARVEST_CACHE_DIR}/x refused: it overlaps the source, ${source}, at ${real}`
					],
					writes: []
				},
				source
			);
		}
		strictEqual(await dir.read_text(`${HARVEST_CACHE_DIR}/x/css/a.html`), '<style>a{}</style>');
	});
});
