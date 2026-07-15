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
    // Inline by value: the layout favors traversal locality over node size, so
    // these declarations are kept inline rather than arena-boxed.
    TSTypeAliasDeclaration(TSTypeAliasDeclaration<'arena>),
    TSInterfaceDeclaration(TSInterfaceDeclaration<'arena>),
    TSDeclareFunction(TSDeclareFunction<'arena>),
    TSEnumDeclaration(TSEnumDeclaration<'arena>),
    TSModuleDeclaration(TSModuleDeclaration<'arena>),
    ReturnStatement(ReturnStatement<'arena>),
    BlockStatement(BlockStatement<'arena>),
    FunctionDeclaration(FunctionDeclaration<'arena>),
    ClassDeclaration(ClassDeclaration<'arena>),
    ExportNamedDeclaration(ExportNamedDeclaration<'arena>),
    ExportDefaultDeclaration(ExportDefaultDeclaration<'arena>),
    ExportAllDeclaration(ExportAllDeclaration<'arena>),
    TSExportAssignment(TSExportAssignment<'arena>),
    TSNamespaceExportDeclaration(TSNamespaceExportDeclaration<'arena>),
    ImportDeclaration(ImportDeclaration<'arena>),
    TSImportEqualsDeclaration(TSImportEqualsDeclaration<'arena>),
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
#[derive(Debug, Clone)]
pub struct ExpressionStatement<'arena> {
    pub expression: Expression<'arena>,
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
    pub argument: Option<Expression<'arena>>,
    pub span: Span,
}

//
// Control Flow Statements
//

/// If statement: `if (test) consequent` or `if (test) consequent else alternate`
#[derive(Debug, Clone)]
pub struct IfStatement<'arena> {
    pub test: Expression<'arena>,
    pub consequent: &'arena Statement<'arena>,
    pub alternate: Option<&'arena Statement<'arena>>,
    pub span: Span,
}

/// For statement: `for (init; test; update) body`
#[derive(Debug, Clone)]
pub struct ForStatement<'arena> {
    /// Initialization: variable declaration or expression (or None)
    pub init: Option<ForInit<'arena>>,
    /// Test condition (or None for infinite loop)
    pub test: Option<Expression<'arena>>,
    /// Update expression (or None)
    pub update: Option<Expression<'arena>>,
    pub body: &'arena Statement<'arena>,
    pub span: Span,
}

/// For statement initialization - either a variable declaration or expression
#[derive(Debug, Clone)]
pub enum ForInit<'arena> {
    VariableDeclaration(VariableDeclaration<'arena>),
    Expression(Expression<'arena>),
}

/// For-in statement: `for (left in right) body`
#[derive(Debug, Clone)]
pub struct ForInStatement<'arena> {
    /// Left side: variable declaration or expression pattern
    pub left: ForInOfLeft<'arena>,
    pub right: Expression<'arena>,
    pub body: &'arena Statement<'arena>,
    pub span: Span,
}

/// For-of statement: `for (left of right) body`
#[derive(Debug, Clone)]
pub struct ForOfStatement<'arena> {
    /// Left side: variable declaration or expression pattern
    pub left: ForInOfLeft<'arena>,
    pub right: Expression<'arena>,
    /// Whether this is `for await (... of ...)`
    pub r#await: bool,
    pub body: &'arena Statement<'arena>,
    pub span: Span,
}

/// Left side of for-in/for-of: either a variable declaration or expression pattern
#[derive(Debug, Clone)]
pub enum ForInOfLeft<'arena> {
    VariableDeclaration(VariableDeclaration<'arena>),
    Pattern(Expression<'arena>),
}

/// While statement: `while (test) body`
#[derive(Debug, Clone)]
pub struct WhileStatement<'arena> {
    pub test: Expression<'arena>,
    pub body: &'arena Statement<'arena>,
    pub span: Span,
}

/// Do-while statement: `do body while (test)`
#[derive(Debug, Clone)]
pub struct DoWhileStatement<'arena> {
    pub body: &'arena Statement<'arena>,
    pub test: Expression<'arena>,
    pub span: Span,
}

/// Switch statement: `switch (discriminant) { cases }`
#[derive(Debug, Clone)]
pub struct SwitchStatement<'arena> {
    pub discriminant: Expression<'arena>,
    pub cases: &'arena [SwitchCase<'arena>],
    pub span: Span,
}

/// Switch case: `case test: consequent` or `default: consequent`
#[derive(Debug, Clone)]
pub struct SwitchCase<'arena> {
    /// Test expression, or None for `default:`
    pub test: Option<Expression<'arena>>,
    pub consequent: &'arena [Statement<'arena>],
    pub span: Span,
}

/// Try statement: `try { block } catch (param) { handler } finally { finalizer }`
#[derive(Debug, Clone)]
pub struct TryStatement<'arena> {
    pub block: BlockStatement<'arena>,
    pub handler: Option<CatchClause<'arena>>,
    pub finalizer: Option<BlockStatement<'arena>>,
    pub span: Span,
}

/// Catch clause: `catch (param) { body }`
#[derive(Debug, Clone)]
pub struct CatchClause<'arena> {
    /// Catch parameter, or None for `catch { }` (optional catch binding)
    pub param: Option<Expression<'arena>>,
    pub body: BlockStatement<'arena>,
    pub span: Span,
}

/// Throw statement: `throw argument`
#[derive(Debug, Clone)]
pub struct ThrowStatement<'arena> {
    pub argument: Expression<'arena>,
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

#[derive(Debug, Clone)]
pub struct VariableDeclarator<'arena> {
    /// The binding pattern (Identifier, ArrayPattern, or ObjectPattern)
    pub id: Expression<'arena>,
    pub init: Option<Expression<'arena>>,
    /// Definite assignment assertion (`!` after identifier, e.g., `let x!: string;`)
    pub definite: bool,
    pub span: Span,
}
