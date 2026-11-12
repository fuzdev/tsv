/**
 * The crates that feed the tsv artifacts the bench measures and the packages ship, stated once
 * for every side that must agree on them: the run-side freshness guard
 * (`check_artifact_freshness.ts`), the artifact table (`tsv_artifacts.ts`), the build-side skip
 * (`scripts/run_if_stale.ts`), the staged-package guard (`scripts/check_staged_freshness.ts`),
 * and the package gates' own staleness checks (`scripts/engine_parity.ts`,
 * `scripts/test_napi_npm.ts`). Pure data importing nothing, so a
 * build-side script reads the lists without loading the artifact table's `node:*` modules.
 *
 * @module
 */

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
