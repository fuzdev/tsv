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

import {
	CANONICAL_PARSER_ROWS,
	type Language,
	LANGUAGES,
	type Logger,
	type ParseGoal,
	type TsvImplementation
} from './types.ts';
import { CanonicalImplementation } from './canonical.ts';
import { check_executed_artifacts } from './check_artifact_freshness.ts';
import { NativeImplementation } from './ffi.ts';
import { NapiImplementation } from './napi.ts';
import { WasmImplementation } from './wasm.ts';
import { current_runtime } from './runtime.ts';
import { OxcImplementation } from './oxc.ts';
import { OxcWasmImplementation } from './oxc_wasm.ts';
import { TscImplementation } from './tsc.ts';
import { YukuImplementation } from './yuku.ts';
import { BiomeImplementation } from './biome.ts';
import { DprintImplementation } from './dprint.ts';
import { MalvaImplementation } from './malva.ts';
import { PostcssImplementation } from './postcss.ts';
import { RsvelteImplementation } from './rsvelte.ts';
import { RsvelteParseImplementation } from './rsvelte_parse.ts';
import { SwcImplementation } from './swc.ts';
import type { AlternativeVersionInfo } from './report.ts';
import { type AllVersions, load_all_versions } from './versions.ts';

/**
 * One optional implementation that failed to initialize on this machine — an
 * uninstalled package, a missing platform binding, a runtime that can't load a
 * given wasm entry, or a binding broken by an upstream bump.
 *
 * The INIT-time record, which is why it is not what the report publishes
 * (`UnavailableImpl` below is): it carries BOTH identities the failure has,
 * because they answer different questions — `impl` is what a human saw scroll
 * past, `key` is what the registry joins on to name the ROWS the failure cost
 * (`unavailable_with_rows`). Only the first survives to the wire.
 */
export interface InitFailure {
	/** The slot in `ImplementationSet` — the machine identity, never rendered. */
	key: ImplKey;
	/** The impl as the ⚠ init line names it, so terminal and report agree. */
	impl: string;
	/** First line of the load error — why it isn't here. */
	reason: string;
}

/**
 * A load failure as the REPORT carries it: the impl that failed, why, and the row
 * names its absence removed from this surface's tables — `InitFailure` above
 * joined against the surface's rows, with the internal `key` dropped.
 *
 * `rows` is the field a consumer can act on. Every other identity the report
 * publishes is a row name — `entries[].name`, `variant_parity.impl`/`.sibling`
 * ("the base ROW of the pair"), `report.ts`'s `DISPLAY_ORDER` — so a reader asking
 * "why is this column blank?" holds a row name and nothing else. `impl` alone
 * could not answer that: it is an init-line label (`OXC WASM`, `Biome`) that
 * matches no row (`oxc-parser-wasm`, `biome-wasm`), and one impl can back several
 * rows (`native` backs four; `oxc` backs `oxc-parser` AND `oxfmt`).
 *
 * Empty `rows` is meaningful, not a bug: the impl failed but defines no row on
 * THIS surface (a `tsc` failure costs the perf surface nothing), so the machine is
 * short while the tables are whole.
 */
export interface UnavailableImpl {
	impl: string;
	reason: string;
	rows: string[];
}

/**
 * The implementation slots plus the versions they were built from — the bag
 * `get_benchmark_tasks` reads to decide which rows exist.
 *
 * Split out from `InitializedImplementations` because it is what every CONSUMER
 * of that result actually takes (the registry, the version summary, the size
 * table), and because the init result then carries a second one of these: the
 * availability-independent `complete` view.
 */
