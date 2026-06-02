/**
 * Centralized version loading from deno.json
 *
 * Single source of truth for all package versions used in benchmarks.
 */

import { extract_version } from './types.ts';

/** Canonical implementation versions */
export interface CanonicalVersions {
	prettier: string;
	'prettier-plugin-svelte': string;
	svelte: string;
	acorn: string;
	'@sveltejs/acorn-typescript': string;
}

/** OXC implementation versions */
export interface OxcVersions {
	'oxc-parser': string;
	oxfmt: string;
}

/** Biome implementation versions */
export interface BiomeVersions {
	js_api: string;
	wasm: string;
}

/** All implementation versions */
export interface AllVersions {
	canonical: CanonicalVersions;
	oxc: OxcVersions;
	biome: BiomeVersions;
}

/** Default versions when loading fails */
const DEFAULT_VERSIONS: AllVersions = {
	canonical: {
		prettier: 'unknown',
		'prettier-plugin-svelte': 'unknown',
		svelte: 'unknown',
		acorn: 'unknown',
		'@sveltejs/acorn-typescript': 'unknown',
	},
	oxc: {
		'oxc-parser': 'unknown',
		oxfmt: 'unknown',
	},
	biome: {
		js_api: 'unknown',
		wasm: 'unknown',
	},
};

/**
 * Load all package versions from deno.json import map.
 *
 * Reads benches/deno/deno.json to extract versions for all implementations.
 */
export async function load_all_versions(): Promise<AllVersions> {
	try {
		const deno_json_path = new URL('../deno.json', import.meta.url).pathname;
		const content = await Deno.readTextFile(deno_json_path);
		const config = JSON.parse(content);
		const imports = config.imports || {};

		return {
			canonical: {
				prettier: extract_version(imports['prettier'] || ''),
				'prettier-plugin-svelte': extract_version(imports['prettier-plugin-svelte'] || ''),
				svelte: extract_version(imports['svelte'] || ''),
				acorn: extract_version(imports['acorn'] || ''),
				'@sveltejs/acorn-typescript': extract_version(
					imports['@sveltejs/acorn-typescript'] || '',
				),
			},
			oxc: {
				'oxc-parser': extract_version(imports['oxc-parser'] || ''),
				oxfmt: extract_version(imports['oxfmt'] || ''),
			},
			biome: {
				js_api: extract_version(imports['@biomejs/js-api'] || ''),
				wasm: extract_version(imports['@biomejs/wasm-bundler'] || ''),
			},
		};
	} catch {
		return DEFAULT_VERSIONS;
	}
}
