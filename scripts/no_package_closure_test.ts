/**
 * The scripts CI runs with no `node_modules` — on Windows, `build:stamped`'s entry, the N-API
 * staging's, `validate:napi`'s, `test:bun`'s launcher, and the PGO pipeline with the drivers it
 * spawns; in the credentialed release jobs, `publish:napi`'s, `release:notes`', and
 * `release:assets`' — and everything each loads name only `node:*` modules and relative files,
 * never a package (docs/tasks.md §`build:stamped`, §`build:napi:packages`, §`validate:napi`,
 * §`test:bun`, §`build:pgo`, §`publish:napi`, §`release:notes`, §`release:assets`). And the
 * publish and release suites, which drive their libraries over in-process fakes, load nothing
 * that can spawn, write, or reach the network: no host module, no capability-bearing `node:*`
 * module, no ambient global. Read from the source text through `scripts/import_closure.ts`, so
 * it holds whatever the root config's Deno walk excludes.
 */

import { strictEqual } from 'node:assert';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterAll, describe, test } from 'vitest';

import { type ImportClosure, walk_import_closure } from './import_closure.ts';

const CHECKOUT = join(dirname(fileURLToPath(import.meta.url)), '..');

/** Reads a root-relative path beneath `root`. */
const reader =
	(root: string) =>
	(path: string): string =>
		readFileSync(join(root, path), 'utf8');

/** What `entry` loads: a type-only import loads nothing, so the walk stops there, but a
 * type-only import of a package is still one (a bare checkout can't resolve it). */
const loads_of = (entry: string, root = CHECKOUT): ImportClosure =>
	walk_import_closure([entry], { read: reader(root), follow_type_only: false });

/** Each import of `closure` outside the walk, as `<file>:<line>: <specifier>`. */
const package_imports = (closure: ImportClosure): string =>
	closure.packages.map((found) => `${found.from}:${found.line}: ${found.specifier}`).join('\n');

/** Each entry, and a module its closure must reach — proof the walk got past the entry. */
const ENTRIES: Array<[entry: string, reaches: string]> = [
	['scripts/roots/cargo_build.ts', 'scripts/cargo_build.ts'],
	['scripts/roots/build_napi_packages.ts', 'scripts/stage_file.ts'],
	['scripts/roots/validate_napi_artifact.ts', 'scripts/size.ts'],
	['scripts/roots/publish_napi.ts', 'scripts/napi_host.ts'],
	['scripts/roots/release_notes.ts', 'scripts/release/changelog.ts'],
	['scripts/roots/release_assets.ts', 'scripts/release/node_host.ts'],
	['scripts/test_bun.ts', 'scripts/check_staged_freshness.ts'],
	['scripts/pgo/pgo.ts', 'scripts/import_closure.ts'],
	['scripts/pgo/addon_ops.ts', 'scripts/pgo/layout.ts'],
	['scripts/pgo/addon_depth.ts', 'scripts/depth_shapes.ts']
];

/** The `import(…)` of a computed specifier each closure may hold, by file, with how many and
 * why: each loads product code a step built, by its path, never a package by name. */
const COMPUTED_IMPORTS: Record<string, { count: number; why: string }> = {
	'scripts/test_bun.ts': { count: 1, why: 'each staged package, by its file URL, under Bun' },
	'scripts/pgo/addon_ops.ts': { count: 1, why: 'the staged `@fuzdev/tsv` loader, by its path' },
	'scripts/pgo/addon_depth.ts': { count: 1, why: 'the staged `@fuzdev/tsv` loader, by its path' }
};

/** Each computed `import(…)` of `closure` past what `COMPUTED_IMPORTS` allows its file. */
const unlisted_computed = (closure: ImportClosure): string => {
	const by_file = new Map<string, number[]>();
	for (const { from, line } of closure.computed)
		by_file.set(from, [...(by_file.get(from) ?? []), line]);
	return [...by_file]
		.filter(([from, lines]) => lines.length !== COMPUTED_IMPORTS[from]?.count)
		.map(([from, lines]) => `${from}:${lines.join(',')}`)
		.join('\n');
};

