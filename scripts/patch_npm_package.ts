/**
 * Patches a wasm-pack `--target web` build into the published npm package shape.
 *
 * Creates:
 * - index.js — Node.js/Bun entry: auto-init via readFileSync + initSync (zero config)
 * - browser.js — Browser/default entry: async `init()` with not-initialized guards
 * - worker.js — the `./worker` subpath: browser.js under the name a worker
 *   consumer reaches for (the same module instance, so the same singleton)
 * - index.d.ts / browser.d.ts — type declarations, one per entry (the Node
 *   entry has one export the lazy entries don't: `wasm_module`)
 *
 * Also patches the generated glue itself (`tsv_wasm.js`): appends the
 * `reinstantiate` trap-recovery hook — only the glue can reach its module-level
 * `wasm` binding, which wasm-bindgen's `initSync` short-circuit otherwise makes
 * unrecoverable after a poisoning stack-overflow trap — and guards each class's
 * FinalizationRegistry so a stale handle GC'd after a reinstantiation leaks
 * instead of freeing an old pointer into the fresh instance. Every entry
 * re-exports the hook. See step 1b below.
 *
 * Also patches package.json (name, description, conditional exports, npm
 * metadata) and copies the variant README + repo LICENSE into the package
 * root. The crate has no `README.md` at its root — `README_format.md`,
 * `README_parse.md`, and `README_all.md` are the canonical sources and ship
 * as each package's `README.md`.
 *
 * The exported function list is extracted from the generated `tsv_wasm.js`
 * (every `export function format_*` / `parse_*`), so adding a language to
 * `lang_bindings!` flows through the JS wrappers with no changes here.
 * `parse_internal_*` exports are bench-only and excluded from the wrappers.
 * Its TYPES do not flow through — every parse/format export is
 * `skip_typescript`, so a new language also needs a declaration hand-added to
 * `TS_PARSE_DECLS` / `TS_FORMAT_DECLS`. This script fails the build when one is
 * missing (the declaration check alongside the export validations), because the
 * omission is otherwise silent all the way to a consumer.
 *
 * For the variants with parse exports (`parse`, `all`), also copies
 * `crates/tsv_wasm/types/tsv_ast.d.ts` into the package root alongside the
 * generated `tsv_wasm.d.ts`. The wasm-bindgen
 * `typescript_type = "import('./tsv_ast').*"` extern types resolve against
 * this bundled file at consumer compile time. It also copies the pure-JS
 * `no-locations` reconstruction helper (`npm/locations.js` + `locations.d.ts`)
 * and re-exports its functions from index.js/browser.js and both `.d.ts` — it works on
 * the span-only parse wire, so it ships only where parsing does (not format-only).
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

import { NPM_SHARED_METADATA } from './npm_metadata.ts';
import { format_size, gzip_size } from './size.ts';

const variant = Deno.args[0];
if (variant !== 'format' && variant !== 'parse' && variant !== 'all') {
	console.error(`Usage: patch_npm_package.ts <format|parse|all>`);
	Deno.exit(1);
}

const PKG_NAMES = {
	format: '@fuzdev/tsv_format_wasm',
	parse: '@fuzdev/tsv_parse_wasm',
	all: '@fuzdev/tsv_wasm'
} as const;
const pkg_name = PKG_NAMES[variant];
const has_format_exports = variant !== 'parse';
const has_parse_exports = variant !== 'format';
const pkg_root = `crates/tsv_wasm/pkg/${variant}/npm`;
const main_js = 'tsv_wasm.js';
const dts_file = 'tsv_wasm.d.ts';
/**
 * `dts_file` as an import specifier — what the generated `.d.ts` re-exports
 * from. Spelled with the **`.js`** extension, not extensionless and not
 * `.d.ts`: under `moduleResolution: node16`/`nodenext` a relative specifier in
 * a declaration file must carry the runtime extension (TS2834/TS2835), and TS
 * resolves `./x.js` to `./x.d.ts`. Extensionless works only for consumers on
 * the legacy resolver or with `skipLibCheck`, which is a coin flip we don't
 * need to take. Same rule for the two hand-written siblings below.
 */
