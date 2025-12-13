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

impl Statement {
    pub fn span(&self) -> Span {
        match self {
            Statement::ExpressionStatement(stmt) => stmt.span,
            Statement::VariableDeclaration(decl) => decl.span,
            Statement::TSTypeAliasDeclaration(decl) => decl.span,
            Statement::TSInterfaceDeclaration(decl) => decl.span,
            Statement::TSDeclareFunction(decl) => decl.span,
            Statement::TSEnumDeclaration(decl) => decl.span,
            Statement::TSModuleDeclaration(decl) => decl.span,
            Statement::ReturnStatement(stmt) => stmt.span,
            Statement::BlockStatement(block) => block.span,
            Statement::FunctionDeclaration(decl) => decl.span,
            Statement::ClassDeclaration(decl) => decl.span,
            Statement::ExportNamedDeclaration(decl) => decl.span,
            Statement::ExportDefaultDeclaration(decl) => decl.span,
            Statement::ExportAllDeclaration(decl) => decl.span,
            Statement::TSExportAssignment(decl) => decl.span,
            Statement::ImportDeclaration(decl) => decl.span,
            Statement::TSImportEqualsDeclaration(decl) => decl.span,
            // Control flow statements
            Statement::IfStatement(stmt) => stmt.span,
            Statement::ForStatement(stmt) => stmt.span,
            Statement::ForInStatement(stmt) => stmt.span,
            Statement::ForOfStatement(stmt) => stmt.span,
            Statement::WhileStatement(stmt) => stmt.span,
            Statement::DoWhileStatement(stmt) => stmt.span,
            Statement::SwitchStatement(stmt) => stmt.span,
            Statement::TryStatement(stmt) => stmt.span,
            Statement::ThrowStatement(stmt) => stmt.span,
            Statement::BreakStatement(stmt) => stmt.span,
            Statement::ContinueStatement(stmt) => stmt.span,
            Statement::LabeledStatement(stmt) => stmt.span,
            Statement::EmptyStatement(stmt) => stmt.span,
        }
    }
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

/// Export named declaration: `export const x = 1;`, `export { x }`, `export { x } from "y"`
#[derive(Debug, Clone)]
pub struct ExportNamedDeclaration {
    /// The declaration being exported (VariableDeclaration, FunctionDeclaration, ClassDeclaration)
    /// None when using specifiers
    pub declaration: Option<Box<Statement>>,
    /// Export specifiers: `export { a, b as c }`
    pub specifiers: Vec<ExportSpecifier>,
    /// Re-export source: `export { x } from "y"` or None for local exports
    pub source: Option<Literal>,
    /// Export kind: "value" for regular exports, "type" for type-only exports
    pub export_kind: ExportKind,
    pub span: Span,
}

/// Export kind for TypeScript type-only exports
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    /// Regular value export: `export { x }`
    Value,
    /// Type-only export: `export type { X }`
    Type,
}

/// Export default declaration: `export default x`, `export default function() {}`
#[derive(Debug, Clone)]
pub struct ExportDefaultDeclaration {
    /// The expression or declaration being exported as default
    pub declaration: ExportDefaultValue,
    pub span: Span,
}

/// Function declaration that may be ambient (TSDeclareFunction) or regular (FunctionDeclaration)
///
/// Used when parsing export function declarations which can be either
/// in ambient context (declare module) or regular context.
#[derive(Debug, Clone)]
pub enum ExportFunctionDeclaration {
    Declaration(FunctionDeclaration),
    Declare(TSDeclareFunction),
}

/// Value of export default - can be expression or declaration
#[derive(Debug, Clone)]
pub enum ExportDefaultValue {
    Expression(Expression),
    FunctionDeclaration(Box<FunctionDeclaration>),
    /// For ambient function declarations (no body)
    TSDeclareFunction(Box<TSDeclareFunction>),
    ClassDeclaration(Box<ClassDeclaration>),
}

/// Export all declaration: `export * from "y"` or `export * as ns from "y"`
/// Also handles type-only: `export type * from "y"`
#[derive(Debug, Clone)]
pub struct ExportAllDeclaration {
    /// For `export * as ns from "y"`, the namespace binding name
    pub exported: Option<Identifier>,
    /// Module source
    pub source: Literal,
    /// Export kind: "value" or "type" (for `export type * from`)
    pub export_kind: ExportKind,
    pub span: Span,
}

/// TypeScript export assignment: `export = value;`
/// CommonJS-style export for TypeScript modules
#[derive(Debug, Clone)]
pub struct TSExportAssignment {
    pub expression: Expression,
    pub span: Span,
}

/// Export specifier: `export { x }` or `export { x as y }`
#[derive(Debug, Clone)]
pub struct ExportSpecifier {
    /// Local name (what's exported from this module)
    pub local: Identifier,
    /// Exported name (what it's called externally, may be same as local)
    pub exported: Identifier,
    pub span: Span,
}

/// Import kind: value import or type-only import
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportKind {
    #[default]
    Value,
    Type,
}

