/**
 * The PGO gates' readers (`scripts/pgo/profile.ts`) against tool output captured from
 * the pinned toolchain's `llvm-profdata` / `llvm-readobj` and a `-Cprofile-use` build
 * log — each gate's two directions, so a reader that stopped matching reads as a
 * failure here rather than as a gate that silently passes.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';

import {
	demangle_legacy_path,
	floor_of,
	grade_profile,
	has_instrumentation_sections,
	HOT_FUNCTIONS,
	type HotFunction,
	llvm_version,
	parse_pgo_diagnostics,
	parse_profile_show,
	profile_instrumentation,
	profile_use_crates,
	strip_ansi,
	type TrainedCounts
} from './profile.ts';

Deno.test('llvm_version reads both spellings', () => {
	strictEqual(
		llvm_version('LLVM (http://llvm.org/):\n  LLVM version 21.1.8-rust-1.94.1-stable\n'),
		'21.1.8'
	);
	strictEqual(llvm_version('binary: rustc\nhost: x86_64\nLLVM version: 21.1.8\n'), '21.1.8');
	strictEqual(llvm_version('no version here'), null);
});

Deno.test('demangle_legacy_path', () => {
	strictEqual(
		demangle_legacy_path('_ZN6tsv_ts9__private12parse_folded17hbed61a5168c09c4aE'),
		'tsv_ts::__private::parse_folded'
	);
	// a local symbol's `file;` prefix, and Mach-O's extra underscore
	strictEqual(
		demangle_legacy_path(
			'tsv_lang.0a1b-cgu.0;_ZN8tsv_lang3doc12arena_render20render_doc_iterative17h0123456789abcdefE'
		),
		'tsv_lang::doc::arena_render::render_doc_iterative'
	);
	strictEqual(
		demangle_legacy_path('__ZN7tsv_css9__private5parse17hf2f2b828515a6dbeE'),
		'tsv_css::__private::parse'
	);
	// a generic component keeps its escaped spelling
	strictEqual(
		demangle_legacy_path(
			'_ZN102_$LT$core..iter..adapters..map..Map$LT$I$C$F$GT$$u20$as$u20$core..iter..traits..iterator..Iterator$GT$4next17h9df72be119ec53dcE'
		),
		'_$LT$core..iter..adapters..map..Map$LT$I$C$F$GT$$u20$as$u20$core..iter..traits..iterator..Iterator$GT$::next'
	);
	strictEqual(demangle_legacy_path('main'), null);
	strictEqual(demangle_legacy_path('_ZN6tsv_ts'), null);
});

/** A `show --all-functions --counts` listing with the given records (`[mangled, counts]`),
 * the entry counted first (`-pgo-instrument-entry`) unless `entry_first` is 0. */
const show = (records: [string, number[]][], entry_first = 1): string =>
	'Counters:\n' +
	records
		.map(
			([name, counts]) =>
				`  ${name}:\n    Hash: 0x0a2898d4450365c0\n    Counters: ${counts.length}\n` +
				`    Block counts: [${counts.join(', ')}]\n`
		)
		.join('') +
	`Instrumentation level: IR  entry_first = ${entry_first}  instrument_loop_entries = 0\n` +
	`Functions shown: ${records.length}\n`;

/** The legacy mangling of a `::` path, as the profile stores it. */
const mangle = (path: string): string =>
	`_ZN${path
		.split('::')
		.map((p) => `${p.length}${p}`)
		.join('')}17h0123456789abcdefE`;

