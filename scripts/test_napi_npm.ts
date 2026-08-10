/**
 * Node.js tests for the staged N-API npm packages — the `@fuzdev/tsv_napi`
 * loader over a real platform package, consumed the way npm lays them out.
 *
 * `scripts/test_napi.ts` covers the raw addon boundary; this covers the
 * PACKAGE surface a consumer installs: the loader's platform resolution, the
 * ESM-imports-CJS named interop (the loader is CommonJS with static
 * `exports.<name> =` assignments — importing it as ESM here proves the named
 * bindings resolve), the wasm-parity `(source, options?)` bags with their
 * exact error strings, the `_json` string variants, the package.json
 * selection fields, and the unsupported-platform error.
 *
 * Stages `crates/tsv_napi/pkg/{napi,<triple>}` into a temp `node_modules`
 * (the loader's `require('@fuzdev/tsv_napi-<triple>')` resolves upward from
 * its own location, so a copy inside a temp node_modules resolves its sibling
 * there). A second staging WITHOUT the platform package asserts the
 * unsupported-platform error path.
 *
 * Usage: node --test scripts/test_napi_npm.ts   (or `deno task test:napi:npm`)
 * Prerequisite: deno task build:napi:packages
 */

import { after, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
	cpSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
	readFileSync,
	rmSync
} from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const pkg_root = 'crates/tsv_napi/pkg';
if (!existsSync(join(pkg_root, 'napi'))) {
	console.error(`${pkg_root}/napi not staged. Run 'deno task build:napi:packages' first.`);
	process.exit(1);
}
const platform_dirs = readdirSync(pkg_root).filter((d) => d !== 'napi');
if (platform_dirs.length !== 1) {
	console.error(
		`expected exactly one staged platform package under ${pkg_root}, got: ${platform_dirs.join(', ') || '(none)'}`
	);
	process.exit(1);
}
// The staged dir name is the BUILD script's triple detection (Deno-side);
// the loader detects with Node's own APIs. The import below succeeds only
// when the two agree, so host-triple agreement is gated here for free.
const triple = platform_dirs[0]!;

/** Stage the loader (+ optionally the platform package) into a fresh temp node_modules. */
const stage = (with_platform: boolean): string => {
	const tmp = mkdtempSync(join(tmpdir(), 'tsv_napi_npm_'));
	const scope = join(tmp, 'node_modules', '@fuzdev');
	mkdirSync(scope, { recursive: true });
	cpSync(join(pkg_root, 'napi'), join(scope, 'tsv_napi'), { recursive: true });
	if (with_platform) {
		cpSync(join(pkg_root, triple), join(scope, `tsv_napi-${triple}`), { recursive: true });
	}
	return tmp;
};

const staged = stage(true);
const staged_bare = stage(false);
after(() => {
	rmSync(staged, { recursive: true, force: true });
	rmSync(staged_bare, { recursive: true, force: true });
});

const loader_path = join(staged, 'node_modules', '@fuzdev', 'tsv_napi', 'index.js');
// ESM import of the CJS loader — named bindings must resolve via the interop.
const api = await import(pathToFileURL(loader_path).href);

/** Assert `fn` throws an Error whose message contains `needle` exactly. */
const throws_with = (fn: () => unknown, needle: string): void => {
	assert.throws(fn, (e: unknown) => {
		assert.ok(e instanceof Error, `expected an Error, got ${typeof e}`);
		assert.ok(
			e.message.includes(needle),
			`error message missing ${JSON.stringify(needle)}: ${e.message}`
		);
		return true;
	});
};

