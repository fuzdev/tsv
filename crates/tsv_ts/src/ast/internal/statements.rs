//! Statement nodes
//!
//! Contains the Statement enum and all statement types including
//! control flow, variable declarations, function declarations,
//! and block statements.

use tsv_lang::Span;

use super::{
    ClassDeclaration, ExportAllDeclaration, ExportDefaultDeclaration, ExportNamedDeclaration,
    Expression, Identifier, ImportDeclaration, TSDeclareFunction, TSEnumDeclaration,
    TSExportAssignment, TSImportEqualsDeclaration, TSInterfaceDeclaration, TSModuleDeclaration,
    TSNamespaceExportDeclaration, TSTypeAliasDeclaration, TSTypeAnnotation,
    TSTypeParameterDeclaration,
};

/// Statement node type
#[derive(Debug, Clone)]
pub enum Statement<'arena> {
    ExpressionStatement(ExpressionStatement<'arena>),
    VariableDeclaration(VariableDeclaration<'arena>),
    // Arena-boxed, unlike every inline variant here: these are the widest
    // declaration heads, and their width would otherwise be paid on every
    // `Statement` SLOT — every element of every `&[Statement]` body and every
    // `?`-propagation — not just on the declaration that needs it.
    //
    // Most are boxed because they are RARE, so the allocation is free: over four
    // app corpora `TSDeclareFunction`, `ExportAllDeclaration`,
    // `TSImportEqualsDeclaration`, `TSEnumDeclaration` and `TSExportAssignment`
    // occur 0.000% of the time, `TSModuleDeclaration` 0.007–0.019%,
    // `TSTypeAliasDeclaration` 0.019–0.028%, `ExportDefaultDeclaration`
    // 0.016–0.047%, `ClassDeclaration` 0.000–0.194%, `FunctionDeclaration`
    // 0.026–0.204% and `TSInterfaceDeclaration` 0.028–0.128%.
    //
    // `ImportDeclaration` (6.8–11.7% of statements) and `ExportNamedDeclaration`
    // (2.4–4.0%) are the exception: they are boxed because they are the CEILING,
    // not because they are rare. Rarity is what makes boxing free, and it is not
    // the only reason to box — the struct is copied into the arena instead of into
    // the enum, so the copy volume is unchanged and the cost is one bump pointer,
    // while all of the file's other statement slots get 24 bytes narrower. With
    // both inline the enum is 96 bytes; boxed it is 72, which is what the
    // `size_of` assert in this module's parent pins. The next-widest inline
    // variant is `TryStatement` at 64, so the ladder stops here.
    //
    // (The loop and `try` heads below hold their own heads by reference for the
    // same reason, one level down; the `Expression`-holding heads below hold
    // theirs by reference for a different one — see `ExpressionStatement`.)
    TSTypeAliasDeclaration(&'arena TSTypeAliasDeclaration<'arena>),
    TSInterfaceDeclaration(&'arena TSInterfaceDeclaration<'arena>),
    TSDeclareFunction(&'arena TSDeclareFunction<'arena>),
    TSEnumDeclaration(&'arena TSEnumDeclaration<'arena>),
    TSModuleDeclaration(&'arena TSModuleDeclaration<'arena>),
    ReturnStatement(ReturnStatement<'arena>),
    BlockStatement(BlockStatement<'arena>),
    FunctionDeclaration(&'arena FunctionDeclaration<'arena>),
    ClassDeclaration(&'arena ClassDeclaration<'arena>),
    ExportNamedDeclaration(&'arena ExportNamedDeclaration<'arena>),
    ExportDefaultDeclaration(&'arena ExportDefaultDeclaration<'arena>),
    ExportAllDeclaration(&'arena ExportAllDeclaration<'arena>),
    TSExportAssignment(&'arena TSExportAssignment<'arena>),
    TSNamespaceExportDeclaration(TSNamespaceExportDeclaration<'arena>),
    ImportDeclaration(&'arena ImportDeclaration<'arena>),
    TSImportEqualsDeclaration(&'arena TSImportEqualsDeclaration<'arena>),
    // Control flow statements
    IfStatement(IfStatement<'arena>),
    ForStatement(ForStatement<'arena>),
    ForInStatement(ForInStatement<'arena>),
    ForOfStatement(ForOfStatement<'arena>),
    WhileStatement(WhileStatement<'arena>),
    DoWhileStatement(DoWhileStatement<'arena>),
    SwitchStatement(SwitchStatement<'arena>),
    TryStatement(TryStatement<'arena>),
    ThrowStatement(ThrowStatement<'arena>),
    BreakStatement(BreakStatement<'arena>),
    ContinueStatement(ContinueStatement<'arena>),
    LabeledStatement(LabeledStatement<'arena>),
    EmptyStatement(EmptyStatement),
    DebuggerStatement(DebuggerStatement),
}

impl<'arena> Statement<'arena> {
    pub fn span(&self) -> Span {
        match self {
            Statement::ExpressionStatement(stmt) => stmt.span,
            Statement::VariableDeclaration(decl) => decl.span,
            Statement::TSTypeAliasDeclaration(decl) => decl.span,
            Statement::TSInterfaceDeclaration(decl) => decl.span,
            Statement::TSDeclareFunction(decl) => decl.span,
            Statement::TSEnumDeclaration(decl) => decl.span,
            Statement::TSModuleDeclaration(decl) => decl.span,
            Statement::ReturnStatement(stmt) => stmt.span,
            Statement::BlockStatement(block) => block.span,
            Statement::FunctionDeclaration(decl) => decl.span,
            Statement::ClassDeclaration(decl) => decl.span,
            Statement::ExportNamedDeclaration(decl) => decl.span,
            Statement::ExportDefaultDeclaration(decl) => decl.span,
            Statement::ExportAllDeclaration(decl) => decl.span,
            Statement::TSExportAssignment(decl) => decl.span,
            Statement::TSNamespaceExportDeclaration(decl) => decl.span,
            Statement::ImportDeclaration(decl) => decl.span,
            Statement::TSImportEqualsDeclaration(decl) => decl.span,
            // Control flow statements
            Statement::IfStatement(stmt) => stmt.span,
            Statement::ForStatement(stmt) => stmt.span,
            Statement::ForInStatement(stmt) => stmt.span,
            Statement::ForOfStatement(stmt) => stmt.span,
            Statement::WhileStatement(stmt) => stmt.span,
            Statement::DoWhileStatement(stmt) => stmt.span,
            Statement::SwitchStatement(stmt) => stmt.span,
            Statement::TryStatement(stmt) => stmt.span,
            Statement::ThrowStatement(stmt) => stmt.span,
            Statement::BreakStatement(stmt) => stmt.span,
            Statement::ContinueStatement(stmt) => stmt.span,
            Statement::LabeledStatement(stmt) => stmt.span,
            Statement::EmptyStatement(stmt) => stmt.span,
            Statement::DebuggerStatement(stmt) => stmt.span,
        }
    }
}

/// Expression statement: an expression used as a statement
///
/// The expression is an arena reference, not an inline value, and this is where
/// every `Expression`-holding statement head takes the same shape. The expression
/// parser threads a `ParsedExpr`, which already holds an `&'arena Expression` — so
/// an inline slot is a 72-byte copy OUT of that allocation, leaving the arena copy
/// dead. Naming the slot by reference removes the copy and adds nothing; it is not
/// the rare-variant boxing trade above, and needs no rarity argument
/// (`parse_expression_ref`).
#[derive(Debug, Clone)]
pub struct ExpressionStatement<'arena> {
    pub expression: &'arena Expression<'arena>,
    pub span: Span,
    /// True when this is a directive prologue entry — an unparenthesized
    /// string-literal statement in the leading run of a `Program` or function
    /// body (e.g. `"use strict";`). Directives are printed verbatim from source
    /// and emit acorn's `directive` field in the public AST.
    pub is_directive: bool,
}

/// Block statement: `{ stmt1; stmt2; }`
///
/// A block of statements surrounded by braces. Used for:
/// - Function bodies
/// - If/else bodies (future)
/// - Loop bodies (future)
#[derive(Debug, Clone)]
pub struct BlockStatement<'arena> {
    pub body: &'arena [Statement<'arena>],
    pub span: Span,
}

/// Return statement: `return expr;` or `return;`
///
/// The argument is optional for void returns.
#[derive(Debug, Clone)]
pub struct ReturnStatement<'arena> {
    pub argument: Option<&'arena Expression<'arena>>,
    pub span: Span,
}

//
// Control Flow Statements
//

/// If statement: `if (test) consequent` or `if (test) consequent else alternate`
#[derive(Debug, Clone)]
pub struct IfStatement<'arena> {
    pub test: &'arena Expression<'arena>,
    pub consequent: &'arena Statement<'arena>,
    pub alternate: Option<&'arena Statement<'arena>>,
    pub span: Span,
}

/// For statement: `for (init; test; update) body`
///
/// The three head slots are arena references, not inline values. `Statement` is a
/// by-value enum stored inline in `&'arena [Statement]`, so every variant pays its
/// widest sibling's size on **every** element of **every** statement slice and on
/// every `?`-propagation copy out of the parser — while a pointer chase is paid only
/// where the variant is actually spelled. `for (;;)` is 0.05–0.22% of statements in
/// real source, and inlining `ForInit` + two `Option<Expression>` here set the whole
/// enum's size on its own. The same trade is taken by `ForInStatement`,
/// `ForOfStatement` and `TryStatement`; the frequent declaration variants above keep
/// their inline layout, where it is the right way round.
#[derive(Debug, Clone)]
pub struct ForStatement<'arena> {
    /// Initialization: variable declaration or expression (or None)
    pub init: Option<&'arena ForInit<'arena>>,
    /// Test condition (or None for infinite loop)
    pub test: Option<&'arena Expression<'arena>>,
    /// Update expression (or None)
    pub update: Option<&'arena Expression<'arena>>,
    pub body: &'arena Statement<'arena>,
    pub span: Span,
}

/// For statement initialization - either a variable declaration or expression
///
/// The `Expression` arm stays INLINE, unlike the `Expression`-holding statement heads
/// above: a for-head's expression comes back from `parse_expression_no_in` (and, for a
/// for-in/of LHS, through the cover-grammar refinement `to_assignable`) as an owned
/// value, not as the `&'arena Expression` the expression spine threads — so naming the
/// slot by reference would ADD an allocation rather than remove a copy, which is the
/// reverse of the trade `ExpressionStatement` takes. `ForInit` is itself reached only
/// through `Option<&'arena ForInit>`, so its width sets nothing.
#[derive(Debug, Clone)]
pub enum ForInit<'arena> {
    VariableDeclaration(VariableDeclaration<'arena>),
    Expression(Expression<'arena>),
}

/// For-in statement: `for (left in right) body`
///
/// Head slots are arena references for the density reason on `ForStatement`.
#[derive(Debug, Clone)]
pub struct ForInStatement<'arena> {
    /// Left side: variable declaration or expression pattern
    pub left: &'arena ForInOfLeft<'arena>,
    pub right: &'arena Expression<'arena>,
    pub body: &'arena Statement<'arena>,
    pub span: Span,
}

/// For-of statement: `for (left of right) body`
///
/// Head slots are arena references for the density reason on `ForStatement`.
#[derive(Debug, Clone)]
pub struct ForOfStatement<'arena> {
    /// Left side: variable declaration or expression pattern
    pub left: &'arena ForInOfLeft<'arena>,
    pub right: &'arena Expression<'arena>,
    /// Whether this is `for await (... of ...)`
    pub r#await: bool,
    pub body: &'arena Statement<'arena>,
    pub span: Span,
}

/// Left side of for-in/for-of: either a variable declaration or expression pattern
///
/// The `Pattern` arm stays inline for the reason on `ForInit`: its producer returns an
/// owned `Expression`, and this enum is reached only through `&'arena ForInOfLeft`.
#[derive(Debug, Clone)]
pub enum ForInOfLeft<'arena> {
    VariableDeclaration(VariableDeclaration<'arena>),
    Pattern(Expression<'arena>),
}

/// While statement: `while (test) body`
#[derive(Debug, Clone)]
pub struct WhileStatement<'arena> {
    pub test: &'arena Expression<'arena>,
    pub body: &'arena Statement<'arena>,
    pub span: Span,
}

/// Do-while statement: `do body while (test)`
#[derive(Debug, Clone)]
pub struct DoWhileStatement<'arena> {
    pub body: &'arena Statement<'arena>,
    pub test: &'arena Expression<'arena>,
    pub span: Span,
}

/// Switch statement: `switch (discriminant) { cases }`
#[derive(Debug, Clone)]
pub struct SwitchStatement<'arena> {
    pub discriminant: &'arena Expression<'arena>,
    pub cases: &'arena [SwitchCase<'arena>],
    pub span: Span,
}

/// Switch case: `case test: consequent` or `default: consequent`
#[derive(Debug, Clone)]
pub struct SwitchCase<'arena> {
    /// Test expression, or None for `default:`
    pub test: Option<&'arena Expression<'arena>>,
    pub consequent: &'arena [Statement<'arena>],
    pub span: Span,
}

/// Try statement: `try { block } catch (param) { handler } finally { finalizer }`
#[derive(Debug, Clone)]
pub struct TryStatement<'arena> {
    pub block: BlockStatement<'arena>,
    /// Arena reference rather than an inline `CatchClause` (104 B on its own) for the
    /// density reason on `ForStatement`.
    pub handler: Option<&'arena CatchClause<'arena>>,
    pub finalizer: Option<BlockStatement<'arena>>,
    pub span: Span,
}

/// Catch clause: `catch (param) { body }`
#[derive(Debug, Clone)]
pub struct CatchClause<'arena> {
    /// Catch parameter, or None for `catch { }` (optional catch binding).
    ///
    /// Inline for the reason on `ForInit`: the parser builds this one as an owned
    /// value rather than through the expression spine, so a reference would add an
    /// allocation instead of removing a copy — and a `CatchClause` is reached only
    /// through `Option<&'arena CatchClause>`, so its width sets nothing.
    pub param: Option<Expression<'arena>>,
    pub body: BlockStatement<'arena>,
    pub span: Span,
}

/// Throw statement: `throw argument`
#[derive(Debug, Clone)]
pub struct ThrowStatement<'arena> {
    pub argument: &'arena Expression<'arena>,
    pub span: Span,
}

/// Break statement: `break` or `break label`
#[derive(Debug, Clone)]
pub struct BreakStatement<'arena> {
    pub label: Option<Identifier<'arena>>,
    pub span: Span,
}

