//! Expression nodes
//!
//! Contains the Expression enum and all expression types including
//! operators, literals, function expressions, and TypeScript type assertions.

use crate::ast::AstView;
use tsv_lang::{Comment, Span};

use super::{
    ArrayPattern, ArrowFnId, AssignmentPattern, BlockStatement, ClassExprId, Expr, ExprId,
    FnExprId, Identifier, Literal, MetaPropId, ObjectPattern, PrivateIdentifier, RestElement, Run,
    TSParameterProperty, TSTypeAnnotation, TSTypeParameterDeclaration,
    TSTypeParameterInstantiation, TaggedTplId, TyId,
};

/// Expression node: the span every variant shares, beside the variant itself.
///
/// The span is hoisted out of the variants so reading it is a field load rather than a
/// dispatch over [`ExpressionKind`]; a variant struct carries no span of its own unless
/// some node also holds that type outside an `Expression` (see [`ExpressionKind`]).
#[derive(Debug, Clone)]
pub struct Expression<'arena> {
    pub span: Span,
    pub kind: ExpressionKind<'arena>,
}

/// The variant half of an [`Expression`].
#[derive(Debug, Clone)]
pub enum ExpressionKind<'arena> {
    Literal(Literal<'arena>),
    Identifier(Identifier<'arena>),
    PrivateIdentifier(PrivateIdentifier<'arena>),
    ObjectExpression(ObjectExpression<'arena>),
    ArrayExpression(ArrayExpression),
    UnaryExpression(UnaryExpression),
    UpdateExpression(UpdateExpression),
    BinaryExpression(BinaryExpression),
    CallExpression(CallExpression),
    NewExpression(NewExpression),
    MemberExpression(MemberExpression),
    ConditionalExpression(ConditionalExpression),
    // Stored, each a row of its own payload store named by id, unlike every variant
    // above: these five are the only ones wide enough to set the enum's own size, and
    // each is rare enough that the extra row is free — over 8 MB of real TypeScript, `ClassExpression` and
    // `TaggedTemplateExpression` occur 0.000% of the time, `FunctionExpression`
    // 0.004–0.019%, `MetaProperty` 0.000–0.014%, and `ArrowFunctionExpression`
    // 3.0–3.8%. Inline they would make an `Expression` 176 bytes instead of 64, and
    // that width is paid on every `Expression` SLOT in the tree — both of a
    // `Property`'s, both of a `VariableDeclarator`'s, and every row of a value run —
    // not just on the rare node that needs it. The next-widest inline variant is
    // `ObjectPattern`, and `CallExpression` at 40 bytes is 14–21% of expressions, so the
    // ladder stops here: storing it would tax the common path.
    //
    // The store hop this adds is on the rare read, and the parser's recursion is
    // unaffected either way — it threads expressions by id (every `parse_*` on its
    // ladder returns an `ExprRet`).
    ArrowFunctionExpression(ArrowFnId),
    FunctionExpression(FnExprId),
    ClassExpression(ClassExprId),
    SpreadElement(SpreadElement),
    TemplateLiteral(TemplateLiteral<'arena>),
    TaggedTemplateExpression(TaggedTplId),
    AwaitExpression(AwaitExpression),
    YieldExpression(YieldExpression),
    SequenceExpression(SequenceExpression),
    RegexLiteral(RegexLiteral),
    ThisExpression(ThisExpression),
    Super(Super),
    // Assignment and patterns
    AssignmentExpression(AssignmentExpression),
    ObjectPattern(ObjectPattern<'arena>),
    ArrayPattern(ArrayPattern<'arena>),
    AssignmentPattern(AssignmentPattern<'arena>),
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
    // Meta property: import.meta, new.target (two inline Identifiers)
    MetaProperty(MetaPropId),
    // A grouping pair bound to the comment before its `(` — the JSDoc type cast
    // `/** @type {T} */ (inner)`, or a glued block whose pair opens with a line comment
    // (`/* c */ (⏎// d⏎inner)`). Internal-only, never serialized
    CommentBoundPair(CommentBoundPair),
    // Preserved grouping parens: `(expr)` — only produced under the parser's
    // `preserve_parens` mode (snippet parameters), serialized to match acorn's
    // `preserveParens` output.
    ParenthesizedExpression(ParenthesizedExpression),
}

impl<'arena> Expression<'arena> {
    #[inline]
    pub fn span(&self) -> Span {
        self.span
    }

    /// Wrap an [`Identifier`], taking the wrapper's span from the node's own.
    ///
    /// The seven variant types some node also holds OUTSIDE an `Expression` keep a span
    /// of their own, so their wrapped form carries it twice. Nothing asserts the two agree
    /// (both are `pub`); they start equal because the wrapped form is built only through
    /// these constructors, and no later span write reaches one of these seven — the
    /// parameter parser's `?` / `: T` extension writes the wrapper span of an array or
    /// object pattern, which has no second span to leave behind.
    #[inline]
    pub fn from_identifier(id: Identifier<'arena>) -> Self {
        Self {
            span: id.span,
            kind: ExpressionKind::Identifier(id),
        }
    }

    /// Wrap a [`Literal`] (see [`Expression::from_identifier`]).
    #[inline]
    pub fn from_literal(lit: Literal<'arena>) -> Self {
        Self {
            span: lit.span,
            kind: ExpressionKind::Literal(lit),
        }
    }

    /// Wrap a [`TemplateLiteral`] (see [`Expression::from_identifier`]).
    #[inline]
    pub fn from_template_literal(template: TemplateLiteral<'arena>) -> Self {
        Self {
            span: template.span,
            kind: ExpressionKind::TemplateLiteral(template),
        }
    }

    /// Wrap a [`RestElement`] (see [`Expression::from_identifier`]).
    #[inline]
    pub fn from_rest_element(rest: RestElement) -> Self {
        Self {
            span: rest.span,
            kind: ExpressionKind::RestElement(rest),
        }
    }

    /// Wrap a [`SpreadElement`] (see [`Expression::from_identifier`]).
    #[inline]
    pub fn from_spread_element(spread: SpreadElement) -> Self {
        Self {
            span: spread.span,
            kind: ExpressionKind::SpreadElement(spread),
        }
    }

    /// Wrap a [`UnaryExpression`] (see [`Expression::from_identifier`]).
    #[inline]
    pub fn from_unary_expression(unary: UnaryExpression) -> Self {
        Self {
            span: unary.span,
            kind: ExpressionKind::UnaryExpression(unary),
        }
    }

    /// Wrap a stored [`FunctionExpression`], `span` being the payload's own, read before its
    /// push (see [`Expression::from_identifier`]).
    #[inline]
    pub fn from_function_expression(span: Span, func: FnExprId) -> Self {
        Self {
            span,
            kind: ExpressionKind::FunctionExpression(func),
        }
    }

    /// Where this expression's doc STOPS PRINTING, which is its span end minus any
    /// trailing stripped-paren shell the span swallowed.
    ///
    /// The two differ at exactly one kind of node: one whose span was extended over a
    /// grouping paren the printer erases, without the closer belonging to any child.
    /// An `AssignmentPattern` is that node — `a = (1)` spans through the `)` while its
    /// doc stops at the value — and a `TSParameterProperty` inherits it through the
    /// parameter it wraps. The region between the two ends therefore holds nothing but
    /// `)`, whitespace and comments.
    ///
    /// This is the anchor a list element's TRAILING-COMMENT scan takes (`docs/comments.md`
    /// §The element-comma seam): starting at the span end begins the scan *past* the
    /// shell's interior, and since the element's own doc never reaches there either, a
    /// comment inside the shell is claimed by nobody and DROPPED. ⚠️ Not the anchor for a
    /// scan that measures a DISTANCE across the shell (a blank-line scan wants
    /// [`crate::printer::Printer::element_shell_end`], the other end of the same
    /// difference), and ⚠️ not for an element the format-ignore directive FROZE — a frozen
    /// element prints its raw source, the shell and its comment included, so shifting the
    /// anchor there DOUBLE-PRINTS it.
    ///
    /// A spread / rest element is deliberately untouched by this: its stripped-paren
    /// interior is its own doc's share to print (§A stripped-paren interior is a partition
    /// too), so its span end is already its printed end.
    pub fn printed_end(&self, view: AstView<'_>) -> u32 {
        match &self.kind {
            ExpressionKind::AssignmentPattern(assign) => view.expr(assign.right).printed_end(view),
            ExpressionKind::TSParameterProperty(param_prop) => {
                view.expr(param_prop.parameter).printed_end(view)
            }
            _ => self.span.end,
        }
    }

    /// Peel type-assertion wrappers (`as` / `satisfies` / non-null `!` / `<T>`),
    /// returning the innermost wrapped expression (e.g. the `x` in `(x as T)!`).
    /// The single source of truth for "what counts as a type assertion", used by the
    /// assignment-target validation (does the assertion wrap a *simple* target?).
    /// `TSInstantiationExpression` (`f<T>`) is deliberately **not** peeled — it is not
    /// an assertion. Peeling is a QUESTION here, never an emission: the wire keeps the
    /// assertion node, and only convert's `unwrap_comment_bound_pairs` drops anything.
    pub fn skip_type_assertions(&self, view: AstView<'arena>) -> &Expression<'arena> {
        match &self.kind {
            ExpressionKind::TSAsExpression(e) => view.expr(e.expression).skip_type_assertions(view),
            ExpressionKind::TSSatisfiesExpression(e) => {
                view.expr(e.expression).skip_type_assertions(view)
            }
            ExpressionKind::TSNonNullExpression(e) => {
                view.expr(e.expression).skip_type_assertions(view)
            }
            ExpressionKind::TSTypeAssertion(e) => {
                view.expr(e.expression).skip_type_assertions(view)
            }
            // A JSDoc `/** @type {T} */ (expr)` cast is a type assertion too —
            // peel it to its inner target like the others.
            ExpressionKind::CommentBoundPair(e) => view.expr(e.inner).skip_type_assertions(view),
            _ => self,
        }
    }

    /// Peel internal-only `CommentBoundPair` wrappers, returning the expression the
    /// public AST will actually carry — a cast converts to its inner
    /// expression (paren-free, matching acorn/Svelte), so structural checks
    /// against "the converted node" must look through the wrapper. Note the
    /// wrapper's own `span()` covers the parens; the public node's span is the
    /// unwrapped inner's.
    pub fn unwrap_comment_bound_pairs(&self, view: AstView<'arena>) -> &Expression<'arena> {
        let mut e = self;
        while let ExpressionKind::CommentBoundPair(cast) = &e.kind {
            e = view.expr(cast.inner);
        }
        e
    }

    /// The stripped-paren interior of a spread or rest element
    /// ([`SpreadElement::paren_interior`], [`RestElement::paren_interior`]) — `None` for
    /// every other expression, which has none. The list printers' one adapter:
    /// `ObjectProperty::paren_interior` and `ObjectPatternProperty::paren_interior` are the
    /// property-list counterparts.
    pub fn paren_interior(&self, view: AstView<'_>) -> Option<Span> {
        match &self.kind {
            ExpressionKind::SpreadElement(s) => Some(s.paren_interior(view)),
            ExpressionKind::RestElement(r) => Some(r.paren_interior(view)),
            _ => None,
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
    ///
    /// This walks the whole spine, so a caller that asks it at *every* spine
    /// node is quadratic in chain depth. The wire writer therefore hands the
    /// answer down instead of re-deriving it — see the convert layer's
    /// `ChainState`.
    ///
    /// A loop down the spine, not a recursion: a chain is as long as the spine limit lets
    /// it be, and a frame per link is what the stack contract prices.
    pub fn has_optional_in_chain(&self, view: AstView<'_>) -> bool {
        let mut expr = self;
        loop {
            let (optional, child) = match &expr.kind {
                ExpressionKind::MemberExpression(m) => (m.optional, m.object),
                ExpressionKind::CallExpression(c) => (c.optional, c.callee),
                ExpressionKind::TSNonNullExpression(n) => (false, n.expression),
                _ => return false,
            };
            if optional {
                return true;
            }
            if expr.span.start < view.expr(child).span.start {
                return false;
            }
            expr = view.expr(child);
        }
    }

    /// True when this is a non-null assertion sealing a parenthesized optional chain
    /// (`(a?.b)!`) — [`TSNonNullExpression::seals_optional_chain`] asked of the node that
    /// holds it.
    pub fn is_sealing_non_null(&self, view: AstView<'_>) -> bool {
        matches!(
            &self.kind,
            ExpressionKind::TSNonNullExpression(n) if n.seals_optional_chain(view, self.span)
        )
    }
}

/// A grouping pair the comment before its `(` binds: the JSDoc type cast
/// `/** @type {T} */ (inner)`, or a block glued to a pair whose interior opens with a
/// line comment (`/* c */ (⏎// d⏎inner)`) — see [`CommentBoundPairKind`].
///
/// Internal-only wrapper recording that the author wrote a parenthesized
/// expression immediately preceded by a block comment that the pair keeps bound to the
/// expression. Ordinary grouping parens are discarded at parse time; these are preserved
/// here so the printer can re-emit them. The kind decides only whether the parser mints
/// the node: every printer reader lays both kinds out alike, which is the point — a
/// glued block's pair prints exactly as a cast's.
///
/// The holding `Expression`'s span covers the parentheses (`(`…`)`); `inner` keeps its
/// own paren-free span. **Never serialized** — the convert layer unwraps to `inner`, so the
/// public AST stays paren-free, matching acorn/Svelte (which carry no
/// `ParenthesizedExpression`). Distinct from a bare grouping paren, the wrapper
/// is opaque to layout heuristics (expand-last etc.), mirroring how acorn's
/// `ParenthesizedExpression` hides the inner type in prettier-plugin-svelte.
#[derive(Debug, Clone)]
pub struct CommentBoundPair {
    pub inner: ExprId,
    /// The comment glued to `span.start` — the `@type`/`@satisfies` comment of a cast, or
    /// the glued block. The node owns it (`Comment::owned_by_node`), so the generic
    /// comment machinery skips it and this node is the only thing that prints it — which
    /// is what keeps a paren synthesized around an enclosing expression from landing
    /// between the comment and the `(` and silently re-binding it.
    pub comment: Comment,
    /// Why the pair is kept.
    pub kind: CommentBoundPairKind,
}

/// Why a [`CommentBoundPair`] keeps its grouping pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentBoundPairKind {
    /// A JSDoc type cast (`/** @type {T} */ (inner)` / `@satisfies`): a TypeScript type
    /// assertion whose parentheses are semantically required — without them the
    /// assertion is dropped. Its comment may sit on a line of its own above the `(`.
    Cast,
    /// A block comment glued to the `(` (same line, spaces/tabs only) of a pair whose
    /// interior opens with a line comment (`/* c */ (⏎// d⏎inner)`, also behind further
    /// `(`s: `/* c */ ((⏎// d⏎x).m)`). A glued block binds the expression it leads, but
    /// with the pair stripped the `//` prints between the two and the block leads
    /// nothing — a `/* @__PURE__ */` there no longer marks the call. Kept, the block
    /// stays glued to the `(` it was written against. Always same-line by construction.
    GluedComment,
}

/// Preserved grouping parentheses: `(expr)`.
///
/// Ordinary grouping parens are discarded at parse time (acorn/Svelte carry no
/// `ParenthesizedExpression`, so the public AST is paren-free). This wrapper is
/// produced **only** under the parser's `preserve_parens` mode — active solely
/// for `{#snippet}` parameters, where Svelte parses with acorn's
/// `preserveParens: true` and, unlike every other template expression, skips
/// `remove_parens` (`1-parse/state/tag.js`) — and for the binding audit's wire-only
/// reparse. The public AST then keeps the `ParenthesizedExpression` node, matching
/// Svelte's output. A JSDoc cast's pair is one of them: under that mode it is this node
/// (the cast comment leading it on Svelte's wire), never a [`CommentBoundPair`].
///
/// The holding `Expression`'s span covers the parentheses (`(`…`)`); `expression`
/// keeps its own paren-free span. Unlike [`CommentBoundPair`], the parens are **not** semantically
/// required — prettier re-derives them (`(b)`→`b`, but keeps `(2, 3)` because a
/// sequence needs them) — so the node is a **wire** shape only: no printer or compiler
/// ever reads a tree that holds one. `tsv_svelte` parses such a head twice and hands every
/// other consumer the paren-free tree, the one a `function` signature with the same text
/// parses to. A pair answers no paren question about its position, so a printer handed
/// one prints `(x + y) * 2` as `x + y * 2`.
#[derive(Debug, Clone)]
pub struct ParenthesizedExpression {
    pub expression: ExprId,
}

impl<'arena> ParenthesizedExpression {
    /// The pair's contents, for a consumer of the paren-free tree — which never holds one.
    ///
    /// The exhaustive matches in the printer and the compiler keep an arm for this node
    /// only because they are exhaustive, and every such arm reads through here: a debug
    /// build refuses a tree that breaks the wire-only contract, and a release build reads
    /// through the pair rather than aborting the host.
    #[track_caller]
    pub fn unreachable_contents(&self, view: AstView<'arena>) -> &'arena Expression<'arena> {
        #[expect(clippy::unreachable)] // debug builds only: see the doc
        if cfg!(debug_assertions) {
            unreachable!("a ParenthesizedExpression reached a reader of the paren-free tree");
        }
        view.expr(self.expression)
    }
}

#[derive(Debug, Clone)]
pub struct ObjectExpression<'arena> {
    pub properties: &'arena [ObjectProperty],
    /// `true` when a trailing comma follows a final spread property (`{...a,}`).
    /// Valid in an object *literal*, but a syntax error once the literal is
    /// refined to an object pattern — the grammar's rest property admits no
    /// trailing comma. The literal parser discards the comma, so this flag is
    /// the only surviving record of it for `refine_assignable` to consult.
    pub spread_trailing_comma: bool,
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

    /// Where the property's doc STOPS PRINTING — the anchor its trailing-comment scan takes
    /// (`docs/comments.md` §The element-comma seam).
    ///
    /// Always the VALUE's end, the key's being only incidentally the same. A **shorthand**
    /// property normally shares one span between key and value, but not always: the
    /// permissively parsed `{ a = 1 }` (a `CoverInitializedName`, whose early error tsv
    /// defers) is shorthand with an `AssignmentExpression` value spanning `a = 1` — text the
    /// property's own doc prints. A key anchor opened the scan over `= 1`, so a comment
    /// before the `=` (`{ a /* c */ = 1 }`) was claimed as trailing the property and
    /// RELOCATED past the initializer, across the name→`=` binding tsv otherwise refuses to
    /// move a comment over.
    pub fn value_end(&self, view: AstView<'_>) -> u32 {
        match self {
            ObjectProperty::Property(p) => view.expr(p.value).span().end,
            // The spread's OWN end, past any grouping parens the parser stripped — not
            // the argument's. The interior of those parens is the spread doc's share to
            // print (`docs/comments.md` §A stripped-paren interior is a partition too);
            // anchoring the object's trailing scan at the argument's end is the anchor
            // shift that hands the object that share as well, printing every same-line
            // block and interior `//` TWICE.
            ObjectProperty::SpreadElement(s) => s.span.end,
        }
    }

    /// The spread's stripped-paren interior ([`SpreadElement::paren_interior`]), if this
    /// property is a spread — the property-list counterpart of
    /// [`Expression::paren_interior`].
    pub fn paren_interior(&self, view: AstView<'_>) -> Option<Span> {
        match self {
            ObjectProperty::SpreadElement(s) => Some(s.paren_interior(view)),
            ObjectProperty::Property(_) => None,
        }
    }
}

/// Array literal expression: `[1, 2, 3]`
///
/// Elements are wrapped in Option to support sparse arrays like `[1,,3]`
/// where missing elements are represented as None.
#[derive(Debug, Clone)]
pub struct ArrayExpression {
    pub elements: Run<Option<ExprId>>,
    /// `true` when a trailing comma follows a final spread element (`[...a,]`).
    /// Valid in an array *literal*, but a syntax error once the literal is
    /// refined to an array pattern — the grammar's rest element admits no
    /// trailing comma. The literal parser discards the comma, so this flag is
    /// the only surviving record of it for `refine_assignable` to consult.
    pub spread_trailing_comma: bool,
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
    pub argument: ExprId,
    pub prefix: bool, // true for `++x`/`--x`, false for `x++`/`x--`
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
    pub argument: ExprId,
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

    /// Whether a RIGHT operand carrying `other` joins this operator's chain.
    ///
    /// Prettier's `isUnbalancedLogicalTree` (`parse/postprocess/index.js`), which
    /// REBALANCES `a op (b op c)` into `(a op b) op c` at parse time for a LogicalExpression
    /// whose right operand carries the same operator — recursively, before the printer runs.
    /// tsv keeps the authored tree and answers the same question here, so the flattened
    /// chain and every rule keyed on it (`should_inline_logical_expression`,
    /// `build_binary_chain_doc_core`'s operand count) see prettier's rebalanced shape.
    ///
    /// Only the logical operators are associative this way — arithmetic keeps its right
    /// parens (`a + (b + c)` is not `a + b + c` for every operand type), and prettier
    /// rebalances no BinaryExpression. Among `&&` / `||` / `??` each has its own
    /// precedence level, so "same operator" is exactly what `can_flatten_with` says there.
    #[inline]
    pub const fn rebalances_with(self, other: BinaryOperator) -> bool {
        // `as u8` and not `==`: `PartialEq` is not const, and `#[repr(u8)]` with no explicit
        // discriminants gives each variant its own value, so the casts compare variants.
        self.is_logical() && self as u8 == other as u8
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
    pub left: ExprId,
    pub operator: BinaryOperator,
    pub right: ExprId,
    /// For a `<` node UNDER A `>` only: whether the region this `<` opens would re-lex as a
    /// TYPE-ARGUMENT LIST once printed — the parser's own relaxed reading of the same
    /// lookahead that decided this `<` is the less-than operator, recorded here because the
    /// printer cannot re-derive it (it holds a tree, and the question is about bytes).
    ///
    /// The two readings grade different TEXT — the printed form against every parser that
    /// reads tsv's output, rather than the source against acorn. The printer's own line
    /// breaks are the plainest case: it folds the author's break away (`fn<A⏎[T]>(t, u)`
    /// prints as `fn < A[T] > (t, u)`, a `CallExpression` on re-parse) and may add its own
    /// past the `>` (`x < y >⏎{ a: 1 }` is an instantiation plus a free-standing block).
    /// Every place the two readings differ, with the seam each is read at, is enumerated
    /// once on `TypeArgScan`.
    ///
    /// A `>` whose LEFT operand is such a `<` therefore keeps a paren pair —
    /// `(fn < A[T]) > (t, u)` — synthesized and retained by `needs_parens`, which is the one
    /// reader of this field.
    ///
    /// It is not the whole of that rule's condition, and the remainder is why this field is
    /// a claim about the SOURCE rather than about every printed region. A `(` the printer
    /// KEEPS around the `<` operand is a region head the scan never saw — the scan looked
    /// THROUGH the shell, since the shells it was written for are ones the printer strips —
    /// so `needs_parens` asks that half itself
    /// (`relational_region_opens_on_a_kept_shell`), where the printed shape is known. The
    /// two disjuncts partition the question; neither is derivable from the other.
    ///
    /// Set LAZILY, at the `>` that would close the region rather than at the `<`: the
    /// verdict is a fact about the join of the two tokens, and a `<` with no `>` over it —
    /// every ordinary comparison — must not pay the scan. The node is already in the arena
    /// by then, so the parser re-allocates this one node to set the bit.
    ///
    /// `false` on every other operator, and free in the struct's existing padding.
    pub relexes_as_type_arguments: bool,
    /// For a `>`, `>>` or `>>>` only: whether enough binary `<` stand AHEAD of it with
    /// their regions still open for it to close every list they opened — one for a `>`,
    /// two for a `>>`, three for a `>>>` (a `<<` opens two) — so this token may be where
    /// tsc's type-argument parse stops. It must then never END A LINE, and never stand
    /// directly ahead of a paren pair the printer invents.
    ///
    /// tsc does not guess at a `<`: `parseTypeArgumentsInExpression` parses a list for
    /// real, with error recovery, and keeps a truthy result errors and all. The list runs
    /// from the `<` to the first token the type grammar cannot take — a missing `)`, `]`
    /// or `}` is reported and stepped over, a comma starts the next argument — and when
    /// that token is a lone `>` (`>=` / `>>` re-scan to something else) the follower
    /// question is asked THERE, where a follower past a LINE BREAK commits the list. So
    /// `x < (a > b)` is a comparison while `b` shares the `>`'s line, and a syntax error
    /// once a break stands between them; `fn(x < q, a >⏎b)` needs no shell at all (the
    /// comma is the argument separator) and is a generic call to acorn-typescript and to
    /// tsv's own parse as well. The pair `relexes_as_type_arguments` earns does not reach
    /// it: that pair ends the OUTER `>`'s region on a `)`, and this `>` is met first.
    ///
    /// Between the two tokens the printer prints what was written — inside a comma-list
    /// region the pairs its style decides follow the source
    /// (`Printer::mark_region_source_pairs`) — and a `(` or template follower commits on
    /// any line, so the printer's free choices at this token are two, and it makes both:
    ///
    /// - where the line breaks: it breaks AHEAD of such a `>` (`aaa⏎> bbb`), its follower
    ///   sharing its line — the binary-chain emitters' `ChainOperator::leads_line`;
    /// - whether a `(` of its own follows: a CLARITY pair around the right operand's
    ///   first token (`c > (await d)`, `c > (d % e) + f`, `c > (function () {})()`) is
    ///   one the tree does not need, and printed behind this `>` it is the `(` that
    ///   commits the list — `fn(a < b, c > (await d))` is the generic call
    ///   `a<b, c>(await d)` to every parser. So the operand prints bare there
    ///   (`Printer::withhold_clarity_pairs_behind_region_close`); a pair the author wrote
    ///   stays, since behind a region that reads as a list it is no pair but that call's
    ///   own argument list.
    ///
    /// A `>>` behind two open regions and a `>>>` behind three are the same token in the
    /// type grammar, which scans a `>` at a time: `fn(a < b < c, d >> (e))` is the call
    /// `a<b<c, d>>(e)`, and `fn(a < b < c, d >>⏎e)` a list with a statement after it.
    ///
    /// A deliberate SUPERSET, decided by the parser because only it sees the tokens in
    /// order: which `<` regions are still open is the parser's `LtRegion`, which states
    /// where a region provably ends. `false` on every other operator, and free in the
    /// struct's existing padding.
    pub may_close_type_arguments: bool,
}

impl<'arena> BinaryExpression {
    /// The right operand of prettier's REBALANCED tree: the deepest right of the spine
    /// [`BinaryOperator::rebalances_with`] joins, which is this chain's LAST operand.
    ///
    /// ⚠️ **A rule prettier asks of a logical node must ask this, not `self.right`** —
    /// prettier rebalances at parse time, so `a ?? (b ?? [1])` is `(a ?? b) ?? [1]` at every
    /// question its printer asks, and a rule keyed on `self.right` is asking about a tree
    /// prettier does not have. That is a whole class of bug: it can only show up on the
    /// paren-nested twin of an authoring, and it shows up as a divergence AND a
    /// non-idempotency, since tsv's own reparse of its output sees the left-nested shape.
    ///
    /// tsv cannot rebalance the tree itself — `parse` is a drop-in replacement for acorn's
    /// public JSON AST, and acorn keeps `a ?? (b ?? c)` right-nested. So the normalization
    /// lives here and in the printer's chain flattening, which is the same rule
    /// ([`BinaryOperator::rebalances_with`]).
    ///
    /// Not every rule needs it: one that is symmetric in the two operands
    /// (`should_group_binary_continuation`, `is_simple_call_argument`) answers the same on
    /// both trees. Verify, don't assume.
    #[must_use]
    pub fn rebalanced_right(&self, view: AstView<'arena>) -> &'arena Expression<'arena> {
        let mut right = view.expr(self.right);
        while let ExpressionKind::BinaryExpression(inner) = &right.kind {
            if !self.operator.rebalances_with(inner.operator) {
                break;
            }
            right = view.expr(inner.right);
        }
        right
    }
}

/// Call expression: `foo()`, `obj.method(arg1, arg2)`, `fn<T>()`
#[derive(Debug, Clone)]
pub struct CallExpression {
    pub callee: ExprId,
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub arguments: Run<ExprId>,
    pub optional: bool, // true for `foo?.()` (optional chaining)
}

/// New expression: `new Date()`, `new Map()`
///
/// Constructor call with the `new` keyword. The callee is typically an
/// identifier or member expression, and arguments are optional.
/// Type arguments like `new Map<K, V>()` are stored in `type_arguments`.
#[derive(Debug, Clone)]
pub struct NewExpression {
    pub callee: ExprId,
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub arguments: Run<ExprId>,
}

/// Dynamic import expression: `import('module')` or `import('module', options)`,
/// optionally phased as `import.source(…)` / `import.defer(…)`.
#[derive(Debug, Clone)]
pub struct ImportExpression {
    pub source: ExprId,
    /// Optional second argument for import attributes: `{with: {type: 'json'}}`
    pub options: Option<ExprId>,
    /// Import phase: `Source`/`Defer` for `import.source(…)` / `import.defer(…)`.
    pub phase: super::modules::ImportPhase,
}

/// Meta property: `import.meta`, `new.target`
#[derive(Debug, Clone)]
pub struct MetaProperty<'arena> {
    /// The keyword: "import" or "new"
    pub meta: Identifier<'arena>,
    /// The property: "meta" or "target"
    pub property: Identifier<'arena>,
}

/// Member expression: `obj.prop`, `arr[0]`
#[derive(Debug, Clone)]
pub struct MemberExpression {
    pub object: ExprId,
    pub property: ExprId,
    pub computed: bool, // true for `arr[0]`, false for `obj.prop`
    pub optional: bool, // true for `obj?.prop` (optional chaining)
}

/// Conditional (ternary) expression: `a ? b : c`
#[derive(Debug, Clone)]
pub struct ConditionalExpression {
    pub test: ExprId,
    pub consequent: ExprId,
    pub alternate: ExprId,
}

/// Arrow function expression: `() => expr` or `() => { stmts }`
///
/// Supports both expression bodies and block bodies:
/// - Expression body: `x => x + 1` (body is Expression)
/// - Block body: `x => { return x + 1; }` (body is BlockStatement)
#[derive(Debug, Clone)]
pub struct ArrowFunctionExpression<'arena> {
    /// Type parameters (TypeScript generics): `<T>() => ...`
    pub type_parameters: Option<TSTypeParameterDeclaration<'arena>>,
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Run<Expr>,
    pub body: ArrowFunctionBody,
    /// Return type annotation (TypeScript): (): number => ...
    pub return_type: Option<TSTypeAnnotation>,
    /// Whether this is an async arrow function: `async () => ...`
    pub r#async: bool,
    /// Position of opening paren for params, if parenthesized.
    /// `Some(pos)` for `(x) => x` or `() => x`, `None` for `x => x`
    pub params_start: Option<u32>,
    /// Byte offset of the `=` in the arrow's `=>` token, captured by the parser
    /// when it consumes the arrow (like `params_start`, a print-time position fact,
    /// not emitted to the wire). Lets the printer split comments around `=>` without
    /// re-scanning source for it.
    pub arrow_token: u32,
}

/// Arrow function body - either an expression or a block statement
#[derive(Debug, Clone)]
pub enum ArrowFunctionBody {
    /// Expression body: `() => expr`
    Expression(ExprId),
    /// Block body: `() => { stmts }`
    BlockStatement(BlockStatement),
}

impl ArrowFunctionBody {
    pub fn span(&self, view: AstView<'_>) -> Span {
        match self {
            ArrowFunctionBody::Expression(expr) => view.expr(*expr).span(),
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
pub struct FunctionExpression<'arena> {
    /// Optional function name (for named function expressions)
    pub id: Option<Identifier<'arena>>,
    /// Type parameters (TypeScript generics): `function<T>() {}`
    pub type_parameters: Option<TSTypeParameterDeclaration<'arena>>,
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Run<Expr>,
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
    pub argument: ExprId,
    pub span: Span,
}

impl SpreadElement {
    /// The source the grouping parens the parser stripped from the argument leave behind
    /// — `[argument end, span end)`, the region between the argument and the shell's last
    /// `)` (`...(x /* c */)`). Its comments are split between the spread's own doc and the
    /// parent list (`docs/comments.md` §A stripped-paren interior is a partition too); the
    /// rest twin is [`RestElement::paren_interior`].
    pub fn paren_interior(&self, view: AstView<'_>) -> Span {
        Span::new(view.expr(self.argument).span().end, self.span.end)
    }
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

/// Object property: `{ key: value }` — and, reused verbatim, a destructuring
/// pattern's property (`ObjectPatternProperty::Property`).
///
/// Both slots are ids rather than inline `Expression`s, which takes the element every
/// object literal and every object pattern moves from 160 B to 20. It costs no row: the
/// parser's expression spine already returns the id of the node it pushed, so an inline
/// slot would be a copy *out of* the store rather than the place the node lives — see
/// the density note in `ast/mod.rs`.
#[derive(Debug, Clone)]
pub struct Property {
    pub key: ExprId,
    pub value: ExprId,
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
pub struct TemplateLiteral<'arena> {
    pub quasis: &'arena [TemplateElement<'arena>],
    pub expressions: Run<ExprId>,
    pub span: Span,
}

/// The decoded ("cooked") value of a template element.
///
/// The common no-escape case (`Verbatim`) carries no allocation: the cooked
/// text equals the element's TRV, recovered via `TemplateElement::trv`.
/// Only genuinely decoded values (escapes present) hold an arena-allocated str.
#[derive(Debug, Clone)]
pub enum TemplateCooked<'arena> {
    /// Cooked value == the element's TRV (no escapes to decode).
    Verbatim,
    /// Escapes were decoded into a value distinct from the raw text.
    Decoded(&'arena str),
    /// No cooked value — an invalid escape in a TAGGED template, which ES2018
    /// permits and acorn reports as `null`. (Untagged, the same escape is a
    /// syntax error, so `Parser::template_cooked` raises there instead.)
    Invalid,
}

/// Template element - a static string part of a template literal
///
/// Each quasi has:
/// - raw_span: Span of the literal source text (escapes preserved); text via `raw(source)`
/// - raw_trv: the same text as a VALUE (line terminators normalized); via `trv(source)`
/// - cooked: The decoded value (escapes interpreted); text via `cooked(source)`
/// - tail: true for the last element in the template
///
/// ⚠️ **`raw` and `trv` are different questions and both have callers.** The
/// source text is what the PRINTER emits (it reproduces the author's bytes);
/// the TRV is what the WIRE reports (`value.raw`, and `String.raw` at runtime).
/// They differ only over a `<CR>` — see `raw_trv`.
#[derive(Debug, Clone)]
pub struct TemplateElement<'arena> {
    /// Span of the raw source text (escape sequences NOT decoded), delimiters
    /// excluded. The raw content is a verbatim sub-slice of source, so it is
    /// stored as a span and recovered on demand via `raw(source)` rather than
    /// owning a `String`. Distinct from `span` (the full token span, which for
    /// middle/tail type quasis starts at the prior `}` brace, not the content).
    pub raw_span: Span,
    /// The element's TRV when it differs from the source slice — i.e. only when
    /// the raw text carries a `<CR>`.
    ///
    /// ECMAScript maps `<CR>` and `<CR><LF>` to a single `<LF>` in **both** the
    /// TRV and the TV of a template's `LineTerminatorSequence`s, while `<LF>`,
    /// `<LS>` and `<PS>` map to themselves (§12.9.6). So `` `x<CR><LF>y` `` has
    /// a 4-byte source span and a 3-character value, and `raw_span` alone cannot
    /// express it. `None` — overwhelmingly the common case — means the source
    /// slice IS the TRV, so this costs no allocation on any ordinary template.
    pub raw_trv: Option<&'arena str>,
    /// The decoded value. `Verbatim` (the common no-escape case) costs no
    /// allocation — its text equals `trv(source)`; recover via `cooked(source)`.
    pub cooked: TemplateCooked<'arena>,
    /// Whether the raw SOURCE text contains a newline (precomputed so newline
    /// checks stay O(1) and source-free, matching `Comment::multiline`). Read by
    /// the printer, which walks `raw_span`, so it answers about the source
    /// bytes — not about `trv`, where a lone `<CR>` becomes a newline.
    pub has_newline: bool,
    /// True if this is the last element (tail)
    pub tail: bool,
    pub span: Span,
}

impl<'arena> TemplateElement<'arena> {
    /// The raw source text (escape sequences NOT decoded), a delimiter-stripped
    /// sub-slice of `source` (the host document the spans were recorded against).
    /// The PRINTER's view — verbatim author bytes, `<CR>`s included.
    #[inline]
    pub fn raw<'s>(&self, source: &'s str) -> &'s str {
        self.raw_span.extract(source)
    }

    /// The element's TRV — the raw text as a *value*, with `<CR>` / `<CR><LF>`
    /// normalized to `<LF>` per ECMAScript. The WIRE's view (`value.raw`), and
    /// what a tagged template's `String.raw` observes at runtime. Equals
    /// [`Self::raw`] unless the source carries a `<CR>`; see `raw_trv`.
    #[inline]
    pub fn trv<'s>(&'s self, source: &'s str) -> &'s str {
        self.raw_trv.unwrap_or_else(|| self.raw(source))
    }

    /// The decoded ("cooked") value as text, or `None` for an invalid escape.
    /// The no-escape case borrows the TRV (no owned string unless a `<CR>`
    /// forced one); the `Decoded` arm borrows from the arena (`'arena: 's` via
    /// `&'s self`).
    #[inline]
    pub fn cooked<'s>(&'s self, source: &'s str) -> Option<&'s str> {
        match self.cooked {
            TemplateCooked::Verbatim => Some(self.trv(source)),
            TemplateCooked::Decoded(decoded) => Some(decoded),
            TemplateCooked::Invalid => None,
        }
    }
}

/// Tagged template expression: tag`content ${expr}`
///
/// The tag is called with the template's static parts and interpolated values.
/// When the tag has type arguments (e.g., `tag<T>\`content\``), they are stored
/// separately rather than wrapping the tag in `TSInstantiationExpression`.
#[derive(Debug, Clone)]
pub struct TaggedTemplateExpression<'arena> {
    pub tag: ExprId,
    pub type_arguments: Option<TSTypeParameterInstantiation>,
    pub quasi: TemplateLiteral<'arena>,
}

/// Await expression: `await promise`
///
/// Used in async functions to wait for a Promise to resolve.
/// The argument is the expression being awaited.
#[derive(Debug, Clone)]
pub struct AwaitExpression {
    pub argument: ExprId,
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
    pub argument: Option<ExprId>,
    /// Whether this is a delegating yield: `yield*`
    pub delegate: bool,
}

/// Sequence expression: `a, b, c`
///
/// Evaluates all expressions left to right, returns the last value.
/// Created by the comma operator at expression level.
#[derive(Debug, Clone)]
pub struct SequenceExpression {
    pub expressions: Run<ExprId>,
}

/// Regular expression literal: `/pattern/flags`
///
/// Represents a regex literal with its pattern and flags.
/// Unlike strings, regex patterns are NOT decoded - escape sequences are preserved.
///
/// TODO: Add regex validation (pattern is valid, flags are valid and unique)
#[derive(Debug, Clone)]
pub struct RegexLiteral {
    /// Span of the pattern between the slashes (e.g., "\\d+"). The pattern is a
    /// verbatim sub-slice of source (escape sequences preserved, never decoded),
    /// so it is stored as a span and recovered via `pattern(source)`.
    pub pattern_span: Span,
    /// Span of the flags after the closing slash (e.g., "gi"). Verbatim sub-slice
    /// of source; recovered via `flags(source)`.
    pub flags_span: Span,
    /// Visual width of the pattern (tab width 2), precomputed so the "simple
    /// call argument" width check stays source-free. Saturates at `u16::MAX`.
    pub pattern_width: u16,
}

impl RegexLiteral {
    /// The pattern between the slashes (a verbatim sub-slice of `source`).
    #[inline]
    pub fn pattern<'s>(&self, source: &'s str) -> &'s str {
        self.pattern_span.extract(source)
    }

    /// The flags after the closing slash (a verbatim sub-slice of `source`).
    #[inline]
    pub fn flags<'s>(&self, source: &'s str) -> &'s str {
        self.flags_span.extract(source)
    }
}

