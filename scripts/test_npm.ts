/**
 * Node.js tests for the built npm packages (@fuzdev/tsv_format_wasm and
 * @fuzdev/tsv_parse_wasm).
 *
 * Verifies the wasm-pack web target + patch_npm_package.ts wrapper works
 * correctly when imported as ESM in Node.js: the auto-init node entry
 * (index.js), the guarded browser entry (browser.js), and the package.json
 * exports/files wiring.
 *
 * Runs under Node (not Deno) on purpose — it validates the package in the
 * runtime consumers use. Node's native type stripping executes the `.ts`
 * directly (requires Node >= 22.18; erasable syntax only).
 *
 * Usage: PKG_DIR=<pkg-dir> node --test scripts/test_npm.ts
 *
 * Examples:
 *   PKG_DIR=crates/tsv_wasm/pkg/format/npm node --test scripts/test_npm.ts
 *   PKG_DIR=crates/tsv_wasm/pkg/parse/npm node --test scripts/test_npm.ts
 *
 * Prerequisites: deno task build:npm:format (or build:npm:parse)
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';

const pkg_dir = process.env.PKG_DIR;
if (!pkg_dir) {
	console.error('Usage: PKG_DIR=<pkg-dir> node --test scripts/test_npm.ts');
	console.error('Example: PKG_DIR=crates/tsv_wasm/pkg/format/npm node --test scripts/test_npm.ts');
	process.exit(1);
}

const is_parse = pkg_dir.includes('parse');

const node_entry = await import(`../${pkg_dir}/index.js`);

describe(`package metadata: ${pkg_dir}`, () => {
	const pkg = JSON.parse(
		readFileSync(new URL(`../${pkg_dir}/package.json`, import.meta.url), 'utf-8'),
	);

	it('has the right name', () => {
		assert.equal(pkg.name, is_parse ? '@fuzdev/tsv_parse_wasm' : '@fuzdev/tsv_format_wasm');
	});

	it('exports map points at files that exist', () => {
		const root = pkg.exports['.'];
		for (const key of ['types', 'node', 'default']) {
			const rel = root[key];
			assert.ok(rel, `exports['.'].${key} missing`);
			assert.ok(
				existsSync(new URL(`../${pkg_dir}/${rel}`, import.meta.url)),
				`exports['.'].${key} → ${rel} does not exist`,
			);
		}
	});

	it('every files[] entry exists', () => {
		for (const rel of pkg.files) {
			assert.ok(
				existsSync(new URL(`../${pkg_dir}/${rel}`, import.meta.url)),
				`files entry ${rel} does not exist`,
			);
		}
	});

	it('index.js is marked side-effectful (auto-init survives tree-shaking)', () => {
		assert.deepEqual(pkg.sideEffects, ['./index.js']);
	});

	it('parse variant bundles tsv_ast.d.ts', { skip: !is_parse }, () => {
		assert.ok(pkg.files.includes('tsv_ast.d.ts'));
	});
});

describe(`node entry (index.js): ${pkg_dir}`, () => {
	it('format_typescript formats', () => {
		assert.equal(node_entry.format_typescript('const   x=1'), 'const x = 1;\n');
	});

	it('format_css formats', () => {
		assert.equal(node_entry.format_css('a{color:red}'), 'a {\n\tcolor: red;\n}\n');
	});

	it('format_svelte formats', () => {
		assert.equal(node_entry.format_svelte('<div   >x</div   >'), '<div>x</div>\n');
	});

	it('formatting is idempotent', () => {
		const once = node_entry.format_svelte('<script>const   x=1</script>\n\n<div>{x}</div>');
		assert.equal(node_entry.format_svelte(once), once);
	});

	it('throws a useful error on invalid syntax', () => {
		assert.throws(() => node_entry.format_typescript('const ='));
	});

	it('parse_typescript returns a Program', { skip: !is_parse }, () => {
		const program = node_entry.parse_typescript('const x = 1;');
		assert.equal(program.type, 'Program');
		assert.ok(Array.isArray(program.body));
	});

	it('parse_typescript_json returns a JSON string', { skip: !is_parse }, () => {
		const json = node_entry.parse_typescript_json('const x = 1;');
		assert.equal(typeof json, 'string');
		assert.equal(JSON.parse(json).type, 'Program');
	});

	it('parse_svelte and parse_css work', { skip: !is_parse }, () => {
		assert.equal(node_entry.parse_svelte('<div>x</div>').type, 'Root');
		assert.equal(node_entry.parse_css('a { color: red }').type, 'StyleSheetFile');
	});
});

// Browser entry (browser.js) — tests the init guard wrapper.
// Imports browser.js which does NOT auto-init WASM, then tests:
// - Pre-init guard throws a clear error
// - Post-init_sync: format functions work, init is idempotent
describe(`browser entry (browser.js): ${pkg_dir}`, () => {
	let browser: any;

	it('import browser.js', async () => {
		browser = await import(`../${pkg_dir}/browser.js`);
	});

	it('format_typescript throws before init', () => {
		assert.throws(() => browser.format_typescript('const x = 1'), /WASM not initialized/);
	});

	it('init_sync initializes WASM', () => {
		const wasm = readFileSync(new URL(`../${pkg_dir}/tsv_wasm_bg.wasm`, import.meta.url));
		browser.init_sync({ module: wasm });
	});

	it('format functions work after init', () => {
		assert.equal(browser.format_typescript('const   x=1'), 'const x = 1;\n');
		assert.equal(browser.format_css('a{color:red}'), 'a {\n\tcolor: red;\n}\n');
		assert.equal(browser.format_svelte('<div   >x</div   >'), '<div>x</div>\n');
	});

	it('parse works after init', { skip: !is_parse }, () => {
		assert.equal(browser.parse_typescript('const x = 1;').type, 'Program');
	});

	it('init is idempotent after init_sync', async () => {
		// Should resolve without re-fetching — just returns early
		await browser.init();
		assert.equal(browser.format_typescript('const   x=1'), 'const x = 1;\n');
	});
});