/// Import declaration: `import x from "y"`, `import { a, b } from "y"`, etc.
#[derive(Debug, Clone)]
pub struct ImportDeclaration {
    /// Import specifiers (default, named, or namespace)
    pub specifiers: Vec<ImportSpecifier>,
    /// Module source (string literal)
    pub source: Literal,
    /// Import attributes: `import x from "y" with { type: "json" }`
    pub attributes: Vec<ImportAttribute>,
    /// Import kind: "value" or "type" (for `import type { ... }`)
    pub import_kind: ImportKind,
    pub span: Span,
}

/// Import specifier variants
#[derive(Debug, Clone)]
pub enum ImportSpecifier {
    /// Default import: `import x from "y"`
    Default(ImportDefaultSpecifier),
    /// Named import: `import { a, b as c } from "y"`
    Named(ImportNamedSpecifier),
    /// Namespace import: `import * as ns from "y"`
    Namespace(ImportNamespaceSpecifier),
}

/// Default import specifier: `import x from "y"`
#[derive(Debug, Clone)]
pub struct ImportDefaultSpecifier {
    /// Local binding name
    pub local: Identifier,
    pub span: Span,
}

/// Named import specifier: `import { a } from "y"` or `import { a as b } from "y"`
#[derive(Debug, Clone)]
pub struct ImportNamedSpecifier {
    /// Imported name (the name in the module)
    pub imported: Identifier,
    /// Local binding name (may be same as imported, or different for `as` renames)
    pub local: Identifier,
    /// Import kind for inline type modifier: `import { type A, B } from "y"`
    pub import_kind: ImportKind,
    pub span: Span,
}

/// Namespace import specifier: `import * as ns from "y"`
#[derive(Debug, Clone)]
pub struct ImportNamespaceSpecifier {
    /// Local binding name
    pub local: Identifier,
    pub span: Span,
}

/// Import attribute: `{ type: "json" }`
#[derive(Debug, Clone)]
pub struct ImportAttribute {
    pub key: Identifier,
    pub value: Literal,
    pub span: Span,
}

/// TypeScript import equals declaration: `import x = require("y")` or `import x = A.B`
#[derive(Debug, Clone)]
pub struct TSImportEqualsDeclaration {
    /// The local binding name
    pub id: Identifier,
    /// The module reference (either external module or entity name)
    pub module_reference: TSModuleReference,
    /// Import kind: "value" or "type"
    pub import_kind: ImportKind,
    /// Whether this is an export: `export import x = require("y")`
    pub is_export: bool,
    pub span: Span,
}

/// Module reference: either external module reference or entity name
#[derive(Debug, Clone)]
pub enum TSModuleReference {
    /// `require("module")`
    ExternalModuleReference(TSExternalModuleReference),
    /// `A.B.C` (entity name)
    EntityName(TSEntityName),
}

