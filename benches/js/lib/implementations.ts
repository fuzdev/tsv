/**
 * Benchmark implementation management.
 *
 * Centralizes initialization and access to parser/formatter implementations.
 * This module provides a clean interface for bench.ts to work with implementations
 * without needing to know the details of each one.
 *
 * Future: Could evolve into a registry pattern where implementations self-register,
 * enabling dynamic discovery and plugin-like architecture.
 */

import type { Language, Logger, ParseGoal, TsvImplementation } from './types.ts';
import { CanonicalImplementation } from './canonical.ts';
import { NativeImplementation } from './ffi.ts';
import { NapiImplementation } from './napi.ts';
import { WasmImplementation } from './wasm.ts';
import { current_runtime } from './runtime.ts';
import { OxcImplementation } from './oxc.ts';
import { OxcWasmImplementation } from './oxc_wasm.ts';
import { BiomeImplementation } from './biome.ts';
import { type AllVersions, load_all_versions } from './versions.ts';

export type { TsvImplementation };

/** Result of initializing implementations */
export interface InitializedImplementations {
	/** All package versions */
	versions: AllVersions;
	/** Canonical implementation (prettier + svelte/compiler) - always available */
	canonical: CanonicalImplementation;
	/** Native implementation — FFI under Deno, N-API under Node/Bun; undefined if not built */
	native: NativeImplementation | NapiImplementation | undefined;
	/** WASM implementation - undefined if not built */
	wasm: WasmImplementation | undefined;
	/** OXC implementation (oxc-parser + oxfmt) - undefined if not available */
	oxc: OxcImplementation | undefined;
	/** OXC WASM implementation (oxc-parser via wasm32-wasi) - undefined if not available */
	oxc_wasm: OxcWasmImplementation | undefined;
	/** Biome implementation (via WASM) - undefined if not available */
	biome: BiomeImplementation | undefined;
}

/** Options for implementation initialization */
export interface InitOptions {
	/** Logger for status messages */
	logger?: Logger;
	/** Whether to skip missing implementations (default: true) */
	skip_missing?: boolean;
	/** Whether canonical is required (default: true) */
	require_canonical?: boolean;
}

/**
 * Initialize all benchmark implementations.
 *
 * @example
 * ```ts
 * const impls = await init_implementations({ logger: console.log });
 * if (impls.native) {
 *   const result = impls.native.format(source, 'svelte');
 * }
 * ```
 */
