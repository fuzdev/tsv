/**
 * Centralized version loading from package.json
 *
 * Single source of truth for all package versions used in benchmarks.
 */

import { readFile } from 'node:fs/promises';

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

/** dprint implementation versions (the engine `deno fmt` runs — see lib/dprint.ts) */
export interface DprintVersions {
	/** The Wasm plugin host (`@dprint/formatter`) */
	formatter: string;
	/** The TS/JS plugin itself (`@dprint/typescript`) — the version worth citing */
	typescript: string;
}

/** All implementation versions */
export interface AllVersions {
	canonical: CanonicalVersions;
	oxc: OxcVersions;
	biome: BiomeVersions;
	dprint: DprintVersions;
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
	dprint: {
		formatter: 'unknown',
		typescript: 'unknown',
	},
};

/** Strip a leading semver range marker (`^`/`~`/`>=`/etc.) from a package.json
 * version spec, leaving the bare `x.y.z`. `'^4.4.3' -> '4.4.3'`. */
function clean_version(spec: string | undefined): string {
	if (!spec) return 'unknown';
	const m = spec.match(/(\d+\.\d+\.\d+)/);
	return m ? m[1] : 'unknown';
}

/**
 * Load all package versions from `package.json` — the single source of truth for
 * the npm deps the bench measures against (both runtimes resolve from it; see
 * benches/js/package.json). Reads `benches/js/package.json` `dependencies`.
 */
export async function load_all_versions(): Promise<AllVersions> {
	try {
		const pkg_json_path = new URL('../package.json', import.meta.url).pathname;
		const content = await readFile(pkg_json_path, 'utf8');
		const config = JSON.parse(content) as { dependencies?: Record<string, string> };
		const deps = config.dependencies ?? {};

		return {
			canonical: {
				prettier: clean_version(deps['prettier']),
				'prettier-plugin-svelte': clean_version(deps['prettier-plugin-svelte']),
				svelte: clean_version(deps['svelte']),
				acorn: clean_version(deps['acorn']),
				'@sveltejs/acorn-typescript': clean_version(deps['@sveltejs/acorn-typescript']),
			},
			oxc: {
				'oxc-parser': clean_version(deps['oxc-parser']),
				oxfmt: clean_version(deps['oxfmt']),
			},
			biome: {
				js_api: clean_version(deps['@biomejs/js-api']),
				wasm: clean_version(deps['@biomejs/wasm-bundler']),
			},
			dprint: {
				formatter: clean_version(deps['@dprint/formatter']),
				typescript: clean_version(deps['@dprint/typescript']),
			},
		};
	} catch {
		return DEFAULT_VERSIONS;
	}
}
