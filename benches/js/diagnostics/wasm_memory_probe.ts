/**
 * Diagnostic: measure WASM **linear-memory high-water** for `format()` — the
 * axis `wasm_format_probe.ts` (wall-time) can't see, and the one that gates all
 * doc-IR memory work (arena/output pre-size, the parked `DocNode` shrink).
 *
 * WASM linear memory only ever grows (`memory.grow`, never shrinks), so
 * `memory.buffer.byteLength` read after a format IS the peak that format drove.
 * The wasm-pack deno glue does not re-export `memory`, so we capture it by
 * monkeypatching `WebAssembly.instantiateStreaming`/`instantiate` before importing
 * the glue (the glue instantiates at import via top-level await). A query-string
 * cache-bust on the glue URL forces a genuinely fresh instance+memory per import,
 * which is how the cold-start mode gets an un-warmed arena per file in-process.
 *
 * Two modes, because the doc-IR memory levers split exactly along them:
 *
 *   --cold  (the lever gate) — a FRESH instance per file, so the thread-local
 *           arena is un-warmed: reserve0 = byteLength before, peak = byteLength
 *           after formatting that one file. `growth = peak - reserve0` is what a
 *           single cold `format()` demands — the regime the arena `2/byte` +
 *           output `nodes*2` pre-sizes target (dlmalloc reserves linear memory
 *           eagerly, so a smaller reservation lowers cold peak). Reports the
 *           per-file peak/growth distribution.
 *
 *   default (steady-state) — ONE warm instance formats the whole corpus; the
 *           final byteLength is the reset()-reuse high-water (bounded by the
 *           largest single file's actual demand, NOT the pre-size hint — the lore
 *           predicts this is ~invariant to the pre-size levers; this mode proves
 *           or refutes that).
 *
 * A/B two builds with --baseline (mirrors wasm_format_probe.ts): each build is a
 * separate module URL → its own instance+memory, so both load in one process.
 *
 *   # A/A (one build — just capture reserve + cold/steady high-water):
 *   deno run --allow-read --allow-env --allow-net --allow-sys \
 *     benches/js/diagnostics/wasm_memory_probe.ts --cold
 *
 *   # A/B (does a pre-size change lower the cold peak?):
 *   cp -r crates/tsv_wasm/pkg/all/deno crates/tsv_wasm/pkg/all/deno.baseline
 *   # ...edit pre-size, deno task build:wasm:all:deno...
 *   deno run --allow-read --allow-env --allow-net --allow-sys \
 *     benches/js/diagnostics/wasm_memory_probe.ts --cold \
 *     --baseline crates/tsv_wasm/pkg/all/deno.baseline/tsv_wasm.js
 *
 * Human output → stderr; --json → a clean object on stdout.
 */

import { resolve } from 'node:path';

import { DirectoryLoader, group_by_language } from '../lib/corpus.ts';
import { LANGUAGES } from '../lib/types.ts';
import type { Language, SourceFile } from '../lib/types.ts';

const DEFAULT_CURRENT = 'crates/tsv_wasm/pkg/all/deno/tsv_wasm.js';
const DEFAULT_CORPUS = '../zzz/src/lib';
const PAGE = 65536;

// ---- arg parsing (positional corpus dir + flags), mirrors wasm_format_probe ----
let corpus_dir = DEFAULT_CORPUS;
let current_path = DEFAULT_CURRENT;
let baseline_path: string | null = null;
let lang_filter: Language | null = null;
let limit = Infinity;
let cold = false;
let json = false;
const argv = Deno.args;
for (let i = 0; i < argv.length; i++) {
	const a = argv[i];
	if (a === '--baseline') baseline_path = argv[++i];
	else if (a === '--current') current_path = argv[++i];
	else if (a === '--lang') lang_filter = argv[++i] as Language;
	else if (a === '--limit') limit = Number(argv[++i]);
	else if (a === '--cold') cold = true;
	else if (a === '--json') json = true;
	else if (!a.startsWith('--')) corpus_dir = a;
}
const langs: Language[] = lang_filter ? [lang_filter] : LANGUAGES;

