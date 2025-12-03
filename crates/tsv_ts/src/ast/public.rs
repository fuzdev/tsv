// Public AST types - with serde, matches Svelte's JSON structure exactly
// Uses u32 for positions (max 4GB file size) for memory efficiency

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub body: Vec<Statement>,
    #[serde(rename = "sourceType")]
    pub source_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Statement {
    ExpressionStatement(ExpressionStatement),
    VariableDeclaration(VariableDeclaration),
    TSTypeAliasDeclaration(TSTypeAliasDeclaration),
    ReturnStatement(ReturnStatement),
    BlockStatement(BlockStatement),
    FunctionDeclaration(FunctionDeclaration),
    ClassDeclaration(ClassDeclaration),
    ExportNamedDeclaration(ExportNamedDeclaration),
    ExportDefaultDeclaration(ExportDefaultDeclaration),
    ExportAllDeclaration(ExportAllDeclaration),
    ImportDeclaration(ImportDeclaration),
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
}

/// Export named declaration: `export const x = 1;`, `export { x }`, `export { x } from "y"`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportNamedDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "exportKind")]
    pub export_kind: String,
    /// Declaration being exported (for `export const x = 1`), or null for specifiers
    pub declaration: Option<Box<Statement>>,
    /// Export specifiers: `export { a, b as c }`
    pub specifiers: Vec<ExportSpecifier>,
    /// Re-export source: `export { x } from "y"` or null for local exports
    pub source: Option<Literal>,
}

/// Export default declaration: `export default x`, `export default function() {}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportDefaultDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "exportKind")]
    pub export_kind: String,
    /// The expression or declaration being exported as default
    pub declaration: ExportDefaultValue,
}

/// Value of export default - can be expression or declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExportDefaultValue {
    Expression(Expression),
    FunctionDeclaration(FunctionDeclaration),
    ClassDeclaration(ClassDeclaration),
}

/// Export all declaration: `export * from "y"` or `export * as ns from "y"`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportAllDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "exportKind")]
    pub export_kind: String,
    /// For `export * as ns from "y"`, the namespace binding name, or null
    pub exported: Option<Identifier>,
    /// Module source
    pub source: Literal,
}

/// Export specifier: `export { x }` or `export { x as y }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSpecifier {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Local name (what's exported from this module)
    pub local: Identifier,
    /// Exported name (what it's called externally)
    pub exported: Identifier,
    #[serde(rename = "exportKind")]
    pub export_kind: String,
}

/// Import declaration: `import x from "y"`, `import { a, b } from "y"`, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "importKind")]
    pub import_kind: String,
    pub specifiers: Vec<ImportSpecifier>,
    pub source: Literal,
    pub attributes: Vec<ImportAttribute>,
}

/// Import specifier: default, named, or namespace
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ImportSpecifier {
    Default(ImportDefaultSpecifier),
    Named(ImportNamedSpecifier),
    Namespace(ImportNamespaceSpecifier),
}

/// Default import: `import x from "y"`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportDefaultSpecifier {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub local: Identifier,
}

/// Named import: `import { a } from "y"` or `import { a as b } from "y"`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportNamedSpecifier {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub imported: Identifier,
    pub local: Identifier,
    #[serde(rename = "importKind")]
    pub import_kind: String,
}

/// Namespace import: `import * as ns from "y"`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportNamespaceSpecifier {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub local: Identifier,
}

/// Import attribute: `{ type: "json" }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportAttribute {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub key: Identifier,
    pub value: Literal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionStatement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: Expression,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Expression {
    Literal(Literal),
    Identifier(Identifier),
    ObjectExpression(ObjectExpression),
    ArrayExpression(ArrayExpression),
    UnaryExpression(UnaryExpression),
    UpdateExpression(UpdateExpression),
    BinaryExpression(BinaryExpression),
    CallExpression(CallExpression),
    NewExpression(NewExpression),
    MemberExpression(MemberExpression),
    ConditionalExpression(ConditionalExpression),
    ArrowFunctionExpression(ArrowFunctionExpression),
    FunctionExpression(FunctionExpression),
    SpreadElement(SpreadElement),
    TemplateLiteral(TemplateLiteral),
    TaggedTemplateExpression(TaggedTemplateExpression),
    AwaitExpression(AwaitExpression),
    SequenceExpression(SequenceExpression),
    RegexLiteral(RegexLiteral),
    Super(Super),
    // Assignment and patterns
    AssignmentExpression(AssignmentExpression),
    ObjectPattern(ObjectPattern),
    ArrayPattern(ArrayPattern),
    AssignmentPattern(AssignmentPattern),
    RestElement(RestElement),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Literal {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(serialize_with = "serialize_literal_value")]
    pub value: serde_json::Value,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identifier {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub name: String,
    /// Whether this is an optional parameter
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub properties: Vec<ObjectProperty>,
}

/// Object property - either a regular property or a spread element
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObjectProperty {
    Property(Property),
    SpreadElement(SpreadElement),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrayExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub elements: Vec<Option<Expression>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnaryExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub operator: String,
    pub prefix: bool,
    pub argument: Box<Expression>,
}

/// Update expression: `++x`, `x++`, `--x`, `x--`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub operator: String,
    pub prefix: bool,
    pub argument: Box<Expression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub left: Box<Expression>,
    pub operator: String,
    pub right: Box<Expression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub callee: Box<Expression>,
    pub arguments: Vec<Expression>,
    pub optional: bool,
}

