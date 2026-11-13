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
 * - `<style>` tags inside a `<script>` block — markup in a JS string a test
 *   builds at runtime (`'<style>' + slottedStyle + '</style>'`). The bytes
 *   between the tags are JS source, not the stylesheet the browser gets: a
 *   concatenation, or a literal whose escapes are JS's (`'\\'`, `\'`) rather
 *   than CSS's. Blocks are still numbered by their position among every
 *   `<style>` match in the file, so a skip here never renames the blocks after
 *   it.
 * - `<style type="…">` blocks whose type is not `text/css` (wpt uses bogus
 *   types deliberately)
 * - whitespace-only blocks
 * - duplicate block content (byte-identical, first path wins) — identical
 *   blocks grade identically, duplicates only add wall-clock (reftest/ref
 *   pairs share styles heavily)
 *
 * Stats JSON to stdout, progress to stderr.
 *
 * Flags: `--if-present` tolerates a missing wpt checkout (warn + exit 0) — for
 * the `bench:conformance` task chain, where an absent `../wpt` should produce
 * a smaller corpus (disclosed via the report's `corpus_sources`), not a failed
 * bench. `--force` re-harvests despite a fresh stamp (default runs skip when
 * the ../wpt commit + pin match the stamp — see `lib/harvest_stamp.ts`; needed
 * after changing this script's extraction logic).
 *
 * Run from the repo root:
 *   npm run bench:harvest:wpt [-- --force]
 *   node benches/js/roots/wpt_css_harvest.ts ../wpt/css --out benches/js/.cache/wpt_css
 *
 * The walk is breadth-first, each directory's entries in code-point order (a recursive
 * `readdir`'s order), so the first path that holds a duplicate block is the same one on every
 * machine.
 *
 * The run clears its output directory before writing, so `--out` is refused, with one line and
 * exit 2 before anything is opened, unless it lies strictly beneath `benches/js/.cache/`,
 * spelled relative to the checkout, apart from every other harvest's cache and stamp and from
 * the source as spelled (`to_wpt_out`); the default, `benches/js/.cache/wpt_css`, is accepted.
 * Once the source and the output are opened, before anything is read or removed, it is refused
 * the same way when a directory on its path beneath the cache is a symlink, when it reaches out
 * of the cache through one, and when it overlaps the source on their real paths, however
 * either is spelled.
 *
 * `run_wpt_css_harvest` is the library function; its root,
 * `benches/js/roots/wpt_css_harvest.ts`, binds its deps (`WptCssHarvestDeps`).
 *
 * @module
 */

import { basename, dirname, join, posix } from 'node:path';

import { WPT_CSS_HARVEST_PIN } from '../lib/gate_counts.ts';
import {
	checkout_skew,
	git_head,
	harvest_up_to_date,
	short_commit,
	type StampGit,
	type StampInputs,
	type StampWrites,
	write_stamp
} from '../lib/harvest_stamp.ts';
import { HARVEST_STAMPS } from '../lib/harvest_stamp_table.ts';
import { sort_listing } from '../lib/listing_order.ts';

/** The wpt source tree, read-only, by paths beneath it. */
export interface WptSource {
	/** Its real path, symlinks resolved. */
	path: string;
	read_text(path: string): Promise<string>;
	/** A directory's entries, in any order. */
	list(path: string): AsyncIterable<{ name: string; kind: string }>;
}

/** The output directory, by paths beneath it. */
export interface WptOut {
	/** The absolute path it resolves to, beneath a cache whose directories are no symlinks. */
	path: string;
	/** Removes the directory and everything in it, if it exists. */
	remove(): Promise<void>;
	/** Makes `path` beneath the directory (`''`, the directory itself), with its parents. */
	mkdir(path: string): Promise<void>;
	write(path: string, data: string): Promise<void>;
}

/** What `run_wpt_css_harvest` reaches the world through. */
export interface WptCssHarvestDeps {
	/** `git` for the stamp's checkout commit. */
	git: StampGit;
	/** The checkout's files by project-root-relative path; writes only the stamp. */
	files: StampWrites;
	/** Opens the source tree at `path` as the run spells it, or `null` when nothing is there. */
	open_source(path: string): Promise<WptSource | null>;
	/**
	 * Opens the output directory at `path`, a project-root-relative path `to_wpt_out` accepted,
	 * or says why it's refused: a symlink on its path beneath `benches/js/.cache`.
	 */
	open_out(path: string): Promise<WptOut | { refused: string }>;
	/** Where the stats JSON goes. */
	stdout: { write(data: string): Promise<unknown> };
}

export const WPT_OUT_DEFAULT = 'benches/js/.cache/wpt_css';
export const WPT_SOURCE_DEFAULT = '../wpt/css';
const STAMP_PATH = HARVEST_STAMPS['wpt-css'].path;
/** The directory every harvest writes beneath, which `--out` must lie strictly beneath. */
const CACHE_DIR = dirname(WPT_OUT_DEFAULT);
/** The exit code of a refused `--out`. */
export const WPT_OUT_USAGE_EXIT = 2;

/** The answer `to_wpt_out` gives an `--out`. */
export type WptOutCheck = { ok: true; path: string } | { ok: false; message: string };

/** Whether the paths `a` and `b` (normalized, both relative or both absolute) are one, or one
 * holds the other. */
const overlaps = (a: string, b: string): boolean => holds(a, b) || holds(b, a);

/** Whether the path `outer` is `inner` or a directory above it. */
const holds = (outer: string, inner: string): boolean =>
	outer === inner || inner.startsWith(outer.endsWith('/') ? outer : `${outer}/`);

/**
 * The output directory `out` names, refusing any but a directory strictly beneath
 * `benches/js/.cache/`, since the run removes it whole: spelled relative to the checkout (an
 * absolute path is refused), its `..` segments resolved lexically, never the cache itself, and
 * apart from every other harvest's cache and stamp, each named for its stamp (a name beneath
 * the cache that is another stamp's file, or that stamp's name alone or followed by `.` or
 * `_`, like `test262_files.json` beside `test262.stamp.json`), from wpt's own stamp, and from a
 * relative `source` as spelled (one spelled absolute is not compared). Lexical alone: the run
 * refuses a symlink on the path and an overlap on real paths once both are opened.
 * @param out - the `--out` value, `undefined` when the flag has none
 * @param source - the source tree as the run spells it
 * @returns the normalized project-root-relative path, or the one-line refusal
 */
export const to_wpt_out = (out: string | undefined, source: string): WptOutCheck => {
	if (out === undefined || out === '') return { ok: false, message: '--out needs a path' };
	const refuse = (why: string): WptOutCheck => ({
		ok: false,
		message: `--out ${out} refused: ${why}; it must be a directory strictly beneath ${CACHE_DIR}/, which the harvest clears`
	});
	if (posix.isAbsolute(out)) return refuse('spell it relative to the checkout');
	const path = posix.normalize(out).replace(/\/+$/, '');
	if (path === CACHE_DIR) return refuse("that is the cache itself, every harvest's");
	if (!path.startsWith(`${CACHE_DIR}/`)) return refuse(`not beneath ${CACHE_DIR}`);
	const name = path.slice(CACHE_DIR.length + 1).split('/')[0]!;
	for (const [harvest, stamp] of Object.entries(HARVEST_STAMPS)) {
		const stamp_name = basename(stamp.path);
		const base = stamp_name.slice(0, -'.stamp.json'.length);
		if (
			name === stamp_name ||
			(harvest !== 'wpt-css' &&
				(name === base || name.startsWith(`${base}.`) || name.startsWith(`${base}_`)))
		) {
			return refuse(`${CACHE_DIR}/${name} is ${harvest}'s cache or stamp`);
		}
	}
	const src = posix.normalize(source).replace(/\/+$/, '');
	// `.`, `..`, `../..`: the checkout or a directory above it, which holds every output
	const holds_checkout = /^\.(\.(\/\.\.)*)?$/.test(src);
	if (!posix.isAbsolute(src) && (holds_checkout || overlaps(path, src))) {
		return refuse(`it overlaps the source, ${source}`);
	}
	return { ok: true, path };
};

const STYLE_BLOCK_RE = /<style\b([^>]*)>([\s\S]*?)<\/style\s*>/gi;
const SCRIPT_BLOCK_RE = /<script\b[^>]*>[\s\S]*?<\/script\s*>/gi;
const TYPE_ATTR_RE = /\btype\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))/i;

