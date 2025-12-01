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
    TSTypeAliasDeclaration(TSTypeAliasDeclaration),
    ReturnStatement(ReturnStatement),
    BlockStatement(BlockStatement),
    FunctionDeclaration(FunctionDeclaration),
    ClassDeclaration(ClassDeclaration),
}

impl Statement {
    pub fn span(&self) -> Span {
        match self {
            Statement::ExpressionStatement(stmt) => stmt.span,
            Statement::VariableDeclaration(decl) => decl.span,
            Statement::TSTypeAliasDeclaration(decl) => decl.span,
            Statement::ReturnStatement(stmt) => stmt.span,
            Statement::BlockStatement(block) => block.span,
            Statement::FunctionDeclaration(decl) => decl.span,
            Statement::ClassDeclaration(decl) => decl.span,
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

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::Literal(lit) => lit.span,
            Expression::Identifier(id) => id.span,
            Expression::ObjectExpression(obj) => obj.span,
            Expression::ArrayExpression(arr) => arr.span,
            Expression::UnaryExpression(unary) => unary.span,
            Expression::UpdateExpression(update) => update.span,
            Expression::BinaryExpression(binary) => binary.span,
            Expression::CallExpression(call) => call.span,
            Expression::NewExpression(new) => new.span,
            Expression::MemberExpression(member) => member.span,
            Expression::ConditionalExpression(cond) => cond.span,
            Expression::ArrowFunctionExpression(arrow) => arrow.span,
            Expression::FunctionExpression(func) => func.span,
            Expression::SpreadElement(spread) => spread.span,
            Expression::TemplateLiteral(template) => template.span,
            Expression::TaggedTemplateExpression(tagged) => tagged.span,
            Expression::AwaitExpression(await_expr) => await_expr.span,
            Expression::SequenceExpression(seq) => seq.span,
            Expression::RegexLiteral(regex) => regex.span,
            Expression::Super(s) => s.span,
            Expression::AssignmentExpression(assign) => assign.span,
            Expression::ObjectPattern(obj) => obj.span,
            Expression::ArrayPattern(arr) => arr.span,
            Expression::AssignmentPattern(assign) => assign.span,
            Expression::RestElement(rest) => rest.span,
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

/// Update expression operator: `++` or `--`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UpdateOperator {
    Increment = 0, // ++
    Decrement = 1, // --
}

impl UpdateOperator {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            UpdateOperator::Increment => "++",
            UpdateOperator::Decrement => "--",
        }
    }
}

/// Update expression: `++x`, `x++`, `--x`, `x--`
///
/// Used for increment and decrement operations. The `prefix` field
/// indicates whether the operator appears before (true) or after (false)
/// the argument.
#[derive(Debug, Clone)]
pub struct UpdateExpression {
    pub operator: UpdateOperator,
    pub argument: Box<Expression>,
    pub prefix: bool, // true for `++x`/`--x`, false for `x++`/`x--`
    pub span: Span,
}

/// Unary expression operator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UnaryOperator {
    Minus = 0,  // -
    Plus = 1,   // +
    Bang = 2,   // !
    Typeof = 3, // typeof
    Void = 4,   // void
    Delete = 5, // delete
    Tilde = 6,  // ~
}