/// This expression: `this`
#[derive(Debug, Clone)]
pub struct ThisExpression;

/// Super expression: `super`
///
/// Used in class methods to reference the parent class:
/// - `super()` calls the parent constructor
/// - `super.method()` calls a parent method
/// - `super.prop` accesses a parent property
/// - `super[expr]` computed property access on parent
#[derive(Debug, Clone)]
pub struct Super;

/// TypeScript angle-bracket type assertion: `<Type>expr`
///
/// Old-style type assertion syntax. Equivalent to `expr as Type` but
/// incompatible with JSX (looks like a JSX element).
///
/// Example: `<string>someValue`, `<T>a`
#[derive(Debug, Clone)]
pub struct TSTypeAssertion {
    /// The target type
    pub type_annotation: TyId,
    /// The expression being type-asserted
    pub expression: ExprId,
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
    pub expression: ExprId,
    /// The target type
    pub type_annotation: TyId,
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
    pub expression: ExprId,
    /// The type to satisfy
    pub type_annotation: TyId,
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
    pub expression: ExprId,
    /// The type arguments: <T, U>
    pub type_arguments: TSTypeParameterInstantiation,
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
    pub expression: ExprId,
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
    ///
    /// `span` is the span of the `Expression` holding this node.
    pub fn seals_optional_chain(&self, view: AstView<'_>, span: Span) -> bool {
        span.start < view.expr(self.expression).span.start
            && view.expr(self.expression).has_optional_in_chain(view)
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
    pub left: ExprId,
    /// The operator: "=" for simple, "+=", "-=", etc. for compound
    pub operator: AssignmentOperator,
    /// The value being assigned
    pub right: ExprId,
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