/// New expression: `new Date()`, `new Map()`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub callee: Box<Expression>,
    pub arguments: Vec<Expression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub object: Box<Expression>,
    pub property: Box<Expression>,
    pub computed: bool,
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub test: Box<Expression>,
    pub consequent: Box<Expression>,
    pub alternate: Box<Expression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrowFunctionExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Option<()>, // always null for arrow functions
    pub expression: bool,
    pub generator: bool,
    #[serde(rename = "async")]
    pub is_async: bool,
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Vec<Expression>,
    pub body: ArrowFunctionBody,
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation>,
}

/// Arrow function body - either expression or block statement
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArrowFunctionBody {
    Expression(Box<Expression>),
    BlockStatement(BlockStatement),
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
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Vec<Expression>,
    /// Return type annotation (e.g., `: number`)
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation>,
    pub body: BlockStatement,
}

/// Class declaration: `class Foo { ... }`
/// For `export default class {}`, id is null.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Class name (None for anonymous export default classes)
    pub id: Option<Identifier>,
    #[serde(rename = "superClass")]
    pub super_class: Option<Box<Expression>>,
    pub body: ClassBody,
}

/// Class body: `{ constructor() {} method() {} prop = value; }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassBody {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub body: Vec<ClassMember>,
}

/// Class member - either method definition or property definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClassMember {
    MethodDefinition(MethodDefinition),
    PropertyDefinition(PropertyDefinition),
}

/// Method definition in a class body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodDefinition {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "static")]
    pub is_static: bool,
    pub computed: bool,
    pub key: Box<Expression>,
    pub kind: String,
    pub value: FunctionExpression,
}

/// Property definition in a class body: `name = value;`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyDefinition {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "static")]
    pub is_static: bool,
    pub computed: bool,
    pub key: Box<Expression>,
    /// Type annotation (e.g., `: number`)
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
    pub value: Option<Box<Expression>>,
}

/// Function expression: `function() {}` or method shorthand `{ foo() {} }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Option<Identifier>,
    pub expression: bool,
    pub generator: bool,
    #[serde(rename = "async")]
    pub is_async: bool,
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

// ============================================================================
// Control Flow Statements
// ============================================================================

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
    #[serde(skip_serializing_if = "Option::is_none")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init: Option<ForInit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<Box<Expression>>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    pub left: ForInOfLeft,
    pub right: Box<Expression>,
    #[serde(rename = "await")]
    pub r#await: bool,
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
    #[serde(skip_serializing_if = "Option::is_none")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler: Option<CatchClause>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
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

// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadElement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub argument: Box<Expression>,
}

/// Template literal expression: `hello ${name}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateLiteral {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub quasis: Vec<TemplateElement>,
    pub expressions: Vec<Expression>,
}

/// Template element - a static string part of a template literal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateElement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub value: TemplateElementValue,
    pub tail: bool,
}

/// Value field of a template element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateElementValue {
    pub raw: String,
    /// Cooked value is null for invalid escape sequences in tagged templates
    pub cooked: Option<String>,
}

/// Tagged template expression: tag`content ${expr}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaggedTemplateExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub tag: Box<Expression>,
    pub quasi: TemplateLiteral,
}

/// Await expression: `await promise`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwaitExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub argument: Box<Expression>,
}

/// Sequence expression: `a, b, c`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expressions: Vec<Expression>,
}