/// Continue statement: `continue` or `continue label`
#[derive(Debug, Clone)]
pub struct ContinueStatement<'arena> {
    pub label: Option<Identifier<'arena>>,
    pub span: Span,
}

/// Labeled statement: `label: statement`
#[derive(Debug, Clone)]
pub struct LabeledStatement<'arena> {
    pub label: Identifier<'arena>,
    pub body: &'arena Statement<'arena>,
    pub span: Span,
}

/// Empty statement: `;`
#[derive(Debug, Clone)]
pub struct EmptyStatement {
    pub span: Span,
}

/// Debugger statement: `debugger;`
#[derive(Debug, Clone)]
pub struct DebuggerStatement {
    pub span: Span,
}

//
// Declarations
//

/// Function declaration: `function foo(x) { return x + 1; }`
///
/// For regular declarations, the id (function name) is required.
/// For `export default function() {}`, the name is optional.
/// Declarations are hoisted and can be called before they appear in source.
#[derive(Debug, Clone)]
pub struct FunctionDeclaration<'arena> {
    /// Function name (required for declarations, optional for export default)
    pub id: Option<Identifier<'arena>>,
    /// Type parameters (TypeScript generics): `function fn<T>() {}`
    pub type_parameters: Option<TSTypeParameterDeclaration<'arena>>,
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: &'arena [Expression<'arena>],
    /// Return type annotation (e.g., `: number` in `function fn(): number {}`)
    pub return_type: Option<TSTypeAnnotation<'arena>>,
    /// Function body (block statement with statements)
    pub body: BlockStatement<'arena>,
    /// Whether this is a generator function (`function*`)
    pub generator: bool,
    /// Whether this is an async function (`async function`)
    pub r#async: bool,
    /// Position of opening paren for params (for comment detection)
    pub params_start: u32,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VariableDeclarationKind {
    Const = 0,
    Let = 1,
    Var = 2,
    /// Explicit Resource Management: `using resource = getResource();`
    Using = 3,
    /// Explicit Resource Management: `await using resource = getAsyncResource();`
    AwaitUsing = 4,
}

