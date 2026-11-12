/**
 * The staleness lists that name a staging's sources (`WASM_STAGING_SOURCES`,
 * `NAPI_STAGING_SOURCES` in `check_staged_freshness.ts`) are exactly the repo modules each
 * staging's entry loads: a module the entry imports that its list omits would let an edit to
 * it leave a staging graded fresh, and a listed path that moved would stale nothing. The
 * imports are read with TypeScript's parser, so an import written inside a generated file's
 * template text is not one; a type-only import loads nothing and is not followed.
 */

import { deepStrictEqual } from 'node:assert';
import { readFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import { test } from 'vitest';

import { NAPI_STAGING_SOURCES, WASM_STAGING_SOURCES } from './check_staged_freshness.ts';

const CHECKOUT = join(dirname(fileURLToPath(import.meta.url)), '..');

/** Whether an import declaration loads nothing: `import type`, or only `type` names. */
const is_type_only = (node: ts.ImportDeclaration): boolean => {
	const clause = node.importClause;
	if (clause === undefined) return false;
	if (clause.isTypeOnly) return true;
	const bindings = clause.namedBindings;
	return (
		clause.name === undefined &&
		bindings !== undefined &&
		ts.isNamedImports(bindings) &&
		bindings.elements.length > 0 &&
		bindings.elements.every((element) => element.isTypeOnly)
	);
};

/** The repo modules `entry` loads, itself included, checkout-relative and sorted. */
const repo_closure = (entry: string): Array<string> => {
	const seen = new Set<string>();
	const visit = (path: string): void => {
		if (seen.has(path)) return;
		seen.add(path);
		const source = ts.createSourceFile(path, readFileSync(path, 'utf8'), ts.ScriptTarget.Latest);
		for (const statement of source.statements) {
			let specifier: string | null = null;
			if (ts.isImportDeclaration(statement) && !is_type_only(statement)) {
				specifier = (statement.moduleSpecifier as ts.StringLiteral).text;
			} else if (
				ts.isExportDeclaration(statement) &&
				!statement.isTypeOnly &&
				statement.moduleSpecifier !== undefined
			) {
				specifier = (statement.moduleSpecifier as ts.StringLiteral).text;
			}
			if (specifier?.startsWith('./') || specifier?.startsWith('../')) {
				visit(join(dirname(path), specifier));
			}
		}
	};
	visit(join(CHECKOUT, entry));
	return [...seen].map((path) => relative(CHECKOUT, path)).sort();
};

for (const [name, list, entry] of [
	['WASM_STAGING_SOURCES', WASM_STAGING_SOURCES, 'scripts/roots/patch_npm_package.ts'],
	['NAPI_STAGING_SOURCES', NAPI_STAGING_SOURCES, 'scripts/roots/build_napi_packages.ts']
] as const) {
	test(`${name} is the repo closure of ${entry}`, () => {
		deepStrictEqual([...list].sort(), repo_closure(entry));
	});
}
