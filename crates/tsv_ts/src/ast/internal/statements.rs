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
    TSTypeAliasDeclaration, TSTypeAnnotation, TSTypeParameterDeclaration,
};

/// Statement node type
#[derive(Debug, Clone)]
pub enum Statement {
    ExpressionStatement(ExpressionStatement),
    VariableDeclaration(VariableDeclaration),
    TSTypeAliasDeclaration(TSTypeAliasDeclaration),
    TSInterfaceDeclaration(TSInterfaceDeclaration),
    TSDeclareFunction(TSDeclareFunction),
    TSEnumDeclaration(TSEnumDeclaration),
    TSModuleDeclaration(TSModuleDeclaration),
    ReturnStatement(ReturnStatement),
    BlockStatement(BlockStatement),
    FunctionDeclaration(FunctionDeclaration),
    ClassDeclaration(ClassDeclaration),
    ExportNamedDeclaration(ExportNamedDeclaration),
    ExportDefaultDeclaration(ExportDefaultDeclaration),
    ExportAllDeclaration(ExportAllDeclaration),
    TSExportAssignment(TSExportAssignment),
    ImportDeclaration(ImportDeclaration),
    TSImportEqualsDeclaration(TSImportEqualsDeclaration),
    // Control flow statements
    IfStatement(IfStatement),
    ForStatement(ForStatement),
    ForInStatement(ForInStatement),
    ForOfStatement(ForOfStatement),
    WhileStatement(WhileStatement),
    DoWhileStatement(DoWhileStatement),
    SwitchStatement(SwitchStatement),
    TryStatement(TryStatement),
    ThrowStatement(ThrowStatement),
    BreakStatement(BreakStatement),
    ContinueStatement(ContinueStatement),
    LabeledStatement(LabeledStatement),
    EmptyStatement(EmptyStatement),
    DebuggerStatement(DebuggerStatement),
}

impl Statement {
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
pub struct ExpressionStatement {
    pub expression: Expression,
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
pub struct BlockStatement {
    pub body: Vec<Statement>,
    pub span: Span,
}

/// Return statement: `return expr;` or `return;`
///
/// The argument is optional for void returns.
#[derive(Debug, Clone)]
pub struct ReturnStatement {
    pub argument: Option<Expression>,
    pub span: Span,
}

//
// Control Flow Statements
//

/// If statement: `if (test) consequent` or `if (test) consequent else alternate`
#[derive(Debug, Clone)]
pub struct IfStatement {
    pub test: Expression,
    pub consequent: Box<Statement>,
    pub alternate: Option<Box<Statement>>,
    pub span: Span,
}

/// For statement: `for (init; test; update) body`
#[derive(Debug, Clone)]
pub struct ForStatement {
    /// Initialization: variable declaration or expression (or None)
    pub init: Option<ForInit>,
    /// Test condition (or None for infinite loop)
    pub test: Option<Expression>,
    /// Update expression (or None)
    pub update: Option<Expression>,
    pub body: Box<Statement>,
    pub span: Span,
}

/// For statement initialization - either a variable declaration or expression
#[derive(Debug, Clone)]
pub enum ForInit {
    VariableDeclaration(VariableDeclaration),
    Expression(Expression),
}

/// For-in statement: `for (left in right) body`
#[derive(Debug, Clone)]
pub struct ForInStatement {
    /// Left side: variable declaration or expression pattern
    pub left: ForInOfLeft,
    pub right: Expression,
    pub body: Box<Statement>,
    pub span: Span,
}

/// For-of statement: `for (left of right) body`
#[derive(Debug, Clone)]
pub struct ForOfStatement {
    /// Left side: variable declaration or expression pattern
    pub left: ForInOfLeft,
    pub right: Expression,
    /// Whether this is `for await (... of ...)`
    pub r#await: bool,
    pub body: Box<Statement>,
    pub span: Span,
}

/// Left side of for-in/for-of: either a variable declaration or expression pattern
#[derive(Debug, Clone)]
pub enum ForInOfLeft {
    VariableDeclaration(VariableDeclaration),
    Pattern(Expression),
}

/// While statement: `while (test) body`
#[derive(Debug, Clone)]
pub struct WhileStatement {
    pub test: Expression,
    pub body: Box<Statement>,
    pub span: Span,
}

/// Do-while statement: `do body while (test)`
#[derive(Debug, Clone)]
pub struct DoWhileStatement {
    pub body: Box<Statement>,
    pub test: Expression,
    pub span: Span,
}

/// Switch statement: `switch (discriminant) { cases }`
#[derive(Debug, Clone)]
pub struct SwitchStatement {
    pub discriminant: Expression,
    pub cases: Vec<SwitchCase>,
    pub span: Span,
}

/// Switch case: `case test: consequent` or `default: consequent`
#[derive(Debug, Clone)]
pub struct SwitchCase {
    /// Test expression, or None for `default:`
    pub test: Option<Expression>,
    pub consequent: Vec<Statement>,
    pub span: Span,
}

/// Try statement: `try { block } catch (param) { handler } finally { finalizer }`
#[derive(Debug, Clone)]
pub struct TryStatement {
    pub block: BlockStatement,
    pub handler: Option<CatchClause>,
    pub finalizer: Option<BlockStatement>,
    pub span: Span,
}

/// Catch clause: `catch (param) { body }`
#[derive(Debug, Clone)]
pub struct CatchClause {
    /// Catch parameter, or None for `catch { }` (optional catch binding)
    pub param: Option<Expression>,
    pub body: BlockStatement,
    pub span: Span,
}

/// Throw statement: `throw argument`
#[derive(Debug, Clone)]
pub struct ThrowStatement {
    pub argument: Expression,
    pub span: Span,
}

/// Break statement: `break` or `break label`
#[derive(Debug, Clone)]
pub struct BreakStatement {
    pub label: Option<Identifier>,
    pub span: Span,
}

/// Continue statement: `continue` or `continue label`
#[derive(Debug, Clone)]
pub struct ContinueStatement {
    pub label: Option<Identifier>,
    pub span: Span,
}

/// Labeled statement: `label: statement`
#[derive(Debug, Clone)]
pub struct LabeledStatement {
    pub label: Identifier,
    pub body: Box<Statement>,
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
pub struct FunctionDeclaration {
    /// Function name (required for declarations, optional for export default)
    pub id: Option<Identifier>,
    /// Type parameters (TypeScript generics): `function fn<T>() {}`
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Vec<Expression>,
    /// Return type annotation (e.g., `: number` in `function fn(): number {}`)
    pub return_type: Option<TSTypeAnnotation>,
    /// Function body (block statement with statements)
    pub body: BlockStatement,
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
    /// ES2024 Explicit Resource Management: `using resource = getResource();`
    Using = 3,
    /// ES2024 Explicit Resource Management: `await using resource = getAsyncResource();`
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
}

#[derive(Debug, Clone)]
pub struct VariableDeclaration {
    pub kind: VariableDeclarationKind,
    pub declarations: Vec<VariableDeclarator>,
    /// Whether this is an ambient declaration (`declare const x: T;`)
    pub declare: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct VariableDeclarator {
    /// The binding pattern (Identifier, ArrayPattern, or ObjectPattern)
    pub id: Expression,
    pub init: Option<Expression>,
    /// Definite assignment assertion (`!` after identifier, e.g., `let x!: string;`)
    pub definite: bool,
    pub span: Span,
}