// ---- capture wasm Memory objects as the glue instantiates them ----
const captured: WebAssembly.Memory[] = [];
function patch<K extends 'instantiate' | 'instantiateStreaming'>(key: K): void {
	// deno-lint-ignore no-explicit-any
	const W = WebAssembly as any;
	const orig = W[key].bind(WebAssembly);
	// deno-lint-ignore no-explicit-any
	W[key] = async (src: any, imports: any) => {
		const r = await orig(src, imports);
		const mem = (r.instance?.exports ?? r.exports)?.memory;
		if (mem instanceof WebAssembly.Memory) captured.push(mem);
		return r;
	};
}
patch('instantiateStreaming');
patch('instantiate');

type FormatBuild = Record<Language, (source: string) => string>;
interface Loaded {
	build: FormatBuild;
	mem: WebAssembly.Memory;
}

let bust = 0;
/** Import a glue build fresh (cache-busted) and pair it with its captured memory. */
async function load_fresh(js_path: string): Promise<Loaded> {
	const before = captured.length;
	const url = `file://${resolve(js_path)}?mem=${bust++}`;
	const mod = await import(url);
	if (typeof mod.default === 'function') await mod.default();
	const mem = captured[captured.length - 1];
	if (captured.length === before || !mem) throw new Error(`no memory captured for ${js_path}`);
	return {
		build: { svelte: mod.format_svelte, typescript: mod.format_typescript, css: mod.format_css },
		mem,
	};
}

const now_bytes = (m: WebAssembly.Memory): number => m.buffer.byteLength;
const mb = (b: number): string => (b / 1e6).toFixed(2);
const pages = (b: number): number => b / PAGE;
const pct = (n: number, d: number): string => ((n / d - 1) * 100).toFixed(2);

// ---- load corpus ----
const files = (await new DirectoryLoader(corpus_dir).load((m) => console.error(m)))
	.filter((f) => langs.includes(f.language));
const by_lang = group_by_language(files);
const scoped: SourceFile[] = [];
for (const lang of langs) scoped.push(...by_lang[lang].slice(0, limit === Infinity ? undefined : limit));
scoped.sort((a, b) => a.bytes - b.bytes);

const builds: { label: string; path: string }[] = [{ label: 'current', path: current_path }];
if (baseline_path) builds.push({ label: 'baseline', path: baseline_path });

const percentile = (xs: number[], p: number): number => {
	if (xs.length === 0) return 0;
	const s = [...xs].sort((a, b) => a - b);
	return s[Math.min(s.length - 1, Math.floor(p * s.length))];
};

// Byte-identity gate (A/B only): a memory A/B is meaningful only when both builds
// format identically — a pre-size/no-behavior-change edit must be byte-identical, else
// the peak delta is an output difference, not a reservation effect. Mirrors
// wasm_format_probe.ts. One warm instance each (byte-identity is instance-independent).
if (baseline_path) {
	const cur = await load_fresh(current_path);
	const base = await load_fresh(baseline_path);
	let mismatches = 0;
	for (const f of scoped) {
		try {
			if (cur.build[f.language](f.content) !== base.build[f.language](f.content)) mismatches++;
		} catch {
			// skip files either build rejects — measurement loops skip them too
		}
	}
	if (mismatches) {
		console.error(
			`ABORT: builds format differently on ${mismatches}/${scoped.length} files — ` +
				`a memory A/B is only meaningful when output is byte-identical.`,
		);
		Deno.exit(1);
	}
	console.error(`byte-identity: ${scoped.length}/${scoped.length} identical`);
}

console.error(
	`\nWASM memory probe · ${scoped.length} files · ${cold ? 'COLD (fresh instance/file)' : 'steady-state (one warm instance)'}` +
		`${baseline_path ? ' · A/B' : ' · A/A'}\n`,
);

// deno-lint-ignore no-explicit-any
const report: Record<string, any> = { mode: cold ? 'cold' : 'steady', files: scoped.length, builds: {} };

