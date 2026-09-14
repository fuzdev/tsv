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

/**
 * The oxc WASM binding — pinned in `package.json`'s `force_installed` map rather
 * than `dependencies`, because its metadata declares `cpu: wasm32` and npm will not
 * install it on any other host; `install_deps.ts` force-fetches it with `--no-save`.
 * Pinned APART from `oxc-parser` (see `package.json`'s `//oxc-wasi` note).
 *
 * Exported as the ONE spelling of a name that several sites must agree on and no
 * `dependencies` entry records: this module labels the report with its pin,
 * `check_node_modules.ts` grades the install against it, and `binary_sizes.ts`
 * reads its `.wasm` for the size table. A rename upstream that reached only some of
 * them would leave the rest silently finding nothing — an ungraded install, or a
 * size row that vanishes into `binary_sizes_absent`. (`lib/oxc_wasm.ts` keeps its
 * own literals: those are import specifiers, which must stay statically analyzable.)
 */
export const OXC_WASI_BINDING = '@oxc-parser/binding-wasm32-wasi';

/**
 * The oxc-parser WASM binding's version — its own `force_installed` pin, so the
 * `oxc-parser-wasm` row is labeled with the binding it actually loads rather than
 * with the native package's pin.
 */
export interface OxcWasmVersions {
	binding: string;
}

/** All implementation versions */
export interface AllVersions {
	canonical: CanonicalVersions;
	oxc: OxcVersions;
	oxc_wasm: OxcWasmVersions;
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

/** The two pin maps `benches/js/package.json` carries. */
type PinSection = 'dependencies' | 'force_installed';

/**
 * An exact pin (`3.9.6`, or a prerelease like `2.0.0-alpha.3`) — the only kind an
 * installed version can be graded against (`check_node_modules.ts`), and the only
 * kind `force_installed` admits (`read_force_installed_pins`).
 *
 * The prerelease tail is matched because a prerelease pin is still EXACT: it names
 * one version, which is the whole property this check rests on. Reading it as a
 * range instead would drop that dep out of the sweep silently, and prereleases are
 * live in this dependency graph (see `package.json`'s `//oxc-wasi` note).
 */
export const EXACT_PIN = /^\d+\.\d+\.\d+(?:-[\w.]+)?$/;

/**
 * The `x.y.z` (plus any prerelease tail) of `name`'s entry in `section`, with any
 * semver range marker (`^`/`~`/`>=`/etc.) stripped — `'^4.4.3' -> '4.4.3'`,
 * `'2.0.0-alpha.3' -> '2.0.0-alpha.3'`.
 *
 * The prerelease tail is KEPT because this file has a second reader that keeps it:
 * `check_node_modules.ts` treats `2.0.0-alpha.3` as an exact pin (`EXACT_PIN`) and
 * compares the whole spec against the installed `version`. Dropping the tail here
 * would make the two readers disagree about what one line of `package.json` says —
 * the install check passing while this labels the report, the prettier cache key,
 * and the fixtures gates' oracle-skew check with a version that was never
 * installed. Keep the two spellings in step.
 *
 * THROWS when the entry is absent or carries no version. Every name read here is a
 * hard entry of its section (`dependencies` or `force_installed`), so a miss is
 * always a bug — a renamed or dropped package, or a typo in the key. Degrading to a
 * literal `'unknown'` instead would publish that bug: the version lands in the
 * report header, in the prettier cache key, and in the fixtures gates' oracle-skew
 * check, all of which then compare against a string that describes nothing.
 */
function dep_version(
	deps: Record<string, string>,
	name: string,
	section: PinSection = 'dependencies'
): string {
	const spec = deps[name];
	if (!spec) {
		throw new Error(
			`benches/js/package.json has no \`${section}\` entry for '${name}' — the harness reads ` +
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
 * (`check_node_modules.ts`), and what to force-fetch (`install_deps.ts`, through
 * `read_force_installed_pins`) — each of which would otherwise spell out the path, the read
 * and the cast for itself. One spelling here means the file's LOCATION and SHAPE
 * are stated once; each caller still owns its own question and its own failure
 * posture.
 *
 * THROWS if the file can't be read or parsed. A missing `dependencies` key yields
 * `{}` rather than throwing — that is a well-formed manifest making a claim (no
 * deps), and the callers each have a better answer for it than a shared one could.
 */
export async function read_dependency_pins(): Promise<Record<string, string>> {
	return (await read_package_pins()).dependencies ?? {};
}

/**
 * The raw `force_installed` map from `benches/js/package.json` — the packages npm
 * will not install as ordinary deps, which `install_deps.ts` force-fetches with
 * `--no-save` at these pins (today only `OXC_WASI_BINDING`). Same posture as
 * `read_dependency_pins`: THROWS on an unreadable file, `{}` when the key is absent.
 *
 * Every entry must be an EXACT pin (`EXACT_PIN`), and this is where that is
 * enforced — the one read all three consumers share. A range here would install
 * whatever it resolves to today, be labeled in the report with the range's floor
 * (`dep_version` strips the marker), and slip past `check_node_modules.ts`, which
 * can only grade an exact pin: the three would disagree about one line of
 * `package.json`, which is the mislabeling the map exists to prevent.
 *
 * ⚠ Exactness stops at the named package. `--no-save` keeps the entry out of the
 * lockfile, so its transitive closure resolves live on every install — see the
 * `//force_installed` note in `package.json`.
 */
export async function read_force_installed_pins(): Promise<Record<string, string>> {
	const forced = (await read_package_pins()).force_installed ?? {};
	for (const [name, spec] of Object.entries(forced)) {
		if (!EXACT_PIN.test(spec)) {
			throw new Error(
				`benches/js/package.json \`force_installed\` entry '${name}' is '${spec}', not an exact ` +
					`x.y.z pin — a range would install one version and label the report with another`
			);
		}
	}
	return forced;
}

/** Both pin maps of `benches/js/package.json`, as authored. */
async function read_package_pins(): Promise<Partial<Record<PinSection, Record<string, string>>>> {
	const pkg_json_path = fileURLToPath(new URL('../package.json', import.meta.url));
	return JSON.parse(await readFile(pkg_json_path, 'utf8')) as Partial<
		Record<PinSection, Record<string, string>>
	>;
}

/**
 * Load all package versions from `package.json` — the single source of truth for
 * the npm deps the bench measures against (both runtimes resolve from it; see
 * benches/js/package.json). Reads `benches/js/package.json` `dependencies` and
 * `force_installed`.
 *
 * THROWS if that file can't be read or parsed, or if any name below is missing
 * from `dependencies` (see `dep_version`). There is no defaulted result: these
 * versions label a committed report, key the prettier cache, and back the fixtures
 * gates' oracle-skew check, so a run that can't read them must stop rather than
 * proceed under placeholder labels.
 */
export async function load_all_versions(): Promise<AllVersions> {
	const pins = await read_package_pins();
	const deps = pins.dependencies ?? {};
	const forced = pins.force_installed ?? {};

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
		oxc_wasm: {
			binding: dep_version(forced, OXC_WASI_BINDING, 'force_installed')
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
