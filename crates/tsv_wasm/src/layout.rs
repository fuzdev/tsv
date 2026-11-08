//! The wasm32 layout pins for the types the shipped bundles are most sensitive to.
//!
//! Each language crate pins its 64-bit widths under `cfg(test)`, where a toolchain that
//! lays a type out differently fails `cargo test` rather than a consumer's build. A
//! test build never targets wasm32, so the 32-bit widths are pinned here instead: this
//! crate is not published, and `deno task build:packages` compiles it for wasm32, so a
//! change that widens one of these fails the package build.

use tsv_lang::doc::DocContext;
use tsv_lang::doc::arena::DocNode;
use tsv_svelte::ast::{AttributeNode, Element, ExpressionTag, FragmentNode};
use tsv_ts::Error;
use tsv_ts::ast::{
    Expression, ExpressionKind, ObjectPatternProperty, ObjectProperty, Property, Statement,
    StatementKind, SwitchCase, TSType, VariableDeclarator,
};

// `DocContext` rides inline in `DocNode::WithContext`; see its native pin in `tsv_lang`
// for the budget it is held to.
const _: () = assert!(size_of::<DocContext>() == 4);
// The doc node store's element: `Group` sizes it on wasm32 (pointers and `usize` are half
// width), where the locality and allocator budget matters most. See the native pin in
// `tsv_lang`'s `doc/arena.rs` for the drivers.
const _: () = assert!(size_of::<DocNode>() == 16);
// The hot AST types: a 4 B pointer, `Expression`'s payload 8 B aligned and `Statement`'s
// 4 B, and the `Option` of those two niche-packed. See the native pins in `tsv_ts`'s
// `ast/mod.rs`.
const _: () = assert!(size_of::<Expression<'static>>() == 48);
const _: () = assert!(size_of::<Option<Expression<'static>>>() == size_of::<Expression<'static>>());
const _: () = assert!(size_of::<ExpressionKind<'static>>() == 40);
const _: () = assert!(size_of::<Statement<'static>>() == 48);
const _: () = assert!(size_of::<Option<Statement<'static>>>() == size_of::<Statement<'static>>());
const _: () = assert!(size_of::<StatementKind<'static>>() == 40);
const _: () = assert!(size_of::<TSType<'static>>() == 56);
// The list elements and their shared `Property` payload, as the same native pins hold them.
const _: () = assert!(size_of::<Property<'static>>() == 20);
const _: () = assert!(size_of::<ObjectProperty<'static>>() == 20);
const _: () = assert!(size_of::<ObjectPatternProperty<'static>>() == 28);
const _: () = assert!(size_of::<VariableDeclarator<'static>>() == 20);
const _: () = assert!(size_of::<SwitchCase<'static>>() == 20);
// The ladders' return currency: a reference plus the pointer-sized `Error` is two words. See
// the native pins in `tsv_ts`'s `parser/expression.rs` and `parser/types.rs`.
const _: () = assert!(size_of::<Result<&Expression<'static>, Error>>() == 2 * size_of::<usize>());
const _: () = assert!(size_of::<Result<&TSType<'static>, Error>>() == 2 * size_of::<usize>());
// The Svelte nodes. See the native pins in `tsv_svelte`'s `ast/mod.rs`.
const _: () = assert!(size_of::<Element<'static>>() == 40);
const _: () = assert!(size_of::<FragmentNode<'static>>() == 80);
const _: () = assert!(size_of::<AttributeNode<'static>>() == 52);
const _: () = assert!(size_of::<ExpressionTag<'static>>() == 12);
