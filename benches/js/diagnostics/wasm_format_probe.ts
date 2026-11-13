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
 * every file identically across the builds, a `--control` copy's included — a mismatch aborts
 * (exit 1) before anything is timed, since timing a build that changed output is meaningless,
 * and a file one build throws on can't be timed at all. Outcomes compare by their bytes
 * (`compare_format_outcomes`): a source every build refuses, or every build throws on with the
 * same error, is skipped, untimed; a refusal or a throw on one side only is a mismatch, listed
 * with the file and what each build did. A crash in any build (`is_tsv_crash`: a trap) fails
 * the run whatever the others did, listed with the build and the file: no build's timing over
 * a source tsv crashes on can be read.
 *
 * A/B workflow (mirrors how T2.7 A/B'd the native CLI — copy the artifact aside
 * before editing; pkg/ is gitignored):
 *
 *   cp -r crates/tsv_wasm/pkg/all/nodejs crates/tsv_wasm/pkg/all/nodejs.baseline
 *   # ... make the source change, then rebuild:
 *   npm run build:wasm:all:nodejs
 *   node benches/js/roots/wasm_format_probe.ts \
 *     --baseline crates/tsv_wasm/pkg/all/nodejs.baseline/tsv_wasm.js
 *
 * Add --control <copy-of-the-rebuilt-artifact>/tsv_wasm.js for a two-instance
 * floor instead of the default same-instance one.
 *
 * A/A only (no source change — just capture the baseline number + floor):
 *
 *   node benches/js/roots/wasm_format_probe.ts
 *
 * Defaults to the zzz corpus the native profiling tools use, for comparability.
 * Human output goes to stderr; stdout stays clean for a future --json.
 * `run_wasm_format_probe` is the library function; its root,
 * `benches/js/roots/wasm_format_probe.ts`, binds its deps (`WasmFormatProbeDeps`).
 *
 * @module
 */

import { CORPORA_COLLECTIONS } from '../lib/corpora.ts';
import { group_by_language } from '../lib/corpus.ts';
import {
	crash_text,
	is_tsv_crash,
	type PanicEntry,
	panic_report_lines
} from '../lib/divergence/panic_errors.ts';
import { first_line } from '../lib/error_text.ts';
import { LANGUAGES } from '../lib/types.ts';
import type { Language, Logger, SourceFile } from '../lib/types.ts';

/**
 * A loaded WASM build: the three format entry points, keyed by language. Each returns the
 * formatted text, or in its place the refusal the glue builds (`{ok: false, error}`), a fresh
 * object per call — which is why outcomes are compared by `compare_format_outcomes`, never by
 * identity.
 */
type FormatBuild = Record<Language, (source: string) => unknown>;
/** Per-language sample arrays (one entry per pair). */
type LangSamples = Record<Language, number[]>;

/** The `nodejs` bundle the bench's WASM rows load, the build a run times by default. */
export const DEFAULT_WASM_GLUE = 'crates/tsv_wasm/pkg/all/nodejs/tsv_wasm.js';
const DEFAULT_CORPUS = `${CORPORA_COLLECTIONS}/zzz/src/lib`;

/** A wasm-pack glue module's exports, of which the probes read the three `format_*`. */
export type WasmGlueExports = Record<string, unknown>;

/** What `run_wasm_format_probe` reaches the world through. */
export interface WasmFormatProbeDeps {
	/** The exports of the wasm-pack `nodejs` glue at `path`, loaded and initialized. */
	load_glue(path: string): Promise<WasmGlueExports>;
	/** The files beneath `path`. */
	load_files(path: string, logger: Logger): Promise<SourceFile[]>;
}

/** A glue module's format entry points, keyed by language. */
export const to_format_build = (mod: WasmGlueExports): FormatBuild =>
	({
		svelte: mod.format_svelte,
		typescript: mod.format_typescript,
		css: mod.format_css
	}) as FormatBuild;

/** What a build threw on a source, held as its outcome (`format_outcome`). */
export class FormatThrew {
	readonly error: unknown;
	constructor(error: unknown) {
		this.error = error;
	}
}

