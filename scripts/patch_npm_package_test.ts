/**
 * `run_patch_npm_package` over a small synthetic wasm-pack output in a temp staging, reading
 * the checkout's own sources: the files it writes and copies, what it prints, and the exit
 * code and untouched staging of each refusal it is tested for. Then over a synthetic checkout
 * too, so every file it generates and the whole summary are compared to their text.
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { test } from 'vitest';

import type { GzipProgram } from './gzip_size.ts';
import { type PatchNpmPackageDeps, run_patch_npm_package } from './patch_npm_package.ts';

const CHECKOUT = join(dirname(fileURLToPath(import.meta.url)), '..');

/** The glue wasm-pack's `web` target emits, cut to the shapes the patch reads: one class with
 * a method, the format family, the registry, the anchors, and the async init's load. */
const GLUE = `export class IgnoreStack {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        IgnoreStackFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_ignorestack_free(ptr, 0);
    }
    is_ignored(path) {
        const ret = wasm.ignorestack_is_ignored(this.__wbg_ptr, path);
        return ret !== 0;
    }
    constructor() {
        const ret = wasm.ignorestack_new();
        this.__wbg_ptr = ret;
        IgnoreStackFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
export function format_css(source, source_type) {}
export function format_svelte(source, source_type) {}
export function format_typescript(source, source_type) {}
export function parse_internal_bench(source) {}
const IgnoreStackFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_ignorestack_free(ptr, 1));
let wasmModule, wasmInstance, wasm;
function initSync(module) {}
async function __wbg_init(module_or_path) {
    const { instance, module } = await __wbg_load(await module_or_path, imports);
    return wasm;
}
export default __wbg_init;
`;

const DTS = `export class IgnoreStack {
    free(): void;
    [Symbol.dispose](): void;
    is_ignored(path: string): boolean;
}
export function format_css(source: string, source_type?: string): string;
`;

const PACKAGE_JSON = `{
  "name": "tsv_wasm",
  "type": "module",
  "version": "0.8.1",
  "files": ["tsv_wasm_bg.wasm", "tsv_wasm.js", "tsv_wasm.d.ts"],
  "module": "tsv_wasm.js",
  "types": "tsv_wasm.d.ts",
  "sideEffects": ["./snippets/*"]
}
`;

/** A staging as wasm-pack leaves it, at `<root>/<variant>/npm`, with `glue` as its JS. */
const make_staging = async (
	variant: string,
	glue = GLUE
): Promise<{ root: FsDir & AsyncDisposable; pkg: FsDir }> => {
	const root = await fs.temp_dir('tsv_patch_npm_package_test_');
	await root.mkdir(`${variant}/npm`, { recursive: true });
	const pkg = root.dir(`${variant}/npm`);
	await pkg.write('tsv_wasm.js', glue);
	await pkg.write('tsv_wasm.d.ts', DTS);
	await pkg.write('package.json', PACKAGE_JSON);
	await pkg.write('tsv_wasm_bg.wasm', new Uint8Array(4096));
	return { root, pkg };
};

/** The deps over `root`'s stagings and the checkout, with what the patch printed and the
 * paths `gzip` was given. */
const make_deps = async (
	root: FsDir,
	gzip_present = true
): Promise<{
	deps: PatchNpmPackageDeps;
	out: Array<string>;
	err: Array<string>;
	gzipped: Array<ReadonlyArray<string>>;
	opened: Array<string>;
}> => {
	const out: Array<string> = [];
	const err: Array<string> = [];
	const gzipped: Array<ReadonlyArray<string>> = [];
	const opened: Array<string> = [];
	const gzip: GzipProgram = {
		out_bytes: async (args) => {
			gzipped.push(args);
			return { stdout: new Uint8Array(1500) };
		}
	};
	return {
		deps: {
			checkout: (await fs.open_dir(CHECKOUT)).readonly(),
			open_package: (variant) => {
				opened.push(variant);
				return root.dir(`${variant}/npm`);
			},
			gzip: gzip_present ? gzip : null,
			print: (line) => out.push(line),
			print_error: (line) => err.push(line)
		},
		out,
		err,
		gzipped,
		opened
	};
};

