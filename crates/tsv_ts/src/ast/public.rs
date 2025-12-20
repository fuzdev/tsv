// Public AST types - with serde, matches Svelte's JSON structure exactly
// Uses u32 for positions (max 4GB file size) for memory efficiency

use serde::{Deserialize, Serialize};

/// Helper for skip_serializing_if to skip false bools
#[allow(clippy::trivially_copy_pass_by_ref)] // serde requires &T signature
fn is_false(b: &bool) -> bool {
    !*b
}

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

/// Decorator: `@expression` applied to classes and class members
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decorator {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The decorator expression
    pub expression: Expression,
}

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
    /// For ambient function declarations (no body)
    TSDeclareFunction(TSDeclareFunction),
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

/// TypeScript export assignment: `export = value;`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSExportAssignment {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: Expression,
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

/// TypeScript import equals declaration: `import x = require("y")` or `import x = A.B`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSImportEqualsDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "importKind")]
    pub import_kind: String,
    #[serde(rename = "isExport")]
    pub is_export: bool,
    pub id: Identifier,
    #[serde(rename = "moduleReference")]
    pub module_reference: TSModuleReference,
}

/// Module reference: either external module reference or entity name
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TSModuleReference {
    ExternalModuleReference(TSExternalModuleReference),
    EntityName(TSEntityName),
}

/// External module reference: `require("module")`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSExternalModuleReference {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: Literal,
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
    PrivateIdentifier(PrivateIdentifier),
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
    ClassExpression(ClassExpression),
    SpreadElement(SpreadElement),
    TemplateLiteral(TemplateLiteral),
    TaggedTemplateExpression(TaggedTemplateExpression),
    AwaitExpression(AwaitExpression),
    YieldExpression(YieldExpression),
    SequenceExpression(SequenceExpression),
    RegexLiteral(RegexLiteral),
    Super(Super),
    // Assignment and patterns
    AssignmentExpression(AssignmentExpression),
    ObjectPattern(ObjectPattern),
    ArrayPattern(ArrayPattern),
    AssignmentPattern(AssignmentPattern),
    RestElement(RestElement),
    // TypeScript type assertions
    TSTypeAssertion(TSTypeAssertion),
    TSAsExpression(TSAsExpression),
    TSSatisfiesExpression(TSSatisfiesExpression),
    // TypeScript instantiation expression: f<T>
    TSInstantiationExpression(TSInstantiationExpression),
    // TypeScript non-null assertion: expr!
    TSNonNullExpression(TSNonNullExpression),
    // Dynamic import: import('...')
    ImportExpression(ImportExpression),
    // Meta property: import.meta, new.target
    MetaProperty(MetaProperty),
    // TypeScript parameter property: constructor(public x)
    TSParameterProperty(TSParameterProperty),
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
    /// BigInt string value (only for BigInt literals like `1n`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bigint: Option<String>,
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
    /// Decorators applied to this parameter (TypeScript parameter decorators)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub decorators: Vec<Decorator>,
}

/// Private identifier: `#foo` in class fields and methods
///
/// Used for truly private class members (ES2022 private class fields).
/// The name does NOT include the `#` prefix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateIdentifier {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The name without the `#` prefix (e.g., "foo" for `#foo`)
    pub name: String,
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
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation>,
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
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub arguments: Vec<Expression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub source: Box<Expression>,
    /// Optional second argument for import attributes: `{with: {type: 'json'}}`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Box<Expression>>,
}

/// Meta property: `import.meta`, `new.target`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaProperty {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The keyword: Identifier("import") or Identifier("new")
    pub meta: Identifier,
    /// The property: Identifier("meta") or Identifier("target")
    pub property: Identifier,
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
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
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

/// Class declaration: `class Foo { ... }`
/// For `export default class {}`, id is null.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Decorators applied to this class
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorators: Option<Vec<Decorator>>,
    /// Whether this is a declare class (ambient declaration)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declare: Option<bool>,
    /// Class name (None for anonymous export default classes)
    pub id: Option<Identifier>,
    /// Type parameters (e.g., `<T>` in `class Foo<T>`)
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    #[serde(rename = "superClass")]
    pub super_class: Option<Box<Expression>>,
    /// Type arguments for superclass (e.g., `<T>` in `extends Base<T>`)
    #[serde(
        rename = "superTypeParameters",
        skip_serializing_if = "Option::is_none"
    )]
    pub super_type_parameters: Option<TSTypeParameterInstantiation>,
    /// Implements clause: `implements Foo, Bar`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implements: Option<Vec<TSExpressionWithTypeArguments>>,
    pub body: ClassBody,
}

