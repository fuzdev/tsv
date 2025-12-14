/**
 * Canonical implementation wrappers (prettier + svelte/compiler)
 *
 * Uses the same approach as tsv_debug's Deno sidecar for consistency.
 */

import type { Language, TsvImplementation } from './types.ts';
import { VERSIONS } from '../../../crates/tsv_debug/src/deno/versions.ts';

// deno-lint-ignore no-explicit-any
let prettier: any = null;
// deno-lint-ignore no-explicit-any
let prettierSvelte: any = null;
// deno-lint-ignore no-explicit-any
let svelteCompiler: any = null;
// deno-lint-ignore no-explicit-any
let acorn: any = null;
// deno-lint-ignore no-explicit-any
let acornTypescript: any = null;

export class CanonicalImplementation implements TsvImplementation {
	name = 'canonical' as const;

	async init(): Promise<void> {
		// Import all dependencies
		[prettier, prettierSvelte, svelteCompiler, acorn, acornTypescript] = await Promise.all([
			import(`npm:prettier@${VERSIONS.prettier}`),
			import(`npm:prettier-plugin-svelte@${VERSIONS['prettier-plugin-svelte']}`),
			import(`npm:svelte@${VERSIONS.svelte}/compiler`),
			import(`npm:acorn@${VERSIONS.acorn}`),
			import(`npm:@sveltejs/acorn-typescript@${VERSIONS['@sveltejs/acorn-typescript']}`),
		]);
	}

	parse(source: string, language: Language): unknown {
		switch (language) {
			case 'svelte':
				return this.parseSvelte(source);
			case 'typescript':
				return this.parseTypeScript(source);
			case 'css':
				// We don't have a canonical CSS parser - our parser IS the reference
				throw new Error('No canonical CSS parser - tsv_css is the reference implementation');
		}
	}

	format(_source: string, _language: Language): string {
		// prettier.format is async-only in 3.x
		throw new Error('Use formatAsync for formatting');
	}

	private parseSvelte(source: string): unknown {
		if (!svelteCompiler) throw new Error('Svelte compiler not initialized');
		return svelteCompiler.parse(source, { modern: true });
	}

	private parseTypeScript(source: string): unknown {
		if (!acorn || !acornTypescript) throw new Error('Acorn not initialized');

		const Parser = acorn.Parser.extend(acornTypescript.tsPlugin);
		return Parser.parse(source, {
			sourceType: 'module',
			ecmaVersion: 'latest',
			locations: true,
		});
	}

	// Async versions for actual use (prettier 3.x is async-only)
	async formatAsync(source: string, language: Language): Promise<string> {
		if (!prettier || !prettierSvelte) throw new Error('Prettier not initialized');

		const parser = language === 'svelte' ? 'svelte' : language === 'css' ? 'css' : 'typescript';
		const plugins = language === 'svelte' ? [prettierSvelte] : [];

		return await prettier.format(source, {
			parser,
			plugins,
			useTabs: true,
		});
	}

	dispose(): void {
		prettier = null;
		prettierSvelte = null;
		svelteCompiler = null;
		acorn = null;
		acornTypescript = null;
	}
}
