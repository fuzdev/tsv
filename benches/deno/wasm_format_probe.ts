/**
 * Diagnostic: measure WASM format wall-time with enough resolution to see the
 * single-digit-% moves that `deno task bench` folds into run noise.
 *
 * The full bench times one impl per file once and reports cross-tool throughput
 * ratios — too coarse for "did killing this allocation site move WASM format
 * time?". This probe A/Bs two WASM builds with the discipline from
 * docs/performance.md §5 (the LD_PRELOAD allocator probe):
 *
 *   - interleaved pairs, alternating which build runs first so machine drift
 *     and JIT/GC warmup cancel within each pair
 *   - compare PAIR MEDIANS of the per-pass ratio, never absolute readings
 *   - measure the A/A noise floor in the SAME run (current vs itself, or vs a
 *     separate identical-code copy via --control). A floor from a separate
 *     invocation is untrustworthy: a rebuild between runs shifts CPU
 *     frequency/thermals by ~10%, dwarfing a ~1% signal. `net` (A/B ÷ floor) is
 *     the drift-corrected effect; the `[min,max]` column shows the A/B spread so
 *     a noisy median is visible rather than hidden.
 *
 * Also a corpus byte-identity gate: a no-behavior-change perf edit must format
 * every file identically across the builds — a mismatch aborts (exit 1), since
 * timing a build that changed output is meaningless.
 *
 * A/B workflow (mirrors how T2.7 A/B'd the native CLI — copy the artifact aside
 * before editing; pkg/ is gitignored):
 *
 *   cp -r crates/tsv_wasm/pkg/all/deno crates/tsv_wasm/pkg/all/deno.baseline
 *   # ... make the source change, then rebuild:
 *   deno task build:wasm:all:deno
 *   deno run --allow-read --allow-env --allow-net --allow-sys \
 *     benches/deno/wasm_format_probe.ts \
 *     --baseline crates/tsv_wasm/pkg/all/deno.baseline/tsv_wasm.js
 *
 * Add --control <copy-of-the-rebuilt-artifact>/tsv_wasm.js for a two-instance
 * floor instead of the default same-instance one.
 *
 * A/A only (no source change — just capture the baseline number + floor):
 *
 *   deno run --allow-read --allow-env --allow-net --allow-sys \
 *     benches/deno/wasm_format_probe.ts
 *
 * Defaults to the zzz corpus the native profiling tools use, for comparability.
 * Human output goes to stderr; stdout stays clean for a future --json.
 */

import { resolve } from 'node:path';

import { DirectoryLoader, group_by_language } from './lib/corpus.ts';
import { LANGUAGES } from './lib/types.ts';
import type { Language, SourceFile } from './lib/types.ts';

/** A loaded WASM build: the three format entry points, keyed by language. */
type FormatBuild = Record<Language, (source: string) => string>;
/** Per-language sample arrays (one entry per pair). */
type LangSamples = Record<Language, number[]>;

const DEFAULT_CURRENT = 'crates/tsv_wasm/pkg/all/deno/tsv_wasm.js';
const DEFAULT_CORPUS = '../zzz/src/lib';

// minimal arg parsing — positional corpus dir + a few flags
let corpus_dir = DEFAULT_CORPUS;
let current_path = DEFAULT_CURRENT;
let baseline_path: string | null = null;
let control_path: string | null = null;
let pairs = 12;
let warmup = 3;
let lang_filter: Language | null = null;
const argv = Deno.args;
for (let i = 0; i < argv.length; i++) {
	const a = argv[i];
	if (a === '--baseline') baseline_path = argv[++i];
	else if (a === '--current') current_path = argv[++i];
	else if (a === '--control') control_path = argv[++i];
	else if (a === '--pairs') pairs = Number(argv[++i]);
	else if (a === '--warmup') warmup = Number(argv[++i]);
	else if (a === '--lang') lang_filter = argv[++i] as Language;
	else if (!a.startsWith('--')) corpus_dir = a;
}
const langs: Language[] = lang_filter ? [lang_filter] : LANGUAGES;

/** Load a wasm-pack deno build and expose its format entry points. */
async function load_build(js_path: string): Promise<FormatBuild> {
	const mod = await import(resolve(js_path));
	if (typeof mod.default === 'function') await mod.default();
	return {
		svelte: mod.format_svelte,
		typescript: mod.format_typescript,
		css: mod.format_css,
	};
}

const current = await load_build(current_path);
const baseline = baseline_path ? await load_build(baseline_path) : null;
const control = control_path ? await load_build(control_path) : null;

const files = await new DirectoryLoader(corpus_dir).load((m) => console.error(m));

// Intersection: keep only files every build formats without throwing, and gate
// byte-identity across them (the .control copy is identical by construction; the
// .baseline is the real check). Timing past a mismatch would be meaningless.
const builds = [current, baseline, control].filter((b): b is FormatBuild => b !== null);
const ok: SourceFile[] = [];
let mismatches = 0;
for (const f of files) {
	if (!langs.includes(f.language)) continue;
	try {
		const outs = builds.map((b) => b[f.language](f.content));
		if (outs.some((o) => o !== outs[0])) mismatches++;
		ok.push(f);
	} catch {
		// skip files any build rejects
	}
}
console.error(
	`\nbyte-identity: ${ok.length - mismatches}/${ok.length} identical` +
		(mismatches ? `  — ${mismatches} MISMATCH` : ''),
);
if (mismatches && baseline) {
	console.error('ABORT: builds format differently — a no-behavior-change edit must be byte-identical.');
	Deno.exit(1);
}
const by_lang = group_by_language(ok);