/// External module reference: `require("module")`
#[derive(Debug, Clone)]
pub struct TSExternalModuleReference {
    /// The module specifier (string literal)
    pub expression: Literal,
    pub span: Span,
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
    // TypeScript parameter property: constructor(public x)
    TSParameterProperty(TSParameterProperty),
    // Dynamic import: import('...')
    ImportExpression(ImportExpression),
    // Meta property: import.meta, new.target
    MetaProperty(MetaProperty),
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

/// Dynamic import expression: `import('module')`
#[derive(Debug, Clone)]
pub struct ImportExpression {
    pub source: Box<Expression>,
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
///
/// For now, we only support expression bodies since block statements
/// require additional parsing infrastructure.
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
/// For regular declarations, the id (function name) is required.
/// For `export default function() {}`, the name is optional.
/// Declarations are hoisted and can be called before they appear in source.
#[derive(Debug, Clone)]
pub struct FunctionDeclaration {
    /// Function name (required for declarations, optional for export default)
    pub id: Option<Identifier>,
    /// Type parameters (TypeScript generics): `function fn<T>() {}`
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

// ============================================================================
// Control Flow Statements
// ============================================================================

/// If statement: `if (test) consequent` or `if (test) consequent else alternate`
#[derive(Debug, Clone)]
pub struct IfStatement {
    pub test: Expression,
    pub consequent: Box<Statement>,
    pub alternate: Option<Box<Statement>>,
    pub span: Span,
}

/// For statement: `for (init; test; update) body`
#[derive(Debug, Clone)]
pub struct ForStatement {
    /// Initialization: variable declaration or expression (or None)
    pub init: Option<ForInit>,
    /// Test condition (or None for infinite loop)
    pub test: Option<Expression>,
    /// Update expression (or None)
    pub update: Option<Expression>,
    pub body: Box<Statement>,
    pub span: Span,
}

/// For statement initialization - either a variable declaration or expression
#[derive(Debug, Clone)]
pub enum ForInit {
    VariableDeclaration(VariableDeclaration),
    Expression(Expression),
}

/// For-in statement: `for (left in right) body`
#[derive(Debug, Clone)]
pub struct ForInStatement {
    /// Left side: variable declaration or expression pattern
    pub left: ForInOfLeft,
    pub right: Expression,
    pub body: Box<Statement>,
    pub span: Span,
}

/// For-of statement: `for (left of right) body`
#[derive(Debug, Clone)]
pub struct ForOfStatement {
    /// Left side: variable declaration or expression pattern
    pub left: ForInOfLeft,
    pub right: Expression,
    /// Whether this is `for await (... of ...)`
    pub r#await: bool,
    pub body: Box<Statement>,
    pub span: Span,
}

/// Left side of for-in/for-of: either a variable declaration or expression pattern
#[derive(Debug, Clone)]
pub enum ForInOfLeft {
    VariableDeclaration(VariableDeclaration),
    Pattern(Expression),
}

/// While statement: `while (test) body`
#[derive(Debug, Clone)]
pub struct WhileStatement {
    pub test: Expression,
    pub body: Box<Statement>,
    pub span: Span,
}

/// Do-while statement: `do body while (test)`
#[derive(Debug, Clone)]
pub struct DoWhileStatement {
    pub body: Box<Statement>,
    pub test: Expression,
    pub span: Span,
}

/// Switch statement: `switch (discriminant) { cases }`
#[derive(Debug, Clone)]
pub struct SwitchStatement {
    pub discriminant: Expression,
    pub cases: Vec<SwitchCase>,
    pub span: Span,
}

/// Switch case: `case test: consequent` or `default: consequent`
#[derive(Debug, Clone)]
pub struct SwitchCase {
    /// Test expression, or None for `default:`
    pub test: Option<Expression>,
    pub consequent: Vec<Statement>,
    pub span: Span,
}

/// Try statement: `try { block } catch (param) { handler } finally { finalizer }`
#[derive(Debug, Clone)]
pub struct TryStatement {
    pub block: BlockStatement,
    pub handler: Option<CatchClause>,
    pub finalizer: Option<BlockStatement>,
    pub span: Span,
}

/// Catch clause: `catch (param) { body }`
#[derive(Debug, Clone)]
pub struct CatchClause {
    /// Catch parameter, or None for `catch { }` (optional catch binding)
    pub param: Option<Expression>,
    pub body: BlockStatement,
    pub span: Span,
}

/// Throw statement: `throw argument`
#[derive(Debug, Clone)]
pub struct ThrowStatement {
    pub argument: Expression,
    pub span: Span,
}

/// Break statement: `break` or `break label`
#[derive(Debug, Clone)]
pub struct BreakStatement {
    pub label: Option<Identifier>,
    pub span: Span,
}

/// Continue statement: `continue` or `continue label`
#[derive(Debug, Clone)]
pub struct ContinueStatement {
    pub label: Option<Identifier>,
    pub span: Span,
}

/// Labeled statement: `label: statement`
#[derive(Debug, Clone)]
pub struct LabeledStatement {
    pub label: Identifier,
    pub body: Box<Statement>,
    pub span: Span,
}

/// Empty statement: `;`
#[derive(Debug, Clone)]
pub struct EmptyStatement {
    pub span: Span,
}

// ============================================================================

/// Class declaration: `class Foo { ... }` or `class Foo extends Bar { ... }`
///
/// Represents a class declaration with optional superclass.
/// For `export default class {}`, the name is optional.
#[derive(Debug, Clone)]
pub struct ClassDeclaration {
    /// Decorators applied to this class
    pub decorators: Vec<Decorator>,
    /// Class name (required for declarations, optional for export default)
    pub id: Option<Identifier>,
    /// Optional superclass expression (for `extends`)
    pub super_class: Option<Box<Expression>>,
    /// Type arguments for superclass (e.g., `<T>` in `extends Base<T>`)
    pub super_type_parameters: Option<TSTypeParameterInstantiation>,
    /// Implements clause for declare class: `implements Foo, Bar`
    pub implements: Vec<TSInterfaceHeritage>,
    /// Class body containing methods and properties
    pub body: ClassBody,
    /// Whether this is a declare class (ambient declaration)
    pub declare: bool,
    /// Whether this is an abstract class
    pub r#abstract: bool,
    /// Type parameters (e.g., `<T>` in `class Foo<T>`)
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub span: Span,
}

/// Class expression: `class { }` or `class Foo<T> extends Bar { }`
///
/// Same as ClassDeclaration but used in expression position.
/// The name is always optional.
#[derive(Debug, Clone)]
pub struct ClassExpression {
    /// Decorators applied to this class
    pub decorators: Vec<Decorator>,
    /// Class name (always optional for expressions)
    pub id: Option<Identifier>,
    /// Optional superclass expression (for `extends`)
    pub super_class: Option<Box<Expression>>,
    /// Type arguments for superclass (e.g., `<T>` in `extends Base<T>`)
    pub super_type_parameters: Option<TSTypeParameterInstantiation>,
    /// Implements clause: `implements Foo, Bar`
    pub implements: Vec<TSInterfaceHeritage>,
    /// Class body containing methods and properties
    pub body: ClassBody,
    /// Whether this is an abstract class
    pub r#abstract: bool,
    /// Type parameters (e.g., `<T>` in `class Foo<T>`)
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub span: Span,
}

/// Class body: `{ constructor() {} method() {} prop = value; }`
///
/// Contains the methods and properties of a class.
#[derive(Debug, Clone)]
pub struct ClassBody {
    pub body: Vec<ClassMember>,
    pub span: Span,
}

/// Class member - method definition, property definition, or static block
#[derive(Debug, Clone)]
pub enum ClassMember {
    MethodDefinition(MethodDefinition),
    PropertyDefinition(PropertyDefinition),
    StaticBlock(StaticBlock),
}

impl ClassMember {
    pub fn span(&self) -> Span {
        match self {
            ClassMember::MethodDefinition(m) => m.span,
            ClassMember::PropertyDefinition(p) => p.span,
            ClassMember::StaticBlock(s) => s.span,
        }
    }
}

/// Static initialization block in a class: `static { ... }` (ES2022)
#[derive(Debug, Clone)]
pub struct StaticBlock {
    pub body: Vec<Statement>,
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

/// Accessibility modifier for class members: public, private, protected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accessibility {
    Public,
    Private,
    Protected,
}

impl Accessibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Accessibility::Public => "public",
            Accessibility::Private => "private",
            Accessibility::Protected => "protected",
        }
    }
}