impl UnaryOperator {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            UnaryOperator::Minus => "-",
            UnaryOperator::Plus => "+",
            UnaryOperator::Bang => "!",
            UnaryOperator::Typeof => "typeof",
            UnaryOperator::Void => "void",
            UnaryOperator::Delete => "delete",
            UnaryOperator::Tilde => "~",
        }
    }

    /// Returns true if this is a keyword operator (needs space after)
    #[inline]
    pub const fn is_keyword_operator(self) -> bool {
        matches!(
            self,
            UnaryOperator::Typeof | UnaryOperator::Void | UnaryOperator::Delete
        )
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BinaryOperator {
    // Arithmetic
    Plus = 0,    // +
    Minus = 1,   // -
    Star = 2,    // *
    Slash = 3,   // /
    Percent = 4, // %
    // Comparison
    LessThan = 5,            // <
    GreaterThan = 6,         // >
    LessThanEquals = 7,      // <=
    GreaterThanEquals = 8,   // >=
    EqualsEquals = 9,        // ==
    EqualsEqualsEquals = 10, // ===
    BangEquals = 11,         // !=
    BangEqualsEquals = 12,   // !==
    // Logical
    AmpersandAmpersand = 13, // &&
    PipePipe = 14,           // ||
    QuestionQuestion = 15,   // ??
    // Bitwise
    Ampersand = 16, // &
    Pipe = 17,      // |
    Caret = 18,     // ^
    // Bitshift
    LeftShift = 19,          // <<
    RightShift = 20,         // >>
    UnsignedRightShift = 21, // >>>
    // Exponentiation
    StarStar = 22, // **
    // Relational keywords
    Instanceof = 23, // instanceof
    In = 24,         // in
}

impl BinaryOperator {
    #[inline]
    pub const fn as_str(self) -> &'static str {
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
            BinaryOperator::Caret => "^",
            BinaryOperator::LeftShift => "<<",
            BinaryOperator::RightShift => ">>",
            BinaryOperator::UnsignedRightShift => ">>>",
            BinaryOperator::StarStar => "**",
            BinaryOperator::Instanceof => "instanceof",
            BinaryOperator::In => "in",
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

/// New expression: `new Date()`, `new Map()`
///
/// Constructor call with the `new` keyword. The callee is typically an
/// identifier or member expression, and arguments are optional.
#[derive(Debug, Clone)]
pub struct NewExpression {
    pub callee: Box<Expression>,
    pub arguments: Vec<Expression>,
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
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Vec<Expression>,
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

/// Function expression: `function() {}` or method shorthand `{ foo() {} }`
///
/// Used for:
/// - Method shorthand in objects: `{ foo() { return 1; } }`
/// - Anonymous function expressions: `const f = function() {}`
/// - Named function expressions: `const f = function name() {}`
#[derive(Debug, Clone)]
pub struct FunctionExpression {
    /// Optional function name (for named function expressions)
    pub id: Option<Identifier>,
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Vec<Expression>,
    /// Function body (block statement with statements)
    pub body: BlockStatement,
    pub span: Span,
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

/// Function declaration: `function foo(x) { return x + 1; }`
///
/// Unlike FunctionExpression, the id (function name) is required.
/// Declarations are hoisted and can be called before they appear in source.
#[derive(Debug, Clone)]
pub struct FunctionDeclaration {
    /// Function name (required for declarations)
    pub id: Identifier,
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Vec<Expression>,
    /// Function body (block statement with statements)
    pub body: BlockStatement,
    /// Whether this is a generator function (`function*`)
    pub generator: bool,
    /// Whether this is an async function (`async function`)
    pub r#async: bool,
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

/// Class declaration: `class Foo { ... }` or `class Foo extends Bar { ... }`
///
/// Represents a class declaration with optional superclass.
#[derive(Debug, Clone)]
pub struct ClassDeclaration {
    /// Class name (required for declarations)
    pub id: Identifier,
    /// Optional superclass expression (for `extends`)
    pub super_class: Option<Box<Expression>>,
    /// Class body containing methods and properties
    pub body: ClassBody,
    pub span: Span,
}

/// Class body: `{ constructor() {} method() {} }`
///
/// Contains the methods and properties of a class.
#[derive(Debug, Clone)]
pub struct ClassBody {
    pub body: Vec<MethodDefinition>,
    pub span: Span,
}

/// Method definition kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MethodKind {
    Constructor = 0,
    Method = 1,
    Get = 2,
    Set = 3,
}

impl MethodKind {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            MethodKind::Constructor => "constructor",
            MethodKind::Method => "method",
            MethodKind::Get => "get",
            MethodKind::Set => "set",
        }
    }
}

/// Method definition in a class body: `method() { ... }` or `get x() { ... }`
#[derive(Debug, Clone)]
pub struct MethodDefinition {
    /// Method name (key)
    pub key: Expression,
    /// Method implementation (value)
    pub value: FunctionExpression,
    /// Method kind (constructor, method, get, set)
    pub kind: MethodKind,
    /// Whether this is a static method
    pub is_static: bool,
    /// Whether the key is computed (`[expr]()`)
    pub computed: bool,
    pub span: Span,
}

/// Spread element: `...expr`
///
/// Used in array literals (`[...arr]`) and object literals (`{...obj}`)
#[derive(Debug, Clone)]
pub struct SpreadElement {
    pub argument: Box<Expression>,
    pub span: Span,
}

/// Property kind: init (regular), get (getter), set (setter)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum PropertyKind {
    #[default]
    Init = 0,
    Get = 1,
    Set = 2,
}

impl PropertyKind {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            PropertyKind::Init => "init",
            PropertyKind::Get => "get",
            PropertyKind::Set => "set",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Property {
    pub key: Expression,
    pub value: Expression,
    pub kind: PropertyKind, // init, get, or set
    pub shorthand: bool,    // true for `{ prop }`, false for `{ prop: value }`
    pub computed: bool,     // true for `{ [expr]: value }`, false for `{ prop: value }`
    pub method: bool,       // true for `{ foo() {} }`, false for regular properties
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
    Boolean(bool),
    Null,
    Undefined,
}

/// Template literal expression: `hello ${name}`
///
/// Template literals consist of:
/// - quasis: Array of TemplateElement nodes (static string parts)
/// - expressions: Array of interpolated expressions (inside ${})
///
/// For a template like `a ${b} c ${d} e`:
/// - quasis: ["a ", " c ", " e"]
/// - expressions: [b, d]
#[derive(Debug, Clone)]
pub struct TemplateLiteral {
    pub quasis: Vec<TemplateElement>,
    pub expressions: Vec<Expression>,
    pub span: Span,
}

/// Template element - a static string part of a template literal
///
/// Each quasi has:
/// - raw: The literal source text (preserving escape syntax)
/// - cooked: The decoded value (escapes interpreted), None if contains invalid escape
/// - tail: true for the last element in the template
#[derive(Debug, Clone)]
pub struct TemplateElement {
    /// The raw source text (escape sequences NOT decoded)
    pub raw: String,
    /// The decoded value (escape sequences interpreted)
    /// None for tagged templates with invalid escapes
    pub cooked: Option<String>,
    /// True if this is the last element (tail)
    pub tail: bool,
    pub span: Span,
}

/// Tagged template expression: tag`content ${expr}`
///
/// The tag is called with the template's static parts and interpolated values.
#[derive(Debug, Clone)]
pub struct TaggedTemplateExpression {
    pub tag: Box<Expression>,
    pub quasi: TemplateLiteral,
    pub span: Span,
}

/// Await expression: `await promise`
///
/// Used in async functions to wait for a Promise to resolve.
/// The argument is the expression being awaited.
#[derive(Debug, Clone)]
pub struct AwaitExpression {
    pub argument: Box<Expression>,
    pub span: Span,
}

/// Sequence expression: `a, b, c`
///
/// Evaluates all expressions left to right, returns the last value.
/// Created by the comma operator at expression level.
#[derive(Debug, Clone)]
pub struct SequenceExpression {
    pub expressions: Vec<Expression>,
    pub span: Span,
}

/// Regular expression literal: `/pattern/flags`
///
/// Represents a regex literal with its pattern and flags.
/// Unlike strings, regex patterns are NOT decoded - escape sequences are preserved.
///
/// TODO: Add regex validation (pattern is valid, flags are valid and unique)
#[derive(Debug, Clone)]
pub struct RegexLiteral {
    /// The pattern between the slashes (e.g., "\\d+")
    /// Contains the raw source text, preserving escape sequences.
    pub pattern: String,
    /// The flags after the closing slash (e.g., "gi")
    pub flags: String,
    pub span: Span,
}

/// Super expression: `super`
///
/// Used in class methods to reference the parent class:
/// - `super()` calls the parent constructor
/// - `super.method()` calls a parent method
/// - `super.prop` accesses a parent property
/// - `super[expr]` computed property access on parent
#[derive(Debug, Clone)]
pub struct Super {
    pub span: Span,
}

/// Assignment expression: `x = value`, `obj.prop = value`, `{a, b} = obj`
///
/// Represents assignment operations including:
/// - Simple assignment: `x = 1`
/// - Member assignment: `obj.x = 1`
/// - Destructuring: `{a, b} = obj`, `[x, y] = arr`
/// - Compound assignment: `x += 1` (uses AssignmentOperator)
#[derive(Debug, Clone)]
pub struct AssignmentExpression {
    /// The assignment target (identifier, member expression, or pattern)
    pub left: Box<Expression>,
    /// The operator: "=" for simple, "+=", "-=", etc. for compound
    pub operator: AssignmentOperator,
    /// The value being assigned
    pub right: Box<Expression>,
    pub span: Span,
}

/// Assignment operator: `=`, `+=`, `-=`, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AssignmentOperator {
    Assign = 0,                   // =
    AddAssign = 1,                // +=
    SubtractAssign = 2,           // -=
    MultiplyAssign = 3,           // *=
    DivideAssign = 4,             // /=
    RemainderAssign = 5,          // %=
    ExponentiateAssign = 6,       // **=
    LeftShiftAssign = 7,          // <<=
    RightShiftAssign = 8,         // >>=
    UnsignedRightShiftAssign = 9, // >>>=
    BitwiseOrAssign = 10,         // |=
    BitwiseXorAssign = 11,        // ^=
    BitwiseAndAssign = 12,        // &=
    LogicalOrAssign = 13,         // ||=
    LogicalAndAssign = 14,        // &&=
    NullishAssign = 15,           // ??=
}

impl AssignmentOperator {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            AssignmentOperator::Assign => "=",
            AssignmentOperator::AddAssign => "+=",
            AssignmentOperator::SubtractAssign => "-=",
            AssignmentOperator::MultiplyAssign => "*=",
            AssignmentOperator::DivideAssign => "/=",
            AssignmentOperator::RemainderAssign => "%=",
            AssignmentOperator::ExponentiateAssign => "**=",
            AssignmentOperator::LeftShiftAssign => "<<=",
            AssignmentOperator::RightShiftAssign => ">>=",
            AssignmentOperator::UnsignedRightShiftAssign => ">>>=",
            AssignmentOperator::BitwiseOrAssign => "|=",
            AssignmentOperator::BitwiseXorAssign => "^=",
            AssignmentOperator::BitwiseAndAssign => "&=",
            AssignmentOperator::LogicalOrAssign => "||=",
            AssignmentOperator::LogicalAndAssign => "&&=",
            AssignmentOperator::NullishAssign => "??=",
        }
    }
}

