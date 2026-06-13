/**
 * Patches a wasm-pack `--target web` build into the published npm package shape.
 *
 * Creates:
 * - index.js — Node.js/Bun entry: auto-init via readFileSync + initSync (zero config)
 * - browser.js — Browser/default entry: async `init()` with not-initialized guards
 * - index.d.ts — type declarations for both entries
 *
 * Also patches package.json (name, description, conditional exports, npm
 * metadata) and copies the variant README + repo LICENSE into the package
 * root. The crate has no `README.md` at its root — `README_format.md`,
 * `README_parse.md`, and `README_all.md` are the canonical sources and ship
 * as each package's `README.md`.
 *
 * The exported function list is extracted from the generated `tsv_wasm.js`
 * (every `export function format_*` / `parse_*`), so adding a language to
 * `lang_bindings!` flows through with no changes here. `parse_internal_*`
 * exports are bench-only and excluded from the wrappers.
 *
 * For the variants with parse exports (`parse`, `all`), also copies
 * `crates/tsv_wasm/types/tsv_ast.d.ts` into the package root alongside the
 * generated `tsv_wasm.d.ts`. The wasm-bindgen
 * `typescript_type = "import('./tsv_ast').*"` extern types resolve against
 * this bundled file at consumer compile time.
 *
 * The `all` variant additionally ships the CLI: `crates/tsv_wasm/npm/cli.js`
 * is copied into the package root and wired up as the `tsv` bin.
 *
 * Usage:  patch_npm_package.ts <format|parse|all>
 *
 *   format → crates/tsv_wasm/pkg/format/npm/ → @fuzdev/tsv_format_wasm
 *   parse  → crates/tsv_wasm/pkg/parse/npm/  → @fuzdev/tsv_parse_wasm
 *   all    → crates/tsv_wasm/pkg/all/npm/    → @fuzdev/tsv_wasm
 */

import { format_size, gzip_size } from './size.ts';

const variant = Deno.args[0];
if (variant !== 'format' && variant !== 'parse' && variant !== 'all') {
	console.error(`Usage: patch_npm_package.ts <format|parse|all>`);
	Deno.exit(1);
}

const PKG_NAMES = {
	format: '@fuzdev/tsv_format_wasm',
	parse: '@fuzdev/tsv_parse_wasm',
	all: '@fuzdev/tsv_wasm',
} as const;
const pkg_name = PKG_NAMES[variant];
const has_format_exports = variant !== 'parse';
const has_parse_exports = variant !== 'format';
const pkg_root = `crates/tsv_wasm/pkg/${variant}/npm`;
const main_js = 'tsv_wasm.js';
const dts_file = 'tsv_wasm.d.ts';
const wasm_file = 'tsv_wasm_bg.wasm';
const cli_file = 'cli.js';

// 1. Extract the public function exports from the generated JS.

const generated_js = Deno.readTextFileSync(`${pkg_root}/${main_js}`);
const fns = [...generated_js.matchAll(/^export function (\w+)/gm)]
	.map((m) => m[1])
	.filter((name) => /^(format|parse)_/.test(name) && !name.startsWith('parse_internal_'))
	.sort();

const expected_formats = ['format_css', 'format_svelte', 'format_typescript'];
const has_format = fns.some((name) => name.startsWith('format_'));
const has_parse = fns.some((name) => name.startsWith('parse_'));
if (has_format_exports) {
	for (const name of expected_formats) {
		if (!fns.includes(name)) {
			console.error(`FAIL: generated ${main_js} is missing expected export \`${name}\``);
			Deno.exit(1);
		}
	}
} else if (has_format) {
	console.error(`FAIL: ${variant} variant contains format_* exports — stale build dir?`);
	Deno.exit(1);
}
if (has_parse_exports && !has_parse) {
	console.error(
		`FAIL: ${variant} variant has no parse_* exports — was \`--features parse\` passed?`,
	);
	Deno.exit(1);
}
if (!has_parse_exports && has_parse) {
	console.error(`FAIL: ${variant} variant contains parse_* exports — stale build dir?`);
	Deno.exit(1);
}
console.log(`Exports: ${fns.join(', ')}`);