const time = (fn: () => void): number => {
	const t0 = performance.now();
	fn();
	return performance.now() - t0;
};

/** Time one full pass over the corpus for a build, split by language. */
const pass = (build: FormatBuild): Record<Language, number> => {
	const out = { svelte: 0, typescript: 0, css: 0 } as Record<Language, number>;
	for (const lang of langs) {
		const fn = build[lang];
		let t = 0;
		for (const f of by_lang[lang]) t += time(() => void fn(f.content));
		out[lang] = t;
	}
	return out;
};

const median = (xs: number[]): number => {
	const s = [...xs].sort((a, b) => a - b);
	const m = Math.floor(s.length / 2);
	return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};

// References to time `current` against, each in the same interleaved pair loop
// so every ratio shares one machine state. 'ab' = the change's effect; 'aa' =
// the noise floor (a separate identical-code instance if --control, else
// current itself — same-instance captures pass-to-pass noise and misses only
// second-order cross-instance layout effects).
const refs: { key: 'ab' | 'aa'; build: FormatBuild }[] = [];
if (baseline) refs.push({ key: 'ab', build: baseline });
refs.push({ key: 'aa', build: control ?? current });

const blank = (): LangSamples => ({ svelte: [], typescript: [], css: [] });
const cur_t: Record<string, LangSamples> = {};
const ref_t: Record<string, LangSamples> = {};
for (const r of refs) {
	cur_t[r.key] = blank();
	ref_t[r.key] = blank();
}

console.error(
	`Probing ${ok.length} files × ${pairs} pairs` +
		`${baseline ? ' (A/B + floor)' : ' (A/A floor only)'}, warmup ${warmup}\n`,
);

for (let w = 0; w < warmup; w++) {
	pass(current);
	for (const r of refs) pass(r.build);
}

for (let p = 0; p < pairs; p++) {
	for (const r of refs) {
		let c: Record<Language, number>;
		let rr: Record<Language, number>;
		// alternate which build runs first so within-pair drift cancels
		if (p % 2 === 0) {
			c = pass(current);
			rr = pass(r.build);
		} else {
			rr = pass(r.build);
			c = pass(current);
		}
		for (const lang of langs) {
			cur_t[r.key][lang].push(c[lang]);
			ref_t[r.key][lang].push(rr[lang]);
		}
	}
}

// per-pair current/ref ratios; for totals, sum languages within each pair first
const ratios = (key: string, lang: Language): number[] =>
	cur_t[key][lang].map((c, i) => c / ref_t[key][lang][i]);
const total_ratios = (key: string): number[] =>
	Array.from(
		{ length: pairs },
		(_, p) =>
			langs.reduce((s, l) => s + cur_t[key][l][p], 0) /
			langs.reduce((s, l) => s + ref_t[key][l][p], 0),
	);

const primary = baseline ? 'ab' : 'aa'; // which ref's current samples to report as cur(ms)
const kb_of = (lang: Language): number =>
	by_lang[lang].reduce((sum, f) => sum + f.bytes, 0) / 1024;
const pad = (s: string, n: number): string => s.padStart(n);

const print_row = (label: string, files_n: number, kb: number, cur_ms: number, ab: number[], aa: number[]): void => {
	let line = `${pad(label, 11)} ${pad(String(files_n), 4)}f ${pad(kb.toFixed(1), 8)}KB  ` +
		`cur ${pad(cur_ms.toFixed(2), 8)}ms ${pad((cur_ms / kb * 1000).toFixed(1), 7)}us/KB  ` +
		`floor ${median(aa).toFixed(4)}`;
	if (baseline) {
		const ab_m = median(ab);
		line += `  A/B ${ab_m.toFixed(4)}  net ${(ab_m / median(aa)).toFixed(4)}  ` +
			`[${Math.min(...ab).toFixed(3)},${Math.max(...ab).toFixed(3)}]`;
	}
	console.error(line);
};

console.error(
	baseline
		? `A/B = current/baseline pair-median (<1 = faster) · net = A/B÷floor · [min,max] = A/B spread\n`
		: `floor = current/current pair-median (noise floor; expect ~1.00)\n`,
);
for (const lang of langs) {
	if (by_lang[lang].length === 0) continue;
	print_row(lang, by_lang[lang].length, kb_of(lang), median(cur_t[primary][lang]), ratios('ab', lang), ratios('aa', lang));
}
const total_cur = median(Array.from({ length: pairs }, (_, p) => langs.reduce((s, l) => s + cur_t[primary][l][p], 0)));
console.error('');
print_row('total', ok.length, langs.reduce((s, l) => s + kb_of(l), 0), total_cur, total_ratios('ab'), total_ratios('aa'));
