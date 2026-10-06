/**
 * Which artifacts `cargo_build.ts` stamps: a leaf target's final file, and never one
 * cargo dates a dependent against.
 */

import { deepStrictEqual } from 'node:assert';

import { stamp_paths } from './cargo_build.ts';

const artifact = (kind: string[], filenames: string[], executable: string | null = null) => ({
	reason: 'compiler-artifact',
	target: { kind },
	filenames,
	executable,
	fresh: true
});

Deno.test('a bin stamps its executable', () => {
	deepStrictEqual(stamp_paths(artifact(['bin'], ['/t/corpus/tsv'], '/t/corpus/tsv')), [
		'/t/corpus/tsv'
	]);
	deepStrictEqual(
		stamp_paths(
			artifact(['bin'], ['/t/release/tsv.exe', '/t/release/tsv.pdb'], '/t/release/tsv.exe')
		),
		['/t/release/tsv.exe']
	);
});

Deno.test('a cdylib stamps its dynamic library, not the files beside it', () => {
	deepStrictEqual(stamp_paths(artifact(['cdylib'], ['/t/corpus/libtsv_ffi.so'])), [
		'/t/corpus/libtsv_ffi.so'
	]);
	deepStrictEqual(stamp_paths(artifact(['cdylib'], ['/t/napi/libtsv_napi.dylib'])), [
		'/t/napi/libtsv_napi.dylib'
	]);
	deepStrictEqual(
		stamp_paths(
			artifact(
				['cdylib'],
				['/t/release/tsv_ffi.dll', '/t/release/tsv_ffi.dll.lib', '/t/release/tsv_ffi.pdb']
			)
		),
		['/t/release/tsv_ffi.dll']
	);
});

Deno.test('a target with dependents is never stamped', () => {
	deepStrictEqual(
		stamp_paths(
			artifact(['lib'], ['/t/corpus/libtsv_cli.rlib', '/t/corpus/deps/libtsv_cli-0.rmeta'])
		),
		[]
	);
	// one unit, two outputs: dating the library forward dates the rlib's unit forward
	deepStrictEqual(
		stamp_paths(
			artifact(['cdylib', 'rlib'], ['/t/release/libtsv_wasm.so', '/t/release/libtsv_wasm.rlib'])
		),
		[]
	);
	deepStrictEqual(
		stamp_paths(artifact(['proc-macro'], ['/t/corpus/deps/libnapi_derive-0.so'])),
		[]
	);
	deepStrictEqual(
		stamp_paths(artifact(['custom-build'], ['/t/corpus/build/tsv_html-0/build-script-build'])),
		[]
	);
});

Deno.test('a message that is not an artifact stamps nothing', () => {
	deepStrictEqual(stamp_paths({ reason: 'build-finished', success: true }), []);
	deepStrictEqual(
		stamp_paths({ reason: 'build-script-executed', out_dir: '/t/corpus/build/x/out' }),
		[]
	);
	deepStrictEqual(stamp_paths(null), []);
	deepStrictEqual(stamp_paths('compiler-artifact'), []);
});