const dts_module = `./${dts_file.replace(/\.d\.ts$/, '.js')}`;
const wasm_file = 'tsv_wasm_bg.wasm';
const cli_file = 'cli.js';
/** The `./worker` subpath — a re-export of `browser.js`, which is already the
 * no-auto-init entry a worker wants (`init_sync({module})` then call). Node and
 * Bun resolve the bare specifier to `index.js` via the `node` condition, so the
 * lazy entry is otherwise unreachable there; this gives it a name that says what
 * it is at the point of use, without a second copy of the wrappers. */
const worker_file = 'worker.js';
/** Declarations for the two lazy-init entries (`browser.js`, and `./worker`,
 * which re-exports it). Separate from `index.d.ts` only because the Node entry
 * has one extra export, `wasm_module`. */
const browser_dts = 'browser.d.ts';
// The pure-JS `no-locations` line/column reconstruction helper (`npm/locations.js`
// + hand-written `locations.d.ts`). Rides the parse-capable packages only — it
// operates on the span-only parse wire, so the format-only package has no use for
// it. Needs no WASM init (pure computation over an already-parsed AST).
const locations_file = 'locations.js';
const locations_dts = 'locations.d.ts';
// Extracted from the helper, not listed here: `scripts/build_napi_packages.ts`
// re-exports the same file from `@fuzdev/tsv`, and a hand-kept list in each
// script is how a fourth entry point reaches one package and misses the other.
const locations_exports = [
	...Deno.readTextFileSync(`crates/tsv_wasm/npm/${locations_file}`).matchAll(
		/^export function (\w+)/gm
	)
].map((m) => m[1]);
if (!locations_exports.length) {
	console.error(`FAIL: no exports found in ${locations_file} — did the helper change shape?`);
	Deno.exit(1);
}
// A plain re-export line (no init guard — pure JS), shared by index.js + browser.js.
const locations_reexport = has_parse_exports
	? `export { ${locations_exports.join(', ')} } from './${locations_file}';\n`
	: '';

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
		`FAIL: ${variant} variant has no parse_* exports — was \`--features parse\` passed?`
	);
	Deno.exit(1);
}
if (!has_parse_exports && has_parse) {
	console.error(`FAIL: ${variant} variant contains parse_* exports — stale build dir?`);
	Deno.exit(1);
}

// wasm-bindgen emits exported structs as `export class` (e.g. IgnoreStack,
// the discovery matcher). They re-export through the facade like functions, but
// can't take the per-call init guard — re-exported directly in browser.js.
const classes = [...generated_js.matchAll(/^export class (\w+)/gm)].map((m) => m[1]).sort();

