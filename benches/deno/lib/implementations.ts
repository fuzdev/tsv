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

import type { Language, Logger, TsvImplementation } from './types.ts';
import { CanonicalImplementation } from './canonical.ts';
import { NativeImplementation } from './ffi.ts';
import { WasmImplementation } from './wasm.ts';
import { OxcImplementation } from './oxc.ts';
import { OxcWasmImplementation } from './oxc_wasm.ts';
import { BiomeImplementation } from './biome.ts';
import { type AllVersions, loadAllVersions } from './versions.ts';

export type { TsvImplementation };

/** Result of initializing implementations */
export interface InitializedImplementations {
	/** All package versions */
	versions: AllVersions;
	/** Canonical implementation (prettier + svelte/compiler) - always available */
	canonical: CanonicalImplementation;
	/** Native FFI implementation - undefined if not built */
	native: NativeImplementation | undefined;
	/** WASM implementation - undefined if not built */
	wasm: WasmImplementation | undefined;
	/** OXC implementation (oxc-parser + oxfmt) - undefined if not available */
	oxc: OxcImplementation | undefined;
	/** OXC WASM implementation (oxc-parser via wasm32-wasi) - undefined if not available */
	oxcWasm: OxcWasmImplementation | undefined;
	/** Biome implementation (via WASM) - undefined if not available */
	biome: BiomeImplementation | undefined;
}

/** Options for implementation initialization */
export interface InitOptions {
	/** Logger for status messages */
	logger?: Logger;
	/** Whether to skip missing implementations (default: true) */
	skipMissing?: boolean;
	/** Whether canonical is required (default: true) */
	requireCanonical?: boolean;
}

/**
 * Initialize all benchmark implementations.
 *
 * @example
 * ```ts
 * const impls = await initImplementations({ logger: console.log });
 * if (impls.native) {
 *   const result = impls.native.format(source, 'svelte');
 * }
 * ```
 */
export async function initImplementations(
	options: InitOptions = {},
): Promise<InitializedImplementations> {
	const { logger = console.log, skipMissing = true, requireCanonical = true } = options;

	// Load all versions once from deno.json
	const versions = await loadAllVersions();

	const canonical = new CanonicalImplementation(versions.canonical);
	const native = new NativeImplementation();
	const wasm = new WasmImplementation();

	logger('Initializing implementations...');

	// Initialize canonical (required by default)
	try {
		await canonical.init();
		logger('  ✓ Canonical (prettier + svelte/compiler)');
	} catch (e) {
		if (requireCanonical) {
			logger(`  ✗ Canonical: ${e}`);
			throw e;
		}
		logger(`  ⚠ Canonical: ${e}`);
	}

	// Initialize native (optional)
	let nativeImpl: NativeImplementation | undefined;
	try {
		await native.init();
		logger('  ✓ Native (FFI)');
		nativeImpl = native;
	} catch (e) {
		if (skipMissing) {
			logger(`  ⚠ Native (FFI): not available`);
		} else {
			throw e;
		}
	}

	// Initialize WASM (optional)
	let wasmImpl: WasmImplementation | undefined;
	try {
		await wasm.init();
		logger('  ✓ WASM');
		wasmImpl = wasm;
	} catch (e) {
		if (skipMissing) {
			logger(`  ⚠ WASM: not available`);
		} else {
			throw e;
		}
	}

	// Initialize OXC (optional)
	let oxcImpl: OxcImplementation | undefined;
	const oxc = new OxcImplementation(versions.oxc);
	try {
		await oxc.init();
		logger('  ✓ OXC (oxc-parser + oxfmt)');
		oxcImpl = oxc;
	} catch (e) {
		if (skipMissing) {
			logger(`  ⚠ OXC: not available`);
		} else {
			throw e;
		}
	}

	// Initialize OXC WASM (optional)
	let oxcWasmImpl: OxcWasmImplementation | undefined;
	const oxcWasm = new OxcWasmImplementation(versions.oxc);
	try {
		await oxcWasm.init();
		logger('  ✓ OXC WASM (oxc-parser)');
		oxcWasmImpl = oxcWasm;
	} catch (e) {
		if (skipMissing) {
			logger(`  ⚠ OXC WASM: not available`);
		} else {
			throw e;
		}
	}

	// Initialize Biome (optional)
	let biomeImpl: BiomeImplementation | undefined;
	const biome = new BiomeImplementation(versions.biome);
	try {
		await biome.init();
		logger('  ✓ Biome (WASM)');
		biomeImpl = biome;
	} catch (e) {
		if (skipMissing) {
			logger(`  ⚠ Biome: not available`);
		} else {
			throw e;
		}
	}

	logger('');

	return {
		versions,
		canonical,
		native: nativeImpl,
		wasm: wasmImpl,
		oxc: oxcImpl,
		oxcWasm: oxcWasmImpl,
		biome: biomeImpl,
	};
}

