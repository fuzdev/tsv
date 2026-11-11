/**
 * The scripts CI runs on Windows and with no `node_modules` — `build:stamped`'s entry, the
 * N-API staging's, `validate:napi`'s, and `test:bun`'s launcher — and everything each imports
 * name only `node:*` modules and relative files, never a package (docs/tasks.md §`build:stamped`,
 * §`build:napi:packages`, §`validate:napi`, §`test:bun`). Read from the source text, so it holds
 * whatever the root config's Deno walk excludes.
 */

import { strictEqual } from 'node:assert';
import { readFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'vitest';

const CHECKOUT = join(dirname(fileURLToPath(import.meta.url)), '..');

/**
 * Every module specifier in `source`'s static imports and re-exports, type-only ones included:
 * `import x from`, `import {…} from`, `import * as x from`, `export {…} from`, `export * from`,
 * and a bare `import '…'`.
 */
const specifiers_of = (source: string): string[] => {
	const clause = String.raw`(?:type\s+)?(?:[\w$]+\s*,?\s*)?(?:\{[^}]*\}\s*|\*(?:\s+as\s+[\w$]+)?\s*)?`;
	const pattern = new RegExp(
		String.raw`^\s*(?:import|export)\s+${clause}from\s*['"]([^'"]+)['"]|^\s*import\s*['"]([^'"]+)['"]`,
		'gm'
	);
	return [...source.matchAll(pattern)].map((m) => (m[1] ?? m[2])!);
};

/** The import closure of `entry`: every file it reaches, and each import naming no `node:*`
 * module or relative file (or one inside a `node_modules`). */
const closure_of = (entry: string): { seen: Set<string>; outside: string[] } => {
	const seen = new Set<string>();
	const outside: string[] = [];
	const visit = (path: string): void => {
		if (seen.has(path)) return;
		seen.add(path);
		for (const specifier of specifiers_of(readFileSync(path, 'utf8'))) {
			if (specifier.startsWith('node:')) continue;
			if (specifier.startsWith('./') || specifier.startsWith('../')) {
				const target = join(dirname(path), specifier);
				if (relative(CHECKOUT, target).split('/').includes('node_modules')) {
					outside.push(`${relative(CHECKOUT, path)}: ${specifier}`);
				} else visit(target);
				continue;
			}
			outside.push(`${relative(CHECKOUT, path)}: ${specifier}`);
		}
	};
	visit(join(CHECKOUT, entry));
	return { seen, outside };
};

/** A call of `import(…)` with an argument, which `specifiers_of` cannot follow: a dynamic
 * import of a package would pass the walk above and the Deno walk alike. */
const DYNAMIC_IMPORT = /\bimport\s*\(\s*[^)\s]/;

/** Each entry, and a module its closure must reach — proof the walk got past the entry. */
const ENTRIES: Array<[entry: string, reaches: string]> = [
	['scripts/roots/cargo_build.ts', 'scripts/cargo_build.ts'],
	['scripts/roots/build_napi_packages.ts', 'scripts/stage_file.ts'],
	['scripts/roots/validate_napi_artifact.ts', 'scripts/size.ts'],
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
