/**
 * Biome implementation wrapper (via WASM)
 *
 * Supports: TypeScript, JS, CSS, Svelte
 */

import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { BaseImplementation, type Language, LANGUAGE_EXTENSIONS, LANGUAGES } from './types.ts';
import type { BiomeVersions } from './versions.ts';
// Type-only — `import type` is erased, so referencing `Biome` here does NOT load
// the package at this module's import. The value imports are deferred to `init()`
// (see there) so a load-time crash can't escape the registry's skip.
import type { Biome, Configuration } from '@biomejs/js-api';
import { assert_format_config_landed, FORMAT_CONFIG_PROBES } from './format_config_probe.ts';

/**
 * The wasm-bindgen glue module (`biome_wasm_bg.js`): the JS half of the binding,
 * which the `.wasm` imports under this one module name and which holds the live
 * instance's exports behind `__wbg_set_wasm`. Untyped upstream.
 */
interface BiomeWasmGlue {
	__wbg_set_wasm(exports: WebAssembly.Exports | undefined): void;
}

/** The import module name `biome_wasm_bg.wasm` declares for every one of its imports. */
const GLUE_IMPORT_MODULE = './biome_wasm_bg.js';

/**
 * Linear-memory GROWTH since the instance was made past which `reset_heap` swaps in a
 * fresh one — i.e. the leak an instance is allowed to accumulate before the row pays a
 * swap for it. A fresh, configured instance sits at ~74 MB; a svelte sweep retains
 * ~30 MB, a TypeScript sweep ~117 MB, a css sweep ~4 MB (`closeFile` frees nothing —
 * see the class doc), so the svelte and TypeScript rows swap before EVERY sweep, the
 * css row every ~4, and a millisecond-sweep row (a `BENCH_LIMIT` probe, kilobytes a
 * sweep) once in thousands — that last one matters, because a 70 MB instantiation per
 * millisecond sweep out-churns the collector and read as a 40x slowdown when the swap
 * was unconditional. Keyed on growth rather than on a size so the rule needs no
 * runtime fact to be true and retires itself: a biome whose `closeFile` frees stops
 * growing after its first sweep and never swaps again. Growth since the SWAP, not since
 * the previous call: a slow leak (css) must still reach the swap, and measured per call
 * it never did (80 css sweeps, 0 swaps, 387 MB).
 *
 * Why every sweep, on a full-corpus row — measured with `diagnostics/biome_heap_probe.ts`
 * (the svelte row: 951 files, 30 consecutive sweeps). What a growing heap costs is
 * RUNTIME-dependent. On V8 (node, deno) the sweep time is FLAT as the heap grows —
 * measured to 974 MB in a bare process and inside the bench's own process context
 * (node: 1034 ms never resetting, cv ≤ 2%) — until Node's external memory passes ~1 GB,
 * where a TypeScript sweep nearly triples. On JSC (bun) a bare process is flat too
 * (857 ms never resetting, to 974 MB, cv 1.0%), but inside the bench process — after
 * the prettier-class tasks of the same group have run and left a large live JS heap —
 * the sweep time CLIMBS with the buffer's size and falls back at each swap: ~0.4 ms
 * per MB after prettier alone (1249 → 1617 ms over 74 → 974 MB, drift +12%), ~1.2 ms/MB
 * after the whole format/svelte group. The earlier rule, a 320 MB size budget, let a
 * swap land inside the timed window (svelte crossed it once per ~9 sweeps, TypeScript
 * every 2–3) and published a sawtooth: 1217 ms at cv 8.6% where a swap before every
 * sweep reads 1046 ms at cv 1.4% in the same context — the row §Unstable Rows flagged
 * under bun, with the TypeScript row's cv 3–4% the same shape below the threshold. The
 * mechanism is not pinned down; the shape fits JSC re-accounting the whole buffer on
 * each of a sweep's ~165 `memory.grow`s and collecting a heap whose cost scales with
 * what prettier left live. The per-sweep swap's own price on a full-corpus row is a
 * fresh instance's first sweep — ~+1% on bun, ~+3% on node (1062 vs 1018 ms, bare
 * process), paid on every sweep of every runtime alike — beyond the ~10 ms swap itself,
 * which runs in the untimed slots. The css row is where that price shows, because it
 * has no slope to buy off: 22-odd grows a sweep, and in the bench context bun reads
 * 88.7 ms with or without swaps (cv 4.3% → 1.9%) while node pays the every-fourth-sweep
 * fresh instance as +4% and a doubled cv (101.2 ms cv 2.4% → 105.3 ms cv 5.2%). The
 * threshold cannot rise to spare it: a svelte sweep's 30 MB is its ceiling, and 64 MiB
 * would put bun's svelte sawtooth back (a swap every third sweep).
 */