describe('@fuzdev/tsv_napi loader (staged npm shape)', () => {
	it('formats every language', () => {
		assert.equal(api.format_typescript('const   x=1'), 'const x = 1;\n');
		assert.equal(api.format_css('a{color:red}'), 'a {\n\tcolor: red;\n}\n');
		assert.equal(api.format_svelte('<div   >x</div   >'), '<div>x</div>\n');
	});

	it('parses to objects with loc, and _json siblings return the string', () => {
		const ast = api.parse_typescript('const x = 1;');
		assert.equal(ast.type, 'Program');
		assert.ok(ast.body[0].loc, 'default wire carries loc');
		assert.equal(api.parse_svelte('<div>x</div>').type, 'Root');
		assert.equal(api.parse_css('a { color: red }').type, 'StyleSheetFile');
		const json = api.parse_typescript_json('const x = 1;');
		assert.equal(typeof json, 'string');
		assert.deepEqual(JSON.parse(json), ast);
	});

	it('locations: false selects the span-only wire (inert for CSS)', () => {
		const ast = api.parse_typescript('const x = 1;', { locations: false });
		assert.equal(ast.type, 'Program');
		assert.equal(ast.body[0].loc, undefined, 'span-only wire omits loc');
		assert.equal(
			api.parse_svelte('<div>x</div>', { locations: false }).fragment.nodes[0].loc,
			undefined
		);
		assert.equal(
			api.parse_css_json('a { color: red }', { locations: false }),
			api.parse_css_json('a { color: red }'),
			'CSS wire has no loc, so the option is inert'
		);
	});

	it('the TypeScript goal axis reaches parse AND format', () => {
		const script_only = 'var await = 1;\n';
		assert.equal(api.parse_typescript(script_only, { goal: 'script' }).type, 'Program');
		assert.throws(() => api.parse_typescript(script_only, { goal: 'module' }));
		assert.throws(() => api.parse_typescript(script_only));
		assert.equal(api.format_typescript('var   await=1', { goal: 'script' }), 'var await = 1;\n');
		assert.throws(() => api.format_typescript('var   await=1'));
		throws_with(
			() => api.parse_typescript('const x = 1;', { goal: 'sloppy' }),
			"invalid goal 'sloppy' (expected 'script' or 'module')"
		);
	});

	it('option bags carry the wasm package error semantics, string for string', () => {
		// Unknown keys error whatever their value — undefined included.
		throws_with(
			() => api.parse_typescript('const x = 1;', { locatons: false }),
			"unknown parse option 'locatons' (expected 'locations' or 'goal')"
		);
		throws_with(
			() => api.parse_typescript('const x = 1;', { locatons: undefined }),
			"unknown parse option 'locatons'"
		);
		// The TS-only goal on other languages: a SET goal throws, undefined forwards.
		throws_with(
			() => api.parse_svelte('<div>x</div>', { goal: 'script' }),
			"parse option 'goal' is only supported for TypeScript"
		);
		assert.equal(api.parse_svelte('<div>x</div>', { goal: undefined }).type, 'Root');
		throws_with(
			() => api.format_css('a{}', { goal: 'script' }),
			"format option 'goal' is only supported for TypeScript"
		);
		// `locations` shapes a parse wire; format emits none — unknown key there.
		throws_with(
			() => api.format_typescript('const x = 1;', { locations: false }),
			"unknown format option 'locations' (expected 'goal')"
		);
		throws_with(
			() => api.format_css('a{}', { locations: false }),
			"unknown format option 'locations' (this export takes no options)"
		);
		// Non-object bags error, arrays included (`sources.map(format_typescript)`).
		throws_with(
			() => api.format_typescript('const x = 1;', ['script']),
			'format options must be an object'
		);
		throws_with(() => api.parse_typescript('const x = 1;', 7), 'parse options must be an object');
		// A boolean-typed key rejects non-booleans.
		throws_with(
			() => api.parse_typescript('const x = 1;', { locations: 'no' }),
			"parse option 'locations' must be a boolean"
		);
	});

	it('parse errors and engine errors are thrown JS errors', () => {
		assert.throws(() => api.parse_typescript('const = ;'));
		assert.throws(() => api.format_svelte('<div {'));
	});

	it('multibyte content survives, and formatting is idempotent', () => {
		const src = "const x = '€🦀';\n";
		const formatted = api.format_typescript(src);
		assert.ok(formatted.includes('€🦀'));
		assert.equal(api.format_typescript(formatted), formatted);
	});

	it('CJS require of the package works too', () => {
		const req = createRequire(loader_path);
		const cjs = req('@fuzdev/tsv_napi');
		assert.equal(cjs.format_typescript('const   x=1'), 'const x = 1;\n');
	});

	it('package.json selection fields and pins are coherent', () => {
		const loader_pkg = JSON.parse(
			readFileSync(join(staged, 'node_modules', '@fuzdev', 'tsv_napi', 'package.json'), 'utf8')
		);
		const platform_pkg = JSON.parse(
			readFileSync(
				join(staged, 'node_modules', '@fuzdev', `tsv_napi-${triple}`, 'package.json'),
				'utf8'
			)
		);
		// Exact-version lockstep: every platform pin is the loader's own version.
		for (const [name, pin] of Object.entries(loader_pkg.optionalDependencies)) {
			assert.equal(pin, loader_pkg.version, `${name} must pin the loader version exactly`);
		}
		assert.equal(platform_pkg.version, loader_pkg.version);
		// The staged platform package's selection fields match its triple.
		const [os, cpu, libc] = triple.split('-');
		assert.deepEqual(platform_pkg.os, [os]);
		assert.deepEqual(platform_pkg.cpu, [cpu]);
		if (libc) assert.deepEqual(platform_pkg.libc, [libc === 'gnu' ? 'glibc' : libc]);
		assert.equal(platform_pkg.main, 'tsv_napi.node');
		// The loader's SUPPORTED list and its optionalDependencies must agree —
		// the build script and index.js each carry the list, and this is the
		// gate that keeps them in sync.
		const source = readFileSync(loader_path, 'utf8');
		const supported = [...source.matchAll(/^\t'([a-z0-9]+-[a-z0-9-]+)'/gm)].map((m) => m[1]);
		assert.deepEqual(
			supported.map((t) => `@fuzdev/tsv_napi-${t}`).sort(),
			Object.keys(loader_pkg.optionalDependencies).sort(),
			'npm/index.js SUPPORTED must match the generated optionalDependencies'
		);
		// Every `files` entry the loader package declares actually shipped.
		for (const file of loader_pkg.files) {
			assert.ok(
				existsSync(join(staged, 'node_modules', '@fuzdev', 'tsv_napi', file)),
				`declared file missing: ${file}`
			);
		}
	});

	it('an unsupported platform fails loudly and points at the WASM package', async () => {
		const bare_loader = join(staged_bare, 'node_modules', '@fuzdev', 'tsv_napi', 'index.js');
		await assert.rejects(import(pathToFileURL(bare_loader).href), (e: unknown) => {
			assert.ok(e instanceof Error);
			assert.ok(e.message.includes(triple), `message names the triple: ${e.message}`);
			assert.ok(
				e.message.includes('@fuzdev/tsv_wasm'),
				`message points at the WASM fallback: ${e.message}`
			);
			return true;
		});
	});
});
