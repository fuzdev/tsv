/**
 * Which host variables reach a `wasm-pack` child (`WASM_PACK_PASS_ENV`): everything a cargo
 * child gets, what wasm-pack, `wasm-bindgen` and `wasm-opt` read, and nothing beyond.
 */

import { strictEqual } from 'node:assert';
import { test } from 'vitest';

import { CARGO_PASS_ENV } from './cargo_env.ts';
import { WASM_PACK_PASS_ENV } from './wasm_pack_env.ts';

/** Whether `name` is passed, by gro's `pass_env` rule: an exact name, or a `PREFIX_*` glob. */
const passes = (name: string): boolean =>
	WASM_PACK_PASS_ENV.some((entry) =>
		entry.endsWith('*') ? name.startsWith(entry.slice(0, -1)) : name === entry
	);

test("every name a cargo child gets reaches wasm-pack's cargo", () => {
	for (const entry of CARGO_PASS_ENV) strictEqual(WASM_PACK_PASS_ENV.includes(entry), true, entry);
});

test('what wasm-pack and the tools it runs read reach it', () => {
	for (const name of [
		'WASM_PACK_CACHE',
		'XDG_CACHE_HOME',
		'WASM_BINDGEN_USE_JS_SYS',
		'WASM_BINDGEN_EXTERNREF',
		'WASM_BINDGEN_MULTI_VALUE',
		'BINARYEN_CORES',
		'BINARYEN_PASS_DEBUG',
		'CARGO_HOME',
		'RUSTUP_TOOLCHAIN',
		'RUSTFLAGS',
		'RUSTC_BOOTSTRAP'
	]) {
		strictEqual(passes(name), true, name);
	}
});

test('nothing beyond the build reaches wasm-pack', () => {
	for (const name of [
		'CI',
		'GITHUB_TOKEN',
		'NODE_AUTH_TOKEN',
		'NPM_TOKEN',
		'TSV_BUILD_FORCE',
		'SSH_AUTH_SOCK',
		'RUST_LOG',
		'WASM',
		'XDG_CONFIG_HOME',
		'LD_PRELOAD'
	]) {
		strictEqual(passes(name), false, name);
	}
});