// 1b. Append the `reinstantiate` hook to the generated glue.
//
// A WASM stack overflow (input nested past the shadow stack, ~1,600 levels) is
// the one trap the instance does not survive: `__stack_pointer` is a plain
// mutable global the trap never restores, so every later call on the instance
// throws `memory access out of bounds` — and wasm-bindgen's `initSync`
// short-circuits once initialized, so no public entry can recover. Only this
// module can reach the glue's module-level `wasm` binding, which is why the
// hook is patched in here rather than written beside the facade. It resets that
// binding and re-runs `initSync` with the retained compiled module
// (`wasmModule`, stored by `__wbg_finalize_init` on every init path), so
// recovery is synchronous and pays one instantiation, never a recompile — and
// because every glue export reads `wasm` at call time, existing bindings
// (facade functions, a worker's wrappers) work against the fresh instance with
// no rebind. The anchors below are asserted so a wasm-bindgen upgrade that
// reshapes the glue fails this build loudly instead of shipping a hook that
// silently misses.
const glue_anchors = ['let wasmModule, wasmInstance, wasm;', 'function initSync(module)'];
for (const anchor of glue_anchors) {
	if (!generated_js.includes(anchor)) {
		console.error(
			`FAIL: generated ${main_js} lacks the \`${anchor}\` anchor the reinstantiate hook ` +
				`patches against — did a wasm-bindgen upgrade reshape the glue?`
		);
		Deno.exit(1);
	}
}
if (generated_js.includes('__tsv_instance_generation')) {
	console.error(
		`FAIL: generated ${main_js} is already patched with the reinstantiate hook — ` +
			`re-run the wasm-pack build before patching again`
	);
	Deno.exit(1);
}
// Guard each exported class's FinalizationRegistry: its callback frees through
// the LIVE `wasm` binding, so a stale un-`free()`d handle GC'd after a
// reinstantiation would free an old pointer into the NEW instance — allocator
// corruption with no error at the point of cause. Guarded, a post-reinstantiate
// finalization leaks the old object's bytes instead (the old instance is
// unreachable anyway); explicit `free()` is unaffected and remains the correct
// consumer move before reinstantiating.
const registry_pattern = /new FinalizationRegistry\(ptr => wasm\.(\w+)\(ptr, 1\)\)/g;
let registry_rewrites = 0;
const patched_glue =
	generated_js.replace(registry_pattern, (_m, free_fn) => {
		registry_rewrites++;
		return (
			`new FinalizationRegistry(ptr => {\n` +
			`    // patched by patch_npm_package.ts: after a reinstantiation this pointer\n` +
			`    // belongs to a discarded instance — freeing it into the current one would\n` +
			`    // corrupt its allocator, so leak it instead\n` +
			`    if (__tsv_instance_generation === 0) wasm.${free_fn}(ptr, 1);\n` +
			`})`
		);
	}) +
	`
// ---- appended by patch_npm_package.ts ----

let __tsv_instance_generation = 0;

/**
 * Discard the current WASM instance and synchronously initialize a fresh one
 * from the already-compiled module — the recovery for a poisoning trap (a stack
 * overflow leaves \`__stack_pointer\` where the deep call left it, so every later
 * call throws \`memory access out of bounds\`). Reuses the compiled
 * \`WebAssembly.Module\`, so this never recompiles. Throws if the module was
 * never initialized. Objects backed by the old instance (e.g. \`IgnoreStack\`)
 * are invalidated — \`free()\` them before calling this and rebuild after.
 */
export function reinstantiate() {
    if (wasm === undefined) {
        throw new Error('cannot reinstantiate: the WASM module was never initialized');
    }
    const module = wasmModule;
    wasm = undefined;
    __tsv_instance_generation += 1;
    initSync({ module });
}
`;
if (registry_rewrites !== classes.length) {
	console.error(
		`FAIL: rewrote ${registry_rewrites} FinalizationRegistry callback(s) in ${main_js} but ` +
			`found ${classes.length} exported class(es) — the registry shape drifted from the ` +
			`pattern the reinstantiate guard rewrites`
	);
	Deno.exit(1);
}
Deno.writeTextFileSync(`${pkg_root}/${main_js}`, patched_glue);
console.log(`Patched ${pkg_root}/${main_js}: reinstantiate hook`);

// IgnoreStack is format-gated, so it rides the format and all variants only.
if (has_format_exports) {
	if (!classes.includes('IgnoreStack')) {
		console.error(`FAIL: generated ${main_js} is missing expected export \`IgnoreStack\``);
		Deno.exit(1);
	}
} else if (classes.length) {
	console.error(
		`FAIL: ${variant} variant contains class exports (${classes.join(', ')}) — stale build dir?`
	);
	Deno.exit(1);
}
console.log(`Exports: ${[...fns, ...classes].join(', ')}`);