export interface ImplementationSet {
	/** All package versions */
	versions: AllVersions;
	/**
	 * The three REQUIRED impls, and the only ones whose type is not `| undefined`:
	 * the oracle every comparison is against, and tsv's own two bindings — the
	 * subject of every tool that calls `init_implementations`.
	 *
	 * A load failure here is a broken tree, not a diminished machine, so it throws
	 * rather than joining `unavailable` (see `init_required`). That distinction is
	 * what makes the types honest: an optional impl really can be absent at run
	 * time, and these three cannot.
	 */
	canonical: CanonicalImplementation;
	/** Native implementation — FFI under Deno, N-API under Node/Bun. */
	native: NativeImplementation | NapiImplementation;
	/** WASM implementation (the runtime's own wasm-pack target bundle). */
	wasm: WasmImplementation;
	/** OXC implementation (oxc-parser + oxfmt) - undefined if not available */
	oxc: OxcImplementation | undefined;
	/** OXC WASM implementation (oxc-parser via wasm32-wasi) - undefined if not available */
	oxc_wasm: OxcWasmImplementation | undefined;
	/** tsc (the TypeScript compiler's own parser) - parse only; undefined if not available */
	tsc: TscImplementation | undefined;
	/** yuku-parser (N-API) - parse only; undefined if not available */
	yuku: YukuImplementation | undefined;
	/** yuku-parser (WASM) - parse only; undefined if not available */
	yuku_wasm: YukuImplementation | undefined;
	/** Biome implementation (via WASM) - undefined if not available */
	biome: BiomeImplementation | undefined;
	/** dprint implementation (via WASM; the engine `deno fmt` runs) - undefined if not available */
	dprint: DprintImplementation | undefined;
	/** malva (via WASM; dprint's CSS plugin) - format only; undefined if not available */
	malva: MalvaImplementation | undefined;
	/** postcss - parse only, CSS; undefined if not available */
	postcss: PostcssImplementation | undefined;
	/**
	 * rsvelte-fmt implementation (native binary, one process per file) - undefined
	 * if this platform has no prebuilt binary. Coverage-only: it has no in-process
	 * API, so it is never timed (see `get_benchmark_tasks`).
	 */
	rsvelte: RsvelteImplementation | undefined;
	/**
	 * rsvelte's Svelte PARSER (N-API addon) - parse only; undefined if not
	 * available. A different package from `rsvelte` above, and unlike it this one
	 * has an in-process API, so it IS timed (see `lib/rsvelte_parse.ts`).
	 */
	rsvelte_parse: RsvelteParseImplementation | undefined;
	/** swc (`@swc/core`, N-API) - parse only, TypeScript/JS; undefined if not available */
	swc: SwcImplementation | undefined;
}

/**
 * The slots that name an implementation — every key of `ImplementationSet` except
 * the version bag. DERIVED, so a new impl gets a key the day its slot appears;
 * a hand-written union would be the second source of truth this whole seam exists
 * to avoid.
 */
export type ImplKey = Exclude<keyof ImplementationSet, 'versions'>;

/** Result of initializing implementations */
export interface InitializedImplementations extends ImplementationSet {
	/**
	 * Every optional impl above that failed to init, in init order — the machine's
	 * shortfall as data rather than as terminal ⚠ lines. Empty `[]` on a full
	 * machine. Reaches the report via `unavailable_with_rows`.
	 */
	unavailable: InitFailure[];
	/**
	 * The same slots with every FAILED impl's constructed-but-uninitialized
	 * instance restored — what this harness would hold if every package loaded.
	 *
	 * This is what makes "which rows did that failure cost?" answerable at all. A
	 * failed impl registers no task, so its rows cannot be recovered from the live
	 * registry after the fact, and the alternative — a hand-written impl→rows map —
	 * would be an unchecked second source of truth that nothing could contradict
	 * (the very drift `SURFACE_DISCLOSURES` and the `DISPLAY_ORDER` guard exist to
	 * prevent). Asking the ONE registry against a complete set keeps the answer
	 * derived.
	 *
	 * Sound because the gates the registry evaluates are all construction-time
	 * facts, never init state: `parse_languages`/`format_languages` are `readonly`
	 * class fields (`BaseImplementation`), and `format`/`parse_internal`/
	 * `parse_no_locations` are prototype methods. An uninitialized instance answers
	 * every one of them exactly as a loaded one would.
	 *
	 * ⚠ Read for SHAPE only. Never hand this to `get_benchmark_tasks` directly — a
	 * task built from it closes over an impl whose `init()` never ran, so running
	 * one calls into a null binding. `get_defined_rows` is the safe accessor: it
	 * returns names, not closures.
	 */
	complete: ImplementationSet;
}

/** Options for implementation initialization */
export interface InitOptions {
	/** Logger for status messages */
	logger?: Logger;
}

