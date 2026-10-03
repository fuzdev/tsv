/**
 * Type-checks the STAGED npm packages' published declarations the way a consumer's
 * compiler sees them: each package is installed into a temp consumer project's
 * `node_modules` (a copy of its `package.json` plus exactly the `files` it declares, so a
 * declaration missing from `files` is missing here too), and TypeScript resolves every
 * import through the package's `exports` map, conditions included.
 *
 * What it grades, per package — the three wasm packages and the native `@fuzdev/tsv`
 * loader:
 *
 * - every exported subpath — the root, `./worker` (wasm), `./locations` (parse-capable) —
 *   through lean consumer modules that call the surface with typed returns, forwarded
 *   option bags, `TsvSyntaxError` and the `IgnoreStack` verdict union, plus
 *   `// @ts-expect-error` negatives for what must NOT compile (an unused one is itself an
 *   error, so the negatives are pinned too);
 * - every ```ts / ```typescript block of the README the package ships, each as its own
 *   module, and every ```js / ```javascript block under `checkJs`;
 * - under a strict matrix (`strict`, `exactOptionalPropertyTypes`,
 *   `noUncheckedIndexedAccess`, `skipLibCheck: false`, `types: []`), once with
 *   `nodenext` resolution (the `node` condition: a wasm package's `index.d.ts`) and once
 *   with `bundler` (no `node` condition: its `browser.d.ts`).
 *
 * The lib is part of the claim. The wasm packages' entries name the DOM lib's
 * `RequestInfo` / `Response` / `WebAssembly` (their READMEs say so), so they grade under
 * `es2022` + `dom`; the napi loader declares none of those and `./locations` is pure JS
 * with no engine, so both grade under `es2022` alone — which pins that claim too. Each
 * package grades in programs of its own, so one package's lib reference can never
 * satisfy another's declarations.
 *
 * The two defect classes it pins: a declaration needing a lib the consumer did not ask for
 * (TS2550 — `IgnoreStack`'s `[Symbol.dispose]` without the `esnext.disposable` reference
 * `patch_npm_package.ts` prepends), and a name lost to star-export ambiguation between
 * `tsv_ast.d.ts` and `locations.d.ts` (TS2308) — pinned from both sides: the root
 * re-exports `Position` / `SourceLocation`, and `./locations` must not.
 *
 * A README block that only compiles under Node — it imports `node:` builtins or the
 * Node-only `wasm_module` — carries a `<!-- typecheck: node -->` line immediately above
 * its fence, and grades under `nodenext` with `@types/node` (from `benches/js`) instead
 * of both resolutions. That is the one marker; any other `typecheck:` comment is an error.
 *
 * Usage: `deno task typecheck:packages [format|parse|all|napi]...` — no arguments grades
 * every staged package (a package that is not staged is skipped, and nothing staged
 * fails); named packages are exactly the set graded, each required. A stale staging is
 * refused as the package suites refuse it (`check_staged_freshness.ts`, the same
 * `BENCH_STALE_OK=1` override). TypeScript and `@types/node` come from
 * `benches/js/node_modules` (`deno task bench:install`); their absence fails the run.
 *
 * TypeScript is loaded through `createRequire` from a computed path, so `deno check`
 * never follows it and `typecheck:scripts` stays node-modules-free.
 *
 * @module
 */

import { existsSync, realpathSync } from 'node:fs';
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { dirname, join, relative } from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import {
	assert_staged_fresh,
	NAPI_LOADER_DIR,
	napi_loader_checks,
	type StagedCheck,
	wasm_package_checks,
	wasm_package_dir
} from './check_staged_freshness.ts';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const BENCH_NODE_MODULES = `${ROOT}benches/js/node_modules`;
const TYPESCRIPT_PATH = `${BENCH_NODE_MODULES}/typescript/lib/typescript.js`;
const TYPES_ROOT = `${BENCH_NODE_MODULES}/@types`;

/** What this check loads from the `benches/js` install, as absolute paths —
 * `scripts/publish.ts` asks for the same set in its preflight. */
export const TYPECHECK_INSTALL: ReadonlyArray<{ what: string; path: string }> = [
	{ what: 'typescript', path: TYPESCRIPT_PATH },
	{ what: '@types/node', path: `${TYPES_ROOT}/node/package.json` }
];

type PackageKey = 'format' | 'parse' | 'all' | 'napi';

