/**
 * Which host variables reach a script's cargo child (`CARGO_PASS_ENV`): the ones a caller or CI
 * sets on purpose and the families a build script reruns on, and nothing beyond cargo's own.
 */

import { strictEqual } from 'node:assert';
import { test } from 'vitest';

import { CARGO_PASS_ENV } from './cargo_env.ts';

/** Whether `name` is passed, by gro's `pass_env` rule: an exact name, or a `PREFIX_*` glob. */
const passes = (name: string): boolean =>
	CARGO_PASS_ENV.some((entry) =>
		entry.endsWith('*') ? name.startsWith(entry.slice(0, -1)) : name === entry
	);

test('the variables a caller or CI sets for a build reach cargo', () => {
	for (const name of [
		'CARGO_HOME',
		'CARGO_TARGET_DIR',
		'CARGO_BUILD_JOBS',
		'CARGO_ENCODED_RUSTFLAGS',
		'CARGO_TERM_COLOR',
		'CARGO_INCREMENTAL',
		'CARGO_PROFILE_RELEASE_LTO',
		'RUSTFLAGS',
		'RUSTUP_TOOLCHAIN',
		'RUSTUP_HOME',
		'RUSTC_WRAPPER',
		'RUST_BACKTRACE',
		'RUST_MIN_STACK'
	]) {
		strictEqual(passes(name), true, name);
	}
});

test('the variables a build script in the graph reruns on reach cargo', () => {
	for (const name of [
		'RUSTC_BOOTSTRAP',
		'RUSTC_STAGE',
		'RUST_LIBC_UNSTABLE_GNU_TIME_BITS',
		'LIBC_BUILD_VERBOSE',
		'LIBNODE_PATH',
		'TYPE_DEF_TMP_PATH',
		'DEBUG_GENERATED_CODE',
		'NAPI_TYPE_DEF_TMP_FOLDER'
	]) {
		strictEqual(passes(name), true, name);
	}
});

test('what cargo fetches through and what wraps rustc reach cargo', () => {
	for (const name of [
		'HTTPS_PROXY',
		'https_proxy',
		'HTTP_PROXY',
		'http_proxy',
		'ALL_PROXY',
		'all_proxy',
		'NO_PROXY',
		'no_proxy',
		'HTTP_TIMEOUT',
		'SSL_CERT_FILE',
		'SSL_CERT_DIR',
		'SCCACHE_DIR',
		'SCCACHE_CACHE_SIZE'
	]) {
		strictEqual(passes(name), true, name);
	}
});

test('nothing beyond the build reaches cargo', () => {
	for (const name of [
		'CI',
		'GITHUB_TOKEN',
		'NODE_AUTH_TOKEN',
		'NPM_TOKEN',
		'SSH_AUTH_SOCK',
		'LD_PRELOAD',
		'RUST_LOG',
		'RUSTX',
		'SSL_KEY_LOG_FILE',
		'SCCACHE',
		'GITHUB_ACTIONS'
	]) {
		strictEqual(passes(name), false, name);
	}
});
