import { deepStrictEqual, throws } from 'node:assert';
import { test } from 'vitest';

import {
	assert_bounds_sites,
	assert_format_check_finished,
	assert_parsed_rows,
	assert_symbols,
	assert_wasm_sweep,
	count_bounds_sites,
	diff_symbols,
	parse_board_text,
	parse_heaptrack_size,
	parse_heaptrack_summary,
	parse_nm,
	parse_rss_kib,
	parse_shape_limits,
	parse_size_sections,
	symbol_key,
	test_run_matched
} from './parsers.ts';

test('symbol_key drops the legacy hash segment and an llvm suffix', () => {
	deepStrictEqual(
		symbol_key('_ZN4core9panicking18panic_bounds_check17h0123456789abcdefE'),
		'core::panicking::panic_bounds_check'
	);
	deepStrictEqual(symbol_key('_ZN6tsv_ts3foo17h0123456789abcdefE.llvm.123456'), 'tsv_ts::foo');
});

test('symbol_key translates the legacy path escapes', () => {
	deepStrictEqual(
		symbol_key('_ZN5alloc3vec16Vec$LT$T$C$A$GT$3new17h0000000000000000E'),
		'alloc::vec::Vec<T,A>::new'
	);
	deepStrictEqual(
		symbol_key('_ZN39_$LT$tsv_ts..A$u20$as$u20$tsv_ts..B$GT$3run17h0000000000000000E'),
		'<tsv_ts::A as tsv_ts::B>::run'
	);
});

test('symbol_key passes through a non-legacy name unchanged (minus an llvm suffix)', () => {
	deepStrictEqual(symbol_key('memcpy'), 'memcpy');
	deepStrictEqual(symbol_key('_RNvCs1234_3foo3bar.llvm.77'), '_RNvCs1234_3foo3bar');
});

test('parse_nm sums same-keyed copies and ignores data and zero-size symbols', () => {
	const text = [
		'0000000000001000 0000000000000010 t _ZN3foo3bar17h0000000000000001E',
		'0000000000002000 0000000000000014 t _ZN3foo3bar17h0000000000000002E',
		'0000000000003000 0000000000000050 r _ZN3foo4DATA17h0000000000000003E',
		'0000000000004000 0000000000000000 T empty',
		'                 U extern_undefined'
	].join('\n');
	const parsed = parse_nm(text);
	deepStrictEqual(parsed.sizes.get('foo::bar'), 10 + 14);
	deepStrictEqual(parsed.copies.get('foo::bar'), 2);
	deepStrictEqual(parsed.sizes.has('foo::DATA'), false);
	deepStrictEqual(parsed.sizes.has('empty'), false);
	deepStrictEqual(parsed.total, 24);
});

test('diff_symbols reports added, removed and changed, widest first, and is empty on A/A', () => {
	const base = {
		sizes: new Map([
			['a', 100],
			['b', 50],
			['gone', 30]
		]),
		copies: new Map(),
		total: 180
	};
	const cand = {
		sizes: new Map([
			['a', 120],
			['b', 50],
			['new', 7]
		]),
		copies: new Map(),
		total: 177
	};
	const diff = diff_symbols(base, cand);
	deepStrictEqual(diff.changed, 1);
	deepStrictEqual(diff.added, 1);
	deepStrictEqual(diff.removed, 1);
	deepStrictEqual(
		diff.movers.map((m) => m.key),
		['gone', 'a', 'new']
	);
	deepStrictEqual(
		diff.movers.map((m) => m.delta),
		[-30, 20, 7]
	);

	const same = diff_symbols(base, base);
	deepStrictEqual(same.movers, []);
});

test('count_bounds_sites counts call and jmp edges per panic group', () => {
	const objdump = [
		'0000000000001000 <_ZN3foo3bar17h0000000000000000E>:',
		'    1000:\tcall   1234 <_ZN4core9panicking18panic_bounds_check17h0000000000000000E>',
		'    1010:\tjmp    1234 <_ZN4core9panicking18panic_bounds_check17h0000000000000000E>',
		'    1020:\tcall   2345 <_ZN4core5slice5index16slice_index_fail17h0000000000000000E>',
		'    1030:\tcall   2345 <_ZN4core3str16slice_error_fail17h0000000000000000E>',
		'    1040:\tcall   5555 <memcpy@plt>',
		'    1050:\tmov    %rax,%rbx'
	].join('\n');
	const counts = count_bounds_sites(objdump);
	deepStrictEqual(counts.get('panic_bounds_check'), 2);
	deepStrictEqual(counts.get('slice index fail'), 1);
	deepStrictEqual(counts.get('str slice_error_fail'), 1);
});

test('count_bounds_sites starts every group at zero', () => {
	const counts = count_bounds_sites('');
	deepStrictEqual(counts.get('panic_bounds_check'), 0);
	deepStrictEqual(counts.get('slice index fail'), 0);
	deepStrictEqual(counts.get('str slice_error_fail'), 0);
});

test('parse_size_sections reads size -A -d', () => {
	const text = [
		'target/release/tsv  :',
		'section                 size     addr',
		'.interp                   28      792',
		'.text                3063808   143360',
		'.rodata               512000  3209216',
		'Total                4567890'
	].join('\n');
	const sections = parse_size_sections(text);
	deepStrictEqual(sections.get('.text'), 3063808);
	deepStrictEqual(sections.get('.rodata'), 512000);
	deepStrictEqual(sections.has('Total'), false);
});