/** A benchmark task definition */
export interface BenchmarkTask {
	/** Display name in benchmark output */
	name: string;
	/** Key for corpus size tracking (e.g., "parse/svelte/native") */
	trackingKey: string;
	/** Whether this benchmark runs async */
	isAsync: boolean;
	/** The benchmark function - processes all files once */
	run: (source: string, language: Language) => unknown;
	/** Async version if isAsync is true */
	runAsync?: (source: string, language: Language) => Promise<unknown>;
}

/**
 * Get all benchmark tasks for a specific operation and language.
 * Returns tasks in display order (canonical first, then alternatives).
 */
export function getBenchmarkTasks(
	impls: InitializedImplementations,
	operation: 'parse' | 'format',
	language: Language,
): BenchmarkTask[] {
	const tasks: BenchmarkTask[] = [];
	const groupName = `${operation}/${language}`;

	if (operation === 'parse') {
		// Canonical parser (always available)
		tasks.push({
			name: canonicalParserLabel(language),
			trackingKey: `${groupName}/canonical`,
			isAsync: false,
			run: (source) => impls.canonical.parse(source, language),
		});

		// Native parser (with JSON serialization)
		if (impls.native) {
			tasks.push({
				name: 'tsv-json',
				trackingKey: `${groupName}/native`,
				isAsync: false,
				run: (source) => impls.native!.parse(source, language),
			});
		}

		// WASM parser (with JSON serialization)
		if (impls.wasm) {
			tasks.push({
				name: 'tsv_wasm-json',
				trackingKey: `${groupName}/wasm`,
				isAsync: false,
				run: (source) => impls.wasm!.parse(source, language),
			});
		}

		// Internal parsing variants (no JSON serialization) - shows JSON overhead
		if (impls.native?.parseInternal) {
			tasks.push({
				name: 'tsv-internal',
				trackingKey: `${groupName}/native-internal`,
				isAsync: false,
				run: (source) => impls.native!.parseInternal!(source, language),
			});
		}

		if (impls.wasm?.parseInternal) {
			tasks.push({
				name: 'tsv_wasm-internal',
				trackingKey: `${groupName}/wasm-internal`,
				isAsync: false,
				run: (source) => impls.wasm!.parseInternal!(source, language),
			});
		}

		// OXC parser (TypeScript/JS only)
		if (impls.oxc?.supportsParseLanguage(language)) {
			tasks.push({
				name: 'oxc-parser',
				trackingKey: `${groupName}/oxc`,
				isAsync: false,
				run: (source) => impls.oxc!.parse(source, language),
			});
		}

		// OXC WASM parser (TypeScript/JS only)
		if (impls.oxcWasm?.supportsParseLanguage(language)) {
			tasks.push({
				name: 'oxc-parser-wasm',
				trackingKey: `${groupName}/oxc-wasm`,
				isAsync: false,
				run: (source) => impls.oxcWasm!.parse(source, language),
			});
		}
	} else {
		// Canonical formatter (prettier) - async
		tasks.push({
			name: 'prettier',
			trackingKey: `${groupName}/canonical`,
			isAsync: true,
			run: () => {
				throw new Error('Use runAsync for prettier');
			},
			runAsync: (source) => impls.canonical.formatAsync(source, language),
		});

		// Native formatter
		if (impls.native?.format) {
			tasks.push({
				name: 'tsv',
				trackingKey: `${groupName}/native`,
				isAsync: false,
				run: (source) => impls.native!.format!(source, language),
			});
		}

		// WASM formatter
		if (impls.wasm?.format) {
			tasks.push({
				name: 'tsv_wasm',
				trackingKey: `${groupName}/wasm`,
				isAsync: false,
				run: (source) => impls.wasm!.format!(source, language),
			});
		}

		// OXC formatter (TypeScript/JS/CSS only) - async
		if (impls.oxc?.supportsFormatLanguage(language)) {
			tasks.push({
				name: 'oxfmt',
				trackingKey: `${groupName}/oxfmt`,
				isAsync: true,
				run: () => {
					throw new Error('Use runAsync for oxfmt');
				},
				runAsync: (source) => impls.oxc!.formatAsync(source, language),
			});
		}

		// Biome formatter
		if (impls.biome?.supportsFormatLanguage(language)) {
			tasks.push({
				name: 'biome-wasm',
				trackingKey: `${groupName}/biome`,
				isAsync: false,
				run: (source) => impls.biome!.format(source, language),
			});
		}
	}

	return tasks;
}