/// TypeScript parameter property in constructor: `constructor(public x: number)`
#[derive(Debug, Clone)]
pub struct TSParameterProperty {
    /// Accessibility modifier: public, private, protected
    pub accessibility: Option<Accessibility>,
    /// Whether the parameter is readonly
    pub readonly: bool,
    /// The actual parameter - can be Identifier or AssignmentPattern (with default value)
    pub parameter: Box<Expression>,
    pub span: Span,
}

/// Method definition in a class body: `method() { ... }` or `get x() { ... }`
#[derive(Debug, Clone)]
pub struct MethodDefinition {
    /// Decorators applied to this method
    pub decorators: Vec<Decorator>,
    /// Method name (key)
    pub key: Expression,
    /// Method implementation (value)
    pub value: FunctionExpression,
    /// Method kind (constructor, method, get, set)
    pub kind: MethodKind,
    /// Accessibility modifier (public, private, protected)
    pub accessibility: Option<Accessibility>,
    /// Whether this is a static method
    pub is_static: bool,
    /// Whether this method overrides a base class method
    pub r#override: bool,
    /// Whether this is an abstract method (no body)
    pub r#abstract: bool,
    /// Whether the key is computed (`[expr]()`)
    pub computed: bool,
    pub span: Span,
}

/// Modifier for class property optionality/definiteness.
///
/// These are mutually exclusive syntactically - they occupy the same position
/// after the property name (`a?: T` vs `a!: T`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PropertyModifier {
    /// No modifier (regular property)
    #[default]
    None,
    /// Optional property (`a?: string`)
    Optional,
    /// Definite assignment assertion (`a!: string`)
    Definite,
}

/// Property definition in a class body: `name = value;` or `name;`
///
/// Unlike methods, properties use `=` for initialization.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct PropertyDefinition {
    /// Decorators applied to this property
    pub decorators: Vec<Decorator>,
    /// Property name (key)
    pub key: Expression,
    /// Type annotation (e.g., `: number` in `a: number = 0;`)
    pub type_annotation: Option<TSTypeAnnotation>,
    /// Optional initial value
    pub value: Option<Expression>,
    /// Accessibility modifier (public, private, protected)
    pub accessibility: Option<Accessibility>,
    /// Whether this is a static property
    pub is_static: bool,
    /// Whether this is an abstract property
    pub r#abstract: bool,
    /// Whether this is a readonly property
    pub readonly: bool,
    /// Whether the key is computed (`[expr] = value`)
    pub computed: bool,
    /// Whether this property uses the accessor keyword (ES decorator proposal)
    pub accessor: bool,
    /// Optional/definite modifier (`?` or `!` after property name)
    pub modifier: PropertyModifier,
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
    /// BigInt literal: `1n`, `100n`, `0xffn`
    /// Value stored as string since BigInt can exceed f64 precision
    BigInt(String),
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
    pub type_annotation: Option<TSTypeAnnotation>,
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
    pub type_annotation: Option<TSTypeAnnotation>,
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
    /// Whether this is an optional parameter (e.g., `a?` in `function fn(a?: number) {}`)
    pub optional: bool,
    pub type_annotation: Option<TSTypeAnnotation>,
    pub span: Span,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VariableDeclarationKind {
    Const = 0,
    Let = 1,
    Var = 2,
    /// ES2024 Explicit Resource Management: `using resource = getResource();`
    Using = 3,
    /// ES2024 Explicit Resource Management: `await using resource = getAsyncResource();`
    AwaitUsing = 4,
}

impl VariableDeclarationKind {
    /// Returns the string representation of the variable declaration kind
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Const => "const",
            Self::Let => "let",
            Self::Var => "var",
            Self::Using => "using",
            Self::AwaitUsing => "await using",
        }
    }
}

