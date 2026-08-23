//! Svelte AST → wire JSON conversion.
//!
//! The writer (`write.rs`) emits the compact wire JSON directly from the
//! internal Svelte AST in one walk, never materializing a typed public tree.
//! `comment_attachment.rs` declares what each comment-bearing `<script>` /
//! template-expression island hands `tsv_ts`'s `CommentAttach` — its comment
//! window plus what acorn's walk reads about a root — which that one walk then
//! drives from its own node opens and closes. `special.rs` holds the
//! `<svelte:options>` readers and the component-global TypeScript decision.

mod comment_attachment;
mod special;
mod write;

pub(crate) use write::{write_root_bytes, write_root_bytes_no_locations};