const STAGED_FORMAT = [
	'README.md',
	'LICENSE',
	'NOTICE',
	'api.js',
	'facade_error.d.ts',
	'facade_format.d.ts'
];
const SOURCES: Record<string, string> = {
	'README.md': 'crates/tsv_wasm/README_format.md',
	LICENSE: 'LICENSE',
	NOTICE: 'crates/tsv_html/NOTICE',
	'api.js': 'crates/tsv_wasm/npm/api.js',
	'facade_error.d.ts': 'crates/tsv_wasm/npm/facade_error.d.ts',
	'facade_format.d.ts': 'crates/tsv_wasm/npm/facade_format.d.ts'
};

test('format: writes the entries, copies the sources, patches the manifest, prints each step', async () => {
	await using root = (await make_staging('format')).root;
	const pkg = root.dir('format/npm');
	const { deps, out, err, gzipped } = await make_deps(root);
	strictEqual(await run_patch_npm_package(deps, ['format']), 0);
	deepStrictEqual(err, []);
	const at = 'crates/tsv_wasm/pkg/format/npm';
	deepStrictEqual(out.slice(0, 16), [
		`Patched ${at}/tsv_wasm.js: reinstantiate hook`,
		'Exports: format_css, format_svelte, format_typescript, IgnoreStack',
		`Created ${at}/index.js`,
		`Created ${at}/browser.js`,
		`Created ${at}/worker.js`,
		`Patched ${at}/tsv_wasm.d.ts: esnext.disposable lib reference`,
		`Created ${at}/index.d.ts`,
		`Created ${at}/browser.d.ts`,
		`Copied crates/tsv_wasm/README_format.md → ${at}/README.md`,
		`Copied LICENSE → ${at}/LICENSE`,
		`Copied crates/tsv_html/NOTICE → ${at}/NOTICE`,
		`Copied crates/tsv_wasm/npm/api.js → ${at}/api.js`,
		`Copied crates/tsv_wasm/npm/facade_error.d.ts → ${at}/facade_error.d.ts`,
		`Copied crates/tsv_wasm/npm/facade_format.d.ts → ${at}/facade_format.d.ts`,
		`Patched ${at}/package.json: name → @fuzdev/tsv-format-wasm, version 0.8.1`,
		`\nPackage contents (${at}):`
	]);
	ok(out.includes('  tsv_wasm_bg.wasm     4.1 KB  →  1.5 KB gzipped'), out.join('\n'));
	deepStrictEqual(gzipped, [['-c', `${pkg.path}/tsv_wasm_bg.wasm`]]);

	const glue = await pkg.read_text('tsv_wasm.js');
	ok(glue.includes('export function reinstantiate()'));
	ok(glue.includes('wasm.ignorestack_is_ignored(__tsv_live(this).__wbg_ptr, path)'));
	ok(glue.includes('if (wasm !== undefined) return wasm;'));
	ok(
		(await pkg.read_text('tsv_wasm.d.ts')).startsWith('/// <reference lib="esnext.disposable" />\n')
	);
	ok((await pkg.read_text('index.js')).includes('initSync({ module: wasm_module });'));
	ok((await pkg.read_text('browser.js')).includes('export class IgnoreStack extends _IgnoreStack'));
	strictEqual(
		await pkg.read_text('worker.js').then((t) => t.includes("export * from './browser.js';")),
		true
	);
	ok((await pkg.read_text('index.d.ts')).includes('export declare const wasm_module'));
	ok(!(await pkg.read_text('browser.d.ts')).includes('wasm_module'));

	const checkout = (await fs.open_dir(CHECKOUT)).readonly();
	for (const name of STAGED_FORMAT) {
		deepStrictEqual(await pkg.read(name), await checkout.read(SOURCES[name]!), name);
		strictEqual((await pkg.stat(name))!.mode, (await checkout.stat(SOURCES[name]!))!.mode, name);
	}
	const manifest = JSON.parse(await pkg.read_text('package.json'));
	strictEqual(manifest.name, '@fuzdev/tsv-format-wasm');
	strictEqual(manifest.module, undefined);
	deepStrictEqual(manifest.sideEffects, ['./index.js']);
	deepStrictEqual(Object.keys(manifest.exports), ['./package.json', '.', './worker']);
	deepStrictEqual(manifest.files, [
		'index.js',
		'index.d.ts',
		'browser.js',
		'browser.d.ts',
		'worker.js',
		'tsv_wasm.js',
		'tsv_wasm.d.ts',
		'tsv_wasm_bg.wasm',
		'api.js',
		'facade_error.d.ts',
		'facade_format.d.ts',
		'README.md',
		'LICENSE',
		'NOTICE'
	]);
	strictEqual((await pkg.read_text('package.json')).endsWith('}\n'), true);
});

