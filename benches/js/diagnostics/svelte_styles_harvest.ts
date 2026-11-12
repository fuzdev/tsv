/**
 * Harvest real-authored CSS for the corpus: extract every component-level
 * `<style>` block from the **perf view's** `.svelte` files and concatenate them
 * per source collection into `benches/js/.cache/svelte_styles/<collection>.css`.
 * That directory is a `benched` corpus entry, so the harvest adds component CSS
 * people actually wrote beside the standalone files (mostly CSS-framework source) —
 * and because the blocks are concatenated per collection, file sizes are
 * driven by reality (each collection's total embedded CSS), not an arbitrary chunking
 * knob, and large files measure engine throughput rather than per-call fixed
 * costs. In the gates view the concats additionally exercise the *standalone*
 * CSS formatting path on real content (embedded CSS rides `EmbedContext` — a
 * different path), so this is conformance coverage too, not just perf sample
 * size. The same bytes are also timed inside the svelte rows; rows are never
 * summed, so that's a disclosure note (benches/js/CLAUDE.md §Corpus), not a
 * distortion.
 *
 * Extraction is textual and deliberately conservative: only `<style …>` /
 * `</style>` tags at line start (the component-level convention every formatter
 * normalizes to), skipping `lang=` blocks that aren't CSS. Svelte allows one
 * style element per component, so this misses nothing structural; the parser is
 * the downstream arbiter (a bad extraction shows up as a gates error, never
 * silently). Each block is dedented by the indent its lines share — the one level
 * it carried inside `<style>` — so the concat reads like authored standalone CSS
 * rather than a uniform one-tab re-indent job for every tool; otherwise the bytes
 * are verbatim, joined with blank lines under a generated-file banner.
 *
 * Stamped like the suite harvests (`lib/harvest_stamp.ts`): the perf view is the
 * pinned `../corpora` snapshot, so the stamp records its `collections/` tree id, the
 * pinned block count, the perf view's entry list (a collection joining a perf
 * tier changes the input with no checkout moving), and the loader's filter
 * fingerprint (an exclusion or extension edit in `lib/corpus.ts` changes what the
 * view yields with nothing else moving), and unchanged inputs skip the walk. The count is an EXACT pin (`SVELTE_STYLES_BLOCKS_PIN`): a move is a
 * snapshot refresh or a view change (re-pin) or a broken extraction, and either fails
 * BEFORE writing so a wrong cache never replaces a good one. `--force` re-harvests
 * despite a fresh stamp (after a change to the extraction itself, which the stamp
 * can't see).
 *
 * Run (from repo root):
 *   npm run bench:harvest:svelte-styles [-- --force]
 *
 * `run_svelte_styles_harvest` is the library function; its root,
 * `benches/js/roots/svelte_styles_harvest.ts`, binds its deps (`SvelteStylesHarvestDeps`).
 *
 * @module
 */

import { relative, sep } from 'node:path';

import { CORPORA_COLLECTIONS, CORPORA_ROOT, CORPORA_URL } from '../lib/corpora.ts';
import { SVELTE_STYLES_CACHE } from '../lib/corpus.ts';
import { GATE_CHECKOUT_IDS, SVELTE_STYLES_BLOCKS_PIN } from '../lib/gate_counts.ts';
import {
	checkout_hash,
	checkout_label,
	corpus_filter_fingerprint,
	harvest_up_to_date,
	short_commit,
	type StampGit,
	type StampWrites,
	write_stamp
} from '../lib/harvest_stamp.ts';
import { HARVEST_STAMPS } from '../lib/harvest_stamp_table.ts';
import type { Logger, SourceFile } from '../lib/types.ts';

/** The perf view, as `lib/corpus.ts` reads it. */
export interface PerfCorpus {
	/** The view's entry list (`corpus_view_paths`). */
	view_paths(view: 'perf'): Promise<string[]>;
	/** The view's files, streamed (`CorpusLoader.stream`). */
	stream(logger: Logger): AsyncIterable<SourceFile>;
}

