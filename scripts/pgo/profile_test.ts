/**
 * The PGO gates' readers (`scripts/pgo/profile.ts`) against tool output captured from
 * the pinned toolchain's `llvm-profdata` / `llvm-readobj` and a `-Cprofile-use` build
 * log — each gate's two directions, so a reader that stopped matching reads as a
 * failure here rather than as a gate that silently passes.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { test } from 'vitest';

import {
	binary_holds,
	classify_diagnostics,
	crate_cgu_hashes,
	demangle_legacy_path,
	DROPS_DEAD_RECORDS,
	floor_of,
	grade_profile,
	has_instrumentation_sections,
	HOT_FUNCTIONS,
	type HotFunction,
	llvm_version,
	missing_profile_function,
	object_format,
	parse_cgu_module,
	parse_pgo_diagnostics,
	parse_profile_show,
	profile_instrumentation,
	profile_use_crates,
	strip_ansi,
	type TrainedCounts
} from './profile.ts';

test('llvm_version reads both spellings', () => {
	strictEqual(
		llvm_version('LLVM (http://llvm.org/):\n  LLVM version 21.1.8-rust-1.94.1-stable\n'),
		'21.1.8'
	);
	strictEqual(llvm_version('binary: rustc\nhost: x86_64\nLLVM version: 21.1.8\n'), '21.1.8');
	strictEqual(llvm_version('no version here'), null);
});

test('demangle_legacy_path', () => {
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

test('parse_profile_show takes the largest count and the entry count per record', () => {
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

test('profile_instrumentation reads the header line', () => {
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

test('grade_profile passes a profile whose hot functions reach their floors', () => {
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
	// the FFI library has neither the import records nor the type strip
	const ffi = grade_profile(
		show(HOT_FUNCTIONS.filter((h) => h.worlds.includes('ffi')).map((h) => [mangle(h.path), [99]])),
		'ffi',
		TRAINED
	);
	deepStrictEqual(ffi.failures, []);
	ok(ffi.hot.every((h) => !h.path.includes('imports') && !h.path.includes('strip')));
	ok(ffi.hot.some((h) => h.path.includes('render_doc_iterative')));
});

test('grade_profile fails an empty profile, a missing, a never-run and an under-floor hot function', () => {
	ok(
		grade_profile('Counters:\n', 'napi', TRAINED).failures.includes(
			'the profile holds no function records'
		)
	);
	const [first, second, third, ...rest] = HOT_FUNCTIONS.filter((h) => h.worlds.includes('napi'));
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
});

test('grade_profile sums the entries of a function over its records', () => {
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

test('grade_profile fails a profile that does not count function entries', () => {
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

test('parse_pgo_diagnostics classifies the build scripts apart', () => {
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

test('parse_pgo_diagnostics reads a coloured log as a plain one', () => {
	const line =
		'warning: tsv_ts.eaeb575bf7bcacc2-cgu.0: no profile data available for function _ZN6tsv_ts1f17h0123456789abcdefE';
	const coloured = `\x1b[1m\x1b[33mwarning\x1b[0m\x1b[1m: ${line.slice('warning: '.length)}\x1b[0m`;
	strictEqual(strip_ansi(coloured), line);
	strictEqual(parse_pgo_diagnostics(coloured).length, 1);
});

test('profile_use_crates reads the flag off each rustc command line', () => {
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

test('has_instrumentation_sections over each object format', () => {
	ok(has_instrumentation_sections('  Section {\n    Name: __llvm_prf_cnts (316)\n'));
	ok(has_instrumentation_sections('    Name: __llvm_prf_data (5F 5F 6C 6C)\n'));
	ok(has_instrumentation_sections('    Name: .lprfc (2E 6C 70 72 66 63)\n'));
	ok(!has_instrumentation_sections('    Name: .text (27)\n    Name: .rodata (35)\n'));
});

test('object_format reads the Format line of each object format', () => {
	// `llvm-readobj --file-headers` over objects the pinned toolchain's llc wrote per target
	strictEqual(object_format('\nFile: tsv\nFormat: elf64-x86-64\nArch: x86_64\n'), 'elf');
	strictEqual(object_format('File: tsv\nFormat: elf64-littleaarch64\nArch: aarch64\n'), 'elf');
	strictEqual(object_format('\nFile: t.o\nFormat: Mach-O 64-bit x86-64\nArch: x86_64\n'), 'macho');
	strictEqual(object_format('\nFile: t.o\nFormat: Mach-O arm64\nArch: aarch64\n'), 'macho');
	strictEqual(object_format('\nFile: t.o\nFormat: COFF-x86-64\nArch: x86_64\n'), 'coff');
	strictEqual(object_format('File: t.wasm\nFormat: WASM\n'), null);
	strictEqual(object_format('no header'), null);
	ok(!DROPS_DEAD_RECORDS.elf && DROPS_DEAD_RECORDS.macho && DROPS_DEAD_RECORDS.coff);
});

test('missing_profile_function names one function', () => {
	strictEqual(
		missing_profile_function(
			'no profile data available for function _ZN4tsv_7fanout5Tally3add17h0123456789abcdefE Hash = 742261418966908927 up to 0 count discarded'
		),
		'_ZN4tsv_7fanout5Tally3add17h0123456789abcdefE'
	);
	strictEqual(
		missing_profile_function(
			'function control flow change detected (hash mismatch) _ZN6tsv_ts5lexer4next17h0123456789abcdefE Hash = 1'
		),
		null
	);
});

/** A use-build log in the shape a Mach-O row writes: LLVM's diagnostics for the workspace
 * crates' functions no training reached, a build script's, and (when asked) a hash mismatch. */
