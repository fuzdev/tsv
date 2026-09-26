//! Internal AST - optimized for traversal and manipulation
//!
//! Identifier names are span-identity ([`IdentName`]): recovered from the
//! source slice, with an arena-allocated `&'arena str` as a rare escape hatch
//! (unicode-escaped names). This is the primary AST representation used by the
//! parser, formatter, and other tools.
//!
//! ## Arena allocation
//!
//! AST nodes are allocated in a per-parse [`bumpalo::Bump`] supplied by the
//! caller. Recursive children are `&'arena T<'arena>` (not `Box`), child
//! collections are `&'arena [T<'arena>]` (not `Vec`), and decoded strings are
//! `&'arena str` (not `String`) — so a whole parse is one bump-allocated graph,
//! freed wholesale when the `Bump` drops, with no per-node `Drop`. The rare
//! escaped identifier name is one such `&'arena str` ([`IdentName::escaped`]),
//! read directly at every consumer — the AST holds no interner and needs no
//! interner lifetime.

mod classes;
mod declarations;
mod expressions;
mod modules;
mod patterns;
mod statements;
mod types;

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
    TSTypeParameterDeclaration, TSTypeParameterInstantiation, TSTypeParameterModifier,
    TSTypeParameterModifiers, TSTypePredicate, TSTypeQuery, TSTypeQueryExprName, TSTypeReference,
    TSUnionType, TemplateLiteralType,
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
    TSNamespaceExportDeclaration,
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
    ReturnStatement, Statement, StatementKind, SwitchCase, SwitchStatement, ThrowStatement,
    TryStatement, VariableDeclaration, VariableDeclarationKind, VariableDeclarator, WhileStatement,
    WithStatement,
};

// Expressions
pub use expressions::{
    ArrayExpression, ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression,
    AssignmentOperator, AwaitExpression, BinaryExpression, BinaryOperator, CallExpression,
    ConditionalExpression, Expression, ExpressionKind, FunctionExpression, ImportExpression,
    JsdocCast, MemberExpression, MetaProperty, NewExpression, ObjectExpression, ObjectProperty,
    ParenthesizedExpression, Property, PropertyKind, RegexLiteral, SequenceExpression,
    SpreadElement, Super, TSAsExpression, TSInstantiationExpression, TSNonNullExpression,
    TSSatisfiesExpression, TSTypeAssertion, TaggedTemplateExpression, TemplateCooked,
    TemplateElement, TemplateLiteral, ThisExpression, UnaryExpression, UnaryOperator,
    UpdateExpression, UpdateOperator, YieldExpression,
};

