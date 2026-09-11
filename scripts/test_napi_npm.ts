/**
 * Node.js tests for the staged N-API npm packages — the `@fuzdev/tsv`
 * loader over a real platform package, consumed the way npm lays them out.
 *
 * `scripts/test_napi.ts` covers the raw addon boundary; this covers the
 * PACKAGE surface a consumer installs: the loader's platform resolution, that
 * both an ESM and a CommonJS host can load it (the loader is ESM, like the
 * wasm packages; the platform `.node` rides a `createRequire` shim), the
 * wasm-parity `(source, options?)` bags with their
 * exact error strings, the `_json` string variants, the package.json
 * selection fields, the unsupported-platform error, and the `tsv` bin —
 * `bin.js`, the dispatcher that execs the platform package's native
 * `tsv_cli` binary: that it really dispatches (argh's help output), that it
 * forwards exit codes, stdio, and stdin, that `--version` matches the staged
 * package version (binary↔package lockstep), that `npm pack` would ship the
 * binary (executable, where a mode exists), that a child's signal death is
 * re-raised, and that with the binary removed or unrunnable it degrades to the
 * shared `cli.js` JS mirror over the native engine. Because both CLIs are
 * present here and only here, this is also where their FLAG SETS are held
 * together — the mirror's are hand-written and would otherwise drift. The
 * discovery-parity scenario table runs through BOTH bin entries — the
 * dispatcher (native discovery through the shim, the real `npx tsv` path)
 * and cli.js directly (the fallback JS loop over the native `IgnoreStack`),
 * mirroring `scripts/test_npm.ts`'s CLI coverage over the wasm copy.
 *
 * Stages `crates/tsv_napi/pkg/{napi,<triple>}` into a temp `node_modules`
 * (the loader's `require('@fuzdev/tsv-<triple>')` resolves upward from
 * its own location, so a copy inside a temp node_modules resolves its sibling
 * there). A second staging WITHOUT the platform package asserts the
 * unsupported-platform error path; a third with the platform package but the
 * CLI binary removed asserts the dispatcher's JS fallback; two further
 * POSIX-only stagings assert the degraded binary paths (non-executable →
 * warn + JS fallback, signal death → re-raise).
 *
 * Usage: node --test scripts/test_napi_npm.ts   (or `deno task test:napi:npm`)
 * Prerequisite: deno task build:napi:packages
 */

import { after, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
	chmodSync,
	cpSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
	readFileSync,
	rmSync,
	statSync,
	writeFileSync
} from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';

import { CORE_CRATES, WASM_CRATES } from '../benches/js/lib/tsv_artifacts.ts';
import { assert_staged_fresh, staged_staleness } from './check_staged_freshness.ts';
import { register_discovery_parity_suite } from './discovery_parity_suite.ts';

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
	cpSync(join(pkg_root, 'napi'), join(scope, 'tsv'), { recursive: true });
	if (with_platform) {
		cpSync(join(pkg_root, triple), join(scope, `tsv-${triple}`), { recursive: true });
	}
	return tmp;
};

const cli_binary_name = process.platform === 'win32' ? 'tsv.exe' : 'tsv';

// A stale staging silently green-tests OLD code: this suite once sat over a
// pre-fix `tsv_cli` binary in crates/tsv_napi/pkg (it still panicked on
// `--jobs 100000`) and `test:napi:npm:run` would have tested it without a
// word. Staged mtimes are compared directly against the SOURCES, which
// catches both lags at once — a `target/` build behind the sources, and a
// staged copy behind the build. `deno task test:napi:npm` (build-first)
// passes this for free; BENCH_STALE_OK=1 is the deliberate-stale override.
await assert_staged_fresh([
	{
		label: 'staged N-API addon',
		staged: `${pkg_root}/${triple}/tsv_napi.node`,
		crates: [...CORE_CRATES, 'tsv_napi'],
		files: ['scripts/build_napi_packages.ts'],
		rebuild: 'deno task build:napi:packages'
	},
	{
		// the incident artifact: the real tsv_cli binary shipped beside the addon
		label: 'staged native CLI binary',
		staged: `${pkg_root}/${triple}/${cli_binary_name}`,
		crates: [...CORE_CRATES, 'tsv_cli', 'tsv_ignore', 'tsv_discover'],
		files: ['scripts/build_napi_packages.ts'],
		rebuild: 'deno task build:napi:packages'
	},
	{
		// Every hand-written CODE source the loader package copies, not just
		// index.js: the staged files are written in one pass, so index.js's mtime
		// dates the whole staging and a sibling edited since then is the same
		// staleness. (`cli.js` gets its own check below only because it is the one
		// shared with the wasm package. `README.md` and `LICENSE` are copied too
		// and deliberately absent — nothing here reads them, so their age cannot
		// make a verdict wrong.)
		label: 'staged loader',
		staged: `${pkg_root}/napi/index.js`,
		crates: [],
		files: [
			'crates/tsv_napi/npm/index.js',
			'crates/tsv_napi/npm/index.d.ts',
			'crates/tsv_napi/npm/platform.js',
			'crates/tsv_napi/npm/bin.js',
			'crates/tsv_wasm/npm/locations.js',
			'crates/tsv_wasm/npm/locations.d.ts',
			'crates/tsv_wasm/types/tsv_ast.d.ts',
			'scripts/build_napi_packages.ts',
			'scripts/npm_metadata.ts'
		],
		rebuild: 'deno task build:napi:packages'
	},
	{
		label: 'staged cli.js mirror',
		staged: `${pkg_root}/napi/cli.js`,
		crates: [],
		files: ['crates/tsv_wasm/npm/cli.js'],
		rebuild: 'deno task build:napi:packages'
	}
]);

// The one artifact this suite READS but does not build: `@fuzdev/tsv_wasm`,
// which the export-set parity claim below compares the loader against. Absent
// or STALE it is skipped, not failed — `deno task test:napi:npm` cannot
// refresh another package's staging, and the two cases are equally unusable
// here, since an export set captured before the delta moved reads as clean.
// Sources are the wasm bundle's own (`scripts/validate_artifacts.ts` names the
// same set), because a feature or an export moves with them.
const wasm_package_index = 'crates/tsv_wasm/pkg/all/npm/index.js';
const wasm_parity_staleness = await staged_staleness({
	label: 'the @fuzdev/tsv_wasm package',
	staged: wasm_package_index,
	crates: [...CORE_CRATES, ...WASM_CRATES],
	files: ['scripts/patch_npm_package.ts', 'scripts/npm_metadata.ts', 'deno.json'],
	rebuild: 'deno task build:npm:all'
});
const wasm_parity_skip: string | false = wasm_parity_staleness
	? `${wasm_parity_staleness.reason} — restage: deno task build:npm:all`
	: false;

const staged = stage(true);
const staged_bare = stage(false);
// The dispatcher-fallback staging: platform package present, CLI binary
// removed — bin.js must degrade to the cli.js JS mirror over the native engine.
const staged_no_binary = stage(true);
rmSync(join(staged_no_binary, 'node_modules', '@fuzdev', `tsv-${triple}`, cli_binary_name));

// Degraded-binary stagings, posix-only: Windows has no execute bit, and the
// signal fake is a shell script.
const posix = process.platform !== 'win32';
let staged_bad_mode = '';
let staged_signal = '';
if (posix) {
	// binary present but not executable — the spawn-EACCES fallback path
	staged_bad_mode = stage(true);
	chmodSync(
		join(staged_bad_mode, 'node_modules', '@fuzdev', `tsv-${triple}`, cli_binary_name),
		0o644
	);
	// "binary" that kills itself — the signal re-raise path
	staged_signal = stage(true);
	writeFileSync(
		join(staged_signal, 'node_modules', '@fuzdev', `tsv-${triple}`, cli_binary_name),
		'#!/bin/sh\nkill -TERM $$\n',
		{ mode: 0o755 }
	);
}

/** The empty tree the `--jobs` verdict table lists (see that suite for why an
 * empty one). Staged here, beside the other temp dirs, so the file's ONE
 * `after` owns every cleanup: a hook registered inside a `describe` does not
 * run when a `--test-name-pattern` filters that suite out, while the body that
 * made the directory runs regardless — so iterating on one test would leak a
 * staging per run. */
const empty_dir = mkdtempSync(join(tmpdir(), 'tsv_jobs_'));

/** Scratch tree for the invalid-UTF-8 parity rows, which need real files on
 * disk to prove neither bin rewrote one. Module scope for the same reason as
 * `empty_dir` — the file's one `after` owns the cleanup. */
