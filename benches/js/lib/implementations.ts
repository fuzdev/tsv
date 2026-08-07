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
import { YukuImplementation } from './yuku.ts';
import { BiomeImplementation } from './biome.ts';
import { DprintImplementation } from './dprint.ts';
import { RsvelteImplementation } from './rsvelte.ts';
import type { AlternativeVersionInfo } from './report.ts';
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
	/** yuku-parser (N-API) - parse only; undefined if not available */
	yuku: YukuImplementation | undefined;
	/** yuku-parser (WASM) - parse only; undefined if not available */
	yuku_wasm: YukuImplementation | undefined;
	/** Biome implementation (via WASM) - undefined if not available */
	biome: BiomeImplementation | undefined;
	/** dprint implementation (via WASM; the engine `deno fmt` runs) - undefined if not available */
	dprint: DprintImplementation | undefined;
	/**
	 * rsvelte-fmt implementation (native binary, one process per file) - undefined
	 * if this platform has no prebuilt binary. Coverage-only: it has no in-process
	 * API, so it is never timed (see `get_benchmark_tasks`).
	 */
	rsvelte: RsvelteImplementation | undefined;
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
 * Initialize one OPTIONAL implementation, returning it on success and `undefined`
 * when it isn't available on this machine.
 *
 * Every alternative impl is optional in the same way — a missing platform binding,
 * an uninstalled package, a runtime that can't load a given wasm entry — so they
 * all want this exact try/catch. Sharing it is what keeps a new impl from arriving
 * with a subtly different failure posture (swallowing where the others rethrow,
 * say). `skip_missing: false` rethrows, which is how a caller demands a full set.
 *
 * `label` is the success line; `missing_label` names the impl in the ⚠ line when it
 * reads differently there (the ✓ lines carry a parenthetical the ⚠ lines don't).
 */
async function init_optional<T extends { init: () => Promise<void> }>(
	impl: T,
	label: string,
	logger: Logger,
	skip_missing: boolean,
	missing_label: string = label
): Promise<T | undefined> {
	try {
		await impl.init();
		logger(`  ✓ ${label}`);
		return impl;
	} catch (e) {
		if (!skip_missing) throw e;
		logger(`  ⚠ ${missing_label}: not available`);
		return undefined;
	}
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
	options: InitOptions = {}
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

	// Every impl below is optional and shares one failure posture — see `init_optional`.
	const optional = <T extends { init: () => Promise<void> }>(
		impl: T,
		label: string,
		missing_label?: string
	) => init_optional(impl, label, logger, skip_missing, missing_label);

	const native_impl = await optional(native, native_label);
	const wasm_impl = await optional(wasm, 'WASM');
	const oxc_impl = await optional(
		new OxcImplementation(versions.oxc),
		'OXC (oxc-parser + oxfmt)',
		'OXC'
	);
	const oxc_wasm_impl = await optional(
		new OxcWasmImplementation(versions.oxc),
		'OXC WASM (oxc-parser)',
		'OXC WASM'
	);
	// One class, two bindings — see lib/yuku.ts.
	const yuku_impl = await optional(
		new YukuImplementation('yuku-parser', versions.yuku),
		'yuku-parser (N-API)',
		'yuku-parser'
	);
	const yuku_wasm_impl = await optional(
		new YukuImplementation('yuku-parser-wasm', versions.yuku),
		'yuku-parser (WASM)',
		'yuku-parser WASM'
	);
	const biome_impl = await optional(
		new BiomeImplementation(versions.biome),
		'Biome (WASM)',
		'Biome'
	);
	const dprint_impl = await optional(
		new DprintImplementation(versions.dprint),
		'dprint (WASM)',
		'dprint'
	);
	const rsvelte_impl = await optional(
		new RsvelteImplementation(versions.rsvelte),
		'rsvelte-fmt (native binary, coverage-only)',
		'rsvelte-fmt'
	);

	logger('');

	return {
		versions,
		canonical,
		native: native_impl,
		wasm: wasm_impl,
		oxc: oxc_impl,
		oxc_wasm: oxc_wasm_impl,
		yuku: yuku_impl,
		yuku_wasm: yuku_wasm_impl,
		biome: biome_impl,
		dprint: dprint_impl,
		rsvelte: rsvelte_impl
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
	/**
	 * Measure this impl's pre-flight coverage but never TIME it. Set for an impl
	 * with no in-process API, where a timed row would be dominated by process
	 * spawn rather than format work (`rsvelte-fmt` — see `lib/rsvelte.ts`).
	 *
	 * `bench.ts` honors this in four places, and all four are load-bearing: the
	 * task is excluded from the timed loop, from the per-group **intersection**
	 * (so a file it rejects can't shrink the corpus the real rows are timed on),
	 * and from the perf 100%-coverage hard-fail (its sub-100% coverage IS the
	 * metric, not a regression); its row is then synthesized with null timing so
	 * the coverage still reaches the report.
	 */
	coverage_only?: boolean;
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
	/**
	 * Which corpus/surface this run measures (default `perf`). Only one task reads
	 * it — the `yuku-parser` N-API row, dropped on the `conformance` surface
	 * because that corpus carries inputs its native binding cannot survive. See the
	 * registration site and CLAUDE.md §Known Issues.
	 */
	corpus_kind?: 'perf' | 'conformance';
}

/**
 * Get all benchmark tasks for a specific operation and language.
 * Returns tasks in display order (canonical first, then alternatives).
 */
export function get_benchmark_tasks(
	impls: InitializedImplementations,
	operation: 'parse' | 'format',
	language: Language,
	options: BenchmarkTaskOptions = {}
): BenchmarkTask[] {
	const tasks: BenchmarkTask[] = [];
	const group_name = `${operation}/${language}`;

	/**
	 * Register a sync task when `enabled`. The gate differs per impl — a missing
	 * binding, an unsupported language, an absent optional method — so each caller
	 * passes its own; `true` is an impl that's always present.
	 */
	const add = (
		enabled: unknown,
		name: string,
		key: string,
		run: BenchmarkTask['run'],
		extra?: Pick<BenchmarkTask, 'coverage_only'>
	): void => {
		if (!enabled) return;
		tasks.push({
			name,
			tracking_key: `${group_name}/${key}`,
			is_async: false,
			run,
			...extra
		});
	};

	/**
	 * Register an async task. `BenchmarkTask` requires `run`, but callers branch on
	 * `is_async` and never call it on an async task — so the throwing stub is
	 * generated here once instead of hand-written per task, where its message was
	 * free to drift from the name it's supposed to identify.
	 */
	const add_async = (
		enabled: unknown,
		name: string,
		key: string,
		run_async: NonNullable<BenchmarkTask['run_async']>
	): void => {
		if (!enabled) return;
		tasks.push({
			name,
			tracking_key: `${group_name}/${key}`,
			is_async: true,
			run: () => {
				throw new Error(`${name} is async — use run_async`);
			},
			run_async
		});
	};

	if (operation === 'parse') {
		// Canonical parser (always available)
		add(true, canonical_parser_label(language), 'canonical', (source, _language, goal) =>
			impls.canonical.parse(source, language, goal)
		);

		// Native + WASM parsers (with JSON serialization)
		add(impls.native, 'tsv-json', 'native', (source, _language, goal) =>
			impls.native!.parse(source, language, goal)
		);
		add(impls.wasm, 'tsv_wasm-json', 'wasm', (source, _language, goal) =>
			impls.wasm!.parse(source, language, goal)
		);

		// The no-locations wire (span-only: no per-node `loc`) — the payload-matched
		// opponent to oxc-parser and yuku-parser, whose default ASTs are also
		// span-only. Materialized in Rust either side, so native and wasm stay
		// mechanism-matched to their `-json` siblings. CSS is skipped — `parseCss`
		// emits no `loc`, so a CSS no-locations row would duplicate `tsv-json`.
		add(
			impls.native?.parse_no_locations && language !== 'css',
			'tsv-json-no-locations',
			'native-no-locations',
			(source, _language, goal) => impls.native!.parse_no_locations!(source, language, goal)
		);
		add(
			impls.wasm?.parse_no_locations && language !== 'css',
			'tsv_wasm-json-no-locations',
			'wasm-no-locations',
			(source, _language, goal) => impls.wasm!.parse_no_locations!(source, language, goal)
		);

		// Internal parsing variants (no JSON serialization) - shows JSON overhead
		add(
			impls.native?.parse_internal,
			'tsv-internal',
			'native-internal',
			(source, _language, goal) => impls.native!.parse_internal!(source, language, goal)
		);
		add(
			impls.wasm?.parse_internal,
			'tsv_wasm-internal',
			'wasm-internal',
			(source, _language, goal) => impls.wasm!.parse_internal!(source, language, goal)
		);

		// OXC parser (TypeScript/JS only) — default mode: serializes to JSON in Rust
		// then JSON.parses in JS, eagerly materializing the full AST (the like-for-like
		// opponent to tsv-json). There is intentionally no `oxc-parser-lazy` row: oxc's
		// `experimentalLazy` raw transfer is setup-dominated in every runtime (measures
		// buffer copy, not parse speed) — see `lib/oxc.ts` and docs/benchmarks.md §Fairness caveats.
		add(
			impls.oxc?.supports_parse_language(language),
			'oxc-parser',
			'oxc',
			(source, _language, goal) => impls.oxc!.parse(source, language, goal)
		);
		add(
			impls.oxc_wasm?.supports_parse_language(language),
			'oxc-parser-wasm',
			'oxc-wasm',
			(source, _language, goal) => impls.oxc_wasm!.parse(source, language, goal)
		);

		// yuku-parser, N-API and WASM (TypeScript/JS only) — a Zig parser whose
		// default AST is span-only and padded exactly like oxc's, so both rows are
		// payload-matched to the oxc pair and to `tsv-json-no-locations` (plain
		// `tsv-json` carries the loc-bearing drop-in AST neither emits). The wrapper
		// forces yuku's LAZY materialization and reads its diagnostics — without
		// either, the row would report an unearned throughput at a fabricated 100%
		// coverage. See lib/yuku.ts + docs/benchmarks.md §Fairness caveats.
		//
		// ⚠ The N-API row is CONFORMANCE-EXCLUDED: yuku's native binding SEGFAULTS
		// the host process on that corpus's escaped-identifier test262 fixtures, so
		// keeping it there would kill every run rather than score a rejection. The
		// WASM binding is memory-safe and carries the engine on that surface. Both
		// rows stay on the perf corpus, which contains no such input. See lib/yuku.ts.
		add(
			impls.yuku?.supports_parse_language(language) && options.corpus_kind !== 'conformance',
			'yuku-parser',
			'yuku',
			(source, _language, goal) => impls.yuku!.parse(source, language, goal)
		);
		add(
			impls.yuku_wasm?.supports_parse_language(language),
			'yuku-parser-wasm',
			'yuku-wasm',
			(source, _language, goal) => impls.yuku_wasm!.parse(source, language, goal)
		);
	} else {
		// Canonical formatter (prettier) - async
		add_async(true, 'prettier', 'canonical', (source) =>
			impls.canonical.format_async(source, language)
		);

		// Native + WASM formatters
		add(impls.native?.format, 'tsv', 'native', (source) => impls.native!.format!(source, language));
		add(impls.wasm?.format, 'tsv_wasm', 'wasm', (source) => impls.wasm!.format!(source, language));

		// Forced-async control (opt-in). Same native engine as `tsv`, routed through
		// the awaited async path so the `tsv` vs `tsv-forced-async` delta measures the
		// per-file await tax; `Promise.resolve` wraps the already-computed result, so
		// the only added cost is the await. Rationale + why it's off by default:
		// `BenchmarkTaskOptions.forced_async`.
		add_async(
			options.forced_async && impls.native?.format,
			'tsv-forced-async',
			'native-forced-async',
			(source, language) => Promise.resolve(impls.native!.format!(source, language))
		);

		// OXC formatter (TypeScript/JS/CSS only) - async
		add_async(impls.oxc?.supports_format_language(language), 'oxfmt', 'oxfmt', (source) =>
			impls.oxc!.format_async(source, language)
		);

		add(impls.biome?.supports_format_language(language), 'biome-wasm', 'biome', (source) =>
			impls.biome!.format(source, language)
		);

		// dprint formatter (TypeScript/JS only — the engine `deno fmt` runs)
		add(impls.dprint?.supports_format_language(language), 'dprint-wasm', 'dprint', (source) =>
			impls.dprint!.format(source, language)
		);

		// rsvelte-fmt (Svelte only) — COVERAGE-ONLY. It ships no in-process format
		// API in any package (the sibling N-API addon is the compiler), so this task
		// spawns a process per file: measured on ~5 KB of Svelte the spawn floor
		// alone is over half the per-file cost and ~15x tsv's entire in-process
		// format, so a timed row would rank `fork`/`exec`, not engines. It is
		// measured for what it ACCEPTS instead. Its end-to-end CLI numbers — the
		// shape that suits a CLI — live in the separate hyperfine comparison
		// published on tsv.fuz.dev. See lib/rsvelte.ts + docs/benchmarks.md §Coverage-only rows.
		add(
			impls.rsvelte?.supports_format_language(language),
			'rsvelte-fmt',
			'rsvelte',
			(source) => impls.rsvelte!.format(source, language),
			{ coverage_only: true }
		);
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

/**
 * Get version info for available alternative implementations.
 * Only includes versions for implementations that initialized successfully.
 *
 * The field list lives in `report.ts` (`AlternativeVersionInfo`) — the module that
 * renders it — so producer and renderer can't disagree about which impls a report
 * carries. Adding an impl extends it there, once.
 */
export function get_alternative_versions(
	impls: InitializedImplementations
): AlternativeVersionInfo {
	return {
		oxc_parser: impls.oxc?.versions['oxc-parser'],
		oxfmt: impls.oxc?.versions.oxfmt,
		// Two packages over one engine, versioned in lockstep upstream — reported
		// separately so a skewed local install is visible rather than implied.
		yuku_parser: impls.yuku?.version,
		yuku_parser_wasm: impls.yuku_wasm?.version,
		biome: impls.biome?.versions.wasm,
		// The plugin version is the one worth citing — `@dprint/formatter` is just
		// the Wasm host, the TS/JS formatting behavior lives in the plugin.
		dprint: impls.dprint?.versions.typescript,
		rsvelte_fmt: impls.rsvelte?.versions.fmt
	};
}
