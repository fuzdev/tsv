/**
 * Diagnostic: does biome's sweep time move with its wasm linear memory, per runtime?
 *
 * `lib/biome.ts` re-instantiates the wasm module between sweeps by a rule
 * (`RESET_GROWTH_BYTES`) whose premise is a measured cost of a grown heap. This probe is
 * what measures it, on ONE row — the bench's own corpus for a language, the bench's own
 * `BiomeImplementation` (init + configuration) — by running N consecutive full sweeps
 * under one of three reset regimes and logging, per sweep, the wall time beside the heap:
 *
 *   never   — `reset_heap` never called: the heap grows monotonically for the whole run
 *   every   — `reset_heap(true)`: a fresh instance before every sweep
 *   budget  — `reset_heap()`: the production rule, whatever it is today
 *
 * If `never` degrades where `every` is flat, the mechanism is wasm-memory growth below
 * the budget; if both degrade alike, it is JS-side or the machine. Meant to be run under
 * bun AND node (node is the control), one process per runtime × regime, alternating the
 * order so a thermal / ordering effect shows up as a between-run difference:
 *
 *   node --disable-warning=ExperimentalWarning benches/js/diagnostics/biome_heap_probe.ts --regime never
 *   bun benches/js/diagnostics/biome_heap_probe.ts --regime never
 *
 * Per sweep: wall ms, `memory.buffer.byteLength` before/after, the number of times the
 * buffer's identity changed during the sweep (a `memory.grow` from inside the wasm
 * replaces the buffer, so this counts grow events at file granularity — a lower bound),
 * and `process.memoryUsage()` (rss / heapUsed / external). No GC is forced between
 * sweeps, like the bench's timed loop. Human table → stderr; `--json` → stdout.
 *
 * `--prelude <rows>` reproduces the bench's PROCESS context instead of a bare one: the
 * whole implementation set is initialized the way `bench.ts` does it, and the named
 * format rows of the same group (`prettier`, `tsv`, `tsv-wasm`, `oxfmt` — the tasks the
 * bench times ahead of `biome-wasm` in `format/<lang>`) each sweep the corpus
 * `--prelude-sweeps` times (default 4) in the bench's order, with the bench's untimed
 * major GC (`settle_heap`) between tasks, before the biome sweeps begin. Isolated, the
 * row can be flat while the bench's reading of it is not; this is how to tell whether
 * what the earlier tasks leave in the JS heap is the difference.
 *
 * Flags: `--regime never|every|budget` (default budget), `--sweeps N` (default 30),
 * `--lang svelte|typescript|css` (default svelte), `--prelude a,b,c`,
 * `--prelude-sweeps N`, `--json`.
 *
 * Findings (svelte, bun 1.4.2 / node 24.14, timed = sweeps 5..N; a svelte sweep retains
 * ~30 MB over a ~74 MB fresh instance and grows the buffer ~165 times):
 *
 *   bare process, 30 sweeps       never              every              budget
 *     bun                         857 ms cv 1.0%     865 ms cv 1.4%     856 ms cv 1.5%
 *     node                        1034 ms cv 1.7%    1062 ms cv 1.3%    1018 ms cv 1.7%
 *   after `--prelude prettier`    never              every              budget
 *     bun                         1402 ms, 1249→1617 (drift +12%)   1046 ms cv 1.4%   1217 ms cv 8.6%
 *   after the whole group, budget: bun 1349 ms cv 9.1% (sawtooth, resets at sweeps 10 and 19);
 *     node 1156 ms cv 1.6% — flat, and equal to node's bench row.
 *
 * So the wasm heap's growth is free on V8 at any size reached here, and free on JSC in
 * a bare process — but inside the bench's process, after prettier-class tasks, bun's
 * sweep time is a slope in the buffer's size that a reset restores, which the 320 MB
 * size budget those runs measured (`budget` above) let into the timed window; the
 * growth-keyed rule that replaced it resets these rows before every sweep.
 * `RESET_GROWTH_BYTES` restates the numbers beside the rule they size.
 */

import { argv, memoryUsage } from 'node:process';

import { BiomeImplementation } from '../lib/biome.ts';
import { CorpusLoader, group_by_language } from '../lib/corpus.ts';
import { init_implementations } from '../lib/implementations.ts';
import { current_runtime } from '../lib/runtime.ts';
import { LANGUAGES, type Language, type SourceFile } from '../lib/types.ts';
import { load_all_versions } from '../lib/versions.ts';

type Regime = 'never' | 'every' | 'budget';
const REGIMES: ReadonlyArray<Regime> = ['never', 'every', 'budget'];

type PreludeRow = 'prettier' | 'tsv' | 'tsv-wasm' | 'oxfmt';
const PRELUDE_ROWS: ReadonlyArray<PreludeRow> = ['prettier', 'tsv', 'tsv-wasm', 'oxfmt'];

