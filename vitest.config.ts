/**
 * Vitest for the JS suites beside their modules under `scripts/` and `benches/js/`
 * (`npm run test:node`), run under Node.
 *
 * The suites are the `_test.ts` files `tsconfig.vitest.json` names, so every suite vitest runs
 * is one `tsc` typechecks, and each is named: a directory would sweep in
 * `benches/js/lib/canonical_test.ts` and `benches/js/lib/compare_engines_test.ts`, which need
 * the bench install
 * (`vitest.canonical.config.ts`). A named file that's missing fails here rather than leaving the run one suite short, and so does a
 * `_test.ts` file under `scripts/`, directly under `benches/js/`, or under its `diagnostics/`
 * or `lib/` that no runner names, which would otherwise never run.
 *
 * `deno task` runs a package's bin under Deno (`vitest`, or `npm run` of a script naming it),
 * which is why the script starts vitest with `node`, and on a machine with no Node it
 * resolves `node` itself to Deno's node-compat shim; either way the suites would run under
 * Deno, and the config refuses that.
 *
 * @module
 */

import { existsSync, globSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

if ('Deno' in globalThis) {
	throw Error('vitest.config.ts: vitest is running under Deno; start it with a real `node`');
}

const root = fileURLToPath(new URL('.', import.meta.url));

const program: { files: Array<string> } = JSON.parse(
	readFileSync(new URL('tsconfig.vitest.json', import.meta.url), 'utf8')
);
const include = program.files.filter((file) => file.endsWith('_test.ts'));
const missing = include.filter((file) => !existsSync(new URL(file, import.meta.url)));
if (missing.length > 0) {
	throw Error(`vitest.config.ts: tsconfig.vitest.json names missing suites: ${missing.join(', ')}`);
}

const read_files = (name: string): Array<string> =>
	JSON.parse(readFileSync(new URL(name, import.meta.url), 'utf8')).files;
const named = new Set([...include, ...read_files('tsconfig.canonical.json')]);
const unnamed = globSync(
	['scripts/**/*_test.ts', 'benches/js/*_test.ts', 'benches/js/{diagnostics,lib}/**/*_test.ts'],
	{
		cwd: root
	}
).filter((file) => !named.has(file));
if (unnamed.length > 0) {
	throw Error(
		`vitest.config.ts: suites no runner names (add each to tsconfig.vitest.json, or to tsconfig.canonical.json when it needs the bench install): ${unnamed.join(', ')}`
	);
}

export default defineConfig({
	root,
	test: { include }
});