export const RESET_GROWTH_BYTES = 16 * 1024 * 1024;

/**
 * Match the prettier/tsv config — tabs, line width 100, single quotes, no
 * trailing commas — so every format row does the same layout work (at
 * biome's defaults, width 80 + double quotes, the rows wrap different
 * amounts of code and the ratios conflate config with engine speed). The
 * per-language `formatter.*` sections DO inherit the top-level ones and
 * override them where they set a key (measured in both directions); each
 * repeats the shared values anyway, so a rename that reached only the
 * top-level block can't silently un-pin every language at once. Biome has no
 * dedicated Svelte formatter — `html.experimentalFullSupportEnabled` is what lets
 * it format `.svelte` at all, via its experimental HTML-superset pipeline; that
 * path formats the embedded `<script>`/`<style>` too (verified), so the svelte row
 * is comparable work to prettier-plugin-svelte / tsv. Without the flag biome skips it.
 *
 * Applied on every fresh workspace (`reset_heap`), not just at init.
 */
const BIOME_CONFIGURATION: Configuration = {
	formatter: {
		indentStyle: 'tab',
		lineWidth: 100
	},
	javascript: {
		formatter: {
			indentStyle: 'tab',
			lineWidth: 100,
			quoteStyle: 'single',
			trailingCommas: 'none'
		}
	},
	css: {
		formatter: {
			indentStyle: 'tab',
			lineWidth: 100,
			quoteStyle: 'single'
		}
	},
	html: {
		experimentalFullSupportEnabled: true,
		formatter: {
			indentStyle: 'tab',
			lineWidth: 100
		}
	}
};

/**
 * Biome implementation using WASM.
 *
 * Supports:
 * - Format: Svelte, TypeScript, JS, CSS
 * - Parse: unsupported — the `@biomejs/js-api` package exposes no parse entry
 *   point (only `formatContent`/`lintContent`/`fixFile`); Biome parses
 *   internally but never surfaces the AST across the JS boundary.
 *
 * **The wasm module is instantiated BY HAND, and re-instantiated whenever the sweeps
 * it has run have leaked more than a budget into its linear memory (`reset_heap`,
 * called between sweeps).**
 * `Workspace.openFile` retains ~4.5 B of linear
 * memory per source byte on every call — identical content, identical path and
 * no formatting at all cost the same, and `closeFile` releases nothing (measured
 * at `@biomejs/wasm-bundler` 2.5.13: 200 open/close pairs on one 13 KB file grow
 * the heap 11.2 MB) — so one full TypeScript sweep leaks ~117 MB that no GC can
 * reach, and Node's per-sweep time nearly triples once the process's external
 * memory passes ~1 GB. A row measured on that heap publishes the leak, not the
 * formatter (on V8 the cost is a STEP, not a slope — sweep time is flat to at least
 * 974 MB — while on JSC it is a slope whose steepness follows the live JS heap;
 * `RESET_GROWTH_BYTES` carries the measurements). Linear memory never shrinks, so the
 * only way back to a clean heap is a new instance: `@biomejs/wasm-bundler`'s own entry (`biome_wasm.js`) binds ONE
 * ESM-cached instance for the life of the process, so this wrapper never imports
 * it — it compiles the `.wasm` bytes once and instantiates them itself against the
 * package's glue (`biome_wasm_bg.js`), which is also what makes the row load under
 * Bun (the entry's `__wbindgen_start` hook is the one Bun's ESM wasm handling never
 * called; here it is called explicitly).
 */
