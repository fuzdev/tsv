/**
 * The scripts CI runs with no `node_modules` — on Windows, `build:stamped`'s entry, the N-API
 * staging's, `validate:napi`'s, and `test:bun`'s launcher; in the credentialed release jobs,
 * `publish:napi`'s, `release:notes`', and `release:assets`' — and everything each imports name
 * only `node:*` modules and relative files, never a package (docs/tasks.md §`build:stamped`,
 * §`build:napi:packages`, §`validate:napi`, §`test:bun`, §`publish:napi`, §`release:notes`,
 * §`release:assets`). And the publish and release suites, which drive their libraries over
 * in-process fakes, load nothing that can spawn, write, or reach the network: no host module, no
 * capability-bearing `node:*` module, no ambient global. Read from the source text, so it holds
 * whatever the root config's Deno walk excludes.
 */

import { strictEqual } from 'node:assert';
import { readFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'vitest';

const CHECKOUT = join(dirname(fileURLToPath(import.meta.url)), '..');

/** One static import or re-export: its module specifier, and whether the whole clause is
 * `type`, so nothing of it loads. */
interface ImportOf {
	specifier: string;
	type_only: boolean;
}

/**
 * Every module specifier in `source`'s static imports and re-exports, type-only ones included:
 * `import x from`, `import {…} from`, `import * as x from`, `export {…} from`, `export * from`,
 * and a bare `import '…'`.
 */
const imports_of = (source: string): ImportOf[] => {
	const clause = String.raw`(type\s+(?!from\b))?(?:[\w$]+\s*,?\s*)?(?:\{[^}]*\}\s*|\*(?:\s+as\s+[\w$]+)?\s*)?`;
	const pattern = new RegExp(
		String.raw`^\s*(?:import|export)\s+${clause}from\s*['"]([^'"]+)['"]|^\s*import\s*['"]([^'"]+)['"]`,
		'gm'
	);
	return [...source.matchAll(pattern)].map((m) => ({
		specifier: (m[2] ?? m[3])!,
		type_only: m[1] !== undefined
	}));
};

/** The import closure of `entry`: every file it reaches, each import naming no `node:*` module
 * or relative file (or one inside a `node_modules`), and each `node:*` import. */
const closure_of = (
	entry: string
): { seen: Set<string>; outside: string[]; packages: ImportOf[]; builtins: ImportOf[] } => {
	const seen = new Set<string>();
	const outside: string[] = [];
	const packages: ImportOf[] = [];
	const builtins: ImportOf[] = [];
	const visit = (path: string): void => {
		if (seen.has(path)) return;
		seen.add(path);
		for (const found of imports_of(readFileSync(path, 'utf8'))) {
			const { specifier } = found;
			if (specifier.startsWith('node:')) {
				builtins.push(found);
				continue;
			}
			if (specifier.startsWith('./') || specifier.startsWith('../')) {
				const target = join(dirname(path), specifier);
				if (relative(CHECKOUT, target).split('/').includes('node_modules')) {
					outside.push(`${relative(CHECKOUT, path)}: ${specifier}`);
					packages.push(found);
				} else if (!found.type_only) visit(target);
				continue;
			}
			outside.push(`${relative(CHECKOUT, path)}: ${specifier}`);
			packages.push(found);
		}
	};
	visit(join(CHECKOUT, entry));
	return { seen, outside, packages, builtins };
};

/** A call of `import(…)` with an argument, which `imports_of` cannot follow: a dynamic
 * import of a package would pass the walk above and the Deno walk alike. */
const DYNAMIC_IMPORT = /\bimport\s*\(\s*[^)\s]/;

/** Each entry, and a module its closure must reach — proof the walk got past the entry. */
const ENTRIES: Array<[entry: string, reaches: string]> = [
	['scripts/roots/cargo_build.ts', 'scripts/cargo_build.ts'],
	['scripts/roots/build_napi_packages.ts', 'scripts/stage_file.ts'],
	['scripts/roots/validate_napi_artifact.ts', 'scripts/size.ts'],
	['scripts/roots/publish_napi.ts', 'scripts/napi_host.ts'],
	['scripts/roots/release_notes.ts', 'scripts/release/changelog.ts'],
	['scripts/roots/release_assets.ts', 'scripts/release/node_host.ts'],
	['scripts/test_bun.ts', 'scripts/check_staged_freshness.ts']
];

for (const [entry, reaches] of ENTRIES) {
	test(`${entry}'s closure imports no package`, () => {
		const { seen, outside } = closure_of(entry);
		strictEqual(outside.join('\n'), '');
		strictEqual(seen.has(join(CHECKOUT, reaches)), true);
	});
}

// `test_bun.ts` is left out: its suite half imports each staged package by computed URL
// under Bun, and its closure's suites hold `import(` in their test inputs
for (const [entry] of ENTRIES.filter(([entry]) => entry !== 'scripts/test_bun.ts')) {
	test(`${entry}'s closure has no dynamic import`, () => {
		const dynamic = [...closure_of(entry).seen].filter((path) =>
			DYNAMIC_IMPORT.test(readFileSync(path, 'utf8'))
		);
		strictEqual(dynamic.map((path) => relative(CHECKOUT, path)).join('\n'), '');
	});
}

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
 * or `global`, or vitest's own importer. */
const AMBIENT_AUTHORITY =
	/(?<![.\w$])(?:fetch\s*[()]|WebSocket\s*\(|(?:process|Deno|Bun|global)\s*[.[]|globalThis\b|require\s*\()|\bimport(?:Actual|Mock)\s*\(/;

for (const [suite, reaches] of FAKES_ONLY_SUITES) {
	test(`${suite}'s closure loads nothing that can spawn, write, or reach the network`, () => {
		const { seen, packages, builtins } = closure_of(suite);
		strictEqual(seen.has(join(CHECKOUT, reaches)), true);
		const loads = [...packages, ...builtins]
			.filter((found) => !found.type_only && !FAKES_ONLY_LOADS.has(found.specifier))
			.map((found) => found.specifier);
		strictEqual(loads.join('\n'), '');
		const ambient = [...seen].filter((path) => {
			const source = readFileSync(path, 'utf8');
			return DYNAMIC_IMPORT.test(source) || AMBIENT_AUTHORITY.test(source);
		});
		strictEqual(ambient.map((path) => relative(CHECKOUT, path)).join('\n'), '');
	});
}
