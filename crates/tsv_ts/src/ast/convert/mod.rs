// Conversion from the internal AST to the public wire JSON (and, behind `_tape`, its tape).
//
// The writer (`write/`) emits the compact wire JSON directly from the internal
// AST in one walk, fusing byte→UTF-16 offset translation into the walk (final
// char-space positions emitted directly via `WirePositions`). It is the sole
// emission path; `convert_ast_json_bytes` / `convert_ast_json_string` and
// `convert_ast_json_bytes_with_locations` in `lib.rs` call it.
//
// The writer is mounted once per sink: here the JSON copy, and behind `_tape` the tape copy
// (`write_tape.rs`, which mounts `write/` a second time). Each mount defines `tape!` and
// `json_only!` before mounting — `write/mod.rs`'s module doc §One walk, two products — and the
// site macros (`write_macros.rs`) are shared. The comment attach (`comments.rs`) and the
// embedding types the host crates hand in are mounted once, outside both copies, so a host
// hands one value to either; its emitter is generic over the sink.

#[macro_use]
mod write_macros;

/// The JSON copy compiles no tape code.
macro_rules! tape {
    ($($t:tt)*) => {};
}

/// The JSON copy's own code.
macro_rules! json_only {
    ($($t:tt)*) => { $($t)* };
}

// The BigInt helper, mounted once per copy (`bigint.rs` says why).
mod bigint;
use bigint::bigint_to_decimal;
mod comments;
// The writer — the JSON copy — and the helpers it writes its own way.
mod write;
mod write_json;
use write_json::{
    header_run, write_chain_any, write_identifier_plain, write_literal_fields_any,
    write_name_field, write_number_value,
};
#[cfg(feature = "_tape")]
mod write_tape;
// The writer's unit tests, over the JSON copy (`write_tests.rs` says why it sits here).
#[cfg(test)]
mod write_tests;

pub use comments::{CommentAttach, CommentMode, EmbedWriter, IslandComments};
pub(crate) use write::write_program_bytes;
pub use write::{
    write_expression_embedded, write_identifier_expression_with_character, write_pattern_embedded,
    write_program_embedded, write_variable_declaration_embedded,
};
#[cfg(feature = "_tape")]
pub(crate) use write_tape::{write_expression_island_tape, write_program_tape};