// 2. Create index.js — Node.js/Bun entry: auto-init via readFileSync + initSync.
// WASM is initialized synchronously at import time, so no init guard needed.

const index_js = `import { readFileSync } from 'node:fs';
import {
	default as init,
	initSync,
${fns.map((f) => `\t${f},`).join('\n')}
} from './${main_js}';

const wasm = readFileSync(new URL('./${wasm_file}', import.meta.url));
initSync({ module: wasm });

export { init, initSync as init_sync, ${fns.join(', ')} };
`;

Deno.writeTextFileSync(`${pkg_root}/index.js`, index_js);
console.log(`Created ${pkg_root}/index.js`);

// 3. Create browser.js — Browser/default entry: async init() with guards.
// Vite and other bundlers pick this via the "default" export condition and handle
// the `new URL('./tsv_wasm_bg.wasm', import.meta.url)` pattern natively.

const browser_js = `import {
	default as _init,
	initSync,
${fns.map((f) => `\t${f} as _${f},`).join('\n')}
} from './${main_js}';

let _ready = false;

function _check() {
	if (!_ready) throw new Error('${pkg_name}: WASM not initialized. Call \\\`await init()\\\` first.');
}

/** Initialize the WASM module. Required in browsers before calling any other export. No-op if already initialized. */
export async function init(...args) {
	if (_ready) return;
	await _init(...args);
	_ready = true;
}

/** Synchronously initialize the WASM module. Works in Workers (not Chrome main thread for >4KB WASM). */
export function init_sync(...args) {
	if (_ready) return;
	initSync(...args);
	_ready = true;
}

${
	fns
		.map(
			(f) =>
				`export function ${f}(source) {
	_check();
	return _${f}(source);
}`,
		)
		.join('\n\n')
}
`;

Deno.writeTextFileSync(`${pkg_root}/browser.js`, browser_js);
console.log(`Created ${pkg_root}/browser.js`);

// 4. Create index.d.ts — type declarations for both entries.
// Re-exports the generated function types, but declares init/init_sync with clean
// signatures to avoid leaking wasm-bindgen internals (InitOutput with raw pointers).

const ast_reexport = has_parse_exports ? `export type * from './tsv_ast';\n` : '';
const index_dts = `${ast_reexport}export {
${fns.map((f) => `\t${f},`).join('\n')}
} from './${dts_file.replace(/\.d\.ts$/, '')}';
/** Initialize the WASM module. Required in browsers before calling any other export. No-op if already initialized. */
export declare function init(module_or_path?: {
	module_or_path: RequestInfo | URL | Response | BufferSource | WebAssembly.Module;
}): Promise<void>;
/** Synchronously initialize the WASM module. Works in Node.js and Workers (not Chrome main thread for >4KB WASM). */
export declare function init_sync(module: {
	module: BufferSource | WebAssembly.Module;
}): void;
`;

Deno.writeTextFileSync(`${pkg_root}/index.d.ts`, index_dts);
console.log(`Created ${pkg_root}/index.d.ts`);

// 5. Copy the variant README and the repo LICENSE into the package root.

const readme_src = `crates/tsv_wasm/README_${variant}.md`;
Deno.copyFileSync(readme_src, `${pkg_root}/README.md`);
console.log(`Copied ${readme_src} → ${pkg_root}/README.md`);

Deno.copyFileSync('LICENSE', `${pkg_root}/LICENSE`);
console.log(`Copied LICENSE → ${pkg_root}/LICENSE`);

if (has_parse_exports) {
	// Bundle the hand-maintained AST types alongside the generated `tsv_wasm.d.ts`.
	Deno.copyFileSync('crates/tsv_wasm/types/tsv_ast.d.ts', `${pkg_root}/tsv_ast.d.ts`);
	console.log(`Copied crates/tsv_wasm/types/tsv_ast.d.ts → ${pkg_root}/tsv_ast.d.ts`);
}

