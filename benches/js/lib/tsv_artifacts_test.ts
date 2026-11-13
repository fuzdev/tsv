/**
 * Tests for the N-API addon's paths and rebuild hints (`tsv_artifacts.ts`): which file a
 * profile names, which one none names (the profile-guided addon the bench rows load), and
 * the task that rebuilds each. Pure functions over the checkout's layout, so no build is
 * needed to run them.
 */

import assert from 'node:assert';
import { fileURLToPath } from 'node:url';
import { test } from 'vitest';
import { native_library_filename } from './runtime.ts';
import {
	napi_library_path,
	napi_loaded_path,
	napi_rebuild_task,
	TSV_ARTIFACTS
} from './tsv_artifacts.ts';

const ROOT = fileURLToPath(new URL('../../..', import.meta.url)).replace(/\/$/, '');
const ADDON = native_library_filename('tsv_napi');

test("a profile's addon is cargo's plain build under target/<profile>", () => {
	assert.strictEqual(napi_library_path('corpus'), `${ROOT}/target/corpus/${ADDON}`);
	assert.strictEqual(napi_library_path('napi'), `${ROOT}/target/napi/${ADDON}`);
	assert.strictEqual(napi_loaded_path('corpus'), `${ROOT}/target/corpus/${ADDON}`);
	assert.strictEqual(napi_loaded_path('napi'), `${ROOT}/target/napi/${ADDON}`);
});

test('no profile loads the profile-guided addon the bench rows measure', () => {
	assert.strictEqual(napi_loaded_path(undefined), `${ROOT}/.pgo/bench/${ADDON}`);
	// the size table's row and the loader's default are one file
	assert.strictEqual(napi_loaded_path(undefined), TSV_ARTIFACTS.tsv_napi.path);
});

test("each profile's rebuild hint names the task that builds its file", () => {
	assert.strictEqual(napi_rebuild_task(undefined), 'npm run build:bench:pgo');
	assert.strictEqual(napi_rebuild_task(undefined), TSV_ARTIFACTS.tsv_napi.rebuild);
	assert.strictEqual(napi_rebuild_task('napi'), 'npm run build:napi');
	assert.strictEqual(napi_rebuild_task('corpus'), 'npm run build:napi:corpus');
});