/// Class expression: `class { }` or `class Foo<T> extends Bar { }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Decorators applied to this class
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorators: Option<Vec<Decorator>>,
    /// Class name (always optional for expressions)
    pub id: Option<Identifier>,
    /// Type parameters (e.g., `<T>` in `class Foo<T>`)
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    #[serde(rename = "superClass")]
    pub super_class: Option<Box<Expression>>,
    /// Type arguments for superclass (e.g., `<T>` in `extends Base<T>`)
    #[serde(
        rename = "superTypeParameters",
        skip_serializing_if = "Option::is_none"
    )]
    pub super_type_parameters: Option<TSTypeParameterInstantiation>,
    /// Implements clause: `implements Foo, Bar`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implements: Option<Vec<TSExpressionWithTypeArguments>>,
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

/// Class member - method definition, property definition, or static block
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClassMember {
    MethodDefinition(MethodDefinition),
    PropertyDefinition(PropertyDefinition),
    StaticBlock(StaticBlock),
    TSIndexSignature(TSIndexSignature),
}

/// Static initialization block in a class: `static { ... }` (ES2022)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticBlock {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub body: Vec<Statement>,
}

/// Method definition in a class body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodDefinition {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Decorators applied to this method
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorators: Option<Vec<Decorator>>,
    /// Accessibility modifier (public, private, protected)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<String>,
    #[serde(rename = "static")]
    pub is_static: bool,
    /// Whether this method overrides a base class method
    #[serde(rename = "override")]
    pub is_override: bool,
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
    /// Decorators applied to this property
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorators: Option<Vec<Decorator>>,
    /// Whether this property uses the accessor keyword (ES decorator proposal)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessor: Option<bool>,
    /// Accessibility modifier (public, private, protected)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<String>,
    /// Whether this is a readonly property
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readonly: Option<bool>,
    #[serde(rename = "static")]
    pub is_static: bool,
    pub computed: bool,
    pub key: Box<Expression>,
    /// Whether this is an optional property (`a?: string`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    /// Whether this has definite assignment assertion (`a!: string`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definite: Option<bool>,
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
    /// Type parameters (TypeScript generics): `function<T>() {}`
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

/// Yield expression: `yield value` or `yield* iterable`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YieldExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The value to yield (None for `yield` with no argument)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argument: Option<Box<Expression>>,
    /// Whether this is a delegating yield: `yield*`
    pub delegate: bool,
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
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
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
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
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
    /// Definite assignment assertion (`!` after identifier, e.g., `let x!: string;`)
    #[serde(skip_serializing_if = "is_false")]
    pub definite: bool,
    pub init: Option<Expression>,
}

// TypeScript expression nodes

/// TypeScript angle-bracket type assertion: `<Type>expr`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSTypeAssertion {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The target type
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
    /// The expression being type-asserted
    pub expression: Box<Expression>,
}

/// TypeScript `as` type assertion: `expr as Type` or `expr as const`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSAsExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression being type-asserted
    pub expression: Box<Expression>,
    /// The target type
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
}

/// TypeScript `satisfies` expression: `expr satisfies Type`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSSatisfiesExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression being checked
    pub expression: Box<Expression>,
    /// The type to satisfy
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
}

/// TypeScript instantiation expression: `f<T>`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSInstantiationExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression being instantiated
    pub expression: Box<Expression>,
    /// The type arguments
    #[serde(rename = "typeArguments")]
    pub type_arguments: TSTypeParameterInstantiation,
}

