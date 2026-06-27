//! Internal AST - optimized for traversal and manipulation
//!
//! Uses string interning for memory efficiency. This is the primary AST representation
//! used by the parser, formatter, and other tools.
//!
//! ## Arena allocation
//!
//! AST nodes are allocated in a per-parse [`bumpalo::Bump`] supplied by the
//! caller. Recursive children are `&'arena T<'arena>` (not `Box`), child
//! collections are `&'arena [T<'arena>]` (not `Vec`), and decoded strings are
//! `&'arena str` (not `String`) — so a whole parse is one bump-allocated graph,
//! freed wholesale when the `Bump` drops, with no per-node `Drop`. Leaf nodes
//! that hold only `Span`/`Symbol`/primitives (`PrivateIdentifier`,
//! `RegexLiteral`, `ThisExpression`, `Super`, the operator enums) carry no
//! lifetime. The interner stays `Rc<RefCell<…>>` (shared across the embedding
//! boundary, mutated during parse) — orthogonal to `'arena`.

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
    ImportNamespaceSpecifier, ImportPhase, ImportSpecifier, ModuleExportName, TSExportAssignment,
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
///
/// Returned by value from `parse`; `body` points into the caller-supplied
/// `'arena`. `comments`/`line_breaks` are root-level owned `Vec`s (single
/// allocations, not the per-node arena target — every consumer borrows them as
/// `&[…]` slices).
#[derive(Debug, Clone)]
pub struct Program<'arena> {
    pub body: &'arena [Statement<'arena>],
    pub comments: Vec<Comment>,
    /// Precomputed line break positions (byte offsets of newlines).
    /// Used for O(log n) line boundary lookups during printing.
    pub line_breaks: Vec<u32>,
    pub span: Span,
    pub interner: std::rc::Rc<std::cell::RefCell<DefaultStringInterner>>,
    /// The goal symbol this program was parsed against. Drives the public AST's
    /// `sourceType` and (eventually) the goal-specific grammar gates.
    pub goal: crate::Goal,
}

/// Decorator: `@expression` applied to classes and class members
///
/// The expression can be an identifier (`@foo`), call expression (`@foo()`),
/// or member expression (`@foo.bar`).
#[derive(Debug, Clone)]
pub struct Decorator<'arena> {
    /// The decorator expression (identifier, call, or member expression)
    pub expression: Expression<'arena>,
    pub span: Span,
}

/// Literal value type - supports numbers, strings, booleans, null, and undefined
#[derive(Debug, Clone)]
pub enum LiteralValue<'arena> {
    Number(f64),
    /// String literal. The decoded value is recovered via
    /// `StringCooked::resolve(span, source)` (no-escape = zero-copy inner slice;
    /// escaped = arena bytes); the quote char via `Literal::string_quote(source)`.
    String(StringCooked<'arena>),
    /// BigInt literal: `1n`, `100n`, `0xffn`. No stored payload — digits via
    /// `Literal::bigint_digits(source)` (span minus trailing `n`); the printer
    /// re-derives from source and convert reads the source slice.
    BigInt,
    Boolean(bool),
    Null,
}

/// The decoded value of a string literal, mirroring [`crate::ast::internal::TemplateCooked`].
///
/// `Verbatim` (the common no-escape case) carries **no allocation** — the decoded
/// value equals the inner source slice (the literal's `span` minus the two quote
/// bytes). Only escaped strings own arena bytes.
#[derive(Debug, Clone)]
pub enum StringCooked<'arena> {
    /// Decoded value == the inner source slice (no escapes to decode).
    Verbatim,
    /// Escapes were decoded into a value distinct from the raw inner text.
    Decoded(&'arena str),
}

impl<'arena> StringCooked<'arena> {
    /// The decoded string value. `span` is the owning [`Literal`]'s span (the
    /// quoted token); `source` is the host document. `Verbatim` slices the inner
    /// text (zero-copy); `Decoded` returns the arena bytes. Both share `'s`
    /// (`'arena: 's` via `&'s self`).
    #[inline]
    pub fn resolve<'s>(&'s self, span: Span, source: &'s str) -> &'s str {
        match self {
            StringCooked::Verbatim => {
                let raw = span.extract(source);
                &raw[1..raw.len() - 1]
            }
            StringCooked::Decoded(decoded) => decoded,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Literal<'arena> {
    pub value: LiteralValue<'arena>,
    pub span: Span,
}

impl<'arena> Literal<'arena> {
    /// The quote character of a string literal — the byte at the span start.
    /// (Recovered from source rather than stored.)
    #[inline]
    pub fn string_quote(&self, source: &str) -> u8 {
        source.as_bytes()[self.span.start as usize]
    }

    /// The BigInt digits — the literal source minus the trailing `n`.
    #[inline]
    pub fn bigint_digits<'s>(&self, source: &'s str) -> &'s str {
        let raw = self.span.extract(source);
        &raw[..raw.len() - 1]
    }
}

#[derive(Debug, Clone)]
pub struct Identifier<'arena> {
    pub name: DefaultSymbol,
    /// Whether this is an optional parameter (e.g., `a?` in `function fn(a?: number) {}`)
    pub optional: bool,
    /// Binding-only state (type annotation + parameter decorators), present only
    /// when this identifier is a *binding* — a parameter, `const x: T` declarator
    /// id, catch param, index-signature param, or `{#snippet}` param. `None` for
    /// every variable *reference* (the overwhelming majority). Folded behind one
    /// arena pointer so `Identifier` stays ~24 B: it is an *inline* `Expression`
    /// variant, so its size drives `sizeof(Expression)`. Read via the
    /// `type_annotation()` / `decorators()` accessors.
    pub extra: Option<&'arena IdentifierParamExtra<'arena>>,
    pub span: Span,
}

/// Binding-only extension of [`Identifier`] — the type annotation and parameter
/// decorators a binding identifier carries. Arena-allocated and pointed to from
/// `Identifier.extra` only at the few binding sites that set it; absent (one null
/// pointer) for every reference.
#[derive(Debug, Clone)]
pub struct IdentifierParamExtra<'arena> {
    pub type_annotation: Option<TSTypeAnnotation<'arena>>,
    pub decorators: Option<&'arena [Decorator<'arena>]>,
}

impl<'arena> Identifier<'arena> {
    /// The type annotation, if this is a typed binding (`None` for a reference).
    #[inline]
    pub fn type_annotation(&self) -> Option<&TSTypeAnnotation<'arena>> {
        self.extra.and_then(|e| e.type_annotation.as_ref())
    }

    /// The parameter decorators, if any (`None` for a reference).
    #[inline]
    pub fn decorators(&self) -> Option<&'arena [Decorator<'arena>]> {
        self.extra.and_then(|e| e.decorators)
    }

    /// Create a simple identifier (a reference): no optional flag, no binding extra.
    ///
    /// Use this for identifiers in expression context (not parameters). For a
    /// binding that carries `?` / a type annotation / decorators, construct
    /// directly with `extra: Some(arena.alloc(IdentifierParamExtra { … }))`.
    #[inline]
    pub fn simple(name: DefaultSymbol, span: Span) -> Self {
        Self {
            name,
            optional: false,
            extra: None,
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

// No `size_of` guards on the hot AST enums: the arena layout deliberately favors
// traversal locality over node size, keeping recursive children that the parser
// reads constantly inline (`Expression`/`Statement`/`TSType` fields and the fat
// variants) rather than arena-boxing them for a smaller enum. Boxing them shrank
// the slice element but added a pointer-chase on hot format-read paths that cost
// more than the density win, so the inline form stands. See TODO_BUMPALO_ARENA.md.
