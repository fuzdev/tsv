/**
 * Tests for which native artifact the freshness guard grades (`native_artifact_check_for`):
 * per runtime and per N-API profile, the label, path, rebuild hint and PGO-stamp requirement
 * it hands the guard; and that the C-FFI loader's rebuild hints name commands that exist. The
 * inputs are passed in, so neither the runtime nor the environment of the test process
 * decides anything, and nothing is built.
 */

import assert from 'node:assert';
import { readFileSync } from 'node:fs';
import { test } from 'vitest';
import { native_artifact_check_for } from './check_artifact_freshness.ts';
import { pgo_bench_library_path } from './pgo_bench.ts';
import { ffi_rebuild_task, napi_library_path } from './tsv_artifacts.ts';

const PGO_NAPI = {
	label: 'N-API (pgo)',
	path: pgo_bench_library_path('napi'),
	binding_crates: ['tsv_napi'],
	rebuild: 'npm run build:bench:pgo',
	pgo_world: 'napi'
};

test('Node and Bun with no profile grade the profile-guided addon, its stamp required', () => {
	for (const runtime of ['node', 'bun'] as const) {
		assert.deepStrictEqual(native_artifact_check_for(runtime, undefined), PGO_NAPI);
	}
});

test("a profile grades that cargo profile's plain addon, with no PGO stamp to name it", () => {
	for (const runtime of ['node', 'bun'] as const) {
		assert.deepStrictEqual(native_artifact_check_for(runtime, 'corpus'), {
			label: 'N-API (corpus)',
			path: napi_library_path('corpus'),
			binding_crates: ['tsv_napi'],
			rebuild: 'npm run build:napi:corpus'
		});
		assert.deepStrictEqual(native_artifact_check_for(runtime, 'napi'), {
			label: 'N-API (napi)',
			path: napi_library_path('napi'),
			binding_crates: ['tsv_napi'],
			rebuild: 'npm run build:napi'
		});
	}
});

test('Deno grades the profile-guided C-FFI library the bench row loads', () => {
	assert.deepStrictEqual(native_artifact_check_for('deno', undefined), {
		label: 'FFI (pgo)',
		path: pgo_bench_library_path('ffi'),
		binding_crates: ['tsv_ffi'],
		rebuild: 'npm run build:bench:pgo',
		pgo_world: 'ffi'
	});
});

test('Deno refuses an N-API profile rather than reading it as a selector', () => {
	assert.throws(
		() => native_artifact_check_for('deno', 'corpus'),
		/profile 'corpus' selects the N-API addon, which Deno does not load/
	);
});

test("each rebuild hint for the C-FFI loader's libraries names a task that exists", () => {
	const tasks = {
		'deno task': new Set(
			Object.keys(
				JSON.parse(readFileSync(new URL('../../../deno.json', import.meta.url), 'utf8')).tasks
			)
		),
		'npm run': new Set(
			Object.keys(
				JSON.parse(readFileSync(new URL('../../../package.json', import.meta.url), 'utf8')).scripts
			)
		)
	};
	for (const profile of [undefined, 'release', 'corpus']) {
		const hint = ffi_rebuild_task(profile);
		const [, runner, task] = /^(deno task|npm run) (\S+)/.exec(hint) ?? [];
		assert.ok(
			runner !== undefined && tasks[runner as keyof typeof tasks].has(task!),
			`${hint} names no task its runner runs`
		);
	}
	assert.strictEqual(
		ffi_rebuild_task('corpus'),
		'npm run build:stamped -- -p tsv_ffi --profile corpus'
	);
});
