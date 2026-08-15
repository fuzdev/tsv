/**
 * Centralized version loading from package.json
 *
 * Single source of truth for all package versions used in benchmarks.
 */

import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

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

/**
 * The `x.y.z` (plus any prerelease tail) of `name`'s `dependencies` entry, with any
 * semver range marker (`^`/`~`/`>=`/etc.) stripped — `'^4.4.3' -> '4.4.3'`,
 * `'2.0.0-alpha.3' -> '2.0.0-alpha.3'`.
 *
 * The prerelease tail is KEPT because this file has a second reader that keeps it:
 * `check_node_modules.ts`'s `EXACT_PIN` treats `2.0.0-alpha.3` as an exact pin and
 * compares the whole spec against the installed `version`. Dropping the tail here
 * would make the two readers disagree about what one line of `package.json` says —
 * the install check passing while this labels the report, the prettier cache key,
 * and the fixtures gates' oracle-skew check with a version that was never
 * installed. Keep the two spellings in step.
 *
 * THROWS when the entry is absent or carries no version. Every name read here is a
 * hard `dependencies` entry, so a miss is always a bug — a renamed or dropped
 * package, or a typo in the key. Degrading to a literal `'unknown'` instead would
 * publish that bug: the version lands in the report header, in the prettier cache
 * key, and in the fixtures gates' oracle-skew check, all of which then compare
 * against a string that describes nothing.
 */
function dep_version(deps: Record<string, string>, name: string): string {
	const spec = deps[name];
	if (!spec) {
		throw new Error(
			`benches/js/package.json has no \`dependencies\` entry for '${name}' — the harness reads ` +
				`it by name, so a rename or removal must update lib/versions.ts in the same change`
		);
	}
	const m = spec.match(/(\d+\.\d+\.\d+(?:-[\w.]+)?)/);
	if (!m) {
		throw new Error(`benches/js/package.json '${name}' version '${spec}' has no x.y.z to read`);
	}
	return m[1];
}

/**
 * The raw `dependencies` map from `benches/js/package.json` — SPECS as authored
 * (`'^4.4.3'`, `'3.9.6'`), not versions.
 *
 * The pins file has three readers asking three different questions — what version
 * labels a report (below), does the install match the pin
 * (`check_node_modules.ts`), and what version to force-fetch the oxc wasi binding
 * at (`install_deps.ts`) — and each one previously spelled out the path, the read
 * and the cast for itself. One spelling here means the file's LOCATION and SHAPE
 * are stated once; each caller still owns its own question and its own failure
 * posture.
 *
 * THROWS if the file can't be read or parsed. A missing `dependencies` key yields
 * `{}` rather than throwing — that is a well-formed manifest making a claim (no
 * deps), and the callers each have a better answer for it than a shared one could.
 */
export async function read_dependency_pins(): Promise<Record<string, string>> {
	const pkg_json_path = fileURLToPath(new URL('../package.json', import.meta.url));
	const content = await readFile(pkg_json_path, 'utf8');
	return (JSON.parse(content) as { dependencies?: Record<string, string> }).dependencies ?? {};
}

/**
 * Load all package versions from `package.json` — the single source of truth for
 * the npm deps the bench measures against (both runtimes resolve from it; see
 * benches/js/package.json). Reads `benches/js/package.json` `dependencies`.
 *
 * THROWS if that file can't be read or parsed, or if any name below is missing
 * from `dependencies` (see `dep_version`). There is no defaulted result: these
 * versions label a committed report, key the prettier cache, and back the fixtures
 * gates' oracle-skew check, so a run that can't read them must stop rather than
 * proceed under placeholder labels.
 */
export async function load_all_versions(): Promise<AllVersions> {
	const deps = await read_dependency_pins();

	return {
		canonical: {
			prettier: dep_version(deps, 'prettier'),
			'prettier-plugin-svelte': dep_version(deps, 'prettier-plugin-svelte'),
			svelte: dep_version(deps, 'svelte'),
			acorn: dep_version(deps, 'acorn'),
			'@sveltejs/acorn-typescript': dep_version(deps, '@sveltejs/acorn-typescript')
		},
		oxc: {
			'oxc-parser': dep_version(deps, 'oxc-parser'),
			oxfmt: dep_version(deps, 'oxfmt')
		},
		tsc: {
			typescript: dep_version(deps, 'typescript')
		},
		yuku: {
			parser: dep_version(deps, 'yuku-parser'),
			wasm: dep_version(deps, '@yuku-parser/wasm')
		},
		biome: {
			js_api: dep_version(deps, '@biomejs/js-api'),
			wasm: dep_version(deps, '@biomejs/wasm-bundler')
		},
		dprint: {
			formatter: dep_version(deps, '@dprint/formatter'),
			typescript: dep_version(deps, '@dprint/typescript')
		},
		rsvelte: {
			fmt: dep_version(deps, '@rsvelte/fmt')
		},
		rsvelte_parse: {
			native: dep_version(deps, '@rsvelte/vite-plugin-svelte-native')
		},
		swc: {
			core: dep_version(deps, '@swc/core')
		},
		malva: {
			malva: dep_version(deps, 'dprint-plugin-malva')
		},
		postcss: {
			postcss: dep_version(deps, 'postcss')
		}
	};
}