// Each family's hand-written option types (the `TS_PARSE_DECLS` /
// `TS_FORMAT_DECLS` custom sections in crates/tsv_wasm/src/lib.rs). The `.d.ts`
// re-exports them by NAME, so the tsv_ast/locations star exports can never
// ambiguate them away (the TS2308 rule).
const parse_option_types = has_parse_exports ? ['ParseOptions', 'TypeScriptParseOptions'] : [];
const format_option_types = has_format_exports ? ['FormatOptions', 'TypeScriptFormatOptions'] : [];

// Every name the generated `.d.ts` will re-export must actually be DECLARED in the
// generated `tsv_wasm.d.ts`. Not a formality: every parse/format export is
// `#[wasm_bindgen(skip_typescript)]`, so its declaration comes from a
// hand-written `typescript_custom_section` rather than from wasm-bindgen, and
// the only thing holding the two in sync is a "must update this block too"
// comment. A name that drifts out does NOT fail loudly downstream — without
// `skipLibCheck` it is a TS2614 *inside the shipped package*, and WITH it (the
// common consumer config) the export silently degrades to `any`, so the package
// keeps type-checking while checking nothing. Nothing in-repo type-checks the
// merged `.d.ts` (`check:ast-types` covers `tsv_ast.d.ts` alone), so this is the
// only place the drift can still fail a build. Checked here, with the other
// export validations, so a failure leaves no half-patched package behind.
const generated_dts = Deno.readTextFileSync(`${pkg_root}/${dts_file}`);
const undeclared = [
	...fns.map((name) => ['function', name] as const),
	...classes.map((name) => ['class', name] as const),
	...[...parse_option_types, ...format_option_types].map((name) => ['interface', name] as const)
].filter(([kind, name]) => !new RegExp(`^export ${kind} ${name}\\b`, 'm').test(generated_dts));
if (undeclared.length) {
	console.error(
		`FAIL: ${dts_file} is missing ${undeclared.map(([kind, name]) => `${kind} \`${name}\``).join(', ')} — ` +
			`index.d.ts re-exports the name${undeclared.length === 1 ? '' : 's'} anyway, which ships ` +
			`untyped under a consumer's skipLibCheck. Add the declaration to TS_PARSE_DECLS / ` +
			`TS_FORMAT_DECLS in crates/tsv_wasm/src/lib.rs.`
	);
	Deno.exit(1);
}

// 2. Create index.js — Node.js/Bun entry: auto-init via readFileSync + initSync.
// WASM is initialized synchronously at import time, so no init guard needed.
// The module is compiled here rather than left to `initSync` (which would
// compile the same bytes internally) so the compiled `WebAssembly.Module` can be
// exported: it is the one piece a worker-pool consumer cannot obtain otherwise,
// since the `.wasm` file is not an exports entry. Handing it to a worker that
// initializes via the `./worker` subpath means no worker recompiles — V8 shares
// compiled wasm code across isolates, so tier-up is paid once process-wide.

const index_js = `import { readFileSync } from 'node:fs';
import {
	default as init,
	initSync,
	reinstantiate,
${[...fns, ...classes].map((f) => `\t${f},`).join('\n')}
} from './${main_js}';

/** The compiled WASM module backing this package's exports. Pass it to a worker
 * (\`workerData\`/\`postMessage\`) and initialize there via \`${pkg_name}/worker\`'s
 * \`init_sync({module})\` — no worker recompiles, and compiled code is shared. */
const wasm_module = new WebAssembly.Module(
	readFileSync(new URL('./${wasm_file}', import.meta.url))
);
initSync({ module: wasm_module });

export { init, initSync as init_sync, reinstantiate, wasm_module, ${[...fns, ...classes].join(', ')} };
${locations_reexport}`;

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
// the trap-recovery hook re-exports as-is — it guards itself (throws until initialized)
export { reinstantiate } from './${main_js}';
${
	// classes re-export as-is — consumers must `await init()` before instantiating
	classes.length ? `export { ${classes.join(', ')} } from './${main_js}';\n` : ''
}${
	// the reconstruction helper is pure JS — re-export directly, no init guard
	locations_reexport
}
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