let regime: Regime = 'budget';
let sweeps = 30;
let language: Language = 'svelte';
let json = false;
let prelude: PreludeRow[] = [];
let prelude_sweeps = 4;
const args = argv.slice(2);
for (let i = 0; i < args.length; i++) {
	const a = args[i];
	if (a === '--prelude') {
		prelude = args[++i].split(',') as PreludeRow[];
		for (const row of prelude) {
			if (!PRELUDE_ROWS.includes(row)) {
				throw new Error(`--prelude rows must be among ${PRELUDE_ROWS.join(',')}`);
			}
		}
	} else if (a === '--prelude-sweeps') {
		prelude_sweeps = Number(args[++i]);
		// 0 is the control: the bench's full init, and no earlier task's sweeps
		if (!Number.isInteger(prelude_sweeps) || prelude_sweeps < 0) {
			throw new Error('--prelude-sweeps must be a non-negative integer');
		}
	} else if (a === '--regime') {
		const v = args[++i] as Regime;
		if (!REGIMES.includes(v)) throw new Error(`--regime must be one of ${REGIMES.join('|')}`);
		regime = v;
	} else if (a === '--sweeps') {
		sweeps = Number(args[++i]);
		if (!Number.isInteger(sweeps) || sweeps < 1)
			throw new Error('--sweeps must be a positive integer');
	} else if (a === '--lang') {
		const v = args[++i] as Language;
		if (!LANGUAGES.includes(v)) throw new Error(`--lang must be one of ${LANGUAGES.join('|')}`);
		language = v;
	} else if (a === '--json') json = true;
	else throw new Error(`unknown argument ${a}`);
}

const runtime = current_runtime();
const log = (...xs: unknown[]): void => console.error(...xs);
const mb = (b: number): string => (b / 1e6).toFixed(1);

/**
 * JSC's own heap accounting, under bun only (`bun:jsc`'s `heapStats`): `heapSize` is
 * the live JS heap and `extraMemorySize` what the collector has been told lives
 * outside it — where a wasm memory's buffers land. Both `0` elsewhere. The specifier
 * is a variable so neither `deno check` nor node tries to resolve it.
 */
const jsc_heap_stats: () => { heap_size: number; extra_memory: number } = await (async () => {
	if (runtime !== 'bun') return () => ({ heap_size: 0, extra_memory: 0 });
	const specifier = 'bun:jsc';
	const jsc = (await import(specifier)) as {
		heapStats: () => { heapSize: number; extraMemorySize: number };
	};
	return () => {
		const s = jsc.heapStats();
		return { heap_size: s.heapSize, extra_memory: s.extraMemorySize };
	};
})();

// The bench's corpus, the bench's way: the perf view, every language streamed, then
// one language's files in the loader's order (the bench applies no limit or filter here).
const loader = new CorpusLoader('perf', { missing: 'fail' });
const all: SourceFile[] = [];
for await (const file of loader.stream(() => {})) all.push(file);
const files = group_by_language(all)[language];
const bytes = files.reduce((sum, f) => sum + f.bytes, 0);

/** The bench's inter-task settle (`settle_heap`): a major GC, or nothing without `--expose-gc`. */
const settle_heap = (): void => {
	globalThis.gc?.();
};

let impl: BiomeImplementation;
if (prelude.length === 0) {
	const versions = await load_all_versions();
	impl = new BiomeImplementation(versions.biome);
	await impl.init();
} else {
	// The bench's own process state: every impl initialized (their modules, wasm
	// instances and JIT state all resident), then the named earlier tasks of the
	// group sweep the corpus ahead of the biome row.
	const impls = await init_implementations({ logger: () => {} });
	if (!impls.biome) throw new Error('biome failed to initialize');
	impl = impls.biome;
	if (typeof globalThis.gc !== 'function') {
		log('⚠ no gc() — run with --expose-gc so the inter-task settle is the bench’s');
	}
	const sweep_of: Record<PreludeRow, (f: SourceFile) => Promise<unknown> | unknown> = {
		prettier: (f) => impls.canonical.format_async(f.content, language),
		tsv: (f) => impls.native.format(f.content, language),
		'tsv-wasm': (f) => impls.wasm.format(f.content, language),
		oxfmt: (f) => {
			if (!impls.oxc) throw new Error('oxfmt failed to initialize');
			return impls.oxc.format_async(f.content, language);
		}
	};
	for (const row of prelude) {
		settle_heap();
		const start = performance.now();
		for (let i = 0; i < prelude_sweeps; i++) {
			for (const f of files) await sweep_of[row](f);
		}
		if (prelude_sweeps > 0) {
			log(
				`prelude ${row}: ${prelude_sweeps} sweeps, ${((performance.now() - start) / prelude_sweeps).toFixed(0)} ms each`
			);
		}
	}
	settle_heap();
}