/// Object pattern for destructuring: `{a, b}`, `{a: x, b: y}`, `{...rest}`
///
/// Used as the left-hand side in destructuring assignments and declarations:
/// - `const {a, b} = obj`
/// - `({a, b} = obj)`
///
/// Properties can include:
/// - Shorthand: `{a}` (key equals value binding)
/// - Renamed: `{a: x}` (bind obj.a to variable x)
/// - Default values: `{a = 1}` (use 1 if obj.a is undefined)
/// - Rest: `{...rest}` (collect remaining properties)
#[derive(Debug, Clone)]
pub struct ObjectPattern {
    pub properties: Vec<ObjectPatternProperty>,
    pub span: Span,
}

/// Object pattern property - either a regular property or a rest element
#[derive(Debug, Clone)]
pub enum ObjectPatternProperty {
    Property(Property),
    RestElement(RestElement),
}

impl ObjectPatternProperty {
    pub fn span(&self) -> Span {
        match self {
            ObjectPatternProperty::Property(p) => p.span,
            ObjectPatternProperty::RestElement(r) => r.span,
        }
    }
}

/// Array pattern for destructuring: `[a, b]`, `[a, , b]`, `[...rest]`
///
/// Used as the left-hand side in destructuring assignments and declarations:
/// - `const [a, b] = arr`
/// - `([a, b] = arr)`
///
/// Elements can include:
/// - Identifiers: `[a, b]`
/// - Nested patterns: `[{a}, [b]]`
/// - Default values: `[a = 1]`
/// - Rest: `[...rest]`
/// - Holes: `[a, , b]` (skip element at index 1)
#[derive(Debug, Clone)]
pub struct ArrayPattern {
    /// Elements are Option to support holes like `[a, , b]`
    pub elements: Vec<Option<Expression>>,
    pub span: Span,
}