for (const [entry, reaches] of ENTRIES) {
	test(`${entry}'s closure imports no package`, () => {
		const closure = loads_of(entry);
		strictEqual(package_imports(closure), '');
		strictEqual(closure.files.includes(reaches), true);
	});

	test(`${entry}'s closure imports nothing by a computed specifier but what it names`, () => {
		strictEqual(unlisted_computed(loads_of(entry)), '');
	});
}

test('each computed import allowed is still there', () => {
	const closures = ENTRIES.map(([entry]) => loads_of(entry));
	for (const [file, { count }] of Object.entries(COMPUTED_IMPORTS)) {
		const lines = new Set(
			closures.flatMap((c) => c.computed.filter((found) => found.from === file).map((f) => f.line))
		);
		strictEqual(lines.size, count, file);
	}
});

/** The suites of `publish`, `publish:crates`, `publish:napi`, `release:notes`, and
 * `release:assets`, and of the release driver's program calls, each with a module its closure
 * must reach. Those scripts are never run; these drive their libraries over fakes, the real
 * publish paths included, and their closures must not be able to do more. */
const FAKES_ONLY_SUITES: Array<[suite: string, reaches: string]> = [
	['scripts/publish_crates_test.ts', 'scripts/release/crates.ts'],
	['scripts/publish_crates_wetrun_test.ts', 'scripts/release/version.ts'],
	['scripts/release/crates_test.ts', 'scripts/release/crates.ts'],
	['scripts/publish_napi_test.ts', 'scripts/publish_napi.ts'],
	['scripts/release_notes_test.ts', 'scripts/release/changelog.ts'],
	['scripts/release_assets_test.ts', 'scripts/release/version.ts'],
	['scripts/publish_test.ts', 'scripts/release/git.ts'],
	['scripts/release/program_test.ts', 'scripts/release/program.ts']
];

/** What those closures may load: assertions, pure `node:*` helpers, the test runner, and gro's
 * grant-free error module. A type-only import loads nothing, so it may name any module. */
const FAKES_ONLY_LOADS = new Set([
	'node:assert',
	'node:os',
	'node:path',
	'node:util',
	'vitest',
	'@fuzdev/gro/fs_error.ts'
]);

/** A use of a global that carries authority on its own, past any import: Node's `fetch` or
 * `WebSocket`, `process`, Deno's or Bun's namespace, `require`, a reach through `globalThis`
 * or `global`, or vitest's own importer. Matched in the raw source, comments included. */