/**
 * A checkout holding what the format variant reads: the facade's declaration files, each family
 * cut to what the patch reads off it, and the files it copies, each holding its own name.
 */
const SYNTHETIC_CHECKOUT: Record<string, string> = {
	'crates/tsv_wasm/npm/facade_format.d.ts':
		'export interface FormatOptions {}\nexport type FormatResult = string;\n' +
		'export declare function format_css(source: string): FormatResult;\n' +
		'export declare function format_svelte(source: string): FormatResult;\n' +
		'export declare function format_typescript(source: string): FormatResult;\n',
	'crates/tsv_wasm/npm/facade_error.d.ts':
		'export interface TsvError {}\nexport interface TsvParseError extends TsvError {}\n',
	'crates/tsv_wasm/npm/facade_parse.d.ts': 'export interface ParseOptions {}\n',
	'crates/tsv_wasm/npm/facade_strip.d.ts': 'export interface StripOptions {}\n',
	'crates/tsv_wasm/npm/facade_imports.d.ts': 'export interface ImportsOptions {}\n',
	'crates/tsv_wasm/npm/api.js': 'api.js\n',
	'crates/tsv_wasm/README_format.md': 'README_format.md\n',
	LICENSE: 'LICENSE\n',
	'crates/tsv_html/NOTICE': 'NOTICE\n'
};

/** What the format variant writes for `GLUE` over `SYNTHETIC_CHECKOUT`, whole: the patched glue
 * and declarations, and every entry it generates. */
