//! Svelte AST → wire JSON conversion (and, behind `_tape`, its tape).
//!
//! The writer (`write.rs`) emits the compact wire JSON directly from the
//! internal Svelte AST in one walk, never materializing a typed public tree.
//! `comment_attachment.rs` declares what each comment-bearing `<script>` /
//! template-expression island hands `tsv_ts`'s `CommentAttach` — its comment
//! window plus what acorn's walk reads about a root — which that one walk then
//! drives from its own node opens and closes. `special.rs` holds the
//! `<svelte:options>` readers and the component-global TypeScript decision.
//!
//! The writer is mounted once per sink: here the JSON copy, and behind `_tape` the tape copy
//! (`write_tape.rs`, which mounts `write.rs` a second time). Each mount defines `tape!` and
//! `json_only!` before mounting — `write.rs`'s module doc §One walk, two products — and
//! provides by name what it writes its own way (`write_json.rs` for this one) and the embedded
//! `tsv_ts` / `tsv_css` writers of its sink; the site macros (`write_macros.rs`) are shared.
//! The comment attach's island declarations and the `<svelte:options>` readers, which the
//! writer alone calls, are mounted once per copy (`write_tape.rs` mounts its own), so neither
//! copy's call outlines them out of the other.

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

/// The JSON copy's half of an expression each copy spells its own way.
macro_rules! copy_pick {
    ($json:expr, $tape:expr) => {
        $json
    };
}

// The writer's helper modules, mounted once per copy of it (`write_tape.rs` mounts its own).
mod comment_attachment;
mod special;
// The writer — the JSON copy — and the helpers it writes its own way.
mod write;
mod write_json;
#[cfg(feature = "_tape")]
mod write_tape;
// The writer's unit tests, over the JSON copy (`write_tests.rs` says why they sit here).
#[cfg(test)]
mod write_tests;

use tsv_css::__private::{write_css_children, write_css_comments};
use tsv_lang::{write_array, write_array as write_value_array, write_or_null};
use tsv_ts::__private::{
    write_expression_embedded, write_identifier_expression_with_character, write_pattern_embedded,
    write_program_embedded, write_variable_declaration_embedded,
};
use write_json::{
    write_attribute_header, write_custom_element_field, write_element_header,
    write_element_name_field, write_name_loc_field, write_raw_then_data, write_root_comment,
    write_scanned_name_field, write_text_header,
};

pub(crate) use write::write_root_bytes;
#[cfg(feature = "_tape")]
pub(crate) use write_tape::write_root_tape;
