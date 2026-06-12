//! Statement types for public AST

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: Expression,
    /// Present only for directive prologue entries (acorn `directive`): the
    /// raw string contents without surrounding quotes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directive: Option<String>,
}

/// Block statement (function body with braces)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub body: Vec<Statement>,
}

/// Function declaration: `function foo(x) { return x + 1; }`
/// For `export default function() {}`, id is null.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Function name (None for anonymous export default functions)
    pub id: Option<Identifier>,
    pub expression: bool,
    pub generator: bool,
    #[serde(rename = "async")]
    pub is_async: bool,
    /// Type parameters (TypeScript generics): `function fn<T>() {}`
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Vec<Expression>,
    /// Return type annotation (e.g., `: number`)
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation>,
    pub body: BlockStatement,
}

/// Return statement: `return expr;` or `return;`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub argument: Option<Box<Expression>>,
}

//
// Control Flow Statements
//

/// If statement: `if (test) consequent` or `if (test) consequent else alternate`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub test: Box<Expression>,
    pub consequent: Box<Statement>,
    pub alternate: Option<Box<Statement>>,
}

/// For statement: `for (init; test; update) body`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub init: Option<ForInit>,
    pub test: Option<Box<Expression>>,
    pub update: Option<Box<Expression>>,
    pub body: Box<Statement>,
}

/// For statement initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ForInit {
    VariableDeclaration(VariableDeclaration),
    Expression(Box<Expression>),
}

/// For-in statement: `for (left in right) body`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForInStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub left: ForInOfLeft,
    pub right: Box<Expression>,
    pub body: Box<Statement>,
}

/// For-of statement: `for (left of right) body`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForOfStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "await")]
    pub r#await: bool,
    pub left: ForInOfLeft,
    pub right: Box<Expression>,
    pub body: Box<Statement>,
}

/// Left side of for-in/for-of
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ForInOfLeft {
    VariableDeclaration(VariableDeclaration),
    Pattern(Box<Expression>),
}

/// While statement: `while (test) body`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhileStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub test: Box<Expression>,
    pub body: Box<Statement>,
}

/// Do-while statement: `do body while (test)`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoWhileStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub body: Box<Statement>,
    pub test: Box<Expression>,
}

/// Switch statement: `switch (discriminant) { cases }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub discriminant: Box<Expression>,
    pub cases: Vec<SwitchCase>,
}

/// Switch case: `case test: consequent` or `default: consequent`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchCase {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub test: Option<Box<Expression>>,
    pub consequent: Vec<Statement>,
}

/// Try statement: `try { block } catch { handler } finally { finalizer }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TryStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub block: BlockStatement,
    pub handler: Option<CatchClause>,
    pub finalizer: Option<BlockStatement>,
}

/// Catch clause: `catch (param) { body }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatchClause {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub param: Option<Box<Expression>>,
    pub body: BlockStatement,
}

/// Throw statement: `throw argument`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrowStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub argument: Box<Expression>,
}

/// Break statement: `break` or `break label`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub label: Option<Identifier>,
}

/// Continue statement: `continue` or `continue label`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinueStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub label: Option<Identifier>,
}

/// Labeled statement: `label: statement`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabeledStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub label: Identifier,
    pub body: Box<Statement>,
}

/// Empty statement: `;`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// Debugger statement: `debugger;`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebuggerStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub declarations: Vec<VariableDeclarator>,
    pub kind: String,
    #[serde(skip_serializing_if = "is_false")]
    pub declare: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDeclarator {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The binding pattern (Identifier, ArrayPattern, or ObjectPattern)
    pub id: Expression,
    /// Definite assignment assertion (`!` after identifier, e.g., `let x!: string;`)
    #[serde(skip_serializing_if = "is_false")]
    pub definite: bool,
    pub init: Option<Expression>,
}