// The two node enums are wide enough that holding one BY VALUE across a recursive
// call is a stack-depth decision, not a style one: a frame is sized once for its
// widest arm, so a dispatcher with N arms that each hold one reserves N times these
// bytes at every recursion level. `docs/cli.md` §Recursion Depth states the byte
// counts and the per-construct depths they produce, and the parser's expression ladder
// returns `&'arena Expression` to keep `Expression` out of those frames.
//
// Both enums are also the ELEMENT WIDTH of the containers that hold them, which is
// the larger cost: an `Expression` slot is paid on every element of every
// `&[Expression]` (parameters, array-pattern elements), so the enum's width
// multiplies through the whole tree. Both are held down the same way — the variants
// wide enough to set the size on their own, and rare enough that an arena allocation
// for each is free, hold their payload by `&'arena` reference: `Expression`'s five
// widest (`ClassExpression` / `FunctionExpression` / `ArrowFunctionExpression` /
// `MetaProperty` / `TaggedTemplateExpression`, together ~3% of expressions, of which
// the widest two are ~0.02%), `TSType`'s three widest (`TSImportType`,
// `TSConstructorType`, `TSInferType`, taking it 112 → 80), `Statement`'s DECLARATION
// heads and its four loop / `try` heads one level down (`internal::statements`). Two
// of those declaration heads — `ImportDeclaration` and `ExportNamedDeclaration` — are
// boxed despite NOT being rare (6.8–11.7% and 2.4–4.0% of statements): they are
// simply the last two rungs of the ladder, and a boxed head's construction copies the
// same bytes into the arena that it would have moved into the enum, so the cost is
// one bump pointer against 24 bytes off every other statement slot.
//
// The LIST-ELEMENT containers whose own width was inline `Expression`s —
// `Property` (an object literal's `key: value`, and a destructuring pattern's),
// `VariableDeclarator`, and every `Expression`-holding `Statement` head
// (`ExpressionStatement` 16 B, `IfStatement` / `SwitchStatement` 24, `SwitchCase` 32,
// `WhileStatement` / `DoWhileStatement` / `WithStatement` 16,
// `ReturnStatement` / `ThrowStatement` 8)
// — instead hold those slots by reference, which is not the same trade: the parser's
// expression spine already returns an arena-allocated `&Expression`,
// so an inline slot is a COPY OUT of the arena rather than a place the node lives.
// Naming the slots by reference removes that copy instead of adding an allocation,
// and takes the element every object-literal and declarator list moves from 160 B to
// 32 (`ObjectPatternProperty` 40, its `RestElement` arm setting it) — see
// `parse_expression_ref`. `CatchClause` keeps its inline `param` deliberately: that
// one is built by the parser as an owned value rather than through the spine, so a
// reference there would ADD an allocation, and `CatchClause` is reached only through
// `Option<&CatchClause>` so its width sets nothing. The expression LISTS whose
// elements come off the spine take the reference per element, since there too the
// reference keeps the spine's allocation rather than copying out of it — call and
// `new` arguments, sequence and template expressions (`&[&Expression]`) and
// array-literal elements (`&[Option<&Expression>]`); the parameter lists and
// `ArrayPattern`'s elements are built as owned values, `CatchClause`'s case, and stay
// by value.
//
// `Expression` and `Statement` are each a header over their variant (`span` +
// `ExpressionKind` / `StatementKind`), so a span read is a field load rather than a
// dispatch; the variant payloads shed their own spans to pay for the header, which is
// why both widths hold at 72.
//
// Pinned so a variant that widens any of them shows up as a failed build rather
// than as a silently lower nesting ceiling and a fatter element slot. The counts are
// pointer-width-relative and the doc's measurements are x86-64; `Expression` and
// `Statement` are also pinned on wasm32 (a 4 B pointer; `Expression`'s payload is 8 B
// aligned, `Statement`'s 4 B), and each one's `Option` must stay niche-packed on both.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<Expression<'static>>() == 72);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<ExpressionKind<'static>>() == 64);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(size_of::<Expression<'static>>() == 48);
const _: () = assert!(size_of::<Option<Expression<'static>>>() == size_of::<Expression<'static>>());
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<Statement<'static>>() == 72);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<StatementKind<'static>>() == 64);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(size_of::<Statement<'static>>() == 48);
const _: () = assert!(size_of::<Option<Statement<'static>>>() == size_of::<Statement<'static>>());
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<Property<'static>>() == 32);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<ObjectProperty<'static>>() == 32);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<ObjectPatternProperty<'static>>() == 40);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<VariableDeclarator<'static>>() == 32);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<SwitchCase<'static>>() == 32);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<TSType<'static>>() == 80);

//
// Foundational Types (defined here, used everywhere)
//

