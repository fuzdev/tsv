/**
 * `build:stamped`'s entry and everything it imports name only `node:*` modules and relative
 * files, never a package: CI runs it with no `node_modules` (docs/tasks.md §`build:stamped`).
 * Read from the source text, so it holds whatever the root config's Deno walk excludes.
 */

import { strictEqual } from 'node:assert';
import { readFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'vitest';

const CHECKOUT = join(dirname(fileURLToPath(import.meta.url)), '..');
const ENTRY = join(CHECKOUT, 'scripts/roots/cargo_build.ts');

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

test("build:stamped's closure imports no package", () => {
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
	visit(ENTRY);
	strictEqual(outside.join('\n'), '');
	// the walk reached the library and its imports, not the entry alone
	strictEqual(seen.has(join(CHECKOUT, 'scripts/cargo_build.ts')), true);
});
