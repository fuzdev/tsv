//! Public AST types - with serde, matches Svelte's JSON structure exactly
//!
//! Uses u32 for positions (max 4GB file size) for memory efficiency.

use serde::{Deserialize, Serialize};

pub mod classes;
pub mod declarations;
pub mod expressions;
pub mod modules;
pub mod patterns;
pub mod statements;
pub mod types;

// ============================================================================
// Re-exports from submodules
// ============================================================================

// Types
pub use types::{
    TSAnyKeyword, TSArrayType, TSBigIntKeyword, TSBooleanKeyword, TSCallSignatureDeclaration,
    TSConditionalType, TSConstructSignatureDeclaration, TSConstructorType, TSEntityName,
    TSFunctionType, TSImportType, TSIndexSignature, TSIndexedAccessType, TSInferType,
    TSInterfaceBody, TSIntersectionType, TSLiteralType, TSLiteralTypeLiteral, TSMappedType,
    TSMappedTypeModifier, TSMappedTypeParameter, TSMethodSignature, TSNamedTupleMember,
    TSNeverKeyword, TSNullKeyword, TSNumberKeyword, TSObjectKeyword, TSOptionalType,
    TSParenthesizedType, TSPropertySignature, TSQualifiedName, TSRestType, TSStringKeyword,
    TSSymbolKeyword, TSTupleType, TSType, TSTypeAliasDeclaration, TSTypeAnnotation, TSTypeElement,
    TSTypeLiteral, TSTypeOperator, TSTypeParameter, TSTypeParameterDeclaration,
    TSTypeParameterInstantiation, TSTypePredicate, TSTypeQuery, TSTypeQueryExprName,
    TSTypeReference, TSUndefinedKeyword, TSUnionType, TSUnknownKeyword, TSVoidKeyword,
    TemplateLiteralType,
};

// Declarations
pub use declarations::{
    TSDeclareFunction, TSEnumDeclaration, TSEnumMember, TSEnumMemberId, TSInterfaceDeclaration,
    TSInterfaceHeritage, TSModuleBlock, TSModuleDeclaration, TSModuleDeclarationBody, TSModuleName,
};

// Modules (imports/exports)
pub use modules::{
    ExportAllDeclaration, ExportDefaultDeclaration, ExportDefaultValue, ExportNamedDeclaration,
    ExportSpecifier, ImportAttribute, ImportDeclaration, ImportDefaultSpecifier,
    ImportNamedSpecifier, ImportNamespaceSpecifier, ImportSpecifier, TSExportAssignment,
    TSExternalModuleReference, TSImportEqualsDeclaration, TSModuleReference,
};

// Classes
pub use classes::{
    ClassBody, ClassDeclaration, ClassExpression, ClassMember, FunctionExpression,
    MethodDefinition, PropertyDefinition, StaticBlock, TSExpressionWithTypeArguments,
    TSParameterProperty,
};

// Patterns
pub use patterns::{
    ArrayPattern, AssignmentPattern, ObjectPattern, ObjectPatternProperty, RestElement,
};

// Statements
pub use statements::{
    BlockStatement, BreakStatement, CatchClause, ContinueStatement, DoWhileStatement,
    EmptyStatement, ExpressionStatement, ForInOfLeft, ForInStatement, ForInit, ForOfStatement,
    ForStatement, FunctionDeclaration, IfStatement, LabeledStatement, ReturnStatement, Statement,
    SwitchCase, SwitchStatement, ThrowStatement, TryStatement, VariableDeclaration,
    VariableDeclarator, WhileStatement,
};

// Expressions
pub use expressions::{
    ArrayExpression, ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression,
    AwaitExpression, BinaryExpression, CallExpression, ConditionalExpression, Expression,
    ImportExpression, MemberExpression, MetaProperty, NewExpression, ObjectExpression,
    ObjectProperty, Property, RegexLiteral, RegexValue, SequenceExpression, SpreadElement, Super,
    TSAsExpression, TSInstantiationExpression, TSNonNullExpression, TSSatisfiesExpression,
    TSTypeAssertion, TaggedTemplateExpression, TemplateElement, TemplateElementValue,
    TemplateLiteral, UnaryExpression, UpdateExpression, YieldExpression,
};

// ============================================================================
// Helper functions
// ============================================================================

/// Helper for skip_serializing_if to skip false bools
#[allow(clippy::trivially_copy_pass_by_ref)] // serde requires &T signature
pub(crate) fn is_false(b: &bool) -> bool {
    !*b
}

/// Serialize numbers as integers if they have no fractional part
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

// ============================================================================
// Foundational Types (defined here, used everywhere)
// ============================================================================

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