/** One staged package and what its declarations promise. */
interface PackageSpec {
	key: PackageKey;
	name: string;
	/** Repo-relative staging directory. */
	dir: string;
	/** `wasm` entries declare the WASM lifecycle and need the DOM lib; `napi` neither. */
	engine: 'wasm' | 'napi';
	format: boolean;
	parse: boolean;
	/** The command that restages it. */
	rebuild: string;
	/** The freshness checks that date the staging, its README included. */
	checks: Array<StagedCheck>;
}

/** The staged README is graded too, so it is dated against its source. */
const readme_check = (key: PackageKey, dir: string, source: string, rebuild: string) => ({
	label: `staged README (${key})`,
	staged: `${dir}/README.md`,
	crates: [],
	files: [source],
	rebuild
});

const wasm_package = (variant: 'format' | 'parse' | 'all', name: string): PackageSpec => {
	const dir = wasm_package_dir(variant);
	const rebuild = `deno task build:npm:${variant}`;
	return {
		key: variant,
		name,
		dir,
		engine: 'wasm',
		format: variant !== 'parse',
		parse: variant !== 'format',
		rebuild,
		checks: [
			...wasm_package_checks(variant),
			readme_check(variant, dir, `crates/tsv_wasm/README_${variant}.md`, rebuild)
		]
	};
};

const NAPI_REBUILD = 'deno task build:napi:packages';

const PACKAGES: Record<PackageKey, PackageSpec> = {
	format: wasm_package('format', '@fuzdev/tsv-format-wasm'),
	parse: wasm_package('parse', '@fuzdev/tsv-parse-wasm'),
	all: wasm_package('all', '@fuzdev/tsv-wasm'),
	napi: {
		key: 'napi',
		name: '@fuzdev/tsv',
		dir: NAPI_LOADER_DIR,
		engine: 'napi',
		format: true,
		parse: true,
		rebuild: NAPI_REBUILD,
		checks: [
			...napi_loader_checks(),
			readme_check('napi', NAPI_LOADER_DIR, 'crates/tsv_napi/npm/README.md', NAPI_REBUILD)
		]
	}
};

// -- consumer modules ---------------------------------------------------------

type Resolution = 'nodenext' | 'bundler';

/** One module of the consumer project and the programs that grade it. */
interface ConsumerFile {
	/** The package it imports — each package grades in programs of its own. */
	package: PackageSpec;
	/** Consumer-relative path. */
	path: string;
	content: string;
	resolutions: ReadonlyArray<Resolution>;
	dom: boolean;
	/** Adds `@types/node` (a README block marked `<!-- typecheck: node -->`). */
	node_types: boolean;
}

const BOTH: ReadonlyArray<Resolution> = ['nodenext', 'bundler'];

/** Type equality, so a declaration that degrades to `any` fails where an assignment
 * would still pass. */
const EXACT = `type Exact<A, B> =
	(<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;`;

/** The format family, over the package root. */
const format_module = (pkg: PackageSpec): string => `
import {format_css, format_svelte, format_typescript, IgnoreStack} from '${pkg.name}';
import type {FormatOptions, TsvSyntaxError, TypeScriptFormatOptions} from '${pkg.name}';

${EXACT}

const returns: [
	Exact<ReturnType<typeof format_svelte>, string>,
	Exact<ReturnType<typeof format_typescript>, string>,
	Exact<ReturnType<typeof format_css>, string>,
	Exact<TsvSyntaxError['start'], number>,
	Exact<TsvSyntaxError['loc'], {line: number; column: number}>,
	Exact<ReturnType<IgnoreStack['classify_dir']>, 'descend' | 'prune' | 'prune_warn'>
] = [true, true, true, true, true, true];

const formatted: string = format_svelte('<p>x</p>');
const typescript_options: TypeScriptFormatOptions = {sourceType: 'script'};
const script: string = format_typescript('let a', typescript_options);
// one bag forwarded to every formatter spells the TypeScript-only key as undefined
const forwarded: FormatOptions = {sourceType: undefined};
const css: string = format_css('a {}', forwarded);
format_typescript('let a', forwarded);
// @ts-expect-error the source type is TypeScript-only
format_css('a {}', {sourceType: 'module'});
// @ts-expect-error an unknown option key
format_svelte('<p>x</p>', {sourceTyp: undefined});
try {
	format_typescript('const = ;');
} catch (error) {
	if (error instanceof SyntaxError) {
		const {start, loc}: {start: number; loc: {line: number; column: number}} =
			error as TsvSyntaxError;
		void [start, loc];
	}
}

${pkg.engine === 'wasm' ? 'using stack = new IgnoreStack();' : 'const stack = new IgnoreStack();'}
stack.push_gitignore('', 'build/\\n');
const verdict: 'descend' | 'prune' | 'prune_warn' = stack.classify_dir('build', 'build', true);
const warning: string | undefined = stack.shadow_warning('src');
void [returns, formatted, script, css, verdict, warning];
`;