#[derive(Debug, Clone)]
pub struct VariableDeclaration {
    pub kind: VariableDeclarationKind,
    pub declarations: Vec<VariableDeclarator>,
    /// Whether this is an ambient declaration (`declare const x: T;`)
    pub declare: bool,
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
    /// Array types (number[], string[], etc.)
    Array(TSArrayType),
    /// Union types: `A | B | C`
    Union(TSUnionType),
    /// Intersection types: `A & B & C`
    Intersection(TSIntersectionType),
    /// Type references: `SomeType`, `Array<T>`
    TypeReference(TSTypeReference),
    /// Object/type literal: `{ prop: T }`
    TypeLiteral(TSTypeLiteral),
    /// Function types: `(x: T) => U`
    Function(TSFunctionType),
    /// Tuple types: `[T, U]`
    Tuple(TSTupleType),
    /// Parenthesized types: `(T)`
    Parenthesized(TSParenthesizedType),
    /// Type predicates: `x is T` or `asserts x is T`
    TypePredicate(TSTypePredicate),
    /// Conditional types: `T extends U ? V : W`
    Conditional(TSConditionalType),
    /// Mapped types: `{ [K in keyof T]: V }`
    Mapped(TSMappedType),
    /// Type operators: `keyof T`, `unique symbol`, `readonly T`
    TypeOperator(TSTypeOperator),
    /// Import types: `import('module')` or `import('module').Foo<T>`
    Import(TSImportType),
    /// Type query: `typeof x`, `typeof Foo.bar`, `typeof import("module")`
    TypeQuery(TSTypeQuery),
    /// Indexed access types: `T[K]`, `Obj["key"]`, `T[keyof T]`
    IndexedAccess(TSIndexedAccessType),
    /// Rest type in tuples: `...T`
    Rest(TSRestType),
    /// Optional type in tuples: `T?`
    Optional(TSOptionalType),
    /// Named tuple member: `label: T` or `label?: T`
    NamedTupleMember(TSNamedTupleMember),
    /// Infer type: `infer U` (in conditional types)
    Infer(TSInferType),
}

impl TSType {
    #[inline]
    pub fn span(&self) -> Span {
        match self {
            TSType::Keyword(kw) => kw.span,
            TSType::Literal(lit) => lit.span(),
            TSType::Array(arr) => arr.span,
            TSType::Union(u) => u.span,
            TSType::Intersection(i) => i.span,
            TSType::TypeReference(r) => r.span,
            TSType::TypeLiteral(t) => t.span,
            TSType::Function(f) => f.span,
            TSType::Tuple(t) => t.span,
            TSType::Parenthesized(p) => p.span,
            TSType::TypePredicate(p) => p.span,
            TSType::Conditional(c) => c.span,
            TSType::Mapped(m) => m.span,
            TSType::TypeOperator(o) => o.span,
            TSType::Import(i) => i.span,
            TSType::TypeQuery(q) => q.span,
            TSType::IndexedAccess(i) => i.span,
            TSType::Rest(r) => r.span,
            TSType::Optional(o) => o.span,
            TSType::NamedTupleMember(n) => n.span,
            TSType::Infer(i) => i.span,
        }
    }
}

/// TypeScript array type: `number[]`, `string[]`, etc.
#[derive(Debug, Clone)]
pub struct TSArrayType {
    /// The element type of the array
    pub element_type: Box<TSType>,
    pub span: Span,
}

/// TypeScript indexed access type: `T[K]`, `Obj["key"]`, `T[keyof T]`
#[derive(Debug, Clone)]
pub struct TSIndexedAccessType {
    /// The object type being indexed
    pub object_type: Box<TSType>,
    /// The index type
    pub index_type: Box<TSType>,
    pub span: Span,
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
    True = 12,
    False = 13,
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
            TSKeywordKind::True => "true",
            TSKeywordKind::False => "false",
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
            TSKeywordKind::True => "TSLiteralType",
            TSKeywordKind::False => "TSLiteralType",
        }
    }

    /// Convert from lexer KeywordKind to AST TSKeywordKind
    /// Returns None for non-type keywords (const, let, var, etc.)
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
            KeywordKind::True => Some(TSKeywordKind::True),
            KeywordKind::False => Some(TSKeywordKind::False),
            // Non-type keywords
            KeywordKind::Const
            | KeywordKind::Let
            | KeywordKind::Var
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
            | KeywordKind::Extends
            | KeywordKind::Export
            // Control flow keywords
            | KeywordKind::If
            | KeywordKind::Else
            | KeywordKind::For
            | KeywordKind::While
            | KeywordKind::Do
            | KeywordKind::Switch
            | KeywordKind::Case
            | KeywordKind::Default
            | KeywordKind::Break
            | KeywordKind::Continue
            | KeywordKind::Try
            | KeywordKind::Catch
            | KeywordKind::Finally
            | KeywordKind::Throw
            // Module keywords
            | KeywordKind::Import
            | KeywordKind::From
            | KeywordKind::As
            | KeywordKind::Satisfies
            // Generator keywords
            | KeywordKind::Yield
            // Declaration keywords (not type keywords)
            | KeywordKind::Enum => None,
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
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub type_annotation: TSType,
    pub span: Span,
}

/// TypeScript literal type: wraps a literal value as a type
///
/// Used for template literal types: `type X = \`hello\``
/// Also supports string, number, boolean, null, undefined literals as types.
#[derive(Debug, Clone)]
pub enum TSLiteralType {
    TemplateLiteral(TemplateLiteralType),
    /// String literal type: `"hello"`, `'world'`
    String(Literal),
    /// Number literal type: `1`, `42.5`
    Number(Literal),
    /// BigInt literal type: `1n`, `100n`
    BigInt(Literal),
    /// Unary expression for negative numbers: `-1`, `-42n`
    UnaryExpression(UnaryExpression),
}

