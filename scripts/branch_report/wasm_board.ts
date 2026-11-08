/**
 * The wasm32 half of the `size_of` board: `tsv_debug type_sizes` runs on the host only, since
 * `tsv_debug` does not compile for wasm32. The board's own Rust text is taken from a tree's
 * `type_sizes.rs` (so a type added to the host board is counted here too) and wrapped in a
 * scratch `cdylib` beside the tree, which reports the rows as text through two exports.
 *
 * @module
 */

/** Where a tree holds the board. */
export const BOARD_SOURCE = 'crates/tsv_debug/src/cli/commands/type_sizes.rs';

/** The balanced `{...}` block that opens at the first `{` at or after `from`. */
export function balanced_block(text: string, from: number): { start: number; end: number } {
	const open = text.indexOf('{', from);
	if (open < 0) throw new Error('no opening brace');
	let depth = 0;
	for (let i = open; i < text.length; i++) {
		const c = text[i];
		if (c === '"') {
			// a string literal's braces do not count
			i++;
			while (i < text.length && text[i] !== '"') i += text[i] === '\\' ? 2 : 1;
		} else if (c === '{') depth++;
		else if (c === '}' && --depth === 0) return { start: open, end: i + 1 };
	}
	throw new Error('unbalanced braces');
}

function item_text(text: string, opener: string): string {
	const at = text.indexOf(opener);
	if (at < 0) throw new Error(`the board source has no \`${opener}\``);
	const { end } = balanced_block(text, at);
	return text.slice(at, end);
}

/**
 * The Rust of the scratch crate's `src/lib.rs`, its board copied from `source`. The whole file
 * is rejected if an expected item is gone, which reads as a refactor to follow here.
 */
export function wasm_board_lib(source: string): string {
	const row_struct = item_text(source, 'pub(crate) struct TypeRow').replace('pub(crate) ', '');
	const row_macro = item_text(source, 'macro_rules! row ');
	const rows_macro = item_text(source, 'macro_rules! rows ');
	const board = item_text(source, 'pub(crate) fn board()').replace('pub(crate) ', '');
	return `#![allow(unused, unfulfilled_lint_expectations)]

use std::sync::OnceLock;
use tsv_lang::Result as TsvResult;

${row_struct}

${row_macro}

${rows_macro}

${board}

static TEXT: OnceLock<String> = OnceLock::new();

fn text() -> &'static str {
    TEXT.get_or_init(|| {
        let mut out = String::new();
        for row in board() {
            out.push_str(row.group);
            out.push('\\t');
            out.push_str(row.name);
            out.push('\\t');
            out.push_str(&row.size.to_string());
            out.push('\\t');
            out.push_str(&row.align.to_string());
            out.push('\\t');
            out.push_str(&row.result_size.to_string());
            out.push('\\n');
        }
        out
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn board_text_ptr() -> *const u8 {
    text().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn board_text_len() -> usize {
    text().len()
}
`;
}

/** The scratch crate's manifest; `src_dir` is the tree's workspace root, relative to the crate. */
export function wasm_board_manifest(src_dir: string): string {
	return `[package]
name = "tsv_type_board_wasm"
version = "0.0.0"
edition = "2024"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
tsv_lang = { path = "${src_dir}/crates/tsv_lang" }
tsv_ts = { path = "${src_dir}/crates/tsv_ts", default-features = false }
tsv_css = { path = "${src_dir}/crates/tsv_css", default-features = false }
tsv_svelte = { path = "${src_dir}/crates/tsv_svelte", default-features = false }

[workspace]

[profile.release]
opt-level = "s"
`;
}

/** The board text from an instantiated scratch wasm module's exports. */
export function board_text_from_exports(exports: WebAssembly.Exports): string {
	const memory = exports.memory as WebAssembly.Memory;
	const ptr = (exports.board_text_ptr as () => number)();
	const len = (exports.board_text_len as () => number)();
	return new TextDecoder().decode(new Uint8Array(memory.buffer, ptr, len));
}
