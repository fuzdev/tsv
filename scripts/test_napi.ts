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
 * Prerequisite: deno task build:napi:probe (the `napi` profile — release +
 * unwind — with the test-only `panic_probe` feature, whose export drives the
 * panic-contract test below). Also runs against a plain `build:napi` artifact
 * (the shipped shape, no probe): the contract test then reports as skipped.
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

	// The panic contract: every export carries `catch_unwind` and the addon
	// builds with the unwinding `napi` profile, so a Rust panic — always a tsv
	// bug — must surface as a thrown JS error with the process AND the
	// per-thread arenas still usable, never abort the host. Driven through the
	// test-only `__panic_probe` export (the `panic_probe` cargo feature, which
	// `deno task test:napi` builds; a published artifact has no probe, so this
	// test skips against one). The probe panics INSIDE `with_ast_arena`, so the
	// calls after it also prove the take/park arena recovery across the real
	// boundary.
	it(
		'a panic surfaces as a thrown JS error and the addon stays usable',
		{ skip: addon.__panic_probe === undefined && 'panic_probe feature not built' },
		() => {
			assert.throws(() => addon.__panic_probe!(), /panic/i, 'panic must throw');
			// Repeatable: a second panic must also throw, not abort.
			assert.throws(() => addon.__panic_probe!(), /panic/i, 'second panic must throw');
			// The process survived and the arenas recovered: parse (AST arena)
			// and format (AST + doc arenas) still produce correct output.
			const ast = JSON.parse(addon.parse_typescript('const x = 1;'));
			assert.equal(ast.type, 'Program', 'parse must work after a panic');
			assert.equal(
				addon.format_typescript('const   x=1'),
				'const x = 1;\n',
				'format must work after a panic'
			);
		}
	);
});
