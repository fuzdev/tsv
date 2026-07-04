/**
 * Diagnostic: attribute the WASM-vs-native JSON parse penalty.
 *
 * Native JSON path:  parse -> convert_ast_json_string -> FFI copy -> JSON.parse (JS)
 *                    (the wire-JSON writer emits directly from the internal AST)
 * WASM JSON path:    parse -> convert_ast_json_string -> boundary string decode
 *                    -> engine JSON.parse (called from Rust via js_sys)
 *
 * Both share the parse; they differ in materialization. This splits total
 * into parse vs materialization for each, and isolates the JS-side
 * JSON.parse cost.
 *
 * Run: deno run --allow-ffi --allow-read --allow-env --allow-net --allow-sys \
 *   benches/js/diagnostics/wasm_json_probe.ts 2>&1 >/dev/null
 */

import { DevReposLoader, group_by_language } from '../lib/corpus.ts';
import { init_implementations } from '../lib/implementations.ts';

const ITERS = 5;

const [files, impls] = await Promise.all([
	new DevReposLoader('gates').load((m) => console.error(m)),
	init_implementations({ logger: (m) => console.error(m) }),
]);
const ts = group_by_language(files).typescript;
if (!impls.native) throw new Error('native FFI not built');
if (!impls.wasm) throw new Error('wasm not built');
const native = impls.native;
const wasm = impls.wasm;

// Use only files both native and wasm materialize (parity with bench
// intersection). The filter pass's native parse doubles as the wire-payload
// capture: re-stringifying the parsed AST ≈ what to_string emits (a
// byte-exact wire would need ffi.ts to expose the raw string;
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
	`  -> native materialization ${
		ms(native_total - native_internal)
	} ms  (convert+translate+to_string+copy+JSON.parse)`,
);
console.error(`wasm   parse total          ${ms(wasm_total)} ms`);
console.error(`wasm   parse_internal       ${ms(wasm_internal)} ms  (pure parse)`);
console.error(
	`  -> wasm materialization   ${
		ms(wasm_total - wasm_internal)
	} ms  (to_string+boundary+JSON.parse)`,
);
console.error(`\nJS-side only:`);
console.error(`  JSON.parse(wire)          ${ms(js_jsonparse)} ms`);
console.error(`  JSON.stringify(obj)       ${ms(js_jsonstringify)} ms`);
console.error(
	`\nwasm matz / native matz = ${
		((wasm_total - wasm_internal) / (native_total - native_internal)).toFixed(2)
	}x`,
);
console.error(
	`JS JSON.parse as share of native matz = ${
		((js_jsonparse / (native_total - native_internal)) * 100).toFixed(0)
	}%`,
);