/// Assignment pattern for default values in destructuring: `a = 1`
///
/// Used when a destructured variable has a default value:
/// - `const {a = 1} = obj`
/// - `const [a = 1] = arr`
/// - `function foo({a = 1}) {}`
///
/// The left side is the binding pattern, the right side is the default value.
#[derive(Debug, Clone)]
pub struct AssignmentPattern {
    /// The binding (identifier or nested pattern)
    pub left: Box<Expression>,
    /// The default value expression
    pub right: Box<Expression>,
    pub span: Span,
}

/// Rest element in destructuring: `...rest`
///
/// Collects remaining elements in array or object destructuring:
/// - `const [a, ...rest] = arr` (rest gets remaining array elements)
/// - `const {a, ...rest} = obj` (rest gets remaining properties)
#[derive(Debug, Clone)]
pub struct RestElement {
    /// The binding for the rest (typically an identifier)
    pub argument: Box<Expression>,
    pub span: Span,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VariableDeclarationKind {
    Const = 0,
    Let = 1,
    Var = 2,
}

impl VariableDeclarationKind {
    /// Returns the string representation of the variable declaration kind
    #[inline]
    pub const fn as_str(self) -> &'static str {
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
    /// The binding pattern (Identifier, ArrayPattern, or ObjectPattern)
    pub id: Expression,
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
    /// Primitive type keywords (number, string, boolean, etc.)
    Keyword(TSKeywordType),
    /// Literal types (template literals, string literals, number literals, etc.)
    Literal(TSLiteralType),
    // TODO: Union, Intersection, Array, Tuple, Function, Object literal, etc.
}

impl TSType {
    #[inline]
    pub fn span(&self) -> Span {
        match self {
            TSType::Keyword(kw) => kw.span,
            TSType::Literal(lit) => lit.span(),
        }
    }
}

/// TypeScript primitive type keyword
///
/// Compact representation using a kind enum + span.
/// Memory: 1 byte (kind) + padding + 8 bytes (span) = 12 bytes total
#[derive(Debug, Clone, Copy)]
pub struct TSKeywordType {
    pub kind: TSKeywordKind,
    pub span: Span,
}

impl TSKeywordType {
    #[inline]
    pub const fn new(kind: TSKeywordKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Enumeration of TypeScript primitive type keywords
///
/// Compact representation (1 byte) for all built-in type keywords.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TSKeywordKind {
    Number = 0,
    String = 1,
    Boolean = 2,
    Any = 3,
    Void = 4,
    Undefined = 5,
    Null = 6,
    Never = 7,
    Unknown = 8,
    Object = 9,
    Symbol = 10,
    BigInt = 11,
}

impl TSKeywordKind {
    /// Returns the string representation of this type keyword
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            TSKeywordKind::Number => "number",
            TSKeywordKind::String => "string",
            TSKeywordKind::Boolean => "boolean",
            TSKeywordKind::Any => "any",
            TSKeywordKind::Void => "void",
            TSKeywordKind::Undefined => "undefined",
            TSKeywordKind::Null => "null",
            TSKeywordKind::Never => "never",
            TSKeywordKind::Unknown => "unknown",
            TSKeywordKind::Object => "object",
            TSKeywordKind::Symbol => "symbol",
            TSKeywordKind::BigInt => "bigint",
        }
    }