/// Program node - the root of the AST
///
/// Returned by value from `parse`; `body` and `comments` point into the
/// caller-supplied `'arena` (the parser gathers comments directly in the bump,
/// so the warm binding loops never malloc for them; `Comment` is a `Copy` POD,
/// satisfying bumpalo's no-`Drop` rule).
#[derive(Debug, Clone)]
pub struct Program<'arena> {
    pub body: &'arena [Statement<'arena>],
    pub comments: &'arena [Comment],
    pub span: Span,
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
    ///
    /// Inside the parser use `Parser::resolve_cooked` instead — there
    /// `self.source` is the local (embedded) slice, not the host document,
    /// so the host-coordinate span must shift back by `base_offset` first.
    #[inline]
    pub fn resolve<'s>(&'s self, span: Span, source: &'s str) -> &'s str {
        match self {
            StringCooked::Verbatim => {
                let raw = span.extract(source);
                // The string token's source slice always includes both quote
                // delimiters (≥2 bytes), so stripping one from each end is in bounds.
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

/// The name channel of an identifier-like node: span-identity by default, with
/// an arena-allocated escape hatch for the rare names the source can't recover.
///
/// `escaped` is `Some` only when the name text differs from the leading
/// `raw_len` bytes at the node's span start — a `\u` unicode escape
/// (`\u0066oo` → `foo`), or a name too long for `raw_len` (> `u16::MAX`
/// bytes) — or when those bytes need a JSON escape, which only a name the grammar
/// does not vouch for can hold (a Svelte directive name like `a\b`; see below).
/// It then carries the name as an `&'arena str`, read directly at every consumer
/// — no interner round-trip: for an escape, the decoded name (the parser's
/// `current_decoded`, already arena-allocated); for an oversized name, its raw
/// text arena-copied; for a Svelte-synthesized name, its text arena-copied
/// verbatim. Otherwise (`None`, >99.99% of
/// identifiers) the name is the raw source slice
/// `span.start .. span.start + raw_len` — nothing stored at all.
///
/// `raw_len` is the raw name-*token* byte length, fixed at token time: the
/// owning node's span may later extend past the name (`?`, `!`, `: Type` —
/// acorn parity), so the name is the leading `raw_len` bytes, never the whole
/// span. When `escaped` is `Some`, `raw_len` is 0 and unused — read the
/// `&'arena str` directly.
///
/// ⚠️ **`escaped: None` is also a claim about the bytes**: the raw slice holds no
/// `"`, no `\` and no control byte, so the wire writer emits it without a JSON
/// escape scan (`write_name_field`). A lexed `IdentifierName` satisfies it by the
/// grammar — its characters are `ID_Start` / `ID_Continue`, `$`, ZWNJ and ZWJ,
/// an escaped spelling always carries its decoded form, and a keyword token is
/// never escaped. A constructor over a slice the grammar does not vouch for (a
/// Svelte directive name, which may hold `\`) must check the bytes and fall back
/// to `escaped: Some`; debug builds assert the claim at the write.
#[derive(Debug, Clone, Copy)]
pub struct IdentName<'arena> {
    pub escaped: Option<&'arena str>,
    pub raw_len: u16,
    /// Whether the raw name is **plain one-column ASCII** — no byte at or above
    /// `0x80`. Answered by the lexer, which walked those bytes anyway and records the
    /// rare tokens that are NOT plain (`Lexer::nonplain_ident_starts`); the parser asks
    /// it by the token's start offset. Like `raw_len`, it describes the *source slice*,
    /// so it is meaningless (and unread) when `escaped` is `Some`; those sites write
    /// `false` the way they write `raw_len: 0`.
    ///
    /// The claim it licenses: an identifier can hold no `\t` and no `\n`
    /// (`IdentifierPart`'s ASCII subset is `[A-Za-z0-9_$]`), so a plain-ASCII name's
    /// **visual width IS `raw_len`** and the printer's width scan has nothing to find
    /// — see `DocArena::source_span_plain`. `false` is always safe: it costs the scan,
    /// never a wrong width, so a site that cannot prove the property says `false`.
    pub plain_ascii: bool,
}

impl<'arena> IdentName<'arena> {
    /// A verbatim name covering `span` exactly (keyword/synthetic sites where
    /// the token has already been consumed — the span is the name token).
    ///
    /// ⚠️ **`plain_ascii` is `true`, and that is a CONTRACT on the caller**: this
    /// constructor is handed a span, not the bytes under it, so the span must cover a
    /// reserved or contextual keyword — ASCII by the grammar. Every caller today is one
    /// of the two meta-properties (`new.target`, `import.meta`). A caller whose span is
    /// an arbitrary name must build the channel from the token instead
    /// (`Parser::current_raw_ident_name`); the printer's name seam asserts the property
    /// in debug builds, so a violation fails the suite rather than shifting a width.
    #[inline]
    pub fn from_span(span: Span) -> Self {
        debug_assert!(u16::try_from(span.end - span.start).is_ok());
        Self {
            escaped: None,
            raw_len: (span.end - span.start) as u16,
            plain_ascii: true,
        }
    }

    /// Resolve the name: the decoded `&'arena str` for an escaped name, else
    /// the leading `raw_len` bytes at `span_start`. `source` must be the host
    /// document the spans were recorded against. `IdentName` is `Copy`, so it
    /// is taken by value; the escaped arena string outlives `source`
    /// (`'arena: 's`), so both arms unify at the source lifetime.
    #[inline]
    pub fn resolve<'s>(self, span_start: u32, source: &'s str) -> &'s str
    where
        'arena: 's,
    {
        match self.escaped {
            Some(s) => s,
            None => &source[span_start as usize..span_start as usize + self.raw_len as usize],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Identifier<'arena> {
    /// The [`IdentName`] channel's escape hatch: the decoded name as an
    /// `&'arena str`, `Some` only for `\u`-escaped or `raw_len`-oversized
    /// names, and for a Svelte-synthesized name holding a JSON-escaped byte (see
    /// [`IdentName`]). Stored flattened (beside `name_len`) rather than as a nested
    /// [`IdentName`] — the nested struct's tail padding would grow `Identifier`,
    /// and it is an *inline* `Expression` variant. (`Expression` is dominated by
    /// far larger variants, so the fat-pointer field does not move
    /// `sizeof(Expression)`.) Read via [`Self::ident_name`].
    pub escaped_name: Option<&'arena str>,
    /// The [`IdentName`] channel's `raw_len`: the raw name-token byte length
    /// (the node span may extend past the name — `?` / `!` / `: Type`).
    pub name_len: u16,
    /// The [`IdentName`] channel's `plain_ascii` — see it for what the flag claims and
    /// who spends it. Flattened beside `name_len` for the same reason `escaped_name`
    /// is, and free: it sits in tail padding the struct already had.
    pub name_plain_ascii: bool,
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

    /// The name channel, reassembled from the flattened fields.
    #[inline]
    pub fn ident_name(&self) -> IdentName<'arena> {
        IdentName {
            escaped: self.escaped_name,
            raw_len: self.name_len,
            plain_ascii: self.name_plain_ascii,
        }
    }

    /// The name's sub-span: the leading `name_len` bytes at the span start (the
    /// node span may extend over `?` / `!` / `: Type`). Only meaningful when
    /// `escaped_name` is `None` — an escaped name reads its `&'arena str` directly.
    #[inline]
    pub fn name_span(&self) -> Span {
        Span::new(self.span.start, self.span.start + self.name_len as u32)
    }

    /// Resolve the identifier's name: the raw source slice (span-identity), or
    /// the decoded `&'arena str` for escaped names. `source` must be the host
    /// document the spans were recorded against.
    #[inline]
    pub fn name<'s>(&self, source: &'s str) -> &'s str
    where
        'arena: 's,
    {
        self.ident_name().resolve(self.span.start, source)
    }

    /// Create a simple identifier (a reference): no optional flag, no binding extra.
    ///
    /// Use this for identifiers in expression context (not parameters). For a
    /// binding that carries `?` / a type annotation / decorators, construct
    /// directly with `extra: Some(arena.alloc(IdentifierParamExtra { … }))`.
    #[inline]
    pub fn simple(name: IdentName<'arena>, span: Span) -> Self {
        Self {
            escaped_name: name.escaped,
            name_len: name.raw_len,
            name_plain_ascii: name.plain_ascii,
            optional: false,
            extra: None,
            span,
        }
    }
}

/// Private identifier: `#foo` in class fields and methods
///
/// Used for truly private class members (ES2022 private class fields).
/// The name does NOT include the `#` prefix, while the holding `Expression`'s span
/// DOES include the `#` character — so the verbatim name is the span minus its
/// leading byte.
#[derive(Debug, Clone)]
pub struct PrivateIdentifier<'arena> {
    /// The name channel (name excludes the `#`; `raw_len` covers the name
    /// bytes after the `#`).
    pub name: IdentName<'arena>,
}