/**
 * Every entry beneath `source`, breadth first, each directory's entries in code-point order,
 * as `path` beneath the source. Directories are walked, never followed through a symlink.
 */
export async function* walk_breadth_first(
	source: WptSource
): AsyncGenerator<{ path: string; name: string; kind: string }> {
	const queue = [''];
	for (let dir = queue.shift(); dir !== undefined; dir = queue.shift()) {
		const entries: Array<{ name: string; kind: string }> = [];
		for await (const entry of source.list(dir)) entries.push(entry);
		for (const entry of sort_listing(entries)) {
			const path = dir === '' ? entry.name : `${dir}/${entry.name}`;
			if (entry.kind === 'directory') queue.push(path);
			yield { path, name: entry.name, kind: entry.kind };
		}
	}
}

/**
 * The harvest.
 * @returns the exit code
 */
export async function run_wpt_css_harvest(
	deps: WptCssHarvestDeps,
	argv: ReadonlyArray<string>
): Promise<number> {
	const out_flag_index = argv.indexOf('--out');
	const out_dir = out_flag_index === -1 ? WPT_OUT_DEFAULT : argv[out_flag_index + 1];
	const if_present = argv.includes('--if-present');
	const force = argv.includes('--force');
	const source_root =
		argv.find(
			(a, i) => !a.startsWith('-') && (out_flag_index === -1 || i !== out_flag_index + 1)
		) ?? WPT_SOURCE_DEFAULT;
	const out_check = to_wpt_out(out_dir, source_root);
	if (!out_check.ok) {
		console.error(`wpt_css_harvest: ${out_check.message}`);
		return WPT_OUT_USAGE_EXIT;
	}

	const source = await deps.open_source(source_root);
	if (source === null) {
		if (if_present) {
			console.error(
				`wpt source root not found at ${source_root} — skipping harvest (--if-present)`
			);
			return 0;
		}
		console.error(`wpt source root not found: ${source_root}`);
		return 1;
	}
	const refuse_out = (why: string): number => {
		console.error(`wpt_css_harvest: --out ${out_dir} refused: ${why}`);
		return WPT_OUT_USAGE_EXIT;
	};
	const out = await deps.open_out(out_check.path);
	if ('refused' in out) return refuse_out(out.refused);
	// on real paths, however either is spelled: the lexical check above sees only the spelling
	if (overlaps(out.path, source.path)) {
		return refuse_out(`it overlaps the source, ${source_root}, at ${source.path}`);
	}

	// Freshness stamp (default source + out only — a custom run is an experiment,
	// not the cache): skip when the ../wpt commit and the pinned count match.
	const stamped_run = source_root === WPT_SOURCE_DEFAULT && out_dir === WPT_OUT_DEFAULT;
	const source_commit = await git_head(deps.git, source_root);
	const stamp_inputs: StampInputs = {
		harvest: 'wpt-css',
		source_commit,
		pin: WPT_CSS_HARVEST_PIN
	};
	if (
		stamped_run &&
		!force &&
		source_commit !== null &&
		(await harvest_up_to_date(deps.files, STAMP_PATH, stamp_inputs, [out_dir]))
	) {
		console.error(
			`wpt harvest up to date (../wpt at ${short_commit(source_commit)}, ` +
				`pin ${WPT_CSS_HARVEST_PIN}) — skipping; --force to re-harvest.`
		);
		return 0;
	}

	const stats = {
		html_files_scanned: 0,
		sub_html_skipped: 0,
		files_with_blocks: 0,
		blocks_written: 0,
		blocks_skipped_in_script: 0,
		blocks_skipped_type: 0,
		blocks_skipped_empty: 0,
		blocks_skipped_duplicate: 0,
		bytes_written: 0
	};

	await out.remove();
	await out.mkdir('');

	const seen_content = new Set<string>();

	for await (const entry of walk_breadth_first(source)) {
		if (entry.kind !== 'file' || !entry.name.endsWith('.html')) continue;
		if (entry.name.endsWith('.sub.html')) {
			stats.sub_html_skipped++;
			continue;
		}
		stats.html_files_scanned++;
		const rel = entry.path;
		const html = await source.read_text(rel);
		const script_spans = [...html.matchAll(SCRIPT_BLOCK_RE)].map(
			(m) => [m.index, m.index + m[0].length] as const
		);
		let block_index = 0;
		let wrote_any = false;
		for (const match of html.matchAll(STYLE_BLOCK_RE)) {
			const index = block_index++;
			if (script_spans.some(([start, end]) => match.index > start && match.index < end)) {
				stats.blocks_skipped_in_script++;
				continue;
			}
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
			const out_dir_rel = dirname(rel);
			await out.mkdir(out_dir_rel === '.' ? '' : out_dir_rel);
			await out.write(join(out_dir_rel, `${basename(rel, '.html')}__${index}.css`), css);
			stats.blocks_written++;
			stats.bytes_written += css.length;
			wrote_any = true;
		}
		if (wrote_any) stats.files_with_blocks++;
		if (stats.html_files_scanned % 5000 === 0) {
			console.error(`scanned ${stats.html_files_scanned} files, ${stats.blocks_written} blocks…`);
		}
	}

	// Pinned count (exact, default source only): the wpt checkout is updated
	// deliberately, so any move — a gutted sparse-checkout harvesting a tiny cache,
	// or a pull growing it — must be re-pinned, not absorbed. On mismatch the cache
	// is removed so downstream loaders see "absent" (disclosed) rather than a
	// wrong-sized cache. Applies regardless of --if-present (that flag tolerates a
	// MISSING source, not a changed harvest). See ../lib/gate_counts.ts.
	if (source_root === WPT_SOURCE_DEFAULT && stats.blocks_written !== WPT_CSS_HARVEST_PIN) {
		console.error(
			`FAIL: pinned count mismatch — harvested ${stats.blocks_written} blocks ≠ pinned ${WPT_CSS_HARVEST_PIN}. ` +
				`Removing ${out_dir}; ${
					checkout_skew({ '../wpt': source_commit }) ??
					'if the move is deliberate (wpt pull), re-pin in lib/gate_counts.ts.'
				}`
		);
		await out.remove();
		return 1;
	}

	if (stamped_run && source_commit !== null) {
		await write_stamp(deps.files, STAMP_PATH, stamp_inputs);
	}
	console.error(
		`done: ${stats.blocks_written} blocks from ${stats.files_with_blocks} files → ${out_dir}`
	);
	await deps.stdout.write(JSON.stringify(stats, null, '\t') + '\n');
	return 0;
}