// Deliberately NOT surface-scoped: every impl initializes on every run, including
// the format-only ones (biome, dprint, malva, rsvelte-fmt) on the parse-only
// conformance surface, where they back no row. Gating init on `OPERATIONS` is the
// obvious saving and it isn't worth taking — measured, the four cost ~233 ms total
// (biome 187, dprint 23, malva 9, rsvelte-fmt 14) against a multi-minute run, and
// two disclosures read the LIVE set rather than `complete`: `collect_binary_sizes`
// gates each artifact on `impls.<key>`, so a skipped impl silently drops its row
// from the published BINARY SIZES table (a catalog of what is on disk, which the
// surface's operation list has no business thinning), and `get_alternative_versions`
// feeds the report's `Versions:` line from the same set. The cost of the saving is a
// thinner published report; the cost of not taking it is a fifth of a second.

/**
 * Initialize one REQUIRED implementation, rethrowing when it can't load.
 *
 * Three impls take this path: `canonical` (the oracle every comparison is
 * against) and tsv's own `native` + `wasm` (the subject every caller of
 * `init_implementations` exists to measure or compare). A failure in any of them
 * is a broken tree — an unbuilt artifact, a corrupt bundle, or a self-check these
 * three run that the alternatives' rows can't invalidate (`canonical`'s
 * prettier-config probe, the tsv bindings' rejection probes) — not a machine coming
 * up short, so it must stop the run rather than join `unavailable`.
 *
 * That is the SAME rule `init_optional` follows, not a competing one: a failed
 * self-check withdraws whatever it contaminates. For an alternative that is its own
 * row, so the row goes to `unavailable`. For these three there is no row to
 * withdraw — the baseline is what every other row is a ratio against, and tsv's
 * bindings are the subject — so the contaminated unit is the whole run.
 *
 * Being fatal is also what lets their slots be non-`undefined` in
 * `ImplementationSet`. That was worth more than the tolerance it replaces: the
 * bench published tsv's OWN rows missing from every table behind a single ⚠ line,
 * and five diagnostics each hand-rolled their own `if (!impls.native) throw`,
 * every one of them a separate chance to word the requirement differently or
 * forget it. The expected-`unavailable` set is never tsv on any runtime (under Bun
 * it is biome), so nothing legitimate is lost by refusing.
 *
 * Note the asymmetry with the freshness guard, which is what leaves a gap for this
 * to close: `check_artifact_freshness` makes a MISSING artifact fatal, but a
 * present-yet-unloadable one only surfaces here.
 */
async function init_required<T extends { init: () => Promise<void> }>(
	impl: T,
	label: string,
	logger: Logger
): Promise<void> {
	try {
		await impl.init();
		logger(`  ✓ ${label}`);
	} catch (e) {
		logger(`  ✗ ${label}: ${e}`);
		throw e;
	}
}

/**
 * Initialize one OPTIONAL implementation, returning it on success and `undefined`
 * when it isn't available on this machine.
 *
 * Every alternative impl is optional in the same way — a missing platform binding,
 * an uninstalled package, a runtime that can't load a given wasm entry — so they
 * all want this exact try/catch. Sharing it is what keeps a new impl from arriving
 * with a subtly different failure posture (swallowing where the others rethrow,
 * say). There is no opt-out: a caller that wants the full set asserts on the
 * returned `unavailable` list, which names what is missing and why — better than a
 * throw that reports only the first absence.
 *
 * `label` is the success line; `missing_label` names the impl in the ⚠ line when it
 * reads differently there (the ✓ lines carry a parenthetical the ⚠ lines don't).
 *
 * Each absence is also pushed to `unavailable`, which reaches the report JSON. The
 * ⚠ line alone lives in the terminal scroll: an impl that stops loading drops its
 * ROW from every table, and a reader diffing the committed report would see the
 * column disappear with nothing saying why. Same disclosure posture as
 * `suppressed_noise` and `variant_parity` in `bench.ts`.
 *
 * `key` rides along for that record — the failure has to be joinable back to the
 * rows it cost, and the display label can't do it (see `UnavailableImpl`).
 */
async function init_optional<T extends { init: () => Promise<void> }>(
	impl: T,
	key: ImplKey,
	label: string,
	logger: Logger,
	unavailable: InitFailure[],
	missing_label: string = label
): Promise<T | undefined> {
	try {
		await impl.init();
		logger(`  ✓ ${label}`);
		return impl;
	} catch (e) {
		logger(`  ⚠ ${missing_label}: not available`);
		// First line only, like the native-panic classifier: a load failure's later
		// lines are a stack trace, machine-specific and worthless in a committed diff.
		unavailable.push({
			key,
			impl: missing_label,
			reason: String(e instanceof Error ? e.message : e).split('\n')[0]
		});
		return undefined;
	}
}