/**
 * One build's outcome on one source: the formatted text, the refusal the glue returns, or
 * what it threw (a trap, an argument refusal), held as a `FormatThrew`.
 */
export function format_outcome(build: FormatBuild, file: SourceFile): unknown {
	try {
		return build[file.language](file.content);
	} catch (e) {
		return new FormatThrew(e);
	}
}

/** An outcome as the A/B reads it: what ended the call, and for a throw, its first line. */
function describe_outcome(outcome: unknown): string {
	if (typeof outcome === 'string') return 'formatted';
	if (!(outcome instanceof FormatThrew)) return 'refused';
	return is_tsv_crash(outcome.error)
		? `crashed (${crash_text(outcome.error)})`
		: `threw (${first_line(outcome.error)})`;
}

/**
 * How the builds' outcomes on one source compare, by their bytes: `crash` when any build
 * crashed on it (`is_tsv_crash`), whatever the others did; `skipped` when every build refused
 * it, or every build threw the same error on it (the file is untimed); `identical` when every
 * build formatted it to the same text; and `mismatch` otherwise — differing text, or builds
 * that ended differently (one formats, refuses, or throws where another doesn't), which is a
 * behavior change too.
 */
export function compare_format_outcomes(
	outcomes: ReadonlyArray<unknown>
): 'identical' | 'mismatch' | 'skipped' | 'crash' {
	const ends = outcomes.map(describe_outcome);
	if (outcomes.some((o) => o instanceof FormatThrew && is_tsv_crash(o.error))) return 'crash';
	if (ends.some((e) => e !== ends[0])) return 'mismatch';
	if (ends[0] !== 'formatted') return 'skipped';
	return outcomes.every((o) => o === outcomes[0]) ? 'identical' : 'mismatch';
}

/** The A/B's line for a source the builds disagree on: the file, then what each build did. */
export function format_mismatch_line(
	file: SourceFile,
	labels: ReadonlyArray<string>,
	outcomes: ReadonlyArray<unknown>
): string {
	const ends = labels.map((l, i) => `${l} ${describe_outcome(outcomes[i])}`);
	return `  MISMATCH ${file.path}: ${ends.join(', ')}`;
}

/** Each build's crash on `file` among `outcomes`, labeled `<build>: <path>` for the FAIL block. */
export function format_crash_entries(
	file: SourceFile,
	labels: ReadonlyArray<string>,
	outcomes: ReadonlyArray<unknown>
): PanicEntry[] {
	return outcomes.flatMap((o, i) =>
		o instanceof FormatThrew && is_tsv_crash(o.error)
			? [{ label: `${labels[i]}: ${file.path}`, error: crash_text(o.error) }]
			: []
	);
}

/**
 * Times the WASM format build at `--current` against itself, a `--control` copy, or a
 * `--baseline`, to stderr.
 *
 * @param deps - the glue loader and the corpus
 * @param argv - the tool's arguments: a corpus directory, `--current`, `--baseline`,
 * `--control`, `--pairs`, `--warmup`, `--lang`
 * @returns the exit code: 1 when a build crashes or a `--baseline` or `--control` formats
 *   differently, else 0
 */
