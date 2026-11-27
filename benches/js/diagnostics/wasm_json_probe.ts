/**
 * Diagnostic: attribute the WASM-vs-native parse-materialization penalty.
 *
 * Both default parse rows take the tape route (`docs/tape_format.md`):
 *
 * Native:  parse -> the wire writer's tape -> the tape's words copied into a JS-owned
 *          `Uint32Array` (N-API) -> generated decoder (JS)
 * WASM:    parse -> the wire writer's tape -> a `Uint32Array` view of linear memory ->
 *          generated decoder (JS)
 *
 * Both share the parse; they differ in materialization. This splits total into parse vs
 * materialization for each, and prices `JSON.parse` of the same wire beside it — the JS step a JSON
 * route would take instead of the decode. Both paths are the bench rows' own calls, so they run
 * through the packages' facade (`lib/tsv_api.ts`), whose argument checks land in the
 * materialization share; `diagnostics/facade_probe.ts` prices those alone, and its `--transfer`
 * arms split the tape route itself.
 *
 * The native side is the bench's own: the profile-guided N-API addon
 * (`npm run build:bench:pgo`), beside the `nodejs` WASM bundle (`npm run build:wasm:all:nodejs`),
 * so the split it prints is the one the bench's rows pay.
 *
 * Run: node --disable-warning=ExperimentalWarning benches/js/roots/wasm_json_probe.ts 2>&1 >/dev/null
 *
 * `run_wasm_json_probe` is the library function; its root,
 * `benches/js/roots/wasm_json_probe.ts`, binds its deps (`WasmJsonProbeDeps`).
 *
 * @module
 */

import { group_by_language } from '../lib/corpus.ts';
import type { Logger, SourceFile } from '../lib/types.ts';

const ITERS = 5;

/** One binding's two parse calls, as the probe times them. */
export interface JsonParseBinding {
	parse(source: string, language: 'typescript'): unknown;
	parse_internal(source: string, language: 'typescript'): void;
}

/** What `run_wasm_json_probe` reaches the world through. */
export interface WasmJsonProbeDeps {
	/** The `gates` corpus view. */
	load_files(logger: Logger): Promise<SourceFile[]>;
	/** tsv's native binding and its WASM bundle. */
	init_implementations(
		logger: Logger
	): Promise<{ native: JsonParseBinding; wasm: JsonParseBinding }>;
}

/** Times the native and WASM JSON parse paths over the corpus's TypeScript, to stderr. */
export async function run_wasm_json_probe(deps: WasmJsonProbeDeps): Promise<void> {
	const logger: Logger = (m) => console.error(m);
	const [files, impls] = await Promise.all([
		deps.load_files(logger),
		deps.init_implementations(logger)
	]);
	const ts = group_by_language(files).typescript;
	const native = impls.native;
	const wasm = impls.wasm;

	// Use only files both native and wasm materialize (parity with bench
	// intersection). The filter pass's native parse doubles as the wire-payload
	// capture: re-stringifying the parsed AST ≈ what to_string emits (a
	// byte-exact wire would need the binding to expose the raw string;
	// JSON.stringify can alter number formatting).
	const ok: typeof ts = [];
	const wire: string[] = [];
	for (const f of ts) {
		try {
			const parsed = native.parse(f.content, 'typescript');
			wasm.parse(f.content, 'typescript');
			ok.push(f);
			wire.push(JSON.stringify(parsed));
		} catch {
			// skip files either impl rejects
		}
	}
	console.error(`\nProbing ${ok.length} TS files × ${ITERS} iters\n`);

	function time(fn: () => void): number {
		const t0 = performance.now();
		fn();
		return performance.now() - t0;
	}

	let native_total = 0,
		native_internal = 0,
		wasm_total = 0,
		wasm_internal = 0,
		js_jsonparse = 0,
		js_jsonstringify = 0;

	for (let i = 0; i < ITERS; i++) {
		for (const f of ok) native_total += time(() => void native.parse(f.content, 'typescript'));
		for (const f of ok) {
			native_internal += time(() => native.parse_internal(f.content, 'typescript'));
		}
		for (const f of ok) wasm_total += time(() => void wasm.parse(f.content, 'typescript'));
		for (const f of ok) {
			wasm_internal += time(() => wasm.parse_internal(f.content, 'typescript'));
		}
		for (const s of wire) js_jsonparse += time(() => void JSON.parse(s));
		for (const s of wire) {
			const o = JSON.parse(s);
			js_jsonstringify += time(() => void JSON.stringify(o));
		}
	}

	const ms = (n: number) => (n / ITERS).toFixed(0).padStart(7);
	const encoder = new TextEncoder();
	const wire_mb = wire.reduce((a, s) => a + encoder.encode(s).length, 0) / 1e6;

	console.error(`wire JSON size (one pass, UTF-8 bytes): ${wire_mb.toFixed(1)} MB\n`);
	console.error(`native parse total          ${ms(native_total)} ms`);
	console.error(`native parse_internal       ${ms(native_internal)} ms  (pure parse)`);
	console.error(
		`  -> native materialization ${ms(native_total - native_internal)} ms  (tape write+copy+decode)`
	);
	console.error(`wasm   parse total          ${ms(wasm_total)} ms`);
	console.error(`wasm   parse_internal       ${ms(wasm_internal)} ms  (pure parse)`);
	console.error(
		`  -> wasm materialization   ${ms(wasm_total - wasm_internal)} ms  (tape write+view+decode)`
	);
	console.error(`\nJS-side only (the JSON route's JS step, for reference):`);
	console.error(`  JSON.parse(wire)          ${ms(js_jsonparse)} ms`);
	console.error(`  JSON.stringify(obj)       ${ms(js_jsonstringify)} ms`);
	console.error(
		`\nwasm matz / native matz = ${(
			(wasm_total - wasm_internal) / (native_total - native_internal)
		).toFixed(2)}x`
	);
	console.error(
		`JSON.parse(wire) as share of native matz = ${(
			(js_jsonparse / (native_total - native_internal)) * 100
		).toFixed(0)}%`
	);
}
