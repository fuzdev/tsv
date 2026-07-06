/**
 * Node.js tests for the built tsv_napi addon — the real N-API JS boundary.
 *
 * The in-crate `cargo test` (crates/tsv_napi/src/lib.rs) drives the plain Rust
 * functions the `#[napi]` macro wraps; it does NOT exercise the marshalling
 * layer napi-rs generates (the JS string ↔ Rust `String` conversion, and the
 * `napi::Error` → *thrown* JS error path). This script closes that gap: it loads
 * the built cdylib as an N-API addon and asserts a format, a JSON-AST parse, and
 * a thrown error across the actual JS boundary — the surface a Node/Bun consumer
 * (and the bench's Node runner) hits.
 *
 * Runs under Node (not Deno) on purpose — it validates the addon in the runtime
 * its consumers use. Node's native type stripping executes the `.ts` directly
 * (requires Node >= 22.18; erasable syntax only).
 *
 * Usage: node --test scripts/test_napi.ts   (or `deno task test:napi`)
 * Prerequisite: deno task build:napi (cargo build -p tsv_napi --release)
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { get_napi_library_path, type NapiAddon } from '../benches/js/lib/napi.ts';

const lib_path = get_napi_library_path();
if (!existsSync(lib_path)) {
	console.error(`tsv_napi addon not found at ${lib_path}. Run 'deno task build:napi' first.`);
	process.exit(1);
}

// `process.dlopen` loads a native addon from any path/extension into the passed
// module's `exports` — the supported way to load a `.so`/`.dylib` not named `.node`.
const mod: { exports: NapiAddon } = { exports: {} as NapiAddon };
process.dlopen(mod, lib_path);
const addon = mod.exports;

describe('tsv_napi addon (real N-API JS boundary)', () => {
	it('format_typescript normalizes across the boundary', () => {
		assert.equal(addon.format_typescript('const   x=1'), 'const x = 1;\n');
	});

	it('format_css normalizes', () => {
		assert.equal(addon.format_css('a{color:red}'), 'a {\n\tcolor: red;\n}\n');
	});

	it('format_svelte normalizes', () => {
		assert.equal(addon.format_svelte('<div   >x</div   >'), '<div>x</div>\n');
	});

	it('parse_typescript returns a JSON AST string the host can JSON.parse', () => {
		const ast = JSON.parse(addon.parse_typescript('const x = 1;'));
		assert.equal(ast.type, 'Program');
	});

	it('an engine error surfaces as a thrown JS error (no {error} envelope)', () => {
		// napi-rs converts `napi::Error` into a thrown JS Error — unlike FFI there
		// is no `{"error": …}` envelope to inspect; the throw just propagates.
		assert.throws(() => addon.format_typescript('const = ;'), /.+/);
		assert.throws(() => addon.parse_typescript('const = ;'), /.+/);
	});

	it('multibyte content survives the JS-string marshalling boundary', () => {
		const src = "const x = '€🦀';\n";
		const formatted = addon.format_typescript(src);
		assert.ok(formatted.includes('€🦀'), `multibyte content lost: ${formatted}`);
		// Re-formatting is stable (idempotent) across the boundary.
		assert.equal(addon.format_typescript(formatted), formatted);
	});
});