/** The parse family and the locator, over the package root. */
const parse_module = (pkg: PackageSpec): string => `
import {
	create_locator,
	parse_css,
	parse_css_json,
	parse_svelte,
	parse_svelte_json,
	parse_typescript,
	parse_typescript_json,
	reconstruct_locations
} from '${pkg.name}';
import type {
	ParseJsonOptions,
	ParseOptions,
	Position,
	Program,
	Root,
	SourceLocation,
	StyleSheetFile,
	TsvSyntaxError,
	TypeScriptParseJsonOptions,
	TypeScriptParseOptions
} from '${pkg.name}';

${EXACT}

const returns: [
	Exact<ReturnType<typeof parse_svelte>, Root>,
	Exact<ReturnType<typeof parse_typescript>, Program>,
	Exact<ReturnType<typeof parse_css>, StyleSheetFile>,
	Exact<ReturnType<typeof parse_svelte_json>, string>,
	Exact<ReturnType<typeof parse_typescript_json>, string>,
	Exact<ReturnType<typeof parse_css_json>, string>,
	Exact<TsvSyntaxError['start'], number>,
	Exact<TsvSyntaxError['loc'], {line: number; column: number}>
] = [true, true, true, true, true, true, true, true];
const typescript_options: TypeScriptParseOptions = {sourceType: 'script', locations: true};
const root: Root = parse_svelte('<p>x</p>', {locations: true});
const program: Program = parse_typescript('let a', typescript_options);
const sheet: StyleSheetFile = parse_css('a {}');
// one bag forwarded to every parser spells the TypeScript-only key as undefined
const forwarded: ParseOptions = {sourceType: undefined, locations: true};
parse_svelte('<p>x</p>', forwarded);
parse_typescript('let a', forwarded);
const typescript_json_options: TypeScriptParseJsonOptions = {sourceType: 'module'};
const wire: string = parse_typescript_json('let a', typescript_json_options);
const json_options: ParseJsonOptions = {sourceType: undefined};
const css_wire: string = parse_css_json('a {}', json_options);
const svelte_wire: string = parse_svelte_json('<p>x</p>');
parse_typescript_json('let a', {sourceType: undefined});
// @ts-expect-error a _json export returns the wire and takes no locations
parse_css_json('a {}', {locations: true});
// @ts-expect-error the source type is TypeScript-only
parse_svelte('<p>x</p>', {sourceType: 'module'});
// @ts-expect-error an unknown option key
parse_typescript('let a', {locatons: true});
try {
	parse_css('a {');
} catch (error) {
	if (error instanceof SyntaxError) {
		const {start, loc}: {start: number; loc: {line: number; column: number}} =
			error as TsvSyntaxError;
		void [start, loc];
	}
}

const locator = create_locator('let a', {language: 'typescript'});
const position: Position = locator.position_at(0);
const location: SourceLocation | null = locator.loc_of(program.body[0]);
const loc_of_returns: Exact<ReturnType<typeof locator.loc_of>, SourceLocation | null> = true;
const again: Program = locator.reconstruct(program);
const same: StyleSheetFile = reconstruct_locations(sheet, 'a {}');
void [returns, root, wire, css_wire, svelte_wire, position, location, loc_of_returns, again, same];
`;

/** `./locations` alone: the helper with no engine, graded without the DOM lib. */
const locations_module = (pkg: PackageSpec): string => `
import {create_locator, reconstruct_locations} from '${pkg.name}/locations';
import type {LocationLanguage, Locator, LocatorOptions} from '${pkg.name}/locations';
// @ts-expect-error Position is tsv_ast.d.ts's; exported here too, the root's star exports clash (TS2308)
import type {Position} from '${pkg.name}/locations';
// @ts-expect-error the subpath is the helper alone, no engine
import {parse_css} from '${pkg.name}/locations';

const language: LocationLanguage = 'svelte';
const options: LocatorOptions = {language};
const locator: Locator = create_locator('<p>x</p>', options);
const line: number = locator.position_at(0).line;
reconstruct_locations({type: 'Root', start: 0, end: 8}, '<p>x</p>', {language: undefined});
// @ts-expect-error the language is required: it picks the line rule
create_locator('a', {});
void [line, parse_css];
`;