/**
 * Initialize all benchmark implementations.
 *
 * Runs the executed-artifact freshness guard first (`check_executed_artifacts`:
 * this runtime's native binding + its `all` WASM bundle, the two `init_required`
 * loads below). The bench and smoke entry points also call it themselves, earlier,
 * so they abort before a corpus load; the guard lives here too because the
 * `diagnostics/` scripts reach these bindings through this function alone, and a
 * stale native library loaded through `Deno.dlopen` does not fail with a message —
 * it segfaulted at init, twelve days behind its sources, with nothing naming the
 * cause. `BENCH_STALE_OK=1` downgrades stale to a warning here as everywhere.
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
	const { logger = console.log } = options;

	await check_executed_artifacts();

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

	// The three REQUIRED impls, in one posture — see `init_required`. Everything
	// below is optional; these are the oracle and the subject.
	await init_required(canonical, 'Canonical (prettier + svelte/compiler)', logger);
	await init_required(native, native_label, logger);
	await init_required(wasm, 'WASM', logger);

	// Every impl below is optional and shares one failure posture — see `init_optional`.
	const unavailable: InitFailure[] = [];
	const optional = <T extends { init: () => Promise<void> }>(
		impl: T,
		key: ImplKey,
		label: string,
		missing_label?: string
	) => init_optional(impl, key, label, logger, unavailable, missing_label);

	// Constructed up front, one `new` per slot, so the two views below are visibly
	// the SAME set read two ways: `complete` is every instance, the returned bag is
	// the subset whose `init()` took. Inlining these into the `optional(...)` calls
	// would leave a failed impl's instance unreachable, and with it the only
	// non-hand-written answer to which rows its absence removed (`complete`).
	const oxc = new OxcImplementation(versions.oxc);
	const oxc_wasm = new OxcWasmImplementation(versions.oxc);
	const tsc = new TscImplementation();
	// One class, two bindings — see lib/yuku.ts.
	const yuku = new YukuImplementation('yuku-parser', versions.yuku);
	const yuku_wasm = new YukuImplementation('yuku-parser-wasm', versions.yuku);
	const biome = new BiomeImplementation(versions.biome);
	const dprint = new DprintImplementation(versions.dprint);
	const malva = new MalvaImplementation(versions.malva);
	const rsvelte = new RsvelteImplementation(versions.rsvelte);
	// A different package from rsvelte-fmt above — the N-API addon, which unlike
	// the fmt CLI does have an in-process API. See lib/rsvelte_parse.ts.
	const rsvelte_parse = new RsvelteParseImplementation(versions.rsvelte_parse);
	const swc = new SwcImplementation(versions.swc);
	const postcss = new PostcssImplementation(versions.postcss);

	const oxc_impl = await optional(oxc, 'oxc', 'OXC (oxc-parser + oxfmt)', 'OXC');
	const oxc_wasm_impl = await optional(oxc_wasm, 'oxc_wasm', 'OXC WASM (oxc-parser)', 'OXC WASM');
	const tsc_impl = await optional(tsc, 'tsc', `tsc ${versions.tsc.typescript} (parse-only)`, 'tsc');
	const yuku_impl = await optional(yuku, 'yuku', 'yuku-parser (N-API)', 'yuku-parser');
	const yuku_wasm_impl = await optional(
		yuku_wasm,
		'yuku_wasm',
		'yuku-parser (WASM)',
		'yuku-parser WASM'
	);
	const biome_impl = await optional(biome, 'biome', 'Biome (WASM)', 'Biome');
	const dprint_impl = await optional(dprint, 'dprint', 'dprint (WASM)', 'dprint');
	const malva_impl = await optional(malva, 'malva', 'malva (WASM, CSS)', 'malva');
	const rsvelte_impl = await optional(
		rsvelte,
		'rsvelte',
		'rsvelte-fmt (native binary, coverage-only)',
		'rsvelte-fmt'
	);
	const rsvelte_parse_impl = await optional(
		rsvelte_parse,
		'rsvelte_parse',
		'rsvelte parse (N-API, svelte)',
		'rsvelte parse'
	);
	// The addon targets a specific upstream Svelte, and the row it feeds sits beside
	// `svelte/compiler` — the oracle — on `parse/svelte`. When those drift apart the
	// two rows are parsing to different language versions, which is a comparison
	// caveat, not a broken setup: WARN and keep the row (the same posture
	// `lib/fixtures_gate.ts` takes on checkout↔pin skew). The report renders both
	// versions either way; this is what makes the disclosure active.
	const rsvelte_svelte_target = rsvelte_parse_impl?.upstream_svelte_version;
	if (rsvelte_svelte_target && rsvelte_svelte_target !== versions.canonical.svelte) {
		logger(
			`  ⚠ rsvelte parse targets svelte@${rsvelte_svelte_target}, but the harness pins ` +
				`svelte@${versions.canonical.svelte} — its parse/svelte rows are graded against a ` +
				`different language version than the svelte/compiler oracle beside them.`
		);
	}

	const swc_impl = await optional(swc, 'swc', 'swc (N-API)', 'swc');
	const postcss_impl = await optional(postcss, 'postcss', 'postcss (CSS)', 'postcss');

	logger('');

	return {
		versions,
		canonical,
		native,
		wasm,
		oxc: oxc_impl,
		oxc_wasm: oxc_wasm_impl,
		tsc: tsc_impl,
		yuku: yuku_impl,
		yuku_wasm: yuku_wasm_impl,
		biome: biome_impl,
		dprint: dprint_impl,
		malva: malva_impl,
		postcss: postcss_impl,
		rsvelte: rsvelte_impl,
		rsvelte_parse: rsvelte_parse_impl,
		swc: swc_impl,
		unavailable,
		// Every instance, initialized or not — see `InitializedImplementations.complete`.
		complete: {
			versions,
			canonical,
			native,
			wasm,
			oxc,
			oxc_wasm,
			tsc,
			yuku,
			yuku_wasm,
			biome,
			dprint,
			malva,
			postcss,
			rsvelte,
			rsvelte_parse,
			swc
		}
	};
}

/** A benchmark task definition */
export interface BenchmarkTask {
	/** Display name in benchmark output */
	name: string;
	/**
	 * The implementation slot backing this row.
	 *
	 * A row's name is its public identity; this is the internal one, and recording
	 * it here is what lets a load failure be attributed to the rows it removed
	 * without a hand-written map (`get_defined_rows` → `unavailable_with_rows`).
	 * Many-to-one on purpose: `native` backs four parse rows, `oxc` backs
	 * `oxc-parser` and `oxfmt`.
	 */
	impl: ImplKey;
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
	 * Which corpus/surface this run measures (default `perf`). Two tasks read it,
	 * in opposite directions: the `yuku-parser` N-API row is DROPPED on the
	 * `conformance` surface (that corpus carries inputs its native binding cannot
	 * survive — CLAUDE.md §Known Issues), and the `tsc` row is added ONLY there (a
	 * verdict row, not a speed row). Both registration sites carry the reasoning.
	 */
	corpus_kind?: 'perf' | 'conformance';
}