impl VariableDeclarationKind {
    /// Returns the string representation of the variable declaration kind
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Const => "const",
            Self::Let => "let",
            Self::Var => "var",
            Self::Using => "using",
            Self::AwaitUsing => "await using",
        }
    }

    /// The kind's source tokens, in order — `await using` is **two**.
    ///
    /// A printer must locate these rather than measure [`as_str`](Self::as_str): the
    /// gap *between* two words is a source position an author can write a comment in
    /// (`await /* c */ using`), and measuring the joined text never scans it, so the
    /// comment is dropped.
    #[inline]
    pub const fn words(self) -> &'static [&'static str] {
        match self {
            Self::Const => &["const"],
            Self::Let => &["let"],
            Self::Var => &["var"],
            Self::Using => &["using"],
            Self::AwaitUsing => &["await", "using"],
        }
    }
}

#[derive(Debug, Clone)]
pub struct VariableDeclaration<'arena> {
    pub kind: VariableDeclarationKind,
    pub declarations: &'arena [VariableDeclarator<'arena>],
    /// Whether this is an ambient declaration (`declare const x: T;`)
    pub declare: bool,
    pub span: Span,
}

/// One declarator of a variable declaration: `x`, `x = init`, `{a, b} = init`.
///
/// Both slots are `&'arena` references rather than inline `Expression`s, for the
/// density reason on [`super::Property`] — 160 B → 32 on every element of every
/// `declarations` slice, and no allocation added, because the parser's expression
/// spine already hands back an arena reference.
#[derive(Debug, Clone)]
pub struct VariableDeclarator<'arena> {
    /// The binding pattern (Identifier, ArrayPattern, or ObjectPattern)
    pub id: &'arena Expression<'arena>,
    pub init: Option<&'arena Expression<'arena>>,
    /// Definite assignment assertion (`!` after identifier, e.g., `let x!: string;`)
    pub definite: bool,
    pub span: Span,
}