export class BiomeImplementation extends BaseImplementation {
	readonly versions: BiomeVersions;
	private _module: WebAssembly.Module | null = null;
	private _glue: (BiomeWasmGlue & WebAssembly.ModuleImports) | null = null;
	private _biome_class: typeof Biome | null = null;
	private _instance: WebAssembly.Instance | null = null;
	private _biome: Biome | null = null;
	private _project_key: number | null = null;
	/** Linear-memory size right after the swap that made the live instance — what growth is measured from. */
	private _fresh_bytes = 0;

	/** The js-api exposes no parser. */
	readonly parse_languages: ReadonlyArray<Language> = [];
	readonly format_languages = LANGUAGES;

	constructor(versions: BiomeVersions) {
		super();
		this.versions = versions;
	}

	async init(): Promise<void> {
		// Load the glue + js-api lazily (not as static top-level imports) so a
		// load-time failure throws HERE, inside init_implementations' per-impl
		// try/catch (and is skipped), instead of throwing during this module's
		// static import graph and aborting the whole registry. The `.wasm` is read
		// as bytes and compiled once; every instance comes from `reset_heap` (see the
		// class doc for why the package's own entry is never imported).
		const glue = (await import(
			'@biomejs/wasm-bundler/biome_wasm_bg.js'
		)) as unknown as BiomeWasmGlue & WebAssembly.ModuleImports;
		const { Biome } = await import('@biomejs/js-api');
		const require = createRequire(import.meta.url);
		const bytes = readFileSync(require.resolve('@biomejs/wasm-bundler/biome_wasm_bg.wasm'));
		this._module = await WebAssembly.compile(bytes);
		this._glue = glue;
		this._biome_class = Biome;
		this.reset_heap();

		// Assert the configuration actually LANDED. `applyConfiguration` accepts an
		// unrecognized key SILENTLY — no throw, no diagnostic (verified) — so a
		// renamed key in a future biome major would leave this row formatting at
		// biome's own defaults (measured: width 80, double quotes, trailing commas)
		// and wrapping a different amount of code than every other format row, with
		// nothing in the report to say so. dprint and malva have a diagnostic channel
		// for this (`getConfigDiagnostics`); biome has none, so the check is
		// behavioral. Routed through `format` rather than `formatContent` so the probe
		// exercises the exact call the timed row makes.
		//
		// ONE probe PER LANGUAGE, because the config above is one section per language
		// and each feeds a different row: a TypeScript-only probe proves the
		// `javascript` section and leaves `css` and `html` — the CSS and svelte rows —
		// free to un-pin silently, which is the failure this check exists to catch. The
		// svelte pass doubles as the only guard on `experimentalFullSupportEnabled`:
		// without it biome returns an EMPTY string for `.svelte`, which the timed row
		// would otherwise score as a successful format.
		//
		// ⚠️ Per-language coverage, impl-wide COST — the same shape `lib/oxc.ts` carries
		// for its two tools: the registry's unit of absence is the impl, so a probe
		// failing on ONE language takes biome's other rows down with it. Sharper here
		// than there, because the likeliest failure is the language whose support is
		// itself experimental: a biome release that changes `.svelte` handling removes
		// the TypeScript and CSS rows too. Deliberate — the alternative is a row
		// publishing a number produced at biome's own defaults — and disclosed rather
		// than silent: `unavailable[].rows` names every row the failure removed.
		for (const language of this.format_languages) {
			assert_format_config_landed(
				'biome',
				language,
				this.format(FORMAT_CONFIG_PROBES[language], language)
			);
		}
	}

