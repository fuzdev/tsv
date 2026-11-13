/**
 * Diagnostic: measure WASM **linear-memory high-water** for `format()` — the
 * axis `wasm_format_probe.ts` (wall-time) can't see, and the one that gates all
 * doc-IR memory work (arena/output pre-size, the parked `DocNode` shrink).
 *
 * WASM linear memory only ever grows (`memory.grow`, never shrinks), so
 * `memory.buffer.byteLength` read after a format IS the peak that format drove.
 * The wasm-pack glue does not re-export `memory`, so the root's loader
 * (`benches/js/roots/wasm_glue.ts`) captures it as the glue instantiates its module, and
 * evaluates the glue afresh on every load, so each load is a genuinely fresh
 * instance+memory, which is how the cold-start mode gets an un-warmed arena per file
 * in-process.
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
 *           largest single file's actual demand, NOT the pre-size hint — the
 *           working notes predict this is ~invariant to the pre-size levers;
 *           this mode proves or refutes that).
 *
 * A/B two builds with --baseline (mirrors wasm_format_probe.ts): each build is a
 * separate module URL → its own instance+memory, so both load in one process. A crash in
 * either build (`is_tsv_crash`: a trap), in the A/B's byte-identity gate or in a measurement,
 * fails the run, listed with the build and the file; any other throw skips the file.
 *
 *   # A/A (one build — just capture reserve + cold/steady high-water):
 *   node benches/js/roots/wasm_memory_probe.ts --cold
 *
 *   # A/B (does a pre-size change lower the cold peak?):
 *   cp -r crates/tsv_wasm/pkg/all/nodejs crates/tsv_wasm/pkg/all/nodejs.baseline
 *   # ...edit pre-size, npm run build:wasm:all:nodejs...
 *   node benches/js/roots/wasm_memory_probe.ts --cold \
 *     --baseline crates/tsv_wasm/pkg/all/nodejs.baseline/tsv_wasm.js
 *
 * Human output → stderr; --json → a clean object on stdout. `run_wasm_memory_probe` is the
 * library function; its root, `benches/js/roots/wasm_memory_probe.ts`, binds its deps
 * (`WasmMemoryProbeDeps`).
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
import { LANGUAGES } from '../lib/types.ts';
import type { Language, Logger, SourceFile } from '../lib/types.ts';
import {
	compare_format_outcomes,
	DEFAULT_WASM_GLUE,
	format_crash_entries,
	format_outcome,
	format_mismatch_line,
	to_format_build,
	type WasmGlueExports
} from './wasm_format_probe.ts';

const DEFAULT_CORPUS = `${CORPORA_COLLECTIONS}/zzz/src/lib`;
const PAGE = 65536;

/** A glue module loaded afresh, with the linear memory its instance got, if one was seen. */
export interface FreshGlue {
	exports: WasmGlueExports;
	memory: WebAssembly.Memory | null;
}

/** What `run_wasm_memory_probe` reaches the world through. */
export interface WasmMemoryProbeDeps {
	/** The wasm-pack `nodejs` glue at `path`, evaluated afresh: a new instance and memory. */
	load_fresh_glue(path: string): Promise<FreshGlue>;
	/** The files beneath `path`. */
	load_files(path: string, logger: Logger): Promise<SourceFile[]>;
	/** Where `--json` writes its report. */
	stdout: { write(data: string): Promise<unknown> };
}

/**
 * Measures the WASM format build's linear-memory high-water, cold per file or steady over the
 * corpus, to stderr (and `--json` to stdout).
 *
 * @param deps - the glue loader, the corpus, stdout
 * @param argv - the tool's arguments: a corpus directory, `--current`, `--baseline`, `--lang`,
 * `--limit`, `--cold`, `--json`
 * @returns the exit code: 1 when a `--baseline` formats differently or a build crashes, else 0
 */