const MACHO_LOG = [
	'warning: tsv_lang.3f0c9d1e2a4b5c6d-cgu.0: no profile data available for function _ZN8tsv_lang10json_writer15stage_copy_cold17h1111111111111111E Hash = 742261418966908927 up to 0 count discarded',
	'warning: tsv_lang.3f0c9d1e2a4b5c6d-cgu.0: no profile data available for function _ZN64_$LT$tsv_lang..error..Error$u20$as$u20$core..fmt..Debug$GT$3fmt17h2222222222222222E Hash = 1 up to 0 count discarded',
	'warning: tsv_discover.0a1b2c3d4e5f6a7b-cgu.0: no profile data available for function _ZN12tsv_discover10is_ignored17h3333333333333333E Hash = 1 up to 0 count discarded',
	'warning: build_script_build.28bececf194f0957-cgu.0: no profile data available for function _ZN18build_script_build4main17h26d66acf13bdd994E Hash = 742261418966908927 up to 0 count discarded'
].join('\n');

/** A local function inlinable across crates: rustc compiles a copy into each module that
 * uses it, and each copy is its own record, `<module>;<symbol>`. */
const U32_DEBUG =
	'_ZN4core3fmt3num50_$LT$impl$u20$core..fmt..Debug$u20$for$u20$u32$GT$3fmt17h9e725510bb447855E';
const RENDER = '_ZN8tsv_lang3doc12arena_render20render_doc_iterative17h0123456789abcdefE';
const PARSE_FOLDED = '_ZN6tsv_ts9__private12parse_folded17hbed61a5168c09c4aE';
const TSV_LANG = 'tsv_lang.3f0c9d1e2a4b5c6d-cgu.0';
const TSV_TS = 'tsv_ts.eaeb575bf7bcacc2-cgu.0';

/** The record table an instrumented binary's raw profile lists (`llvm-profdata show
 * --all-functions` over a `.profraw`): names only, local functions `<module>;`-prefixed,
 * an external one bare. The `u32` copy survives in `tsv_ts` alone — the link dropped the
 * `tsv_lang` copy's record with its dead code. */
const RAW_SHOW = [
	'Counters:',
	`  ${TSV_LANG};${RENDER}:`,
	'    Hash: 0x03c13cf48702e2f5',
	'    Counters: 23',
	`  ${TSV_TS};${U32_DEBUG}:`,
	'    Hash: 0x0000000000000018',
	'    Counters: 1',
	`  ${PARSE_FOLDED}:`,
	'    Hash: 0x0a4d0ad3efffffff',
	'    Counters: 1',
	'Instrumentation level: IR  entry_first = 1  instrument_loop_entries = 0',
	'Functions shown: 3',
	''
].join('\n');

const raw_records = (text: string): Set<string> =>
	new Set(parse_profile_show(text).map((r) => r.name));

const missing_in = (module: string, symbol: string): string =>
	`warning: ${module}: no profile data available for function ${symbol} Hash = 9 up to 0 count discarded`;