/// TypeScript non-null assertion expression: `expr!`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSNonNullExpression {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression being asserted non-null
    pub expression: Box<Expression>,
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
    TSUnionType(TSUnionType),
    TSIntersectionType(TSIntersectionType),
    TSTypeReference(TSTypeReference),
    TSTypeLiteral(TSTypeLiteral),
    TSFunctionType(TSFunctionType),
    TSConstructorType(TSConstructorType),
    TSTupleType(TSTupleType),
    TSParenthesizedType(TSParenthesizedType),
    TSTypePredicate(TSTypePredicate),
    TSConditionalType(TSConditionalType),
    TSMappedType(TSMappedType),
    TSTypeOperator(TSTypeOperator),
    TSImportType(TSImportType),
    TSTypeQuery(TSTypeQuery),
    TSIndexedAccessType(TSIndexedAccessType),
    TSRestType(TSRestType),
    TSOptionalType(TSOptionalType),
    TSNamedTupleMember(TSNamedTupleMember),
    TSInferType(TSInferType),
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

/// TypeScript indexed access type: `T[K]`, `Obj["key"]`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSIndexedAccessType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "objectType")]
    pub object_type: Box<TSType>,
    #[serde(rename = "indexType")]
    pub index_type: Box<TSType>,
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
    /// Unary expression for negative numbers: `-1`, `-42n`
    UnaryExpression(UnaryExpression),
    /// Literal value (string, number, bigint)
    Literal(Literal),
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

/// TypeScript interface declaration: `interface Foo { ... }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSInterfaceDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Identifier,
    #[serde(rename = "extends", skip_serializing_if = "Vec::is_empty")]
    pub extends: Vec<TSInterfaceHeritage>,
    pub body: TSInterfaceBody,
}

/// Interface heritage: `extends Foo, Bar`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSInterfaceHeritage {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: TSEntityName,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterInstantiation>,
}

/// Interface body: `{ members }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSInterfaceBody {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub body: Vec<TSTypeElement>,
}

/// Entity name: `Foo` or `Foo.Bar.Baz`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TSEntityName {
    Identifier(Identifier),
    QualifiedName(TSQualifiedName),
}

/// Qualified name: `Foo.Bar`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSQualifiedName {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub left: Box<TSEntityName>,
    pub right: Identifier,
}

/// Type parameter instantiation: `<T, U>`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSTypeParameterInstantiation {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub params: Vec<TSType>,
}

/// Type parameter declaration: `<T extends U = V>`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSTypeParameterDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub params: Vec<TSTypeParameter>,
}

/// Single type parameter: `T extends U = V`
/// With optional modifiers: `const T`, `in T`, `out T`, `in out T`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSTypeParameter {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// `const` modifier (TS 5.0): `<const T>`
    #[serde(rename = "const", skip_serializing_if = "is_false")]
    pub is_const: bool,
    /// `in` variance modifier (TS 4.7): `<in T>`
    #[serde(rename = "in", skip_serializing_if = "is_false")]
    pub is_in: bool,
    /// `out` variance modifier (TS 4.7): `<out T>`
    #[serde(rename = "out", skip_serializing_if = "is_false")]
    pub is_out: bool,
    pub name: Identifier,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<Box<TSType>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Box<TSType>>,
}

/// Expression with type arguments for implements clause: `implements Foo<T>`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSExpressionWithTypeArguments {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: Expression,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterInstantiation>,
}

/// Type element - member of a type literal or interface
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TSTypeElement {
    PropertySignature(TSPropertySignature),
    MethodSignature(TSMethodSignature),
    CallSignature(TSCallSignatureDeclaration),
    ConstructSignature(TSConstructSignatureDeclaration),
    IndexSignature(TSIndexSignature),
}

/// Property signature: `prop: T`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSPropertySignature {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub key: Expression,
    pub computed: bool,
    pub optional: bool,
    pub readonly: bool,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
}

/// Method signature: `method(): T` or `method<T>(x: T): T` or `get x(): T` or `set x(v: T)`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSMethodSignature {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub computed: bool,
    pub key: Expression,
    /// Method kind: "get" or "set" for accessor signatures (omitted for regular methods)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub parameters: Vec<Expression>,
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation>,
}

/// Call signature: `(): T` or `<T>(): T`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSCallSignatureDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation>,
}

/// Construct signature: `new (): T` or `new <T>(): T`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSConstructSignatureDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation>,
}

/// Index signature: `[key: string]: T`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSIndexSignature {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub parameters: Vec<Identifier>,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: TSTypeAnnotation,
    pub readonly: bool,
}

