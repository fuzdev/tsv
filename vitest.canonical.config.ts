/**
 * Vitest for the suites that load the bench install (`npm run test:node:canonical`), run under
 * Node: `benches/js/lib/canonical_test.ts`, the canonical prettier baseline's parser routing,
 * and `benches/js/lib/compare_engines_test.ts`, the comparison tools' engine binding. Both
 * reach prettier, Svelte and acorn in `benches/js/node_modules`, so they need
 * `npm run bench:install` and stay out of `check` (`vitest.config.ts` holds the suites that don't, and says why the config refuses
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
	test: { include: ['benches/js/lib/canonical_test.ts', 'benches/js/lib/compare_engines_test.ts'] }
});