    /// Returns the AST node type name for JSON serialization
    #[inline]
    pub const fn node_type_name(self) -> &'static str {
        match self {
            TSKeywordKind::Number => "TSNumberKeyword",
            TSKeywordKind::String => "TSStringKeyword",
            TSKeywordKind::Boolean => "TSBooleanKeyword",
            TSKeywordKind::Any => "TSAnyKeyword",
            TSKeywordKind::Void => "TSVoidKeyword",
            TSKeywordKind::Undefined => "TSUndefinedKeyword",
            TSKeywordKind::Null => "TSNullKeyword",
            TSKeywordKind::Never => "TSNeverKeyword",
            TSKeywordKind::Unknown => "TSUnknownKeyword",
            TSKeywordKind::Object => "TSObjectKeyword",
            TSKeywordKind::Symbol => "TSSymbolKeyword",
            TSKeywordKind::BigInt => "TSBigIntKeyword",
        }
    }

    /// Convert from lexer KeywordKind to AST TSKeywordKind
    /// Returns None for non-type keywords (const, let, var, true, false)
    #[inline]
    pub fn from_lexer_keyword(kw: crate::lexer::KeywordKind) -> Option<Self> {
        use crate::lexer::KeywordKind;
        match kw {
            KeywordKind::Number => Some(TSKeywordKind::Number),
            KeywordKind::String => Some(TSKeywordKind::String),
            KeywordKind::Boolean => Some(TSKeywordKind::Boolean),
            KeywordKind::Any => Some(TSKeywordKind::Any),
            KeywordKind::Void => Some(TSKeywordKind::Void),
            KeywordKind::Undefined => Some(TSKeywordKind::Undefined),
            KeywordKind::Null => Some(TSKeywordKind::Null),
            KeywordKind::Never => Some(TSKeywordKind::Never),
            KeywordKind::Unknown => Some(TSKeywordKind::Unknown),
            KeywordKind::Object => Some(TSKeywordKind::Object),
            KeywordKind::Symbol => Some(TSKeywordKind::Symbol),
            KeywordKind::Bigint => Some(TSKeywordKind::BigInt),
            // Non-type keywords
            KeywordKind::Const
            | KeywordKind::Let
            | KeywordKind::Var
            | KeywordKind::True
            | KeywordKind::False
            | KeywordKind::New
            | KeywordKind::Instanceof
            | KeywordKind::In
            | KeywordKind::Return
            | KeywordKind::Function
            | KeywordKind::Class
            | KeywordKind::Typeof
            | KeywordKind::Delete
            | KeywordKind::Async
            | KeywordKind::Await
            | KeywordKind::Super
            | KeywordKind::Extends => None,
        }
    }
}