/// Regular expression literal.
/// Serializes with type "Literal" to match acorn/Svelte AST.
/// Example: `/hello/gi` becomes `{type: "Literal", value: {}, raw: "/hello/gi", regex: {pattern: "hello", flags: "gi"}}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexLiteral {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Always serializes as empty object {} since regex can't be represented in JSON
    pub value: serde_json::Value,
    /// The full raw source text including slashes: /pattern/flags
    pub raw: String,
    /// Pattern and flags extracted for convenience
    pub regex: RegexValue,
}

/// Regex pattern and flags for the AST.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexValue {
    pub pattern: String,
    pub flags: String,
}

/// Super expression: `super`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Super {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// Assignment expression: `x = value`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub operator: String,
    pub left: Box<Expression>,
    pub right: Box<Expression>,
}

/// Object pattern for destructuring: `{a, b}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectPattern {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub properties: Vec<ObjectPatternProperty>,
}

/// Object pattern property - either a regular property or a rest element
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObjectPatternProperty {
    Property(Property),
    RestElement(RestElement),
}

/// Array pattern for destructuring: `[a, b]`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrayPattern {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub elements: Vec<Option<Expression>>,
}

/// Assignment pattern for default values: `a = 1`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentPattern {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub left: Box<Expression>,
    pub right: Box<Expression>,
}

/// Rest element in destructuring: `...rest`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestElement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub argument: Box<Expression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub method: bool,
    pub shorthand: bool,
    pub computed: bool,
    pub key: Box<Expression>,
    pub value: Box<Expression>,
    pub kind: String,
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
    pub init: Option<Expression>,
}

// TypeScript type annotation nodes

/// Public AST representation of TypeScript type annotation
///
/// Serializes to JSON matching Svelte's/acorn-typescript's format:
/// ```json
/// {
///   "type": "TSTypeAnnotation",
///   "start": 7,
///   "end": 15,
///   "loc": { "start": { "line": 1, "column": 7 }, ... },
///   "typeAnnotation": { "type": "TSNumberKeyword", ... }
/// }
/// ```
///
/// Note the nested `typeAnnotation` field uses camelCase for JSON compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSTypeAnnotation {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
}

/// TypeScript type expression
///
/// Uses serde's untagged enum to serialize each variant based on its structure.
/// Each variant serializes to a flat object with its own `type` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TSType {
    TSNumberKeyword(TSNumberKeyword),
    TSStringKeyword(TSStringKeyword),
    TSBooleanKeyword(TSBooleanKeyword),
    TSAnyKeyword(TSAnyKeyword),
    TSVoidKeyword(TSVoidKeyword),
    TSUndefinedKeyword(TSUndefinedKeyword),
    TSNullKeyword(TSNullKeyword),
    TSNeverKeyword(TSNeverKeyword),
    TSUnknownKeyword(TSUnknownKeyword),
    TSObjectKeyword(TSObjectKeyword),
    TSSymbolKeyword(TSSymbolKeyword),
    TSBigIntKeyword(TSBigIntKeyword),
    TSLiteralType(TSLiteralType),
    TSArrayType(TSArrayType),
}

/// TypeScript array type: `number[]`, `string[]`, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSArrayType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "elementType")]
    pub element_type: Box<TSType>,
}

/// TypeScript `number` type keyword
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSNumberKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `string` type keyword
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSStringKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `boolean` type keyword
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSBooleanKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `any` type keyword
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSAnyKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `void` type keyword
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSVoidKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `undefined` type keyword
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSUndefinedKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `null` type keyword
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSNullKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `never` type keyword
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSNeverKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `unknown` type keyword
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSUnknownKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `object` type keyword
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSObjectKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `symbol` type keyword
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSSymbolKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript `bigint` type keyword
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSBigIntKeyword {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
}

/// TypeScript type alias declaration: `type X = T`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSTypeAliasDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Identifier,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: TSType,
}

/// TypeScript literal type: `type X = 'hello'` or `type X = \`template\``
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSLiteralType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub literal: TSLiteralTypeLiteral,
}

/// The literal value inside a TSLiteralType
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TSLiteralTypeLiteral {
    TemplateLiteral(TemplateLiteralType),
    // TODO: Add String, Number, Boolean literal variants
}

/// Template literal used as a type (same structure as TemplateLiteral but expressions are TSType)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateLiteralType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub quasis: Vec<TemplateElement>,
    pub expressions: Vec<TSType>,
}

// Serialize numbers as integers if they have no fractional part
fn serialize_literal_value<S>(value: &serde_json::Value, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    // For numbers, ensure integers are serialized without decimals
    if let Some(f) = value.as_f64()
        && f.fract() == 0.0
        && f.is_finite()
    {
        return serializer.serialize_i64(f as i64);
    }
    value.serialize(serializer)
}
