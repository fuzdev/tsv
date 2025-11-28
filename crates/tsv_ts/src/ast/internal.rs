// Internal AST - optimized for traversal and manipulation
// Uses string interning for memory efficiency

use string_interner::{DefaultStringInterner, DefaultSymbol};
pub use tsv_lang::{Comment, Span};

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
    ArrayExpression(ArrayExpression),
    UnaryExpression(UnaryExpression),
    BinaryExpression(BinaryExpression),
    CallExpression(CallExpression),
    MemberExpression(MemberExpression),
    ConditionalExpression(ConditionalExpression),
    ArrowFunctionExpression(ArrowFunctionExpression),
    SpreadElement(SpreadElement),
}

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::Literal(lit) => lit.span,
            Expression::Identifier(id) => id.span,
            Expression::ObjectExpression(obj) => obj.span,
            Expression::ArrayExpression(arr) => arr.span,
            Expression::UnaryExpression(unary) => unary.span,
            Expression::BinaryExpression(binary) => binary.span,
            Expression::CallExpression(call) => call.span,
            Expression::MemberExpression(member) => member.span,
            Expression::ConditionalExpression(cond) => cond.span,
            Expression::ArrowFunctionExpression(arrow) => arrow.span,
            Expression::SpreadElement(spread) => spread.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectExpression {
    pub properties: Vec<ObjectProperty>,
    pub span: Span,
}

/// Object property - either a regular property or a spread element
#[derive(Debug, Clone)]
pub enum ObjectProperty {
    Property(Property),
    SpreadElement(SpreadElement),
}

impl ObjectProperty {
    pub fn span(&self) -> Span {
        match self {
            ObjectProperty::Property(p) => p.span,
            ObjectProperty::SpreadElement(s) => s.span,
        }
    }

    /// Get the end position of the property value (for determining separator position)
    pub fn value_end(&self) -> u32 {
        match self {
            ObjectProperty::Property(p) => {
                if p.shorthand {
                    p.key.span().end
                } else {
                    p.value.span().end
                }
            }
            ObjectProperty::SpreadElement(s) => s.argument.span().end,
        }
    }

    /// Check if this is a shorthand property (only makes sense for Property)
    pub fn is_shorthand(&self) -> bool {
        match self {
            ObjectProperty::Property(p) => p.shorthand,
            ObjectProperty::SpreadElement(_) => false,
        }
    }

    /// Get the property as a regular Property, if it is one
    pub fn as_property(&self) -> Option<&Property> {
        match self {
            ObjectProperty::Property(p) => Some(p),
            ObjectProperty::SpreadElement(_) => None,
        }
    }

    /// Get the spread element, if it is one
    pub fn as_spread(&self) -> Option<&SpreadElement> {
        match self {
            ObjectProperty::Property(_) => None,
            ObjectProperty::SpreadElement(s) => Some(s),
        }
    }
}

/// Array literal expression: `[1, 2, 3]`
///
/// Elements are wrapped in Option to support sparse arrays like `[1,,3]`
/// where missing elements are represented as None.
#[derive(Debug, Clone)]
pub struct ArrayExpression {
    pub elements: Vec<Option<Expression>>,
    pub span: Span,
}

/// Unary expression operator
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOperator {
    Minus, // -
    Plus,  // +
    Bang,  // !
           // TODO: typeof, void, delete, ~
}

impl UnaryOperator {
    pub fn as_str(&self) -> &'static str {
        match self {
            UnaryOperator::Minus => "-",
            UnaryOperator::Plus => "+",
            UnaryOperator::Bang => "!",
        }
    }
}

/// Unary expression: `-x`, `+x`, `!x`, etc.
#[derive(Debug, Clone)]
pub struct UnaryExpression {
    pub operator: UnaryOperator,
    pub argument: Box<Expression>,
    pub prefix: bool, // always true for now (prefix operators)
    pub span: Span,
}

/// Binary expression operator
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOperator {
    // Arithmetic
    Plus,    // +
    Minus,   // -
    Star,    // *
    Slash,   // /
    Percent, // %
    // Comparison
    LessThan,           // <
    GreaterThan,        // >
    LessThanEquals,     // <=
    GreaterThanEquals,  // >=
    EqualsEquals,       // ==
    EqualsEqualsEquals, // ===
    BangEquals,         // !=
    BangEqualsEquals,   // !==
    // Logical
    AmpersandAmpersand, // &&
    PipePipe,           // ||
    QuestionQuestion,   // ??
    // Bitwise
    Ampersand, // &
    Pipe,      // |
}

