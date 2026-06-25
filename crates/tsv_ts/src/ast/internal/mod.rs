//! Internal AST - optimized for traversal and manipulation
//!
//! Uses string interning for memory efficiency. This is the primary AST representation
//! used by the parser, formatter, and other tools.

mod classes;
mod declarations;
mod expressions;
mod modules;
mod patterns;
mod statements;
mod types;

use string_interner::{DefaultStringInterner, DefaultSymbol};
pub use tsv_lang::{Comment, Span};

//
// Re-exports from submodules
//

// Types
pub use types::{
    TSArrayType, TSCallSignatureDeclaration, TSConditionalType, TSConstructSignatureDeclaration,
    TSConstructorType, TSEntityName, TSFunctionType, TSImportType, TSIndexSignature,
    TSIndexedAccessType, TSInferType, TSIntersectionType, TSKeywordKind, TSKeywordType,
    TSLiteralType, TSMappedType, TSMappedTypeModifier, TSMappedTypeParameter, TSMethodSignature,
    TSNamedTupleMember, TSOptionalType, TSParenthesizedType, TSPropertySignature, TSQualifiedName,
    TSRestType, TSThisType, TSTupleType, TSType, TSTypeAliasDeclaration, TSTypeAnnotation,
    TSTypeElement, TSTypeLiteral, TSTypeOperator, TSTypeOperatorKind, TSTypeParameter,
    TSTypeParameterDeclaration, TSTypeParameterInstantiation, TSTypePredicate, TSTypeQuery,
    TSTypeQueryExprName, TSTypeReference, TSUnionType, TemplateLiteralType,
};

// Declarations
pub use declarations::{
    TSDeclareFunction, TSEnumDeclaration, TSEnumMember, TSEnumMemberId, TSInterfaceBody,
    TSInterfaceDeclaration, TSInterfaceHeritage, TSModuleBlock, TSModuleDeclaration,
    TSModuleDeclarationBody, TSModuleDeclarationKind, TSModuleName,
};

// Modules (imports/exports)
pub use modules::{
    ExportAllDeclaration, ExportDefaultDeclaration, ExportDefaultValue, ExportFunctionDeclaration,
    ExportKind, ExportNamedDeclaration, ExportSpecifier, ImportAttribute, ImportAttributeKey,
    ImportDeclaration, ImportDefaultSpecifier, ImportKind, ImportNamedSpecifier,
    ImportNamespaceSpecifier, ImportSpecifier, ModuleExportName, TSExportAssignment,
    TSExternalModuleReference, TSImportEqualsDeclaration, TSModuleReference,
};

// Classes
pub use classes::{
    Accessibility, ClassBody, ClassDeclaration, ClassExpression, ClassMember, MethodDefinition,
    MethodKind, PropertyDefinition, PropertyModifier, StaticBlock, TSParameterProperty,
};

// Patterns
pub use patterns::{
    ArrayPattern, AssignmentPattern, ObjectPattern, ObjectPatternProperty, RestElement,
};

// Statements
pub use statements::{
    BlockStatement, BreakStatement, CatchClause, ContinueStatement, DebuggerStatement,
    DoWhileStatement, EmptyStatement, ExpressionStatement, ForInOfLeft, ForInStatement, ForInit,
    ForOfStatement, ForStatement, FunctionDeclaration, IfStatement, LabeledStatement,
    ReturnStatement, Statement, SwitchCase, SwitchStatement, ThrowStatement, TryStatement,
    VariableDeclaration, VariableDeclarationKind, VariableDeclarator, WhileStatement,
};

// Expressions
pub use expressions::{
    ArrayExpression, ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression,
    AssignmentOperator, AwaitExpression, BinaryExpression, BinaryOperator, CallExpression,
    ConditionalExpression, Expression, FunctionExpression, ImportExpression, JsdocCast,
    MemberExpression, MetaProperty, NewExpression, ObjectExpression, ObjectProperty, Property,
    PropertyKind, RegexLiteral, SequenceExpression, SpreadElement, Super, TSAsExpression,
    TSInstantiationExpression, TSNonNullExpression, TSSatisfiesExpression, TSTypeAssertion,
    TaggedTemplateExpression, TemplateCooked, TemplateElement, TemplateLiteral, ThisExpression,
    UnaryExpression, UnaryOperator, UpdateExpression, UpdateOperator, YieldExpression,
};

//
// Foundational Types (defined here, used everywhere)
//

/// Program node - the root of the AST
#[derive(Debug, Clone)]
pub struct Program {
    pub body: Vec<Statement>,
    pub comments: Vec<Comment>,
    /// Precomputed line break positions (byte offsets of newlines).
    /// Used for O(log n) line boundary lookups during printing.
    pub line_breaks: Vec<u32>,
    pub span: Span,
    pub interner: std::rc::Rc<std::cell::RefCell<DefaultStringInterner>>,
}

/// Decorator: `@expression` applied to classes and class members
///
/// The expression can be an identifier (`@foo`), call expression (`@foo()`),
/// or member expression (`@foo.bar`).
#[derive(Debug, Clone)]
pub struct Decorator {
    /// The decorator expression (identifier, call, or member expression)
    pub expression: Expression,
    pub span: Span,
}

/// Literal value type - supports numbers, strings, booleans, null, and undefined
#[derive(Debug, Clone)]
pub enum LiteralValue {
    Number(f64),
    String {
        content: String, // string content without quotes (decoded)
        quote: char,     // original quote character (' or ")
    },
    /// BigInt literal: `1n`, `100n`, `0xffn`
    /// Value stored as string since BigInt can exceed f64 precision
    BigInt(String),
    Boolean(bool),
    Null,
    Undefined,
}

#[derive(Debug, Clone)]
pub struct Literal {
    pub value: LiteralValue,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Identifier {
    pub name: DefaultSymbol,
    /// Whether this is an optional parameter (e.g., `a?` in `function fn(a?: number) {}`)
    pub optional: bool,
    pub type_annotation: Option<TSTypeAnnotation>,
    /// Decorators applied to this parameter (TypeScript parameter decorators)
    pub decorators: Option<Vec<Decorator>>,
    pub span: Span,
}

impl Identifier {
    /// Create a simple identifier with no optional flag or type annotation.
    ///
    /// Use this for identifiers in expression context (not parameters).
    /// For parameters that may have `?` or type annotations, construct directly.
    #[inline]
    pub fn simple(name: DefaultSymbol, span: Span) -> Self {
        Self {
            name,
            optional: false,
            type_annotation: None,
            decorators: None,
            span,
        }
    }
}

/// Private identifier: `#foo` in class fields and methods
///
/// Used for truly private class members (ES2022 private class fields).
/// The name does NOT include the `#` prefix - it's stored separately.
/// The span DOES include the `#` character.
#[derive(Debug, Clone)]
pub struct PrivateIdentifier {
    /// The name without the `#` prefix (e.g., "foo" for `#foo`)
    pub name: DefaultSymbol,
    pub span: Span,
}
