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
 *
 * Two host-level contracts live here that no in-crate test can reach: the panic
 * contract (a Rust panic must throw, not abort) and the THREADING contract (the
 * addon must load in a worker thread, and its thread-local arenas must stay
 * thread-local under concurrency). Both are properties of the shipped artifact
 * rather than of the engine, which is why they are asserted at this boundary.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { Worker } from 'node:worker_threads';
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

	// Worker threads are the one host shape that exercises the addon's threading
	// contract, and it has two independent halves:
	//
	//   1. **Context-awareness.** A worker is a fresh JS context in the same
	//      process. A non-context-aware addon either refuses to load there or
	//      corrupts state shared across contexts, and nothing else in the suite
	//      loads the addon twice.
	//   2. **The per-thread arenas.** Every export runs inside `with_ast_arena` /
	//      `with_doc_arena` (crate `tsv_arena`), which are thread-local and reused
	//      across calls. Single-threaded tests cannot distinguish "per thread" from
	//      "one global", so a change that made an arena shared would pass everything
	//      else and corrupt output only under concurrency — the failure mode a
	//      consumer hits in a worker pool and never in CI.
	//
	// Each worker formats and parses in a loop and reports the DISTINCT outputs it
	// observed; a torn or interleaved result shows up as a second distinct value,
	// which is why the assertion is on the distinct set rather than on a sample.
	it('loads and produces stable output across concurrent worker threads', async () => {
		const worker_count = 4;
		const iterations = 50;
		const workers = Array.from(
			{ length: worker_count },
			() =>
				new Worker(WORKER_SOURCE, {
					eval: true,
					workerData: { lib_path, iterations }
				})
		);
		const outcomes = await Promise.all(workers.map(await_worker_message));
		for (const [i, outcome] of outcomes.entries()) {
			assert.deepEqual(
				outcome.formatted,
				['const x = 1;\n'],
				`worker ${i} saw more than one format result`
			);
			assert.deepEqual(outcome.css, ['a {\n\tcolor: red;\n}\n'], `worker ${i} css result drifted`);
			assert.deepEqual(outcome.parsed_types, ['Program'], `worker ${i} parse result drifted`);
			assert.equal(outcome.errors, iterations, `worker ${i} lost the thrown-error path`);
		}
	});
});

/** What `WORKER_SOURCE` posts back: the DISTINCT results each worker observed. */
interface WorkerOutcome {
	formatted: string[];
	css: string[];
	parsed_types: string[];
	errors: number;
}

/**
 * The worker body, as CommonJS source for `new Worker(..., { eval: true })`.
 *
 * Inline rather than a sibling file on purpose: a second file would need Node's
 * type stripping to reach it, and the point of the test is the addon's behavior in
 * a bare worker context, not the loader path that got the worker there.
 */
const WORKER_SOURCE = `
const { parentPort, workerData } = require('node:worker_threads');
const mod = { exports: {} };
process.dlopen(mod, workerData.lib_path);
const addon = mod.exports;
const formatted = new Set();
const css = new Set();
const parsed_types = new Set();
let errors = 0;
for (let i = 0; i < workerData.iterations; i++) {
	formatted.add(addon.format_typescript('const   x=1'));
	css.add(addon.format_css('a{color:red}'));
	parsed_types.add(JSON.parse(addon.parse_typescript('const x = 1;')).type);
	// A throw on every iteration too: the error path unwinds through the same
	// arenas, so an arena the panic/error path fails to park would surface as a
	// corrupted result on the NEXT iteration rather than as an error here.
	try {
		addon.format_typescript('const = ;');
	} catch {
		errors++;
	}
}
parentPort.postMessage({
	formatted: [...formatted],
	css: [...css],
	parsed_types: [...parsed_types],
	errors
});
`;

/**
 * Resolve with a worker's single posted message, rejecting on a worker error or a
 * non-zero exit. The `exit` arm is what turns an ABORT — the failure mode this
 * whole file exists to rule out — into a failed assertion instead of a hung test:
 * an aborting worker posts nothing, so a message-only wait would time out with no
 * attribution.
 */
function await_worker_message(worker: Worker): Promise<WorkerOutcome> {
	return new Promise<WorkerOutcome>((resolve, reject) => {
		worker.once('message', resolve);
		worker.once('error', reject);
		worker.once('exit', (code) => {
			if (code !== 0) reject(new Error(`worker exited with code ${code} before posting a result`));
		});
	});
}
