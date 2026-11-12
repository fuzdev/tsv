/**
 * The tsv artifacts the bench measures and reports — the crates that feed them and
 * where each build lands — stated once for the readers that must agree on them: the
 * run-side freshness guard (`check_artifact_freshness.ts`), the build-side skip
 * (`scripts/run_if_stale.ts`), the staged-package guard
 * (`scripts/check_staged_freshness.ts` via the npm test suites) and the size table
 * (`binary_sizes.ts`). A path or crate list spelled in two of them is how a guard
 * ends up vouching for an artifact the table sizes from somewhere else.
 *
 * The LOADERS are readers too, and that is the half worth stating: the guard's
 * subject has to be the file the loader actually opens, so `ffi.ts`, `napi.ts` and
 * `wasm.ts` build their paths from this module's exported builders
 * ({@link ffi_library_path}, {@link wasm_bundle_dir} / {@link wasm_bundle_path}) and
 * from `pgo_bench.ts`'s `pgo_bench_library_path` (the profile-guided native pair the
 * timed rows load) rather than re-deriving the layout. Only the two AXES a loader varies stay its
 * own — the FFI profile (`TSV_FFI_PROFILE`) and the wasm-pack target — which is
 * exactly why the freshness guard's executed-vs-size-only split can be a path
 * comparison at all: an entry differing there is a real difference, not a spelling.
 *
 * The release gate is a reader too: `scripts/validate_artifacts.ts` sizes and
 * smoke-imports every `pkg/<variant>/<target>` bundle through
 * {@link wasm_bundle_dir} / {@link wasm_bundle_path}, walking `npm` as one more
 * target — a staging directory rather than a wasm-pack one, but the same two files
 * at the same place. A size gate that spelled the layout itself would keep passing
 * over the old location after a move.
 *
 * Still spelled elsewhere, and deliberately — the remaining places a layout move
 * has to be mirrored by hand: `package.json`'s wasm build scripts pass
 * `--target <path>` to `run_if_stale.ts` (a task string can't call a function),
 * `scripts/build_napi_packages.ts` carries `target/napi/…` and `target/release/…`
 * as the DEFAULTS of flags CI overrides per target, and
 * `scripts/roots/patch_npm_package.ts` opens the `npm` staging dir this table only
 * reads (which `scripts/patch_npm_package.ts` prints).
 *
 * Node-modules-free by construction (only `node:` builtins + `runtime.ts`): the
 * build-side scripts import it, and `typecheck:scripts` walks them on a bare
 * checkout — a `type` import from the size table was enough to drag that whole
 * graph in, which is why `BinaryKind` lives here rather than there.
 */

import { fileURLToPath } from 'node:url';

import { type PgoBenchWorld, pgo_bench_library_path } from './pgo_bench.ts';
import { native_library_filename } from './runtime.ts';

/**
 * Binary kind for grouping size comparisons. `js` is a minified JS bundle the
 * harness builds itself (`canonical_bundles.ts`) — the one kind no package ships.
 */
export type BinaryKind = 'wasm' | 'native' | 'js';

/**
 * Crates whose source compiles into EVERY measured tsv artifact (the shared
 * core): the language crates plus `tsv_arena` (all three bindings' per-thread
 * reuse). Applied as the freshness floor for every check.
 *
 * `tsv_ignore` + `tsv_discover` are deliberately NOT here — they feed only the
 * WASM bundle (the `IgnoreStack` export), not `tsv_ffi` / `tsv_napi`, so they
 * live in `WASM_CRATES`. Sharing them would false-stale the native checks: a
 * `tsv_discover` edit never rebuilds the FFI (it's not in its dependency
 * graph), so the guard would demand a rebuild that compiles nothing — one that
 * clears it only because the build tasks stamp (`scripts/cargo_build.ts`).
 *
 * Deliberately excludes the dev-tooling crates (`tsv_debug`, `tsv_cli`): they
 * don't feed the measured artifacts, and including them would force wasm
 * rebuilds on every fixture-workflow edit. (The staged guard's CLI-binary check
 * names `tsv_cli` itself where it needs it.)
 */