impl TSLiteralType {
    #[inline]
    pub fn span(&self) -> Span {
        match self {
            TSLiteralType::TemplateLiteral(t) => t.span,
            TSLiteralType::String(lit) => lit.span,
            TSLiteralType::Number(lit) => lit.span,
            TSLiteralType::BigInt(lit) => lit.span,
            TSLiteralType::UnaryExpression(unary) => unary.span,
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

// ============================================================================
// TypeScript Type Nodes
// ============================================================================

/// Union type: `A | B | C`
#[derive(Debug, Clone)]
pub struct TSUnionType {
    pub types: Vec<TSType>,
    pub span: Span,
}

/// Intersection type: `A & B & C`
#[derive(Debug, Clone)]
pub struct TSIntersectionType {
    pub types: Vec<TSType>,
    pub span: Span,
}

/// Type reference: `SomeType` or `Array<T>`
#[derive(Debug, Clone)]
pub struct TSTypeReference {
    pub type_name: TSEntityName,
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub span: Span,
}

/// Entity name: `Foo` or `Foo.Bar.Baz`
#[derive(Debug, Clone)]
pub enum TSEntityName {
    Identifier(Identifier),
    QualifiedName(Box<TSQualifiedName>),
}

impl TSEntityName {
    pub fn span(&self) -> Span {
        match self {
            TSEntityName::Identifier(id) => id.span,
            TSEntityName::QualifiedName(qn) => qn.span,
        }
    }
}

/// Qualified name: `Foo.Bar`
#[derive(Debug, Clone)]
pub struct TSQualifiedName {
    pub left: TSEntityName,
    pub right: Identifier,
    pub span: Span,
}

/// Type parameter instantiation: `<T, U>` (for type arguments)
#[derive(Debug, Clone)]
pub struct TSTypeParameterInstantiation {
    pub params: Vec<TSType>,
    pub span: Span,
}

/// Type parameter declaration: `<T, U>` (for declaring type parameters)
#[derive(Debug, Clone)]
pub struct TSTypeParameterDeclaration {
    pub params: Vec<TSTypeParameter>,
    pub span: Span,
}

/// Single type parameter: `T`, `T extends U`, or `T extends U = V`
/// With optional modifiers: `const T`, `in T`, `out T`, `in out T`
#[derive(Debug, Clone)]
pub struct TSTypeParameter {
    pub name: Identifier,
    pub constraint: Option<Box<TSType>>,
    pub default: Option<Box<TSType>>,
    /// `const` modifier (TS 5.0): `<const T>`
    pub is_const: bool,
    /// `in` variance modifier (TS 4.7): `<in T>`
    pub is_in: bool,
    /// `out` variance modifier (TS 4.7): `<out T>`
    pub is_out: bool,
    pub span: Span,
}

/// Type literal (object type): `{ prop: T; method(): U }`
#[derive(Debug, Clone)]
pub struct TSTypeLiteral {
    pub members: Vec<TSTypeElement>,
    pub span: Span,
}

/// Type element - member of a type literal or interface
#[derive(Debug, Clone)]
pub enum TSTypeElement {
    PropertySignature(TSPropertySignature),
    MethodSignature(TSMethodSignature),
    CallSignature(TSCallSignatureDeclaration),
    ConstructSignature(TSConstructSignatureDeclaration),
    IndexSignature(TSIndexSignature),
}

impl TSTypeElement {
    pub fn span(&self) -> Span {
        match self {
            TSTypeElement::PropertySignature(p) => p.span,
            TSTypeElement::MethodSignature(m) => m.span,
            TSTypeElement::CallSignature(c) => c.span,
            TSTypeElement::ConstructSignature(c) => c.span,
            TSTypeElement::IndexSignature(i) => i.span,
        }
    }
}

/// Property signature: `prop: T` or `prop?: T` or `readonly prop: T`
#[derive(Debug, Clone)]
pub struct TSPropertySignature {
    pub key: Expression,
    pub computed: bool,
    pub optional: bool,
    pub readonly: bool,
    pub type_annotation: Option<TSTypeAnnotation>,
    pub span: Span,
}

/// Method signature: `method(): T` or `method<T>(x: T): T`
#[derive(Debug, Clone)]
pub struct TSMethodSignature {
    pub key: Expression,
    pub computed: bool,
    pub optional: bool,
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    pub return_type: Option<TSTypeAnnotation>,
    pub span: Span,
}

/// Call signature: `(): T` or `<T>(): T` or `(x: A): T`
#[derive(Debug, Clone)]
pub struct TSCallSignatureDeclaration {
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    pub return_type: Option<TSTypeAnnotation>,
    pub span: Span,
}

/// Construct signature: `new (): T` or `new <T>(): T` or `new (x: A): T`
#[derive(Debug, Clone)]
pub struct TSConstructSignatureDeclaration {
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    pub return_type: Option<TSTypeAnnotation>,
    pub span: Span,
}

/// Index signature: `[key: string]: T`
#[derive(Debug, Clone)]
pub struct TSIndexSignature {
    pub parameters: Vec<Identifier>,
    pub type_annotation: TSTypeAnnotation,
    pub readonly: bool,
    pub span: Span,
}

/// Function type: `(x: T) => U` or `<T>(x: T) => U`
#[derive(Debug, Clone)]
pub struct TSFunctionType {
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    pub return_type: Box<TSTypeAnnotation>,
    pub span: Span,
}

/// Tuple type: `[T, U, V]`
#[derive(Debug, Clone)]
pub struct TSTupleType {
    pub element_types: Vec<TSType>,
    pub span: Span,
}

/// Rest type in tuples: `...T`
#[derive(Debug, Clone)]
pub struct TSRestType {
    /// The type being spread
    pub type_annotation: Box<TSType>,
    pub span: Span,
}

/// Optional type in tuples: `T?`
#[derive(Debug, Clone)]
pub struct TSOptionalType {
    /// The type that is optional
    pub type_annotation: Box<TSType>,
    pub span: Span,
}

/// Named tuple member: `label: T` or `label?: T`
#[derive(Debug, Clone)]
pub struct TSNamedTupleMember {
    /// The label identifier
    pub label: Identifier,
    /// The element type
    pub element_type: Box<TSType>,
    /// Whether this element is optional (label?: T)
    pub optional: bool,
    pub span: Span,
}

/// Infer type: `infer U` (in conditional types)
///
/// Used in the extends clause of conditional types to introduce a type variable
/// that can be inferred from the matched type.
#[derive(Debug, Clone)]
pub struct TSInferType {
    /// The type parameter being inferred
    pub type_parameter: TSTypeParameter,
    pub span: Span,
}

/// Parenthesized type: `(T)`
#[derive(Debug, Clone)]
pub struct TSParenthesizedType {
    pub type_annotation: Box<TSType>,
    pub span: Span,
}

/// Conditional type: `T extends U ? V : W`
#[derive(Debug, Clone)]
pub struct TSConditionalType {
    pub check_type: Box<TSType>,
    pub extends_type: Box<TSType>,
    pub true_type: Box<TSType>,
    pub false_type: Box<TSType>,
    pub span: Span,
}

/// Type predicate: `x is T` or `asserts x is T`
///
/// Used for type guards and assertion functions.
#[derive(Debug, Clone)]
pub struct TSTypePredicate {
    /// The parameter name being checked (e.g., `x` in `x is string`)
    pub parameter_name: Identifier,
    /// The type being asserted (e.g., `string` in `x is string`)
    /// None for `asserts x` without `is T`
    pub type_annotation: Option<Box<TSType>>,
    /// Whether this is an assertion predicate (`asserts x is T`)
    pub asserts: bool,
    pub span: Span,
}

/// Mapped type: `{ [K in keyof T]: V }`
///
/// Transforms properties from one type to another.
#[derive(Debug, Clone)]
pub struct TSMappedType {
    /// The type parameter with constraint: `K in keyof T`
    pub type_parameter: TSMappedTypeParameter,
    /// Optional key remapping: `as NewK`
    pub name_type: Option<Box<TSType>>,
    /// The value type
    pub type_annotation: Option<Box<TSType>>,
    /// Readonly modifier: None, Some(true) for `readonly`, Some(false) for `-readonly`
    pub readonly: Option<bool>,
    /// Optional modifier: None, Some(true) for `?`, Some(false) for `-?`
    pub optional: Option<bool>,
    pub span: Span,
}

/// Type parameter in a mapped type: `K in keyof T`
#[derive(Debug, Clone)]
pub struct TSMappedTypeParameter {
    /// The parameter name (just the string, not an Identifier)
    pub name: String,
    /// The constraint type (e.g., `keyof T`)
    pub constraint: Box<TSType>,
    pub span: Span,
}

/// Type operator: `keyof T`, `unique symbol`, `readonly T`
#[derive(Debug, Clone)]
pub struct TSTypeOperator {
    /// The operator: "keyof", "unique", "readonly"
    pub operator: TSTypeOperatorKind,
    /// The type being operated on
    pub type_annotation: Box<TSType>,
    pub span: Span,
}

/// Type operator kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TSTypeOperatorKind {
    Keyof,
    Unique,
    Readonly,
}

impl TSTypeOperatorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TSTypeOperatorKind::Keyof => "keyof",
            TSTypeOperatorKind::Unique => "unique",
            TSTypeOperatorKind::Readonly => "readonly",
        }
    }
}

