/**
 * WASM bindings to tsv
 *
 * Uses wasm-pack generated bindings for WebAssembly performance testing.
 * Runtime-aware: each runtime loads its own wasm-pack *target* bundle (same
 * `tsv_wasm_bg.wasm`, different JS glue), both carrying the full export set
 * including the benchmark-only `parse_internal_*`:
 *  - Deno: the `deno` target (ESM; explicit `default()` init)
 *  - Node/Bun: the `nodejs` target (CommonJS; self-initializing on require)
 * The shipped `@fuzdev/tsv-wasm` (web) bundle is deliberately NOT used here — it
 * curates out `parse_internal_*`, which the `tsv-wasm-internal` row needs.
 */

import { stat } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { wasm_target } from './runtime.ts';
import { wasm_bundle_dir } from './tsv_artifacts.ts';
import { BaseImplementation, goal_for, type Language, LANGUAGES, type ParseGoal } from './types.ts';
import { assert_binding_reports_rejection } from './reject_probe.ts';
import { assert_binding_emits_span_only } from './locations_probe.ts';

/**
 * WASM module function signatures — the raw wasm-bindgen exports, flat like the two
 * native bindings': each takes the source type as a trailing OPTIONAL string
 * (`'script'` / `'module'`; omitted = none named). Svelte and CSS REJECT a set one
 * rather than ignoring it, so the wrappers below withhold it for them. `parse_<lang>`
 * returns the span-only wire as an object, `JSON.parse`d engine-side via `js_sys`.
 */
interface WasmModule {
	parse_svelte: (source: string, source_type?: string) => unknown;
	parse_internal_svelte: (source: string, source_type?: string) => void;
	format_svelte: (source: string, source_type?: string) => string;
	parse_typescript: (source: string, source_type?: string) => unknown;
	parse_internal_typescript: (source: string, source_type?: string) => void;
	format_typescript: (source: string, source_type?: string) => string;
	parse_css: (source: string, source_type?: string) => unknown;
	parse_internal_css: (source: string, source_type?: string) => void;
	format_css: (source: string, source_type?: string) => string;
}

/**
 * The per-language export tables, resolved ONCE in `init()`.
 *
 * The native siblings' `FfiTables` / `NapiTables` for the same reason: these were
 * getters returning a fresh object literal, so every timed call allocated one —
 * harness-side allocation charged to whichever row it sat under, which belongs to
 * no impl.
 */
interface WasmTables {
	/** The span-only wire — the one parse wire every tsv binding emits. */
	parse: Record<Language, (source: string, source_type?: string) => unknown>;
	parse_internal: Record<Language, (source: string, source_type?: string) => void>;
	format: Record<Language, (source: string, source_type?: string) => string>;
}

export class WasmImplementation extends BaseImplementation {
	private _module: WasmModule | null = null;
	private _tables: WasmTables | null = null;

	readonly parse_languages = LANGUAGES;
	readonly format_languages = LANGUAGES;

	/** Get initialized module or throw */
	private get module(): WasmModule {
		if (!this._module) throw new Error('WASM module not initialized');
		return this._module;
	}

	/** The per-language export tables, or throw if `init()` hasn't run. */
	private get tables(): WasmTables {
		if (!this._tables) throw new Error('WASM module not initialized');
		return this._tables;
	}