const GENERATED_FORMAT: Record<string, string> = {
	'tsv_wasm.js': `export class IgnoreStack {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        IgnoreStackFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        // patched by patch_npm_package.ts: a handle from a discarded instance is
        // already gone — freeing its pointer into the live one would corrupt the
        // allocator, so this is a no-op for it
        if (this.__tsv_gen === __tsv_instance_generation) wasm.__wbg_ignorestack_free(ptr, 0);
    }
    is_ignored(path) {
        const ret = wasm.ignorestack_is_ignored(__tsv_live(this).__wbg_ptr, path);
        return ret !== 0;
    }
    constructor() {
        const ret = wasm.ignorestack_new();
        this.__wbg_ptr = ret;
        IgnoreStackFinalization.register(this, { ptr: this.__wbg_ptr, gen: (this.__tsv_gen = __tsv_instance_generation) }, this);
        return this;
    }
}
export function format_css(source, source_type) {}
export function format_svelte(source, source_type) {}
export function format_typescript(source, source_type) {}
export function parse_internal_bench(source) {}
const IgnoreStackFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(({ ptr, gen }) => {
    // patched by patch_npm_package.ts: a pointer stamped with an older
    // instance generation belongs to a discarded instance — freeing it into
    // the current one would corrupt its allocator, so leak it instead
    if (gen === __tsv_instance_generation) wasm.__wbg_ignorestack_free(ptr, 1);
});
let wasmModule, wasmInstance, wasm;
function initSync(module) {}
async function __wbg_init(module_or_path) {
    const { instance, module } = await __wbg_load(await module_or_path, imports);
    // patched by patch_npm_package.ts: an overlapping init (a second init, or
    // an initSync during the await) may have initialized the module meanwhile —
    // keep that instance, since replacing it would orphan every handle minted
    // against it
    if (wasm !== undefined) return wasm;
    return wasm;
}
export default __wbg_init;

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
`,
	'tsv_wasm.d.ts': `/// <reference lib="esnext.disposable" />
export class IgnoreStack {
    free(): void;
    [Symbol.dispose](): void;
    is_ignored(path: string): boolean;
}
export function format_css(source: string, source_type?: string): string;
`,
	'index.js': `import { readFileSync } from 'node:fs';
import {
	default as _init,
	initSync,
	reinstantiate,
	format_css as _format_css,
	format_svelte as _format_svelte,
	format_typescript as _format_typescript,
	IgnoreStack,
} from './tsv_wasm.js';
import { create_format_api } from './api.js';

/** The compiled WASM module backing this package's exports. Pass it to a worker
 * (\`workerData\`/\`postMessage\`) and initialize there via \`@fuzdev/tsv-format-wasm/worker\`'s
 * \`init_sync({module})\` — no worker recompiles, and compiled code is shared. */
const wasm_module = new WebAssembly.Module(
	readFileSync(new URL('./tsv_wasm_bg.wasm', import.meta.url))
);
initSync({ module: wasm_module });

// The lifecycle pair the lazy entries export too — already initialized above, so both
// are no-ops here. Declared rather than re-exported under an alias, so each carries its
// export name (the glue's own are \`__wbg_init\` and \`initSync\`).
/** Initialize the WASM module — done at import on this entry, so a no-op. */
export async function init(...args) {
	await _init(...args);
}
/** Synchronously initialize the WASM module — done at import on this entry, so a no-op. */
export function init_sync(...args) {
	initSync(...args);
}

// the published functions: the shared facade over the raw flat exports
export const { format_css, format_svelte, format_typescript } = create_format_api({
	css: _format_css,
	svelte: _format_svelte,
	typescript: _format_typescript
});

export { reinstantiate, wasm_module, IgnoreStack };
`,
	'browser.js': `import {
	default as _init,
	initSync,
	format_css as _format_css,
	format_svelte as _format_svelte,
	format_typescript as _format_typescript,
	IgnoreStack as _IgnoreStack,
} from './tsv_wasm.js';
import { create_format_api } from './api.js';
// the trap-recovery hook re-exports as-is — it guards itself (throws until initialized)
export { reinstantiate } from './tsv_wasm.js';

let _ready = false;
/** The in-flight async init, shared by every overlapping \`init()\` so they compile and
 * instantiate once; cleared on failure so a later call can retry. */
let _init_p;

function _check() {
	if (!_ready) throw new Error('@fuzdev/tsv-format-wasm: WASM not initialized. Call \\\`await init()\\\` first.');
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

/**
 * Initialize the WASM module. Required in browsers before any engine call; the option
 * and source checks need none. No-op if already initialized; overlapping calls share one
 * initialization.
 */
export async function init(...args) {
	if (_ready) return;
	await (_init_p ??= _init(...args).then(
		() => {
			_ready = true;
		},
		(error) => {
			_init_p = undefined;
			// an init_sync() that landed while this one was pending already initialized
			if (_ready) return;
			throw error;
		}
	));
}

/** Synchronously initialize the WASM module. Works in Workers (not Chrome main thread for >4KB WASM). */
export function init_sync(...args) {
	if (_ready) return;
	initSync(...args);
	_ready = true;
}

export class IgnoreStack extends _IgnoreStack {
	constructor(...args) {
		_check();
		super(...args);
	}
}

// the published functions: the shared facade over the guarded raw exports
export const { format_css, format_svelte, format_typescript } = create_format_api({
	css: _guarded(_format_css),
	svelte: _guarded(_format_svelte),
	typescript: _guarded(_format_typescript)
});
`,
	'worker.js': `// The \`@fuzdev/tsv-format-wasm/worker\` entry: the same lazy-init exports as the browser
// entry, under the name a worker reaches for. Initialize once per worker with
// \`init_sync({module})\`, passing the \`wasm_module\` the main thread exports from
// \`@fuzdev/tsv-format-wasm\` — that shares compiled code across isolates instead of
// recompiling per worker. In a browser Web Worker, \`await init()\` works too.
export * from './browser.js';
`,
	'index.d.ts': `export type { FormatOptions, FormatResult } from './facade_format.js';
export type { TsvError, TsvParseError } from './facade_error.js';
export { format_css, format_svelte, format_typescript } from './facade_format.js';
export { IgnoreStack } from './tsv_wasm.js';
/**
 * Initialize the WASM module. Required in browsers before any engine call; the option
 * and source checks need none. No-op if already initialized; overlapping calls share one
 * initialization.
 */
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
 * invalidated — rebuild them after: every method on a stale one throws, and
 * \`free()\` on it is a safe no-op.
 */
export declare function reinstantiate(): void;
/**
 * The compiled WASM module backing this package's exports, for handing to a
 * worker (\`workerData\` / \`postMessage\`) which then initializes from it via
 * \`@fuzdev/tsv-format-wasm/worker\`'s \`init_sync({module})\` — sharing compiled code across
 * isolates instead of recompiling per worker.
 */
export declare const wasm_module: WebAssembly.Module;
`,
	'browser.d.ts': `export type { FormatOptions, FormatResult } from './facade_format.js';
export type { TsvError, TsvParseError } from './facade_error.js';
export { format_css, format_svelte, format_typescript } from './facade_format.js';
export { IgnoreStack } from './tsv_wasm.js';
/**
 * Initialize the WASM module. Required in browsers before any engine call; the option
 * and source checks need none. No-op if already initialized; overlapping calls share one
 * initialization.
 */
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
 * invalidated — rebuild them after: every method on a stale one throws, and
 * \`free()\` on it is a safe no-op.
 */
export declare function reinstantiate(): void;
`
};