test('heaptrack sizes and summary', () => {
	deepStrictEqual(parse_heaptrack_size('682.35K'), 682350);
	deepStrictEqual(parse_heaptrack_size('1.20G'), 1_200_000_000);
	deepStrictEqual(parse_heaptrack_size('512B'), 512);
	deepStrictEqual(parse_heaptrack_size('oops'), null);

	const text = [
		'total runtime: 0.00s.',
		'calls to allocation functions: 2049 (512250/s)',
		'temporary memory allocations: 2 (500/s)',
		'peak heap memory consumption: 682.35K',
		'peak RSS (including heaptrack overhead): 5.71M',
		'total memory leaked: 604.49K'
	].join('\n');
	deepStrictEqual(parse_heaptrack_summary(text), {
		allocation_calls: 2049,
		temporary_allocations: 2,
		peak_heap_bytes: 682350
	});
	deepStrictEqual(parse_heaptrack_summary('nothing'), null);
});

test('parse_shape_limits reads name, language and n, unescaping the name', () => {
	const text = 'parens\ttypescript\t256\t(\t(a\t)\t\t\nmember\\tcall\tsvelte\t6000\tx\ty\tz\t\t\n';
	deepStrictEqual(parse_shape_limits(text), [
		{ name: 'parens', lang: 'typescript', n: 256 },
		{ name: 'member\tcall', lang: 'svelte', n: 6000 }
	]);
});

test('parse_board_text reads the wasm32 board tab format', () => {
	deepStrictEqual(
		parse_board_text('ts/expression\tExpression\t56\t8\t64\nts/statement\tStatement\t24\t8\t24\n'),
		[
			{ group: 'ts/expression', name: 'Expression', size: 56, align: 8, result_size: 64 },
			{ group: 'ts/statement', name: 'Statement', size: 24, align: 8, result_size: 24 }
		]
	);
});

test('format --check exit 2 is not a finished run, and names its stderr', () => {
	assert_format_check_finished(0, '', 'x');
	assert_format_check_finished(1, '', 'x');
	throws(
		() => assert_format_check_finished(2, 'error: unrecognized flag --jobs', 'x'),
		/unrecognized flag/
	);
});

test('an empty symbol table is refused, not read as identical', () => {
	throws(
		() => assert_symbols({ sizes: new Map(), copies: new Map(), total: 0 }, '/bin/x'),
		/\/bin\/x/
	);
	const symbols = { sizes: new Map([['a', 1]]), copies: new Map(), total: 1 };
	deepStrictEqual(assert_symbols(symbols, '/bin/x'), symbols);
});

test('all-zero bounds sites are refused, one non-zero group is fine', () => {
	throws(() => assert_bounds_sites(count_bounds_sites('nothing here'), '/bin/x'), /no bounds-miss/);
	const counts = new Map([
		['panic_bounds_check', 0],
		['slice index fail', 3]
	]);
	deepStrictEqual(assert_bounds_sites(counts, '/bin/x'), counts);
});

test('count_bounds_sites takes callq/jmpq, and not a jump inside a panic function', () => {
	const objdump = [
		'    1000:\tcallq  1234 <_ZN4core9panicking18panic_bounds_check17h0000000000000000E>',
		'    1010:\tjmpq   1234 <_ZN4core9panicking18panic_bounds_check17h0000000000000000E>',
		'    1020:\tjmp    1250 <_ZN4core9panicking18panic_bounds_check17h0000000000000000E+0x12>'
	].join('\n');
	deepStrictEqual(count_bounds_sites(objdump).get('panic_bounds_check'), 2);
});

test('an empty board or shape parse is refused', () => {
	throws(() => assert_parsed_rows([], 'the base board'), /the base board parsed to no rows/);
	deepStrictEqual(assert_parsed_rows([1], 'x'), [1]);
	throws(() => assert_parsed_rows(parse_shape_limits(''), 'the base shape-limit file'));
});

test('parse_rss_kib: empty stderr, non-numeric and zero are not an RSS', () => {
	deepStrictEqual(parse_rss_kib('24000\n'), 24000);
	deepStrictEqual(parse_rss_kib('some warning\n24000\n'), 24000);
	deepStrictEqual(parse_rss_kib(''), null);
	deepStrictEqual(parse_rss_kib('Command exited with non-zero status 1\n'), null);
	deepStrictEqual(parse_rss_kib('0\n'), null);
});

test('a libtest filter that matched nothing did not pass', () => {
	deepStrictEqual(test_run_matched('test result: ok. 1 passed; 0 failed; 0 ignored'), true);
	deepStrictEqual(
		test_run_matched('running 0 tests\n\ntest result: ok. 0 passed; 0 failed'),
		false
	);
	deepStrictEqual(test_run_matched(''), false);
});

test('a wasm sweep over no files, or with a throw, is refused', () => {
	const sweep = { files: 10, errors: 0, first_error: null };
	assert_wasm_sweep(sweep, 'x');
	throws(() => assert_wasm_sweep({ ...sweep, files: 0 }, 'x'), /0 files/);
	throws(
		() => assert_wasm_sweep({ ...sweep, errors: 2, first_error: 'a.ts: boom' }, 'x'),
		/a\.ts: boom/
	);
});