Deno.test('parse_profile_show takes the largest count and the entry count per record', () => {
	const records = parse_profile_show(
		show([
			['_ZN1a1b17h0123456789abcdefE', [2, 7, 3]],
			['x.cgu.0;_ZN1a1c17h0123456789abcdefE', [0, 0]]
		]) + '  _ZN1a1d17h0123456789abcdefE:\n    Hash: 0x1\n    Function count: 9\n'
	);
	deepStrictEqual(records, [
		{ name: '_ZN1a1b17h0123456789abcdefE', max_count: 7, entry_count: 2 },
		{ name: 'x.cgu.0;_ZN1a1c17h0123456789abcdefE', max_count: 0, entry_count: 0 },
		{ name: '_ZN1a1d17h0123456789abcdefE', max_count: 9, entry_count: 9 }
	]);
	// without the entry counted first, the first counter is some edge: no entry count
	deepStrictEqual(
		parse_profile_show(show([['_ZN1a1b17h0123456789abcdefE', [2, 7]]], 0)).map(
			(r) => r.entry_count
		),
		[null]
	);
});

Deno.test('profile_instrumentation reads the header line', () => {
	deepStrictEqual(profile_instrumentation(show([], 1)), { ir: true, entry_first: true });
	deepStrictEqual(profile_instrumentation(show([], 0)), { ir: true, entry_first: false });
	strictEqual(profile_instrumentation('Counters:\n'), null);
});

const TRAINED: TrainedCounts = {
	files: 10,
	typescript: 5,
	svelte: 3,
	css: 2,
	strip: 4,
	imports_typescript: 5,
	imports_svelte: 3
};

/** The napi world's hot functions, each run `times` times. */
const napi_hot = (times: (h: HotFunction) => number): [string, number[]][] =>
	HOT_FUNCTIONS.filter((h) => h.worlds.includes('napi')).map((h) => [
		mangle(h.path),
		[times(h), 3 * times(h)]
	]);

Deno.test('grade_profile passes a profile whose hot functions reach their floors', () => {
	const verdict = grade_profile(show(napi_hot((h) => TRAINED[h.per])), 'napi', TRAINED);
	deepStrictEqual(verdict.failures, []);
	strictEqual(verdict.executed, verdict.hot.length);
	// the CLI ships no import records, so its world does not ask for them
	const cli = grade_profile(
		show(HOT_FUNCTIONS.filter((h) => h.worlds.includes('cli')).map((h) => [mangle(h.path), [99]])),
		'cli',
		TRAINED
	);
	deepStrictEqual(cli.failures, []);
	ok(cli.hot.every((h) => !h.path.includes('imports')));
});

Deno.test(
	'grade_profile fails an empty profile, a missing, a never-run and an under-floor hot function',
	() => {
		ok(
			grade_profile('Counters:\n', 'napi', TRAINED).failures.includes(
				'the profile holds no function records'
			)
		);
		const [first, second, third, ...rest] = HOT_FUNCTIONS;
		// the third's loop counter is far over its floor, its entry one under: the entry decides
		const under = floor_of(third!, TRAINED) - 1;
		const verdict = grade_profile(
			show([
				[mangle(first!.path), [0, 0]],
				[mangle(third!.path), [under, 1000]],
				...rest.map((h): [string, number[]] => [mangle(h.path), [TRAINED[h.per]]])
			]),
			'napi',
			TRAINED
		);
		deepStrictEqual(verdict.failures, [
			`${first!.path} never ran in training (all counts 0)`,
			`no record for ${second!.path}`,
			`${third!.path} was entered ${under} times, under its floor of ${floor_of(third!, TRAINED)} (the ${TRAINED[third!.per]} inputs of its class trained)`
		]);
	}
);

Deno.test('grade_profile sums the entries of a function over its records', () => {
	const [first, ...rest] = HOT_FUNCTIONS.filter((h) => h.worlds.includes('napi'));
	const half = Math.ceil(TRAINED[first!.per] / 2);
	const verdict = grade_profile(
		show([
			[mangle(first!.path), [half]],
			[`other.cgu.0;${mangle(first!.path)}`, [TRAINED[first!.per] - half]],
			...rest.map((h): [string, number[]] => [mangle(h.path), [TRAINED[h.per]]])
		]),
		'napi',
		TRAINED
	);
	deepStrictEqual(verdict.failures, []);
	strictEqual(verdict.hot[0]!.entry_count, TRAINED[first!.per]);
});