const utf8_dir = mkdtempSync(join(tmpdir(), 'tsv_utf8_'));

after(() => {
	// Best-effort: on Windows the loaded addon stays mapped into the process,
	// so its .node file — and therefore `staged` — is undeletable until exit
	// (EPERM). A leaked temp staging is harmless (ephemeral CI runners, OS
	// temp); a cleanup failure must not fail an otherwise-green suite. The
	// bare staging loads nothing and deletes everywhere.
	for (const dir of [
		staged,
		staged_bare,
		staged_no_binary,
		staged_bad_mode,
		staged_signal,
		empty_dir,
		utf8_dir
	]) {
		if (!dir) continue;
		try {
			rmSync(dir, { recursive: true, force: true });
		} catch {
			// leaked — see above
		}
	}
});

const loader_path = join(staged, 'node_modules', '@fuzdev', 'tsv', 'index.js');
// ESM import of the ESM loader — the ordinary consumer path.
const api = await import(pathToFileURL(loader_path).href);
/**
 * The libc detection, loaded here rather than in its own suite far below.
 * Two reasons, and the second is a trap:
 *
 * - A `let` filled in by the suite's first test makes every later row depend
 *   on that one having run, so a `--test-name-pattern` selecting a single row
 *   reports `is_musl is not a function` instead of the verdict it is about.
 * - ⚠️ NO TOP-LEVEL `await` MAY FOLLOW THE FIRST `describe` IN THIS FILE. Each
 *   one yields, and node:test drains whatever is already registered while it
 *   does — including the root `after`, which deletes `staged`. Registration
 *   below then races a teardown that already ran, and the suites past the
 *   await fail with `Cannot find module .../@fuzdev/tsv/cli.js`. A full run
 *   usually wins the race (the registered suites outlast the await) and a
 *   filtered one loses it, which is the worst shape a trap can have. Every
 *   top-level await in this file therefore sits above the first `describe`.
 */
const { is_musl, platform_triple } = (await import(
	pathToFileURL(join(staged, 'node_modules', '@fuzdev', 'tsv', 'platform.js')).href
)) as {
	is_musl: (probes?: unknown) => boolean;
	platform_triple: () => string;
};

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