export async function init_implementations(
	options: InitOptions = {},
): Promise<InitializedImplementations> {
	const { logger = console.log, skip_missing = true, require_canonical = true } = options;

	// Load all versions once from package.json
	const versions = await load_all_versions();

	// The native path is runtime-specific: Deno loads the C-FFI library via
	// Deno.dlopen; Node/Bun load the N-API addon via process.dlopen. Same engine,
	// different binding boundary — one is instantiated per runtime.
	const is_deno = current_runtime() === 'deno';
	const native_label = is_deno ? 'Native (FFI)' : 'Native (N-API)';

	const canonical = new CanonicalImplementation(versions.canonical);
	const native = is_deno ? new NativeImplementation() : new NapiImplementation();
	const wasm = new WasmImplementation();

	logger('Initializing implementations...');

	// Initialize canonical (required by default)
	try {
		await canonical.init();
		logger('  ✓ Canonical (prettier + svelte/compiler)');
	} catch (e) {
		if (require_canonical) {
			logger(`  ✗ Canonical: ${e}`);
			throw e;
		}
		logger(`  ⚠ Canonical: ${e}`);
	}

	// Initialize native (optional)
	let native_impl: NativeImplementation | NapiImplementation | undefined;
	try {
		await native.init();
		logger(`  ✓ ${native_label}`);
		native_impl = native;
	} catch (e) {
		if (skip_missing) {
			logger(`  ⚠ ${native_label}: not available`);
		} else {
			throw e;
		}
	}

	// Initialize WASM (optional)
	let wasm_impl: WasmImplementation | undefined;
	try {
		await wasm.init();
		logger('  ✓ WASM');
		wasm_impl = wasm;
	} catch (e) {
		if (skip_missing) {
			logger(`  ⚠ WASM: not available`);
		} else {
			throw e;
		}
	}

	// Initialize OXC (optional)
	let oxc_impl: OxcImplementation | undefined;
	const oxc = new OxcImplementation(versions.oxc);
	try {
		await oxc.init();
		logger('  ✓ OXC (oxc-parser + oxfmt)');
		oxc_impl = oxc;
	} catch (e) {
		if (skip_missing) {
			logger(`  ⚠ OXC: not available`);
		} else {
			throw e;
		}
	}

	// Initialize OXC WASM (optional)
	let oxc_wasm_impl: OxcWasmImplementation | undefined;
	const oxc_wasm = new OxcWasmImplementation(versions.oxc);
	try {
		await oxc_wasm.init();
		logger('  ✓ OXC WASM (oxc-parser)');
		oxc_wasm_impl = oxc_wasm;
	} catch (e) {
		if (skip_missing) {
			logger(`  ⚠ OXC WASM: not available`);
		} else {
			throw e;
		}
	}

	// Initialize Biome (optional)
	let biome_impl: BiomeImplementation | undefined;
	const biome = new BiomeImplementation(versions.biome);
	try {
		await biome.init();
		logger('  ✓ Biome (WASM)');
		biome_impl = biome;
	} catch (e) {
		if (skip_missing) {
			logger(`  ⚠ Biome: not available`);
		} else {
			throw e;
		}
	}

	logger('');

	return {
		versions,
		canonical,
		native: native_impl,
		wasm: wasm_impl,
		oxc: oxc_impl,
		oxc_wasm: oxc_wasm_impl,
		biome: biome_impl,
	};
}

/** A benchmark task definition */
export interface BenchmarkTask {
	/** Display name in benchmark output */
	name: string;
	/** Key for corpus size tracking (e.g., "parse/svelte/native") */
	tracking_key: string;
	/** Whether this benchmark runs async */
	is_async: boolean;
	/** The benchmark function - processes all files once. `goal` (TS-only, from
	 * the conformance surface's test262 files) selects the parse goal; parse tasks
	 * forward it, format tasks ignore it. */
	run: (source: string, language: Language, goal?: ParseGoal) => unknown;
	/** Async version if is_async is true */
	run_async?: (source: string, language: Language, goal?: ParseGoal) => Promise<unknown>;
}

/** Options controlling which optional/diagnostic tasks are included. */
export interface BenchmarkTaskOptions {
	/**
	 * Include the `tsv-forced-async` control row in the format groups (default
	 * off; opt in via `BENCH_FORCED_ASYNC=1`). Not a real impl — the same native
	 * engine as `tsv`, routed through the awaited async path to measure the
	 * per-file await tax the async-only impls (`prettier`, `oxfmt`) pay. That tax
	 * sits below the run-to-run noise floor, so the row is kept OUT of the
	 * published `report.<runtime>.{json,md}` and the regression baseline (where a
	 * noise-level delta would throw spurious flags) — it's an on-demand
	 * re-confirmation tool, not a standing measurement.
	 */
	forced_async?: boolean;
}

/**
 * Get all benchmark tasks for a specific operation and language.
 * Returns tasks in display order (canonical first, then alternatives).
 */
