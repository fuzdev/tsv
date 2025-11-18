// Internal AST - optimized for traversal and manipulation
// Uses string interning for memory efficiency

use string_interner::{DefaultStringInterner, DefaultSymbol};
use tsv_lang::Span;

#[derive(Debug, Clone)]
pub struct Comment {
    pub content: String,
    pub is_block: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub body: Vec<Statement>,
    pub comments: Vec<Comment>,
    pub span: Span,
    pub interner: std::rc::Rc<std::cell::RefCell<DefaultStringInterner>>,
}

#[derive(Debug, Clone)]
pub enum Statement {
    ExpressionStatement(ExpressionStatement),
    VariableDeclaration(VariableDeclaration),
}

impl Statement {
    pub fn span(&self) -> Span {
        match self {
            Statement::ExpressionStatement(stmt) => stmt.span,
            Statement::VariableDeclaration(decl) => decl.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExpressionStatement {
    pub expression: Expression,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Expression {
    Literal(Literal),
    Identifier(Identifier),
    ObjectExpression(ObjectExpression),
    // TODO: BinaryExpression, etc.
}

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::Literal(lit) => lit.span,
            Expression::Identifier(id) => id.span,
            Expression::ObjectExpression(obj) => obj.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectExpression {
    pub properties: Vec<Property>,
    pub span: Span,
}

// TODO: Refactor Property to use PropertyKind enum for type safety
// Current: Separate bool fields (shorthand, computed, method)
// Proposed: PropertyKind enum with Init/Get/Set variants
// Benefits: Type-safe, easier to add getters/setters, cleaner pattern matching
// Example:
//   enum PropertyKind {
//     Init { shorthand: bool, computed: bool, method: bool },
//     Get { computed: bool },
//     Set { computed: bool },
//   }
// This would make it impossible to have invalid combinations like shorthand getter

#[derive(Debug, Clone)]
pub struct Property {
    pub key: Expression,
    pub value: Expression,
    pub shorthand: bool,   // true for `{ prop }`, false for `{ prop: value }`
    pub computed: bool,    // true for `{ [expr]: value }`, false for `{ prop: value }`
    pub method: bool,      // true for `{ foo() {} }`, false for regular properties
    // TODO: Add support for property decorators (TypeScript)
    // Requires: decorators: Vec<Decorator> field
    // See: TypeScript AST PropertyDeclaration
    pub span: Span,
}

/// Literal value type - supports numbers and strings
#[derive(Debug, Clone)]
pub enum LiteralValue {
    Number(f64),
    String {
        content: String, // string content without quotes (decoded)
        quote: char,     // original quote character (' or ")
    },
}

#[derive(Debug, Clone)]
pub struct Literal {
    pub value: LiteralValue,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Identifier {
    pub name: DefaultSymbol,
    pub type_annotation: Option<TSTypeAnnotation>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VariableDeclarationKind {
    Const,
    Let,
    Var,
}

impl VariableDeclarationKind {
    /// Returns the string representation of the variable declaration kind
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Const => "const",
            Self::Let => "let",
            Self::Var => "var",
        }
    }
}

#[derive(Debug, Clone)]
pub struct VariableDeclaration {
    pub kind: VariableDeclarationKind,
    pub declarations: Vec<VariableDeclarator>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct VariableDeclarator {
    pub id: Identifier,
    pub init: Option<Expression>,
    pub span: Span,
}

// TypeScript type annotation nodes

/// TypeScript type annotation node (e.g., `: number` in `const a: number = 5`)
///
/// Represents the full type annotation including the colon. The span covers the
/// entire annotation (`: number`), while the inner type covers just the type (`number`).
///
/// # Memory Layout
/// Uses `Box<TSType>` to avoid bloating `Identifier` size. The indirection is acceptable
/// since type annotations are relatively rare and accessed infrequently during traversal.
#[derive(Debug, Clone)]
pub struct TSTypeAnnotation {
    pub type_annotation: Box<TSType>,
    pub span: Span,
}

/// TypeScript type expression
///
/// Represents the various types in TypeScript's type system. Currently only
/// primitive keyword types are implemented. Complex types (unions, intersections,
/// generics, etc.) will be added incrementally.
#[derive(Debug, Clone)]
pub enum TSType {
    /// The `number` type keyword
    TSNumberKeyword(TSNumberKeyword),
    // TODO: TSStringKeyword, TSBooleanKeyword, etc.
}

impl TSType {
    pub fn span(&self) -> Span {
        match self {
            TSType::TSNumberKeyword(node) => node.span,
        }
    }
}

/// TypeScript `number` type keyword
///
/// Represents the primitive `number` type in TypeScript.
/// The span covers just the keyword itself (not including surrounding whitespace).
#[derive(Debug, Clone)]
pub struct TSNumberKeyword {
    pub span: Span,
}
