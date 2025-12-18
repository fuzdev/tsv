/**
 * Canonical implementation wrappers (prettier + svelte/compiler)
 *
 * Uses the same approach as tsv_debug's Deno sidecar for consistency.
 */

import type { Language, TsvImplementation } from './types.ts';

// Versions are defined in deno.json import map (single source of truth)
// These are extracted for display in benchmark output
// SYNC: Keep in sync with deno.json imports and crates/tsv_debug/src/deno/sidecar.ts
export const VERSIONS = {
	prettier: '3.7.4',
	'prettier-plugin-svelte': '3.4.0',
	svelte: '5.45.8',
	acorn: '8.15.0',
	'@sveltejs/acorn-typescript': '1.0.8',
} as const;

// deno-lint-ignore no-explicit-any
let prettier: any = null;
// deno-lint-ignore no-explicit-any
let prettierSvelte: any = null;
// deno-lint-ignore no-explicit-any
let svelteCompiler: any = null;
// deno-lint-ignore no-explicit-any
let acornTsParser: any = null;

export class CanonicalImplementation implements TsvImplementation {
	name = 'canonical' as const;

	async init(): Promise<void> {
		// Import all dependencies via deno.json import map
		const [prettierMod, prettierSvelteMod, svelteMod, acornMod, acornTsMod] = await Promise.all([
			import('prettier'),
			import('prettier-plugin-svelte'),
			import('svelte/compiler'),
			import('acorn'),
			import('@sveltejs/acorn-typescript'),
		]);
		prettier = prettierMod;
		prettierSvelte = prettierSvelteMod;
		svelteCompiler = svelteMod;
		// Create TypeScript parser once (acorn.Parser.extend is expensive)
		// deno-lint-ignore no-explicit-any
		acornTsParser = acornMod.Parser.extend(acornTsMod.tsPlugin() as any);
	}

	parse(source: string, language: Language): unknown {
		switch (language) {
			case 'svelte':
				return this.parseSvelte(source);
			case 'typescript':
				return this.parseTypeScript(source);
			case 'css':
				return this.parseCss(source);
		}
	}

	private parseSvelte(source: string): unknown {
		if (!svelteCompiler) throw new Error('Svelte compiler not initialized');
		return svelteCompiler.parse(source, { modern: true });
	}

	private parseTypeScript(source: string): unknown {
		if (!acornTsParser) throw new Error('Acorn not initialized');
		return acornTsParser.parse(source, {
			sourceType: 'module',
			ecmaVersion: 2025,
			locations: true,
		});
	}

	private parseCss(source: string): unknown {
		// Wrap CSS in <style> tags and parse as Svelte to get CSS AST
		if (!svelteCompiler) throw new Error('Svelte compiler not initialized');
		return svelteCompiler.parse(`<style>${source}</style>`, { modern: true });
	}

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
		acornTsParser = null;
	}
}