/** `./worker`: the lazy entry a worker initializes from a handed module. */
const worker_module = (pkg: PackageSpec): string => `
import {${pkg.format ? 'format_css' : 'parse_css'}, init, init_sync} from '${pkg.name}/worker';
// @ts-expect-error the lazy entry compiles nothing, so it has no wasm_module
import {wasm_module} from '${pkg.name}/worker';

await init();
init_sync({module: new WebAssembly.Module(new Uint8Array(0))});
${pkg.format ? 'format_css' : 'parse_css'}('a {}');
void wasm_module;
`;

/** The root's lifecycle, per condition: `node` lands on the auto-init entry that exports
 * `wasm_module`, every other condition on the lazy one that does not. */
const entry_module = (pkg: PackageSpec, resolution: Resolution): string =>
	resolution === 'nodenext'
		? `
import {init, init_sync, reinstantiate, wasm_module} from '${pkg.name}';
import {init_sync as init_worker} from '${pkg.name}/worker';

${EXACT}

const module: Exact<typeof wasm_module, WebAssembly.Module> = true;
await init();
init_sync({module: wasm_module});
init_worker({module: wasm_module});
reinstantiate();
void module;
`
		: `
import {init, reinstantiate} from '${pkg.name}';
// @ts-expect-error the browser entry compiles nothing until init(), so it has no wasm_module
import {wasm_module} from '${pkg.name}';

await init();
reinstantiate();
void wasm_module;
`;

/** What a family-less package must not export. */
const absent_module = (pkg: PackageSpec): string => {
	const lines: Array<string> = [];
	if (!pkg.parse) {
		lines.push(
			'// @ts-expect-error the format-only package has no parser',
			`import {parse_css} from '${pkg.name}';`,
			'// @ts-expect-error the format-only package has no ./locations subpath',
			`import {create_locator} from '${pkg.name}/locations';`,
			'void [parse_css, create_locator];'
		);
	}
	if (!pkg.format) {
		lines.push(
			'// @ts-expect-error the parse-only package has no formatter',
			`import {format_css} from '${pkg.name}';`,
			'// @ts-expect-error the parse-only package ships no format option types',
			`import type {FormatOptions} from '${pkg.name}';`,
			'void format_css;'
		);
	}
	if (pkg.engine === 'napi') {
		lines.push(
			'// @ts-expect-error the native loader has no WASM lifecycle',
			`import {init} from '${pkg.name}';`,
			'void init;'
		);
	}
	return lines.length === 0 ? '' : `\n${lines.join('\n')}\n`;
};

/** A fenced TypeScript or JavaScript block of a README, with the line its fence opens on. */
export interface ReadmeBlock {
	line: number;
	lang: 'ts' | 'js';
	code: string;
	node: boolean;
}

const MARKER_RE = /^<!--\s*typecheck:\s*(.*?)\s*-->$/;
/** A fence line and its info string's first word (empty for a bare fence). */
const FENCE_RE = /^```\s*([^\s`]*)/;
const FENCE_LANGS: Record<string, 'ts' | 'js'> = {
	ts: 'ts',
	typescript: 'ts',
	js: 'js',
	javascript: 'js'
};
const graded_lang = (line: string | undefined): 'ts' | 'js' | undefined => {
	const info = FENCE_RE.exec(line ?? '')?.[1];
	return info === undefined ? undefined : FENCE_LANGS[info];
};

/**
 * The ```ts / ```typescript / ```js / ```javascript blocks of a README, each flagged when
 * a `<!-- typecheck: node -->` line sits immediately above its fence. A fence opens on any
 * info string (a language word plus whatever follows it) and closes on a bare fence.
 *
 * @throws Error on a `typecheck:` marker with any other value, one not immediately
 *   followed by a graded fence — either would otherwise grade nothing silently — or an
 *   unclosed fence
 */
