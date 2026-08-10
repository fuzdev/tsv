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

/**
 * TypeScript compiler version (the `tsc` parse row + the tsc-corpus harvest's
 * validity oracle). 6.x is the last JS implementation — 7.x is the Go port, whose
 * npm package ships a binary with no in-process parser API.
 */
export interface TscVersions {
	typescript: string;
}

/**
 * yuku-parser implementation versions. Two npm packages, one Zig engine behind
 * two bindings — they version in lockstep upstream, but both are read so a
 * skewed local install shows up in the report instead of hiding.
 */
export interface YukuVersions {
	/** The N-API package (`yuku-parser`) */
	parser: string;
	/** The WASM package (`@yuku-parser/wasm`) */
	wasm: string;
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

/** rsvelte-fmt implementation versions (the CLI package; the platform binary
 * tracks it in lockstep via `optionalDependencies`) */
export interface RsvelteVersions {
	fmt: string;
}

/**
 * rsvelte parse versions — the N-API addon, a DIFFERENT package from
 * `@rsvelte/fmt` above and versioned independently of it. Its `VERSION` export
 * additionally names the upstream Svelte it targets, reported separately by
 * `lib/rsvelte_parse.ts` so a drift from the harness's `svelte` pin is visible.
 */
export interface RsvelteParseVersions {
	native: string;
}

/** swc version (`@swc/core` — the N-API parse row) */
export interface SwcVersions {
	core: string;
}

/** malva version (`dprint-plugin-malva`, over the shared `@dprint/formatter` host) */
export interface MalvaVersions {
	/** The CSS plugin itself — the version worth citing */
	malva: string;
}

/** postcss version (the `parse/css` alternative engine) */
export interface PostcssVersions {
	postcss: string;
}

/** All implementation versions */
export interface AllVersions {
	canonical: CanonicalVersions;
	oxc: OxcVersions;
	tsc: TscVersions;
	yuku: YukuVersions;
	biome: BiomeVersions;
	dprint: DprintVersions;
	rsvelte: RsvelteVersions;
	rsvelte_parse: RsvelteParseVersions;
	swc: SwcVersions;
	malva: MalvaVersions;
	postcss: PostcssVersions;
}

/** Default versions when loading fails */
const DEFAULT_VERSIONS: AllVersions = {
	canonical: {
		prettier: 'unknown',
		'prettier-plugin-svelte': 'unknown',
		svelte: 'unknown',
		acorn: 'unknown',
		'@sveltejs/acorn-typescript': 'unknown'
	},
	oxc: {
		'oxc-parser': 'unknown',
		oxfmt: 'unknown'
	},
	tsc: {
		typescript: 'unknown'
	},
	yuku: {
		parser: 'unknown',
		wasm: 'unknown'
	},
	biome: {
		js_api: 'unknown',
		wasm: 'unknown'
	},
	dprint: {
		formatter: 'unknown',
		typescript: 'unknown'
	},
	rsvelte: {
		fmt: 'unknown'
	},
	rsvelte_parse: {
		native: 'unknown'
	},
	swc: {
		core: 'unknown'
	},
	malva: {
		malva: 'unknown'
	},
	postcss: {
		postcss: 'unknown'
	}
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
				'@sveltejs/acorn-typescript': clean_version(deps['@sveltejs/acorn-typescript'])
			},
			oxc: {
				'oxc-parser': clean_version(deps['oxc-parser']),
				oxfmt: clean_version(deps['oxfmt'])
			},
			tsc: {
				typescript: clean_version(deps['typescript'])
			},
			yuku: {
				parser: clean_version(deps['yuku-parser']),
				wasm: clean_version(deps['@yuku-parser/wasm'])
			},
			biome: {
				js_api: clean_version(deps['@biomejs/js-api']),
				wasm: clean_version(deps['@biomejs/wasm-bundler'])
			},
			dprint: {
				formatter: clean_version(deps['@dprint/formatter']),
				typescript: clean_version(deps['@dprint/typescript'])
			},
			rsvelte: {
				fmt: clean_version(deps['@rsvelte/fmt'])
			},
			rsvelte_parse: {
				native: clean_version(deps['@rsvelte/vite-plugin-svelte-native'])
			},
			swc: {
				core: clean_version(deps['@swc/core'])
			},
			malva: {
				malva: clean_version(deps['dprint-plugin-malva'])
			},
			postcss: {
				postcss: clean_version(deps['postcss'])
			}
		};
	} catch {
		return DEFAULT_VERSIONS;
	}
}