Deno.test('grade_profile fails a profile that does not count function entries', () => {
	const verdict = grade_profile(
		show(
			napi_hot((h) => TRAINED[h.per]),
			0
		),
		'napi',
		TRAINED
	);
	ok(verdict.failures[0]!.includes('entry_first = 0'), verdict.failures[0]);
});

Deno.test('parse_pgo_diagnostics classifies the build scripts apart', () => {
	const log = [
		'   Compiling tsv_napi v0.8.0 (/w/crates/tsv_napi)',
		'warning: build_script_build.28bececf194f0957-cgu.0: no profile data available for function _ZN18build_script_build4main17h26d66acf13bdd994E Hash = 742261418966908927 up to 0 count discarded',
		'',
		'warning: tsv_ts.eaeb575bf7bcacc2-cgu.0: function control flow change detected (hash mismatch) _ZN6tsv_ts5lexer4next17h0123456789abcdefE Hash = 1',
		'warning: `tsv_napi` (build script) generated 9 warnings',
		'warning: unused variable: `x`'
	].join('\n');
	deepStrictEqual(
		parse_pgo_diagnostics(log).map((d) => [d.module, d.build_script]),
		[
			['build_script_build.28bececf194f0957-cgu.0', true],
			['tsv_ts.eaeb575bf7bcacc2-cgu.0', false]
		]
	);
});

Deno.test('parse_pgo_diagnostics reads a coloured log as a plain one', () => {
	const line =
		'warning: tsv_ts.eaeb575bf7bcacc2-cgu.0: no profile data available for function _ZN6tsv_ts1f17h0123456789abcdefE';
	const coloured = `\x1b[1m\x1b[33mwarning\x1b[0m\x1b[1m: ${line.slice('warning: '.length)}\x1b[0m`;
	strictEqual(strip_ansi(coloured), line);
	strictEqual(parse_pgo_diagnostics(coloured).length, 1);
});

Deno.test('profile_use_crates reads the flag off each rustc command line', () => {
	const log = [
		'   Compiling tsv_ts v0.8.0 (/w/crates/tsv_ts)',
		'     Running `/r/bin/rustc --crate-name tsv_ts --edition=2024 crates/tsv_ts/src/lib.rs -C opt-level=3 -Cprofile-use=/w/.pgo/napi.profdata -Cllvm-args=-pgo-warn-missing-function`',
		'     Running `D:\\r\\rustc.exe --crate-name tsv_css --edition=2024 -Cprofile-use=D:/a/w/.pgo/napi.profdata`',
		'     Running `/r/bin/rustc --crate-name tsv_lang -Cprofile-use=/w/.pgo/cli.profdata`',
		'     Running `/r/bin/rustc --crate-name smallvec -C opt-level=3`',
		'\x1b[1m     Running\x1b[0m `/r/bin/rustc --crate-name tsv_svelte -Cprofile-use=/w/.pgo/napi.profdata`'
	].join('\n');
	deepStrictEqual([...profile_use_crates(log, 'napi.profdata')].sort(), [
		'tsv_css',
		'tsv_svelte',
		'tsv_ts'
	]);
	deepStrictEqual([...profile_use_crates(log, 'cli.profdata')], ['tsv_lang']);
});

Deno.test('has_instrumentation_sections over each object format', () => {
	ok(has_instrumentation_sections('  Section {\n    Name: __llvm_prf_cnts (316)\n'));
	ok(has_instrumentation_sections('    Name: __llvm_prf_data (5F 5F 6C 6C)\n'));
	ok(has_instrumentation_sections('    Name: .lprfc (2E 6C 70 72 66 63)\n'));
	ok(!has_instrumentation_sections('    Name: .text (27)\n    Name: .rodata (35)\n'));
});
