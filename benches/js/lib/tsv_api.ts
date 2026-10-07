/**
 * The tsv packages' published API over one engine — what tsv's own bench rows call.
 *
 * Every tsv npm package publishes through one hand-written facade
 * (`crates/tsv_wasm/npm/api.js` + `api_parse.js`): per call it checks the source is a
 * well-formed UTF-16 string, reads the options bag, rethrows a parse failure as the
 * typed `SyntaxError`, and — over an engine that hands back the wire as a string —
 * `JSON.parse`s it. A consumer's `parse_typescript(source)` pays that, as every
 * third-party row here pays its own package's front door, so the three tsv bindings
 * (`lib/ffi.ts`, `lib/napi.ts`, `lib/wasm.ts`) hand their flat exports to
 * `TsvBinding.bind` and time the result: the same two factories, wired the way each
 * package's entry wires them (`crates/tsv_napi/npm/index.js`, the staged wasm
 * `index.js`), imported from the source the packages stage verbatim.
 *
 * The C FFI ships in no npm package, so over it the facade is the one tsv WOULD
 * publish rather than one it does — taken so the `tsv` row means the published API
 * over the runtime's native binding under all three runtimes, not under two.
 *
 * `parse_internal_*` never passes through the facade in any package (it is bench-only),
 * so the `-internal` rows stay on the raw export, in each wrapper.
 *
 * What the facade costs over the flat exports is `diagnostics/facade_probe.ts`'s to
 * measure, against `TsvEngine` — which is why a binding keeps its engine reachable.
 */

import { create_format_api } from '../../../crates/tsv_wasm/npm/api.js';
import { create_parse_api } from '../../../crates/tsv_wasm/npm/api_parse.js';
import { BaseImplementation, goal_for, type Language, LANGUAGES, type ParseGoal } from './types.ts';

/**
 * A flat engine export: the source, then the optional source type — a `string` because
 * that is what the facade declares it hands over, though it grades the value first, so
 * an engine only ever sees `'script'`, `'module'` or none.
 */
export type EngineFn<T> = (source: string, source_type?: string) => T;

/**
 * One binding's flat exports, keyed by language and resolved once, so a timed call
 * allocates no table.
 *
 * `parse_json`, `parse` and `format` are what the facade's two factories take.
 * `parse_json` returns the span-only wire as a string. `parse` is present only for an
 * engine that materializes the wire itself (the WASM bindings run `JSON.parse`
 * engine-side); without it the facade parses `parse_json`'s string, as it does over the
 * N-API addon. `parse_internal` is the bench-only parse that converts nothing: no
 * package publishes it, so it is called as is.
 */
export interface TsvEngine {
	parse_json: Record<Language, EngineFn<string>>;
	parse?: Record<Language, EngineFn<unknown>>;
	format: Record<Language, EngineFn<string>>;
	parse_internal: Record<Language, EngineFn<void>>;
}

/** A published function: the source, then the options bag. */
type Published<T> = (source: string, options?: object) => T;

/** What `bind` leaves: the engine as given, and the facade's functions over it. */
interface Bound {
	engine: TsvEngine;
	parse: Record<Language, Published<unknown>>;
	format: Record<Language, Published<string>>;
}

// The bags a call passes, built once: a consumer writes its bag as a literal, but one
// allocated per timed call here would be the harness's allocation charged to the row
// (the reason the wrappers resolve their export tables once).
const SOURCE_TYPE_OPTIONS = {
	script: { sourceType: 'script' },
	module: { sourceType: 'module' }
} as const;
const LOCATIONS_OPTIONS = { locations: true } as const;
const LOCATIONS_SOURCE_TYPE_OPTIONS = {
	script: { locations: true, sourceType: 'script' },
	module: { locations: true, sourceType: 'module' }
} as const;

const per_language = <T>(of: (language: Language) => T): Record<Language, T> => ({
	svelte: of('svelte'),
	typescript: of('typescript'),
	css: of('css')
});

/**
 * Shared base of tsv's three binding wrappers: the published calls, once.
 *
 * A wrapper owns how its binding loads and marshals; what its rows call is the same
 * whichever engine it loaded — the published API for `parse` / `parse_with_locations` /
 * `format`, the raw export for the bench-only `parse_internal` — so those live here.
 * The wrapper hands its flat exports to `bind` from `init()`, ahead of its init probes
 * — which then probe the row's own call, facade included.
 *
 * Each method is one table lookup away from the facade's function, so a row carries no
 * more of the harness than a wrapper calling its export directly would.
 */
export abstract class TsvBinding extends BaseImplementation {
	readonly parse_languages = LANGUAGES;
	readonly format_languages = LANGUAGES;

	#bound: Bound | null = null;

	/** Publish `engine` — the wrapper's `init()` calls this once its binding has loaded. */
	protected bind(engine: TsvEngine): void {
		const published: Record<string, Published<unknown>> = {
			...create_parse_api({ parse_json: engine.parse_json, parse: engine.parse }),
			...create_format_api(engine.format)
		};
		this.#bound = {
			engine,
			parse: per_language((language) => published[`parse_${language}`]),
			format: per_language((language) => published[`format_${language}`] as Published<string>)
		};
	}

	/** Drop the engine. A wrapper holding more (a library handle) overrides and calls up. */
	dispose(): void {
		this.#bound = null;
	}

	private get bound(): Bound {
		if (!this.#bound) throw new Error('tsv binding not initialized');
		return this.#bound;
	}

	/** The flat exports beneath the published API, as `bind` was given them. */
	get engine(): TsvEngine {
		return this.bound.engine;
	}

	/**
	 * `parse_<lang>(source)` — the package's default parse, the span-only tree.
	 *
	 * A call with no goal passes no bag at all, which is every call on the perf surface.
	 * `goal_for` withholds the goal for Svelte and CSS, which refuse a set source type;
	 * the conformance surface's goal-tagged TypeScript files pass theirs as `sourceType`.
	 */
	parse(source: string, language: Language, goal?: ParseGoal): unknown {
		const parse = this.bound.parse[language];
		const source_type = goal_for(language, goal);
		return source_type === undefined
			? parse(source)
			: parse(source, SOURCE_TYPE_OPTIONS[source_type]);
	}

	/** `parse_<lang>(source, {locations: true})` — the default parse plus `loc`. */
	parse_with_locations(source: string, language: Language, goal?: ParseGoal): unknown {
		const source_type = goal_for(language, goal);
		return this.bound.parse[language](
			source,
			source_type === undefined ? LOCATIONS_OPTIONS : LOCATIONS_SOURCE_TYPE_OPTIONS[source_type]
		);
	}

	/**
	 * `format_<lang>(source)` — no source type named: the shipped default on every
	 * surface, and the one that reaches the module-then-script fallback
	 * (`tsv_ts::parse_with_goal_or_fallback`).
	 */
	format(source: string, language: Language): string {
		return this.bound.format[language](source);
	}

	/**
	 * The bench-only parse that converts nothing — the raw export, past the facade, so
	 * the goal is handed over as the engine takes it (withheld for Svelte and CSS, as
	 * above: every binding refuses a set one there rather than ignoring it).
	 */
	parse_internal(source: string, language: Language, goal?: ParseGoal): void {
		this.bound.engine.parse_internal[language](source, goal_for(language, goal));
	}
}
