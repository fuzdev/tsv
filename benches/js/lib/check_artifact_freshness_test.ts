/**
 * Tests for which native artifact the freshness guard grades (`native_artifact_check_for`):
 * per runtime, per N-API profile, and per `TSV_FFI_PROFILE` value, the label, path, rebuild
 * hint and PGO-stamp requirement it hands the guard. The inputs are passed in, so neither
 * the runtime nor the environment of the test process decides anything, and nothing is
 * built or read.
 */

import assert from 'node:assert';
import { test } from 'vitest';
import { native_artifact_check_for } from './check_artifact_freshness.ts';
import { pgo_bench_library_path } from './pgo_bench.ts';
import { ffi_library_path, napi_library_path } from './tsv_artifacts.ts';

const PGO_NAPI = {
	label: 'N-API (pgo)',
	path: pgo_bench_library_path('napi'),
	binding_crates: ['tsv_napi'],
	rebuild: 'deno task build:bench:pgo',
	pgo_world: 'napi'
};

test('Node and Bun with no profile grade the profile-guided addon, its stamp required', () => {
	for (const runtime of ['node', 'bun'] as const) {
		assert.deepStrictEqual(native_artifact_check_for(runtime, undefined, undefined), PGO_NAPI);
		// the C FFI's variable is the Deno row's alone
		assert.deepStrictEqual(native_artifact_check_for(runtime, undefined, 'corpus'), PGO_NAPI);
	}
});

test("a profile grades that cargo profile's plain addon, with no PGO stamp to name it", () => {
	for (const runtime of ['node', 'bun'] as const) {
		assert.deepStrictEqual(native_artifact_check_for(runtime, 'corpus', undefined), {
			label: 'N-API (corpus)',
			path: napi_library_path('corpus'),
			binding_crates: ['tsv_napi'],
			rebuild: 'deno task build:napi:corpus'
		});
		assert.deepStrictEqual(native_artifact_check_for(runtime, 'napi', 'release'), {
			label: 'N-API (napi)',
			path: napi_library_path('napi'),
			binding_crates: ['tsv_napi'],
			rebuild: 'deno task build:napi'
		});
	}
});

test('Deno grades the C-FFI library TSV_FFI_PROFILE names', () => {
	assert.deepStrictEqual(native_artifact_check_for('deno', undefined, undefined), {
		label: 'FFI (pgo)',
		path: pgo_bench_library_path('ffi'),
		binding_crates: ['tsv_ffi'],
		rebuild: 'deno task build:bench:pgo',
		pgo_world: 'ffi'
	});
	assert.deepStrictEqual(native_artifact_check_for('deno', undefined, 'corpus'), {
		label: 'FFI (corpus)',
		path: ffi_library_path('corpus'),
		binding_crates: ['tsv_ffi'],
		rebuild: 'deno task build:ffi:corpus'
	});
});

test('Deno refuses an N-API profile rather than reading it as a second selector', () => {
	assert.throws(
		() => native_artifact_check_for('deno', 'corpus', undefined),
		/profile 'corpus' selects the N-API addon, which Deno does not load/
	);
	assert.throws(() => native_artifact_check_for('deno', 'corpus', 'corpus'), /TSV_FFI_PROFILE/);
});