/// TypeScript type alias declaration: `type X = T`
///
/// Represents a type alias that creates a new name for an existing type.
/// Supports template literal types: `type X = \`hello\``
#[derive(Debug, Clone)]
pub struct TSTypeAliasDeclaration {
    pub id: Identifier,
    pub type_annotation: TSType,
    // TODO: Add type_parameters for generic type aliases: `type X<T> = T[]`
    pub span: Span,
}

/// TypeScript literal type: wraps a literal value as a type
///
/// Used for template literal types: `type X = \`hello\``
/// Also supports string, number, boolean, null, undefined literals as types.
#[derive(Debug, Clone)]
pub enum TSLiteralType {
    TemplateLiteral(TemplateLiteralType),
    // TODO: Add String, Number, Boolean, Null, Undefined for other literal types
}

impl TSLiteralType {
    #[inline]
    pub fn span(&self) -> Span {
        match self {
            TSLiteralType::TemplateLiteral(t) => t.span,
        }
    }
}

/// Template literal used as a type: `\`hello ${string} world\``
///
/// Similar to TemplateLiteral but interpolations contain types, not expressions.
/// Used in TypeScript template literal types.
#[derive(Debug, Clone)]
pub struct TemplateLiteralType {
    pub quasis: Vec<TemplateElement>,
    pub types: Vec<TSType>,
    pub span: Span,
}
