/**
 * Regression test for the canonical (prettier) baseline's `filepath` handling.
 *
 * The corpus comparison's prettier baseline MUST format with a `filepath` hint so
 * prettier applies the same extension-specific heuristics a real on-disk file
 * gets (and that `tsv_debug`'s sidecar already applies). Without it, prettier
 * can't tell `.ts` from `.tsx` and force-adds the JSX-disambiguating trailing
 * comma to single-type-param arrows (`<T,>`) — a comma a real `.ts` run never
 * emits, which once manufactured ~39 phantom corpus divergences against `.ts`
 * code tsv was formatting correctly. See `canonical.ts` and prettier's
 * `shouldForceTrailingComma` in `src/language-js/print/type-parameters.js`.
 */

import { deepStrictEqual as assertEquals, ok as assert } from 'node:assert';
import { CanonicalImplementation } from './canonical.ts';
import { load_all_versions } from './versions.ts';

async function make_canonical(): Promise<CanonicalImplementation> {
	const impl = new CanonicalImplementation((await load_all_versions()).canonical);
	await impl.init();
	return impl;
}

Deno.test('canonical: single-type-param arrow keeps `<T>` (no comma) in pure .ts', async () => {
	const impl = await make_canonical();
	try {
		const out = await impl.format_async('const f = <T>(x: T) => x;\n', 'typescript');
		assertEquals(out, 'const f = <T>(x: T) => x;\n');
		assert(!out.includes('<T,>'), `expected no JSX-disambiguating comma in .ts, got: ${out}`);
	} finally {
		impl.dispose();
	}
});

Deno.test('canonical: an existing `<T,>` is normalized to `<T>` in pure .ts', async () => {
	const impl = await make_canonical();
	try {
		// Filepath-aware prettier strips the comma for `.ts`; a missing filepath would keep it.
		const out = await impl.format_async('const f = <T,>(x: T) => x;\n', 'typescript');
		assertEquals(out, 'const f = <T>(x: T) => x;\n');
	} finally {
		impl.dispose();
	}
});

Deno.test('canonical: single-type-param arrow keeps `<T,>` in .svelte (JSX disambiguation)', async () => {
	const impl = await make_canonical();
	try {
		// In a Svelte `<script lang="ts">`, prettier force-adds the JSX-disambiguating
		// comma (no `.ts` filepath, so its `shouldForceTrailingComma` guard fires). This
		// pins that canonical baseline behavior — the corpus comparison depends on it. tsv
		// itself diverges here, emitting the bare `<T>` (it has no JSX, and Svelte's parser
		// accepts the bare form); see the single_type_param_prettier_divergence fixture and
		// docs/conformance_prettier.md §TypeScript.
		const out = await impl.format_async(
			'<script lang="ts">\n\tconst f = <T>(x: T) => x;\n</script>\n',
			'svelte',
		);
		assert(out.includes('<T,>'), `expected disambiguating comma in .svelte, got: ${out}`);
	} finally {
		impl.dispose();
	}
});