if (variant === 'all') {
	// The full-tool package ships the CLI (`tsv` bin); the subsets stay pure libraries.
	Deno.copyFileSync(`crates/tsv_wasm/npm/${cli_file}`, `${pkg_root}/${cli_file}`);
	console.log(`Copied crates/tsv_wasm/npm/${cli_file} → ${pkg_root}/${cli_file}`);
}

// 6. Patch package.json.

const pkg_path = `${pkg_root}/package.json`;
const pkg = JSON.parse(Deno.readTextFileSync(pkg_path));

pkg.name = pkg_name;
pkg.description = {
	format: 'formatter for Svelte, TypeScript, and CSS',
	parse: 'parser for Svelte, TypeScript, and CSS',
	all: 'formatter and parser for Svelte, TypeScript, and CSS',
}[variant];
pkg.type = 'module';
pkg.exports = {
	'./package.json': './package.json',
	'.': {
		types: './index.d.ts',
		node: './index.js',
		default: './browser.js',
	},
};
if (variant === 'all') {
	pkg.bin = { tsv: `./${cli_file}` };
}
pkg.files = [
	'index.js',
	'index.d.ts',
	'browser.js',
	main_js,
	dts_file,
	wasm_file,
	...(has_parse_exports ? ['tsv_ast.d.ts'] : []),
	...(variant === 'all' ? [cli_file] : []),
	'README.md',
	'LICENSE',
];
pkg.keywords = [
	'typescript',
	'svelte',
	'css',
	...(has_format_exports ? ['formatter', 'prettier'] : []),
	...(has_parse_exports ? ['parser', 'ast', 'acorn'] : []),
	...(variant === 'all' ? ['cli'] : []),
	'wasm',
	'webassembly',
];
pkg.homepage = 'https://github.com/fuzdev/tsv';
pkg.author = {
	name: 'Ryan Atkinson',
	email: 'mail@ryanatkn.com',
	url: 'https://www.ryanatkn.com/',
};
pkg.repository = {
	type: 'git',
	url: 'git+https://github.com/fuzdev/tsv.git',
};
pkg.bugs = 'https://github.com/fuzdev/tsv/issues';
pkg.funding = 'https://www.ryanatkn.com/funding';
pkg.engines = { node: '>=20' };
// wasm-pack emits `sideEffects: ["./snippets/*"]`, declaring index.js
// side-effect-free — but its top-level readFileSync + initSync IS the side
// effect. Without this, a tree-shaking bundler on the `node` condition may
// bypass the re-export facade and skip initialization entirely.
pkg.sideEffects = ['./index.js'];

// Remove wasm-pack web target fields superseded by exports
delete pkg.main;
delete pkg.types;
delete pkg.module;

Deno.writeTextFileSync(pkg_path, JSON.stringify(pkg, null, '\t') + '\n');
console.log(`Patched ${pkg_path}: name → ${pkg.name}, version ${pkg.version}`);

await print_summary(pkg_root, [...pkg.files, 'package.json']);

/** Lists what actually ships (`files[]` + package.json), not everything in
 * the build dir — wasm-pack leaves strays like `tsv_wasm_bg.wasm.d.ts`. */
async function print_summary(dir: string, files: string[]): Promise<void> {
	const entries = files
		.map((name) => {
			const path = `${dir}/${name}`;
			return { name, path, size: Deno.statSync(path).size };
		})
		.sort((a, b) => b.size - a.size);

	const wasm = entries.find((e) => e.name.endsWith('.wasm'));
	const wasm_gzipped = wasm ? await gzip_size(wasm.path) : null;

	const name_width = Math.max(...entries.map((e) => e.name.length));
	const size_width = Math.max(...entries.map((e) => format_size(e.size).length));

	console.log(`\nPackage contents (${dir}):`);
	for (const e of entries) {
		const name = e.name.padEnd(name_width);
		const size = format_size(e.size).padStart(size_width);
		const annotation = e === wasm && wasm_gzipped !== null
			? `  →  ${format_size(wasm_gzipped)} gzipped`
			: '';
		console.log(`  ${name}  ${size}${annotation}`);
	}
}