export function get_benchmark_tasks(
	impls: InitializedImplementations,
	operation: 'parse' | 'format',
	language: Language,
	options: BenchmarkTaskOptions = {},
): BenchmarkTask[] {
	const tasks: BenchmarkTask[] = [];
	const group_name = `${operation}/${language}`;

	if (operation === 'parse') {
		// Canonical parser (always available)
		tasks.push({
			name: canonical_parser_label(language),
			tracking_key: `${group_name}/canonical`,
			is_async: false,
			run: (source, _language, goal) => impls.canonical.parse(source, language, goal),
		});

		// Native parser (with JSON serialization)
		if (impls.native) {
			tasks.push({
				name: 'tsv-json',
				tracking_key: `${group_name}/native`,
				is_async: false,
				run: (source, _language, goal) => impls.native!.parse(source, language, goal),
			});
		}

		// Native parser, no-locations wire (span-only: no per-node `loc`). The
		// payload-matched opponent to oxc-parser, whose default AST is also
		// span-only. CSS is skipped — `parseCss` emits no `loc`, so a CSS
		// no-locations row would duplicate `tsv-json`.
		if (impls.native?.parse_no_locations && language !== 'css') {
			tasks.push({
				name: 'tsv-json-no-locations',
				tracking_key: `${group_name}/native-no-locations`,
				is_async: false,
				run: (source, _language, goal) =>
					impls.native!.parse_no_locations!(source, language, goal),
			});
		}

		// WASM parser (with JSON serialization)
		if (impls.wasm) {
			tasks.push({
				name: 'tsv_wasm-json',
				tracking_key: `${group_name}/wasm`,
				is_async: false,
				run: (source, _language, goal) => impls.wasm!.parse(source, language, goal),
			});
		}

		// WASM parser, no-locations wire (span-only). Materialized in Rust like
		// `tsv_wasm-json`, so the two are mechanism-matched. CSS skipped (no `loc`).
		if (impls.wasm?.parse_no_locations && language !== 'css') {
			tasks.push({
				name: 'tsv_wasm-json-no-locations',
				tracking_key: `${group_name}/wasm-no-locations`,
				is_async: false,
				run: (source, _language, goal) =>
					impls.wasm!.parse_no_locations!(source, language, goal),
			});
		}

		// Internal parsing variants (no JSON serialization) - shows JSON overhead
		if (impls.native?.parse_internal) {
			tasks.push({
				name: 'tsv-internal',
				tracking_key: `${group_name}/native-internal`,
				is_async: false,
				run: (source, _language, goal) =>
					impls.native!.parse_internal!(source, language, goal),
			});
		}

		if (impls.wasm?.parse_internal) {
			tasks.push({
				name: 'tsv_wasm-internal',
				tracking_key: `${group_name}/wasm-internal`,
				is_async: false,
				run: (source, _language, goal) =>
					impls.wasm!.parse_internal!(source, language, goal),
			});
		}

		// OXC parser (TypeScript/JS only) — default mode: serializes to JSON in Rust
		// then JSON.parses in JS, eagerly materializing the full AST (the like-for-like
		// opponent to tsv-json). There is intentionally no `oxc-parser-lazy` row: oxc's
		// `experimentalLazy` raw transfer is setup-dominated in every runtime (measures
		// buffer copy, not parse speed) — see `lib/oxc.ts` and CLAUDE.md → Fairness Caveats.
		if (impls.oxc?.supports_parse_language(language)) {
			tasks.push({
				name: 'oxc-parser',
				tracking_key: `${group_name}/oxc`,
				is_async: false,
				run: (source, _language, goal) => impls.oxc!.parse(source, language, goal),
			});
		}

		// OXC WASM parser (TypeScript/JS only)
		if (impls.oxc_wasm?.supports_parse_language(language)) {
			tasks.push({
				name: 'oxc-parser-wasm',
				tracking_key: `${group_name}/oxc-wasm`,
				is_async: false,
				run: (source, _language, goal) => impls.oxc_wasm!.parse(source, language, goal),
			});
		}
	} else {
		// Canonical formatter (prettier) - async
		tasks.push({
			name: 'prettier',
			tracking_key: `${group_name}/canonical`,
			is_async: true,
			run: () => {
				throw new Error('Use run_async for prettier');
			},
			run_async: (source) => impls.canonical.format_async(source, language),
		});

		// Native formatter
		if (impls.native?.format) {
			tasks.push({
				name: 'tsv',
				tracking_key: `${group_name}/native`,
				is_async: false,
				run: (source) => impls.native!.format!(source, language),
			});
		}

		// Forced-async control (opt-in). Same native engine as `tsv`, routed through
		// the awaited async path so the `tsv` vs `tsv-forced-async` delta measures the
		// per-file await tax; `Promise.resolve` wraps the already-computed result, so
		// the only added cost is the await. Rationale + why it's off by default:
		// `BenchmarkTaskOptions.forced_async`.
		if (options.forced_async && impls.native?.format) {
			tasks.push({
				name: 'tsv-forced-async',
				tracking_key: `${group_name}/native-forced-async`,
				is_async: true,
				run: () => {
					throw new Error('Use run_async for tsv-forced-async');
				},
				run_async: (source, language) =>
					Promise.resolve(impls.native!.format!(source, language)),
			});
		}

		// WASM formatter
		if (impls.wasm?.format) {
			tasks.push({
				name: 'tsv_wasm',
				tracking_key: `${group_name}/wasm`,
				is_async: false,
				run: (source) => impls.wasm!.format!(source, language),
			});
		}

		// OXC formatter (TypeScript/JS/CSS only) - async
		if (impls.oxc?.supports_format_language(language)) {
			tasks.push({
				name: 'oxfmt',
				tracking_key: `${group_name}/oxfmt`,
				is_async: true,
				run: () => {
					throw new Error('Use run_async for oxfmt');
				},
				run_async: (source) => impls.oxc!.format_async(source, language),
			});
		}

		// Biome formatter
		if (impls.biome?.supports_format_language(language)) {
			tasks.push({
				name: 'biome-wasm',
				tracking_key: `${group_name}/biome`,
				is_async: false,
				run: (source) => impls.biome!.format(source, language),
			});
		}
	}

	return tasks;
}