export async function run_wasm_format_probe(
	deps: WasmFormatProbeDeps,
	argv: ReadonlyArray<string>
): Promise<number> {
	// minimal arg parsing — positional corpus dir + a few flags
	let corpus_dir = DEFAULT_CORPUS;
	let current_path = DEFAULT_WASM_GLUE;
	let baseline_path: string | null = null;
	let control_path: string | null = null;
	let pairs = 12;
	let warmup = 3;
	let lang_filter: Language | null = null;
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

	/** Load a wasm-pack build and expose its format entry points. */
	const load_build = async (js_path: string): Promise<FormatBuild> =>
		to_format_build(await deps.load_glue(js_path));

	const current = await load_build(current_path);
	const baseline = baseline_path ? await load_build(baseline_path) : null;
	const control = control_path ? await load_build(control_path) : null;

	const files = await deps.load_files(corpus_dir, (m) => console.error(m));

	// Intersection: keep only files some build formats, skipping those every build refuses
	// or throws the same error on, and gate byte-identity across them (the .control copy is
	// identical by construction; the .baseline is the real check). Timing past a mismatch
	// would be meaningless, and so would timing past a crash.
	const labeled = [
		{ label: 'current', build: current },
		{ label: 'baseline', build: baseline },
		{ label: 'control', build: control }
	].filter((b): b is { label: string; build: FormatBuild } => b.build !== null);
	const builds = labeled.map((b) => b.build);
	const labels = labeled.map((b) => b.label);
	const ok: SourceFile[] = [];
	const mismatch_lines: string[] = [];
	const crashes: PanicEntry[] = [];
	for (const f of files) {
		if (!langs.includes(f.language)) continue;
		const outcomes = builds.map((b) => format_outcome(b, f));
		const outcome = compare_format_outcomes(outcomes);
		if (outcome === 'crash') {
			crashes.push(...format_crash_entries(f, labels, outcomes));
			continue;
		}
		if (outcome === 'skipped') continue;
		if (outcome === 'mismatch') mismatch_lines.push(format_mismatch_line(f, labels, outcomes));
		ok.push(f);
	}
	const mismatches = mismatch_lines.length;
	console.error(
		`\nbyte-identity: ${ok.length - mismatches}/${ok.length} identical` +
			(mismatches ? `  — ${mismatches} MISMATCH` : '')
	);
	for (const line of mismatch_lines) console.error(line);
	if (crashes.length > 0) {
		for (const line of panic_report_lines(crashes)) console.error(line);
		return 1;
	}
	// a `--control` copy is the same code, so a mismatch there fails as a baseline's does: the
	// floor would time two builds that don't agree, and a file one of them throws on can't be timed
	if (mismatches) {
		console.error(
			'ABORT: builds format differently — a no-behavior-change edit must be byte-identical.'
		);
		return 1;
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
			`${baseline ? ' (A/B + floor)' : ' (A/A floor only)'}, warmup ${warmup}\n`
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
				langs.reduce((s, l) => s + ref_t[key][l][p], 0)
		);

	const primary = baseline ? 'ab' : 'aa'; // which ref's current samples to report as cur(ms)
	const kb_of = (lang: Language): number =>
		by_lang[lang].reduce((sum, f) => sum + f.bytes, 0) / 1024;
	const pad = (s: string, n: number): string => s.padStart(n);

	const print_row = (
		label: string,
		files_n: number,
		kb: number,
		cur_ms: number,
		ab: number[],
		aa: number[]
	): void => {
		let line =
			`${pad(label, 11)} ${pad(String(files_n), 4)}f ${pad(kb.toFixed(1), 8)}KB  ` +
			`cur ${pad(cur_ms.toFixed(2), 8)}ms ${pad(((cur_ms / kb) * 1000).toFixed(1), 7)}us/KB  ` +
			`floor ${median(aa).toFixed(4)}`;
		if (baseline) {
			const ab_m = median(ab);
			line +=
				`  A/B ${ab_m.toFixed(4)}  net ${(ab_m / median(aa)).toFixed(4)}  ` +
				`[${Math.min(...ab).toFixed(3)},${Math.max(...ab).toFixed(3)}]`;
		}
		console.error(line);
	};

	console.error(
		baseline
			? `A/B = current/baseline pair-median (<1 = faster) · net = A/B÷floor · [min,max] = A/B spread\n`
			: `floor = current/current pair-median (noise floor; expect ~1.00)\n`
	);
	for (const lang of langs) {
		if (by_lang[lang].length === 0) continue;
		print_row(
			lang,
			by_lang[lang].length,
			kb_of(lang),
			median(cur_t[primary][lang]),
			baseline ? ratios('ab', lang) : [],
			ratios('aa', lang)
		);
	}
	const total_cur = median(
		Array.from({ length: pairs }, (_, p) => langs.reduce((s, l) => s + cur_t[primary][l][p], 0))
	);
	console.error('');
	print_row(
		'total',
		ok.length,
		langs.reduce((s, l) => s + kb_of(l), 0),
		total_cur,
		baseline ? total_ratios('ab') : [],
		total_ratios('aa')
	);
	return 0;
}
