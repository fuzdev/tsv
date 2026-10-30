//! The wasm32 layout pins for the types the shipped bundles are most sensitive to.
//!
//! Each language crate pins its 64-bit widths under `cfg(test)`, where a toolchain that
//! lays a type out differently fails `cargo test` rather than a consumer's build. A
//! test build never targets wasm32, so the 32-bit widths are pinned here instead: this
//! crate is not published, and `deno task build:packages` compiles it for wasm32, so a
//! change that widens one of these fails the package build.

use tsv_lang::doc::DocContext;
use tsv_lang::doc::arena::DocNode;
use tsv_ts::ast::{Expression, Statement};

// `DocContext` rides inline in `DocNode::WithContext`; see its native pin in `tsv_lang`
// for the budget it is held to.
const _: () = assert!(size_of::<DocContext>() == 4);
// The doc node store's element: `Group` sizes it on wasm32 (pointers and `usize` are half
// width), where the locality and allocator budget matters most. See the native pin in
// `tsv_lang`'s `doc/arena.rs` for the drivers.
const _: () = assert!(size_of::<DocNode>() == 16);
// The hot AST enums: a 4 B pointer, `Expression`'s payload 8 B aligned and `Statement`'s
// 4 B, and each one's `Option` niche-packed. See the native pins in `tsv_ts`'s
// `ast/mod.rs`.
const _: () = assert!(size_of::<Expression<'static>>() == 48);
const _: () = assert!(size_of::<Option<Expression<'static>>>() == size_of::<Expression<'static>>());
const _: () = assert!(size_of::<Statement<'static>>() == 48);
const _: () = assert!(size_of::<Option<Statement<'static>>>() == size_of::<Statement<'static>>());
