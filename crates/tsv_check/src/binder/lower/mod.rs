//! The lowering walk — `SoaWalk`'s per-node visitor methods, split by the AST
//! shape they descend (statements, expressions, types). Each submodule
//! contributes its own `impl SoaWalk { ... }` block; multiple `impl` blocks for
//! the same type are ordinary Rust, so the `SoaWalk` struct itself (its fields
//! and the `add`/`close`/`leaf` id-recording primitives) stays defined once in
//! the parent `binder` module. Purely a locality split — no behavior
//! distinction between the three files.

mod expression;
mod statement;
mod types;
