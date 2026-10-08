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
 *
 * The `parse` and `format` rows call the published API over these exports — the
 * packages' facade, wired as the staged `index.js` wires it over the web-target glue
 * (`parse_<lang>`, which materializes engine-side, beside `parse_<lang>_json`). So what
 * stands between these rows and the package is the wasm-bindgen TARGET's glue alone:
 * same engine, same facade. See `lib/tsv_api.ts`.
 */

import { stat } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { wasm_target } from './runtime.ts';
import { wasm_bundle_dir } from './tsv_artifacts.ts';
import { LOCATIONS_PROBE_SOURCES } from './locations_probe.ts';
import { scope_call, TsvBinding } from './tsv_api.ts';
import { type InitScope, LANGUAGES } from './types.ts';

/**
 * WASM module function signatures — the raw wasm-bindgen exports, flat like the two
 * native bindings': each takes the source type as a trailing OPTIONAL string
 * (`'script'` / `'module'`; omitted = none named). Svelte and CSS REJECT a set one
 * rather than ignoring it (`crates/tsv_wasm/src/lib.rs`'s `wasm_source_type`), so
 * `TsvBinding` withholds it for them. `parse_<lang>`
 * returns the span-only wire as an object, `JSON.parse`d engine-side via `js_sys`;
 * `parse_<lang>_json` returns the wire string itself.
 */
interface WasmModule {
	parse_svelte: (source: string, source_type?: string) => unknown;
	parse_svelte_json: (source: string, source_type?: string) => string;
	parse_internal_svelte: (source: string, source_type?: string) => void;
	format_svelte: (source: string, source_type?: string) => string;
	parse_typescript: (source: string, source_type?: string) => unknown;
	parse_typescript_json: (source: string, source_type?: string) => string;
	parse_internal_typescript: (source: string, source_type?: string) => void;
	format_typescript: (source: string, source_type?: string) => string;
	parse_css: (source: string, source_type?: string) => unknown;
	parse_css_json: (source: string, source_type?: string) => string;
	parse_internal_css: (source: string, source_type?: string) => void;
	format_css: (source: string, source_type?: string) => string;
}

export class WasmImplementation extends TsvBinding {
	async init(scope?: InitScope): Promise<void> {
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
		// function names off both through the typed `WasmModule` shape.
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

		// The bundle's flat exports, with `parse_<lang>` beside `parse_<lang>_json` as
		// the facade's parse engine — the staged `index.js`'s own wiring.
		this.bind({
			parse: {
				svelte: module.parse_svelte,
				typescript: module.parse_typescript,
				css: module.parse_css
			},
			parse_json: {
				svelte: module.parse_svelte_json,
				typescript: module.parse_typescript_json,
				css: module.parse_css_json
			},
			format: {
				svelte: module.format_svelte,
				typescript: module.format_typescript,
				css: module.format_css
			},
			parse_internal: {
				svelte: module.parse_internal_svelte,
				typescript: module.parse_internal_typescript,
				css: module.parse_internal_css
			}
		});

		// Fairness guard for the parse rows: the wasm parse fns must return a
		// js_sys-materialized OBJECT (the engine runs the host's JSON.parse from
		// Rust). If a glue/build regression ever handed back the raw JSON string
		// instead, the timed `tsv-wasm` parse rows would silently skip
		// materialization and read artificially fast vs their native sibling. Probed
		// here, outside any timed loop, on the raw export (the facade passes through
		// whatever the engine's `parse` returns) — and, like the probes below, only for
		// a row that calls it (the default parse and `{locations: true}` both do), on
		// that row's language, when the process is one row's (`scope`).
		const call = scope === undefined ? undefined : scope_call(scope);
		if (call === undefined || call === 'parse' || call === 'parse_with_locations') {
			for (const language of scope === undefined ? LANGUAGES : [scope.language]) {
				const probe = this.engine.parse![language](LOCATIONS_PROBE_SOURCES[language]);
				if (typeof probe !== 'object' || probe === null) {
					throw new Error(
						`tsv-wasm parse_${language} returned a ${typeof probe} — expected a materialized AST object`
					);
				}
			}
		}

		// The bindings throw natively today; probed anyway so the three can't come to
		// disagree about what surfacing a refusal MEANS. The guard above asks what a
		// SUCCESS returns; the reject probe asks what a REFUSAL does. And the parse wire
		// is span-only, so a stale bundle whose `parse_<lang>` still returns the
		// loc-bearing wire can't be timed under the span rows' label — the freshness
		// guard refuses such a bundle unless `BENCH_STALE_OK=1`, and this is what still
		// stands then.
		this.probe(
			'tsv (WASM)',
			{ path: wasm_path, rebuild: `deno task build:wasm:all:${target}` },
			scope
		);
	}
}
