/**
 * Diagnostic: is it faster to (A) get the full loc-bearing wire and materialize
 * it in Rust, or (B) get the smaller `no-locations` wire and reconstruct `loc` in
 * JS? The perf sibling of `no_locations_parity.ts` (which proves the
 * reconstruction is correct); this measures whether it's the faster path.
 *
 * For each corpus file it times three end-to-end paths (FFI call → fully
 * materialized object), reporting sum-of-medians and the A/B, A/B' ratios:
 * - **A** — full wire, `loc` materialized in Rust (`native.parse`).
 * - **B** — `no-locations` wire + reconstruct ALL `loc` in JS. This DOGFOODS the
 *   shipped `create_locator(...).reconstruct(...)` helper (`crates/tsv_wasm/npm/
 *   locations.js`). Like the probe, the line-start table is built once per file
 *   (via `create_locator`) and reused across timed iterations, so B is the
 *   "consumer holds a locator" model — reconstruct cost, not table-build cost.
 * - **B'** — `no-locations` wire, parsed only (the ceiling for a loc-sparse /
 *   loc-free consumer that reconstructs lazily or not at all).
 *
 * The finding (measured on real TS files): B beats A even when you need ALL `loc`
 * — the full wire's `loc` bytes (~46%) cost real `JSON.parse` tokenization, while
 * a line-start table + binary-search lookup is cheaper. So pre-materializing `loc`
 * in Rust is NOT optimal for a JS consumer.
 *
 * TypeScript reconstruction is EXACT (pure offset → line/col); Svelte is
 * APPROXIMATE (the `<script>` `Program` tag-position override + destructure
 * `+1`-column quirk + dropped `name_loc` — see `no_locations_parity.ts`), so its
 * B path is still a valid *perf* measurement but its output isn't Svelte's exact
 * `loc`. Both are reported; the headline "reconstruct beats materialize" number is
 * the exact TS one.
 *
 * Corpus: the `perf` view (real-world code), TS + Svelte. `BENCH_LIMIT` caps files
 * per language (default below); `BENCH_FILTER` filters by path substring.
 *
 * Run: deno run --allow-ffi --allow-read --allow-env --allow-net --allow-sys \
 *   benches/js/diagnostics/reconstruct_vs_materialize.ts 2>&1 | tail -20
 */

import { create_locator } from '../../../crates/tsv_wasm/npm/locations.js';
import { DevReposLoader, group_by_language } from '../lib/corpus.ts';
import { init_implementations } from '../lib/implementations.ts';
import type { Language } from '../lib/types.ts';

/** Files per language (default). `BENCH_LIMIT` overrides. Kept modest so a manual
 * run finishes in a minute or two — each file times three calibrated blocks. */
const DEFAULT_LIMIT = 40;
const limit = Number(Deno.env.get('BENCH_LIMIT') ?? DEFAULT_LIMIT);
const filter = Deno.env.get('BENCH_FILTER') ?? '';

/**
 * Median per-call time (ms) for `fn`, via the probe's calibrate-then-sample loop:
 * warm up, grow an inner iteration count until a block spans ~`block_ms`, then
 * take the median of `samples` such blocks. Amortizes per-call timer overhead.
 */
function bench(fn: () => void, samples = 21, block_ms = 10): number {
	for (let i = 0; i < 5; i++) fn();
	let inner = 1;
	for (;;) {
		const t0 = performance.now();
		for (let i = 0; i < inner; i++) fn();
		const dt = performance.now() - t0;
		if (dt >= block_ms || inner >= 1 << 22) break;
		inner = Math.max(inner * 2, Math.ceil((inner * block_ms) / Math.max(dt, 0.001)));
	}
	const ts: number[] = [];
	for (let s = 0; s < samples; s++) {
		const t0 = performance.now();
		for (let i = 0; i < inner; i++) fn();
		ts.push((performance.now() - t0) / inner);
	}
	ts.sort((a, b) => a - b);
	return ts[ts.length >> 1];
}

const impls = await init_implementations({ logger: (m) => console.error(m) });
if (!impls.native) throw new Error('native FFI not built — run deno task build:ffi');
const native = impls.native;
if (!native.parse_no_locations) throw new Error('native.parse_no_locations unavailable');

const files = await new DevReposLoader('perf').load((m) => console.error(m));
const by_lang = group_by_language(files);

interface Totals {
	files: number;
	a: number;
	b: number;
	b_prime: number;
}

const results: Partial<Record<Language, Totals>> = {};

for (const language of ['typescript', 'svelte'] as Language[]) {
	let selected = by_lang[language] ?? [];
	if (filter) selected = selected.filter((f) => f.path.includes(filter));
	selected = selected.slice(0, limit);

	const t: Totals = { files: 0, a: 0, b: 0, b_prime: 0 };
	let sanity_done = false;
	for (const f of selected) {
		const src = f.content;
		// Skip files either path rejects (keep A and B measuring the same set).
		try {
			native.parse(src, language);
			native.parse_no_locations(src, language);
		} catch {
			continue;
		}

		// One locator per file (line table built once) — the "consumer holds a
		// locator" model, matching the probe's prebuilt-table B path.
		const locator = create_locator(src, { language });

		// Sanity (first file per language): reconstruct must actually add `loc`.
		if (!sanity_done) {
			const probe = native.parse_no_locations(src, language) as Record<string, unknown>;
			locator.reconstruct(probe);
			if (!probe.loc) throw new Error(`reconstruct added no loc for ${language} (${f.path})`);
			sanity_done = true;
		}

		const t_a = bench(() => {
			native.parse(src, language);
		});
		const t_b = bench(() => {
			locator.reconstruct(native.parse_no_locations!(src, language));
		});
		const t_bp = bench(() => {
			native.parse_no_locations!(src, language);
		});
		t.a += t_a;
		t.b += t_b;
		t.b_prime += t_bp;
		t.files++;
		console.error(
			`  ${f.path.split('/').slice(-2).join('/')}: A ${t_a.toFixed(3)} B ${t_b.toFixed(3)} B' ${
				t_bp.toFixed(3)
			} ms`,
		);
	}
	results[language] = t;
}

native.dispose();

const ratio = (num: number, den: number) => (den > 0 ? (num / den).toFixed(2) : 'n/a');

console.log('\n=== reconstruct-vs-materialize (Deno FFI + materialize), sum of medians (ms) ===');
for (const language of ['typescript', 'svelte'] as Language[]) {
	const t = results[language];
	if (!t || t.files === 0) continue;
	const exactness = language === 'typescript'
		? 'EXACT reconstruction'
		: 'APPROXIMATE — omits name_loc + 2 quirks; see no_locations_parity.ts';
	console.log(`\n${language} (${exactness}), ${t.files} files:`);
	console.log(`  A  full wire, loc materialized in Rust     : ${t.a.toFixed(2)}`);
	console.log(
		`  B  no-loc wire + reconstruct ALL loc in JS  : ${t.b.toFixed(2)}  (${
			ratio(t.a, t.b)
		}x vs A; >1 = B faster)`,
	);
	console.log(
		`  B' no-loc wire, no reconstruction (sparse)  : ${t.b_prime.toFixed(2)}  (${
			ratio(t.a, t.b_prime)
		}x vs A)`,
	);
}
console.log(
	'\nB vs A = "reconstruct-all in JS" vs "materialize in Rust"; B\' vs A = the loc-sparse/free ceiling.',
);
console.log('The headline (reconstruct beats materialize) is the EXACT TypeScript row.');