	/**
	 * Drop the live wasm instance and start a fresh one — the same project and
	 * configuration on a linear memory that holds nothing — once the live one has
	 * grown by more than `RESET_GROWTH_BYTES` since it was made (the leak of every sweep
	 * it has run); a no-op otherwise. Synchronous, ~10 ms when it swaps
	 * (`new WebAssembly.Instance` over the compiled module, then a workspace), which is
	 * what lets the bench call it from the untimed slots between sweeps.
	 *
	 * Order matters twice. The old `Workspace` is `shutdown()` BEFORE the swap: its
	 * `FinalizationRegistry` would otherwise free an old pointer into the NEW
	 * instance. And the old memory is `grow(0)`-ed, which detaches its buffer: the
	 * glue caches its `DataView` / `Uint8Array` views and refreshes them only when
	 * the buffer it holds is detached (`biome_wasm_bg.js`, `getDataViewMemory0`),
	 * so a live old buffer leaves the views pointing at the wrong instance and the
	 * first call into the new one panics in `__rust_dealloc`.
	 *
	 * @param force - swap regardless of growth (`diagnostics/biome_heap_probe.ts`'s
	 * every-sweep regime); the bench never passes it
	 */
	reset_heap(force = false): void {
		if (!this._module || !this._glue || !this._biome_class) {
			throw new Error('Biome not initialized');
		}
		if (this._instance) {
			const memory = this._instance.exports.memory as WebAssembly.Memory;
			if (!force && memory.buffer.byteLength - this._fresh_bytes <= RESET_GROWTH_BYTES) return;
			this._biome?.shutdown();
			memory.grow(0);
		}
		const instance = new WebAssembly.Instance(this._module, { [GLUE_IMPORT_MODULE]: this._glue });
		this._glue.__wbg_set_wasm(instance.exports);
		(instance.exports.__wbindgen_start as () => void)();
		// A fresh object per instance: js-api runs the module's `main()` once per
		// module OBJECT (a `WeakSet`), and the new instance needs its own.
		const biome = new this._biome_class({ ...this._glue } as unknown as ConstructorParameters<
			typeof Biome
		>[0]);
		const { projectKey } = biome.openProject('/tmp');
		biome.applyConfiguration(projectKey, BIOME_CONFIGURATION);
		this._instance = instance;
		this._biome = biome;
		this._project_key = projectKey;
		this._fresh_bytes = (instance.exports.memory as WebAssembly.Memory).buffer.byteLength;
	}

	/**
	 * The live instance's linear memory — whose growth `reset_heap` grades.
	 * Read by `diagnostics/biome_heap_probe.ts` to log the heap beside each sweep's time.
	 */
	linear_memory(): WebAssembly.Memory {
		if (!this._instance) throw new Error('Biome not initialized');
		return this._instance.exports.memory as WebAssembly.Memory;
	}

	parse(_source: string, _language: Language): unknown {
		throw new Error('Biome has no parser: the @biomejs/js-api package exposes no parse API');
	}

	format(source: string, language: Language): string {
		// `=== null` on the key, not a truthiness test: `openProject` hands back a
		// numeric handle, and a legitimate `0` would read as uninitialized.
		if (!this._biome || this._project_key === null) {
			throw new Error('Biome not initialized');
		}
		if (!this.supports_format_language(language)) {
			throw new Error(`Biome does not support ${language}`);
		}

		try {
			const result = this._biome.formatContent(this._project_key, source, {
				filePath: `file${LANGUAGE_EXTENSIONS[language]}`
			});
			return result.content;
		} catch (e: unknown) {
			// Biome WASM panics have minimal info in the error - the full panic message
			// is printed to stderr by the WASM module (not capturable here).
			// Provide a cleaner error message for the benchmark output.
			if (e && typeof e === 'object' && 'stackTrace' in e) {
				const stack_trace = String((e as { stackTrace: unknown }).stackTrace);
				if (stack_trace.includes('unreachable')) {
					throw new Error('Biome internal error (WASM panic)');
				}
			}
			// For errors with actual messages, pass them through
			if (e instanceof Error && e.message) {
				throw e;
			}
			throw new Error('Biome format failed');
		}
	}

	// deno-lint-ignore require-await
	async format_async(source: string, language: Language): Promise<string> {
		return this.format(source, language);
	}

	dispose(): void {
		if (this._biome) this._biome.shutdown();
		this._glue?.__wbg_set_wasm(undefined);
		this._instance = null;
		this._biome = null;
		this._project_key = null;
	}
}
