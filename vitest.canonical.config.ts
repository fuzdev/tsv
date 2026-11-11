/**
 * Vitest for `benches/js/lib/canonical_test.ts` (`npm run test:node:canonical`), run under
 * Node: the canonical prettier baseline's parser routing, which loads prettier, Svelte and
 * acorn from `benches/js/node_modules`, so it needs `npm run bench:install` and stays out of
 * `check` (`vitest.config.ts` holds the suites that don't, and says why the config refuses
 * Deno).
 *
 * @module
 */

import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

if ('Deno' in globalThis) {
	throw Error(
		'vitest.canonical.config.ts: vitest is running under Deno; start it with a real `node`'
	);
}

export default defineConfig({
	root: fileURLToPath(new URL('.', import.meta.url)),
	test: { include: ['benches/js/lib/canonical_test.ts'] }
});