const AMBIENT_AUTHORITY =
	/(?<![.\w$])(?:fetch\s*[()]|WebSocket\s*\(|(?:process|Deno|Bun|global)\s*[.[]|globalThis\b|require\s*\()|\bimport(?:Actual|Mock)\s*\(/;

/** What `suite`'s closure loads past `FAKES_ONLY_LOADS`, any computed `import(…)`, and each
 * file reaching an ambient global, one per line. */
const fakes_only_violations = (suite: string, root = CHECKOUT): string => {
	const closure = loads_of(suite, root);
	const loads = [...closure.packages, ...closure.builtins]
		.filter((found) => !found.type_only && !FAKES_ONLY_LOADS.has(found.specifier))
		.map((found) => `${found.from}:${found.line}: ${found.specifier}`);
	const computed = closure.computed.map(({ from, line }) => `${from}:${line}: import(…)`);
	const ambient = closure.files.filter((path) => AMBIENT_AUTHORITY.test(reader(root)(path)));
	return [...loads, ...computed, ...ambient].join('\n');
};

for (const [suite, reaches] of FAKES_ONLY_SUITES) {
	test(`${suite}'s closure loads nothing that can spawn, write, or reach the network`, () => {
		strictEqual(loads_of(suite).files.includes(reaches), true);
		strictEqual(fakes_only_violations(suite), '');
	});
}

// `run_if_stale`'s library, which the `build:wasm:*` tasks run on every bench build, reads the
// crate lists from an import-free module, not from the bench's artifact table, whose closure
// loads `node:fs`, `node:crypto`, and `node:os`; `node:os` comes in only through
// `scripts/exit_code.ts`, for the platform's signal numbers; a bare `fs`, `crypto`, or `os`
// (the walk's package, Node's builtin) is the same module
test("scripts/run_if_stale.ts's closure loads no node:fs or node:crypto, and node:os only for signal numbers", () => {
	const closure = loads_of('scripts/run_if_stale.ts');
	strictEqual(closure.files.includes('benches/js/lib/crate_lists.ts'), true);
	const loaded = [...closure.builtins, ...closure.packages]
		.filter((found) => /^(node:)?(fs|crypto|os)(\/|$)/.test(found.specifier))
		.filter((found) => !(found.specifier === 'node:os' && found.from === 'scripts/exit_code.ts'))
		.map((found) => `${found.from}: ${found.specifier}`);
	strictEqual(loaded.join('\n'), '');
});

// the rows' own checks, over a scratch tree whose entry's closure plants an import where a
// line-anchored scan never looked
describe('the checks over a planted import', () => {
	const root = mkdtempSync(join(tmpdir(), 'tsv_closure_'));
	afterAll(() => rmSync(root, { recursive: true, force: true }));
	const plant = (name: string, lib: string): string => {
		mkdirSync(join(root, name), { recursive: true });
		writeFileSync(
			join(root, name, 'entry.ts'),
			"import { x } from './lib.ts';\nexport const y = x;\n"
		);
		writeFileSync(join(root, name, 'lib.ts'), lib);
		return `${name}/entry.ts`;
	};
	const PLANTS: Array<[name: string, lib: string, found: string]> = [
		[
			'after_code',
			"export const x = 1; import { spawn } from '@fuzdev/gro/spawn.ts';\n",
			'after_code/lib.ts:1: @fuzdev/gro/spawn.ts'
		],
		[
			'after_comment',
			"/* the spawn */ import { spawn } from '@fuzdev/gro/spawn.ts';\nexport const x = spawn;\n",
			'after_comment/lib.ts:1: @fuzdev/gro/spawn.ts'
		],
		[
			'after_block_comment',
			"/**\n * the spawn\n */ import { spawn } from '@fuzdev/gro/spawn.ts';\nexport const x = spawn;\n",
			'after_block_comment/lib.ts:3: @fuzdev/gro/spawn.ts'
		],
		[
			'crlf',
			"// a comment\r\nexport const x = 1;\r\nimport { spawn } from '@fuzdev/gro/spawn.ts';\r\n",
			'crlf/lib.ts:3: @fuzdev/gro/spawn.ts'
		],
		[
			'dynamic',
			"export const x = async () => (await import('@fuzdev/gro/spawn.ts')).spawn;\n",
			'dynamic/lib.ts:1: @fuzdev/gro/spawn.ts'
		]
	];
	for (const [name, lib, found] of PLANTS) {
		test(`the no-package rows catch ${name}`, () => {
			strictEqual(package_imports(loads_of(plant(name, lib), root)), found);
		});
		test(`the fakes-only rows catch ${name}`, () => {
			strictEqual(fakes_only_violations(plant(name, lib), root), found);
		});
	}
	test('a computed import fails the rows that allow none', () => {
		const entry = plant('computed', 'export const x = (name: string) => import(name);\n');
		strictEqual(unlisted_computed(loads_of(entry, root)), 'computed/lib.ts:1');
		strictEqual(fakes_only_violations(entry, root), 'computed/lib.ts:1: import(…)');
	});
});