/// Declare function: `declare function foo(): void`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSDeclareFunction {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Identifier,
    pub params: Vec<Expression>,
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation>,
}

/// Enum declaration: `enum Foo { A, B }`, `const enum Foo { A = 1 }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSEnumDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Enum name
    pub id: Identifier,
    /// Enum members
    pub members: Vec<TSEnumMember>,
    /// Whether this is a const enum (only serialized when true)
    #[serde(rename = "const", skip_serializing_if = "std::ops::Not::not")]
    pub is_const: bool,
    /// Whether this is a declare enum (ambient declaration, only serialized when true)
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub declare: bool,
}

/// Enum member: `A`, `A = 1`, `A = "value"`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSEnumMember {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Member name (identifier or string literal)
    pub id: TSEnumMemberId,
    /// Optional initializer expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initializer: Option<Expression>,
}

/// Enum member id - can be identifier or string literal
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TSEnumMemberId {
    Identifier(Identifier),
    Literal(Literal),
}

/// TypeScript module/namespace declaration: `namespace Utils { ... }` or `module Utils { ... }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSModuleDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Module/namespace name - identifier for regular namespaces, string literal for ambient modules
    pub id: TSModuleName,
    /// Module body - either a block or nested module declaration (for `A.B.C`)
    /// `None` for shorthand ambient modules: `declare module 'name';`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<TSModuleDeclarationBody>,
    /// Whether this is an ambient declaration (`declare namespace/module`)
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub declare: bool,
    /// For `declare global {}` - uses module kind but has special semantics
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub global: bool,
}

/// Module/namespace name - can be an identifier or a string literal
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TSModuleName {
    /// Regular identifier: `namespace Foo { }`
    Identifier(Identifier),
    /// String literal for ambient modules: `declare module 'name' { }`
    Literal(Literal),
}

/// Body of a TypeScript module declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TSModuleDeclarationBody {
    /// Block body with statements: `namespace A { ... }`
    TSModuleBlock(TSModuleBlock),
    /// Nested module declaration: `namespace A.B { ... }` - the B part
    TSModuleDeclaration(Box<TSModuleDeclaration>),
}

/// TypeScript module block: the `{ ... }` part of a namespace/module declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSModuleBlock {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Statements inside the module block
    pub body: Vec<Statement>,
}

/// Union type: `A | B | C`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSUnionType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub types: Vec<TSType>,
}

/// Intersection type: `A & B & C`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSIntersectionType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub types: Vec<TSType>,
}

/// Type reference: `SomeType` or `Array<T>`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSTypeReference {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeName")]
    pub type_name: TSEntityName,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterInstantiation>,
}

/// Type literal (object type): `{ prop: T }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSTypeLiteral {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub members: Vec<TSTypeElement>,
}

/// Function type: `(x: T) => U` or `<T>(x: T) => U`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSFunctionType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    #[serde(rename = "returnType")]
    pub return_type: Box<TSTypeAnnotation>,
}

/// Constructor type: `new () => T` or `abstract new <T>() => T`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSConstructorType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub abstract_: bool,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    #[serde(rename = "typeAnnotation")]
    pub return_type: Box<TSTypeAnnotation>,
}

/// Tuple type: `[T, U, V]`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSTupleType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "elementTypes")]
    pub element_types: Vec<TSType>,
}

/// Rest type in tuples: `...T`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSRestType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
}

/// Optional type in tuples: `T?`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSOptionalType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
}

/// Named tuple member: `label: T` or `label?: T`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSNamedTupleMember {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub label: Identifier,
    #[serde(rename = "elementType")]
    pub element_type: Box<TSType>,
    pub optional: bool,
}

/// Infer type: `infer U` (in conditional types)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSInferType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeParameter")]
    pub type_parameter: TSTypeParameter,
}

/// Parenthesized type: `(T)`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSParenthesizedType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
}

/// TypeScript type predicate: `x is T` or `asserts x is T`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSTypePredicate {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "parameterName")]
    pub parameter_name: Identifier,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<Box<TSTypeAnnotation>>,
    pub asserts: bool,
}