${fns
	.map(
		// Arity-agnostic passthrough: every export takes an optional trailing
		// options object, so the guard must forward every argument, not a fixed
		// parameter list.
		(f) =>
			`export function ${f}(...args) {
	_check();
	return _${f}(...args);
}`
	)
	.join('\n\n')}
`;

Deno.writeTextFileSync(`${pkg_root}/browser.js`, browser_js);
console.log(`Created ${pkg_root}/browser.js`);

// 3b. Create worker.js — the `./worker` subpath, a re-export of browser.js.
// `export *` keeps it the SAME module instance, so `init_sync` here initializes
// the singleton every other import of it sees. A copy of the wrappers would not.

const worker_js = `// The \`${pkg_name}/worker\` entry: the same lazy-init exports as the browser
// entry, under the name a worker reaches for. Initialize once per worker with
// \`init_sync({module})\`, passing the \`wasm_module\` the main thread exports from
// \`${pkg_name}\` — that shares compiled code across isolates instead of
// recompiling per worker. In a browser Web Worker, \`await init()\` works too.
export * from './browser.js';
`;

Deno.writeTextFileSync(`${pkg_root}/${worker_file}`, worker_js);
console.log(`Created ${pkg_root}/${worker_file}`);

// 4. Create the type declarations — one file per entry.
// Re-exports the generated function types, but declares init/init_sync with clean
// signatures to avoid leaking wasm-bindgen internals (InitOutput with raw pointers).

const ast_reexport = has_parse_exports ? `export type * from './tsv_ast.js';\n` : '';
// `export *` (not `export type *`) so the helper's functions AND its types flow through.
const locations_reexport_dts = has_parse_exports
	? `export * from './${locations_dts.replace(/\.d\.ts$/, '.js')}';\n`
	: '';
const options_reexport = (names: Array<string>) =>
	names.length ? `export type { ${names.join(', ')} } from '${dts_module}';\n` : '';
const parse_options_reexport = options_reexport(parse_option_types);
const format_options_reexport = options_reexport(format_option_types);
// Everything all three entries share. `wasm_module` is deliberately NOT here:
// only `index.js` compiles at import and exports it, so declaring it for the
// browser and `./worker` entries would name an export that does not exist —
// under a bundler that is a build error TypeScript said was fine, which is a
// worse failure than a nullable type. Hence one `.d.ts` per entry, and the
// per-condition `types` in `exports` that lets each be reached.
const shared_dts = `${ast_reexport}${locations_reexport_dts}${parse_options_reexport}${format_options_reexport}export {
${[...fns, ...classes].map((f) => `\t${f},`).join('\n')}
} from '${dts_module}';
/** Initialize the WASM module. Required in browsers before calling any other export. No-op if already initialized. */
export declare function init(module_or_path?: {
	module_or_path: RequestInfo | URL | Response | BufferSource | WebAssembly.Module;
}): Promise<void>;
/** Synchronously initialize the WASM module. Works in Node.js and Workers (not Chrome main thread for >4KB WASM). */
export declare function init_sync(module: {
	module: BufferSource | WebAssembly.Module;
}): void;
/**
 * Discard the current WASM instance and synchronously initialize a fresh one
 * from the already-compiled module — the recovery for a poisoning trap: a stack
 * overflow (input nested past the ~1 MiB shadow stack) leaves the instance
 * throwing \`memory access out of bounds\` on every later call, and \`init_sync\`
 * short-circuits once initialized. Never recompiles (the compiled
 * \`WebAssembly.Module\` is retained); synchronous, so the same environment
 * constraints as \`init_sync\` apply. Throws if the module was never
 * initialized. Objects backed by the old instance (e.g. \`IgnoreStack\`) are
 * invalidated — \`free()\` them before calling this and rebuild after.
 */