export function readme_blocks(markdown: string, label: string): Array<ReadmeBlock> {
	const lines = markdown.split('\n');
	const blocks: Array<ReadmeBlock> = [];
	for (let i = 0; i < lines.length; i++) {
		const line = lines[i]!;
		const marked = MARKER_RE.exec(line.trim());
		if (marked) {
			if (marked[1] !== 'node') {
				throw new Error(`${label}:${i + 1}: unknown marker '${line.trim()}' (only 'node')`);
			}
			if (graded_lang(lines[i + 1]) === undefined) {
				throw new Error(
					`${label}:${i + 1}: marker not immediately followed by a TypeScript or JavaScript fence`
				);
			}
			continue;
		}
		if (!FENCE_RE.test(line)) continue;
		const close = lines.findIndex((l, j) => j > i && /^```\s*$/.test(l));
		if (close === -1) throw new Error(`${label}:${i + 1}: unclosed fence`);
		const lang = graded_lang(line);
		if (lang) {
			blocks.push({
				line: i + 1,
				lang,
				code: lines.slice(i + 1, close).join('\n'),
				node: i > 0 && MARKER_RE.test(lines[i - 1]!.trim())
			});
		}
		i = close;
	}
	return blocks;
}

/** Where a consumer module grades: both resolutions by default; the package's lib. */
interface AddOptions {
	resolutions?: ReadonlyArray<Resolution>;
	/** `false` grades without the DOM lib even for a wasm package. */
	dom?: boolean;
	node_types?: boolean;
	extension?: 'ts' | 'js';
}

/** Every consumer module for one package, its README blocks included. */
function consumer_files(pkg: PackageSpec, readme: string): Array<ConsumerFile> {
	const files: Array<ConsumerFile> = [];
	const add = (name: string, content: string, options: AddOptions = {}): void => {
		if (content === '') return;
		files.push({
			package: pkg,
			path: `${pkg.key}/${name}.${options.extension ?? 'ts'}`,
			content,
			resolutions: options.resolutions ?? BOTH,
			dom: options.dom ?? pkg.engine === 'wasm',
			node_types: options.node_types ?? false
		});
	};
	if (pkg.format) add('format', format_module(pkg));
	if (pkg.parse) {
		add('parse', parse_module(pkg));
		add('locations', locations_module(pkg), { dom: false });
	}
	if (pkg.engine === 'wasm') {
		add('worker', worker_module(pkg));
		add('entry_node', entry_module(pkg, 'nodenext'), { resolutions: ['nodenext'] });
		add('entry_bundler', entry_module(pkg, 'bundler'), { resolutions: ['bundler'] });
	}
	add('absent', absent_module(pkg));
	for (const block of readme_blocks(readme, `${pkg.dir}/README.md`)) {
		// `export {}` makes every block its own module, whatever it declares
		add(`readme_line_${block.line}`, `${block.code}\nexport {};\n`, {
			extension: block.lang,
			...(block.node ? { resolutions: ['nodenext'], node_types: true } : {})
		});
	}
	return files;
}

// -- staging ------------------------------------------------------------------

/** Every `types` target an `exports` map names, at any condition depth. */
function export_types(exports: unknown): Set<string> {
	const found = new Set<string>();
	const walk = (node: unknown): void => {
		if (node === null || typeof node !== 'object') return;
		for (const [key, value] of Object.entries(node)) {
			if (key === 'types' && typeof value === 'string') found.add(value);
			else walk(value);
		}
	};
	walk(exports);
	if (found.size === 0) throw new Error('the exports map names no types');
	return found;
}

/**
 * Install each package into `consumer_dir/node_modules` as npm would — `package.json`
 * plus the `files` it declares — and write the consumer modules beside it.
 *
 * @returns the consumer modules, and every declaration file the packages' `exports` maps
 *   name (consumer-relative) — the entry points a consumer's compiler starts from, which
 *   the grading must load at least once
 * @throws Error naming the file and the restage command when a staging lacks a file its
 *   `files` declares
 */
async function stage_consumer(
	consumer_dir: string,
	packages: ReadonlyArray<PackageSpec>
): Promise<{ files: Array<ConsumerFile>; declarations: Array<string> }> {
	await writeFile(
		join(consumer_dir, 'package.json'),
		JSON.stringify({ private: true, type: 'module' }, null, '\t') + '\n'
	);
	const files: Array<ConsumerFile> = [];
	const declarations: Array<string> = [];
	for (const pkg of packages) {
		const source_dir = join(ROOT, pkg.dir);
		const manifest = JSON.parse(await readFile(join(source_dir, 'package.json'), 'utf8')) as {
			files?: Array<string>;
			exports?: unknown;
		};
		if (!manifest.files) throw new Error(`${pkg.dir}/package.json declares no files`);
		const installed = join('node_modules', pkg.name);
		await mkdir(join(consumer_dir, installed), { recursive: true });
		for (const file of ['package.json', ...manifest.files]) {
			if (!existsSync(join(source_dir, file))) {
				throw new Error(
					`${pkg.name}: the staging lacks ${pkg.dir}/${file}, which its files list declares — restage: ${pkg.rebuild}`
				);
			}
			await cp(join(source_dir, file), join(consumer_dir, installed, file), { recursive: true });
		}
		for (const types of export_types(manifest.exports)) {
			declarations.push(join(installed, types));
		}
		const readme = await readFile(join(source_dir, 'README.md'), 'utf8');
		files.push(...consumer_files(pkg, readme));
	}
	for (const file of files) {
		const path = join(consumer_dir, file.path);
		await mkdir(dirname(path), { recursive: true });
		await writeFile(path, file.content);
	}
	return { files, declarations };
}

// -- grading ------------------------------------------------------------------

/** The slice of the TypeScript compiler API this uses — typed here because the module is
 * loaded at runtime from `benches/js/node_modules`, where `deno check` cannot follow. */
interface TypeScriptApi {
	version: string;
	convertCompilerOptionsFromJson(
		json: Record<string, unknown>,
		base_path: string
	): { options: unknown; errors: ReadonlyArray<unknown> };
	createProgram(root_names: ReadonlyArray<string>, options: unknown): TypeScriptProgram;
	getPreEmitDiagnostics(program: TypeScriptProgram): ReadonlyArray<unknown>;
	formatDiagnostics(diagnostics: ReadonlyArray<unknown>, host: unknown): string;
}

interface TypeScriptProgram {
	getSourceFiles(): ReadonlyArray<{ fileName: string }>;
}

/** Load the TypeScript the bench harness installs, or exit naming the install. */
function load_typescript(): TypeScriptApi {
	for (const { what, path } of TYPECHECK_INSTALL) {
		if (!existsSync(path)) {
			console.error(
				`✗ ${what} is not installed at ${relative(ROOT, path)} — run \`deno task bench:install\``
			);
			process.exit(1);
		}
	}
	return createRequire(import.meta.url)(TYPESCRIPT_PATH) as TypeScriptApi;
}

