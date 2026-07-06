//! Svelte AST → wire JSON conversion.
//!
//! The writer (`write.rs`) emits the compact wire JSON directly from the
//! internal Svelte AST in one walk, never materializing a typed public tree.
//! `special.rs` and `comment_attachment.rs` provide the byte-space skeleton +
//! acorn comment-attach machinery it composes for the comment-bearing
//! `<script>` / template-expression islands.

mod comment_attachment;
mod special;
mod write;

pub(crate) use write::{write_root_bytes, write_root_bytes_no_locations};