describe('@fuzdev/tsv loader (staged npm shape)', () => {
	it('formats every language', () => {
		assert.equal(api.format_typescript('const   x=1'), 'const x = 1;\n');
		assert.equal(api.format_css('a{color:red}'), 'a {\n\tcolor: red;\n}\n');
		assert.equal(api.format_svelte('<div   >x</div   >'), '<div>x</div>\n');
	});

	// The four WASM lifecycle exports are deliberately ABSENT, and the absence is API:
	// cli.js reads a missing `wasm_module` as "bind workers to this loader" and
	// a missing `reinstantiate` as "a trapped engine cannot be recovered" — an
	// accidental export here would silently flip those branches.
	it('exports none of the four WASM lifecycle exports', () => {
		assert.strictEqual(api.wasm_module, undefined);
		assert.strictEqual(api.reinstantiate, undefined);
		assert.strictEqual(api.init, undefined);
		assert.strictEqual(api.init_sync, undefined);
	});

	// The class carries the same kind of delta one level down: a GC-managed
	// native object has no hand to free, so `free()` and the `[Symbol.dispose]`
	// wasm-bindgen aliases to it are absent here. Stated as its own claim
	// because the README lists it beside the four above, and a reader who takes
	// "export for export" literally would otherwise expect `free()` to exist.
	it('its IgnoreStack carries no manual-lifetime methods', () => {
		const proto = api.IgnoreStack.prototype;
		assert.strictEqual(proto.free, undefined);
		assert.strictEqual(proto[Symbol.dispose], undefined);
		assert.strictEqual(typeof proto.classify_dir, 'function');
	});

	// The parity claim itself, as a SET rather than as prose maintained in two
	// places (`npm/index.js`'s module doc and the README both list the delta).
	// SKIPPED, not failed, when the wasm package is absent or stale: this task
	// does not build that package, so grading it either way would be answering
	// about code the run cannot refresh — and a stale staging is exactly as
	// unusable here as a missing one, since an export set from before the delta
	// moved reads as clean. So it gates wherever both are current: after
	// `build:packages` locally, and in the CI job that builds every publishable
	// bundle. The verdict is `wasm_parity_skip`, graded at module scope because
	// reading mtimes is async and a `describe` body must register its tests
	// synchronously.
	it(
		'differs from @fuzdev/tsv_wasm by exactly the WASM lifecycle',
		{ skip: wasm_parity_skip },
		async () => {
			const wasm = await import(pathToFileURL(wasm_package_index).href);
			assert.deepEqual(
				Object.keys(wasm)
					.filter((name) => !(name in api))
					.sort(),
				['init', 'init_sync', 'reinstantiate', 'wasm_module'],
				'the wasm package has an export this loader is missing'
			);
			assert.deepEqual(
				Object.keys(api)
					.filter((name) => !(name in wasm))
					.sort(),
				[],
				'this loader has an export the wasm package is missing'
			);
			// And one level down, over the shared class. `Reflect.ownKeys`, not
			// `getOwnPropertyNames`: half the stated delta is `[Symbol.dispose]`,
			// which the glue assigns as an own SYMBOL-keyed property after the
			// class body, and a name-only walk cannot see it — so the prose would
			// have gone on claiming a delta the set diff never graded.
			const own_keys = (proto: object): Array<string> => Reflect.ownKeys(proto).map(String);
			const wasm_methods = own_keys(wasm.IgnoreStack.prototype);
			const native_methods = own_keys(api.IgnoreStack.prototype);
			assert.deepEqual(
				wasm_methods.filter((name) => !native_methods.includes(name)).sort(),
				['Symbol(Symbol.dispose)', '__destroy_into_raw', 'free'],
				'the wasm IgnoreStack has a method this one is missing beyond its manual lifetime'
			);
			assert.deepEqual(
				native_methods.filter((name) => !wasm_methods.includes(name)).sort(),
				[],
				'the native IgnoreStack has a method the wasm one is missing'
			);
		}
	);

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

	// The span-only wire and the helper that makes it usable ship together —
	// the pure-JS `locations.js` is copied in from the wasm packages, so this
	// asserts the copy landed AND that its output matches the loc-bearing wire
	// this same package emits by default.
	it('the locations helpers ship alongside the span-only wire', () => {
		const source = 'const x = 1;\nconst y = 2;\n';
		const spans = api.parse_typescript(source, { locations: false });
		assert.equal(spans.body[1].loc, undefined, 'span-only wire starts without loc');
		assert.equal(api.reconstruct_locations(spans, source), spans, 'mutates and returns the ast');
		assert.deepEqual(spans.body[1].loc, api.parse_typescript(source).body[1].loc);
		// The amortized entry point over the same source.
		const locator = api.create_locator(source);
		assert.deepEqual(locator.loc_of(spans.body[1]), spans.body[1].loc);
		assert.deepEqual(api.loc_of(spans.body[1], source), spans.body[1].loc);
	});

	it('the TypeScript sourceType axis reaches parse AND format', () => {
		const script_only = 'var await = 1;\n';
		assert.equal(api.parse_typescript(script_only, { sourceType: 'script' }).type, 'Program');
		assert.throws(() => api.parse_typescript(script_only, { sourceType: 'module' }));
		assert.throws(() => api.parse_typescript(script_only));
		assert.equal(
			api.format_typescript('var   await=1', { sourceType: 'script' }),
			'var await = 1;\n'
		);
		// A set source type is exact; an unset one takes the module-then-script
		// fallback, so the same script-only source formats with no bag at all.
		assert.throws(() => api.format_typescript('var   await=1', { sourceType: 'module' }));
		assert.equal(api.format_typescript('var   await=1'), 'var await = 1;\n');
		assert.equal(api.format_typescript('export const   x=1'), 'export const x = 1;\n');
		// Broken under BOTH grammars: the script retry died at line 1's `import` — a goal
		// gate, which settles the file as a module — so the module's error is the reported
		// one (`with` at line 2).
		throws_with(
			() => api.format_typescript("import x from 'y';\nwith (a) {\n\tb;\n}\n"),
			"The 'with' statement is not allowed in strict mode"
		);
		throws_with(
			() => api.parse_typescript('const x = 1;', { sourceType: 'sloppy' }),
			"invalid sourceType 'sloppy' (expected 'script' or 'module')"
		);
	});

	it('option bags carry the wasm package error semantics, string for string', () => {
		// Unknown keys error whatever their value — undefined included.
		throws_with(
			() => api.parse_typescript('const x = 1;', { locatons: false }),
			"unknown parse option 'locatons' (expected 'locations' or 'sourceType')"
		);
		throws_with(
			() => api.parse_typescript('const x = 1;', { locatons: undefined }),
			"unknown parse option 'locatons'"
		);
		// The TS-only sourceType on other languages: a SET value throws, undefined forwards.
		throws_with(
			() => api.parse_svelte('<div>x</div>', { sourceType: 'script' }),
			"parse option 'sourceType' is only supported for TypeScript"
		);
		assert.equal(api.parse_svelte('<div>x</div>', { sourceType: undefined }).type, 'Root');
		throws_with(
			() => api.format_css('a{}', { sourceType: 'script' }),
			"format option 'sourceType' is only supported for TypeScript"
		);
		// `locations` shapes a parse wire; format emits none — unknown key there.
		throws_with(
			() => api.format_typescript('const x = 1;', { locations: false }),
			"unknown format option 'locations' (expected 'sourceType')"
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

	// The discovery matcher rides the package as a class. The `undefined`
	// assertions are the load-bearing ones: napi-rs maps `None` to `null` by
	// default, so the binding spells these `Either<String, Undefined>` to match
	// what wasm-bindgen hands a caller. A regression here is silent under
	// truthiness checks and only bites a consumer testing `=== undefined`.
	it('IgnoreStack mirrors the wasm discovery surface', () => {
		const stack = new api.IgnoreStack();
		assert.equal(stack.is_empty(), true);
		stack.push_gitignore('', 'dist/\n');
		assert.equal(stack.is_empty(), false);
		assert.equal(stack.is_ignored('dist', true), true);
		assert.equal(stack.is_ignored('src/a.ts', false), false);
		assert.equal(stack.classify_dir('node_modules', 'node_modules', false), 'prune');
		assert.equal(stack.classify_dir('src', 'src', false), 'descend');
		assert.equal(stack.should_format_file('a.ts', 'src/a.ts'), true);
		assert.equal(stack.should_format_file('a.txt', 'src/a.txt'), false);
		assert.equal(stack.is_path_pruned('node_modules/x.ts'), true);
		// A formattable extension yields no error, spelled `undefined` like wasm's.
		assert.strictEqual(stack.unsupported_extension_error('a.ts'), undefined);
		assert.match(stack.unsupported_extension_error('a.txt')!, /unsupported file extension/);
		assert.strictEqual(stack.prettierignore_shadowed_warning('d', true, false, false), undefined);
		assert.match(stack.prettierignore_shadowed_warning('d', true, true, true)!, /shadowed/);
		assert.strictEqual(
			stack.prettierignore_outside_repo_warning('d', true, true, false),
			undefined
		);
		stack.pop_gitignore();
		assert.equal(stack.is_empty(), true);
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

	// The loader is ESM, so a CommonJS host reaches it by dynamic import — the
	// path that works on every Node the package supports. (Node >= 22.12 also
	// allows a plain `require()` of ESM, but the `engines` floor is 22.0, so the
	// universal path is what's gated here.) `createRequire` still resolves the
	// specifier from a CJS context, which is what makes this a CJS-host test.
	it('a CommonJS host can load the package', async () => {
		const req = createRequire(loader_path);
		const resolved = req.resolve('@fuzdev/tsv');
		const from_cjs = await import(pathToFileURL(resolved).href);
		assert.equal(from_cjs.format_typescript('const   x=1'), 'const x = 1;\n');
	});

	// The ESM half of the same claim. The CJS test above resolves through
	// `require.resolve`, which walks the `require` condition; an `import` walks
	// its own. Nothing else in this suite reaches the package by its NAME —
	// every other test opens a staged file by path, which bypasses the exports
	// map entirely — so this is the only place the `.` entry is resolved the way
	// a consumer's `import '@fuzdev/tsv'` resolves it, from a cwd inside the
	// staged install. The second half is the encapsulation: a file the map does
	// not name stays unreachable.
	it('an ESM host resolves the bare specifier, and only the named subpaths', () => {
		const probe = spawnSync(
			process.execPath,
			[
				'--input-type=module',
				'--eval',
				`const tsv = await import('@fuzdev/tsv');
const unexported = await import('@fuzdev/tsv/cli.js').then(() => 'resolved', (error) => error.code);
process.stdout.write(JSON.stringify({out: tsv.format_typescript('const   x=1'), unexported}));`
			],
			{ cwd: staged, encoding: 'utf-8' }
		);
		assert.equal(probe.status, 0, probe.stderr);
		assert.deepEqual(JSON.parse(probe.stdout), {
			out: 'const x = 1;\n',
			unexported: 'ERR_PACKAGE_PATH_NOT_EXPORTED'
		});
	});

	it('package.json selection fields and pins are coherent', () => {
		const loader_pkg = JSON.parse(
			readFileSync(join(staged, 'node_modules', '@fuzdev', 'tsv', 'package.json'), 'utf8')
		);
		const platform_pkg = JSON.parse(
			readFileSync(join(staged, 'node_modules', '@fuzdev', `tsv-${triple}`, 'package.json'), 'utf8')
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
		// The native CLI binary ships beside the addon, executable — what the
		// loader's bin.js execs. Every declared platform file actually shipped.
		assert.ok(platform_pkg.files.includes(cli_binary_name), `files declares ${cli_binary_name}`);
		for (const file of platform_pkg.files) {
			const path = join(staged, 'node_modules', '@fuzdev', `tsv-${triple}`, file);
			assert.ok(existsSync(path), `declared platform file missing: ${file}`);
		}
		if (process.platform !== 'win32') {
			const mode = statSync(
				join(staged, 'node_modules', '@fuzdev', `tsv-${triple}`, cli_binary_name)
			).mode;
			assert.ok(mode & 0o111, 'the CLI binary must be executable');
		}
		// The loader's SUPPORTED list and its optionalDependencies must agree —
		// the build script and index.js each carry the list, and this is the
		// gate that keeps them in sync.
		const source = readFileSync(loader_path, 'utf8');
		const supported = [...source.matchAll(/^\t'([a-z0-9]+-[a-z0-9-]+)'/gm)].map((m) => m[1]);
		assert.deepEqual(
			supported.map((t) => `@fuzdev/tsv-${t}`).sort(),
			Object.keys(loader_pkg.optionalDependencies).sort(),
			'npm/index.js SUPPORTED must match the generated optionalDependencies'
		);
		// Every `files` entry the loader package declares actually shipped.
		for (const file of loader_pkg.files) {
			assert.ok(
				existsSync(join(staged, 'node_modules', '@fuzdev', 'tsv', file)),
				`declared file missing: ${file}`
			);
		}
	});

	// The same rule `scripts/test_npm.ts` pins over the wasm packages, over this
	// package's hand-written declarations. Under `moduleResolution:
	// node16`/`nodenext` an extensionless relative specifier inside a `.d.ts` is
	// TS2834/TS2835 raised from inside the package, at every consumer without
	// `skipLibCheck` — and nothing in-repo type-checks the merged package
	// `.d.ts`, so it is invisible until someone else compiles.
	it('every .d.ts relative specifier carries the .js extension', () => {
		const loader_dir = join(staged, 'node_modules', '@fuzdev', 'tsv');
		const loader_pkg = JSON.parse(readFileSync(join(loader_dir, 'package.json'), 'utf8'));
		const bad: Array<string> = [];
		for (const rel of loader_pkg.files.filter((f: string) => f.endsWith('.d.ts'))) {
			const source = readFileSync(join(loader_dir, rel), 'utf8');
			for (const [, spec] of source.matchAll(/(?:from|import\()\s*['"](\.[^'"]*)['"]/g)) {
				if (!/\.(?:js|mjs|cjs|json)$/.test(spec)) bad.push(`${rel}: ${spec}`);
			}
		}
		assert.deepEqual(
			bad,
			[],
			`extensionless relative specifiers in shipped .d.ts:\n${bad.join('\n')}`
		);
	});

	// This host IS a prebuilt platform, so the bare staging exercises the
	// "supported but not installed" arm (a lockfile from another OS,
	// --omit=optional): the message must name the missing package as the remedy,
	// not list the user's own platform as prebuilt and tell them to switch engines.
	// The genuinely-unsupported arm can't be reached from a supported host.
	it('a missing platform package fails loudly, naming the package to install', async () => {
		const bare_loader = join(staged_bare, 'node_modules', '@fuzdev', 'tsv', 'index.js');
		await assert.rejects(import(pathToFileURL(bare_loader).href), (e: unknown) => {
			assert.ok(e instanceof Error);
			assert.ok(e.message.includes(triple), `message names the triple: ${e.message}`);
			assert.ok(
				e.message.includes(`npm i @fuzdev/tsv-${triple}`),
				`message names the install remedy: ${e.message}`
			);
			assert.ok(
				e.message.includes('@fuzdev/tsv_wasm'),
				`message points at the WASM fallback: ${e.message}`
			);
			return true;
		});
	});
});

// Libc detection — the one piece of the loader that decides which package to
// resolve before anything is loaded, and the one that cannot be exercised on
// the host that runs this suite: every question it asks is about a machine
// this is not. `is_musl` therefore takes its three probes as a bag, so the
// DECISION can be driven over hosts the CI matrix will never have (an Alpine
// container, a Debian box with the `musl` package installed, a runtime whose
// `process.report` is absent or partial).
describe('libc detection (platform.js)', () => {
	/** A stub probe bag that records which probes a verdict actually asked. */
	const probe_bag = (facts: {
		musl_loader: boolean;
		mapped_libc?: string;
		reported_glibc?: string;
	}) => {
		const asked: Array<string> = [];
		return {
			asked,
			probes: {
				musl_loader: () => {
					asked.push('musl_loader');
					return facts.musl_loader;
				},
				mapped_libc: () => {
					asked.push('mapped_libc');
					return facts.mapped_libc;
				},
				reported_glibc: () => {
					asked.push('reported_glibc');
					return facts.reported_glibc;
				}
			}
		};
	};

	it('the shipped module exports the detection', () => {
		assert.equal(typeof is_musl, 'function');
		assert.equal(typeof platform_triple, 'function');
	});

	// A stock glibc system: one readdir, and nothing else is paid for. The
	// ordering IS the contract — the two probes below it cost ~2x and ~20x —
	// so the assertion is on what was asked, not only on the verdict.
	it('no musl loader ends it: gnu, with no further probe', () => {
		const { asked, probes } = probe_bag({ musl_loader: false });
		assert.equal(is_musl(probes), false);
		assert.deepEqual(asked, ['musl_loader']);
	});

	it('a musl loader with musl mapped: musl, without consulting the report', () => {
		const { asked, probes } = probe_bag({ musl_loader: true, mapped_libc: 'musl' });
		assert.equal(is_musl(probes), true);
		assert.deepEqual(asked, ['musl_loader', 'mapped_libc']);
	});

	// The case the loader used to get wrong whenever the report was silent: a
	// glibc host that merely has musl INSTALLED carries the loader, so only the
	// mapped libc separates it from Alpine.
	it('a musl loader with glibc mapped: gnu (musl installed on a glibc host)', () => {
		const { asked, probes } = probe_bag({ musl_loader: true, mapped_libc: 'gnu' });
		assert.equal(is_musl(probes), false);
		assert.deepEqual(asked, ['musl_loader', 'mapped_libc']);
	});

	// Map unreadable (no `/proc`, or a statically linked host binary): the
	// report is the last word, and it is trusted only positively.
	it('an unreadable map falls back to the report naming a glibc runtime: gnu', () => {
		const { asked, probes } = probe_bag({ musl_loader: true, reported_glibc: '2.41' });
		assert.equal(is_musl(probes), false);
		assert.deepEqual(asked, ['musl_loader', 'mapped_libc', 'reported_glibc']);
	});

	it('an unreadable map and a silent report: musl (the loader stands)', () => {
		const { probes } = probe_bag({ musl_loader: true });
		assert.equal(is_musl(probes), true);
	});

	// The real host, against the triple the BUILD script detected (Deno-side)
	// and staged the platform package under. The loader answering differently
	// is how a `require('@fuzdev/tsv-<triple>')` misses a package that installed
	// correctly.
	it('the detected triple matches the staged platform package', () => {
		assert.equal(platform_triple(), triple);
	});
});

// The `tsv` bin — `bin.js`, the dispatcher that execs the platform package's
// native `tsv_cli` binary, with the shared `cli.js` (the wasm package's bin,
// staged here bound to the native loader) as its fallback. The full
// flag/exit-code matrix lives in `scripts/test_npm.ts` over the wasm copy and
// in `tests/cli_tests.rs` over the binary itself; what THIS suite pins is the
// dispatch — that the bin really reaches the native binary — and the
// forwarding: exit codes, stdout/stderr split, stdin piping, in-place writes.
const bin_path = join(staged, 'node_modules', '@fuzdev', 'tsv', 'bin.js');
const cli_path = join(staged, 'node_modules', '@fuzdev', 'tsv', 'cli.js');
/** The real `tsv_cli` binary the platform package ships — what `bin.js` execs. */
const native_path = join(staged, 'node_modules', '@fuzdev', `tsv-${triple}`, cli_binary_name);
/** Through the `tsv` bin — the `npx tsv` path, and this suite's subject. */
const run_cli = (args: Array<string>, stdin?: string) =>
	spawnSync(process.execPath, [bin_path, ...args], { encoding: 'utf-8', input: stdin });
/**
 * The native binary DIRECTLY, for the parity suites below.
 *
 * Not through `bin.js`, which would be one hop with a fallback in it: the
 * dispatcher degrades to `cli.js` when the binary is missing or unrunnable, so
 * a parity suite reading the "native" side through it would compare `cli.js`
 * to `cli.js` and pass every row while measuring nothing. (Verified: pointing
 * `run_cli` at `cli_path` leaves all 87 parity assertions green.) Something
 * else pins the dispatch — this leaves the parity verdicts with nothing to
 * degrade into.
 */
const run_native = (args: Array<string>, input: string | Buffer = '', cwd?: string) =>
	spawnSync(native_path, args, { encoding: 'utf-8', input, cwd });

describe('cli (bin.js): the tsv bin dispatching to the native CLI binary', () => {
	it('is wired as the package bin', () => {
		const pkg = JSON.parse(readFileSync(join(dirname(bin_path), 'package.json'), 'utf8'));
		assert.deepEqual(pkg.bin, { tsv: 'bin.js' });
	});

	// The dispatch discriminator: argh's generated help carries a section
	// header (`Positional Arguments:`) the JS mirror's hand-written help never
	// prints — so this output can only have come from the native binary.
	it('dispatches to the native binary, not the JS loop', () => {
		const result = run_cli(['help', 'format']);
		assert.equal(result.status, 0, result.stderr);
		assert.match(result.stdout, /Positional Arguments:/);
	});

	// A real lockstep gate: the binary's compiled-in version (workspace
	// Cargo.toml) must equal the staged package version (read from the same
	// Cargo.toml at stage time) — a stale binary from an older checkout
	// staged into a fresh package fails here.
	it('--version reports the native binary version, in lockstep with the package', () => {
		const result = run_cli(['--version']);
		assert.equal(result.status, 0, result.stderr);
		const loader_pkg = JSON.parse(readFileSync(join(dirname(bin_path), 'package.json'), 'utf8'));
		assert.equal(result.stdout, `tsv ${loader_pkg.version}\n`);
	});

	// What npm would actually publish, on every platform — because it is the
	// one claim the package.json check above can't make. That one proves the
	// `files` array names this platform's binary and that the file is on disk;
	// this one proves npm's own packing rules then put it in the TARBALL, which
	// is what a consumer installs. Without it, `npx tsv` could silently degrade
	// to the JS mirror on a platform whose staging looked perfect.
	// The execute bit is posix-only: npm packs the on-disk mode and only chmods
	// `bin` entries at install, so a 644 there ships a broken npx — on Windows
	// there is no mode to pin. (`npm` is a .cmd there, hence the shell.)
	it('npm pack ships the CLI binary', () => {
		const result = spawnSync('npm', ['pack', '--dry-run', '--json'], {
			cwd: join(pkg_root, triple),
			encoding: 'utf-8',
			shell: process.platform === 'win32'
		});
		assert.equal(result.status, 0, result.stderr);
		const [report] = JSON.parse(result.stdout);
		const entry = report.files.find((f: { path: string }) => f.path === cli_binary_name);
		assert.ok(entry, `${cli_binary_name} missing from the packed file list`);
		if (posix) {
			assert.ok(entry.mode & 0o111, `packed mode ${entry.mode.toString(8)} is not executable`);
		}
	});

	it('format --content prints formatted source', () => {
		const result = run_cli(['format', '--content', 'const   x=1', '--parser', 'ts']);
		assert.equal(result.status, 0, result.stderr);
		assert.equal(result.stdout, 'const x = 1;\n');
	});

	it('format --stdin reads stdin', () => {
		const result = run_cli(['format', '--stdin', '--parser', 'css'], 'a{color:red}');
		assert.equal(result.status, 0, result.stderr);
		assert.equal(result.stdout, 'a {\n\tcolor: red;\n}\n');
	});

	it('format --check --content exits 1 on would-change, 0 on clean', () => {
		assert.equal(
			run_cli(['format', '--check', '--content', 'const   x=1', '--parser', 'ts']).status,
			1
		);
		assert.equal(
			run_cli(['format', '--check', '--content', 'const x = 1;\n', '--parser', 'ts']).status,
			0
		);
	});

	it('format on invalid syntax exits 2', () => {
		const result = run_cli(['format', '--content', 'const =', '--parser', 'ts']);
		assert.equal(result.status, 2);
		assert.match(result.stderr, /Parse error/);
	});

	it('a bad --parser value exits 1 in both commands (argument-parsing error)', () => {
		for (const command of ['format', 'parse']) {
			const result = run_cli([command, '--content', 'const x = 1;', '--parser', 'bogus']);
			assert.equal(result.status, 1);
			assert.match(result.stderr, /Unknown parser type/);
		}
	});

	it('parse --content emits the wire; --no-locations omits loc', () => {
		const full = run_cli(['parse', '--content', 'const x = 1;', '--parser', 'ts']);
		assert.equal(full.status, 0, full.stderr);
		assert.match(full.stdout, /"loc"/);
		assert.equal(JSON.parse(full.stdout).type, 'Program');
		const bare = run_cli([
			'parse',
			'--no-locations',
			'--content',
			'const x = 1;',
			'--parser',
			'ts'
		]);
		assert.equal(bare.status, 0, bare.stderr);
		assert.doesNotMatch(bare.stdout, /"loc"/);
	});

	it('path mode formats in place, --jobs forwarded (real parallelism here)', () => {
		const root = mkdtempSync(join(tmpdir(), 'tsv-napi-cli-'));
		try {
			writeFileSync(join(root, 'a.ts'), 'const   a=1\n');
			writeFileSync(join(root, 'b.ts'), 'const b = 1;\n');
			const result = run_cli(['format', '--jobs', '4', root]);
			assert.equal(result.status, 0, result.stderr);
			assert.equal(result.stdout, `${join(root, 'a.ts')}\n`);
			assert.equal(readFileSync(join(root, 'a.ts'), 'utf8'), 'const a = 1;\n');
			assert.match(result.stderr, /1 formatted, 1 unchanged/);
		} finally {
			rmSync(root, { recursive: true, force: true });
		}
	});

	it('--jobs with --content exits 2 (native-CLI parity)', () => {
		const result = run_cli([
			'format',
			'--jobs',
			'2',
			'--content',
			'const x = 1;',
			'--parser',
			'ts'
		]);
		assert.equal(result.status, 2);
	});

	it('help exits 0 (argh)', () => {
		const result = run_cli(['help', 'format']);
		assert.equal(result.status, 0);
		assert.match(result.stdout, /Usage: tsv format/);
	});
});

// With the CLI binary removed from the platform package, bin.js must degrade
// to the shared cli.js — the JS mirror of the same contract over the native
// engine — rather than fail. The help probe inverts: the JS mirror's
// hand-written help has no argh section header.
const fallback_bin = join(staged_no_binary, 'node_modules', '@fuzdev', 'tsv', 'bin.js');
const run_fallback = (args: Array<string>, stdin?: string) =>
	spawnSync(process.execPath, [fallback_bin, ...args], { encoding: 'utf-8', input: stdin });

describe('cli (bin.js): fallback to the JS mirror without the binary', () => {
	it('serves the format contract through cli.js', () => {
		const result = run_fallback(['format', '--content', 'const   x=1', '--parser', 'ts']);
		assert.equal(result.status, 0, result.stderr);
		assert.equal(result.stdout, 'const x = 1;\n');
	});

	it('runs the JS loop, not the binary', () => {
		const result = run_fallback(['help', 'format']);
		assert.equal(result.status, 0);
		assert.match(result.stdout, /Usage: tsv format/);
		assert.doesNotMatch(result.stdout, /Positional Arguments:/);
	});

	it('forwards the JS mirror exit codes', () => {
		assert.equal(
			run_fallback(['format', '--check', '--content', 'const   x=1', '--parser', 'ts']).status,
			1
		);
		assert.equal(run_fallback(['format', '--content', 'const =', '--parser', 'ts']).status, 2);
	});

	// The mirror's worker pool over the NATIVE engine. Its wasm sibling gets the
	// main thread's compiled module handed across; this package exports none, so
	// its workers load the addon themselves — a different engine-binding path
	// through the same pool, and the only place it runs.
	//
	// The tree is sized from the shipped source, not a literal, for the reason
	// `scripts/test_npm.ts` spells out: parallel and sequential are DESIGNED to
	// be indistinguishable from outside, so a tree that fell under a raised
	// threshold would keep passing while exercising nothing. This copy reads
	// NATIVE_WORKER_FILE_THRESHOLD — the two engines size their pools
	// differently (there is no wasm tier-up competing for cores here), so each
	// suite must read its own engine's constant or one of them goes vacuous the
	// next time the other moves.
	// The mirror is ONE source staged into two packages, and that is now
	// load-bearing rather than tidy: cli.js carries an engine discriminant and a
	// pair of engine-specific pool constants, so a staging step that ever
	// transformed the file would break the branch this package depends on —
	// invisibly, because each suite otherwise only ever reads its own copy.
	it('is the shared cli.js verbatim, not a transformed copy', () => {
		assert.equal(
			readFileSync(join(dirname(fallback_bin), 'cli.js'), 'utf8'),
			readFileSync(new URL('../crates/tsv_wasm/npm/cli.js', import.meta.url), 'utf8'),
			'the staged cli.js differs from crates/tsv_wasm/npm/cli.js — the two packages must ship one source'
		);
	});

	it('fans a worker-sized tree onto threads, matching --jobs 1 exactly', () => {
		const source = readFileSync(join(dirname(fallback_bin), 'cli.js'), 'utf8');
		const match = /const NATIVE_WORKER_FILE_THRESHOLD = (\d+);/.exec(source);
		assert.ok(
			match,
			'could not read NATIVE_WORKER_FILE_THRESHOLD out of cli.js — did it get renamed?'
		);
		// even, so the half-unformatted split below lands on a whole number
		const count = 2 * Math.ceil((Number(match[1]) + 1) / 2);
		const dir = mkdtempSync(join(tmpdir(), 'tsv-napi-jobs-'));
		try {
			for (let i = 0; i < count; i++) {
				writeFileSync(
					join(dir, `f${i}.ts`),
					i % 2 === 0 ? `const  a${i}=${i}` : `const a${i} = ${i};\n`
				);
			}
			const one = run_fallback(['format', '--check', '--jobs', '1', dir]);
			const many = run_fallback(['format', '--check', dir]);
			assert.equal(one.status, 1, one.stderr);
			assert.equal(many.status, one.status);
			assert.equal(many.stdout, one.stdout);
			assert.equal(many.stderr, one.stderr);
			const half = count / 2;
			assert.match(many.stderr, new RegExp(`^${half} would change, ${half} unchanged$`, 'm'));
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});
});

// Flag-set parity between the two CLIs that serve the same contract. This is
// the only place both exist at once (the wasm package ships no binary), and the
// mirror's flag table and help text are hand-written — so a flag added to argh
// and not to `cli.js`, or left in the mirror after the native CLI dropped it,
// drifts silently: every other test here drives flags it names itself, and so
// only ever covers the intersection both sides already agree on.
//
// The claim is RECOGNITION, not behavior (each flag's semantics are pinned by
// the matrix in `scripts/test_npm.ts` and `tests/cli_tests.rs`): each side is
// handed the bare flag and must not answer with its unknown-flag error. A
// missing value or missing input is a different error and passes — that is the
// point, since it means the flag was understood. Scope is the two commands;
// the top-level `--version` / `--help` have their own tests on both sides.

/** The `--flag`s a help text advertises. Two vacuity checks come with it: a
 * regex that quietly matched nothing would make every assertion below pass. */
const advertised_flags = (help: string, source: string): Array<string> => {
	const options = help.slice(help.indexOf('\nOptions:'));
	const flags = [...options.matchAll(/^ {2}(--[a-z0-9-]+)/gm)]
		.map((m) => m[1])
		// argh generates `--help` for every command; the mirror handles it
		// without advertising it, so it is the one flag the sets can't share.
		.filter((flag) => flag !== '--help');
	assert.ok(flags.length >= 5, `parsed too few flags from ${source}: ${flags}`);
	assert.ok(flags.includes('--parser'), `parsed no --parser from ${source}: ${flags}`);
	return flags;
};

const run_mirror = (args: Array<string>, input: string | Buffer = '', cwd?: string) =>
	spawnSync(process.execPath, [cli_path, ...args], { encoding: 'utf-8', input, cwd });

// The three parity suites below all compare `run_native` against `run_mirror`,
// and a faithful mirror compared against ITSELF passes every row — so a runner
// quietly pointing at the wrong bin would leave 38 green assertions measuring
// nothing. `run_native` spawns the binary with no dispatcher in between, which
// removes the way that happens by accident; this pins the rest. argh's
// generated help prints a `Positional Arguments:` section header that the
// mirror's hand-written help never does, so one probe separates the two bins
// in both directions.
describe('parity anchor: the two bins under comparison really are the two bins', () => {
	it('run_native reaches argh, and run_mirror does not', () => {
		const native = run_native(['help', 'format']);
		assert.equal(native.status, 0, native.stderr);
		assert.match(
			native.stdout,
			/Positional Arguments:/,
			'the native runner is not reaching the real binary — every parity row below is vacuous'
		);
		const mirror = run_mirror(['help', 'format']);
		assert.equal(mirror.status, 0, mirror.stderr);
		assert.doesNotMatch(
			mirror.stdout,
			/Positional Arguments:/,
			'the mirror runner is reaching the native binary — every parity row below is vacuous'
		);
	});
});

describe('flag parity: the native CLI and cli.js recognize the same flags', () => {
	for (const command of ['format', 'parse']) {
		it(`${command}: every flag the native CLI advertises, cli.js recognizes`, () => {
			const help = run_native(['help', command]);
			assert.equal(help.status, 0, help.stderr);
			const flags = advertised_flags(help.stdout, `argh's \`${command}\` help`);
			for (const flag of flags) {
				const result = run_mirror([command, flag]);
				assert.doesNotMatch(
					result.stderr,
					/Unknown option/,
					`cli.js does not know \`${command} ${flag}\`, which the native CLI advertises`
				);
			}
		});

		it(`${command}: every flag cli.js advertises, the native CLI recognizes`, () => {
			const help = run_mirror(['help', command]);
			assert.equal(help.status, 0, help.stderr);
			const flags = advertised_flags(help.stdout, `cli.js's \`${command}\` help`);
			for (const flag of flags) {
				const result = run_native([command, flag]);
				assert.doesNotMatch(
					result.stderr,
					/Unrecognized argument/,
					`the native CLI does not know \`${command} ${flag}\`, which cli.js advertises`
				);
			}
		});
	}
});

// Message PRECEDENCE parity. The suite above pins that both CLIs RECOGNIZE the
// same flags; this pins what they SAY when several are wrong at once. Order is
// real contract: each command validates in a fixed sequence, so a doubly-bad
// invocation has exactly one right answer, and the mirror re-states that
// sequence by hand — the shape that drifts silently, since every other test
// here drives one fault at a time and so only ever sees the winner it already
// expected.
//
// Every row is a usage error that lands BEFORE any file is touched, so the
// table needs no fixture tree; `x.ts` is a name, never a file. Three claims per
// row: the two bins exit alike, on the code the row names; their stderr is
// byte-identical; and it is still the message the row is about (without which a
// row whose case stopped being reachable would pass on two identical
// somethings). The rows deliberately stop where the messages stop being tsv's
// own — argh and `parseArgs` word their own failures, and those agree on the
// exit code alone.
const USAGE_ROWS: Array<{ args: Array<string>; exit: number; says: string }> = [
	// `format` — the single-input mode, in its validation order
	{ args: ['format'], exit: 2, says: 'No input provided' },
	{ args: ['format', '--content', 'x'], exit: 2, says: '--content requires --parser' },
	{
		args: ['format', '--stdin', '--parser', 'ts', 'x.ts'],
		exit: 2,
		says: '--content/--stdin cannot be combined with file paths'
	},
	{
		args: ['format', '--content', 'x', '--parser', 'ts', '--jobs', '2'],
		exit: 2,
		says: '--jobs applies to file paths'
	},
	{
		args: ['format', '--content', 'x', '--parser', 'ts', '--list'],
		exit: 2,
		says: '--list applies to file paths'
	},
	{
		args: ['format', '--content', 'x', '--parser', 'ts', '--source-type', 'bogus'],
		exit: 2,
		says: "invalid --source-type 'bogus'"
	},
	{
		args: ['format', '--content', 'a{color:red}', '--parser', 'css', '--source-type', 'script'],
		exit: 2,
		says: '--source-type is only supported for typescript'
	},
	// `format` — path mode
	{ args: ['format', '--parser', 'ts', 'x.ts'], exit: 2, says: '--parser applies to' },
	{
		args: ['format', '--source-type', 'script', 'x.ts'],
		exit: 2,
		says: '--source-type applies to'
	},
	{ args: ['format', '--list', '--check', 'x.ts'], exit: 2, says: '--list and --check' },
	// PRECEDENCE — each row is faulty in two or more ways, and names the winner
	{
		args: ['format', '--content', 'x', '--parser', 'ts', '--jobs', '2', '--list', 'x.ts'],
		exit: 2,
		says: '--content/--stdin cannot be combined with file paths'
	},
	{
		args: ['format', '--content', 'x', '--parser', 'ts', '--jobs', '2', '--list'],
		exit: 2,
		says: '--jobs applies to file paths'
	},
	{
		// the mode refusals precede the source type, which precedes the parser
		args: ['format', '--content', 'x', '--list', '--source-type', 'bogus'],
		exit: 2,
		says: '--list applies to file paths'
	},
	{
		args: ['format', '--content', 'x', '--source-type', 'bogus'],
		exit: 2,
		says: "invalid --source-type 'bogus'"
	},
	{
		// a bad VALUE outranks the language that has no source type at all
		args: ['format', '--content', 'x', '--parser', 'css', '--source-type', 'bogus'],
		exit: 2,
		says: "invalid --source-type 'bogus'"
	},
	{
		args: ['format', '--parser', 'ts', '--source-type', 'script', 'x.ts'],
		exit: 2,
		says: '--parser applies to'
	},
	{
		args: ['format', '--parser', 'ts', '--list', '--check', 'x.ts'],
		exit: 2,
		says: '--parser applies to'
	},
	{
		args: ['format', '--source-type', 'script', '--list', '--check', 'x.ts'],
		exit: 2,
		says: '--source-type applies to'
	},
	// `parse` — the same questions, answered with its own exit code
	{ args: ['parse'], exit: 1, says: 'No input provided' },
	{ args: ['parse', '--content', 'x'], exit: 1, says: '--content requires --parser' },
	{
		args: ['parse', '--content', 'x', '--source-type', 'bogus'],
		exit: 1,
		says: "invalid --source-type 'bogus'"
	},
	{
		args: ['parse', '--content', 'x', '--parser', 'css', '--source-type', 'bogus'],
		exit: 1,
		says: "invalid --source-type 'bogus'"
	},
	// argh's own grammar, restated by the mirror ahead of `parseArgs` (`parse_argv`):
	// no inline values, no short flags, a value-taking flag with nothing after it,
	// and the extra positional refused before any value is looked at
	{
		args: ['format', '--content=x', '--parser', 'ts'],
		exit: 1,
		says: 'Unrecognized argument: --content=x'
	},
	{ args: ['format', '--check=1', 'x.ts'], exit: 1, says: 'Unrecognized argument: --check=1' },
	{ args: ['format', '-x', 'x.ts'], exit: 1, says: 'Unrecognized argument: -x' },
	{ args: ['format', '-', 'x.ts'], exit: 1, says: 'Unrecognized argument: -' },
	{ args: ['format', '--content'], exit: 1, says: "No value provided for option '--content'." },
	{ args: ['help', 'bogus'], exit: 1, says: 'Unrecognized argument: bogus' },
	{
		args: ['parse', 'a.ts', 'b.ts', '--source-type', 'bogus'],
		exit: 1,
		says: 'Unrecognized argument: b.ts'
	}
];

/** Rows that SUCCEED on both bins with byte-identical stdout: the word `help` in a
 * subcommand's argv prints that subcommand's help (argh's help word, which the
 * mirror once took for a path and formatted a directory named `help`), and a
 * value-taking flag takes the next word verbatim even when it is flag-shaped. */
const STDOUT_ROWS: Array<{ args: Array<string>; starts: string }> = [
	{ args: ['format', 'help'], starts: 'Usage: tsv format' },
	{ args: ['format', '--check', 'help', 'extra'], starts: 'Usage: tsv format' },
	{ args: ['parse', 'help'], starts: 'Usage: tsv parse' },
	{ args: ['format', '--content', '--check', '--parser', 'ts'], starts: '--check;\n' }
];

describe('message parity: the native CLI and cli.js refuse in the same order', () => {
	for (const { args, exit, says } of USAGE_ROWS) {
		it(`${args.join(' ')} → exit ${exit}, ${says}`, () => {
			const native = run_native(args);
			const mirror = run_mirror(args);
			assert.equal(native.status, exit, `native stderr: ${native.stderr}`);
			assert.equal(mirror.status, exit, `cli.js stderr: ${mirror.stderr}`);
			assert.equal(mirror.stderr, native.stderr, 'the two bins must word this refusal alike');
			// A plain substring, not a regex: every `says` is literal text, and
			// hand-escaping it for `assert.match` only invents a way to get the
			// escape set wrong.
			assert.ok(
				native.stderr.includes(says),
				`the row's own message is gone; both bins now say: ${native.stderr}`
			);
		});
	}

	for (const { args, starts } of STDOUT_ROWS) {
		it(`${args.join(' ')} → exit 0, stdout ${JSON.stringify(starts)}`, () => {
			// a directory named `help` holding an unformatted file: the row is also
			// the proof that neither bin formats it
			const dir = mkdtempSync(join(tmpdir(), 'tsv-help-word-'));
			try {
				mkdirSync(join(dir, 'help'));
				writeFileSync(join(dir, 'help', 'c.ts'), 'const  z=1\n');
				const native = run_native(args, '', dir);
				const mirror = run_mirror(args, '', dir);
				assert.equal(native.status, 0, `native stderr: ${native.stderr}`);
				assert.equal(mirror.status, 0, `cli.js stderr: ${mirror.stderr}`);
				assert.ok(native.stdout.startsWith(starts), `native stdout: ${native.stdout}`);
				assert.ok(mirror.stdout.startsWith(starts), `cli.js stdout: ${mirror.stdout}`);
				assert.equal(readFileSync(join(dir, 'help', 'c.ts'), 'utf-8'), 'const  z=1\n');
			} finally {
				rmSync(dir, { recursive: true, force: true });
			}
		});
	}

	// `--pretty` is a re-serialization on the mirror (`JSON.stringify(JSON.parse(…))`)
	// and a byte re-indent natively, and the two are held byte-identical — which
	// rests on three writer facts (ECMAScript number spelling, no integer-like keys,
	// serde's escape set), so it is pinned rather than argued. The sample carries the
	// tokens that would part them: a large integer, a float at the exponent switch,
	// a tiny float, a lone surrogate, U+2028, an astral char, and nesting.
	it('parse --pretty is byte-identical on both bins', () => {
		const sample =
			'const a = [9007199254740993, 1e21, 1e-7, 0.000001, "\\ud800", "\u2028", "😀", [[[{}]]]];\n';
		const args = ['parse', '--pretty', '--content', sample, '--parser', 'typescript'];
		const native = run_native(args);
		const mirror = run_mirror(args);
		assert.equal(native.status, 0, native.stderr);
		assert.equal(mirror.status, 0, mirror.stderr);
		assert.ok(native.stdout.includes('\t"type": "Program"'), 'the pretty form is tab-indented');
		assert.equal(mirror.stdout, native.stdout);
	});

	// The mirror's `format` help hand-restates the extension list (its help text is a
	// literal, as argh's is), so it is held against the list the binding renders from
	// `tsv_discover::FORMATTABLE_EXTENSIONS` — the same const the native help is pinned
	// to by `tests/cli_tests.rs`. A ninth language then cannot ship a JS help naming
	// eight.
	it('the mirror help names exactly the extensions the binding formats', () => {
		const mirror = run_mirror(['help', 'format']);
		assert.equal(mirror.status, 0, mirror.stderr);
		const empty = mkdtempSync(join(tmpdir(), 'tsv-help-ext-'));
		try {
			// the binding's list, rendered `/`-joined by the nothing-in-scope refusal
			const refused = run_mirror(['format', empty]);
			const match = /no unignored (\S+) files in scope/.exec(refused.stderr);
			assert.ok(match, refused.stderr);
			assert.ok(
				mirror.stdout.includes(match[1]),
				`help says ${JSON.stringify(mirror.stdout)}, the binding formats ${match[1]}`
			);
		} finally {
			rmSync(empty, { recursive: true, force: true });
		}
	});

	// The nothing-in-scope refusal, which needs a real empty directory and so cannot
	// be a `USAGE_ROWS` entry. It is the one refusal whose text is *derived* on the
	// native side — rendered from `tsv_discover::FORMATTABLE_EXTENSIONS`, the const the
	// discovery filter itself reads — while `cli.js` restates the finished sentence by
	// hand (as it does `clamp_worker_count` and `Goal::from_extension`). So this row is
	// what makes a ninth formattable extension fail here instead of shipping two bins
	// that name different sets.
	it(`format <empty dir> → exit 2, both bins name the same extension set`, () => {
		const args = ['format', empty_dir];
		const native = run_native(args);
		const mirror = run_mirror(args);
		assert.equal(native.status, 2, `native stderr: ${native.stderr}`);
		assert.equal(mirror.status, 2, `cli.js stderr: ${mirror.stderr}`);
		assert.equal(mirror.stderr, native.stderr, 'the two bins must word this refusal alike');
		assert.ok(
			native.stderr.includes('No files to format'),
			`the row's own message is gone; both bins now say: ${native.stderr}`
		);
	});
});

// Invalid UTF-8. The native CLI reads every file and stdin with Rust's
// `read_to_string`, which REFUSES invalid bytes; Node's
// `readFileSync(path, 'utf-8')` substitutes U+FFFD and returns a string that
// looks fine. On the format path that is not a wrong message but DATA LOSS:
// a lone stray byte inside a string literal still parses after the
// substitution, so the mirror used to write the repaired text back over the
// author's file and report `1 formatted`, exit 0, where the native CLI
// refuses and leaves the bytes alone. `cli.js` now decodes strictly
// (`decode_source`), throwing Rust's own wording so the refusals read alike.
//
// The file-untouched assertion is the one that matters: an exit code can be
// argued about, a rewritten byte cannot.
describe('invalid UTF-8 parity: both CLIs refuse, and neither rewrites the file', () => {
	/** Valid TypeScript except for one invalid UTF-8 byte inside a string —
	 * the shape that survives a lossy decode and therefore gets written back. */
	const bad_bytes = Buffer.concat([
		Buffer.from("const   s = '"),
		Buffer.from([0xff]),
		Buffer.from("hi';\n")
	]);

	const write_case = (name: string): string => {
		const file = join(utf8_dir, name);
		writeFileSync(file, bad_bytes);
		return file;
	};

	it('format <path> refuses on both, and leaves every byte in place', () => {
		const native_file = write_case('native.ts');
		const mirror_file = write_case('mirror.ts');
		const native = run_native(['format', native_file]);
		const mirror = run_mirror(['format', mirror_file]);
		assert.equal(native.status, 2, `native: ${native.stderr}`);
		assert.equal(mirror.status, 2, `cli.js formatted a file it could not read: ${mirror.stderr}`);
		assert.ok(
			readFileSync(native_file).equals(bad_bytes),
			'the native CLI rewrote a file it refused'
		);
		assert.ok(
			readFileSync(mirror_file).equals(bad_bytes),
			'cli.js rewrote the invalid bytes — U+FFFD substitution reached the disk'
		);
		// same refusal, modulo the two different filenames
		const strip = (text: string, file: string) => text.split(file).join('<path>');
		assert.equal(strip(mirror.stderr, mirror_file), strip(native.stderr, native_file));
		assert.match(native.stderr, /read failed: stream did not contain valid UTF-8/);
	});

	it('parse <path> refuses on both rather than emitting a mangled AST', () => {
		const native_file = write_case('native_parse.ts');
		const mirror_file = write_case('mirror_parse.ts');
		const native = run_native(['parse', native_file]);
		const mirror = run_mirror(['parse', mirror_file]);
		assert.equal(native.status, 1, `native: ${native.stderr}`);
		assert.equal(
			mirror.status,
			1,
			`cli.js parsed undecodable bytes: ${mirror.stdout.slice(0, 200)}`
		);
		assert.equal(mirror.stdout, '');
		assert.match(native.stderr, /stream did not contain valid UTF-8/);
	});

	for (const [command, code] of [
		['format', 2],
		['parse', 1]
	] as Array<[string, number]>) {
		it(`${command} --stdin refuses on both, with the same message and exit ${code}`, () => {
			const args = [command, '--stdin', '--parser', 'ts'];
			const native = run_native(args, bad_bytes);
			const mirror = run_mirror(args, bad_bytes);
			assert.equal(native.status, code, `native: ${native.stderr}`);
			assert.equal(mirror.status, native.status, `cli.js: ${mirror.stderr || mirror.stdout}`);
			assert.equal(mirror.stderr, native.stderr, 'the two bins must word this refusal alike');
			assert.match(native.stderr, /Error reading from stdin: stream did not contain valid UTF-8/);
		});
	}
});

// Repeated value-taking options. argh refuses a second `--content`/`--parser`/
// `--source-type`/`--jobs` ("duplicate values provided", exit 1) where
// `parseArgs` silently keeps the last, so `cli.js` restates the refusal — the
// `--jobs` grammar's sibling, and the sharper half: an unrefused
// `--parser ts --parser css` does not merely pick, it formats the input under
// a grammar the same invocation named against. The verdict is what is pinned,
// not the message (each argument parser words its own parse failures), and the
// SWITCHES ride along as the control: argh counts a repeated `--check` without
// complaint, which is what taking the last already means, so those must NOT be
// refused by either bin.
describe('repeated-option parity: both CLIs refuse a second value, and neither refuses a second switch', () => {
	const repeated: Array<{ args: Array<string>; refused: boolean; why: string }> = [
		{
			args: ['format', '--content', 'x', '--parser', 'ts', '--content', 'y'],
			refused: true,
			why: 'a second --content silently formatted the LAST one'
		},
		{
			args: ['format', '--content', 'x', '--parser', 'ts', '--parser', 'css'],
			refused: true,
			why: 'a second --parser formatted TS input as CSS'
		},
		{
			args: [
				'format',
				'--content',
				'x',
				'--parser',
				'ts',
				'--source-type',
				'module',
				'--source-type',
				'script'
			],
			refused: true,
			why: 'a second --source-type'
		},
		{
			args: ['format', '--list', '--jobs', '2', '--jobs', '3', '.'],
			refused: true,
			why: 'a second --jobs'
		},
		{
			args: ['parse', '--content', 'x', '--parser', 'ts', '--content', 'y'],
			refused: true,
			why: 'the same rule on the other command'
		},
		// The controls. Both must come out CLEAN, so each is an invocation whose
		// single-switch form exits 0 — a repeated `--check` would exit 1 on
		// changed input and prove nothing about the repetition.
		{
			args: ['format', '--list', '--list', empty_dir],
			refused: false,
			why: 'a repeated switch is fine (argh counts it)'
		},
		{
			args: ['parse', '--content', 'x', '--parser', 'ts', '--pretty', '--pretty'],
			refused: false,
			why: 'the same, on parse'
		}
	];

	for (const { args, refused, why } of repeated) {
		it(`${args.join(' ')} is ${refused ? 'refused' : 'accepted'} by both (${why})`, () => {
			const native = run_native(args);
			const mirror = run_mirror(args);
			// A refusal is exit 1 (an argument error on both sides); acceptance is
			// a clean 0, which is why the controls are invocations that do no work.
			assert.equal(
				native.status,
				refused ? 1 : 0,
				`the native CLI disagrees with the row: ${native.stderr}`
			);
			assert.equal(
				mirror.status,
				native.status,
				`cli.js ${mirror.status === 0 ? 'accepted' : 'refused'} what the native CLI did not: ${mirror.stderr || native.stderr}`
			);
		});
	}
});

// `--jobs` is the one flag whose accepted SET is stated twice: argh parses it as
// a Rust `usize`, `cli.js` re-states that with a regex. The messages are each
// argument parser's own and will never match, so what is pinned here is the
// verdict — a value one bin runs and the other refuses is the drift, and both
// edges of `usize` are the ones a regex misses. `--list` is the probe because it
// returns before the pool is ever sized: an accepted value does no work and a
// refused one is an argument error, so the exit code is a clean yes/no.
describe('--jobs parity: both CLIs accept exactly what a Rust usize accepts', () => {
	const rows: Array<{ value: string; accepted: boolean; why: string }> = [
		{ value: '2', accepted: true, why: 'an ordinary count' },
		{ value: '0', accepted: true, why: 'each pool floors it at 1 itself' },
		{ value: '007', accepted: true, why: 'leading zeros' },
		{ value: '+5', accepted: true, why: "Rust's FromStr takes a leading +" },
		{ value: '18446744073709551615', accepted: true, why: 'usize::MAX' },
		{ value: '18446744073709551616', accepted: false, why: 'one past usize::MAX' },
		{ value: '-1', accepted: false, why: 'negative' },
		{ value: '5.0', accepted: false, why: 'not an integer' },
		{ value: '5e3', accepted: false, why: 'exponent notation' },
		{ value: '0x5', accepted: false, why: 'hex' },
		{ value: ' 5', accepted: false, why: 'leading space' },
		{ value: '', accepted: false, why: 'empty' }
	];

	for (const { value, accepted, why } of rows) {
		it(`--jobs ${JSON.stringify(value)} is ${accepted ? 'accepted' : 'refused'} by both (${why})`, () => {
			const args = ['format', '--jobs', value, '--list', empty_dir];
			const native = run_native(args);
			const mirror = run_mirror(args);
			assert.equal(
				native.status,
				accepted ? 0 : 1,
				`native disagrees with the row: ${native.stderr}`
			);
			assert.equal(
				mirror.status,
				native.status,
				`cli.js ${mirror.status === 0 ? 'accepted' : 'refused'} what the native CLI did not: ${mirror.stderr || native.stderr}`
			);
		});
	}
});

// The degraded-binary paths, posix-only (see the staging comment): a binary
// that exists but can't run, and a child that dies by signal.
describe('cli (bin.js): degraded-binary paths', { skip: !posix }, () => {
	// The exact failure the publish-side chmod guards (a mode lost in
	// transit): the run still succeeds, loudly, through the JS mirror.
	it('a present-but-unrunnable binary warns and falls back to the JS mirror', () => {
		const bad_bin = join(staged_bad_mode, 'node_modules', '@fuzdev', 'tsv', 'bin.js');
		const result = spawnSync(
			process.execPath,
			[bad_bin, 'format', '--content', 'const   x=1', '--parser', 'ts'],
			{ encoding: 'utf-8' }
		);
		assert.equal(result.status, 0, result.stderr);
		assert.equal(result.stdout, 'const x = 1;\n');
		assert.match(result.stderr, /could not run its native CLI/);
		assert.match(result.stderr, /falling back to the JS CLI/);
	});

	// The README-promised signal contract: a child killed by a signal is
	// re-raised, so the dispatcher's own exit status reports the same signal
	// death instead of a plain exit code.
	it('re-raises the signal a child died by', () => {
		const sig_bin = join(staged_signal, 'node_modules', '@fuzdev', 'tsv', 'bin.js');
		const result = spawnSync(process.execPath, [sig_bin, 'help'], { encoding: 'utf-8' });
		assert.equal(
			result.signal,
			'SIGTERM',
			`expected signal death, got status=${result.status} stderr=${result.stderr}`
		);
	});
});

// Discovery parity — the shared scenario table runs through BOTH bin entries
// (see `scripts/discovery_parity_suite.ts`). Through bin.js it exercises the
// native CLI's own discovery via the shim — the real `npx tsv` path, proving
// the dispatcher preserves cwd/argv/stdio over the whole table. Through
// cli.js directly it stays the table's third consumer: the JS loop over the
// NATIVE `IgnoreStack` (the `#[napi]` twin — the fallback path), so the
// addon's discovery verdicts can't drift from the wasm binding's or the
// native CLI's.
register_discovery_parity_suite('discovery parity (bin.js): the native CLI via the shim', bin_path);
register_discovery_parity_suite('discovery parity (cli.js): the native IgnoreStack', cli_path);