/// Import type: `import('module')` or `import('module', {with: {...}}).Qualifier<T>`
#[derive(Debug, Clone)]
pub struct TSImportType {
    /// The module specifier (string literal)
    pub argument: Literal,
    /// Optional options object: `{with: {type: 'json'}}`
    pub options: Option<Box<Expression>>,
    /// Optional qualifier: `.Foo` or `.Foo.Bar` after the import
    pub qualifier: Option<TSEntityName>,
    /// Optional type arguments: `<T, U>`
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub span: Span,
}

/// Type query expression name: Identifier, QualifiedName, or ImportType
///
/// The `exprName` field of `TSTypeQuery` can be:
/// - `Identifier` for `typeof x`
/// - `TSQualifiedName` for `typeof Foo.bar`
/// - `TSImportType` for `typeof import("module")`
#[derive(Debug, Clone)]
pub enum TSTypeQueryExprName {
    /// Entity name (Identifier or QualifiedName): `typeof x`, `typeof Foo.bar`
    EntityName(TSEntityName),
    /// Import type: `typeof import("module")`
    Import(Box<TSImportType>),
}

impl TSTypeQueryExprName {
    pub fn span(&self) -> Span {
        match self {
            TSTypeQueryExprName::EntityName(e) => e.span(),
            TSTypeQueryExprName::Import(i) => i.span,
        }
    }
}

