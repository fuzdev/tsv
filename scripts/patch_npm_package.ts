/**
 * Patches a wasm-pack `--target web` build into the published npm package shape.
 *
 * Creates:
 * - index.js — Node.js/Bun entry: auto-init via readFileSync + initSync (zero config)
 * - browser.js — Browser/default entry: async `init()` with not-initialized
 *   guards — one around each engine function the facade calls, and a guarded
 *   SUBCLASS per class export, since a constructor cannot take a per-call guard
 *   and `new` before init would otherwise report the glue's opaque `TypeError`
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
 * The raw exports are flat — `parse_<lang>(source, source_type?)`,
 * `parse_<lang>_json(…)`, `format_<lang>(…)` — and every entry publishes them
 * through the hand-written facade (`crates/tsv_wasm/npm/api.js` + `api_parse.js`,
 * shared with the native `@fuzdev/tsv`), which owns the `(source, options?)` bag,
 * its errors, and the `{locations: true}` sugar. The function list is extracted
 * from the generated `tsv_wasm.js` (every `export function format_*` / `parse_*`),
 * so adding a language to `lang_bindings!` flows through the entries with no
 * changes here. `parse_internal_*` exports are bench-only and excluded. Its TYPES
 * do not flow through — the published declarations are the facade's hand-written
 * `api.d.ts` / `api_parse.d.ts`, so a new language also needs a declaration added
 * there. This script fails the build when one is missing (the declaration check
 * alongside the export validations), because the omission is otherwise silent all
 * the way to a consumer.
 *
 * Every variant gets the facade's shared half (`api.js` + `api.d.ts`: the options
 * reader and the format family). For the variants with parse exports (`parse`,
 * `all`), it also copies the parse half (`api_parse.js` + `api_parse.d.ts`),
 * `crates/tsv_wasm/types/tsv_ast.d.ts` (the AST types `api_parse.d.ts` names), and
 * the pure-JS reconstruction helper (`npm/locations.js` + `locations.d.ts`), whose
 * functions it re-exports from index.js/browser.js and both `.d.ts` — the facade's
 * `{locations: true}` runs it, and it ships only where parsing does (the format-only
 * package loads neither it nor `api_parse.js`).
 *
 * The `all` variant additionally ships the CLI: `crates/tsv_wasm/npm/cli.js`
 * is copied into the package root and wired up as the `tsv` bin.
 *
 * Usage:  patch_npm_package.ts <format|parse|all>
 *
 *   format → crates/tsv_wasm/pkg/format/npm/ → @fuzdev/tsv-format-wasm
 *   parse  → crates/tsv_wasm/pkg/parse/npm/  → @fuzdev/tsv-parse-wasm
 *   all    → crates/tsv_wasm/pkg/all/npm/    → @fuzdev/tsv-wasm
 */

import { FACADE_SOURCE_DIR, facade_files } from './npm_facade.ts';
import { NPM_SHARED_METADATA } from './npm_metadata.ts';
import { format_size, gzip_size } from './size.ts';

const variant = Deno.args[0];
if (variant !== 'format' && variant !== 'parse' && variant !== 'all') {
	console.error(`Usage: patch_npm_package.ts <format|parse|all>`);
	Deno.exit(1);
}