/** Get canonical parser label for a language */
export function canonicalParserLabel(lang: Language): string {
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
	isAsync: boolean;
	format?: (source: string, language: Language) => string;
	formatAsync?: (source: string, language: Language) => Promise<string>;
	supportsLanguage: (language: Language) => boolean;
}

/**
 * Collect every available formatter wrapped in a uniform handle.
 * Used by the smoke test (`deno task smoke`). Preserves the sync/async
 * distinction — callers should branch on `isAsync`.
 */
export function getFormatters(impls: InitializedImplementations): FormatterInfo[] {
	const formatters: FormatterInfo[] = [];

	// Canonical (prettier) - async
	formatters.push({
		name: 'prettier',
		isAsync: true,
		formatAsync: (source, lang) => impls.canonical.formatAsync(source, lang),
		supportsLanguage: () => true,
	});

	// Native - sync
	if (impls.native?.format) {
		formatters.push({
			name: 'tsv',
			isAsync: false,
			format: (source, lang) => impls.native!.format!(source, lang),
			supportsLanguage: () => true,
		});
	}

	// WASM - sync
	if (impls.wasm?.format) {
		formatters.push({
			name: 'tsv_wasm',
			isAsync: false,
			format: (source, lang) => impls.wasm!.format!(source, lang),
			supportsLanguage: () => true,
		});
	}

	// OXC (oxfmt) - async
	if (impls.oxc) {
		formatters.push({
			name: 'oxfmt',
			isAsync: true,
			formatAsync: (source, lang) => impls.oxc!.formatAsync(source, lang),
			supportsLanguage: (lang) => impls.oxc!.supportsFormatLanguage(lang),
		});
	}

	// Biome - sync
	if (impls.biome) {
		formatters.push({
			name: 'biome-wasm',
			isAsync: false,
			format: (source, lang) => impls.biome!.format(source, lang),
			supportsLanguage: (lang) => impls.biome!.supportsFormatLanguage(lang),
		});
	}

	return formatters;
}

/** Version info for available alternative implementations */
export interface AlternativeVersions {
	oxcParser?: string;
	oxfmt?: string;
	biome?: string;
}

/**
 * Get version info for available alternative implementations.
 * Only includes versions for implementations that initialized successfully.
 */
export function getAlternativeVersions(impls: InitializedImplementations): AlternativeVersions {
	return {
		oxcParser: impls.oxc?.versions['oxc-parser'],
		oxfmt: impls.oxc?.versions.oxfmt,
		biome: impls.biome?.versions.wasm,
	};
}
