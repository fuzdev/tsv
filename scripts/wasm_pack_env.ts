/**
 * What a `wasm-pack` child reads from the host: the cargo build it drives, and the
 * `wasm-bindgen` and `wasm-opt` it runs after. Like a cargo child (`scripts/cargo_env.ts`), it
 * gets gro's base environment (`PATH`, `HOME`, `TMPDIR`, `TERM`, `LC_ALL=C`, `NO_COLOR`,
 * `FORCE_COLOR`) and the names a root lists. A wasm build through a script and one through a
 * plain `wasm-pack` npm script share cargo's `target/` and find each other's output fresh only
 * when the list holds every variable that build reads, so it is cargo's whole list plus
 * wasm-pack's own.
 *
 * @module
 */

import { CARGO_PASS_ENV } from './cargo_env.ts';

/**
 * The host variables a `wasm-pack` child is given beside gro's base environment:
 *
 * - `CARGO_PASS_ENV` - the cargo build wasm-pack runs, and rustup, which it asks for the wasm
 *   target
 * - `WASM_PACK_*` - wasm-pack's own (`WASM_PACK_CACHE`, where it keeps the `wasm-bindgen` and
 *   `wasm-opt` it downloads)
 * - `XDG_CACHE_HOME` - that cache's default parent (`$XDG_CACHE_HOME/.wasm-pack`, else
 *   `$HOME/.cache/.wasm-pack`)
 * - `WASM_BINDGEN_*` - read by the `wasm-bindgen` macros as they expand
 *   (`WASM_BINDGEN_USE_JS_SYS`) and by the `wasm-bindgen` CLI (`WASM_BINDGEN_EXTERNREF`,
 *   `WASM_BINDGEN_MULTI_VALUE`), each of which changes the bundle
 * - `BINARYEN_*` - read by `wasm-opt` (`BINARYEN_CORES`, its threads; the `BINARYEN_*DEBUG*`
 *   switches)
 *
 * Not given: the rest of the host's environment, `CI`, `GITHUB_*`, and `RUST_LOG` (the
 * `wasm-bindgen` CLI's log level) included.
 */
export const WASM_PACK_PASS_ENV: ReadonlyArray<string> = [
	...CARGO_PASS_ENV,
	'WASM_PACK_*',
	'XDG_CACHE_HOME',
	'WASM_BINDGEN_*',
	'BINARYEN_*'
];