const PKG_NAMES = {
	format: '@fuzdev/tsv-format-wasm',
	parse: '@fuzdev/tsv-parse-wasm',
	all: '@fuzdev/tsv-wasm'
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
// The hand-written facade every entry publishes through (`npm/api.js` +
// `api_parse.js`, with their `.d.ts`), shared with the native `@fuzdev/tsv`. The
// shared half (the options reader + the format family) rides every variant; the
// parse half rides the parse-capable ones, beside the helper it imports.
const api_file = 'api.js';
const api_dts = 'api.d.ts';
const api_parse_file = 'api_parse.js';
const api_parse_dts = 'api_parse.d.ts';
// The pure-JS line/column reconstruction helper (`npm/locations.js` + hand-written
// `locations.d.ts`), which `api_parse.js` runs for `{locations: true}` and every
// parse-capable entry also re-exports. Rides the parse-capable packages only — the
// format-only package has no use for it. Needs no WASM init (pure computation over
// an already-parsed AST).
const locations_file = 'locations.js';
const locations_dts = 'locations.d.ts';
// A whole-module re-export (no init guard — pure JS), shared by index.js + browser.js,
// so a new helper export reaches every entry, and `@fuzdev/tsv`'s, with no list to keep.
const locations_reexport = has_parse_exports ? `export * from './${locations_file}';\n` : '';

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
// the discovery matcher). They re-export through the facade like functions;
// the lazy entries wrap each in a guarded subclass so `new` before init reports
// the same not-initialized error the function exports do (see step 3).
const classes = [...generated_js.matchAll(/^export class (\w+)/gm)].map((m) => m[1]).sort();

// 1b. Append the `reinstantiate` hook to the generated glue.
//
// A WASM stack overflow (input nested past the shadow stack, ~2,500 levels) is
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
// Guard each exported class's lifecycle against a reinstantiation. Both free
// paths — the FinalizationRegistry callback and explicit `free()` — go through
// the LIVE `wasm` binding, so a handle minted against a discarded instance and
// freed after a reinstantiation would free an old pointer into the NEW
// instance: allocator corruption with no error at the point of cause. Each
// handle is therefore stamped with the instance generation it was created
// under (on the object for `free()`, in the registry's held value for the
// callback — the generation must travel WITH the handle, since a module-level
// "has any reinstantiation happened" check is a one-way fuse that would leak
// every handle minted after the first recovery, fresh ones included). A stale
// handle's free is a no-op that leaks the old object's bytes, which the
// unreachable old instance leaks anyway; a current handle frees normally.
const registry_pattern = /new FinalizationRegistry\(ptr => wasm\.(\w+)\(ptr, 1\)\)/g;
const register_pattern = /(\w+)Finalization\.register\((\w+), \2\.__wbg_ptr, \2\)/g;
const free_pattern =
	/free\(\) \{\n(\s+)const ptr = this\.__destroy_into_raw\(\);\n\s+wasm\.(\w+)\(ptr, 0\);\n/g;
// Every exported METHOD too — not only the two free paths. A method hands
// `this.__wbg_ptr` to the live `wasm` binding, so a handle from a discarded
// instance would read the new instance's memory at an offset that meant
// something else entirely: a wrong `is_ignored` verdict, or a fresh trap, with
// nothing said at the point of cause. The receiver is routed through
// `__tsv_live`, one generation compare per call, which throws on a stale handle
// instead. Both call shapes wasm-bindgen emits are covered: the receiver first,
// or after the return-slot pointer (`retptr`) of a method returning a string.
const method_pattern = /(wasm\.\w+\((?:retptr, )?)this\.__wbg_ptr/g;
let registry_rewrites = 0;
let free_rewrites = 0;
let method_rewrites = 0;
/** Register sites per class — the constructor's, plus `__wrap`'s if Rust ever
 * hands one back. The count is read twice below: as a rewrite tally, and as the
 * `__wrap` tripwire the lazy entries' guarded subclass needs. */
const register_sites = new Map<string, number>();
const patched_glue =
	generated_js
		.replace(registry_pattern, (_m, free_fn) => {
			registry_rewrites++;
			return (
				`new FinalizationRegistry(({ ptr, gen }) => {\n` +
				`    // patched by patch_npm_package.ts: a pointer stamped with an older\n` +
				`    // instance generation belongs to a discarded instance — freeing it into\n` +
				`    // the current one would corrupt its allocator, so leak it instead\n` +
				`    if (gen === __tsv_instance_generation) wasm.${free_fn}(ptr, 1);\n` +
				`})`
			);
		})
		.replace(register_pattern, (_m, cls, obj) => {
			register_sites.set(cls, (register_sites.get(cls) ?? 0) + 1);
			return (
				`${cls}Finalization.register(${obj}, ` +
				`{ ptr: ${obj}.__wbg_ptr, gen: (${obj}.__tsv_gen = __tsv_instance_generation) }, ${obj})`
			);
		})
		.replace(free_pattern, (_m, indent, free_fn) => {
			free_rewrites++;
			return (
				`free() {\n` +
				`${indent}const ptr = this.__destroy_into_raw();\n` +
				`${indent}// patched by patch_npm_package.ts: a handle from a discarded instance is\n` +
				`${indent}// already gone — freeing its pointer into the live one would corrupt the\n` +
				`${indent}// allocator, so this is a no-op for it\n` +
				`${indent}if (this.__tsv_gen === __tsv_instance_generation) wasm.${free_fn}(ptr, 0);\n`
			);
		})
		.replace(method_pattern, (_m, call) => {
			method_rewrites++;
			return `${call}__tsv_live(this).__wbg_ptr`;
		}) +
	`
// ---- appended by patch_npm_package.ts ----

let __tsv_instance_generation = 0;

/**
 * The receiver of every exported method, checked against the instance it was
 * minted under: a handle that outlived a \`reinstantiate()\` points into a
 * discarded linear memory, and calling into the live one with it would read
 * unrelated bytes as the object. Throws instead. (\`free()\` on such a handle is
 * a no-op rather than a throw — a finalizer or a cleanup path must not fail.)
 */
function __tsv_live(handle) {
    if (handle.__tsv_gen !== __tsv_instance_generation) {
        throw new Error(
            'this handle belongs to a WASM instance that reinstantiate() discarded — rebuild it'
        );
    }
    return handle;
}

/**
 * Discard the current WASM instance and synchronously initialize a fresh one
 * from the already-compiled module — the recovery for a poisoning trap (a stack
 * overflow leaves \`__stack_pointer\` where the deep call left it, so every later
 * call throws \`memory access out of bounds\`). Reuses the compiled
 * \`WebAssembly.Module\`, so this never recompiles. Throws if the module was
 * never initialized. Objects backed by the old instance (e.g. \`IgnoreStack\`)
 * are invalidated — rebuild them after: every method on a stale one throws, and
 * \`free()\` on it is a safe no-op.
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
// Every method site must be routed, and none may remain: a receiver handed to
// `wasm` any other way is a stale-handle path the guard does not cover.
if (classes.length > 0 && method_rewrites === 0) {
	console.error(
		`FAIL: found ${classes.length} exported class(es) in ${main_js} but no method site handing ` +
			`\`this.__wbg_ptr\` to \`wasm\` — the method shape drifted from the pattern the ` +
			`stale-handle guard rewrites`
	);
	Deno.exit(1);
}
if (/wasm\.\w+\([^)]*\bthis\.__wbg_ptr/.test(patched_glue)) {
	console.error(
		`FAIL: a method in ${main_js} still hands \`this.__wbg_ptr\` to \`wasm\` unguarded — a ` +
			`call shape the stale-handle guard does not recognize`
	);
	Deno.exit(1);
}
// The complement, which neither check above can see because it names no `wasm` call:
// every raw receiver left in the glue must be a shape that never hands a pointer to
// the live instance — `__destroy_into_raw`'s read-and-zero, the constructor's
// assignment, its (rewritten) registration — and only `free()` may call
// `__destroy_into_raw()`. The read is counted rather than exempted outright: the same
// `const ptr = this.__wbg_ptr;` line in a method is exactly the aliasing to catch, as
// is a `self`-consuming method (whose glue destroys the handle and passes the pointer
// on) — stale-handle paths the rewrite would otherwise miss in silence.
const raw_receivers = patched_glue
	.split('\n')
	.filter((line) => /\bthis\.__wbg_ptr\b/.test(line))
	.filter(
		(line) =>
			!/^\s*const ptr = this\.__wbg_ptr;$/.test(line) &&
			!/^\s*this\.__wbg_ptr = (?:0|ret(?: >>> 0)?);$/.test(line) &&
			!/Finalization\.register\(this, \{ ptr: this\.__wbg_ptr, gen: /.test(line)
	);
const ptr_reads = patched_glue.match(/^\s*const ptr = this\.__wbg_ptr;$/gm)?.length ?? 0;
const destroy_calls = patched_glue.match(/\bthis\.__destroy_into_raw\(\)/g)?.length ?? 0;
if (raw_receivers.length > 0 || ptr_reads !== classes.length || destroy_calls !== classes.length) {
	console.error(
		`FAIL: ${main_js} reads a receiver the stale-handle guard does not cover — ` +
			`${raw_receivers.length} unrecognized \`this.__wbg_ptr\` line(s)` +
			raw_receivers.map((line) => `\n  ${line.trim()}`).join('') +
			`\n${ptr_reads} \`const ptr = this.__wbg_ptr;\` read(s) and ${destroy_calls} ` +
			`\`this.__destroy_into_raw()\` call(s) for ${classes.length} class(es), where each ` +
			`class's \`__destroy_into_raw\` makes exactly one read and its \`free()\` one call`
	);
	Deno.exit(1);
}
if (registry_rewrites !== classes.length || free_rewrites !== classes.length) {
	console.error(
		`FAIL: rewrote ${registry_rewrites} FinalizationRegistry callback(s) and ${free_rewrites} ` +
			`free() method(s) in ${main_js} but found ${classes.length} exported class(es) — the ` +
			`class shape drifted from the pattern the reinstantiate guard rewrites`
	);
	Deno.exit(1);
}
// A class registers at least once (its constructor); a class that is also
// returned from Rust registers again in its `__wrap`.
const register_rewrites = [...register_sites.values()].reduce((sum, n) => sum + n, 0);
if (register_rewrites < classes.length) {
	console.error(
		`FAIL: rewrote ${register_rewrites} FinalizationRegistry.register site(s) in ${main_js} but ` +
			`found ${classes.length} exported class(es) — the register shape drifted from the ` +
			`pattern the reinstantiate guard stamps`
	);
	Deno.exit(1);
}
// The `__wrap` tripwire for the lazy entries' guarded subclass (step 3). A class
// RETURNED from Rust is wrapped by the glue's own `X.__wrap`, which builds the
// BASE prototype — so such an instance would fail `instanceof` against the name
// `browser.js` and `./worker` export, silently, in browsers only. No class is
// returned today; this fails the build the day one is, rather than letting the
// subclass quietly become wrong.
const wrapped_classes = classes.filter((name) => (register_sites.get(name) ?? 0) > 1);
if (wrapped_classes.length) {
	console.error(
		`FAIL: class(es) ${wrapped_classes.join(', ')} are RETURNED from Rust (a second ` +
			`FinalizationRegistry.register site, in \`__wrap\`). The lazy entries wrap each class in a ` +
			`guarded subclass, and \`__wrap\` builds the base prototype — so an instance handed back ` +
			`from Rust would not be \`instanceof\` the class browser.js/worker.js exports. Decide how ` +
			`those entries should present this class before shipping it.`
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

// The facade's families, keyed by language — read off the raw export names, so a new
// `lang_bindings!` language reaches the entries with no edit here.
const languages_of = (pattern: RegExp): Array<string> =>
	fns.map((name) => pattern.exec(name)?.[1]).filter((l): l is string => l !== undefined);
const format_languages = languages_of(/^format_(\w+)$/);
const parse_languages = languages_of(/^parse_(\w+)_json$/);
for (const language of parse_languages) {
	if (!fns.includes(`parse_${language}`)) {
		console.error(
			`FAIL: generated ${main_js} has \`parse_${language}_json\` but no \`parse_${language}\``
		);
		Deno.exit(1);
	}
}
/** The names the facade publishes per family — the raw names, which it keeps. */
const format_fns = fns.filter((name) => name.startsWith('format_'));
const parse_fns = fns.filter((name) => name.startsWith('parse_'));

// Each family's hand-written option types, declared beside the facade (`api.d.ts` /
// `api_parse.d.ts`). The `.d.ts` re-exports them by NAME, so the tsv_ast/locations
// star exports can never ambiguate them away (the TS2308 rule).
const parse_option_types = has_parse_exports
	? ['ParseOptions', 'TypeScriptParseOptions', 'ParseJsonOptions', 'TypeScriptParseJsonOptions']
	: [];
const format_option_types = has_format_exports ? ['FormatOptions', 'TypeScriptFormatOptions'] : [];

// Every name the entries' `.d.ts` will re-export must actually be DECLARED where it
// is re-exported from: the facade's functions and option types in its hand-written
// `api.d.ts` / `api_parse.d.ts`, the classes in the generated `tsv_wasm.d.ts`. Not a
// formality: nothing ties a hand-written declaration to the export it types, and a
// name that drifts out does NOT fail loudly downstream — without `skipLibCheck` it is
// a TS2614 *inside the shipped package*, and WITH it (the common consumer config)
// the export silently degrades to `any`, so the package keeps type-checking while
// checking nothing. Nothing in-repo type-checks the merged `.d.ts`
// (`check:ast-types` covers `tsv_ast.d.ts` alone), so this is the only place the
// drift can still fail a build. Checked here, with the other export validations,
// so a failure leaves no half-patched package behind.
const generated_dts = Deno.readTextFileSync(`${pkg_root}/${dts_file}`);
const api_dts_source = Deno.readTextFileSync(`crates/tsv_wasm/npm/${api_dts}`);
const api_parse_dts_source = Deno.readTextFileSync(`crates/tsv_wasm/npm/${api_parse_dts}`);
const undeclared = [
	...format_fns.map((name) => [api_dts, api_dts_source, 'declare function', name] as const),
	...parse_fns.map(
		(name) => [api_parse_dts, api_parse_dts_source, 'declare function', name] as const
	),
	...format_option_types.map((name) => [api_dts, api_dts_source, 'interface', name] as const),
	...parse_option_types.map(
		(name) => [api_parse_dts, api_parse_dts_source, 'interface', name] as const
	),
	...classes.map((name) => [dts_file, generated_dts, 'class', name] as const)
].filter(([, source, kind, name]) => !new RegExp(`^export ${kind} ${name}\\b`, 'm').test(source));
if (undeclared.length) {
	console.error(
		`FAIL: missing declarations — ${undeclared.map(([file, , kind, name]) => `${kind} \`${name}\` in ${file}`).join(', ')}. ` +
			`index.d.ts re-exports the name${undeclared.length === 1 ? '' : 's'} anyway, which ships ` +
			`untyped under a consumer's skipLibCheck. Declare facade functions and option types in ` +
			`crates/tsv_wasm/npm/${api_dts} / ${api_parse_dts}; a class comes from the generated ${dts_file}.`
	);
	Deno.exit(1);
}

// The facade wiring both entries share: the engine families handed to `create_*_api`
// (each raw export under its alias `_<name>`, optionally wrapped), then the published
// names destructured out of the result.
const engine_table = (languages: Array<string>, raw: (language: string) => string): string =>
	`{ ${languages.map((language) => `${language}: ${raw(language)}`).join(', ')} }`;
// each factory is imported only where an export uses it — a parse-only entry has no
// format functions, though its `api_parse.js` still imports `api.js`'s shared helpers
const facade_imports =
	(format_fns.length ? `import { create_format_api } from './${api_file}';\n` : '') +
	(has_parse_exports ? `import { create_parse_api } from './${api_parse_file}';\n` : '');
const facade_exports = (wrap: (raw: string) => string): string =>
	(format_fns.length
		? `export const { ${format_fns.join(', ')} } = create_format_api(\n` +
			`\t${engine_table(format_languages, (l) => wrap(`_format_${l}`))}\n);\n`
		: '') +
	(parse_fns.length
		? `export const { ${parse_fns.join(', ')} } = create_parse_api({\n` +
			`\tparse: ${engine_table(parse_languages, (l) => wrap(`_parse_${l}`))},\n` +
			`\tparse_json: ${engine_table(parse_languages, (l) => wrap(`_parse_${l}_json`))}\n});\n`
		: '');

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
${fns.map((f) => `\t${f} as _${f},`).join('\n')}
${classes.map((c) => `\t${c},`).join('\n')}
} from './${main_js}';
${facade_imports}
/** The compiled WASM module backing this package's exports. Pass it to a worker
 * (\`workerData\`/\`postMessage\`) and initialize there via \`${pkg_name}/worker\`'s
 * \`init_sync({module})\` — no worker recompiles, and compiled code is shared. */
const wasm_module = new WebAssembly.Module(
	readFileSync(new URL('./${wasm_file}', import.meta.url))
);
initSync({ module: wasm_module });

// the published functions: the shared facade over the raw flat exports
${facade_exports((raw) => raw)}
export { init, initSync as init_sync, reinstantiate, wasm_module${classes.map((c) => `, ${c}`).join('')} };
${locations_reexport}`;

Deno.writeTextFileSync(`${pkg_root}/index.js`, index_js);
console.log(`Created ${pkg_root}/index.js`);

// 3. Create browser.js — Browser/default entry: async init() with guards.
// Vite and other bundlers pick this via the "default" export condition and handle
// the `new URL('./tsv_wasm_bg.wasm', import.meta.url)` pattern natively.

const browser_js = `import {
	default as _init,
	initSync,
${[...fns, ...classes].map((f) => `\t${f} as _${f},`).join('\n')}
} from './${main_js}';
${facade_imports}// the trap-recovery hook re-exports as-is — it guards itself (throws until initialized)
export { reinstantiate } from './${main_js}';
${
	// the reconstruction helper is pure JS — re-export directly, no init guard
	locations_reexport
}
let _ready = false;

function _check() {
	if (!_ready) throw new Error('${pkg_name}: WASM not initialized. Call \\\`await init()\\\` first.');
}

/** One engine function behind the not-initialized guard. The facade validates the
 * options bag before it calls in here, so a bad bag reports itself even before
 * \`init()\`; a well-formed call before it gets this guard's message. */
const _guarded =
	(f) =>
	(...args) => {
		_check();
		return f(...args);
	};

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

${classes
	.map(
		// A guarded SUBCLASS, not a re-export: constructing before init reaches the
		// glue's `wasm.<ctor>()` with `wasm` still undefined, which throws an opaque
		// `TypeError: Cannot read properties of undefined` — the one export family in
		// this entry that answered the not-initialized case differently from every
		// other. Subclassing keeps everything else the base class's: `instanceof`, the
		// prototype methods, `free()`, and the FinalizationRegistry stamping its
		// constructor does. The guard runs before `super()`, which is legal precisely
		// because it does not touch `this`.
		(c) =>
			`export class ${c} extends _${c} {
	constructor(...args) {
		_check();
		super(...args);
	}
}`
	)
	.join(
		'\n\n'
	)}${classes.length ? '\n\n' : ''}// the published functions: the shared facade over the guarded raw exports
${facade_exports((raw) => `_guarded(${raw})`)}`;

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
// The facade's functions and option types re-export by name from its hand-written
// declarations, the classes from the generated ones; init/init_sync are declared with
// clean signatures to avoid leaking wasm-bindgen internals (InitOutput with raw
// pointers).

const ast_reexport = has_parse_exports ? `export type * from './tsv_ast.js';\n` : '';
// `export *` (not `export type *`) so the helper's functions AND its types flow through.
const locations_reexport_dts = has_parse_exports
	? `export * from './${locations_dts.replace(/\.d\.ts$/, '.js')}';\n`
	: '';
const named_reexport = (names: Array<string>, from: string, type_only: boolean): string =>
	names.length
		? `export ${type_only ? 'type ' : ''}{ ${names.join(', ')} } from './${from}';\n`
		: '';
// Everything all three entries share. `wasm_module` is deliberately NOT here:
// only `index.js` compiles at import and exports it, so declaring it for the
// browser and `./worker` entries would name an export that does not exist —
// under a bundler that is a build error TypeScript said was fine, which is a
// worse failure than a nullable type. Hence one `.d.ts` per entry, and the
// per-condition `types` in `exports` that lets each be reached.
const shared_dts = `${ast_reexport}${locations_reexport_dts}${named_reexport(parse_option_types, api_parse_file, true)}${named_reexport(format_option_types, api_file, true)}${named_reexport(parse_fns, api_parse_file, false)}${named_reexport(format_fns, api_file, false)}${
	classes.length ? `export { ${classes.join(', ')} } from '${dts_module}';\n` : ''
}/** Initialize the WASM module. Required in browsers before calling any other export. No-op if already initialized. */
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
 * initialized.${
		classes.length
			? ` Objects backed by the old instance (e.g. \`${classes[0]}\`) are
 * invalidated — rebuild them after: every method on a stale one throws, and
 * \`free()\` on it is a safe no-op.`
			: ''
	}
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

// The facade the entries import — its parse half (with the reconstruction helper
// index.js/browser.js/index.d.ts re-export) in the parse-capable variants only.
for (const file of facade_files(has_parse_exports)) {
	Deno.copyFileSync(`${FACADE_SOURCE_DIR}/${file}`, `${pkg_root}/${file}`);
	console.log(`Copied ${FACADE_SOURCE_DIR}/${file} → ${pkg_root}/${file}`);
}

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
	...facade_files(has_parse_exports),
	...(has_parse_exports ? ['tsv_ast.d.ts'] : []),
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

// wasm-pack's web-target fields point at the raw glue; `exports` supersedes them,
// so replace them with the Node facade for pre-`exports` resolvers (an old
// `moduleResolution: node10` consumer, tooling that reads `main` directly) — the
// same pair the N-API loader declares, so both package sets resolve alike.
pkg.main = 'index.js';
pkg.types = 'index.d.ts';
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
