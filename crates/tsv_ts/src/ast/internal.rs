// Internal AST - optimized for traversal and manipulation
// Uses string interning for memory efficiency

use string_interner::{DefaultStringInterner, DefaultSymbol};
use tsv_lang::Span;

#[derive(Debug, Clone)]
pub struct Program {
    pub body: Vec<Statement>,
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
    // TODO: BinaryExpression, etc.
}

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::Literal(lit) => lit.span,
            Expression::Identifier(id) => id.span,
        }
    }
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
