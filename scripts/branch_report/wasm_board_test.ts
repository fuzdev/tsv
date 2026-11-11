import { deepStrictEqual, throws } from 'node:assert';
import { test } from 'vitest';

import { balanced_block, wasm_board_lib, wasm_board_manifest } from './wasm_board.ts';

test('balanced_block finds the matching brace past strings and nesting', () => {
	const text = 'pub fn f() { let s = "}{"; if x { y } }';
	const { start, end } = balanced_block(text, 0);
	deepStrictEqual(text.slice(start, end), '{ let s = "}{"; if x { y } }');
});

test('balanced_block rejects a missing or unbalanced block', () => {
	throws(() => balanced_block('no braces', 0));
	throws(() => balanced_block('fn f() { unclosed', 0));
});

const BOARD = `
use tsv_lang::Result as TsvResult;
pub(crate) struct TypeRow {
    pub(crate) group: &'static str,
    pub(crate) name: &'static str,
    pub(crate) size: usize,
    pub(crate) align: usize,
    pub(crate) result_size: usize,
}
macro_rules! row {
    ($group:expr, $t:ty) => {
        TypeRow { group: $group, name: stringify!($t), size: size_of::<$t>(), align: align_of::<$t>(), result_size: size_of::<TsvResult<$t>>() }
    };
}
macro_rules! rows {
    ($out:expr, $group:expr, [$($t:ty),* $(,)?]) => {
        $out.extend([$(row!($group, $t)),*]);
    };
}
pub(crate) fn board() -> Vec<TypeRow> {
    use tsv_ts::ast as ts;
    let mut out = Vec::new();
    rows!(out, "ts/expression", [ts::Foo<'static>]);
    out
}
`;

test('wasm_board_lib wraps the board and exports its text', () => {
	const lib = wasm_board_lib(BOARD);
	deepStrictEqual(lib.includes('struct TypeRow'), true);
	deepStrictEqual(lib.includes('pub(crate) struct'), false);
	deepStrictEqual(lib.includes('macro_rules! rows'), true);
	deepStrictEqual(lib.includes('fn board() -> Vec<TypeRow>'), true);
	deepStrictEqual(lib.includes('board_text_ptr'), true);
});

test('wasm_board_lib rejects a board source missing an item', () => {
	throws(() => wasm_board_lib('fn nothing() {}'));
});

test('wasm_board_manifest depends on the tree crates by relative path', () => {
	const manifest = wasm_board_manifest('../src');
	deepStrictEqual(manifest.includes('path = "../src/crates/tsv_ts"'), true);
	deepStrictEqual(manifest.includes('[workspace]'), true);
});