export const CORE_CRATES = ['tsv_lang', 'tsv_arena', 'tsv_html', 'tsv_ts', 'tsv_css', 'tsv_svelte'];

/**
 * Crates that feed the WASM bundle beyond `CORE_CRATES`: the binding crate
 * itself plus `tsv_ignore` + `tsv_discover` (the `IgnoreStack` export). Of the
 * measured bindings `tsv_ffi` links neither; `tsv_napi` links both (its `format`
 * feature, for the same export), which its own staleness row names
 * (`napi_addon_check` in `scripts/check_staged_freshness.ts`).
 */
export const WASM_CRATES = ['tsv_wasm', 'tsv_ignore', 'tsv_discover'];

/** One built tsv artifact: the size table's row, and a freshness check's subject. */
export interface TsvArtifact {
	/** Display label — the size table's row identity (`binary_sizes.ts` `LABELS`). */
	label: string;
	kind: BinaryKind;
	/** Absolute path of the built file. */
	path: string;
	/** Crates feeding it beyond `CORE_CRATES`. */
	binding_crates: readonly string[];
	/** The task that rebuilds it — surfaced by every staleness message. */
	rebuild: string;
	/**
	 * Which build a native row is: `pgo` for the profile-guided libraries the timed rows load
	 * (`pgo_bench.ts` — the guard also requires the stamp to name their bytes), `plain` for a
	 * size-only build no row executes. The size table publishes it beside each native size.
	 */
	build?: 'pgo' | 'plain';
	/** The PGO world whose stamp entry must name this file's bytes (a `pgo` build only). */
	pgo_world?: PgoBenchWorld;
}

const PROJECT_ROOT = fileURLToPath(new URL('../../..', import.meta.url)).replace(/\/$/, '');
const FFI_LIB = native_library_filename('tsv_ffi');

/**
 * The plain C-FFI shared library cargo builds for a profile — `corpus` for the unwinding
 * build the corpus tools load (`TSV_FFI_PROFILE`). The bench's timed row loads the
 * profile-guided library instead (`pgo_bench_library_path('ffi')`). The profile is the
 * caller's axis; the layout is this module's.
 */
export function ffi_library_path(profile: string): string {
	return `${PROJECT_ROOT}/target/${profile}/${FFI_LIB}`;
}

/**
 * The C-FFI library a process loads for a `TSV_FFI_PROFILE` value: the bench's
 * profile-guided build (`pgo_bench_library_path('ffi')`) when it is unset, else that cargo
 * profile's plain build under `target/` (the corpus tools set `corpus` for panic recovery).
 * The caller reads the variable, under whichever runtime it runs: `ffi.ts` under Deno, the
 * freshness guard under any.
 */
export function ffi_loaded_path(profile: string | undefined): string {
	return profile === undefined ? pgo_bench_library_path('ffi') : ffi_library_path(profile);
}

/** The task that rebuilds the library `ffi_loaded_path` names for the same profile. */
export function ffi_rebuild_task(profile: string | undefined): string {
	return profile === undefined
		? 'deno task build:bench:pgo'
		: profile === 'release'
			? 'deno task build:ffi'
			: `deno task build:ffi:${profile}`;
}

/**
 * The plain N-API addon cargo builds — the `napi` profile (release + unwind) — which
 * `test:napi` loads. The bench's rows load the profile-guided addon instead
 * (`pgo_bench_library_path('napi')`).
 */
export function napi_library_path(): string {
	return `${PROJECT_ROOT}/target/napi/${native_library_filename('tsv_napi')}`;
}

/** The three wasm-pack builds, by the cargo features each selects. */
export type WasmVariant = 'format' | 'parse' | 'all';

/**
 * A wasm-pack output directory, holding `tsv_wasm_bg.wasm` beside the `tsv_wasm.js`
 * glue — so the guard's subject and the loader's import resolve from one expression.
 * The target is the caller's axis (Deno loads `deno`, Node/Bun `nodejs`); the size
 * table pins `deno`, since the `.wasm` is the same bytes under either.
 *
 * `npm` is a fourth value the release gate passes: a STAGING directory rather than
 * a wasm-pack target (`scripts/patch_npm_package.ts` writes it), but it holds the
 * same two files at the same place, so it is one more target here rather than a
 * second spelling of the layout.
 */
