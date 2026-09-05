/**
 * Harvest real-authored CSS for the corpus: extract every component-level
 * `<style>` block from the **perf view's** `.svelte` files and concatenate them
 * per source collection into `benches/js/.cache/svelte_styles/<collection>.css`.
 * That directory is a `real`-tier corpus entry, so the harvest ~3×es the
 * standalone-CSS sample (otherwise ~30 tiny files) with content people actually
 * wrote — and because the blocks are concatenated per collection, file sizes are
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
 * silently). Blocks keep their authored bytes verbatim (including the one-level
 * embedded indent), joined with blank lines under a generated-file banner.
 *
 * Stamped like the suite harvests (`lib/harvest_stamp.ts`): the perf view is the
 * pinned `../corpora` snapshot, so the stamp records its `collections/` tree id, the
 * pinned block count, and the perf view's entry list (a collection joining a perf
 * tier changes the input with no checkout moving), and an unchanged triple
 * skips the walk. The count is an EXACT pin (`SVELTE_STYLES_BLOCKS_PIN`): a move is a
 * snapshot refresh or a view change (re-pin) or a broken extraction, and either fails
 * BEFORE writing so a wrong cache never replaces a good one. `--force` re-harvests
 * despite a fresh stamp (after a change to the extraction itself, which the stamp
 * can't see).
 *
 * Run (from repo root):
 *   deno run --allow-read --allow-write=benches/js/.cache --allow-env \
 *     --allow-sys --allow-run=git --config benches/js/deno.json \
 *     benches/js/diagnostics/svelte_styles_harvest.ts [--force]
 */

import { mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { relative, resolve, sep } from 'node:path';

import { CORPORA_COLLECTIONS, CORPORA_ROOT, CORPORA_URL } from '../lib/corpora.ts';
import { CorpusLoader, corpus_view_paths } from '../lib/corpus.ts';
import { GATE_CHECKOUT_IDS, SVELTE_STYLES_BLOCKS_PIN } from '../lib/gate_counts.ts';
import {
	HARVEST_STAMPS,
	checkout_hash,
	checkout_label,
	harvest_up_to_date,
	short_commit,
	write_stamp
} from '../lib/harvest_stamp.ts';

const CACHE_DIR = 'benches/js/.cache/svelte_styles';
/** The snapshot every perf-view `.svelte` file comes from — the tree the stamp records. */
const collections_root = resolve(CORPORA_COLLECTIONS);

/** Line-anchored component-level style blocks; group 1 = attrs, group 2 = content. */
const STYLE_RE = /^<style([^>]*)>\n([\s\S]*?)^<\/style>/gm;

async function main(): Promise<void> {
	const force = Deno.args.includes('--force');
	const stamp = HARVEST_STAMPS['svelte-styles'];
	const tree_input = stamp.checkouts.corpora_tree;
	const corpora_tree = checkout_hash(tree_input);
	if (corpora_tree === null) {
		console.error(
			`svelte_styles_harvest: ${checkout_label(tree_input)} is not a git checkout tree — ` +
				`the perf view reads the corpora snapshot (git clone ${CORPORA_URL} ${CORPORA_ROOT})`
		);
		Deno.exit(1);
	}
	const inputs = {
		corpora_tree,
		blocks_pin: SVELTE_STYLES_BLOCKS_PIN,
		perf_entries: (await corpus_view_paths('perf')).join(' ')
	};
	if (!force && (await harvest_up_to_date(stamp.path, inputs, [CACHE_DIR]))) {
		console.error(
			`svelte_styles_harvest: up to date (${checkout_label(tree_input)} at ${short_commit(corpora_tree)}, ` +
				`${SVELTE_STYLES_BLOCKS_PIN} blocks, same perf entries) — skipping; --force to re-harvest`
		);
		return;
	}

	// The styles cache itself is a perf-view entry, so on a re-run the loader
	// yields the previous concats too — harmless, the `.svelte` filter drops
	// them (and its absence on a first run is `optional`, a warning). A missing
	// snapshot collection fails fast with the loader's clear error instead of
	// surfacing as a misleading pin trip on a silently smaller harvest.
	const loader = new CorpusLoader('perf');
	const by_collection = new Map<string, { blocks: string[]; bytes: number }>();
	let blocks = 0;
	for await (const f of loader.stream((m) => console.error(m))) {
		if (f.language !== 'svelte' || !f.path.endsWith('.svelte')) continue;
		for (const m of f.content.matchAll(STYLE_RE)) {
			const attrs = m[1];
			const lang = /lang\s*=\s*["']?([\w-]+)/.exec(attrs)?.[1];
			if (lang !== undefined && lang !== 'css') continue; // scss etc. — not CSS
			const content = m[2].replace(/^\n+/, '').trimEnd();
			if (content === '') continue;
			const in_snapshot = relative(collections_root, f.path);
			if (in_snapshot.startsWith('..')) {
				// The per-collection grouping below reads the first path segment under the
				// snapshot; a perf-view `.svelte` file from anywhere else would silently
				// land in a `...css` concat — say so instead.
				console.error(
					`svelte_styles_harvest: perf-view .svelte file outside the snapshot: ${f.path}`
				);
				Deno.exit(1);
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
					? ' — same tree, so either the perf view changed (a collection joined a perf tier: re-pin ' +
						'SVELTE_STYLES_BLOCKS_PIN in lib/gate_counts.ts) or the extraction broke: investigate first.'
					: ' — a snapshot refresh moves this deliberately (re-pin SVELTE_STYLES_BLOCKS_PIN in ' +
						'lib/gate_counts.ts beside the new tree id); otherwise check out the pinned snapshot.')
		);
		Deno.exit(1);
	}

	const out_dir = resolve(CACHE_DIR);
	await mkdir(out_dir, { recursive: true });
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
		const path = resolve(out_dir, name);
		const previous = await readFile(path, 'utf8').catch(() => null);
		if (previous === content) {
			unchanged++;
			continue;
		}
		await writeFile(path, content);
		written++;
	}
	// Delete strays so a collection that stopped contributing doesn't linger in the corpus.
	let removed = 0;
	for (const name of await readdir(out_dir)) {
		if (!expected.has(name)) {
			await rm(resolve(out_dir, name));
			removed++;
		}
	}
	await write_stamp(stamp.path, inputs);

	const total_bytes = [...by_collection.values()].reduce((s, e) => s + e.bytes, 0);
	console.error(
		`svelte_styles_harvest: ${blocks} style blocks / ${(total_bytes / 1024).toFixed(0)} KB ` +
			`from ${by_collection.size} collections → ${relative(resolve('.'), out_dir)}/ ` +
			`(${written} written, ${unchanged} unchanged${removed > 0 ? `, ${removed} stray removed` : ''}; ` +
			`stamped at ${checkout_label(tree_input)} ${short_commit(corpora_tree)})`
	);
}

await main();