/** Get canonical parser label for a language */
export function canonical_parser_label(lang: Language): string {
	switch (lang) {
		case 'svelte':
			return 'svelte/compiler';
		case 'typescript':
			return 'acorn-typescript';
		case 'css':
			return 'svelte/compiler';
	}
}

/** Uniform formatter handle (sync or async, with per-language support gate) */
export interface FormatterInfo {
	name: string;
	is_async: boolean;
	format?: (source: string, language: Language) => string;
	format_async?: (source: string, language: Language) => Promise<string>;
	supports_language: (language: Language) => boolean;
}

/**
 * Collect every available formatter wrapped in a uniform handle.
 * Used by the smoke test (`deno task smoke`). Preserves the sync/async
 * distinction — callers should branch on `is_async`.
 */
export function get_formatters(impls: InitializedImplementations): FormatterInfo[] {
	const formatters: FormatterInfo[] = [];

	// Canonical (prettier) - async
	formatters.push({
		name: 'prettier',
		is_async: true,
		format_async: (source, lang) => impls.canonical.format_async(source, lang),
		supports_language: () => true,
	});

	// Native - sync
	if (impls.native?.format) {
		formatters.push({
			name: 'tsv',
			is_async: false,
			format: (source, lang) => impls.native!.format!(source, lang),
			supports_language: () => true,
		});
	}

	// WASM - sync
	if (impls.wasm?.format) {
		formatters.push({
			name: 'tsv_wasm',
			is_async: false,
			format: (source, lang) => impls.wasm!.format!(source, lang),
			supports_language: () => true,
		});
	}

	// OXC (oxfmt) - async
	if (impls.oxc) {
		formatters.push({
			name: 'oxfmt',
			is_async: true,
			format_async: (source, lang) => impls.oxc!.format_async(source, lang),
			supports_language: (lang) => impls.oxc!.supports_format_language(lang),
		});
	}

	// Biome - sync
	if (impls.biome) {
		formatters.push({
			name: 'biome-wasm',
			is_async: false,
			format: (source, lang) => impls.biome!.format(source, lang),
			supports_language: (lang) => impls.biome!.supports_format_language(lang),
		});
	}

	return formatters;
}

/** Version info for available alternative implementations */
export interface AlternativeVersions {
	oxc_parser?: string;
	oxfmt?: string;
	biome?: string;
}

/**
 * Get version info for available alternative implementations.
 * Only includes versions for implementations that initialized successfully.
 */
export function get_alternative_versions(impls: InitializedImplementations): AlternativeVersions {
	return {
		oxc_parser: impls.oxc?.versions['oxc-parser'],
		oxfmt: impls.oxc?.versions.oxfmt,
		biome: impls.biome?.versions.wasm,
	};
}
