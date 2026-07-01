/**
 * Harvest `<style>` blocks from the wpt `css/` corpus into a cache of
 * standalone `.css` files, so the existing corpus tools (`arena_stats`,
 * `skip_triage`, `corpus:compare:parse`, `corpus:compare:format`) can grade
 * them without teaching the corpus loader wpt-specific extraction — the loader
 * maps `.html` to Svelte, which would parse wpt's real-world HTML as a Svelte
 * component (noise about HTML, not signal about CSS).
 *
 * Output mirrors the source tree (`<out>/<spec-dir>/.../<file>__<i>.css`), so
 * path filters and per-spec-dir runs work downstream. The cache is derived and
 * gitignored; the out dir is cleared on each run — regenerate any time.
 *
 * Skips:
 * - `*.sub.html` — wpt server-side substitution templates (`{{…}}`
 *   placeholders are not CSS)
 * - `<style type="…">` blocks whose type is not `text/css` (wpt uses bogus
 *   types deliberately)
 * - whitespace-only blocks
 * - duplicate block content (byte-identical, first path wins) — identical
 *   blocks grade identically, duplicates only add wall-clock (reftest/ref
 *   pairs share styles heavily)
 *
 * Stats JSON to stdout, progress to stderr.
 *
 * Run from the repo root:
 *   deno run --allow-read --allow-write benches/js/diagnostics/wpt_css_harvest.ts
 *   deno run --allow-read --allow-write benches/js/diagnostics/wpt_css_harvest.ts ../wpt/css --out benches/js/.cache/wpt_css
 */

import { mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { basename, dirname, join, relative } from 'node:path';

const OUT_DEFAULT = 'benches/js/.cache/wpt_css';

const out_flag_index = Deno.args.indexOf('--out');
const out_dir = out_flag_index === -1 ? OUT_DEFAULT : Deno.args[out_flag_index + 1];
const source_root =
	Deno.args.find((a, i) => !a.startsWith('-') && i !== out_flag_index + 1) ?? '../wpt/css';

const STYLE_BLOCK_RE = /<style\b([^>]*)>([\s\S]*?)<\/style\s*>/gi;
const TYPE_ATTR_RE = /\btype\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))/i;

const stats = {
	html_files_scanned: 0,
	sub_html_skipped: 0,
	files_with_blocks: 0,
	blocks_written: 0,
	blocks_skipped_type: 0,
	blocks_skipped_empty: 0,
	blocks_skipped_duplicate: 0,
	bytes_written: 0,
};

await rm(out_dir, { recursive: true, force: true });
await mkdir(out_dir, { recursive: true });

const seen_content = new Set<string>();
const entries = await readdir(source_root, { recursive: true, withFileTypes: true });

for (const entry of entries) {
	if (!entry.isFile() || !entry.name.endsWith('.html')) continue;
	if (entry.name.endsWith('.sub.html')) {
		stats.sub_html_skipped++;
		continue;
	}
	stats.html_files_scanned++;
	const path = join(entry.parentPath, entry.name);
	const html = await readFile(path, 'utf8');
	let block_index = 0;
	let wrote_any = false;
	for (const match of html.matchAll(STYLE_BLOCK_RE)) {
		const index = block_index++;
		const type = match[1].match(TYPE_ATTR_RE);
		if (type) {
			const value = (type[1] ?? type[2] ?? type[3]).trim().toLowerCase();
			if (value !== 'text/css') {
				stats.blocks_skipped_type++;
				continue;
			}
		}
		const css = match[2];
		if (css.trim() === '') {
			stats.blocks_skipped_empty++;
			continue;
		}
		if (seen_content.has(css)) {
			stats.blocks_skipped_duplicate++;
			continue;
		}
		seen_content.add(css);
		const rel = relative(source_root, path);
		const out_path = join(out_dir, dirname(rel), `${basename(rel, '.html')}__${index}.css`);
		await mkdir(dirname(out_path), { recursive: true });
		await writeFile(out_path, css);
		stats.blocks_written++;
		stats.bytes_written += css.length;
		wrote_any = true;
	}
	if (wrote_any) stats.files_with_blocks++;
	if (stats.html_files_scanned % 5000 === 0) {
		console.error(`scanned ${stats.html_files_scanned} files, ${stats.blocks_written} blocks…`);
	}
}

console.error(`done: ${stats.blocks_written} blocks from ${stats.files_with_blocks} files → ${out_dir}`);
console.log(JSON.stringify(stats, null, '\t'));
