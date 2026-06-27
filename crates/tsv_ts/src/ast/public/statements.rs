//! Statement types for public AST

use serde::Serialize;
use std::borrow::Cow;

use super::classes::ClassDeclaration;
use super::declarations::{
    TSDeclareFunction, TSEnumDeclaration, TSInterfaceDeclaration, TSModuleDeclaration,
};
use super::modules::{
    ExportAllDeclaration, ExportDefaultDeclaration, ExportNamedDeclaration, ImportDeclaration,
    TSExportAssignment, TSImportEqualsDeclaration,
};
use super::types::{TSTypeAliasDeclaration, TSTypeAnnotation, TSTypeParameterDeclaration};
use super::{Expression, Identifier, SourceLocation, is_false};

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Statement<'src> {
    ExpressionStatement(ExpressionStatement<'src>),
    VariableDeclaration(VariableDeclaration<'src>),
    TSTypeAliasDeclaration(TSTypeAliasDeclaration<'src>),
    TSInterfaceDeclaration(TSInterfaceDeclaration<'src>),
    TSDeclareFunction(TSDeclareFunction<'src>),
    TSEnumDeclaration(TSEnumDeclaration<'src>),
    TSModuleDeclaration(TSModuleDeclaration<'src>),
    ReturnStatement(ReturnStatement<'src>),
    BlockStatement(BlockStatement<'src>),
    FunctionDeclaration(FunctionDeclaration<'src>),
    ClassDeclaration(ClassDeclaration<'src>),
    ExportNamedDeclaration(ExportNamedDeclaration<'src>),
    ExportDefaultDeclaration(ExportDefaultDeclaration<'src>),
    ExportAllDeclaration(ExportAllDeclaration<'src>),
    TSExportAssignment(TSExportAssignment<'src>),
    ImportDeclaration(ImportDeclaration<'src>),
    TSImportEqualsDeclaration(TSImportEqualsDeclaration<'src>),
    // Control flow statements
    IfStatement(IfStatement<'src>),
    ForStatement(ForStatement<'src>),
    ForInStatement(ForInStatement<'src>),
    ForOfStatement(ForOfStatement<'src>),
    WhileStatement(WhileStatement<'src>),
    DoWhileStatement(DoWhileStatement<'src>),
    SwitchStatement(SwitchStatement<'src>),
    TryStatement(TryStatement<'src>),
    ThrowStatement(ThrowStatement<'src>),
    BreakStatement(BreakStatement<'src>),
    ContinueStatement(ContinueStatement<'src>),
    LabeledStatement(LabeledStatement<'src>),
    EmptyStatement(EmptyStatement),
    DebuggerStatement(DebuggerStatement),
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpressionStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: Expression<'src>,
    /// Present only for directive prologue entries (acorn `directive`): the
    /// raw string contents without surrounding quotes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directive: Option<Cow<'src, str>>,
}

/// Block statement (function body with braces)
#[derive(Debug, Clone, Serialize)]
pub struct BlockStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub body: Vec<Statement<'src>>,
}

/// Function declaration: `function foo(x) { return x + 1; }`
/// For `export default function() {}`, id is null.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Function name (None for anonymous export default functions)
    pub id: Option<Identifier<'src>>,
    pub expression: bool,
    pub generator: bool,
    #[serde(rename = "async")]
    pub is_async: bool,
    /// Type parameters (TypeScript generics): `function fn<T>() {}`
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration<'src>>,
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Vec<Expression<'src>>,
    /// Return type annotation (e.g., `: number`)
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation<'src>>,
    pub body: BlockStatement<'src>,
}

/// Return statement: `return expr;` or `return;`
#[derive(Debug, Clone, Serialize)]
pub struct ReturnStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub argument: Option<Box<Expression<'src>>>,
}

//
// Control Flow Statements
//

/// If statement: `if (test) consequent` or `if (test) consequent else alternate`
#[derive(Debug, Clone, Serialize)]
pub struct IfStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub test: Box<Expression<'src>>,
    pub consequent: Box<Statement<'src>>,
    pub alternate: Option<Box<Statement<'src>>>,
}

/// For statement: `for (init; test; update) body`
#[derive(Debug, Clone, Serialize)]
pub struct ForStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub init: Option<ForInit<'src>>,
    pub test: Option<Box<Expression<'src>>>,
    pub update: Option<Box<Expression<'src>>>,
    pub body: Box<Statement<'src>>,
}

/// For statement initialization
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ForInit<'src> {
    VariableDeclaration(VariableDeclaration<'src>),
    Expression(Box<Expression<'src>>),
}

/// For-in statement: `for (left in right) body`
#[derive(Debug, Clone, Serialize)]
pub struct ForInStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub left: ForInOfLeft<'src>,
    pub right: Box<Expression<'src>>,
    pub body: Box<Statement<'src>>,
}

/// For-of statement: `for (left of right) body`
#[derive(Debug, Clone, Serialize)]
pub struct ForOfStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "await")]
    pub r#await: bool,
    pub left: ForInOfLeft<'src>,
    pub right: Box<Expression<'src>>,
    pub body: Box<Statement<'src>>,
}

/// Left side of for-in/for-of
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ForInOfLeft<'src> {
    VariableDeclaration(VariableDeclaration<'src>),
    Pattern(Box<Expression<'src>>),
}

/// While statement: `while (test) body`
#[derive(Debug, Clone, Serialize)]
pub struct WhileStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub test: Box<Expression<'src>>,
    pub body: Box<Statement<'src>>,
}

/// Do-while statement: `do body while (test)`
#[derive(Debug, Clone, Serialize)]
pub struct DoWhileStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub body: Box<Statement<'src>>,
    pub test: Box<Expression<'src>>,
}

/// Switch statement: `switch (discriminant) { cases }`
#[derive(Debug, Clone, Serialize)]
pub struct SwitchStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub discriminant: Box<Expression<'src>>,
    pub cases: Vec<SwitchCase<'src>>,
}

/// Switch case: `case test: consequent` or `default: consequent`
#[derive(Debug, Clone, Serialize)]
pub struct SwitchCase<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub test: Option<Box<Expression<'src>>>,
    pub consequent: Vec<Statement<'src>>,
}

/// Try statement: `try { block } catch { handler } finally { finalizer }`
#[derive(Debug, Clone, Serialize)]
pub struct TryStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub block: BlockStatement<'src>,
    pub handler: Option<CatchClause<'src>>,
    pub finalizer: Option<BlockStatement<'src>>,
}

/// Catch clause: `catch (param) { body }`
#[derive(Debug, Clone, Serialize)]
pub struct CatchClause<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub param: Option<Box<Expression<'src>>>,
    pub body: BlockStatement<'src>,
}

/// Throw statement: `throw argument`
#[derive(Debug, Clone, Serialize)]
pub struct ThrowStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub argument: Box<Expression<'src>>,
}

/// Break statement: `break` or `break label`
#[derive(Debug, Clone, Serialize)]
pub struct BreakStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub label: Option<Identifier<'src>>,
}

/// Continue statement: `continue` or `continue label`
#[derive(Debug, Clone, Serialize)]
pub struct ContinueStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub label: Option<Identifier<'src>>,
}

/// Labeled statement: `label: statement`
#[derive(Debug, Clone, Serialize)]
pub struct LabeledStatement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub label: Identifier<'src>,
    pub body: Box<Statement<'src>>,
}

/// Empty statement: `;`
#[derive(Debug, Clone, Serialize)]
pub struct EmptyStatement {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// Debugger statement: `debugger;`
#[derive(Debug, Clone, Serialize)]
pub struct DebuggerStatement {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone, Serialize)]
pub struct VariableDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub declarations: Vec<VariableDeclarator<'src>>,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "is_false")]
    pub declare: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct VariableDeclarator<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The binding pattern (Identifier, ArrayPattern, or ObjectPattern)
    pub id: Expression<'src>,
    /// Definite assignment assertion (`!` after identifier, e.g., `let x!: string;`)
    #[serde(skip_serializing_if = "is_false")]
    pub definite: bool,
    pub init: Option<Expression<'src>>,
}