/** The manifest the format variant writes, as parsed; its text is this, tab-indented. */
const MANIFEST_FORMAT = {
	name: '@fuzdev/tsv-format-wasm',
	type: 'module',
	version: '0.8.1',
	files: [
		'index.js',
		'index.d.ts',
		'browser.js',
		'browser.d.ts',
		'worker.js',
		'tsv_wasm.js',
		'tsv_wasm.d.ts',
		'tsv_wasm_bg.wasm',
		'api.js',
		'facade_error.d.ts',
		'facade_format.d.ts',
		'README.md',
		'LICENSE',
		'NOTICE'
	],
	types: 'index.d.ts',
	sideEffects: ['./index.js'],
	description: 'formatter for Svelte, TypeScript, and CSS',
	exports: {
		'./package.json': './package.json',
		'.': {
			node: {
				types: './index.d.ts',
				default: './index.js'
			},
			default: {
				types: './browser.d.ts',
				default: './browser.js'
			}
		},
		'./worker': {
			types: './browser.d.ts',
			default: './worker.js'
		}
	},
	keywords: ['typescript', 'svelte', 'css', 'formatter', 'prettier', 'wasm', 'webassembly'],
	license: '0BSD AND BSD-3-Clause',
	homepage: 'https://github.com/fuzdev/tsv',
	author: {
		name: 'Ryan Atkinson',
		email: 'mail@ryanatkn.com',
		url: 'https://www.ryanatkn.com/'
	},
	repository: {
		type: 'git',
		url: 'git+https://github.com/fuzdev/tsv.git'
	},
	bugs: 'https://github.com/fuzdev/tsv/issues',
	funding: 'https://www.ryanatkn.com/funding',
	engines: {
		node: '>=22'
	},
	publishConfig: {
		access: 'public'
	},
	main: 'index.js'
};