impl<'arena> PrivateIdentifier<'arena> {
    /// The name's sub-span: the trailing `raw_len` bytes of `span`, the span of the
    /// `Expression` holding this node (the name token ends the span; anchoring at the
    /// end stays correct even if the parser ever tolerated separation after the `#`).
    #[inline]
    pub fn name_span(&self, span: Span) -> Span {
        Span::new(span.end - self.name.raw_len as u32, span.end)
    }

    /// Resolve the name (without `#`): the raw source slice, or the decoded
    /// `&'arena str` for escaped names. `span` is the holding `Expression`'s.
    #[inline]
    pub fn name<'s>(&self, span: Span, source: &'s str) -> &'s str
    where
        'arena: 's,
    {
        self.name
            .resolve(span.end - self.name.raw_len as u32, source)
    }
}

// The hot AST enums are deliberately NOT boxed down to a smaller node: the arena
// layout favors traversal locality over node size, keeping recursive children that
// the parser reads constantly inline (`Expression`/`Statement`/`TSType` fields and
// the fat variants) rather than arena-boxing them. Boxing them shrank the slice
// element but added a pointer-chase on hot format-read paths that cost more than the
// density win, so the inline form stands. The `size_of` asserts at the top of this
// file are the OTHER HALF of that trade, not a contradiction of it: the inline form
// buys read locality and pays a recursion ceiling (`docs/cli.md` §Recursion Depth),
// and pinning the two widths is what makes a variant that widens either enum fail the
// build instead of silently lowering that ceiling.
