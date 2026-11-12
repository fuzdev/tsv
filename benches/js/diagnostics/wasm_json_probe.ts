/**
 * Diagnostic: attribute the WASM-vs-native JSON parse penalty.
 *
 * Native JSON path:  parse -> convert_ast_json_bytes -> FFI copy
 *                    -> JSON.parse (JS) (the span-only wire every binding ships; the
 *                    wire-JSON writer emits directly from the internal AST)
 * WASM JSON path:    parse -> convert_ast_json_string -> boundary string
 *                    decode -> engine JSON.parse (called from Rust via js_sys)
 *
 * Both share the parse; they differ in materialization. This splits total
 * into parse vs materialization for each, and isolates the JS-side
 * JSON.parse cost. Both JSON paths are the bench rows' own calls, so they run
 * through the packages' facade (`lib/tsv_api.ts`), whose argument checks land in
 * the materialization share; `diagnostics/facade_probe.ts` prices those alone.
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
		`  -> native materialization ${ms(
			native_total - native_internal
		)} ms  (convert+translate+to_string+copy+JSON.parse)`
	);
	console.error(`wasm   parse total          ${ms(wasm_total)} ms`);
	console.error(`wasm   parse_internal       ${ms(wasm_internal)} ms  (pure parse)`);
	console.error(
		`  -> wasm materialization   ${ms(
			wasm_total - wasm_internal
		)} ms  (to_string+boundary+JSON.parse)`
	);
	console.error(`\nJS-side only:`);
	console.error(`  JSON.parse(wire)          ${ms(js_jsonparse)} ms`);
	console.error(`  JSON.stringify(obj)       ${ms(js_jsonstringify)} ms`);
	console.error(
		`\nwasm matz / native matz = ${(
			(wasm_total - wasm_internal) / (native_total - native_internal)
		).toFixed(2)}x`
	);
	console.error(
		`JS JSON.parse as share of native matz = ${(
			(js_jsonparse / (native_total - native_internal)) * 100
		).toFixed(0)}%`
	);
}