/** One compiler configuration of the matrix. */
interface ProgramKey {
	resolution: Resolution;
	dom: boolean;
	node_types: boolean;
}

const key_label = (key: ProgramKey): string =>
	`${key.resolution} · es2022${key.dom ? '+dom' : ''}${key.node_types ? ' · @types/node' : ''}`;

const compiler_options = (key: ProgramKey): Record<string, unknown> => ({
	strict: true,
	exactOptionalPropertyTypes: true,
	noUncheckedIndexedAccess: true,
	skipLibCheck: false,
	// the README's JavaScript blocks, under the same strictness
	allowJs: true,
	checkJs: true,
	// a JS reader's catch binding is `any`; no declaration's typing reads this
	useUnknownInCatchVariables: false,
	noEmit: true,
	target: 'es2022',
	lib: key.dom ? ['es2022', 'dom'] : ['es2022'],
	...(key.resolution === 'nodenext'
		? { module: 'nodenext', moduleResolution: 'nodenext' }
		: { module: 'esnext', moduleResolution: 'bundler' }),
	...(key.node_types ? { types: ['node'], typeRoots: [TYPES_ROOT] } : { types: [] })
});

/** One graded program's verdict. */
interface ProgramResult {
	label: string;
	files: number;
	/** Formatted diagnostics, empty when clean. */
	diagnostics: string;
	error_count: number;
}

/**
 * Compile every consumer module under each configuration it names, one program per
 * package and configuration, and check that every declaration entry point was loaded by
 * some program — an entry nothing loads would be graded by nothing.
 */
