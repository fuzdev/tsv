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
 * packages' facade, wired as the staged `parse.js` / `index.js` wire it over the web-target
 * glue (`parse_<lang>` decoding `parse_tape_<lang>`'s tape, a view of the instance's linear
 * memory, beside `parse_<lang>_json`). So what stands between these rows and the package is
 * the wasm-bindgen TARGET's glue alone: same engine, same facade. See `lib/tsv_api.ts`.
 */

import { stat } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { wasm_target } from './runtime.ts';
import { wasm_bundle_dir } from './tsv_artifacts.ts';
import { type EngineFailure, type EngineFn, TsvBinding } from './tsv_api.ts';
import type { InitScope } from './types.ts';

/**
 * WASM module function signatures — the raw wasm-bindgen exports, flat like the two
 * native bindings': each takes the source type as a trailing OPTIONAL string
 * (`'script'` / `'module'`; omitted = none named). Svelte and CSS REJECT a set one
 * rather than ignoring it (`crates/tsv_wasm/src/lib.rs`'s `wasm_source_type`), so
 * `TsvBinding` withholds it for them. `parse_tape_<lang>` returns the tape as a `Uint32Array`
 * view of the instance's linear memory, valid until the next call into it;
 * `parse_<lang>_json` returns the wire string itself; each returns a parse failure as
 * `{ok: false, error}`. (The raw `parse_<lang>`, the wire `JSON.parse`d engine-side, no
 * published entry calls, so no row does.)
 */
interface WasmModule {
	parse_tape_svelte: EngineFn<Uint32Array | EngineFailure>;
	parse_svelte_json: EngineFn<string | EngineFailure>;
	parse_internal_svelte: EngineFn<EngineFailure | undefined>;
	format_svelte: EngineFn<string | EngineFailure>;
	parse_tape_typescript: EngineFn<Uint32Array | EngineFailure>;
	parse_typescript_json: EngineFn<string | EngineFailure>;
	parse_internal_typescript: EngineFn<EngineFailure | undefined>;
	format_typescript: EngineFn<string | EngineFailure>;
	parse_tape_css: EngineFn<Uint32Array | EngineFailure>;
	parse_css_json: EngineFn<string | EngineFailure>;
	parse_internal_css: EngineFn<EngineFailure | undefined>;
	format_css: EngineFn<string | EngineFailure>;
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
				`WASM module not found at ${wasm_path}. Run 'npm run build:wasm:all:${target}' first.`
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

		// The bundle's flat exports, the tape exports beneath `parse_<lang>` and the wire
		// beneath `parse_<lang>_json` — the staged `parse.js`'s own wiring.
		this.bind({
			parse_tape: {
				svelte: module.parse_tape_svelte,
				typescript: module.parse_tape_typescript,
				css: module.parse_tape_css
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

		// The bindings return a parse failure as `{ok: false, error}`, which the wrapper
		// rethrows as the row's rejection; probed anyway so the three can't come to
		// disagree about what surfacing a refusal MEANS. And the parse wire is span-only, so
		// a stale bundle whose parse still emits the loc-bearing wire can't be timed under the
		// span rows' label — the freshness guard refuses such a bundle unless
		// `BENCH_STALE_OK=1`, and this is what still stands then. The tape probe is the
		// fairness guard the parse rows need: the row's parse runs through the tape export
		// and decodes, in the published wiring, to the tree `JSON.parse` makes of the
		// `_json` wire — so no glue or build regression can hand the rows a cheaper product
		// than the packages' (`lib/tape_probe.ts`).
		this.probe(
			'tsv (WASM)',
			{ path: wasm_path, rebuild: `npm run build:wasm:all:${target}` },
			scope
		);
	}
}