	async init(): Promise<void> {
		// The same directory the freshness guard resolves — both sides go through
		// `tsv_artifacts.ts`'s `wasm_bundle_dir`, so the bundle guarded is the bundle
		// loaded (the guard names the `.wasm`, this names the `.js` glue beside it).
		// It was two spellings of one layout under a comment already claiming
		// otherwise, which is the shape that lets a guard vouch for a file nothing
		// opens.
		const target = wasm_target();
		const wasm_path = `${wasm_bundle_dir('all', target)}/tsv_wasm.js`;

		try {
			await stat(wasm_path);
		} catch {
			throw new Error(
				`WASM module not found at ${wasm_path}. ` +
					`Run 'deno task build:wasm:all:${target}' first.`
			);
		}

		// The deno target is ESM with an explicit `default()` initializer; the
		// nodejs target is CommonJS and self-initializes on require. Load each in
		// its native module system (both resolve to `any`), then read the same
		// function names off both into the typed `WasmModule` shape below.
		let module: WasmModule;
		if (target === 'deno') {
			const esm = await import(wasm_path);
			if (typeof esm.default === 'function') {
				await esm.default();
			}
			module = esm;
		} else {
			module = createRequire(import.meta.url)(wasm_path);
		}

		this._module = {
			parse_svelte: module.parse_svelte,
			parse_internal_svelte: module.parse_internal_svelte,
			format_svelte: module.format_svelte,
			parse_typescript: module.parse_typescript,
			parse_internal_typescript: module.parse_internal_typescript,
			format_typescript: module.format_typescript,
			parse_css: module.parse_css,
			parse_internal_css: module.parse_internal_css,
			format_css: module.format_css
		};

		// Resolve every export table once — see `WasmTables`.
		this._tables = {
			parse: {
				svelte: this._module.parse_svelte,
				typescript: this._module.parse_typescript,
				css: this._module.parse_css
			},
			parse_internal: {
				svelte: this._module.parse_internal_svelte,
				typescript: this._module.parse_internal_typescript,
				css: this._module.parse_internal_css
			},
			format: {
				svelte: this._module.format_svelte,
				typescript: this._module.format_typescript,
				css: this._module.format_css
			}
		};

		// Fairness guard for the parse rows: the wasm parse fns must return a
		// js_sys-materialized OBJECT (the engine runs the host's JSON.parse from
		// Rust). If a glue/build regression ever handed back the raw JSON string
		// instead, the timed `tsv-wasm-json-no-locations` rows would silently skip
		// materialization and read artificially fast vs their native sibling. Probe
		// once here, outside any timed loop.
		const probe = this._module.parse_typescript('const x = 1;');
		if (typeof probe !== 'object' || probe === null) {
			throw new Error(
				`tsv-wasm parse returned a ${typeof probe} — expected a materialized AST object`
			);
		}

		// The bindings throw natively today; probed anyway so the three can't come to
		// disagree about what surfacing a refusal MEANS — see `lib/reject_probe.ts`.
		// The guard above asks what a SUCCESS returns; this asks what a REFUSAL does.
		assert_binding_reports_rejection('tsv (WASM)', this);

		// The parse wire is span-only — prove this bundle emits it, so a stale one whose
		// `parse_<lang>` still returns the loc-bearing wire can't be timed under the span
		// rows' label. The
		// freshness guard refuses such a bundle unless `BENCH_STALE_OK=1`; this is what
		// still stands then — see `lib/locations_probe.ts`.
		assert_binding_emits_span_only(
			'tsv (WASM)',
			{ path: wasm_path, rebuild: `deno task build:wasm:all:${target}` },
			this
		);
	}

	// `goal_for` withholds the goal for svelte/css, which REJECT a set one rather than
	// ignoring it (`crates/tsv_wasm/src/lib.rs`'s `wasm_source_type`). The same helper
	// the two native wrappers use; see its doc in `lib/types.ts`.
	parse(source: string, language: Language, goal?: ParseGoal): unknown {
		return this.tables.parse[language](source, goal_for(language, goal));
	}

	parse_internal(source: string, language: Language, goal?: ParseGoal): void {
		this.tables.parse_internal[language](source, goal_for(language, goal));
	}

	// No source type: the shipped default on every surface, and the one that reaches
	// the module-then-script fallback (`tsv_ts::parse_with_goal_or_fallback`).
	format(source: string, language: Language): string {
		return this.tables.format[language](source);
	}

	dispose(): void {
		this._module = null;
		this._tables = null;
	}
}
