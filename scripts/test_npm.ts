/**
 * Node.js tests for the built npm packages (@fuzdev/tsv_format_wasm,
 * @fuzdev/tsv_parse_wasm, and @fuzdev/tsv_wasm).
 *
 * Verifies the wasm-pack web target + patch_npm_package.ts wrapper works
 * correctly when imported as ESM in Node.js: the auto-init node entry
 * (index.js), the guarded browser entry (browser.js), the package.json
 * exports/files wiring, and (for the `all` variant) the `tsv` bin.
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
 *   PKG_DIR=crates/tsv_wasm/pkg/all/npm node --test scripts/test_npm.ts
 *
 * Prerequisites: deno task build:npm:format (or build:npm:parse / build:npm:all)
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const pkg_dir = process.env.PKG_DIR;
if (!pkg_dir) {
	console.error('Usage: PKG_DIR=<pkg-dir> node --test scripts/test_npm.ts');
	console.error('Example: PKG_DIR=crates/tsv_wasm/pkg/format/npm node --test scripts/test_npm.ts');
	process.exit(1);
}

const variant = /\/(format|parse|all)\/npm\/?$/.exec(pkg_dir)?.[1];
if (!variant) {
	console.error(`PKG_DIR must end in /(format|parse|all)/npm — got ${pkg_dir}`);
	process.exit(1);
}
const has_format = variant !== 'parse';
const has_parse = variant !== 'format';
const PKG_NAMES = {
	format: '@fuzdev/tsv_format_wasm',
	parse: '@fuzdev/tsv_parse_wasm',
	all: '@fuzdev/tsv_wasm',
};

const node_entry = await import(`../${pkg_dir}/index.js`);

describe(`package metadata: ${pkg_dir}`, () => {
	const pkg = JSON.parse(
		readFileSync(new URL(`../${pkg_dir}/package.json`, import.meta.url), 'utf-8'),
	);

	it('has the right name', () => {
		assert.equal(pkg.name, PKG_NAMES[variant]);
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

	it('parse-capable variants bundle tsv_ast.d.ts', { skip: !has_parse }, () => {
		assert.ok(pkg.files.includes('tsv_ast.d.ts'));
	});

	it('all variant ships the tsv bin', { skip: variant !== 'all' }, () => {
		assert.deepEqual(pkg.bin, { tsv: './cli.js' });
		assert.ok(pkg.files.includes('cli.js'));
	});

	it('subset variants ship no bin', { skip: variant === 'all' }, () => {
		assert.equal(pkg.bin, undefined);
	});
});

describe(`node entry (index.js): ${pkg_dir}`, () => {
	it('format_typescript formats', { skip: !has_format }, () => {
		assert.equal(node_entry.format_typescript('const   x=1'), 'const x = 1;\n');
	});

	it('format_css formats', { skip: !has_format }, () => {
		assert.equal(node_entry.format_css('a{color:red}'), 'a {\n\tcolor: red;\n}\n');
	});

	it('format_svelte formats', { skip: !has_format }, () => {
		assert.equal(node_entry.format_svelte('<div   >x</div   >'), '<div>x</div>\n');
	});

	it('formatting is idempotent', { skip: !has_format }, () => {
		const once = node_entry.format_svelte('<script>const   x=1</script>\n\n<div>{x}</div>');
		assert.equal(node_entry.format_svelte(once), once);
	});

	it('throws a useful error on invalid syntax', { skip: !has_format }, () => {
		assert.throws(() => node_entry.format_typescript('const ='));
	});

	it('format_* absent from the parse-only build', { skip: has_format }, () => {
		assert.equal(node_entry.format_typescript, undefined);
	});

	it('parse_* absent from the format-only build', { skip: has_parse }, () => {
		assert.equal(node_entry.parse_typescript, undefined);
	});

	it('parse_typescript returns a Program', { skip: !has_parse }, () => {
		const program = node_entry.parse_typescript('const x = 1;');
		assert.equal(program.type, 'Program');
		assert.ok(Array.isArray(program.body));
	});

	it('parse_typescript_json returns a JSON string', { skip: !has_parse }, () => {
		const json = node_entry.parse_typescript_json('const x = 1;');
		assert.equal(typeof json, 'string');
		assert.equal(JSON.parse(json).type, 'Program');
	});

	it('parse_svelte and parse_css work', { skip: !has_parse }, () => {
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

	it('exports throw before init', () => {
		const guarded = has_format ? 'format_typescript' : 'parse_typescript';
		assert.throws(() => browser[guarded]('const x = 1'), /WASM not initialized/);
	});

	it('init_sync initializes WASM', () => {
		const wasm = readFileSync(new URL(`../${pkg_dir}/tsv_wasm_bg.wasm`, import.meta.url));
		browser.init_sync({ module: wasm });
	});

	it('format functions work after init', { skip: !has_format }, () => {
		assert.equal(browser.format_typescript('const   x=1'), 'const x = 1;\n');
		assert.equal(browser.format_css('a{color:red}'), 'a {\n\tcolor: red;\n}\n');
		assert.equal(browser.format_svelte('<div   >x</div   >'), '<div>x</div>\n');
	});

	it('parse works after init', { skip: !has_parse }, () => {
		assert.equal(browser.parse_typescript('const x = 1;').type, 'Program');
	});

	it('init is idempotent after init_sync', async () => {
		// Should resolve without re-fetching — just returns early
		await browser.init();
		if (has_format) {
			assert.equal(browser.format_typescript('const   x=1'), 'const x = 1;\n');
		} else {
			assert.equal(browser.parse_typescript('const x = 1;').type, 'Program');
		}
	});
});

// CLI (`tsv` bin, `all` variant only) — subprocess tests against the contract
// the JS CLI mirrors from the native tsv_cli: flags, exit codes, output streams.
describe(`cli (cli.js): ${pkg_dir}`, { skip: variant !== 'all' }, () => {
	const cli_path = new URL(`../${pkg_dir}/cli.js`, import.meta.url).pathname;
	const run_cli = (args: Array<string>, stdin?: string, cwd?: string) =>
		spawnSync(process.execPath, [cli_path, ...args], {
			encoding: 'utf-8',
			input: stdin,
			cwd,
		});

	it('format --content prints formatted source', () => {
		const result = run_cli(['format', '--content', 'const   x=1', '--parser', 'typescript']);
		assert.equal(result.status, 0);
		assert.equal(result.stdout, 'const x = 1;\n');
	});

	it('--parser accepts the ts alias', () => {
		const result = run_cli(['format', '--content', 'const   x=1', '--parser', 'ts']);
		assert.equal(result.status, 0);
		assert.equal(result.stdout, 'const x = 1;\n');
	});

	it('format --stdin reads stdin', () => {
		const result = run_cli(['format', '--stdin', '--parser', 'css'], 'a{color:red}');
		assert.equal(result.status, 0);
		assert.equal(result.stdout, 'a {\n\tcolor: red;\n}\n');
	});

	it('format --content without --parser exits 2', () => {
		const result = run_cli(['format', '--content', 'const x = 1;']);
		assert.equal(result.status, 2);
		assert.match(result.stderr, /requires --parser/);
	});

	it('format --check --content exits 1 on would-change', () => {
		const result = run_cli(['format', '--check', '--content', 'const   x=1', '--parser', 'ts']);
		assert.equal(result.status, 1);
		assert.match(result.stderr, /would change/);
	});

	it('format --check --content exits 0 on clean input', () => {
		const result = run_cli(['format', '--check', '--content', 'const x = 1;\n', '--parser', 'ts']);
		assert.equal(result.status, 0);
	});

	it('format on invalid syntax exits 2', () => {
		const result = run_cli(['format', '--content', 'const =', '--parser', 'ts']);
		assert.equal(result.status, 2);
		assert.match(result.stderr, /Parse error/);
	});

	it('unknown flags exit 1', () => {
		const result = run_cli(['format', '--bogus']);
		assert.equal(result.status, 1);
	});

	it('a bad --parser value exits 1 in both commands (argument-parsing error)', () => {
		for (const command of ['format', 'parse']) {
			const result = run_cli([command, '--content', 'const x = 1;', '--parser', 'bogus']);
			assert.equal(result.status, 1);
			assert.match(result.stderr, /Unknown parser type/);
		}
	});

	it('format paths writes in place, recurses, and skips excluded dirs', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.ts'), 'const   x=1');
			writeFileSync(join(dir, 'clean.css'), 'a {\n\tcolor: red;\n}\n');
			mkdirSync(join(dir, 'nested'));
			writeFileSync(join(dir, 'nested', 'b.svelte'), '<div   >x</div   >');
			mkdirSync(join(dir, 'node_modules'));
			writeFileSync(join(dir, 'node_modules', 'skip.ts'), 'const   y=2');

			const result = run_cli(['format', dir]);
			assert.equal(result.status, 0);
			assert.deepEqual(
				result.stdout.trim().split('\n').sort(),
				[join(dir, 'a.ts'), join(dir, 'nested', 'b.svelte')],
			);
			assert.match(result.stderr, /2 formatted, 1 unchanged/);
			assert.equal(readFileSync(join(dir, 'a.ts'), 'utf-8'), 'const x = 1;\n');
			assert.equal(readFileSync(join(dir, 'nested', 'b.svelte'), 'utf-8'), '<div>x</div>\n');
			assert.equal(readFileSync(join(dir, 'node_modules', 'skip.ts'), 'utf-8'), 'const   y=2');

			// Second run: everything clean, --check passes.
			const check = run_cli(['format', '--check', dir]);
			assert.equal(check.status, 0);
			assert.match(check.stderr, /0 would change, 3 unchanged/);
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('format --check exits 1 and leaves files untouched', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.ts'), 'const   x=1');
			const result = run_cli(['format', '--check', dir]);
			assert.equal(result.status, 1);
			assert.match(result.stderr, /1 would change/);
			assert.equal(readFileSync(join(dir, 'a.ts'), 'utf-8'), 'const   x=1');
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('format on a missing path exits 2', () => {
		const result = run_cli(['format', '/nonexistent/tsv-cli-test']);
		assert.equal(result.status, 2);
		assert.match(result.stderr, /not a file or directory/);
	});

	it('format --content combined with a path exits 2', () => {
		const result = run_cli(['format', '--content', 'const x = 1;', '--parser', 'ts', 'a.ts']);
		assert.equal(result.status, 2);
		assert.match(result.stderr, /cannot be combined with file paths/);
	});

	it('format --parser with paths exits 2 (paths use extension detection)', () => {
		const result = run_cli(['format', '--parser', 'ts', 'a.ts']);
		assert.equal(result.status, 2);
		assert.match(result.stderr, /applies to --content/);
	});

	it('format --jobs is accepted in path mode and ignored', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.ts'), 'const x = 1;\n');
			const result = run_cli(['format', '--check', '--jobs', '4', dir]);
			assert.equal(result.status, 0);
			assert.match(result.stderr, /0 would change, 1 unchanged/);
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('format --jobs with --content exits 2 (path mode only)', () => {
		const result = run_cli([
			'format',
			'--jobs',
			'4',
			'--content',
			'const x = 1;',
			'--parser',
			'ts',
		]);
		assert.equal(result.status, 2);
		assert.match(result.stderr, /--jobs applies to file paths/);
	});

	it('format --jobs with a non-integer value exits 1 (argument-parsing error)', () => {
		const result = run_cli(['format', '--jobs', 'many', 'a.ts']);
		assert.equal(result.status, 1);
		assert.match(result.stderr, /--jobs expects an integer/);
	});

	it('format on a trailing-slash root reports single-slash paths (PathBuf::push parity)', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.ts'), 'const   x=1');
			const result = run_cli(['format', '--check', `${dir}/`]);
			assert.equal(result.status, 1);
			assert.equal(result.stdout, `${dir}/a.ts\n`);
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('format dedupes overlapping root spellings by canonical path', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.ts'), 'const   x=1');
			// the same file via a relative traversal ('.') and an absolute explicit arg
			const result = run_cli(['format', '.', join(dir, 'a.ts')], undefined, dir);
			assert.equal(result.status, 0);
			assert.match(result.stderr, /1 formatted, 0 unchanged/);
			assert.equal(result.stdout.trim().split('\n').length, 1);
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('format trusts an explicit file arg regardless of extension', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.txt'), 'const   x=1');
			// traversal skips it (no supported extension)…
			const traversed = run_cli(['format', dir]);
			assert.equal(traversed.status, 2);
			assert.match(traversed.stderr, /No supported files found/);
			// …but the explicit arg is formatted (extension default: typescript)
			const explicit = run_cli(['format', join(dir, 'a.txt')]);
			assert.equal(explicit.status, 0);
			assert.equal(readFileSync(join(dir, 'a.txt'), 'utf-8'), 'const x = 1;\n');
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('parse --content prints compact JSON with trailing newline', () => {
		const result = run_cli(['parse', '--content', 'const x = 1;', '--parser', 'typescript']);
		assert.equal(result.status, 0);
		assert.ok(result.stdout.endsWith('\n'));
		assert.equal(JSON.parse(result.stdout).type, 'Program');
	});

	it('parse --pretty prints tab-indented JSON', () => {
		const result = run_cli([
			'parse',
			'--pretty',
			'--content',
			'const x = 1;',
			'--parser',
			'ts',
		]);
		assert.equal(result.status, 0);
		assert.match(result.stdout, /^\{\n\t"type": "Program",\n/);
	});

	it('parse a file detects the parser from the extension', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.svelte'), '<div>x</div>');
			const result = run_cli(['parse', join(dir, 'a.svelte')]);
			assert.equal(result.status, 0);
			assert.equal(JSON.parse(result.stdout).type, 'Root');
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('parse --parser overrides the file extension', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'data.ts'), '<div>x</div>');
			const result = run_cli(['parse', '--parser', 'svelte', join(dir, 'data.ts')]);
			assert.equal(result.status, 0);
			assert.equal(JSON.parse(result.stdout).type, 'Root');
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('parse on invalid syntax exits 1', () => {
		const result = run_cli(['parse', '--content', 'const =', '--parser', 'ts']);
		assert.equal(result.status, 1);
		assert.match(result.stderr, /Parse error/);
	});

	it('parse rejects a second positional', () => {
		const result = run_cli(['parse', 'a.ts', 'b.ts']);
		assert.equal(result.status, 1);
		assert.match(result.stderr, /Unrecognized argument/);
	});

	it('no command prints usage and exits 1', () => {
		const result = run_cli([]);
		assert.equal(result.status, 1);
		assert.match(result.stderr, /Usage: tsv/);
	});

	it('--help exits 0', () => {
		const result = run_cli(['--help']);
		assert.equal(result.status, 0);
		assert.match(result.stdout, /Usage: tsv/);
	});

	it('help subcommand exits 0 (mirrors argh)', () => {
		const result = run_cli(['help', 'format']);
		assert.equal(result.status, 0);
		assert.match(result.stdout, /Usage: tsv format/);
	});
});
