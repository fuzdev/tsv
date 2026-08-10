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
 * binary executable, that a child's signal death is re-raised, and that with
 * the binary removed or unrunnable it degrades to the shared `cli.js` JS
 * mirror over the native engine. The
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
 * CLI binary removed asserts the dispatcher's JS fallback.
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

after(() => {
	// Best-effort: on Windows the loaded addon stays mapped into the process,
	// so its .node file — and therefore `staged` — is undeletable until exit
	// (EPERM). A leaked temp staging is harmless (ephemeral CI runners, OS
	// temp); a cleanup failure must not fail an otherwise-green suite. The
	// bare staging loads nothing and deletes everywhere.
	for (const dir of [staged, staged_bare, staged_no_binary, staged_bad_mode, staged_signal]) {
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

	it('an unsupported platform fails loudly and points at the WASM package', async () => {
		const bare_loader = join(staged_bare, 'node_modules', '@fuzdev', 'tsv', 'index.js');
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

// The `tsv` bin — `bin.js`, the dispatcher that execs the platform package's
// native `tsv_cli` binary, with the shared `cli.js` (the wasm package's bin,
// staged here bound to the native loader) as its fallback. The full
// flag/exit-code matrix lives in `scripts/test_npm.ts` over the wasm copy and
// in `tests/cli_tests.rs` over the binary itself; what THIS suite pins is the
// dispatch — that the bin really reaches the native binary — and the
// forwarding: exit codes, stdout/stderr split, stdin piping, in-place writes.
const bin_path = join(staged, 'node_modules', '@fuzdev', 'tsv', 'bin.js');
const cli_path = join(staged, 'node_modules', '@fuzdev', 'tsv', 'cli.js');
const run_cli = (args: Array<string>, stdin?: string) =>
	spawnSync(process.execPath, [bin_path, ...args], { encoding: 'utf-8', input: stdin });

describe('cli (bin.js): the tsv bin dispatching to the native CLI binary', () => {
	it('is wired as the package bin', () => {
		const pkg = JSON.parse(readFileSync(join(dirname(bin_path), 'package.json'), 'utf8'));
		assert.deepEqual(pkg.bin, { tsv: './bin.js' });
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

	// What npm would actually publish: the pack file list must carry the CLI
	// binary WITH its execute bit — npm packs the on-disk mode, and npm only
	// chmods `bin` entries at install, so a 644 here ships a broken npx.
	// Posix-only: Windows has no mode to pin (and `npm` there is a .cmd,
	// which spawnSync can't run shell-less).
	it('npm pack ships the CLI binary, executable', { skip: !posix }, () => {
		const result = spawnSync('npm', ['pack', '--dry-run', '--json'], {
			cwd: join(pkg_root, triple),
			encoding: 'utf-8'
		});
		assert.equal(result.status, 0, result.stderr);
		const [report] = JSON.parse(result.stdout);
		const entry = report.files.find((f: { path: string }) => f.path === cli_binary_name);
		assert.ok(entry, `${cli_binary_name} missing from the packed file list`);
		assert.ok(entry.mode & 0o111, `packed mode ${entry.mode.toString(8)} is not executable`);
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