/** The cache directory, `SVELTE_STYLES_CACHE`, by names in it. */
export interface StylesDir {
	read_text_if_exists(name: string): Promise<string | null>;
	write(name: string, data: string): Promise<void>;
	/** The directory's entries, in any order. */
	list(): AsyncIterable<{ name: string }>;
	remove(name: string): Promise<void>;
}

/** What `run_svelte_styles_harvest` reaches the world through. */
export interface SvelteStylesHarvestDeps {
	/** `git` for the stamp's tree id. */
	git: StampGit;
	/** The checkout's files by project-root-relative path; writes the stamp and makes the cache. */
	files: StampWrites;
	corpus: PerfCorpus;
	/** The cache the concats go to; made by the harvest before its first use. */
	styles: StylesDir;
	/** The snapshot's collections directory, as the loader's paths spell it beneath. */
	collections_root: string;
}

/** Line-anchored component-level style blocks; group 1 = attrs, group 2 = content. */
const STYLE_RE = /^<style([^>]*)>\n([\s\S]*?)^<\/style>/gm;

/**
 * Strip the leading whitespace every non-blank line of `block` shares. Only the
 * common prefix goes, so a block whose lines are unevenly indented loses just the
 * level they all carry, and one with an unindented line is returned untouched.
 */
const dedent = (block: string): string => {
	const lines = block.split('\n');
	let common: string | undefined;
	for (const line of lines) {
		if (line.trim() === '') continue;
		const indent = /^[\t ]*/.exec(line)![0];
		if (common === undefined) {
			common = indent;
		} else {
			let i = 0;
			while (i < common.length && common[i] === indent[i]) i++;
			common = common.slice(0, i);
		}
		if (common === '') return block;
	}
	if (!common) return block; // all-blank input (the caller already skips it)
	const prefix = common;
	return lines.map((l) => (l.startsWith(prefix) ? l.slice(prefix.length) : l)).join('\n');
};

/**
 * The harvest.
 * @returns the exit code
 */