function grade_consumer(
	ts: TypeScriptApi,
	consumer_dir: string,
	files: ReadonlyArray<ConsumerFile>,
	declarations: ReadonlyArray<string>
): { results: Array<ProgramResult>; unloaded: Array<string> } {
	const groups = new Map<string, { key: ProgramKey; roots: Array<string> }>();
	for (const file of files) {
		for (const resolution of file.resolutions) {
			const key: ProgramKey = { resolution, dom: file.dom, node_types: file.node_types };
			const label = `${file.package.name} · ${key_label(key)}`;
			let group = groups.get(label);
			if (!group) groups.set(label, (group = { key, roots: [] }));
			group.roots.push(join(consumer_dir, file.path));
		}
	}
	const host = {
		getCanonicalFileName: (f: string) => f,
		getCurrentDirectory: () => consumer_dir,
		getNewLine: () => '\n'
	};
	const loaded = new Set<string>();
	const results: Array<ProgramResult> = [];
	for (const [label, { key, roots }] of groups) {
		const converted = ts.convertCompilerOptionsFromJson(compiler_options(key), consumer_dir);
		if (converted.errors.length > 0) {
			throw new Error(
				`${label}: invalid compiler options\n${ts.formatDiagnostics(converted.errors, host)}`
			);
		}
		const program = ts.createProgram(roots, converted.options);
		for (const source of program.getSourceFiles()) loaded.add(source.fileName);
		const diagnostics = ts.getPreEmitDiagnostics(program);
		results.push({
			label,
			files: roots.length,
			diagnostics: diagnostics.length === 0 ? '' : ts.formatDiagnostics(diagnostics, host),
			error_count: diagnostics.length
		});
	}
	const unloaded = declarations.filter((d) => !loaded.has(join(consumer_dir, d)));
	return { results, unloaded };
}

// -- main ---------------------------------------------------------------------

/** Which packages to grade: the named ones, or every staged one. */
function select_packages(args: ReadonlyArray<string>): Array<PackageSpec> {
	if (args.length > 0) {
		const unknown = args.filter((arg) => !(arg in PACKAGES));
		if (unknown.length > 0) {
			console.error(
				`✗ unknown package ${unknown.join(', ')} (expected format, parse, all or napi)`
			);
			process.exit(1);
		}
		return [...new Set(args)].map((arg) => PACKAGES[arg as PackageKey]);
	}
	const staged: Array<PackageSpec> = [];
	for (const pkg of Object.values(PACKAGES)) {
		if (existsSync(join(ROOT, pkg.dir, 'package.json'))) staged.push(pkg);
		else console.log(`  skip: ${pkg.name} is not staged (${pkg.dir})`);
	}
	if (staged.length === 0) {
		console.error(
			'✗ no package is staged — run `deno task build:packages` and/or `deno task build:napi:packages`'
		);
		process.exit(1);
	}
	return staged;
}

/** Stage, grade and report; true when every program is clean. */
async function grade(ts: TypeScriptApi, packages: ReadonlyArray<PackageSpec>): Promise<boolean> {
	// the real path, so the compiler's file names match what this reads back
	const consumer_dir = realpathSync(await mkdtemp(join(tmpdir(), 'tsv-typecheck-packages-')));
	try {
		const { files, declarations } = await stage_consumer(consumer_dir, packages);
		const { results, unloaded } = grade_consumer(ts, consumer_dir, files, declarations);
		let clean = true;
		for (const result of results) {
			if (result.error_count === 0) {
				console.log(`  PASS: ${result.label} (${result.files} modules)`);
			} else {
				clean = false;
				console.error(`  FAIL: ${result.label} — ${result.error_count} diagnostics`);
				console.error(result.diagnostics.replace(/^/gm, '    '));
			}
		}
		if (unloaded.length > 0) {
			clean = false;
			console.error(
				`  FAIL: declaration entry points no program loaded:\n    ${unloaded.join('\n    ')}`
			);
		}
		return clean;
	} catch (error) {
		console.error(`  FAIL: ${error instanceof Error ? error.message : String(error)}`);
		return false;
	} finally {
		await rm(consumer_dir, { recursive: true, force: true });
	}
}

async function main(): Promise<void> {
	const ts = load_typescript();
	const packages = select_packages(process.argv.slice(2));
	// a named package that is not staged fails here too: a missing staged file is fatal
	await assert_staged_fresh(packages.flatMap((pkg) => pkg.checks));
	console.log(`Type-checking the staged packages' declarations with TypeScript ${ts.version}`);
	const names = packages.map((p) => p.name).join(', ');
	if (!(await grade(ts, packages))) {
		console.error(`\n✗ ${names}: declarations do not type-check`);
		process.exit(1);
	}
	console.log(`\n✓ ${names}: declarations type-check`);
}

if (import.meta.main) await main();
