/**
 * WASM bindings to tsv
 *
 * Uses wasm-pack generated bindings for WebAssembly performance testing.
 * Runtime-aware: each runtime loads its own wasm-pack *target* bundle (same
 * `tsv_wasm_bg.wasm`, different JS glue), both carrying the full export set
 * including the benchmark-only `parse_internal_*`:
 *  - Deno: the `deno` target (ESM; explicit `default()` init)
 *  - Node/Bun: the `nodejs` target (CommonJS; self-initializing on require)
 * The shipped `@fuzdev/tsv_wasm` (web) bundle is deliberately NOT used here — it
 * curates out `parse_internal_*`, which the `tsv_wasm-internal` row needs.
 */

import { stat } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { wasm_target } from './runtime.ts';
import { wasm_bundle_dir } from './tsv_artifacts.ts';
import { BaseImplementation, type Language, LANGUAGES, type ParseGoal } from './types.ts';
import { assert_binding_reports_rejection } from './reject_probe.ts';

/** The `{locations?, goal?}` options bag the parse exports take (`goal` is
 * TypeScript-only — the other languages reject the key). */
interface WasmParseOptions {
	locations?: boolean;
	goal?: ParseGoal;
}

/** The `{goal?}` bag the format exports take — format emits no wire, so
 * `locations` is an unknown key there, and `goal` stays TypeScript-only. */
interface WasmFormatOptions {
	goal?: ParseGoal;
}

/** WASM module function signatures */
interface WasmModule {
	parse_svelte: (source: string, options?: WasmParseOptions) => unknown;
	parse_internal_svelte: (source: string, options?: WasmParseOptions) => void;
	format_svelte: (source: string, options?: WasmFormatOptions) => string;
	parse_typescript: (source: string, options?: WasmParseOptions) => unknown;
	parse_internal_typescript: (source: string, options?: WasmParseOptions) => void;
	format_typescript: (source: string, options?: WasmFormatOptions) => string;
	parse_css: (source: string, options?: WasmParseOptions) => unknown;
	parse_internal_css: (source: string, options?: WasmParseOptions) => void;
	format_css: (source: string, options?: WasmFormatOptions) => string;
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
	parse: Record<Language, (source: string, options?: WasmParseOptions) => unknown>;
	parse_internal: Record<Language, (source: string, options?: WasmParseOptions) => void>;
	format: Record<Language, (source: string, options?: WasmFormatOptions) => string>;
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
		// instead, the timed `tsv_wasm-json` rows would silently skip
		// materialization and read artificially fast vs `tsv-json`. Probe once
		// here, outside any timed loop.
		const probe = this._module.parse_typescript('const x = 1;');
		if (typeof probe !== 'object' || probe === null) {
			throw new Error(
				`tsv_wasm parse returned a ${typeof probe} — expected a materialized AST object`
			);
		}

		// The bindings throw natively today; probed anyway so the three can't come to
		// disagree about what surfacing a refusal MEANS — see `lib/reject_probe.ts`.
		// The guard above asks what a SUCCESS returns; this asks what a REFUSAL does.
		assert_binding_reports_rejection('tsv (WASM)', this);
	}

	// `goal` is TypeScript's alone — the other languages reject a SET goal, not
	// the key itself: `crates/tsv_wasm/src/lib.rs` declares `goal?: undefined` on
	// `ParseOptions` precisely so one options bag can be forwarded to whichever
	// export (the documented forwarding idiom `npm/cli.js` uses). So `parse` withholds
	// the bag for tidiness, and `parse_no_locations` below spells the inapplicable
	// goal `undefined` — both legal, neither one relying on the other's rule.
	parse(source: string, language: Language, goal?: ParseGoal): unknown {
		return this.tables.parse[language](
			source,
			goal && language === 'typescript' ? { goal } : undefined
		);
	}

	parse_internal(source: string, language: Language, goal?: ParseGoal): void {
		this.tables.parse_internal[language](
			source,
			goal && language === 'typescript' ? { goal } : undefined
		);
	}

	parse_no_locations(source: string, language: Language, goal?: ParseGoal): unknown {
		return this.tables.parse[language](source, {
			locations: false,
			goal: goal && language === 'typescript' ? goal : undefined
		});
	}

	format(source: string, language: Language): string {
		return this.tables.format[language](source);
	}

	dispose(): void {
		this._module = null;
		this._tables = null;
	}
}
