//! The ECMAScript parse goal symbol (`Script` vs `Module`).
//!
//! The goal is a parse-time input — literally which symbol the grammar starts
//! from (`ParseScript` vs `ParseModule` in the spec). It governs *syntactic
//! availability* of a handful of constructs and nothing else. It is orthogonal
//! to strictness: `tsv` is **always strict** (it has no sloppy mode and never
//! inspects a `"use strict"` directive), so `Goal` toggles only the
//! goal-specific grammar, not the lexical/early-error rejections that strict
//! mode owns.

/// The syntactic goal symbol a parse runs against.
///
/// Defaults to [`Goal::Module`], which is correct for Svelte `<script>` blocks
/// (always modules) and essentially all real-world TypeScript. [`Goal::Script`]
/// exists for standalone scripts and parser-conformance grading, where the
/// goal-specific constructs differ.
///
/// Both variants are **strict** — the only axis this enum moves is the goal
/// symbol. The four constructs that differ between the goals:
///
/// | construct | `Module` | `Script` |
/// | --- | --- | --- |
/// | `await` as an identifier / binding / label / class name | reserved | allowed (`[~Await]`) |
/// | top-level `await` *expression* | allowed | syntax error |
/// | `import.meta` | allowed | syntax error |
/// | top-level `import` / `export` *declarations* | allowed | syntax error |
///
/// Dynamic `import(...)` is a call expression valid under both goals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Goal {
    /// `ParseModule` — the default. Top-level `import`/`export`, `import.meta`,
    /// and top-level `await` expressions are available; `await` is reserved as
    /// an identifier. Mirrors acorn's `sourceType: 'module'`.
    #[default]
    Module,
    /// `ParseScript` — `await` is an ordinary identifier (the top level is
    /// `[~Await]`); `import`/`export` declarations, `import.meta`, and top-level
    /// `await` expressions are syntax errors. Still strict (`tsv` has no sloppy
    /// mode), so `with`, legacy octal, etc. remain rejected.
    Script,
}

impl Goal {
    /// The acorn `sourceType` string this goal serializes to in the public AST
    /// (`Program.sourceType`).
    pub const fn source_type(self) -> &'static str {
        match self {
            Goal::Module => "module",
            Goal::Script => "script",
        }
    }
}
