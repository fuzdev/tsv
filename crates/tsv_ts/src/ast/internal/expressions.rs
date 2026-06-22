//! Expression nodes
//!
//! Contains the Expression enum and all expression types including
//! operators, literals, function expressions, and TypeScript type assertions.

use tsv_lang::Span;

use super::{
    ArrayPattern, AssignmentPattern, BlockStatement, ClassExpression, Identifier, Literal,
    ObjectPattern, PrivateIdentifier, RestElement, TSParameterProperty, TSType, TSTypeAnnotation,
    TSTypeParameterDeclaration, TSTypeParameterInstantiation,
};

/// Expression node type
#[derive(Debug, Clone)]
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
    ThisExpression(ThisExpression),
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
    // TypeScript parameter property: constructor(public x)
    TSParameterProperty(TSParameterProperty),
    // Dynamic import: import('...')
    ImportExpression(ImportExpression),
    // Meta property: import.meta, new.target
    MetaProperty(MetaProperty),
    // JSDoc type cast: `/** @type {T} */ (inner)` — internal-only, never serialized
    JsdocCast(JsdocCast),
}

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::Literal(lit) => lit.span,
            Expression::Identifier(id) => id.span,
            Expression::PrivateIdentifier(pid) => pid.span,
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
            Expression::ClassExpression(class_expr) => class_expr.span,
            Expression::SpreadElement(spread) => spread.span,
            Expression::TemplateLiteral(template) => template.span,
            Expression::TaggedTemplateExpression(tagged) => tagged.span,
            Expression::AwaitExpression(await_expr) => await_expr.span,
            Expression::YieldExpression(yield_expr) => yield_expr.span,
            Expression::SequenceExpression(seq) => seq.span,
            Expression::RegexLiteral(regex) => regex.span,
            Expression::ThisExpression(t) => t.span,
            Expression::Super(s) => s.span,
            Expression::AssignmentExpression(assign) => assign.span,
            Expression::ObjectPattern(obj) => obj.span,
            Expression::ArrayPattern(arr) => arr.span,
            Expression::AssignmentPattern(assign) => assign.span,
            Expression::RestElement(rest) => rest.span,
            Expression::TSTypeAssertion(type_assert) => type_assert.span,
            Expression::TSAsExpression(as_expr) => as_expr.span,
            Expression::TSSatisfiesExpression(sat_expr) => sat_expr.span,
            Expression::TSInstantiationExpression(inst) => inst.span,
            Expression::TSNonNullExpression(non_null) => non_null.span,
            Expression::TSParameterProperty(param_prop) => param_prop.span,
            Expression::ImportExpression(import) => import.span,
            Expression::MetaProperty(meta) => meta.span,
            Expression::JsdocCast(cast) => cast.span,
        }
    }

    /// Check if this expression is a chain root that needs ChainExpression wrapping.
    ///
    /// Returns true if this is a MemberExpression/CallExpression (or TSNonNullExpression
    /// wrapping one) that contains at least one `optional: true` node anywhere in
    /// the callee/object chain.
    ///
    /// The walk stops at a **parenthesized** object/callee/operand: source parens
    /// terminate an optional chain (`(a?.b).c` — the `.c` is *not* part of the
    /// chain; `(a?.b)!.c` — the chain seals at `a?.b`, with `!` and `.c` outside),
    /// so the optionals inside the parens don't extend this node's chain. The
    /// grouping parens are stripped, so the only signal is the span gap — the
    /// parent's span starts before the child's (it covers the `(`). For the
    /// non-null arm that means a parenthesized inner chain (`(a?.b)!`) seals here.
    /// Without this, `(a?.b).c` / `(a?.b)!.c` would wrap the whole thing in
    /// `ChainExpression`, diverging from acorn and dropping the
    /// semantically-required parens.
    pub fn has_optional_in_chain(&self) -> bool {
        match self {
            Expression::MemberExpression(m) => {
                m.optional
                    || (m.span.start >= m.object.span().start && m.object.has_optional_in_chain())
            }
            Expression::CallExpression(c) => {
                c.optional
                    || (c.span.start >= c.callee.span().start && c.callee.has_optional_in_chain())
            }
            Expression::TSNonNullExpression(n) => {
                n.span.start >= n.expression.span().start && n.expression.has_optional_in_chain()
            }
            _ => false,
        }
    }
}

/// JSDoc type cast: `/** @type {T} */ (inner)`.
///
/// Internal-only wrapper recording that the author wrote a parenthesized
/// expression immediately preceded by a `@type`/`@satisfies` block comment — a
/// TypeScript type **cast** whose parentheses are semantically required (without
/// them the assertion is dropped). Ordinary grouping parens are discarded at
/// parse time; cast parens are preserved here so the printer can re-emit them.
///
/// `span` covers the parentheses (`(`…`)`); `inner` keeps its own paren-free
/// span. **Never serialized** — the convert layer unwraps to `inner`, so the
/// public AST stays paren-free, matching acorn/Svelte (which carry no
/// `ParenthesizedExpression`). Distinct from a bare grouping paren, the wrapper
/// is opaque to layout heuristics (expand-last etc.), mirroring how acorn's
/// `ParenthesizedExpression` hides the inner type in prettier-plugin-svelte.
#[derive(Debug, Clone)]
pub struct JsdocCast {
    pub inner: Box<Expression>,
    pub span: Span,
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