export declare function reinstantiate(): void;
`;

const index_dts = `${shared_dts}/**
 * The compiled WASM module backing this package's exports, for handing to a
 * worker (\`workerData\` / \`postMessage\`) which then initializes from it via
 * \`${pkg_name}/worker\`'s \`init_sync({module})\` — sharing compiled code across
 * isolates instead of recompiling per worker.
 */
export declare const wasm_module: WebAssembly.Module;
`;

Deno.writeTextFileSync(`${pkg_root}/index.d.ts`, index_dts);
console.log(`Created ${pkg_root}/index.d.ts`);

// The browser and `./worker` entries: the same exports minus `wasm_module`,
// which neither compiles. `browser.js` IS `worker.js` (the subpath re-exports
// it), so one declaration file serves both.
Deno.writeTextFileSync(`${pkg_root}/${browser_dts}`, shared_dts);
console.log(`Created ${pkg_root}/${browser_dts}`);

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
	// Bundle the pure-JS `no-locations` line/column reconstruction helper + its
	// hand-written types (re-exported from index.js/browser.js/index.d.ts above).
	Deno.copyFileSync(`crates/tsv_wasm/npm/${locations_file}`, `${pkg_root}/${locations_file}`);
	console.log(`Copied crates/tsv_wasm/npm/${locations_file} → ${pkg_root}/${locations_file}`);
	Deno.copyFileSync(`crates/tsv_wasm/npm/${locations_dts}`, `${pkg_root}/${locations_dts}`);
	console.log(`Copied crates/tsv_wasm/npm/${locations_dts} → ${pkg_root}/${locations_dts}`);
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
	all: 'formatter and parser for Svelte, TypeScript, and CSS'
}[variant];
pkg.type = 'module';
pkg.exports = {
	'./package.json': './package.json',
	// `types` nests INSIDE each condition rather than sitting beside them: the
	// two entries do not have the same exports (only the Node one compiles at
	// import, so only it has `wasm_module`), and a single hoisted `types` would
	// declare that export for the browser build too.
	'.': {
		node: {
			types: './index.d.ts',
			default: './index.js'
		},
		default: {
			types: `./${browser_dts}`,
			default: './browser.js'
		}
	},
	// the no-auto-init entry, reachable from Node and Bun (where the `node`
	// condition would otherwise always resolve the auto-init one)
	'./worker': {
		types: `./${browser_dts}`,
		default: `./${worker_file}`
	}
};
if (variant === 'all') {
	// No `./` prefix: npm normalizes bin targets to bare relative paths at publish,
	// and its change report words that normalization as the bin being "invalid and
	// removed" (it isn't — the bin survives). Writing the normalized form avoids
	// the scare warning on every publish.
	pkg.bin = { tsv: cli_file };
}
pkg.files = [
	'index.js',
	'index.d.ts',
	'browser.js',
	browser_dts,
	worker_file,
	main_js,
	dts_file,
	wasm_file,
	...(has_parse_exports ? ['tsv_ast.d.ts', locations_file, locations_dts] : []),
	...(variant === 'all' ? [cli_file] : []),
	'README.md',
	'LICENSE'
];
pkg.keywords = [
	'typescript',
	'svelte',
	'css',
	...(has_format_exports ? ['formatter', 'prettier'] : []),
	...(has_parse_exports ? ['parser', 'ast', 'acorn'] : []),
	...(variant === 'all' ? ['cli'] : []),
	'wasm',
	'webassembly'
];
// The registry-identity fields shared with the N-API set (license included —
// wasm-pack derives the same MIT from Cargo.toml, but the module is the
// authority so every published tsv package presents identically).
Object.assign(pkg, NPM_SHARED_METADATA);
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
		const annotation =
			e === wasm && wasm_gzipped !== null ? `  →  ${format_size(wasm_gzipped)} gzipped` : '';
		console.log(`  ${name}  ${size}${annotation}`);
	}
}
