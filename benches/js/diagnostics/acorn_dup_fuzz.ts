/**
 * Comment-duplication fuzz over acorn-typescript's own construct corpus (diagnostic;
 * ad-hoc, not wired into `deno task`).
 *
 * Uses acorn-typescript's ~200 test inputs as a construct catalog and fuzzes a block
 * comment into EVERY position of each, then checks whether acorn's `onComment` fires the
 * same span >=2x (the backtrack-reparse duplication signature). This is the broadest net
 * for "a duplicating construct nobody enumerated" — acorn's own tests barely use comments
 * (they assert AST shape), so the as-is corpus is a near-empty dup net; the position fuzz
 * makes it exhaustive. Pinned acorn 8.16.0 + @sveltejs/acorn-typescript 1.0.10 (tsv's set).
 *
 * Two uses:
 *   1. Completeness — confirm the duplicating-construct set tsv corrects is the whole set
 *      (cross-checks the deep-dive's mechanism enumeration against acorn's full catalog).
 *   2. Upstream-fix validation — after applying the comment-dedup patch to a patched
 *      acorn-typescript build, re-run pointing at that build; a correct fix drops the
 *      double-fire count to 0.
 *
 * Run (default reads ../test of a sibling acorn-typescript checkout; pass a path to override):
 *   deno run --allow-read --allow-env --allow-net --allow-sys \
 *     --config benches/js/deno.json benches/js/diagnostics/acorn_dup_fuzz.ts [TEST_DIR]
 */

import { readdir, stat } from 'node:fs/promises';
import * as acorn from 'acorn';
import { tsPlugin } from '@sveltejs/acorn-typescript';

// deno-lint-ignore no-explicit-any
const P = acorn.Parser.extend(tsPlugin() as any);

// default: a sibling acorn-typescript checkout next to this repo; override via argv[0]
const TEST_ROOT = Deno.args[0] ??
	new URL('../../../../acorn-typescript/test', import.meta.url).pathname;
const COMMENT = '/*Z*/';

/** Parse `src`; return a span string if any comment span is emitted >=2x, else null; null on parse error. */
function dup_span(src: string): string | null {
	const seen = new Map<string, number>();
	let hit: string | null = null;
	try {
		P.parse(src, {
			sourceType: 'module',
			ecmaVersion: 2025,
			locations: true,
			// deno-lint-ignore no-explicit-any
			onComment: ((_b: boolean, _t: string, s: number, e: number) => {
				const k = `${s},${e}`;
				const n = (seen.get(k) ?? 0) + 1;
				seen.set(k, n);
				if (n >= 2) hit = k;
			}) as any,
		});
	} catch {
		return null; // insertion broke the parse — skip
	}
	return hit;
}

/** Collapse `<cat>_type_test_<case>` / `<cat>_<case>` to a coarse construct family. */
function category(name: string): string {
	const m = name.match(/^(.*?_type)_test_/) ?? name.match(/^([a-z]+(?:-[a-z]+)?)/);
	return m ? m[1] : name;
}

let scanned = 0, parseable = 0, dup_as_is = 0;
const dup_hits: { name: string; cat: string; pos: number; sample: string }[] = [];

for (const rel of await readdir(TEST_ROOT, { recursive: true })) {
	if (!rel.endsWith('input.ts')) continue;
	const path = `${TEST_ROOT}/${rel}`;
	if (!(await stat(path)).isFile()) continue;
	scanned++;
	const src = await Deno.readTextFile(path);
	const name = path.split('/test/')[1].replace(/\/input\.ts$/, '');
	if (dup_span(src) !== null) dup_as_is++; // baseline: dups using the file's own comments
	try {
		P.parse(src, { sourceType: 'module', ecmaVersion: 2025, locations: true });
		parseable++;
	} catch {
		continue; // intentional parse-error test, or syntax acorn-ts rejects
	}
	let found: { pos: number; sample: string } | null = null;
	for (let i = 1; i < src.length && !found; i++) {
		const variant = src.slice(0, i) + COMMENT + src.slice(i);
		if (dup_span(variant) !== null) found = { pos: i, sample: variant.replace(/\n/g, '\\n') };
	}
	if (found) dup_hits.push({ name, cat: category(name), pos: found.pos, sample: found.sample });
}

console.log(`test root:                     ${TEST_ROOT}`);
console.log(`acorn test inputs scanned:     ${scanned}`);
console.log(`bare-input parseable:          ${parseable}`);
console.log(`dup with file's own comments:  ${dup_as_is}`);
console.log(`inputs that double-fire under comment fuzz: ${dup_hits.length}\n`);

const by_cat = new Map<string, number>();
for (const h of dup_hits) by_cat.set(h.cat, (by_cat.get(h.cat) ?? 0) + 1);
console.log('duplicating construct families (acorn test categories):');
for (const [c, n] of [...by_cat].sort((a, b) => b[1] - a[1])) console.log(`  ${c.padEnd(28)} ${n}`);

console.log('\nsample duplicating input (one per family):');
const shown = new Set<string>();
for (const h of dup_hits) {
	if (shown.has(h.cat)) continue;
	shown.add(h.cat);
	console.log(`  [${h.cat}] ${h.name}`);
	console.log(`      ${h.sample}`);
}