interface Sweep {
	sweep: number;
	wall_ms: number;
	heap_before: number;
	heap_after: number;
	grows: number;
	rss: number;
	heap_used: number;
	external: number;
	jsc_heap: number;
	jsc_extra: number;
}

const rows: Sweep[] = [];
log(
	`biome heap probe · ${runtime} · ${language} · ${files.length} files (${mb(bytes)} MB) · regime ${regime}` +
		` · ${sweeps} sweeps`
);
log(
	'sweep   wall_ms   heap_before  heap_after  grows      rss  heap_used  external  jsc_heap  jsc_extra'
);
for (let i = 1; i <= sweeps; i++) {
	// The bench's untimed slot: the reset is offered BEFORE every timed sweep (its
	// `setup` before the first, `on_iteration` before each later one).
	if (regime !== 'never') impl.reset_heap(regime === 'every');
	const memory = impl.linear_memory();
	const heap_before = memory.buffer.byteLength;
	let buffer = memory.buffer;
	let grows = 0;
	const start = performance.now();
	for (const f of files) {
		const out = impl.format(f.content, language);
		// the bench's `assert_output_present`: an empty file is the one input an empty output is right for
		if (f.bytes > 0 && out.length === 0) throw new Error(`empty output for ${f.path}`);
		if (memory.buffer !== buffer) {
			grows++;
			buffer = memory.buffer;
		}
	}
	const wall_ms = performance.now() - start;
	const heap_after = memory.buffer.byteLength;
	const mu = memoryUsage();
	const jsc = jsc_heap_stats();
	const row: Sweep = {
		sweep: i,
		wall_ms,
		heap_before,
		heap_after,
		grows,
		rss: mu.rss,
		heap_used: mu.heapUsed,
		external: mu.external,
		jsc_heap: jsc.heap_size,
		jsc_extra: jsc.extra_memory
	};
	rows.push(row);
	log(
		`${String(i).padStart(5)}  ${wall_ms.toFixed(1).padStart(8)}  ${mb(heap_before).padStart(11)}  ` +
			`${mb(heap_after).padStart(10)}  ${String(grows).padStart(5)}  ${mb(mu.rss).padStart(7)}  ` +
			`${mb(mu.heapUsed).padStart(9)}  ${mb(mu.external).padStart(8)}  ${mb(jsc.heap_size).padStart(8)}  ${mb(jsc.extra_memory).padStart(9)}`
	);
}

// The bench's readings over the sweeps a bench row would have timed: the first
// WARMUP sweeps stand in for its time-sized warmup (~5 s of sweeps), the rest are the
// timed window — mean, and the drift statistic bench.ts publishes (the median of the
// second half against the first's).
const WARMUP = 4;
const timed = rows.slice(WARMUP).map((r) => r.wall_ms);
const median = (xs: number[]): number => {
	const s = [...xs].sort((a, b) => a - b);
	const mid = Math.floor(s.length / 2);
	return s.length % 2 === 0 ? (s[mid - 1] + s[mid]) / 2 : s[mid];
};
const mean = timed.reduce((a, b) => a + b, 0) / timed.length;
const half = Math.floor(timed.length / 2);
const drift =
	median(timed.slice(0, half)) > 0
		? median(timed.slice(timed.length - half)) / median(timed.slice(0, half)) - 1
		: null;
const sd = Math.sqrt(timed.reduce((a, t) => a + (t - mean) ** 2, 0) / (timed.length - 1));
const summary = {
	runtime,
	language,
	regime,
	prelude,
	prelude_sweeps: prelude.length === 0 ? 0 : prelude_sweeps,
	files: files.length,
	bytes,
	sweeps,
	warmup_sweeps: WARMUP,
	timed_mean_ms: mean,
	timed_min_ms: Math.min(...timed),
	timed_max_ms: Math.max(...timed),
	timed_cv: sd / mean,
	drift,
	heap_final_bytes: rows[rows.length - 1].heap_after,
	resets: rows.filter((r, i) => i > 0 && r.heap_before < rows[i - 1].heap_after).length
};
log(
	`timed (sweeps ${WARMUP + 1}..${sweeps}): mean ${mean.toFixed(1)} ms · min ${summary.timed_min_ms.toFixed(1)} · ` +
		`max ${summary.timed_max_ms.toFixed(1)} · cv ${(summary.timed_cv * 100).toFixed(1)}% · ` +
		`drift ${drift === null ? 'n/a' : `${(drift * 100).toFixed(1)}%`} · resets ${summary.resets} · final heap ${mb(summary.heap_final_bytes)} MB`
);
impl.dispose();

if (json) console.log(JSON.stringify({ ...summary, rows }, null, 2));