test('format: the generated files and the summary, whole', async () => {
	await using root = (await make_staging('format')).root;
	await using checkout = await fs.temp_dir('tsv_patch_npm_package_test_checkout_');
	for (const [path, text] of Object.entries(SYNTHETIC_CHECKOUT)) {
		const dir = path.split('/').slice(0, -1).join('/');
		if (dir !== '') await checkout.mkdir(dir, { recursive: true });
		await checkout.write(path, text);
	}
	const { deps, out, err } = await make_deps(root);
	strictEqual(
		await run_patch_npm_package({ ...deps, checkout: checkout.readonly() }, ['format']),
		0
	);
	deepStrictEqual(err, []);
	const pkg = root.dir('format/npm');
	for (const [name, text] of Object.entries(GENERATED_FORMAT)) {
		strictEqual(await pkg.read_text(name), text, name);
	}
	strictEqual(
		await pkg.read_text('package.json'),
		JSON.stringify(MANIFEST_FORMAT, null, '\t') + '\n'
	);
	for (const [name, source] of Object.entries(SOURCES)) {
		strictEqual(await pkg.read_text(name), SYNTHETIC_CHECKOUT[source], name);
	}
	// the summary lists what ships, largest first, and files of one size in the manifest's order
	deepStrictEqual(out, [
		'Patched crates/tsv_wasm/pkg/format/npm/tsv_wasm.js: reinstantiate hook',
		'Exports: format_css, format_svelte, format_typescript, IgnoreStack',
		'Created crates/tsv_wasm/pkg/format/npm/index.js',
		'Created crates/tsv_wasm/pkg/format/npm/browser.js',
		'Created crates/tsv_wasm/pkg/format/npm/worker.js',
		'Patched crates/tsv_wasm/pkg/format/npm/tsv_wasm.d.ts: esnext.disposable lib reference',
		'Created crates/tsv_wasm/pkg/format/npm/index.d.ts',
		'Created crates/tsv_wasm/pkg/format/npm/browser.d.ts',
		'Copied crates/tsv_wasm/README_format.md → crates/tsv_wasm/pkg/format/npm/README.md',
		'Copied LICENSE → crates/tsv_wasm/pkg/format/npm/LICENSE',
		'Copied crates/tsv_html/NOTICE → crates/tsv_wasm/pkg/format/npm/NOTICE',
		'Copied crates/tsv_wasm/npm/api.js → crates/tsv_wasm/pkg/format/npm/api.js',
		'Copied crates/tsv_wasm/npm/facade_error.d.ts → crates/tsv_wasm/pkg/format/npm/facade_error.d.ts',
		'Copied crates/tsv_wasm/npm/facade_format.d.ts → crates/tsv_wasm/pkg/format/npm/facade_format.d.ts',
		'Patched crates/tsv_wasm/pkg/format/npm/package.json: name → @fuzdev/tsv-format-wasm, version 0.8.1',
		'\nPackage contents (crates/tsv_wasm/pkg/format/npm):',
		'  tsv_wasm_bg.wasm    4.1 KB  →  1.5 KB gzipped',
		'  tsv_wasm.js         3.8 KB',
		'  browser.js          2.1 KB',
		'  index.d.ts          2.0 KB',
		'  browser.d.ts        1.6 KB',
		'  index.js            1.5 KB',
		'  package.json        1.4 KB',
		'  worker.js            443 B',
		'  facade_format.d.ts   277 B',
		'  tsv_wasm.d.ts        232 B',
		'  facade_error.d.ts     80 B',
		'  README.md             17 B',
		'  LICENSE                8 B',
		'  api.js                 7 B',
		'  NOTICE                 7 B'
	]);
});