impl BinaryOperator {
    pub fn as_str(&self) -> &'static str {
        match self {
            BinaryOperator::Plus => "+",
            BinaryOperator::Minus => "-",
            BinaryOperator::Star => "*",
            BinaryOperator::Slash => "/",
            BinaryOperator::Percent => "%",
            BinaryOperator::LessThan => "<",
            BinaryOperator::GreaterThan => ">",
            BinaryOperator::LessThanEquals => "<=",
            BinaryOperator::GreaterThanEquals => ">=",
            BinaryOperator::EqualsEquals => "==",
            BinaryOperator::EqualsEqualsEquals => "===",
            BinaryOperator::BangEquals => "!=",
            BinaryOperator::BangEqualsEquals => "!==",
            BinaryOperator::AmpersandAmpersand => "&&",
            BinaryOperator::PipePipe => "||",
            BinaryOperator::QuestionQuestion => "??",
            BinaryOperator::Ampersand => "&",
            BinaryOperator::Pipe => "|",
        }
    }

    /// Get precedence level for this operator
    pub fn precedence(&self) -> crate::ast::precedence::PrecedenceLevel {
        crate::ast::precedence::get_precedence(*self)
    }

    /// Check if this operator can flatten with another operator
    pub fn can_flatten_with(&self, other: BinaryOperator) -> bool {
        crate::ast::precedence::should_flatten(*self, other)
    }
}

/// Binary expression: `a + b`, `x && y`, etc.
#[derive(Debug, Clone)]
pub struct BinaryExpression {
    pub left: Box<Expression>,
    pub operator: BinaryOperator,
    pub right: Box<Expression>,
    pub span: Span,
}

/// Call expression: `foo()`, `obj.method(arg1, arg2)`
#[derive(Debug, Clone)]
pub struct CallExpression {
    pub callee: Box<Expression>,
    pub arguments: Vec<Expression>,
    pub optional: bool, // true for `foo?.()` (optional chaining)
    pub span: Span,
}

/// Member expression: `obj.prop`, `arr[0]`
#[derive(Debug, Clone)]
pub struct MemberExpression {
    pub object: Box<Expression>,
    pub property: Box<Expression>,
    pub computed: bool, // true for `arr[0]`, false for `obj.prop`
    pub optional: bool, // true for `obj?.prop` (optional chaining)
    pub span: Span,
}

/// Conditional (ternary) expression: `a ? b : c`
#[derive(Debug, Clone)]
pub struct ConditionalExpression {
    pub test: Box<Expression>,
    pub consequent: Box<Expression>,
    pub alternate: Box<Expression>,
    pub span: Span,
}

/// Arrow function expression: `() => expr` or `() => { stmts }`
///
/// Supports both expression bodies and block bodies:
/// - Expression body: `x => x + 1` (body is Expression)
/// - Block body: `x => { return x + 1; }` (body is BlockStatement)
///
/// For now, we only support expression bodies since block statements
/// require additional parsing infrastructure.
#[derive(Debug, Clone)]
pub struct ArrowFunctionExpression {
    pub params: Vec<Identifier>,
    pub body: ArrowFunctionBody,
    pub expression: bool, // true for expression body, false for block body
    pub span: Span,
}

/// Arrow function body - either an expression or a block statement
#[derive(Debug, Clone)]
pub enum ArrowFunctionBody {
    /// Expression body: `() => expr`
    Expression(Box<Expression>),
    /// Block body: `() => { stmts }` - stores raw source for now
    /// TODO: Parse block statements properly
    BlockStatement { span: Span },
}

impl ArrowFunctionBody {
    pub fn span(&self) -> Span {
        match self {
            ArrowFunctionBody::Expression(expr) => expr.span(),
            ArrowFunctionBody::BlockStatement { span } => *span,
        }
    }
}

/// Spread element: `...expr`
///
/// Used in array literals (`[...arr]`) and object literals (`{...obj}`)
#[derive(Debug, Clone)]
pub struct SpreadElement {
    pub argument: Box<Expression>,
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
    pub shorthand: bool, // true for `{ prop }`, false for `{ prop: value }`
    pub computed: bool,  // true for `{ [expr]: value }`, false for `{ prop: value }`
    pub method: bool,    // true for `{ foo() {} }`, false for regular properties
    // TODO: Add support for property decorators (TypeScript)
    // Requires: decorators: Vec<Decorator> field
    // See: TypeScript AST PropertyDeclaration
    pub span: Span,
}

/// Literal value type - supports numbers, strings, booleans, and null
#[derive(Debug, Clone)]
pub enum LiteralValue {
    Number(f64),
    String {
        content: String, // string content without quotes (decoded)
        quote: char,     // original quote character (' or ")
    },
    Boolean(bool),
    Null,
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