    /// Check if this is a logical operator (&&, ||, ??)
    #[inline]
    pub const fn is_logical(self) -> bool {
        matches!(
            self,
            BinaryOperator::AmpersandAmpersand
                | BinaryOperator::PipePipe
                | BinaryOperator::QuestionQuestion
        )
    }

    /// Check if this is a bitwise operator (|, ^, &, <<, >>, >>>)
    #[inline]
    pub const fn is_bitwise(self) -> bool {
        matches!(
            self,
            BinaryOperator::Pipe
                | BinaryOperator::Caret
                | BinaryOperator::Ampersand
                | BinaryOperator::LeftShift
                | BinaryOperator::RightShift
                | BinaryOperator::UnsignedRightShift
        )
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

/// Call expression: `foo()`, `obj.method(arg1, arg2)`, `fn<T>()`
#[derive(Debug, Clone)]
pub struct CallExpression {
    pub callee: Box<Expression>,
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub arguments: Vec<Expression>,
    pub optional: bool, // true for `foo?.()` (optional chaining)
    pub span: Span,
}

/// New expression: `new Date()`, `new Map()`
///
/// Constructor call with the `new` keyword. The callee is typically an
/// identifier or member expression, and arguments are optional.
/// Type arguments like `new Map<K, V>()` are stored in `type_arguments`.
#[derive(Debug, Clone)]
pub struct NewExpression {
    pub callee: Box<Expression>,
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub arguments: Vec<Expression>,
    pub span: Span,
}

/// Dynamic import expression: `import('module')` or `import('module', options)`
#[derive(Debug, Clone)]
pub struct ImportExpression {
    pub source: Box<Expression>,
    /// Optional second argument for import attributes: `{with: {type: 'json'}}`
    pub options: Option<Box<Expression>>,
    pub span: Span,
}

/// Meta property: `import.meta`, `new.target`
#[derive(Debug, Clone)]
pub struct MetaProperty {
    /// The keyword: "import" or "new"
    pub meta: Identifier,
    /// The property: "meta" or "target"
    pub property: Identifier,
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
#[derive(Debug, Clone)]
pub struct ArrowFunctionExpression {
    /// Type parameters (TypeScript generics): `<T>() => ...`
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Vec<Expression>,
    pub body: ArrowFunctionBody,
    /// Return type annotation (TypeScript): (): number => ...
    pub return_type: Option<TSTypeAnnotation>,
    /// Whether this is an async arrow function: `async () => ...`
    pub r#async: bool,
    /// Position of opening paren for params, if parenthesized.
    /// `Some(pos)` for `(x) => x` or `() => x`, `None` for `x => x`
    pub params_start: Option<u32>,
    pub span: Span,
}

/// Arrow function body - either an expression or a block statement
#[derive(Debug, Clone)]
pub enum ArrowFunctionBody {
    /// Expression body: `() => expr`
    Expression(Box<Expression>),
    /// Block body: `() => { stmts }`
    BlockStatement(BlockStatement),
}

impl ArrowFunctionBody {
    pub fn span(&self) -> Span {
        match self {
            ArrowFunctionBody::Expression(expr) => expr.span(),
            ArrowFunctionBody::BlockStatement(block) => block.span,
        }
    }

    /// Returns true if this is an expression body (not a block)
    pub fn is_expression(&self) -> bool {
        matches!(self, ArrowFunctionBody::Expression(_))
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
    /// Type parameters (TypeScript generics): `function<T>() {}`
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Vec<Expression>,
    /// Return type annotation (e.g., `: number` in `function fn(): number {}`)
    pub return_type: Option<TSTypeAnnotation>,
    /// Function body (block statement with statements)
    pub body: BlockStatement,
    /// Whether this is a generator function (`function*`)
    pub generator: bool,
    /// Whether this is an async function (`async function`)
    pub r#async: bool,
    /// Position of opening paren for params (for comment detection)
    pub params_start: u32,
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
/// When the tag has type arguments (e.g., `tag<T>\`content\``), they are stored
/// separately rather than wrapping the tag in `TSInstantiationExpression`.
#[derive(Debug, Clone)]
pub struct TaggedTemplateExpression {
    pub tag: Box<Expression>,
    pub type_arguments: Option<TSTypeParameterInstantiation>,
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

/// Yield expression: `yield value` or `yield* iterable`
///
/// Used in generator functions to produce values.
/// - `yield` with no argument yields undefined
/// - `yield value` yields the given value
/// - `yield* iterable` delegates to another generator/iterable
#[derive(Debug, Clone)]
pub struct YieldExpression {
    /// The value to yield (None for `yield` with no argument)
    pub argument: Option<Box<Expression>>,
    /// Whether this is a delegating yield: `yield*`
    pub delegate: bool,
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

/// This expression: `this`
#[derive(Debug, Clone)]
pub struct ThisExpression {
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

/// TypeScript angle-bracket type assertion: `<Type>expr`
///
/// Old-style type assertion syntax. Equivalent to `expr as Type` but
/// incompatible with JSX (looks like a JSX element).
///
/// Example: `<string>someValue`, `<T>a`
#[derive(Debug, Clone)]
pub struct TSTypeAssertion {
    /// The target type
    pub type_annotation: Box<TSType>,
    /// The expression being type-asserted
    pub expression: Box<Expression>,
    pub span: Span,
}

/// TypeScript `as` type assertion: `expr as Type` or `expr as const`
///
/// Type assertion that tells the compiler to treat an expression as a specific type.
/// Unlike angle-bracket syntax (`<Type>expr`), this works in JSX/TSX.
///
/// Note: `as const` is represented as a type reference with name "const".
#[derive(Debug, Clone)]
pub struct TSAsExpression {
    /// The expression being type-asserted
    pub expression: Box<Expression>,
    /// The target type
    pub type_annotation: Box<TSType>,
    pub span: Span,
}

/// TypeScript `satisfies` expression: `expr satisfies Type`
///
/// Checks that an expression conforms to a type while preserving its inferred type.
/// Unlike `as`, this doesn't widen the type - the expression keeps its specific type.
///
/// Example: `{ a: 1 } satisfies Record<string, number>` keeps type `{ a: number }`
/// but verifies it's compatible with `Record<string, number>`.
#[derive(Debug, Clone)]
pub struct TSSatisfiesExpression {
    /// The expression being checked
    pub expression: Box<Expression>,
    /// The type to satisfy
    pub type_annotation: Box<TSType>,
    pub span: Span,
}

/// TypeScript instantiation expression: `f<T>`, `SomeClass<number>`
///
/// Instantiates a generic value with specific type arguments without calling it.
/// This is different from CallExpression with type arguments (`f<T>()`) - this
/// just provides type arguments to a generic function/class reference.
///
/// Example: `const boundF = f<number>;` gives `f` with type parameter bound to `number`.
#[derive(Debug, Clone)]
pub struct TSInstantiationExpression {
    /// The expression being instantiated
    pub expression: Box<Expression>,
    /// The type arguments: <T, U>
    pub type_arguments: TSTypeParameterInstantiation,
    pub span: Span,
}

/// TypeScript non-null assertion expression: `expr!`
///
/// Asserts that an expression is not null or undefined.
/// This is a compile-time assertion that has no runtime effect.
///
/// Example: `document.getElementById("app")!`
#[derive(Debug, Clone)]
pub struct TSNonNullExpression {
    /// The expression being asserted non-null
    pub expression: Box<Expression>,
    pub span: Span,
}

impl TSNonNullExpression {
    /// True when this non-null assertion seals a **parenthesized** optional chain
    /// (`(a?.b)!` — the `!` outside the source parens). The grouping parens are
    /// stripped, so the only signal is the span gap: this node's span starts before
    /// its inner expression's (covering the `(`) and the inner is an optional chain.
    /// Such a chain is sealed — a trailing access reached through it (`(a?.b)!.c`),
    /// or an always-required-parens position (`` (a?.b)!`x` ``, `new (a?.b)!()`),
    /// must keep the parens. Complements [`Expression::has_optional_in_chain`]'s
    /// non-null arm, which detects the opposite (`>=` — the chain *continues* through
    /// the `!`, no sealing parens).
    pub fn seals_optional_chain(&self) -> bool {
        self.span.start < self.expression.span().start && self.expression.has_optional_in_chain()
    }
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

    /// Returns the operator string with a leading space (e.g., `" ="`, `" +="`)
    /// for use in assignment layout formatting.
    #[inline]
    pub const fn as_str_with_leading_space(self) -> &'static str {
        match self {
            AssignmentOperator::Assign => " =",
            AssignmentOperator::AddAssign => " +=",
            AssignmentOperator::SubtractAssign => " -=",
            AssignmentOperator::MultiplyAssign => " *=",
            AssignmentOperator::DivideAssign => " /=",
            AssignmentOperator::RemainderAssign => " %=",
            AssignmentOperator::ExponentiateAssign => " **=",
            AssignmentOperator::LeftShiftAssign => " <<=",
            AssignmentOperator::RightShiftAssign => " >>=",
            AssignmentOperator::UnsignedRightShiftAssign => " >>>=",
            AssignmentOperator::BitwiseOrAssign => " |=",
            AssignmentOperator::BitwiseXorAssign => " ^=",
            AssignmentOperator::BitwiseAndAssign => " &=",
            AssignmentOperator::LogicalOrAssign => " ||=",
            AssignmentOperator::LogicalAndAssign => " &&=",
            AssignmentOperator::NullishAssign => " ??=",
        }
    }
}
