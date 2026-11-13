//! The wasm32 layout pins for the types the shipped bundles are most sensitive to.
//!
//! Each language crate pins its 64-bit widths under `cfg(test)`, where a toolchain that
//! lays a type out differently fails `cargo test` rather than a consumer's build. A
//! test build never targets wasm32, so the 32-bit widths are pinned here instead: this
//! crate is not published, and `deno task build:packages` compiles it for wasm32, so a
//! change that widens one of these fails the package build.

use tsv_css::ast::{ColorChannel, CssValue, SimpleSelector};
use tsv_lang::doc::DocContext;
use tsv_lang::doc::arena::DocNode;
use tsv_lang::store::{Id, IdRet, ListBuf, ListMark, PoolText, Run, Store, StoreView, TextPool};
use tsv_svelte::ast::{AttributeNode, Element, ExpressionTag, FragmentNode};
use tsv_ts::Error;
use tsv_ts::ast::{
    CommentList, Expr, ExprId, Expression, ExpressionKind, ObjectPatternProperty, ObjectProperty,
    PoolStr, Property, Statement, StatementKind, StmtId, SwitchCase, TSType, TSTypeAnnotation,
    TSTypeParameterDeclaration, TSTypeParameterInstantiation, Ty, TyId, VariableDeclarator,
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
const _: () = assert!(size_of::<Expression>() == 48);
const _: () = assert!(size_of::<Option<Expression>>() == size_of::<Expression>());
const _: () = assert!(size_of::<ExpressionKind>() == 40);
const _: () = assert!(size_of::<Statement>() == 48);
const _: () = assert!(size_of::<Option<Statement>>() == size_of::<Statement>());
const _: () = assert!(size_of::<StatementKind>() == 40);
const _: () = assert!(size_of::<TSType>() == 56);
// The type-edge holders' `Option`s, folded into the id's niche and the run's. See the native
// pins in `tsv_ts`'s `ast/mod.rs`.
const _: () = assert!(size_of::<Option<TSTypeAnnotation>>() == size_of::<TSTypeAnnotation>());
const _: () = assert!(
    size_of::<Option<TSTypeParameterInstantiation>>() == size_of::<TSTypeParameterInstantiation>()
);
const _: () = assert!(
    size_of::<Option<TSTypeParameterDeclaration>>() == size_of::<TSTypeParameterDeclaration>()
);
// The list elements and their shared `Property` payload, as the same native pins hold them.
const _: () = assert!(size_of::<Property>() == 20);
const _: () = assert!(size_of::<ObjectProperty>() == 20);
const _: () = assert!(size_of::<ObjectPatternProperty>() == 28);
const _: () = assert!(size_of::<VariableDeclarator>() == 20);
const _: () = assert!(size_of::<SwitchCase>() == 20);
// The ladders' return currency: an id word plus the pointer-sized `Error` is two words. See
// the native pins in `tsv_ts`'s `parser/expression.rs` and `parser/types.rs`.
const _: () = assert!(size_of::<Result<IdRet<Expr>, Error>>() == 2 * size_of::<usize>());
const _: () = assert!(size_of::<Result<IdRet<Ty>, Error>>() == 2 * size_of::<usize>());
// The CSS numeric carriers, 4-aligned through `PackedF64`. See the native `ColorChannel` and
// `CssValue` pins in `tsv_css`'s `ast/mod.rs`.
const _: () = assert!(size_of::<ColorChannel>() == 12);
const _: () = assert!(size_of::<CssValue<'static>>() == 60);
const _: () = assert!(size_of::<SimpleSelector<'static>>() == 60);
// The Svelte nodes. See the native pins in `tsv_svelte`'s `ast/mod.rs`.
const _: () = assert!(size_of::<Element<'static>>() == 40);
const _: () = assert!(size_of::<FragmentNode<'static>>() == 80);
const _: () = assert!(size_of::<AttributeNode<'static>>() == 52);
const _: () = assert!(size_of::<ExpressionTag>() == 12);

enum Family {}

// The store handles, each niche-packed. See the native pins in `tsv_lang`'s `store.rs`.
const _: () = assert!(size_of::<ExprId>() == 4);
const _: () = assert!(size_of::<Option<ExprId>>() == 4);
const _: () = assert!(size_of::<Run<ExprId>>() == 8);
const _: () = assert!(size_of::<StmtId>() == 4);
const _: () = assert!(size_of::<Option<StmtId>>() == 4);
const _: () = assert!(size_of::<Run<StmtId>>() == 8);
const _: () = assert!(size_of::<TyId>() == 4);
const _: () = assert!(size_of::<Option<TyId>>() == 4);
const _: () = assert!(size_of::<Run<TyId>>() == 8);
const _: () = assert!(size_of::<Id<Family>>() == 4);
const _: () = assert!(size_of::<Option<Id<Family>>>() == 4);
const _: () = assert!(size_of::<Run<Family>>() == 8);
const _: () = assert!(size_of::<Option<Run<Family>>>() == 8);
const _: () = assert!(size_of::<PoolText<Family>>() == 8);
const _: () = assert!(size_of::<Option<PoolText<Family>>>() == 8);
const _: () = assert!(size_of::<ListMark<Family>>() == 4);
const _: () = assert!(size_of::<IdRet<Family>>() == 4);
const _: () = assert!(size_of::<Result<IdRet<Family>, Error>>() == 8);
const _: () = assert!(size_of::<StoreView<'static, Id<Family>>>() == 8);
const _: () = assert!(size_of::<Store<Id<Family>>>() == 12);
const _: () = assert!(size_of::<ListBuf<Id<Family>>>() == 12);
const _: () = assert!(size_of::<TextPool<Family>>() == 12);
// The tree's text and comment handles, niche-packed as the pool's and the store's are. See the
// native pins in `tsv_ts`'s `ast/stores.rs`.
const _: () = assert!(size_of::<PoolStr>() == 8);
const _: () = assert!(size_of::<Option<PoolStr>>() == 8);
const _: () = assert!(size_of::<CommentList>() == 8);
const _: () = assert!(size_of::<Option<CommentList>>() == 8);