/// Type query: `typeof x`, `typeof Foo.bar`, `typeof import("module")`, `typeof Array<T>`
///
/// Gets the type of a value expression.
#[derive(Debug, Clone)]
pub struct TSTypeQuery {
    /// The expression whose type is being queried
    pub expr_name: TSTypeQueryExprName,
    /// Optional type arguments: `<T, U>` (e.g., `typeof Array<string>`)
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub span: Span,
}

// ============================================================================
// TypeScript Declaration Nodes
// ============================================================================

/// Interface declaration: `interface Foo { ... }` or `interface Foo extends Bar { ... }`
#[derive(Debug, Clone)]
pub struct TSInterfaceDeclaration {
    pub id: Identifier,
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub extends: Vec<TSInterfaceHeritage>,
    pub body: TSInterfaceBody,
    pub span: Span,
}

/// Interface heritage: `extends Foo, Bar`
#[derive(Debug, Clone)]
pub struct TSInterfaceHeritage {
    pub expression: TSEntityName,
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub span: Span,
}

/// Interface body: `{ members }`
#[derive(Debug, Clone)]
pub struct TSInterfaceBody {
    pub body: Vec<TSTypeElement>,
    pub span: Span,
}

/// Declare function: `declare function foo(): void`
///
/// Also used for functions inside `declare namespace` where `declare` is implicit.
#[derive(Debug, Clone)]
pub struct TSDeclareFunction {
    pub id: Identifier,
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    pub return_type: Option<TSTypeAnnotation>,
    /// Whether to print the `declare` keyword.
    /// True for top-level `declare function`, false inside `declare namespace`.
    pub declare: bool,
    pub span: Span,
}

/// TypeScript enum declaration: `enum Foo { A, B }`, `const enum Foo { A = 1 }`
///
/// Represents an enum declaration. Enums can be:
/// - Regular: `enum Foo { A, B }`
/// - Const: `const enum Foo { A, B }` (inlined at compile time)
/// - Declare: `declare enum Foo { A, B }` (ambient declaration)
/// - Declare const: `declare const enum Foo { A, B }`
#[derive(Debug, Clone)]
pub struct TSEnumDeclaration {
    /// Enum name
    pub id: Identifier,
    /// Enum members
    pub members: Vec<TSEnumMember>,
    /// Whether this is a const enum
    pub r#const: bool,
    /// Whether this is an ambient declaration (declare enum)
    pub declare: bool,
    pub span: Span,
}

/// TypeScript enum member: `A`, `A = 1`, `A = "value"`
///
/// Represents a single member in an enum declaration.
#[derive(Debug, Clone)]
pub struct TSEnumMember {
    /// Member name (identifier or computed)
    pub id: TSEnumMemberId,
    /// Optional initializer expression
    pub initializer: Option<Expression>,
    pub span: Span,
}

/// Enum member id: can be an identifier or a string literal (for computed names)
#[derive(Debug, Clone)]
pub enum TSEnumMemberId {
    Identifier(Identifier),
    /// String literal for computed names like `"hello"` in `enum { "hello" = 1 }`
    String(Literal),
}

/// TypeScript module/namespace declaration: `namespace Utils { ... }` or `module Utils { ... }`
///
/// The `module` keyword is the older syntax, while `namespace` is the modern syntax.
/// Both produce the same AST structure. For nested namespaces like `namespace Outer.Inner`,
/// the parser creates nested TSModuleDeclaration nodes.
#[derive(Debug, Clone)]
pub struct TSModuleDeclaration {
    /// Module/namespace name - identifier for regular namespaces, string literal for ambient modules
    pub id: TSModuleName,
    /// Module body - either a block or nested module declaration (for `A.B.C`)
    /// `None` for shorthand ambient modules: `declare module 'name';`
    pub body: Option<TSModuleDeclarationBody>,
    /// Whether this is an ambient declaration (`declare namespace/module`)
    pub declare: bool,
    /// The keyword used: `namespace` or `module`
    pub kind: TSModuleDeclarationKind,
    /// For `declare global {}` - uses module kind but has special semantics
    pub global: bool,
    pub span: Span,
}

/// Module/namespace name - can be an identifier or a string literal
#[derive(Debug, Clone)]
pub enum TSModuleName {
    /// Regular identifier: `namespace Foo { }`
    Identifier(Identifier),
    /// String literal for ambient modules: `declare module 'name' { }`
    Literal(Literal),
}

/// The keyword used in a module/namespace declaration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TSModuleDeclarationKind {
    /// `namespace` keyword
    Namespace,
    /// `module` keyword (legacy syntax, same semantics as `namespace`)
    Module,
}

/// Body of a TypeScript module declaration
#[derive(Debug, Clone)]
pub enum TSModuleDeclarationBody {
    /// Block body with statements: `namespace A { ... }`
    TSModuleBlock(TSModuleBlock),
    /// Nested module declaration: `namespace A.B { ... }` - the B part
    TSModuleDeclaration(Box<TSModuleDeclaration>),
}

/// TypeScript module block: the `{ ... }` part of a namespace/module declaration
#[derive(Debug, Clone)]
pub struct TSModuleBlock {
    /// Statements inside the module block
    pub body: Vec<Statement>,
    pub span: Span,
}