test('binary_holds matches a local record in its own module, an external one anywhere', () => {
	const records = raw_records(RAW_SHOW);
	ok(binary_holds(records, TSV_TS, U32_DEBUG));
	ok(!binary_holds(records, TSV_LANG, U32_DEBUG));
	ok(binary_holds(records, TSV_LANG, RENDER));
	ok(!binary_holds(records, TSV_TS, RENDER));
	ok(binary_holds(records, TSV_LANG, PARSE_FOLDED));
	ok(binary_holds(records, TSV_TS, PARSE_FOLDED));
});

test('classify_diagnostics passes a dead-stripped function on Mach-O and COFF', () => {
	const records = raw_records(RAW_SHOW);
	strictEqual(records.size, 3);
	for (const format of ['macho', 'coff'] as const) {
		const c = classify_diagnostics(parse_pgo_diagnostics(MACHO_LOG), records, format);
		strictEqual(c.build_script.length, 1);
		strictEqual(c.dead_stripped.length, 3);
		strictEqual(c.stale.length + c.unexpected.length, 0);
	}
});

test('classify_diagnostics fails a missing function on ELF, which keeps every record', () => {
	const c = classify_diagnostics(parse_pgo_diagnostics(MACHO_LOG), raw_records(RAW_SHOW), 'elf');
	strictEqual(c.dead_stripped.length, 0);
	strictEqual(c.unexpected.length, 3);
	strictEqual(c.build_script.length, 1);
});

test('classify_diagnostics reads a local copy recorded only in another module as dead', () => {
	// the `tsv_lang` copy's record went with its dead code; the live `tsv_ts` copy's record
	// names another function, so it holds nothing of the copy diagnosed
	const log = missing_in(TSV_LANG, U32_DEBUG);
	for (const format of ['macho', 'coff'] as const) {
		const c = classify_diagnostics(parse_pgo_diagnostics(log), raw_records(RAW_SHOW), format);
		strictEqual(c.dead_stripped.length, 1, format);
		strictEqual(c.stale.length + c.unexpected.length, 0, format);
	}
	const c = classify_diagnostics(parse_pgo_diagnostics(log), raw_records(RAW_SHOW), 'elf');
	strictEqual(c.unexpected.length, 1);
	strictEqual(c.dead_stripped.length + c.stale.length, 0);
});

test('classify_diagnostics fails a missing function the instrumented binary records', () => {
	// a profile from other source: the binary records the function in the module diagnosed
	// (a local copy, or an external function from any module), the profile has none of it
	const cases = [
		[missing_in(TSV_TS, U32_DEBUG), 'u32'],
		[missing_in(TSV_LANG, RENDER), 'render_doc_iterative'],
		[missing_in(TSV_TS, PARSE_FOLDED), 'parse_folded'],
		[missing_in('tsv_svelte.0123456789abcdef-cgu.0', PARSE_FOLDED), 'parse_folded']
	] as const;
	for (const [line, name] of cases) {
		for (const format of ['macho', 'coff', 'elf'] as const) {
			const log = [MACHO_LOG, line].join('\n');
			const c = classify_diagnostics(parse_pgo_diagnostics(log), raw_records(RAW_SHOW), format);
			strictEqual(c.stale.length, 1, `${name} ${format}`);
			ok(c.stale[0]!.message.includes(name));
		}
	}
});

test('classify_diagnostics fails every hash mismatch, on every format', () => {
	const log =
		'warning: tsv_ts.eaeb575bf7bcacc2-cgu.0: function control flow change detected (hash mismatch) _ZN6tsv_ts9__private12parse_folded17hbed61a5168c09c4aE Hash = 1';
	for (const format of ['macho', 'coff', 'elf'] as const) {
		const c = classify_diagnostics(parse_pgo_diagnostics(log), raw_records(RAW_SHOW), format);
		strictEqual(c.unexpected.length, 1, format);
		strictEqual(c.dead_stripped.length + c.stale.length, 0);
	}
	// and one whose function the binary does not record
	const c = classify_diagnostics(
		parse_pgo_diagnostics(
			log.replace('parse_folded17hbed61a5168c09c4aE', 'gone17h0000000000000000E')
		),
		raw_records(RAW_SHOW),
		'macho'
	);
	strictEqual(c.unexpected.length, 1);
});