test('a usage error exits 1 and opens no staging', async () => {
	await using root = (await make_staging('format')).root;
	for (const argv of [[], ['bogus'], ['npm'], ['../x'], ['format/../parse']]) {
		const { deps, out, err, opened } = await make_deps(root);
		strictEqual(await run_patch_npm_package(deps, argv), 1);
		deepStrictEqual(err, ['Usage: patch_npm_package.ts <format|parse|all>']);
		deepStrictEqual([out, opened], [[], []]);
	}
});

test('a glue patched already is refused, and left as it was', async () => {
	await using root = (await make_staging('format')).root;
	strictEqual(await run_patch_npm_package((await make_deps(root)).deps, ['format']), 0);
	const patched = await root.read_text('format/npm/tsv_wasm.js');
	const { deps, err } = await make_deps(root);
	strictEqual(await run_patch_npm_package(deps, ['format']), 1);
	deepStrictEqual(err, [
		'FAIL: generated tsv_wasm.js is already patched with the reinstantiate hook — re-run the wasm-pack build before patching again'
	]);
	strictEqual(await root.read_text('format/npm/tsv_wasm.js'), patched);
});

/** Each refusal: the variant, the glue, and the error it prints. The staging is unwritten. */
const REFUSALS: Array<[name: string, variant: string, glue: string, error: string]> = [
	[
		'a missing format export',
		'format',
		GLUE.replace('export function format_css', 'function format_css'),
		'FAIL: generated tsv_wasm.js is missing expected export `format_css`'
	],
	[
		'format exports in the parse variant',
		'parse',
		GLUE,
		'FAIL: parse variant contains format_* exports — stale build dir?'
	],
	[
		'no parse exports in the all variant',
		'all',
		GLUE,
		'FAIL: all variant has no parse_* exports — was `--features parse` passed?'
	],
	[
		'no type strip in the all variant',
		'all',
		GLUE + 'export function parse_css(source) {}\n',
		'FAIL: generated tsv_wasm.js is missing expected export `strip_typescript` — was the default `strip` feature dropped?'
	],
	[
		'parse exports in the format variant',
		'format',
		GLUE + 'export function parse_css(source) {}\n',
		'FAIL: format variant contains parse_* exports — stale build dir?'
	],
	[
		'strip exports in the format variant',
		'format',
		GLUE + 'export function strip_typescript(source) {}\n',
		'FAIL: format variant contains strip_* exports — stale build dir?'
	],
	[
		'import records in the format variant',
		'format',
		GLUE + 'export function parse_imports_svelte(source) {}\n',
		'FAIL: format variant contains parse_imports_* exports — stale build dir?'
	],
	[
		'a missing glue anchor',
		'format',
		GLUE.replace('function initSync(module)', 'function initSync(mod)'),
		'FAIL: generated tsv_wasm.js lacks the `function initSync(module)` anchor the reinstantiate hook patches against — did a wasm-bindgen upgrade reshape the glue?'
	],
	[
		'an async init with no load site',
		'format',
		GLUE.replace('const { instance, module } = await', 'const { instance, module } =  await'),
		'FAIL: found 0 `const { instance, module } = await __wbg_load(…);` site(s) in tsv_wasm.js, expected exactly one (in `__wbg_init`) — the async init drifted from the shape the overlapping-init re-check patches'
	],
	[
		'a class with no method site the guard rewrites',
		'format',
		GLUE.replace(
			'wasm.ignorestack_is_ignored(this.__wbg_ptr, path)',
			'wasm.ignorestack_is_ignored(0, 0, this.__wbg_ptr, path)'
		),
		'FAIL: found 1 exported class(es) in tsv_wasm.js but no method site handing `this.__wbg_ptr` to `wasm` — the method shape drifted from the pattern the stale-handle guard rewrites'
	],
	[
		'a method the guard does not recognize',
		'format',
		GLUE.replace(
			'    constructor() {',
			'    pop() {\n        wasm.ignorestack_pop(0, 0, this.__wbg_ptr);\n    }\n    constructor() {'
		),
		'FAIL: a method in tsv_wasm.js still hands `this.__wbg_ptr` to `wasm` unguarded — a call shape the stale-handle guard does not recognize'
	],
	[
		'a receiver read outside the shapes the guard covers',
		'format',
		GLUE.replace(
			'    constructor() {',
			'    alias() {\n        const p = this.__wbg_ptr;\n        return p;\n    }\n    constructor() {'
		),
		"FAIL: tsv_wasm.js reads a receiver the stale-handle guard does not cover — 1 unrecognized `this.__wbg_ptr` line(s)\n  const p = this.__wbg_ptr;\n1 `const ptr = this.__wbg_ptr;` read(s) and 1 `this.__destroy_into_raw()` call(s) for 1 class(es), where each class's `__destroy_into_raw` makes exactly one read and its `free()` one call"
	],
	[
		'a registry callback of another shape',
		'format',
		GLUE.replace('new FinalizationRegistry(ptr =>', 'new FinalizationRegistry((ptr) =>'),
		'FAIL: rewrote 0 FinalizationRegistry callback(s) and 1 free() method(s) in tsv_wasm.js but found 1 exported class(es) — the class shape drifted from the pattern the reinstantiate guard rewrites'
	],
	[
		'a class returned from Rust',
		'format',
		GLUE.replace(
			'        return this;\n    }\n}',
			'        return this;\n    }\n    static __wrap(obj) {\n        IgnoreStackFinalization.register(obj, obj.__wbg_ptr, obj);\n    }\n}'
		),
		'FAIL: class(es) IgnoreStack are RETURNED from Rust (a second FinalizationRegistry.register site, in `__wrap`). The lazy entries wrap each class in a guarded subclass, and `__wrap` builds the base prototype — so an instance handed back from Rust would not be `instanceof` the class browser.js/worker.js exports. Decide how those entries should present this class before shipping it.'
	]
];