export async function run_svelte_styles_harvest(
	deps: SvelteStylesHarvestDeps,
	argv: ReadonlyArray<string>
): Promise<number> {
	const force = argv.includes('--force');
	const stamp = HARVEST_STAMPS['svelte-styles'];
	const tree_input = stamp.checkouts.corpora_tree;
	const corpora_tree = await checkout_hash(deps.git, tree_input);
	if (corpora_tree === null) {
		console.error(
			`svelte_styles_harvest: ${checkout_label(tree_input)} is not a git checkout tree — ` +
				`the perf view reads the corpora snapshot (git clone ${CORPORA_URL} ${CORPORA_ROOT})`
		);
		return 1;
	}
	const inputs = {
		corpora_tree,
		blocks_pin: SVELTE_STYLES_BLOCKS_PIN,
		perf_entries: (await deps.corpus.view_paths('perf')).join(' '),
		filters: await corpus_filter_fingerprint(deps.files, 'perf')
	};
	if (!force && (await harvest_up_to_date(deps.files, stamp.path, inputs, [SVELTE_STYLES_CACHE]))) {
		console.error(
			`svelte_styles_harvest: up to date (${checkout_label(tree_input)} at ${short_commit(corpora_tree)}, ` +
				`${SVELTE_STYLES_BLOCKS_PIN} blocks, same perf entries) — skipping; --force to re-harvest`
		);
		return 0;
	}

	// The styles cache itself is a perf-view entry, so on a re-run the loader
	// yields the previous concats too — harmless, the `.svelte` filter drops
	// them (and its absence on a first run is `optional`, a warning). A missing
	// snapshot collection fails fast with the loader's clear error instead of
	// surfacing as a misleading pin trip on a silently smaller harvest.
	const by_collection = new Map<string, { blocks: string[]; bytes: number }>();
	let blocks = 0;
	for await (const f of deps.corpus.stream((m) => console.error(m))) {
		if (f.language !== 'svelte' || !f.path.endsWith('.svelte')) continue;
		for (const m of f.content.matchAll(STYLE_RE)) {
			const attrs = m[1];
			const lang = /lang\s*=\s*["']?([\w-]+)/.exec(attrs)?.[1];
			if (lang !== undefined && lang !== 'css') continue; // scss etc. — not CSS
			const content = dedent(m[2].replace(/^\n+/, '').trimEnd());
			if (content === '') continue;
			const in_snapshot = relative(deps.collections_root, f.path);
			if (in_snapshot.startsWith('..')) {
				// The per-collection grouping below reads the first path segment under the
				// snapshot; a perf-view `.svelte` file from anywhere else would silently
				// land in a `...css` concat — say so instead.
				console.error(
					`svelte_styles_harvest: perf-view .svelte file outside the snapshot: ${f.path}`
				);
				return 1;
			}
			const collection = in_snapshot.split(sep)[0];
			const entry = by_collection.get(collection) ?? { blocks: [], bytes: 0 };
			entry.blocks.push(content);
			entry.bytes += content.length;
			by_collection.set(collection, entry);
			blocks++;
		}
	}

	if (blocks !== SVELTE_STYLES_BLOCKS_PIN) {
		// Name the checkout's tree id beside the pin's: a `../corpora` whose collections
		// are at any other id is skew, not a finding, and the two diagnoses call for
		// opposite actions — unless the perf view itself changed, the third cause.
		const pinned_at = GATE_CHECKOUT_IDS[CORPORA_ROOT].hash;
		const aligned = corpora_tree.startsWith(pinned_at);
		console.error(
			`FAIL: ${blocks} style blocks ≠ pinned ${SVELTE_STYLES_BLOCKS_PIN}; cache not written. ` +
				`${checkout_label(tree_input)} is at ${short_commit(corpora_tree)}, the pin was measured at ${pinned_at}` +
				(aligned
					? ' — same tree, so either the perf view changed (a collection joined the bench: re-pin ' +
						'SVELTE_STYLES_BLOCKS_PIN in lib/gate_counts.ts) or the extraction broke: investigate first.'
					: ' — a snapshot refresh moves this deliberately (re-pin SVELTE_STYLES_BLOCKS_PIN in ' +
						'lib/gate_counts.ts beside the new tree id); otherwise check out the pinned snapshot.')
		);
		return 1;
	}

	await deps.files.mkdir(SVELTE_STYLES_CACHE, { recursive: true });
	let written = 0;
	let unchanged = 0;
	const expected = new Set<string>();
	for (const [collection, entry] of by_collection) {
		const name = `${collection}.css`;
		expected.add(name);
		const banner =
			`/* generated by \`deno task bench:harvest:svelte-styles\` — CSS extracted from ` +
			`${CORPORA_COLLECTIONS}/${collection}'s .svelte <style> blocks (${entry.blocks.length} blocks); do not edit */`;
		const content = banner + '\n\n' + entry.blocks.join('\n\n') + '\n';
		const previous = await deps.styles.read_text_if_exists(name).catch(() => null);
		if (previous === content) {
			unchanged++;
			continue;
		}
		await deps.styles.write(name, content);
		written++;
	}
	// Delete strays so a collection that stopped contributing doesn't linger in the corpus.
	const names: string[] = [];
	for await (const { name } of deps.styles.list()) names.push(name);
	let removed = 0;
	for (const name of names) {
		if (!expected.has(name)) {
			await deps.styles.remove(name);
			removed++;
		}
	}
	await write_stamp(deps.files, stamp.path, inputs);

	const total_bytes = [...by_collection.values()].reduce((s, e) => s + e.bytes, 0);
	console.error(
		`svelte_styles_harvest: ${blocks} style blocks / ${(total_bytes / 1024).toFixed(0)} KB ` +
			`from ${by_collection.size} collections → ${SVELTE_STYLES_CACHE}/ ` +
			`(${written} written, ${unchanged} unchanged${removed > 0 ? `, ${removed} stray removed` : ''}; ` +
			`stamped at ${checkout_label(tree_input)} ${short_commit(corpora_tree)})`
	);
	return 0;
}