/**
 * Get all benchmark tasks for a specific operation and language.
 * Returns tasks in display order (canonical first, then alternatives).
 */
export function get_benchmark_tasks(
	impls: ImplementationSet,
	operation: 'parse' | 'format',
	language: Language,
	options: BenchmarkTaskOptions = {}
): BenchmarkTask[] {
	const tasks: BenchmarkTask[] = [];
	const group_name = `${operation}/${language}`;

	/**
	 * Register a sync task for implementation `impl` when `enabled`. The gate
	 * differs per impl — a missing binding, an unsupported language, an absent
	 * optional method, a surface this row is excluded from — so each caller passes
	 * its own; `true` is an impl that's always present.
	 *
	 * `impl` is the OWNER, not the gate. It names which slot backs the row, which is
	 * what lets the same traversal answer both "what runs here" (against the live
	 * set) and "what does this surface define" (against `complete`).
	 */
	const add = (
		impl: ImplKey,
		enabled: unknown,
		name: string,
		key: string,
		run: BenchmarkTask['run'],
		extra?: Pick<BenchmarkTask, 'coverage_only'>
	): void => {
		if (!enabled) return;
		tasks.push({
			name,
			impl,
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
		impl: ImplKey,
		enabled: unknown,
		name: string,
		key: string,
		run_async: NonNullable<BenchmarkTask['run_async']>
	): void => {
		if (!enabled) return;
		tasks.push({
			name,
			impl,
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
		add(
			'canonical',
			true,
			CANONICAL_PARSER_ROWS[language],
			'canonical',
			(source, _language, goal) => impls.canonical.parse(source, language, goal)
		);

		// Native + WASM parsers (with JSON serialization)
		add('native', true, 'tsv-json', 'native', (source, _language, goal) =>
			impls.native.parse(source, language, goal)
		);
		add('wasm', true, 'tsv_wasm-json', 'wasm', (source, _language, goal) =>
			impls.wasm.parse(source, language, goal)
		);

		// The no-locations wire (span-only: no per-node `loc`) — the payload-matched
		// opponent to oxc-parser and yuku-parser, whose default ASTs are also
		// span-only. Materialized in Rust either side, so native and wasm stay
		// mechanism-matched to their `-json` siblings. CSS is skipped — `parseCss`
		// emits no `loc`, so a CSS no-locations row would duplicate `tsv-json`.
		add(
			'native',
			language !== 'css',
			'tsv-json-no-locations',
			'native-no-locations',
			(source, _language, goal) => impls.native.parse_no_locations(source, language, goal)
		);
		add(
			'wasm',
			language !== 'css',
			'tsv_wasm-json-no-locations',
			'wasm-no-locations',
			(source, _language, goal) => impls.wasm.parse_no_locations(source, language, goal)
		);

		// Internal parsing variants (no JSON serialization) - shows JSON overhead
		add('native', true, 'tsv-internal', 'native-internal', (source, _language, goal) =>
			impls.native.parse_internal(source, language, goal)
		);
		add('wasm', true, 'tsv_wasm-internal', 'wasm-internal', (source, _language, goal) =>
			impls.wasm.parse_internal(source, language, goal)
		);

		// OXC parser (TypeScript/JS only) — default mode: serializes to JSON in Rust
		// then JSON.parses in JS, eagerly materializing the full AST (the like-for-like
		// opponent to tsv-json). There is intentionally no `oxc-parser-lazy` row: oxc's
		// `experimentalLazy` raw transfer is setup-dominated in every runtime (measures
		// buffer copy, not parse speed) — see `lib/oxc.ts` and docs/benchmarks.md §Fairness caveats.
		add(
			'oxc',
			impls.oxc?.supports_parse_language(language),
			'oxc-parser',
			'oxc',
			(source, _language, goal) => impls.oxc!.parse(source, language, goal)
		);
		add(
			'oxc_wasm',
			impls.oxc_wasm?.supports_parse_language(language),
			'oxc-parser-wasm',
			'oxc-wasm',
			(source, _language, goal) => impls.oxc_wasm!.parse(source, language, goal)
		);

		// tsc (TypeScript/JS only) — the language's own parser, so this row is the
		// DEFINITION the other TS rows are measured against, the way svelte/compiler
		// is on the Svelte surface. Two properties decide where it belongs:
		//
		// 1. It is CONFORMANCE-ONLY. The value of a tsc row is a verdict, not a
		//    speed: adding it to the perf surface would put a new row in the
		//    published throughput tables (which tsv.fuz.dev renders) for a tool
		//    nobody formats with. Flipping it on there is a one-word change here,
		//    and a deliberate one.
		// 2. On the tsc-corpus entry it is the ORACLE (100% by construction — the
		//    harvest keeps exactly the files this parser accepts), while on test262
		//    and the prettier suites it is an independent parser like any other. The
		//    per-source coverage breakdown is what keeps those two readings apart;
		//    the aggregate row alone would blend them. See lib/tsc.ts.
		add(
			'tsc',
			impls.tsc?.supports_parse_language(language) && options.corpus_kind === 'conformance',
			'tsc',
			'tsc',
			(source, _language, goal) => impls.tsc!.parse(source, language, goal)
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
			'yuku',
			impls.yuku?.supports_parse_language(language) && options.corpus_kind !== 'conformance',
			'yuku-parser',
			'yuku',
			(source, _language, goal) => impls.yuku!.parse(source, language, goal)
		);
		add(
			'yuku_wasm',
			impls.yuku_wasm?.supports_parse_language(language),
			'yuku-parser-wasm',
			'yuku-wasm',
			(source, _language, goal) => impls.yuku_wasm!.parse(source, language, goal)
		);

		// rsvelte's parser (Svelte only) — the ONLY third-party engine on this surface;
		// the rest of the group is `svelte/compiler` (the oracle) and tsv's own
		// variants. `parse()` returns JSON the caller parses, exactly the
		// mechanism `tsv-json` measures, so the two are apples-to-apples. It also
		// claims tsv's own drop-in contract, which makes the row a conformance datum
		// as much as a speed one. See lib/rsvelte_parse.ts.
		add(
			'rsvelte_parse',
			impls.rsvelte_parse?.supports_parse_language(language),
			'rsvelte-parse',
			'rsvelte-parse',
			(source) => impls.rsvelte_parse!.parse(source, language)
		);
		// ⚠ Named for the OPTION it passes, not for tsv's `no-locations` wire: the two
		// reductions differ (tsv drops per-node `loc` throughout, ~46%; rsvelte drops
		// only nested expression `loc` and keeps top-level start/end, -34%), so this
		// row is NOT payload-matched to `tsv-json-no-locations` and is deliberately
		// absent from report.ts's curated payload-matched lines.
		add(
			'rsvelte_parse',
			impls.rsvelte_parse?.supports_parse_language(language),
			'rsvelte-parse-skip-expr-loc',
			'rsvelte-parse-skip-expr-loc',
			(source) => impls.rsvelte_parse!.parse_skip_expression_loc(source, language)
		);

		// swc (TypeScript/JS only) — the most widely deployed Rust TS parser. Its AST
		// is its own dialect (root `Module`, `span` not `loc`, `Ts`-prefixed kinds), so
		// it carries the oxc-class payload disclosure and is NOT an opponent for the
		// span-only curated lines. Goal-aware via `isModule` (see lib/swc.ts), which is
		// what lets it join the conformance surface without scoring script-goal
		// test262 files as module-goal failures. Three real-corpus `.d.ts` rejections
		// are catalogued in lib/perf_omit.ts.
		add(
			'swc',
			impls.swc?.supports_parse_language(language),
			'swc',
			'swc',
			(source, _language, goal) => impls.swc!.parse(source, language, goal)
		);

		// postcss (CSS only) — the first third-party engine on `parse/css`, and the
		// parser behind prettier's CSS printer, i.e. behind the `format/css` baseline.
		// No native peer exists to add here: no Rust CSS parser exposes an AST to JS
		// (lightningcss is transform-only, biome's js-api exposes no parse, malva is a
		// formatter, oxc has no CSS parse binding). See lib/postcss.ts.
		add(
			'postcss',
			impls.postcss?.supports_parse_language(language),
			'postcss',
			'postcss',
			(source) => impls.postcss!.parse(source, language)
		);
	} else {
		// Canonical formatter (prettier) - async
		add_async('canonical', true, 'prettier', 'canonical', (source) =>
			impls.canonical.format_async(source, language)
		);

		// Native + WASM formatters
		add('native', true, 'tsv', 'native', (source) => impls.native.format(source, language));
		add('wasm', true, 'tsv_wasm', 'wasm', (source) => impls.wasm.format(source, language));

		// Forced-async control (opt-in). Same native engine as `tsv`, routed through
		// the awaited async path so the `tsv` vs `tsv-forced-async` delta measures the
		// per-file await tax; `Promise.resolve` wraps the already-computed result, so
		// the only added cost is the await. Rationale + why it's off by default:
		// `BenchmarkTaskOptions.forced_async`.
		add_async(
			'native',
			options.forced_async,
			'tsv-forced-async',
			'native-forced-async',
			(source, language) => Promise.resolve(impls.native.format(source, language))
		);

		// OXC formatter (TypeScript/JS/CSS only) - async
		add_async('oxc', impls.oxc?.supports_format_language(language), 'oxfmt', 'oxfmt', (source) =>
			impls.oxc!.format_async(source, language)
		);

		add('biome', impls.biome?.supports_format_language(language), 'biome-wasm', 'biome', (source) =>
			impls.biome!.format(source, language)
		);

		// dprint formatter (TypeScript/JS only — the engine `deno fmt` runs)
		add(
			'dprint',
			impls.dprint?.supports_format_language(language),
			'dprint-wasm',
			'dprint',
			(source) => impls.dprint!.format(source, language)
		);

		// malva formatter (CSS only) — dprint's CSS plugin, over the same
		// `@dprint/formatter` host. Gives format/css a second wasm-tier engine (the
		// only other one is biome-wasm). See lib/malva.ts.
		add('malva', impls.malva?.supports_format_language(language), 'malva-wasm', 'malva', (source) =>
			impls.malva!.format(source, language)
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
			'rsvelte',
			impls.rsvelte?.supports_format_language(language),
			'rsvelte-fmt',
			'rsvelte',
			(source) => impls.rsvelte!.format(source, language),
			{ coverage_only: true }
		);
	}

	return tasks;
}

/** One row this surface defines, and the implementation slot behind it. */
export interface DefinedRow {
	name: string;
	impl: ImplKey;
}

/**
 * Every row this surface DEFINES over `operations`, independent of what loaded —
 * the policy question, asked of `get_benchmark_tasks` and answered against
 * `impls.complete`.
 *
 * Two callers, for which the availability-dependent version gives a subtly
 * wrong answer:
 *
 * - the report's row-composition guards (`SURFACE_DISCLOSURES`, the `DISPLAY_ORDER`
 *   check). Asked of the LIVE set, an `excluded` claim passes vacuously whenever
 *   the impl merely failed to load — so re-enabling a row on a machine whose
 *   binding didn't install would publish the stale sentence with the guard silent.
 * - `unavailable_with_rows`, which needs the rows a failure removed.
 *
 * Returns plain data, never tasks: a task built from `complete` closes over an
 * uninitialized impl, so the closures must not escape this function. Deduped by
 * name — a row spans several languages (`tsv-json` is in all three) and this
 * answers about rows, not cells.
 */
export function get_defined_rows(
	impls: InitializedImplementations,
	operations: ReadonlyArray<'parse' | 'format'>,
	options: BenchmarkTaskOptions = {}
): DefinedRow[] {
	const rows = new Map<string, DefinedRow>();
	for (const operation of operations) {
		for (const language of LANGUAGES) {
			for (const task of get_benchmark_tasks(impls.complete, operation, language, options)) {
				if (!rows.has(task.name)) rows.set(task.name, { name: task.name, impl: task.impl });
			}
		}
	}
	return [...rows.values()];
}

/**
 * The report's `unavailable`: each load failure with the ROW names its absence
 * removed from this surface (see `UnavailableImpl`).
 *
 * A pure join over an ALREADY-COMPUTED `defined`, rather than a second
 * `get_defined_rows` call, for the reason its caller states: the disclosure guard
 * asks the same registry, and two builds are two chances to answer from different
 * sets. Taking the rows as an argument also makes the surface-scoping visible —
 * the same failure costs different rows on different surfaces (a `tsc` failure
 * costs the perf surface nothing, a `yuku` failure costs the conformance surface
 * nothing), so there is no answer independent of which rows are in play.
 */
export function unavailable_with_rows(
	unavailable: readonly InitFailure[],
	defined: readonly DefinedRow[]
): UnavailableImpl[] {
	return unavailable.map((u) => ({
		impl: u.impl,
		reason: u.reason,
		rows: defined.filter((row) => row.impl === u.key).map((row) => row.name)
	}));
}

/**
 * Get version info for available alternative implementations.
 * Only includes versions for implementations that initialized successfully.
 *
 * The field list lives in `report.ts` (`AlternativeVersionInfo`) — the module that
 * renders it — so producer and renderer can't disagree about which impls a report
 * carries. Adding an impl extends it there, once.
 */
export function get_alternative_versions(impls: ImplementationSet): AlternativeVersionInfo {
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
		// Same reasoning as dprint: the CSS plugin, not the shared host.
		malva: impls.malva?.versions.malva,
		postcss: impls.postcss?.versions.postcss,
		rsvelte_fmt: impls.rsvelte?.versions.fmt,
		// Two facts, both reported: the addon's own version, and the upstream Svelte
		// it targets — a drift of the latter from the harness's `svelte` pin means the
		// row is parsing to a different Svelte than the oracle it's compared against.
		rsvelte_parse: impls.rsvelte_parse?.versions.native,
		rsvelte_parse_svelte_target: impls.rsvelte_parse?.upstream_svelte_version,
		swc: impls.swc?.versions.core
	};
}