for (const [name, variant, glue, error] of REFUSALS) {
	test(`refuses ${name}, writing nothing`, async () => {
		await using root = (await make_staging(variant, glue)).root;
		const { deps, out, err } = await make_deps(root);
		strictEqual(await run_patch_npm_package(deps, [variant]), 1);
		deepStrictEqual(err, [error]);
		deepStrictEqual(out, []);
		strictEqual(await root.read_text(`${variant}/npm/tsv_wasm.js`), glue);
		deepStrictEqual(
			(await Array.fromAsync(root.dir(`${variant}/npm`).list())).map((e) => e.name).sort(),
			['package.json', 'tsv_wasm.d.ts', 'tsv_wasm.js', 'tsv_wasm_bg.wasm']
		);
	});
}

test('a function the facade does not declare is refused after the glue is patched', async () => {
	const glue = GLUE + 'export function format_zzz(source) {}\n';
	await using root = (await make_staging('format', glue)).root;
	const { deps, err } = await make_deps(root);
	strictEqual(await run_patch_npm_package(deps, ['format']), 1);
	strictEqual(err.length, 1);
	ok(
		err[0]!.startsWith(
			'FAIL: missing declarations — declare function `format_zzz` in facade_format.d.ts.'
		)
	);
	strictEqual(await root.exists('format/npm/index.js'), false);
});

test('with no gzip the package is written and the summary fails', async () => {
	await using root = (await make_staging('format')).root;
	const { deps } = await make_deps(root, false);
	await rejects(run_patch_npm_package(deps, ['format']), /gzip is not on PATH/);
	ok(await root.exists('format/npm/package.json'));
	strictEqual(
		JSON.parse(await root.read_text('format/npm/package.json')).name,
		'@fuzdev/tsv-format-wasm'
	);
});
