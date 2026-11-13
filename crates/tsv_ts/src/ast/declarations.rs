//! TypeScript declaration nodes
//!
//! Contains TS-specific declarations: interfaces, enums, namespaces/modules,
//! and declare function.

use tsv_lang::Span;

use super::{
    AstView, EnumMem, Expr, ExprId, Heritage, Identifier, Literal, ModuleDeclId, Run, StmtId,
    TSEntityName, TSTypeAnnotation, TSTypeParameterDeclaration, TSTypeParameterInstantiation,
    TypeMember,
};

/// Interface declaration: `interface Foo { ... }` or `interface Foo extends Bar { ... }`
#[derive(Debug, Clone)]
pub struct TSInterfaceDeclaration {
    pub id: Identifier,
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub extends: Run<Heritage>,
    pub body: TSInterfaceBody,
    pub declare: bool,
    pub span: Span,
}

/// Interface heritage: `extends Foo, Bar`
#[derive(Debug, Clone)]
pub struct TSInterfaceHeritage {
    pub expression: TSEntityName,
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub span: Span,
}

/// Interface body: `{ members }`
#[derive(Debug, Clone)]
pub struct TSInterfaceBody {
    pub body: Run<TypeMember>,
    pub span: Span,
}

/// Declare function: `declare function foo(): void`
///
/// Also used for functions inside `declare namespace` where `declare` is implicit,
/// and for function overload signatures (no body).
#[derive(Debug, Clone)]
pub struct TSDeclareFunction {
    pub id: Identifier,
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Run<Expr>,
    pub return_type: Option<TSTypeAnnotation>,
    /// Whether to print the `declare` keyword.
    /// True for top-level `declare function`, false inside `declare namespace`.
    pub declare: bool,
    /// Whether this is an async function.
    pub r#async: bool,
    /// Whether this is a generator function.
    pub generator: bool,
    pub span: Span,
}

/// TypeScript enum declaration: `enum Foo { A, B }`, `const enum Foo { A = 1 }`
///
/// Represents an enum declaration. Enums can be:
/// - Regular: `enum Foo { A, B }`
/// - Const: `const enum Foo { A, B }` (inlined at compile time)
/// - Declare: `declare enum Foo { A, B }` (ambient declaration)
/// - Declare const: `declare const enum Foo { A, B }`
#[derive(Debug, Clone)]
pub struct TSEnumDeclaration {
    /// Enum name
    pub id: Identifier,
    /// Enum members
    pub members: Run<EnumMem>,
    /// Whether this is a const enum
    pub r#const: bool,
    /// Whether this is an ambient declaration (declare enum)
    pub declare: bool,
}

/// TypeScript enum member: `A`, `A = 1`, `A = "value"`
///
/// Represents a single member in an enum declaration.
#[derive(Debug, Clone)]
pub struct TSEnumMember {
    /// Member name (identifier or computed)
    pub id: TSEnumMemberId,
    /// Optional initializer expression
    pub initializer: Option<ExprId>,
    /// The member's full source extent, running through a grouping paren the
    /// initializer's own span stops inside (`A = (a, b)`) — acorn's shape, and
    /// what `prettier-ignore` must reproduce verbatim. Ask
    /// [`Self::printed_end`] instead wherever the question is where this
    /// member's DOC stops.
    pub span: Span,
}

impl TSEnumMember {
    /// Where the member's own doc stops printing — the sibling of
    /// [`Expression::printed_end`], and the anchor every comment seam in the
    /// enum body must use.
    ///
    /// ⚠️ It is NOT `span.end`. A grouping paren is no node, so `span` runs past
    /// a shell the initializer's span stops inside, and the paren the member
    /// prints is one the PRINTER synthesizes rather than the author's byte. A
    /// seam anchored at `span.end` therefore starts past the shell's interior,
    /// where nothing else emits — a DROPPED comment
    /// (`docs/comments.md` §The element-comma seam).
    pub fn printed_end(&self, view: AstView<'_>) -> u32 {
        view.opt(self.initializer)
            .map_or_else(|| self.id.span().end, |e| e.printed_end(view))
    }
}

/// Enum member id: can be an identifier or a string literal (for computed names)
#[derive(Debug, Clone)]
pub enum TSEnumMemberId {
    Identifier(Identifier),
    /// String literal for computed names like `"hello"` in `enum { "hello" = 1 }`
    String(Literal),
}

impl TSEnumMemberId {
    pub fn span(&self) -> Span {
        match self {
            TSEnumMemberId::Identifier(id) => id.span,
            TSEnumMemberId::String(lit) => lit.span,
        }
    }
}

/// TypeScript module/namespace declaration: `namespace Utils { ... }` or `module Utils { ... }`
///
/// The `module` keyword is the older syntax, while `namespace` is the modern syntax.
/// Both produce the same AST structure. For nested namespaces like `namespace Outer.Inner`,
/// the parser creates nested TSModuleDeclaration nodes.
#[derive(Debug, Clone)]
pub struct TSModuleDeclaration {
    /// Module/namespace name - identifier for regular namespaces, string literal for ambient modules
    pub id: TSModuleName,
    /// Module body - either a block or nested module declaration (for `A.B.C`)
    /// `None` for shorthand ambient modules: `declare module 'name';`
    pub body: Option<TSModuleDeclarationBody>,
    /// Whether this is an ambient declaration (`declare namespace/module`)
    pub declare: bool,
    /// The keyword used: `namespace` or `module`
    pub kind: TSModuleDeclarationKind,
    /// For `declare global {}` - uses module kind but has special semantics
    pub global: bool,
    pub span: Span,
}

/// Module/namespace name - can be an identifier or a string literal
#[derive(Debug, Clone)]
pub enum TSModuleName {
    /// Regular identifier: `namespace Foo { }`
    Identifier(Identifier),
    /// String literal for ambient modules: `declare module 'name' { }`
    Literal(Literal),
}

impl TSModuleName {
    pub fn span(&self) -> Span {
        match self {
            TSModuleName::Identifier(id) => id.span,
            TSModuleName::Literal(lit) => lit.span,
        }
    }
}

/// The keyword used in a module/namespace declaration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TSModuleDeclarationKind {
    /// `namespace` keyword
    Namespace,
    /// `module` keyword (legacy syntax, same semantics as `namespace`)
    Module,
}

/// Body of a TypeScript module declaration
#[derive(Debug, Clone)]
pub enum TSModuleDeclarationBody {
    /// Block body with statements: `namespace A { ... }`
    TSModuleBlock(TSModuleBlock),
    /// Nested module declaration: `namespace A.B { ... }` - the B part
    TSModuleDeclaration(ModuleDeclId),
}

/// TypeScript module block: the `{ ... }` part of a namespace/module declaration
#[derive(Debug, Clone)]
pub struct TSModuleBlock {
    /// Statements inside the module block
    pub body: Run<StmtId>,
    pub span: Span,
}