if (cold) {
	// Fresh instance per file: reserve0 (pre-format) → peak (post-format), growth = peak-reserve0.
	for (const b of builds) {
		const reserves: number[] = [];
		const peaks: number[] = [];
		const growths: number[] = [];
		let worst: { path: string; peak: number } = { path: '', peak: 0 };
		for (const f of scoped) {
			let ld: Loaded;
			try {
				ld = await load_fresh(b.path);
			} catch (e) {
				console.error(`load failed (${b.path}): ${(e as Error).message}`);
				break;
			}
			const reserve0 = now_bytes(ld.mem);
			try {
				ld.build[f.language](f.content);
			} catch {
				continue; // skip files this build rejects
			}
			const peak = now_bytes(ld.mem);
			reserves.push(reserve0);
			peaks.push(peak);
			growths.push(peak - reserve0);
			if (peak > worst.peak) worst = { path: f.path, peak };
		}
		const n = peaks.length;
		report.builds[b.label] = {
			reserve_bytes: reserves[0] ?? 0,
			peak_p50: percentile(peaks, 0.5),
			peak_p90: percentile(peaks, 0.9),
			peak_p99: percentile(peaks, 0.99),
			peak_max: Math.max(0, ...peaks),
			growth_p50: percentile(growths, 0.5),
			growth_p90: percentile(growths, 0.9),
			growth_max: Math.max(0, ...growths),
			worst_file: worst.path,
			n,
		};
		const r = report.builds[b.label];
		console.error(
			`${b.label.padStart(9)}: reserve ${mb(r.reserve_bytes)}MB (${pages(r.reserve_bytes)}p) · ` +
				`cold peak p50 ${mb(r.peak_p50)} / p90 ${mb(r.peak_p90)} / p99 ${mb(r.peak_p99)} / max ${mb(r.peak_max)}MB · ` +
				`growth p50 ${mb(r.growth_p50)} / max ${mb(r.growth_max)}MB (n=${n})`,
		);
	}
	if (baseline_path) {
		const c = report.builds.current, bl = report.builds.baseline;
		console.error(
			`\nA/B (current vs baseline): reserve ${pct(c.reserve_bytes, bl.reserve_bytes)}% · ` +
				`peak_p90 ${pct(c.peak_p90, bl.peak_p90)}% · peak_max ${pct(c.peak_max, bl.peak_max)}% ` +
				`(negative = current uses less)`,
		);
	}
} else {
	// One warm instance formats the whole corpus; final byteLength = reset-reuse high-water.
	for (const b of builds) {
		let ld: Loaded;
		try {
			ld = await load_fresh(b.path);
		} catch (e) {
			console.error(`load failed (${b.path}): ${(e as Error).message}`);
			continue;
		}
		const reserve = now_bytes(ld.mem);
		let high = reserve;
		let processed = 0;
		for (const f of scoped) {
			try {
				ld.build[f.language](f.content);
			} catch {
				continue;
			}
			processed++;
			const cur = now_bytes(ld.mem);
			if (cur > high) high = cur;
		}
		report.builds[b.label] = {
			reserve_bytes: reserve,
			high_water_bytes: high,
			total_growth_bytes: high - reserve,
			processed,
		};
		const r = report.builds[b.label];
		console.error(
			`${b.label.padStart(9)}: reserve ${mb(r.reserve_bytes)}MB (${pages(r.reserve_bytes)}p) · ` +
				`steady high-water ${mb(r.high_water_bytes)}MB (${pages(r.high_water_bytes)}p) · ` +
				`corpus growth ${mb(r.total_growth_bytes)}MB (${processed} files)`,
		);
	}
	if (baseline_path) {
		const c = report.builds.current, bl = report.builds.baseline;
		console.error(
			`\nA/B (current vs baseline): reserve ${pct(c.reserve_bytes, bl.reserve_bytes)}% · ` +
				`high-water ${pct(c.high_water_bytes, bl.high_water_bytes)}% (negative = current uses less)`,
		);
	}
}

if (json) console.log(JSON.stringify(report, null, 2));