export function wasm_bundle_dir(variant: WasmVariant, target: string): string {
	return `${PROJECT_ROOT}/crates/tsv_wasm/pkg/${variant}/${target}`;
}

/** The compiled `.wasm` in that directory — the size table's row and the guard's subject. */
export function wasm_bundle_path(variant: WasmVariant, target: string): string {
	return `${wasm_bundle_dir(variant, target)}/tsv_wasm_bg.wasm`;
}

/**
 * Every tsv build the bench REPORTS. Which of these a runtime EXECUTES is the
 * freshness guard's question (`executed_artifact_checks`); the size table reports
 * all seven from every runtime, existence-gated rather than registry-gated, so the
 * comparison across bindings stays alive on the runtime that loads the other one.
 * The three WASM rows size the `deno`-target bundles on every runtime: the `.wasm`
 * is the same bytes under each wasm-pack target, only the JS glue differs.
 */
export const TSV_ARTIFACTS = {
	// The profile-guided library the Deno row loads, as `build:bench:pgo` staged it.
	tsv_ffi: {
		label: 'tsv (ffi)',
		kind: 'native',
		path: pgo_bench_library_path('ffi'),
		binding_crates: ['tsv_ffi'],
		rebuild: 'deno task build:bench:pgo',
		build: 'pgo',
		pgo_world: 'ffi'
	},
	// The native mirror of @fuzdev/tsv-format-wasm: dropping the convert/JSON layer
	// (and the parse exports) leaves a scope-matched comparison against oxfmt
	// (napi), which is format-only too. A separate target dir so it doesn't
	// clobber the full library the perf rows load.
	tsv_format_ffi: {
		label: 'tsv format (ffi)',
		kind: 'native',
		path: `${PROJECT_ROOT}/target/ffi-format/release/${FFI_LIB}`,
		binding_crates: ['tsv_ffi'],
		rebuild: 'deno task build:ffi:format',
		build: 'plain'
	},
	// The native mirror of @fuzdev/tsv-parse-wasm: keeps the parse exports + the
	// convert/JSON layer and drops the printers, scope-matched to oxc-parser
	// (napi), which also materializes a JSON AST.
	tsv_parse_ffi: {
		label: 'tsv parse (ffi)',
		kind: 'native',
		path: `${PROJECT_ROOT}/target/ffi-parse/release/${FFI_LIB}`,
		binding_crates: ['tsv_ffi'],
		rebuild: 'deno task build:ffi:parse',
		build: 'plain'
	},
	// The Node/Bun native path, the sibling of the FFI library Deno loads. Same
	// engine, different binding boundary; the profile-guided addon `build:bench:pgo`
	// staged — the shipped `.node` is the same pipeline's output, renamed.
	tsv_napi: {
		label: 'tsv (napi)',
		kind: 'native',
		path: pgo_bench_library_path('napi'),
		binding_crates: ['tsv_napi'],
		rebuild: 'deno task build:bench:pgo',
		build: 'pgo',
		pgo_world: 'napi'
	},
	// Three WASM builds from one crate via the `format`/`parse` features:
	// @fuzdev/tsv-format-wasm, @fuzdev/tsv-parse-wasm, and @fuzdev/tsv-wasm (both —
	// the bundle the bench executes).
	tsv_format_wasm: {
		label: 'tsv-format-wasm',
		kind: 'wasm',
		path: wasm_bundle_path('format', 'deno'),
		binding_crates: WASM_CRATES,
		rebuild: 'deno task build:wasm:deno'
	},
	tsv_parse_wasm: {
		label: 'tsv-parse-wasm',
		kind: 'wasm',
		path: wasm_bundle_path('parse', 'deno'),
		binding_crates: WASM_CRATES,
		rebuild: 'deno task build:wasm:parse:deno'
	},
	tsv_wasm: {
		label: 'tsv-wasm',
		kind: 'wasm',
		path: wasm_bundle_path('all', 'deno'),
		binding_crates: WASM_CRATES,
		rebuild: 'deno task build:wasm:all:deno'
	}
} as const satisfies Record<string, TsvArtifact>;