/// TypeScript conditional type: `T extends U ? V : W`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSConditionalType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "checkType")]
    pub check_type: Box<TSType>,
    #[serde(rename = "extendsType")]
    pub extends_type: Box<TSType>,
    #[serde(rename = "trueType")]
    pub true_type: Box<TSType>,
    #[serde(rename = "falseType")]
    pub false_type: Box<TSType>,
}

/// Mapped type: `{ [K in keyof T]: V }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSMappedType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "typeParameter")]
    pub type_parameter: TSMappedTypeParameter,
    /// Optional key remapping: `as NewK`
    #[serde(rename = "nameType")]
    pub name_type: Option<Box<TSType>>,
    /// The value type
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Option<Box<TSType>>,
    /// Readonly modifier: true, "+", "-", or absent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readonly: Option<TSMappedTypeModifier>,
    /// Optional modifier: true, "+", "-", or absent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<TSMappedTypeModifier>,
}

/// Type parameter in a mapped type: `K in keyof T`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSMappedTypeParameter {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The parameter name (just the string, not an Identifier in mapped types)
    pub name: String,
    /// The constraint type (e.g., `keyof T`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<Box<TSType>>,
}

/// Mapped type modifier value: true, "+", or "-"
#[derive(Debug, Clone)]
pub enum TSMappedTypeModifier {
    True,
    Plus,
    Minus,
}

impl Serialize for TSMappedTypeModifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            TSMappedTypeModifier::True => serializer.serialize_bool(true),
            TSMappedTypeModifier::Plus => serializer.serialize_str("+"),
            TSMappedTypeModifier::Minus => serializer.serialize_str("-"),
        }
    }
}

impl<'de> Deserialize<'de> for TSMappedTypeModifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct ModifierVisitor;

        impl<'de> Visitor<'de> for ModifierVisitor {
            type Value = TSMappedTypeModifier;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("true, \"+\", or \"-\"")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value {
                    Ok(TSMappedTypeModifier::True)
                } else {
                    Err(E::custom("expected true, not false"))
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    "+" => Ok(TSMappedTypeModifier::Plus),
                    "-" => Ok(TSMappedTypeModifier::Minus),
                    _ => Err(E::custom(format!("expected '+' or '-', got '{value}'"))),
                }
            }
        }

        deserializer.deserialize_any(ModifierVisitor)
    }
}

/// Type operator: `keyof T`, `unique symbol`, `readonly T`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSTypeOperator {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The operator: "keyof", "unique", "readonly"
    pub operator: String,
    /// The type being operated on
    #[serde(rename = "typeAnnotation")]
    pub type_annotation: Box<TSType>,
}

/// Import type: `import('module')` or `import('module', {with: {...}}).Qualifier<T>`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSImportType {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The module specifier (string literal)
    pub argument: Literal,
    /// Optional options object: `{with: {type: 'json'}}`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Box<Expression>>,
    /// Optional qualifier: `.Foo` or `.Foo.Bar` after the import
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualifier: Option<TSEntityName>,
    /// Optional type arguments: `<T, U>`
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation>,
}

/// Type query expression name: Identifier, QualifiedName, or ImportType
///
/// The `exprName` field of `TSTypeQuery` can be:
/// - `Identifier` for `typeof x`
/// - `TSQualifiedName` for `typeof Foo.bar`
/// - `TSImportType` for `typeof import("module")`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TSTypeQueryExprName {
    Identifier(Identifier),
    QualifiedName(TSQualifiedName),
    Import(TSImportType),
}

/// Type query: `typeof x`, `typeof Foo.bar`, `typeof import("module")`, `typeof Array<T>`
///
/// Gets the type of a value expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSTypeQuery {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// The expression whose type is being queried
    #[serde(rename = "exprName")]
    pub expr_name: TSTypeQueryExprName,
    /// Optional type arguments: `<T, U>` (e.g., `typeof Array<string>`)
    #[serde(rename = "typeArguments", skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<TSTypeParameterInstantiation>,
}

/// TypeScript parameter property: `constructor(public x: number)`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSParameterProperty {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Accessibility modifier: "public", "private", or "protected"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<String>,
    /// Whether the parameter is readonly
    #[serde(skip_serializing_if = "is_false")]
    pub readonly: bool,
    /// The parameter - can be Identifier or AssignmentPattern (with default value)
    pub parameter: Box<Expression>,
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