test('parse_cgu_module reads a codegen unit name, and only that shape', () => {
	deepStrictEqual(parse_cgu_module(TSV_LANG), { crate: 'tsv_lang', hash: '3f0c9d1e2a4b5c6d' });
	// rustc drops a hash's leading zeros
	deepStrictEqual(parse_cgu_module('argh.a1aa8553fa9069d-cgu.12'), {
		crate: 'argh',
		hash: 'a1aa8553fa9069d'
	});
	for (const other of [
		'tsv_lang',
		'tsv_lang.cgu.0',
		'x.cgu.0',
		'tsv_lang.3F0C-cgu.0',
		'a.b-cgu.'
	]) {
		strictEqual(parse_cgu_module(other), null, other);
	}
});

test('crate_cgu_hashes maps the crates the prefixed records name', () => {
	const map = crate_cgu_hashes([
		...raw_records(RAW_SHOW),
		'tsv_lang.00aa-cgu.1;_ZN1a1b17h0123456789abcdefE',
		'x.cgu.0;_ZN1a1c17h0123456789abcdefE'
	]);
	deepStrictEqual(
		[...map].map(([crate, hashes]) => [crate, [...hashes].sort()]),
		[
			['tsv_lang', ['00aa', '3f0c9d1e2a4b5c6d']],
			['tsv_ts', ['eaeb575bf7bcacc2']]
		]
	);
});

test('classify_diagnostics fails a module from another crate world, on every format', () => {
	// `tsv_lang` compiled under another `-C metadata`: no `<module>;` record can match, so on
	// Mach-O and COFF the missing function would otherwise read as dead code
	const drifted = 'tsv_lang.9999999999999999-cgu.0';
	const log = [missing_in(drifted, RENDER), missing_in(drifted, U32_DEBUG)].join('\n');
	for (const format of ['macho', 'coff', 'elf'] as const) {
		const c = classify_diagnostics(parse_pgo_diagnostics(log), raw_records(RAW_SHOW), format);
		strictEqual(c.foreign_module.length, 2, format);
		strictEqual(c.dead_stripped.length + c.stale.length + c.unexpected.length, 0, format);
	}
	// a hash mismatch from another crate world is graded the same way
	const mismatch = `warning: ${drifted}: function control flow change detected (hash mismatch) ${RENDER} Hash = 1`;
	const c = classify_diagnostics(parse_pgo_diagnostics(mismatch), raw_records(RAW_SHOW), 'macho');
	strictEqual(c.foreign_module.length, 1);
});

test('classify_diagnostics fails a module name it cannot read', () => {
	const log = missing_in('tsv_lang', U32_DEBUG);
	for (const format of ['macho', 'coff', 'elf'] as const) {
		const c = classify_diagnostics(parse_pgo_diagnostics(log), raw_records(RAW_SHOW), format);
		strictEqual(c.foreign_module.length, 1, format);
	}
});

test('classify_diagnostics keeps a dead copy in a module the instrumented build compiled', () => {
	// the same crate world: `tsv_lang`'s hash matches its records, the copy is just dead
	for (const format of ['macho', 'coff'] as const) {
		const c = classify_diagnostics(
			parse_pgo_diagnostics(missing_in(TSV_LANG, U32_DEBUG)),
			raw_records(RAW_SHOW),
			format
		);
		strictEqual(c.foreign_module.length, 0, format);
		strictEqual(c.dead_stripped.length, 1, format);
	}
});

test('classify_diagnostics leaves a crate its records do not name ungraded by module', () => {
	// `tsv_discover` has no prefixed record (every function of it external), so any hash is
	// held to the record rule alone
	const log = missing_in(
		'tsv_discover.0a1b2c3d4e5f6a7b-cgu.0',
		'_ZN12tsv_discover1f17h0123456789abcdefE'
	);
	for (const format of ['macho', 'coff'] as const) {
		const c = classify_diagnostics(parse_pgo_diagnostics(log), raw_records(RAW_SHOW), format);
		strictEqual(c.foreign_module.length, 0, format);
		strictEqual(c.dead_stripped.length, 1, format);
	}
	const c = classify_diagnostics(parse_pgo_diagnostics(log), raw_records(RAW_SHOW), 'elf');
	strictEqual(c.foreign_module.length, 0);
	strictEqual(c.unexpected.length, 1);
});