export async function run_wasm_memory_probe(
	deps: WasmMemoryProbeDeps,
	argv: ReadonlyArray<string>
): Promise<number> {
	// ---- arg parsing (positional corpus dir + flags), mirrors wasm_format_probe ----
	let corpus_dir = DEFAULT_CORPUS;
	let current_path = DEFAULT_WASM_GLUE;
	let baseline_path: string | null = null;
	let lang_filter: Language | null = null;
	let limit = Infinity;
	let cold = false;
	let json = false;
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

	type FormatBuild = ReturnType<typeof to_format_build>;
	interface Loaded {
		build: FormatBuild;
		mem: WebAssembly.Memory;
	}

	/** Load a glue build fresh and pair it with its memory. */
	async function load_fresh(js_path: string): Promise<Loaded> {
		const { exports, memory } = await deps.load_fresh_glue(js_path);
		if (!memory) throw new Error(`no memory captured for ${js_path}`);
		return { build: to_format_build(exports), mem: memory };
	}

	const now_bytes = (m: WebAssembly.Memory): number => m.buffer.byteLength;
	const mb = (b: number): string => (b / 1e6).toFixed(2);
	const pages = (b: number): number => b / PAGE;
	const pct = (n: number, d: number): string => ((n / d - 1) * 100).toFixed(2);

	// ---- load corpus ----
	const files = (await deps.load_files(corpus_dir, (m) => console.error(m))).filter((f) =>
		langs.includes(f.language)
	);
	const by_lang = group_by_language(files);
	const scoped: SourceFile[] = [];
	for (const lang of langs)
		scoped.push(...by_lang[lang].slice(0, limit === Infinity ? undefined : limit));
	scoped.sort((a, b) => a.bytes - b.bytes);

	const builds: { label: string; path: string }[] = [{ label: 'current', path: current_path }];
	if (baseline_path) builds.push({ label: 'baseline', path: baseline_path });

	const percentile = (xs: number[], p: number): number => {
		if (xs.length === 0) return 0;
		const s = [...xs].sort((a, b) => a - b);
		return s[Math.min(s.length - 1, Math.floor(p * s.length))];
	};

	/** The measurement loops' crashes (`is_tsv_crash`), each with its build and file. */
	const measured_crashes: PanicEntry[] = [];
	/** Records `thrown` when it is a crash; any other throw is the file's skip. */
	const measure_crash = (label: string, file: SourceFile, thrown: unknown): void => {
		if (is_tsv_crash(thrown)) {
			measured_crashes.push({ label: `${label}: ${file.path}`, error: crash_text(thrown) });
		}
	};

	// Byte-identity gate (A/B only): a memory A/B is meaningful only when both builds
	// format identically — a pre-size/no-behavior-change edit must be byte-identical, else
	// the peak delta is an output difference, not a reservation effect. Mirrors
	// wasm_format_probe.ts (`compare_format_outcomes`: a throw or refusal on one side only is a
	// mismatch, a crash on either fails). One warm instance each (byte-identity is
	// instance-independent).
	if (baseline_path) {
		const cur = await load_fresh(current_path);
		const base = await load_fresh(baseline_path);
		const labels = ['current', 'baseline'];
		const mismatch_lines: string[] = [];
		const crashes: PanicEntry[] = [];
		for (const f of scoped) {
			const outcomes = [format_outcome(cur.build, f), format_outcome(base.build, f)];
			const outcome = compare_format_outcomes(outcomes);
			if (outcome === 'crash') crashes.push(...format_crash_entries(f, labels, outcomes));
			else if (outcome === 'mismatch')
				mismatch_lines.push(format_mismatch_line(f, labels, outcomes));
		}
		for (const line of mismatch_lines) console.error(line);
		if (crashes.length > 0) {
			for (const line of panic_report_lines(crashes)) console.error(line);
			return 1;
		}
		const mismatches = mismatch_lines.length;
		if (mismatches) {
			console.error(
				`ABORT: builds format differently on ${mismatches}/${scoped.length} files — ` +
					`a memory A/B is only meaningful when output is byte-identical.`
			);
			return 1;
		}
		console.error(`byte-identity: ${scoped.length}/${scoped.length} identical`);
	}

	console.error(
		`\nWASM memory probe · ${scoped.length} files · ${cold ? 'COLD (fresh instance/file)' : 'steady-state (one warm instance)'}` +
			`${baseline_path ? ' · A/B' : ' · A/A'}\n`
	);

	// deno-lint-ignore no-explicit-any
	const report: Record<string, any> = {
		mode: cold ? 'cold' : 'steady',
		files: scoped.length,
		builds: {}
	};

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
				} catch (e) {
					measure_crash(b.label, f, e);
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
				n
			};
			const r = report.builds[b.label];
			console.error(
				`${b.label.padStart(9)}: reserve ${mb(r.reserve_bytes)}MB (${pages(r.reserve_bytes)}p) · ` +
					`cold peak p50 ${mb(r.peak_p50)} / p90 ${mb(r.peak_p90)} / p99 ${mb(r.peak_p99)} / max ${mb(r.peak_max)}MB · ` +
					`growth p50 ${mb(r.growth_p50)} / max ${mb(r.growth_max)}MB (n=${n})`
			);
		}
		if (baseline_path) {
			const c = report.builds.current,
				bl = report.builds.baseline;
			console.error(
				`\nA/B (current vs baseline): reserve ${pct(c.reserve_bytes, bl.reserve_bytes)}% · ` +
					`peak_p90 ${pct(c.peak_p90, bl.peak_p90)}% · peak_max ${pct(c.peak_max, bl.peak_max)}% ` +
					`(negative = current uses less)`
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
				} catch (e) {
					measure_crash(b.label, f, e);
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
				processed
			};
			const r = report.builds[b.label];
			console.error(
				`${b.label.padStart(9)}: reserve ${mb(r.reserve_bytes)}MB (${pages(r.reserve_bytes)}p) · ` +
					`steady high-water ${mb(r.high_water_bytes)}MB (${pages(r.high_water_bytes)}p) · ` +
					`corpus growth ${mb(r.total_growth_bytes)}MB (${processed} files)`
			);
		}
		if (baseline_path) {
			const c = report.builds.current,
				bl = report.builds.baseline;
			console.error(
				`\nA/B (current vs baseline): reserve ${pct(c.reserve_bytes, bl.reserve_bytes)}% · ` +
					`high-water ${pct(c.high_water_bytes, bl.high_water_bytes)}% (negative = current uses less)`
			);
		}
	}

	if (json) await deps.stdout.write(JSON.stringify(report, null, 2) + '\n');
	// after the report, on stderr: a build's numbers over a source it crashed on can't be read
	if (measured_crashes.length > 0) {
		for (const line of panic_report_lines(measured_crashes)) console.error(line);
		return 1;
	}
	return 0;
}
