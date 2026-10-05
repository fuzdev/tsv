// Centralized parenthesization logic for TypeScript printer
//
// This module implements prettier's parenthesization system with a single entry point:
// `needs_parens(expr, ctx)` - determines if an expression needs parens in a given context.
//
// ## Architecture
//
// prettier's parenthesization (`parentheses/needs-parentheses.js`) works by:
// - Switching on the node type (expression being printed)
// - Each case examines the parent context and key (which child position)
// - Returns true if parens needed, false otherwise
//
// We model the "parent context + key" as a `ParenContext` enum.
//
// ## References
// - `needsParentheses` (`parentheses/needs-parentheses.js`) and its parent-side half
//   `parentNeedsParentheses` (`parentheses/parent-needs-parentheses.js`)
// - `printPathNoParens`'s caller (`print/index.js`, the application layer)

use crate::ast::internal::{
    ArrowFunctionBody, AssignmentOperator, BinaryExpression, BinaryOperator, Expression,
    ExpressionKind, LiteralValue, TSKeywordKind, TSType, TSTypeParameterInstantiation,
    UnaryOperator, UpdateOperator,
};
use crate::printer::chain::child_stops_optional_chain;
use crate::printer::class_expr_has_decorators;
use crate::printer::comments::{
    next_significant_byte, paren_shell_close_after, printed_left_spine_step,
};
use crate::printer::expressions::conditional::ternary_branch_needs_parens;
use smallvec::SmallVec;
use tsv_lang::Span;

/// Context for parenthesization decisions
///
/// This enum captures WHERE an expression appears in the AST, which determines
/// whether it needs parentheses.
#[derive(Debug, Clone, Copy)]
pub enum ParenContext {
    /// Variable declarator init: `const x = <expr>`
    VariableInit,

    /// `for`-in / `for`-of iterable: `for (const x of <expr>)`
    ///
    /// An assignment as a value takes clarity parens here exactly as it does at a
    /// declarator init. Prettier's `for` exemption for an assignment is keyed on
    /// `ForStatement` init/update — a C-style header's own clause expression — and does
    /// not reach `ForInStatement` / `ForOfStatement`.
    ForInOfRight,

    /// TypeScript export assignment value: `export = <expr>`
    ///
    /// The `export default` twin, minus its leftmost-token rule
    /// ([`export_default_needs_parens`]) — nothing here can reparse as a declaration.
    ExportAssignment,

    /// Enum member initializer: `enum E { A = <expr> }`
    ///
    /// Prettier's assignment rule parenthesizes by default and `TSEnumMember` is not among
    /// its exemptions, so an assignment value takes clarity parens here as at a declarator
    /// init (`A = (a = b)`).
    EnumMemberInit,

    /// Expression statement: `<expr>;`
    ExpressionStatement,

    /// Binary left operand: `<expr> + y`
    BinaryLeft { parent_op: BinaryOperator },

    /// Binary right operand: `x + <expr>`
    BinaryRight { parent_op: BinaryOperator },

    /// Callee position: `<expr>()` or tagged template tag: `<expr>`template``
    Callee,

    /// New expression callee: `new <expr>()`
    NewCallee,

    /// Tagged template tag: `` <expr>`template` ``
    ///
    /// Same precedence rules as `Callee`, plus an optional chain always needs
    /// parens here — an optional chain can't be a template tag per spec
    /// (`` a?.b`x` `` is a syntax error), so the parens seal it.
    TaggedTemplateTag,

    /// Base of member/call chain: `<expr>.method()`
    ChainBase,

    /// Inside TSNonNullExpression: `<expr>!`
    NonNull,

    /// Left side of `as` or `satisfies`: `<expr> as T`
    /// Only angle-bracket `<T>x` needs parens here (as/satisfies are left-associative)
    TypeAssertion,

    /// Expression in angle-bracket assertion: `<T><expr>`
    /// All type assertions need parens here
    AngleBracketAssertion,

    /// Expression in TSInstantiationExpression: `<expr><T>`. `source_pair` is the
    /// printer's answer where the SOURCE settles the pair, facts the printer holds and the
    /// node alone does not: an instantiation `<expr>` that keeps a pair ahead of this second
    /// list ([`instantiation_keeps_pair_before_type_args`], which reads whether this node's
    /// own owner continues its chain), or an optional chain an authored pair seals
    /// ([`instantiation_operand_is_sealed_chain`]).
    InstantiationExpression { source_pair: bool },

    /// Argument of a unary operator: `!<expr>`, `typeof <expr>`, `-<expr>`.
    /// Carries the parent operator so a `+`/`-` operand that would re-tokenize
    /// (`+(+x)` → `++x`, `-(--x)` → `---x`) gets parenthesized.
    UnaryArgument { parent_op: UnaryOperator },

    /// Argument of an update operator: `<expr>++`, `++<expr>`.
    /// An operand looser than a member access keeps its parens — bare `a as T++`
    /// binds `++` to `T`, `a * b++` binds it to `b`. `postfix` names which side the
    /// operator prints on: an instantiation operand keeps its parens only ahead of
    /// a postfix operator (`(f<T>)++`), the one placement where the operator would
    /// follow the type argument list.
    UpdateArgument { postfix: bool },

    /// Argument of await: `await <expr>`
    AwaitArgument,

    /// Argument of yield: `yield <expr>`
    /// Only AssignmentExpression needs parens (yield has lower precedence than binary/conditional)
    YieldArgument,

    /// Arrow function body (expression form): `() => <expr>`
    ArrowBody,

    /// Object property value: `{key: <expr>}`
    ObjectPropertyValue,

    /// Default value of a parameter/pattern (`(a = <expr>) =>`) or a class
    /// property value (`a = <expr>;`)
    DefaultValue,

    /// Spread element argument: `...<expr>`
    SpreadArgument,

    /// Call/array/new argument: `fn(<expr>)`, `[<expr>]`, `new Fn(<expr>)`
    /// Assignment expressions need parens for clarity
    Argument,

    /// Template literal expression: `${<expr>}`
    /// Assignment expressions need parens for clarity
    TemplateLiteralExpression,

    /// Computed `[<expr>]` bracket — an object/class computed property key
    /// (`{[<expr>]: value}`, `class C { [<expr>] }`) or a computed member-access
    /// index (`arr[<expr>]`, `obj?.[<expr>]`). An assignment expression is
    /// parenthesized for clarity in all three (a sequence self-parenthesizes).
    ComputedPropertyKey,

    /// Statement test condition: `if (<expr>)`, `while (<expr>)`, `for (;<expr>;)`,
    /// `do {} while (<expr>)` — and a switch **case** test (`case (<expr>):`), which
    /// asks the same question and takes the same answer.
    /// Assignment expressions need double-parens for clarity: `while ((x = y))`
    StatementTest,

    /// Superclass of a class heritage clause: `class C extends <expr> {}`
    /// Prettier wraps everything that isn't a bare identifier/member/call/literal
    /// (incl. `new`, tagged templates, and non-null, which are valid `extends`
    /// operands but still parenthesized for clarity).
    SuperClass,

    /// Left side of an assignment: `<expr> = …` / `<expr> += …`, whole or as a
    /// destructuring default's target. Every left the grammar's
    /// `LeftHandSideExpression` does not derive bare keeps a pair — see the arm in
    /// [`needs_parens`]. Non-null `x!` is valid bare, so it isn't wrapped (matches
    /// prettier). `operator` is the assignment's own (`=` for a destructuring default):
    /// an instantiation target keeps its pair only ahead of a `>`-led one.
    AssignmentTarget { operator: AssignmentOperator },
}

impl ParenContext {
    /// Whether a comparison written bare at this position would reach out of the operand's
    /// own extent — an operator ahead of it binding to its first operand, or a postfix
    /// after it joining its right side: the operand of a binary or a prefix operator, the
    /// left side of `as` / `satisfies`, an angle-bracket assertion's operand, and every
    /// left-spine position (a callee, a `new` callee, a tag, a member object, a `!`
    /// operand, an instantiation head, a heritage). One side of the operand is always
    /// the parent's own token here, which is what lets
    /// [`crate::printer::Printer::needs_parens`] read a `(`…`)` around it as the author's.
    pub(crate) fn binds_tighter_than_a_comparison(self) -> bool {
        matches!(
            self,
            Self::BinaryLeft { .. }
                | Self::BinaryRight { .. }
                | Self::Callee
                | Self::NewCallee
                | Self::TaggedTemplateTag
                | Self::ChainBase
                | Self::NonNull
                | Self::TypeAssertion
                | Self::AngleBracketAssertion
                | Self::InstantiationExpression { .. }
                | Self::UnaryArgument { .. }
                | Self::UpdateArgument { .. }
                | Self::AwaitArgument
                | Self::SuperClass
        )
    }
}

/// Whether `expr` is an `in` binary expression — the operator that must be
/// parenthesized inside a `for` header init so it isn't read as the `for (x in
/// y)` separator. Shared by `needs_parens` (the ambient for-init rule) and the
/// surgical `in`-wrap at positions that build an expression without a
/// `needs_parens` check.
pub(crate) fn is_in_binary(expr: &Expression<'_>) -> bool {
    matches!(&expr.kind, ExpressionKind::BinaryExpression(b) if b.operator == BinaryOperator::In)
}

/// A second type argument list that follows an instantiation expression's close
/// directly (`f<T><U>…`), with the node it belongs to — what
/// [`instantiation_keeps_pair_before_type_args`] reads to know how tsc takes the bare
/// spelling.
#[derive(Clone, Copy)]
pub(crate) enum SecondTypeArgs<'e> {
    /// A call's or a `new`'s list, with the argument list after it: `f<T><U>(x)`,
    /// `new f<T><U>(x)`. A `new` written without one prints an empty list.
    Arguments(
        &'e TSTypeParameterInstantiation<'e>,
        &'e [&'e Expression<'e>],
    ),
    /// A tag's list: `` f<T><U>`x` ``.
    Template(&'e TSTypeParameterInstantiation<'e>),
    /// Another instantiation's list (`f<T><U>`). `continues` is whether that
    /// instantiation's OWN owner — the next list along, a call, a `new` or a tag — prints
    /// it bare (`new f<T><U><V>(x)`), so the chain goes on and that owner answers for the
    /// rest of it; with none, the assertion `<U>` has nothing to assert.
    Instantiation {
        list: &'e TSTypeParameterInstantiation<'e>,
        continues: bool,
    },
    /// A class heritage's list (`extends (f<T>)<U>`), whose bare spelling no parser but
    /// acorn-typescript takes: tsc and tsv read `extends f<T>` and stop at the `<`.
    Heritage,
}

impl SecondTypeArgs<'_> {
    /// Whether tsc's parser rejects the bare spelling. tsc takes no type argument list
    /// ahead of a `<`, so it reads `f<T><U>(x)` as the comparison `(f < T) > <U>(x)`: the
    /// second list is an angle-bracket type assertion — one type, no trailing comma —
    /// over whatever follows it, and a parenthesized expression holds neither an empty
    /// list nor a spread.
    fn bare_is_refused(self, source: &str) -> bool {
        let list = match self {
            Self::Arguments(list, _) | Self::Template(list) | Self::Instantiation { list, .. } => {
                list
            }
            Self::Heritage => return true,
        };
        !asserts_one_type(source, list)
            || match self {
                Self::Arguments(_, arguments) => {
                    arguments.is_empty()
                        || arguments.iter().any(|argument| {
                            matches!(argument.kind, ExpressionKind::SpreadElement(_))
                        })
                }
                Self::Instantiation { continues, .. } => !continues,
                Self::Template(_) | Self::Heritage => false,
            }
    }
}

/// Whether a type argument list reads as an angle-bracket type assertion's `<T>` to tsc:
/// exactly one type, with no trailing comma.
fn asserts_one_type(source: &str, list: &TSTypeParameterInstantiation<'_>) -> bool {
    match list.params {
        [only] => next_significant_byte(source, only.span().end, list.span.end)
            .is_none_or(|pos| source.as_bytes()[pos] != b','),
        _ => false,
    }
}

/// Whether tsc takes the bare spelling of the unparenthesized instantiation chain `head`
/// ends, as far as the chain itself decides: every list past the first reads as an
/// assertion (`f<T><U, V>` does not), and the FIRST — which tsc reads as the right operand
/// of a `<` comparison — reads as an expression ([`first_list_reads_as_expression`]). The
/// chain's owner answers for what follows it.
fn chain_reads_bare_under_tsc(source: &str, head: &Expression<'_>) -> bool {
    let mut level = head;
    while let ExpressionKind::TSInstantiationExpression(inst) = &level.kind {
        let inner = inst.expression;
        if holds_first_list_of_chain(source, inner) {
            return first_list_reads_as_expression(source, &inst.type_arguments);
        }
        if !asserts_one_type(source, &inst.type_arguments) {
            return false;
        }
        level = inner;
    }
    true
}

/// Whether the instantiation expression over `inner` carries the FIRST type argument list of
/// its chain: `inner` is no instantiation itself, or it is one an authored paren shell closes
/// (`(f<T>)<U>` starts a chain of its own at `<U>`).
pub(crate) fn holds_first_list_of_chain(source: &str, inner: &Expression<'_>) -> bool {
    !matches!(inner.kind, ExpressionKind::TSInstantiationExpression(_))
        || paren_shell_close_after(source, inner.span().end).is_some()
}

/// Whether tsc's comparison reading of a chain's first list (`f<T>…` read as `f < T > …`)
/// can take its text as an expression: no trailing comma (`f < T, >` does not parse), and
/// no type whose syntax no expression shares ([`type_is_never_an_expression`]). Only a
/// definite refusal counts — a type whose text may parse as an expression (a type literal,
/// a mapped type, a spread, `void[]`, a parenthesized function type) leaves the spelling
/// bare, since a pair added where tsc DID read the comparison would change the program tsc
/// reads.
fn first_list_reads_as_expression(source: &str, list: &TSTypeParameterInstantiation<'_>) -> bool {
    let trailing_comma = list.params.last().is_some_and(|last| {
        next_significant_byte(source, last.span().end, list.span.end)
            .is_some_and(|pos| source.as_bytes()[pos] == b',')
    });
    !trailing_comma && !list.params.iter().any(|ty| type_is_never_an_expression(ty))
}

/// Whether a type's text can never parse as the operand of a `<` comparison: an array type
/// (`T[]` indexes nothing) other than `void[]` (the unary `void []`), a type operator whose operand opens with neither `[` nor `(`
/// (`keyof T`), a conditional (`extends`), a bare
/// function or constructor type (an arrow is no relational operand), `void` with nothing
/// to apply to, an optional or named tuple member, and any type built on one of those. A
/// spread (`[...A]`, an array-literal element) and a PARENTHESIZED arrow (`(() => T)`, a
/// primary expression) are operands, so only what they are built on decides.
fn type_is_never_an_expression(ty: &TSType<'_>) -> bool {
    let never = type_is_never_an_expression;
    match ty {
        // A type operator is an expression when its operand opens with `[` or `(`
        // (`keyof [A]` indexes `keyof`, `keyof (A)` calls it), so only that operand decides
        // — read AS PRINTED ([`operator_operand_prints_opening_group`]).
        TSType::TypeOperator(op) => {
            !operator_operand_prints_opening_group(op.type_annotation) || never(op.type_annotation)
        }
        // `void[]` is the unary `void []` to tsc, so only an array over the `void` keyword
        // as written is an operand.
        TSType::Array(array) => !matches!(
            array.element_type,
            TSType::Keyword(keyword) if matches!(keyword.kind, TSKeywordKind::Void)
        ),
        TSType::Conditional(_)
        | TSType::Function(_)
        | TSType::Constructor(_)
        | TSType::Optional(_)
        | TSType::NamedTupleMember(_)
        | TSType::Infer(_)
        | TSType::TypePredicate(_) => true,
        TSType::Keyword(keyword) => matches!(keyword.kind, TSKeywordKind::Void),
        TSType::Union(union) => union.types.iter().any(|t| never(t)),
        TSType::Intersection(intersection) => intersection.types.iter().any(|t| never(t)),
        TSType::Tuple(tuple) => tuple.element_types.iter().any(|t| never(t)),
        TSType::Rest(rest) => never(rest.type_annotation),
        TSType::Parenthesized(inner) => {
            !matches!(inner.type_annotation, TSType::Function(_)) && never(inner.type_annotation)
        }
        TSType::IndexedAccess(access) => {
            never(access.object_type) || index_is_never_an_expression(access.index_type)
        }
        TSType::TypeReference(reference) => reference
            .type_arguments
            .as_ref()
            .is_some_and(|list| list.params.iter().any(|t| never(t))),
        _ => false,
    }
}

/// [`type_is_never_an_expression`] for an indexed access's INDEX — the one position in a
/// type's text where the comparison reading takes a whole expression rather than an
/// operand, so a function type there is an arrow FUNCTION (`f<A[() => B]><U>(x)` is
/// `f < A[() => B] > <U>(x)` to tsc, an arrow indexing `A`), and only what stands where its
/// body would decides: `A[() => B[]]` and `A[(a) => a is B]` return nothing an expression
/// spells.
fn index_is_never_an_expression(index: &TSType<'_>) -> bool {
    match index {
        TSType::Function(function) => {
            index_is_never_an_expression(function.return_type.type_annotation)
        }
        _ => type_is_never_an_expression(index),
    }
}

/// Whether a type operator's operand, as the first list of a chain printed bare prints it,
/// opens with `[` or `(`: a tuple; a paren the author wrote directly under the operator,
/// which that position keeps ([`crate::printer::Printer::first_list_keeps_maybe_parens`]);
/// a pair the prefix-operator rule adds (`readonly (readonly [A])`); or, down the left
/// spine of an array or indexed-access operand, a tuple or a pair that position's own rule
/// keeps. A redundant paren further down is stripped (`keyof (A)[0]` prints
/// `keyof A[0]`), so it does not count.
fn operator_operand_prints_opening_group(operand: &TSType<'_>) -> bool {
    match operand {
        TSType::Parenthesized(_) | TSType::Tuple(_) => true,
        _ if type_takes_pair_under_postfix_or_operator(operand) => true,
        TSType::Array(array) => leftmost_prints_opening_group(array.element_type),
        TSType::IndexedAccess(access) => leftmost_prints_opening_group(access.object_type),
        _ => false,
    }
}

/// [`operator_operand_prints_opening_group`] below an array's element or an indexed
/// access's object, where an authored paren survives only if that position needs it.
fn leftmost_prints_opening_group(ty: &TSType<'_>) -> bool {
    match ty {
        TSType::Parenthesized(inner) => {
            takes_pair_under_postfix(inner.type_annotation)
                || leftmost_prints_opening_group(inner.type_annotation)
        }
        TSType::Tuple(_) => true,
        _ if takes_pair_under_postfix(ty) => true,
        TSType::Array(array) => leftmost_prints_opening_group(array.element_type),
        TSType::IndexedAccess(access) => leftmost_prints_opening_group(access.object_type),
        _ => false,
    }
}

/// The types the type printer wraps in a pair under an array's `[]` or an indexed access's
/// `[K]`: the prefix-operator set plus a `typeof` query.
fn takes_pair_under_postfix(ty: &TSType<'_>) -> bool {
    type_takes_pair_under_postfix_or_operator(ty) || matches!(ty, TSType::TypeQuery(_))
}

/// The types the type printer wraps in a pair under a prefix type operator — the shared
/// core of that rule and the array / indexed-access ones.
fn type_takes_pair_under_postfix_or_operator(ty: &TSType<'_>) -> bool {
    matches!(
        ty,
        TSType::Union(_)
            | TSType::Intersection(_)
            | TSType::TypeOperator(_)
            | TSType::Conditional(_)
            | TSType::Infer(_)
            | TSType::Function(_)
            | TSType::Constructor(_)
    )
}

/// Whether an instantiation expression `head` keeps a paren pair ahead of the second type
/// argument list that follows it, `follow`.
///
/// acorn-typescript — the parser Svelte compiles through — reads the bare `f<T><U>(x)` as
/// two lists, the same tree as the paired `(f<T>)<U>(x)`; tsc reads the bare spelling as a
/// comparison. So the printer changes neither parser's reading: an authored pair is kept
/// (it is what tsc read), a bare spelling tsc accepts prints bare (tsc read the
/// comparison, and still does), and a bare spelling tsc REJECTS takes the pair, the one
/// form both parsers read alike — the repair `fn<T> >= 1` gets.
///
/// The one exception is a head that ends an OPEN optional chain (`a?.b<T><U>()`): a pair
/// there would cut the chain in two — acorn reads the whole call short-circuited, the
/// paired `(a?.b<T>)<U>()` calls whatever the chain produced — and no paren-free spelling
/// tsc accepts exists, so the bare spelling stays, acorn's reading intact and tsc
/// rejecting it as it did the input. A heritage's list is outside the exception: nothing
/// follows it for the pair to cut away from the chain.
pub(crate) fn instantiation_keeps_pair_before_type_args(
    source: &str,
    head: &Expression<'_>,
    follow: SecondTypeArgs<'_>,
) -> bool {
    matches!(head.kind, ExpressionKind::TSInstantiationExpression(_))
        && (paren_shell_close_after(source, head.span().end).is_some()
            || ((follow.bare_is_refused(source) || !chain_reads_bare_under_tsc(source, head))
                && (matches!(follow, SecondTypeArgs::Heritage) || !ends_open_optional_chain(head))))
}

/// Whether an instantiation expression starting at `inst_start` instantiates an optional
/// chain the author SEALED with a pair — `(a?.b)<T>`, or its non-null spelling
/// `(a?.b)!<T>` — which the printer keeps.
///
/// The pair is where the chain ends. Bare, the type arguments join the chain
/// (`a?.b<T>`), and so does whatever follows the instantiation: a tag or a `new` callee
/// written that way is a syntax error (`` a?.b<T>`t` ``, `new a?.b()<T>()`), and a plain
/// call joins it too (`a?.b<T>()` short-circuits the call). Where nothing follows, or an
/// optional link does, the two spellings behave alike at runtime and type alike to tsc, but
/// acorn-typescript — whose tree Svelte compiles — reads them as two trees
/// (`((a?.b)<T>)?.()` printed `a?.b<T>?.()` is one chain where the author wrote two).
pub(crate) fn instantiation_operand_is_sealed_chain(
    inst_start: u32,
    operand: &Expression<'_>,
) -> bool {
    child_stops_optional_chain(inst_start, false, operand) || operand.is_sealing_non_null()
}

/// Whether the instantiation chain `head` is built on an optional chain no authored pair
/// has sealed (`a?.b<T>`, not `(a?.b)<T>`).
fn ends_open_optional_chain(head: &Expression<'_>) -> bool {
    let mut level = head;
    while let ExpressionKind::TSInstantiationExpression(inst) = &level.kind {
        if level.span().start < inst.expression.span().start {
            return false;
        }
        level = inst.expression;
    }
    level.has_optional_in_chain()
}

/// Whether tsc reads `expr` as a comparison where acorn-typescript reads a call, a `new`,
/// a tag or an instantiation: its left spine holds a second type argument list printed
/// bare ([`instantiation_keeps_pair_before_type_args`]), so to tsc the first list is a
/// `<`…`>` comparison and everything after it is the operand of the assertion the second
/// list opens (`f<T><U>(x)` is `(f < T) > <U>(x)`).
///
/// Such an expression reaches out of its own extent in a tsc reading: an operator ahead of
/// it binds to `f` alone, and a postfix after it joins the assertion's operand. So a pair
/// the author wrote around one must survive, wherever the position would otherwise strip
/// it ([`crate::printer::Printer::needs_parens`]). An authored pair on the spine below stops the
/// walk: that pair survives by the same rule and ends the comparison inside it.
pub(crate) fn prints_as_tsc_comparison(source: &str, expr: &Expression<'_>) -> bool {
    let bare_owner = |head: &Expression<'_>, follow: Option<SecondTypeArgs<'_>>| {
        matches!(head.kind, ExpressionKind::TSInstantiationExpression(_))
            && follow.is_some_and(|follow| {
                !instantiation_keeps_pair_before_type_args(source, head, follow)
            })
    };
    let descend = |child: &Expression<'_>| {
        expr.span().start == child.span().start && prints_as_tsc_comparison(source, child)
    };
    match &expr.kind {
        ExpressionKind::CallExpression(call) => {
            bare_owner(
                call.callee,
                call.type_arguments
                    .as_ref()
                    .map(|list| SecondTypeArgs::Arguments(list, call.arguments)),
            ) || descend(call.callee)
        }
        ExpressionKind::NewExpression(new_expr) => {
            bare_owner(
                new_expr.callee,
                new_expr
                    .type_arguments
                    .as_ref()
                    .map(|list| SecondTypeArgs::Arguments(list, new_expr.arguments)),
            ) || (next_significant_byte(
                source,
                expr.span().start + "new".len() as u32,
                new_expr.callee.span().start,
            )
            .is_none()
                && prints_as_tsc_comparison(source, new_expr.callee))
        }
        ExpressionKind::TaggedTemplateExpression(tagged) => {
            bare_owner(
                tagged.tag,
                tagged.type_arguments.as_ref().map(SecondTypeArgs::Template),
            ) || descend(tagged.tag)
        }
        ExpressionKind::MemberExpression(member) => descend(member.object),
        ExpressionKind::TSNonNullExpression(non_null) => descend(non_null.expression),
        ExpressionKind::TSInstantiationExpression(inst) => descend(inst.expression),
        _ => false,
    }
}

/// Determines if an expression needs parentheses in a given context.
///
/// This is the central entry point for all parenthesization decisions.
///
/// `in_for_init` is the ambient "building a `for` header init clause" flag: when
/// set, an `in` binary expression always needs parens (prettier parenthesizes
/// every `in` lexically under the init, regardless of context). It's threaded as
/// a parameter rather than read from a context because parenthesization is a pure
/// function of the node and its surroundings.
pub fn needs_parens(expr: &Expression<'_>, ctx: ParenContext, in_for_init: bool) -> bool {
    // Ambient for-init rule: an `in` binary always needs parens here. ORed ahead
    // of the context match so it applies uniformly (call args, object values,
    // binary operands, etc.) and never double-wraps a node a context already
    // parenthesizes for precedence (`!(a in b)`, `(a in b).p`).
    if in_for_init && is_in_binary(expr) {
        return true;
    }
    match ctx {
        // Clarity parens around an assignment, and nothing else: prettier's
        // `AssignmentExpression` rule, default-true ([`assignment_value_needs_parens`]).
        // A sequence supplies its own pair, and every other operand at these positions
        // (`??`, `as`, a ternary, `await`) is bare in prettier too.
        // - a declarator init, a `for`-in/of iterable, `export =`, an enum member:
        //   `const x = (y = z);`, `for (const x of (y = z))`, `export = (y = z);`,
        //   `enum E { A = (y = z) }`
        // - an object property value (not in an `ObjectPattern`, which the caller never
        //   asks): `{key: (a = b)}`
        // - a parameter / pattern default or a class property value:
        //   `(a = (b = c)) =>`, `a = (this.a = b);`
        // - a call / array / `new` argument, a template literal expression, a computed
        //   key or index: `fn((a = b))`, `[(a = b)]`, `${(a = b)}`, `{[(a = b)]: c}`
        // - a `yield` argument (unlike `await`, `yield` is looser than a binary or a
        //   ternary, so those stay bare): `yield (x ??= y)`
        // - a statement test, where the doubled pair signals an intentional assignment
        //   rather than a typo for `==`: `while ((x = y))`, `if ((x = getValue()))`
        ParenContext::VariableInit
        | ParenContext::ForInOfRight
        | ParenContext::ExportAssignment
        | ParenContext::EnumMemberInit
        | ParenContext::ObjectPropertyValue
        | ParenContext::DefaultValue
        | ParenContext::Argument
        | ParenContext::TemplateLiteralExpression
        | ParenContext::ComputedPropertyKey
        | ParenContext::YieldArgument
        | ParenContext::StatementTest => assignment_value_needs_parens(expr),

        // Object pattern assignment needs parens: `({a} = x);`
        ParenContext::ExpressionStatement => needs_parens_expression_statement(expr),

        // Binary operand precedence
        ParenContext::BinaryLeft { parent_op } => {
            needs_parens_binary_operand(expr, parent_op, false, in_for_init)
        }
        ParenContext::BinaryRight { parent_op } => {
            needs_parens_binary_operand(expr, parent_op, true, in_for_init)
        }

        // Callee: `(a ? b : c)()`, `(a + b)()`, `(() => {})()`, `(x as T)()`, `(<T>x)()`, etc.
        // TaggedTemplateTag (`(x as T)`template``) shares these precedence rules; both it
        // and NewCallee add the optional-chain rule below.
        // Note: SequenceExpression already adds its own parens in build_sequence_doc
        // Note: ClassExpression needs parens only in NewCallee: `class {}()` is valid but `new class {}()` is not
        ParenContext::Callee | ParenContext::NewCallee | ParenContext::TaggedTemplateTag => {
            if matches!(ctx, ParenContext::NewCallee) {
                // ClassExpression only needs parens in `new` context
                if matches!(expr.kind, ExpressionKind::ClassExpression(_)) {
                    return true;
                }
                // A `new` callee holding a call on its left spine that no pair encloses
                // takes one too, so the arguments bind to the `new` rather than to the
                // inner call (`new (f())()`). That rule reads which pairs the printer keeps
                // for a reason the source settles, so it is the printer's to answer
                // (`Printer::new_callee_holds_bare_call`), not this function's.
            }
            // A `new` callee or template tag may NOT be an (unsealed) optional chain
            // per spec — `new a?.b()` / `` a?.b`x` `` are syntax errors. So the parens
            // are *always* required (unlike the boundary-dependent member/call/non-null
            // cases, which depend on what follows the chain). The plain call `Callee`
            // context is excluded: `(a?.b)()` strips to the valid `a?.b()`. A non-null
            // assertion that seals the chain (`(a?.b)!`) is handled by the sealed-base
            // rendering, not here (`has_optional_in_chain` returns false for it). An
            // instantiation of an open chain is still that chain (`new (a?.b<T>)()`,
            // `` (a?.b()?.k<T>)`t` ``), so the test reads through its lists
            // ([`ends_open_optional_chain`]).
            if matches!(
                ctx,
                ParenContext::NewCallee | ParenContext::TaggedTemplateTag
            ) && ends_open_optional_chain(expr)
            {
                return true;
            }
            is_await_or_yield(expr)
                || is_type_assertion(expr)
                || is_function_like(expr)
                || is_unary_or_update(expr)
                || matches!(
                    expr.kind,
                    ExpressionKind::ConditionalExpression(_)
                        | ExpressionKind::BinaryExpression(_)
                        | ExpressionKind::AssignmentExpression(_)
                )
        }

        // Chain base: `(a + b).method()`, `(await x).method()`, `(yield x).method()`, etc.
        // Numeric literals need parens for `.method()` calls: `0.toString()` is invalid syntax.
        // Prettier normalizes `0..toString()` to `(0).toString()`.
        //
        // Update/unary expressions and arrow functions as a member-access object
        // also need parens: `(++c).p`, `(-a).p`, `(!a).p`, `(typeof a).p`,
        // `(() => 1).p`. Without them the prefix operator binds to the member
        // access (`-a.p` is `-(a.p)`) or the arrow body absorbs it (`() => 1.p` is
        // an arrow returning `1.p`). Function/class/object expressions do NOT need
        // them — their brace-delimited bodies make the parens redundant, and
        // prettier strips them (`(function () {}).p` → `function () {}.p`).
        //
        // An instantiation expression does: a `.`/`?.` after a type argument list is
        // rejected outright (`A<T>.x`, tsc's "cannot be followed by a property access"),
        // a `[` re-lexes it as a relational chain (`A<T>[0]` is `(A < T) > [0]`), and
        // dropping the type args instead would be data loss — `(A<T>).x` keeps the pair
        // (prettier agrees for a member object). The chain linearizer reads this
        // verdict for its base node, so an instantiation reached as a member object or a
        // `!` operand becomes a parenthesized base by the same rule.
        ParenContext::ChainBase => {
            is_lower_precedence(expr)
                || is_numeric_literal(expr)
                || is_unary_or_update(expr)
                || matches!(
                    expr.kind,
                    ExpressionKind::ArrowFunctionExpression(_)
                        | ExpressionKind::TSInstantiationExpression(_)
                )
        }

        // Spread argument: `...(a || b)`, `...(a ? b : c)`, `...(await x)`, `...(x as T)`
        ParenContext::SpreadArgument => is_lower_precedence(expr),

        // Non-null: `(a + b)!`, `(!x)!`, `(a ? b : c)!`, `(yield x)!`, `(++x)!`, etc.
        // UpdateExpression needs parens too: `(++x)!` is `NonNull(++x)`, but `++x!`
        // parses as `++(x!)` (`Update(NonNull)`) — a different AST. An arrow function
        // needs parens as well (`((a) => a)!` — bare `(a) => a!` is `(a) => (a!)`, and
        // a block-body `(a) => {}!` can't postfix the arrow at all → unreparseable).
        // Function/class expressions don't: their brace-delimited bodies make the
        // parens redundant, and prettier strips them. An instantiation expression
        // does: a `!` starts an expression, so it cannot follow a type argument list
        // (`f<T>!` does not parse — tsc's `canFollowTypeArgumentsInExpression`), and
        // prettier's bare `f<T>!` is a cataloged ◆prettier_bug.
        ParenContext::NonNull => {
            is_lower_precedence(expr)
                || is_unary_or_update(expr)
                || matches!(
                    expr.kind,
                    ExpressionKind::ArrowFunctionExpression(_)
                        | ExpressionKind::TSInstantiationExpression(_)
                )
        }

        // Type assertion (as/satisfies): `(a + b) as T`, `(await x) as T`, `(<U>x) as T`
        // Arrow functions need parens because `(...args) => x as T` parses as `(...args) => (x as T)`
        // Ternary/assignment need parens: `(a ? b : c) as T` vs `a ? b : c as T` (different semantics)
        // Only angle-bracket assertions need parens here (as/satisfies are left-associative)
        //
        // One more pair is kept by the JOIN of two tokens rather than by kind — the
        // instantiation-tail rule of the binary-left arm, with the keyword as the follower:
        // an operand whose last printed token is a type argument list's `>` keeps its pair
        // (`(f<T>) as T`, `(-f<T>) satisfies T`). tsc takes the list ahead of any binary
        // operator, `as` and `satisfies` included, but acorn-typescript — the parser Svelte
        // compiles with — gives it up ahead of any token that can start an expression on the
        // same line, and a word can: bare, `f<T> as T` does not parse there (nor under tsv's
        // own parser), and `f<T> as [T]` is the comparison `f < T > as[T]`. Prettier strips
        // it ("Instantiation expression parens").
        ParenContext::TypeAssertion => {
            is_await_or_yield(expr)
                || ends_with_instantiation_close(expr, in_for_init)
                || matches!(
                    expr.kind,
                    ExpressionKind::BinaryExpression(_)
                        | ExpressionKind::ConditionalExpression(_)
                        | ExpressionKind::AssignmentExpression(_)
                        | ExpressionKind::ArrowFunctionExpression(_)
                        | ExpressionKind::TSTypeAssertion(_)
                )
        }

        // Angle-bracket assertion: `<T>(a + b)`, `<T>(<U>x)`, `<T>(x as U)`, `<T>(a ? b : c)`
        // Both need parens for: await/yield, all type assertions, binary, conditional, assignment, arrow
        ParenContext::AngleBracketAssertion => needs_parens_unary_arg_common(expr),

        // Unary argument: `!(a + b)`, `!(await x)`, `!(<T>x)`, `typeof (a ? b : c)`.
        // The shared rule, plus the `+`/`-` same-sign guard so an operand that
        // would re-tokenize with the parent (`+(+x)`, `-(--x)`, `+(++x)`) is
        // parenthesized (prettier's UnaryExpression/UpdateExpression cases).
        // An operand led by a *forward-binding* comment (a JSDoc cast, a bundler
        // annotation) also takes a wrapping pair — `!(/** @type {A} */ (x).y)`, not
        // `!/** @type {A} */ (x).y` — because bare, the comment reads as annotating the
        // operator rather than the operand. That is NOT decided here: the comment sits in
        // the gap between the operator and the operand, so `build_unary_doc` sees it
        // positionally and adds the parens itself. Deciding it here too would double-wrap.
        ParenContext::UnaryArgument { parent_op } => {
            needs_parens_unary_arg_common(expr) || needs_parens_unary_same_sign(expr, parent_op)
        }

        // Update argument: `(a * b)++`, `(-b)++`, `(a = b)++`, `(a as T)++`, `(f<T>)++`.
        //
        // The same shape as `NonNull` above, for the same reason: an update operator
        // binds on its operand exactly as `!` does, so every operand looser than a
        // member access takes the pair (bar a sequence, looser still — `build_sequence_doc`
        // prints its own). Bare, the operator captures the wrong operand
        // (`a * b++` is `a * (b++)`, `-b++` is `-(b++)`, `(a) => a++` is an arrow whose
        // body is the update) — a different tree, and at the postfix spelling one the
        // author could not have written, since the operand's grammar there is a
        // `LeftHandSideExpression`. The parser ACCEPTS these: an invalid update target
        // is a deferred early error (the "Assigning to rvalue" acorn reports), so the
        // printer has to print the tree it was handed. The prefix operand's grammar is
        // the wider `UnaryExpression`, which admits a unary, another update and an
        // `await` bare — but none of those is a valid target either, so the one rule
        // costs only a redundant pair on code no program can run.
        //
        // The instantiation half is postfix-only: a `++` starts an expression, so it
        // cannot follow a type argument list (tsc's `canFollowTypeArgumentsInExpression`)
        // and bare `f<T>++` does not parse at all, while a prefix `++f<T>` leaves nothing
        // after the `>` — so `++(f<T>)` still strips. A BARE instantiation is the only
        // operand this clause can ever see: the precedence clauses ahead of it already
        // parenthesize every composite operand, so the join axis — the operand's
        // rightmost printed token, `ends_with_instantiation_close` — is read only where a
        // composite operand can reach it bare (a joining binary left, the left of `as` /
        // `satisfies`, a `>`-led or `/=` assignment target).
        ParenContext::UpdateArgument { postfix } => {
            is_lower_precedence(expr)
                || is_unary_or_update(expr)
                || matches!(expr.kind, ExpressionKind::ArrowFunctionExpression(_))
                || (postfix && matches!(expr.kind, ExpressionKind::TSInstantiationExpression(_)))
        }

        // Instantiation: `(<T>() => {})<U>`, `(x as A)<T>`, `(<T>x)<U>`, `(await x)<T>`, `(a = b)<T>`
        // Ternary/binary/assignment need parens to preserve semantics:
        // `(a ? b : c)<T>` vs `a ? b : c<T>` (different - ternary result vs alternate instantiated)
        // A unary or update operand does too: bare, the operator takes the instantiation
        // (`typeof a<T>` is `typeof (a<T>)`, `-a<T>` is `-(a<T>)`), and a postfix update
        // has no bare spelling at all (`a++<T>` does not parse).
        // An instantiation instantiated again takes the pair the printer decided.
        ParenContext::InstantiationExpression { source_pair } => {
            source_pair
                || is_await_or_yield(expr)
                || is_type_assertion(expr)
                || is_function_like(expr)
                || is_unary_or_update(expr)
                || matches!(
                    expr.kind,
                    ExpressionKind::ConditionalExpression(_)
                        | ExpressionKind::AssignmentExpression(_)
                        | ExpressionKind::BinaryExpression(_)
                )
        }

        // Await argument: `await (a + b)`, `await (x as T)`, `await (<T>x)`, `await (a ? b : c)`
        // Parens needed for precedence/semantics - await has higher precedence than ?:
        // Assignment: `await (x ??= y)` — without parens, parses as `(await x) ??= y` (syntax error)
        // `await` takes a UnaryExpression operand, so the lower-precedence yield and
        // arrow forms need parens too: `await (yield x)` (bare `await yield x` is a
        // syntax error), `await (() => {})` (bare `await () => {}` is invalid).
        ParenContext::AwaitArgument => {
            is_type_assertion(expr)
                || matches!(
                    expr.kind,
                    ExpressionKind::BinaryExpression(_)
                        | ExpressionKind::ConditionalExpression(_)
                        | ExpressionKind::AssignmentExpression(_)
                        | ExpressionKind::YieldExpression(_)
                        | ExpressionKind::ArrowFunctionExpression(_)
                )
        }

        // Arrow body: `() => ({})`, `() => (x = y)`, `() => (@dec class {})`
        // Note: ConditionalExpression is handled specially in build_arrow_body_doc
        // using if_break - parens only when inline, not when on new line
        ParenContext::ArrowBody => match &expr.kind {
            ExpressionKind::ObjectExpression(_) | ExpressionKind::AssignmentExpression(_) => true,
            // A DECORATED class expression cannot open a concise body: `@` is not a token
            // `ConciseBody` admits, so `() => @dec class {}` does not reparse. An
            // undecorated `class {}` opens one fine and stays bare, as prettier keeps it.
            // The same leftmost-token question a composite body asks through
            // `leftmost_arrow_body_parens_span`, at the body's own root.
            ExpressionKind::ClassExpression(c) => class_expr_has_decorators(c),
            _ => false,
        },

        // Superclass: `extends (a + b)`, `extends (a ? b : c)`, `extends (await x)`,
        // `extends ((a) => b)`, `extends (x as T)`, `extends (-x)`. The
        // lower-precedence and unary/update forms cover the operator cases; beyond
        // those prettier also parenthesizes `new`, tagged templates, and a bare object
        // (which would otherwise be read as the class body) — all valid `extends`
        // operands it still wraps for clarity. Bare identifiers, member/call chains,
        // literals, untagged templates, and `class`/`function` expressions stay
        // unparenthesized. `SequenceExpression` is absent because `build_sequence_doc`
        // already adds its own parens.
        //
        // Prettier first strips the chain-element wrappers (non-null `!` — and, in
        // ESTree, the `ChainExpression` optional-chain wrapper, which tsv folds into
        // member/call nodes) and tests the *inner* expression (#18652): a lone
        // `extends (Base!)` drops to `extends Base!`, while `extends (new Base()!)`
        // keeps the parens because the stripped `new` still wraps.
        ParenContext::SuperClass => {
            let stripped = strip_non_null_wrappers(expr);
            is_lower_precedence(stripped)
                || is_unary_or_update(stripped)
                || matches!(
                    stripped.kind,
                    ExpressionKind::ArrowFunctionExpression(_)
                        | ExpressionKind::NewExpression(_)
                        | ExpressionKind::TaggedTemplateExpression(_)
                        | ExpressionKind::ObjectExpression(_)
                )
                // A *decorated* class expression must be parenthesized —
                // `extends @deco class {}` reads the `@deco` as decorating the
                // enclosing class and the inner `class {}` as the heritage body,
                // producing unreparseable output. A bare (undecorated) class
                // expression stays unwrapped (prettier keeps `extends class {}`).
                || matches!(
                    &stripped.kind,
                    ExpressionKind::ClassExpression(c) if class_expr_has_decorators(c)
                )
        }

        // An assignment's left must be a `LeftHandSideExpression`; a target that is not
        // one bare parsed only because the author parenthesized it (the deferred
        // `AssignmentTargetType` early error — tsc's parser accepts the pair), so the
        // pair is load-bearing and stays, by KIND, under every assignment operator:
        // - a type assertion (`(x as T) = …`; bare `x as T = 1` is a tsc parse error);
        // - an operator expression — unary, update, `await`, binary, logical — whose
        //   bare reprint (`-a = 1`) is a grammar error;
        // - an alternative of `AssignmentExpression` itself — conditional, arrow,
        //   `yield` — whose bare reprint absorbs the `=` (`a ? b : c = 1` is
        //   `a ? b : (c = 1)`), a different program;
        // - a function expression, whose body tsc's parser does not continue past into
        //   an `=` (TS2809 on `x = function () {} = 1`); a class expression it does, so
        //   that one stays bare.
        // Prettier strips all but the assertions (docs/conformance_prettier_ts.md
        // §TypeScript, "Non-LHS assignment target parens"). Non-null `x!` is a valid
        // bare target, so it isn't wrapped.
        //
        // One more pair is kept by the JOIN of two tokens rather than by kind — the
        // instantiation-tail rule of the binary-left arm, one operator family over: a
        // target whose last printed token is a type argument list's `>` keeps its pair
        // ahead of a `>`-led `>>=` / `>>>=`, before which tsc's grammar takes no type
        // argument list (`f<T> >>= c` does not parse; `(f<T>) >>= c` does), and ahead of a
        // `/=`, which can start a regular expression and so refuses the list on the same
        // line (`f<T>⏎/= c` parses; the joined `f<T> /= c` does not). Prettier strips it
        // too ("Instantiation expression parens").
        ParenContext::AssignmentTarget { operator } => {
            (matches!(
                operator,
                AssignmentOperator::RightShiftAssign
                    | AssignmentOperator::UnsignedRightShiftAssign
                    | AssignmentOperator::DivideAssign
            ) && ends_with_instantiation_close(expr, in_for_init))
                || is_type_assertion(expr)
                || matches!(
                    expr.kind,
                    ExpressionKind::UnaryExpression(_)
                        | ExpressionKind::UpdateExpression(_)
                        | ExpressionKind::AwaitExpression(_)
                        | ExpressionKind::BinaryExpression(_)
                        | ExpressionKind::ConditionalExpression(_)
                        | ExpressionKind::ArrowFunctionExpression(_)
                        | ExpressionKind::YieldExpression(_)
                        | ExpressionKind::FunctionExpression(_)
                )
        }
    }
}

//
// Simple predicates (expression type groupings)
//

/// Strip trailing non-null assertion (`!`) wrappers, returning the inner expression.
///
/// tsv's mirror of prettier's `stripChainElementWrappers`: tsv has no distinct
/// `ChainExpression` node (optional chains fold into member/call), so only the
/// non-null wrapper needs unwrapping. Shared by the `extends`-clause paren decision
/// (#18652: `extends (Base!)` → `extends Base!`, the `!` binds tightly so the
/// heritage paren is redundant) and the call-arg arrow-body check
/// (`arrow_body_is_call_through_non_null`, `couldExpandArg`'s
/// `isCallExpression(stripChainElementWrappers(body))` — `=> fn()!` is a call body
/// that hugs the open paren).
pub(in crate::printer) fn strip_non_null_wrappers<'a>(
    mut expr: &'a Expression<'a>,
) -> &'a Expression<'a> {
    while let ExpressionKind::TSNonNullExpression(non_null) = &expr.kind {
        expr = non_null.expression;
    }
    expr
}

/// `await x` or `yield x` - always need parens together in most contexts
fn is_await_or_yield(expr: &Expression<'_>) -> bool {
    matches!(
        expr.kind,
        ExpressionKind::AwaitExpression(_) | ExpressionKind::YieldExpression(_)
    )
}

/// Lower precedence expressions that need parens in chain/spread/non-null contexts
/// Combines: await/yield + type assertions + binary/conditional/assignment
fn is_lower_precedence(expr: &Expression<'_>) -> bool {
    is_await_or_yield(expr)
        || is_type_assertion(expr)
        || matches!(
            expr.kind,
            ExpressionKind::BinaryExpression(_)
                | ExpressionKind::ConditionalExpression(_)
                | ExpressionKind::AssignmentExpression(_)
        )
}

/// `x as T`, `x satisfies T`, or `<T>x` - TypeScript type assertions
fn is_type_assertion(expr: &Expression<'_>) -> bool {
    matches!(
        expr.kind,
        ExpressionKind::TSAsExpression(_)
            | ExpressionKind::TSSatisfiesExpression(_)
            | ExpressionKind::TSTypeAssertion(_)
    )
}

/// The rule shared by a unary operator's argument and an angle-bracket
/// assertion's operand: await/yield, any type assertion, and the
/// lower-precedence binary / conditional / assignment / arrow forms all need
/// parens.
fn needs_parens_unary_arg_common(expr: &Expression<'_>) -> bool {
    is_await_or_yield(expr)
        || is_type_assertion(expr)
        || matches!(
            expr.kind,
            ExpressionKind::BinaryExpression(_)
                | ExpressionKind::ConditionalExpression(_)
                | ExpressionKind::AssignmentExpression(_)
                | ExpressionKind::ArrowFunctionExpression(_)
        )
}

/// Prettier's UnaryExpression/UpdateExpression-under-UnaryExpression rule: a
/// `+`/`-` operand that would re-tokenize with the parent operator needs parens
/// — a same-operator unary (`+(+x)`, `-(-x)`) or a matching-sign *prefix* update
/// (`+(++x)`, `-(--x)`). Otherwise `+ +` / `- -` / `+ ++` / `- --` glue into
/// `++` / `--` / `+++` / `---`. Postfix updates (`+(x++)` → `+x++`) bind tightly
/// and never merge, so they're excluded; `!`/`~`/`typeof`/`void`/`delete` never
/// form a longer token, so only `+`/`-` parents apply.
fn needs_parens_unary_same_sign(expr: &Expression<'_>, parent_op: UnaryOperator) -> bool {
    let (unary_sign, update_sign) = match parent_op {
        UnaryOperator::Plus => (UnaryOperator::Plus, UpdateOperator::Increment),
        UnaryOperator::Minus => (UnaryOperator::Minus, UpdateOperator::Decrement),
        _ => return false,
    };
    match &expr.kind {
        ExpressionKind::UnaryExpression(u) => u.operator == unary_sign,
        ExpressionKind::UpdateExpression(u) => u.prefix && u.operator == update_sign,
        _ => false,
    }
}

/// Arrow function or function expression
fn is_function_like(expr: &Expression<'_>) -> bool {
    matches!(
        expr.kind,
        ExpressionKind::ArrowFunctionExpression(_) | ExpressionKind::FunctionExpression(_)
    )
}

/// Prefix/postfix unary or update expression (`-x`, `!x`, `typeof x`, `void x`,
/// `delete x`, `++x`, `x--`). These bind looser than member access, call, and
/// the postfix `!` non-null operator, so they need parens as a member-access
/// object (`(-x).p`), a chain callee (`(-x)()`), or a non-null operand (`(++x)!`)
/// — without them the operator captures the wrong operand (`-x.p` is `-(x.p)`;
/// `++x!` is `++(x!)`). `UpdateExpression` is easy to omit when adding such a
/// context (it was missed for `ChainBase` and `NonNull`); routing every
/// postfix/access-precedence arm through this predicate keeps them in lockstep.
fn is_unary_or_update(expr: &Expression<'_>) -> bool {
    matches!(
        expr.kind,
        ExpressionKind::UnaryExpression(_) | ExpressionKind::UpdateExpression(_)
    )
}

/// Whether a binary operator's printed spelling JOINS a `>` the operand before it ended on
/// — one table for two askers: the unfrozen binary-left rule
/// ([`needs_parens_binary_operand`], via `ends_with_instantiation_close`) and the tail half
/// of the frozen `new X<T>` slice's join question
/// ([`super::Printer::frozen_slice_absorbs_left_binding_suffix`]).
///
/// The answer is a REJECTION table over the three parsers that grade the output — tsc,
/// acorn-typescript and tsv itself — plus the one operator pair that rebinds silently. A tail
/// joins when the bare spelling is not a form all three read as the input meant:
///
/// - `+` and `-` are the operators that also OPEN an expression, so the `>` takes the
///   operator's own right-hand side and the whole thing reads as a relational chain
///   (`new X<T> + 1` is `((new X) < T) > +1` at every parser, tsv included) — accepted
///   everywhere and a different tree everywhere;
/// - `>`, `>>` and `>>>` are rejected by all three;
/// - `<` and `>=` are rejected by **tsc** (`'>' expected` / `Expression expected`), and so by
///   prettier, which is a front end for it. acorn-typescript and tsv accept them as the same
///   tree, which is exactly why no reparse of tsv's own output can see the loss;
/// - `<<` is the mirror: tsc accepts it as the same tree, and **acorn-typescript** rejects it.
///   tsv is acorn's drop-in, so the pair stays.
///
/// Every other operator lets all three backtrack to the type arguments, which is what makes
/// the bare spelling AST-identical there.
///
/// This is the TAIL half of the `canFollowTypeArgumentsInExpression` seam — an operand that
/// ENDS on a `>`. Its DUAL, an operand that ends on the `>`'s left and whose own `<`…`>`
/// region would re-lex as the argument list, lives in the arm of
/// [`needs_parens_binary_operand`] keyed on
/// `internal::BinaryExpression::relexes_as_type_arguments`. Neither half is derivable from
/// the other, and a change to one is a question about the other: the two are findable from
/// here and from there, and nowhere else.
pub(in crate::printer) const fn joins_a_trailing_angle_bracket(op: BinaryOperator) -> bool {
    matches!(
        op,
        BinaryOperator::Plus
            | BinaryOperator::Minus
            | BinaryOperator::LessThan
            | BinaryOperator::GreaterThan
            | BinaryOperator::GreaterThanEquals
            | BinaryOperator::LeftShift
            | BinaryOperator::RightShift
            | BinaryOperator::UnsignedRightShift
    )
}

/// Whether the LAST token `expr` prints is the closing `>` of an instantiation
/// expression's type argument list — the instantiation itself, or a node whose
/// rightmost child prints bare at its end (a binary's printed right operand — the
/// innermost one of a rebalanced same-operator logical chain — a prefix
/// operator's argument, an angle-bracket assertion's operand, a conditional's
/// alternate, an arrow's expression body, an `await` / `yield` argument). A child that
/// takes its own pair in that position ends the operand on a `)` instead, so the walk
/// stops there; every other node ends on a token of its own (`!`, `)`, `]`, `}`, a
/// name, a literal). A shell the PRINTER retains for a comment is invisible here —
/// the walk asks `needs_parens`, which does not see that decision — so such an
/// operand takes a redundant outer pair, at this arm and at every other alike. The
/// `new X<T>` spelling ends on the `()` the printer always emits, so it is not in
/// the class, and `as` / `satisfies` end on a TYPE rather than on an expression, so
/// nothing can re-lex their tail.
///
/// The conditional, arrow, `await` and `yield` descents are live only at a position
/// that pairs none of those kinds — a Svelte block head, asked through
/// [`crate::prints_ending_on_instantiation_close`]. Every `ParenContext` arm that reads
/// this walk (a joining binary left, a `>`-led or `/=` assignment target, the left of
/// `as` / `satisfies`) already pairs all four by kind, so there the walk only agrees
/// with a pair the arm takes anyway: one pair, never two. An arrow's CONDITIONAL body is
/// the one layout-dependent end — flat, the arrow prints it in a pair of its own, and
/// broken it prints it bare — so the walk takes the broken answer, which costs a
/// redundant outer pair in the flat layout and is sound in both.
pub(crate) fn ends_with_instantiation_close(expr: &Expression<'_>, in_for_init: bool) -> bool {
    match &expr.kind {
        ExpressionKind::TSInstantiationExpression(_) => true,
        ExpressionKind::ConditionalExpression(conditional) => {
            !ternary_branch_needs_parens(conditional.alternate)
                && ends_with_instantiation_close(conditional.alternate, in_for_init)
        }
        ExpressionKind::ArrowFunctionExpression(arrow) => match arrow.body {
            // A sequence body prints its own pair; a conditional one takes the arrow's
            // pair only when flat (see above), so it counts as bare.
            ArrowFunctionBody::Expression(body) => {
                let bare = matches!(body.kind, ExpressionKind::ConditionalExpression(_))
                    || (!needs_parens(body, ParenContext::ArrowBody, in_for_init)
                        && !matches!(body.kind, ExpressionKind::SequenceExpression(_)));
                bare && ends_with_instantiation_close(body, in_for_init)
            }
            ArrowFunctionBody::BlockStatement(_) => false,
        },
        ExpressionKind::AwaitExpression(await_expr) => {
            !needs_parens(
                await_expr.argument,
                ParenContext::AwaitArgument,
                in_for_init,
            ) && ends_with_instantiation_close(await_expr.argument, in_for_init)
        }
        ExpressionKind::YieldExpression(yield_expr) => {
            yield_expr.argument.is_some_and(|argument| {
                !needs_parens(argument, ParenContext::YieldArgument, in_for_init)
                    && ends_with_instantiation_close(argument, in_for_init)
            })
        }
        // The PRINTED right operand: a same-operator logical chain nested to the right
        // (`a ?? (b ?? f<T>)`) prints rebalanced, with no pair (`a ?? b ?? f<T>`), so its
        // last operand is the innermost right one (`rebalanced_right`, the view the binary
        // printer reads). Every other operator's right operand is its own.
        ExpressionKind::BinaryExpression(binary) => {
            let right = binary.rebalanced_right();
            let ctx = ParenContext::BinaryRight {
                parent_op: binary.operator,
            };
            !needs_parens(right, ctx, in_for_init)
                && ends_with_instantiation_close(right, in_for_init)
        }
        ExpressionKind::UnaryExpression(unary) => {
            let ctx = ParenContext::UnaryArgument {
                parent_op: unary.operator,
            };
            !needs_parens(unary.argument, ctx, in_for_init)
                && ends_with_instantiation_close(unary.argument, in_for_init)
        }
        ExpressionKind::UpdateExpression(update) if update.prefix => {
            let ctx = ParenContext::UpdateArgument { postfix: false };
            !needs_parens(update.argument, ctx, in_for_init)
                && ends_with_instantiation_close(update.argument, in_for_init)
        }
        ExpressionKind::TSTypeAssertion(assertion) => {
            let ctx = ParenContext::AngleBracketAssertion;
            !needs_parens(assertion.expression, ctx, in_for_init)
                && ends_with_instantiation_close(assertion.expression, in_for_init)
        }
        _ => false,
    }
}

/// Numeric literal - needs parens in chain base context because `0.toString()` is invalid.
/// Prettier normalizes `0..toString()` to `(0).toString()`.
fn is_numeric_literal(expr: &Expression<'_>) -> bool {
    matches!(
        &expr.kind,
        ExpressionKind::Literal(lit) if matches!(lit.value, LiteralValue::Number(_))
    )
}

//
// Complex helpers (non-trivial logic)
//

/// Expression statement: `<expr>;`
/// Object/function/class expressions and object pattern assignments need parens
/// when they start the statement, to avoid being reparsed as a block, function
/// declaration, or class declaration. `({...});`, `(function () {});`,
/// `(class {});` — matches prettier's "statement starts with `{`/`function`/`class`"
/// rule (parentheses/needs-parentheses.js).
fn needs_parens_expression_statement(expr: &Expression<'_>) -> bool {
    match &expr.kind {
        // Object expression: `({...});` needs parens to avoid being parsed as a block
        ExpressionKind::ObjectExpression(_) => true,
        // Function/class expression: `(function () {});` / `(class {});` need parens
        // to avoid being reparsed as a declaration (which also changes meaning —
        // an anonymous declaration is a syntax error).
        ExpressionKind::FunctionExpression(_) | ExpressionKind::ClassExpression(_) => true,
        // Object pattern assignment: `({a, b} = obj);` needs parens
        ExpressionKind::AssignmentExpression(assign) => {
            matches!(assign.left.kind, ExpressionKind::ObjectPattern(_))
        }
        // Sequence: check the first expression
        ExpressionKind::SequenceExpression(seq) => seq
            .expressions
            .first()
            .copied()
            .is_some_and(needs_parens_expression_statement),
        _ => false,
    }
}

/// Walk to the leftmost (first-printed) leaf of an expression, mirroring
/// prettier's `startsWithNoLookaheadToken` (utilities/starts-with-no-lookahead-token.js).
///
/// Used to decide whether an expression statement must be wrapped in parens
/// because its leftmost token is an object/function/class — e.g. `(class {}).foo`
/// wraps the class, not the whole member expression. Recurses through the
/// positions that print first (`.left`, `.object`, `.callee`, `.test`, …) and
/// stops at IIFE callees/tags (already parenthesized) to match prettier.
pub(crate) fn leftmost_no_lookahead<'a>(expr: &'a Expression<'a>) -> &'a Expression<'a> {
    leftmost_no_lookahead_reached(expr, LeftmostText::Printed).0
}

/// Which text a leftmost walk reads the first token of.
///
/// The two differ at one node: a function or class dividend ([`is_function_dividend`]),
/// whose pair the binary's own builder supplies — and a verbatim slice has no builder.
// TODO: the walks' older stops — an IIFE callee or tag, a non-LHS assignment target — rest
// on a printed pair too, and still stand under `Frozen`: `(⏎// prettier-ignore⏎function ()
// {}()⏎);` freezes to a line that opens on `function`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LeftmostText {
    /// The printed form: a function or class dividend takes its own pair, so the division
    /// opens on that `(` and the walk stops there — descending would wrap it twice.
    Printed,
    /// A format-ignore freeze of the walked expression: its verbatim slice stands in for
    /// the builders, so a dividend opens on a pair only where the author wrote one. A bare
    /// one (`function () {}   / 2`) opens on its keyword, and the walk reads through to it.
    Frozen,
}

impl LeftmostText {
    /// The text a value prints as: its verbatim slice when a freeze replaces its builders.
    pub(crate) const fn of_value(frozen: bool) -> Self {
        if frozen { Self::Frozen } else { Self::Printed }
    }
}

/// [`leftmost_no_lookahead`], plus the one fact about *how* the walk arrived: whether
/// the leftmost node is the OBJECT of a computed, non-optional member expression.
///
/// That is the shape `ExpressionStatement`'s `[lookahead ∉ { `let [` }]` restriction
/// keys on, and the walk is the only place that knows it — by the time a caller holds
/// the leftmost node the step that reached it is gone. One walk answers both questions
/// so the two readings can't drift.
pub(crate) fn leftmost_no_lookahead_reached<'a>(
    expr: &'a Expression<'a>,
    text: LeftmostText,
) -> (&'a Expression<'a>, bool) {
    fn walk<'a>(
        expr: &'a Expression<'a>,
        text: LeftmostText,
        computed_member_object: bool,
    ) -> (&'a Expression<'a>, bool) {
        match &expr.kind {
            // Binary and logical share `BinaryExpression` here — recurse into `.left`, except
            // into a function or class dividend that opens on its own pair
            // ([`dividend_opens_on_its_pair`]).
            ExpressionKind::BinaryExpression(b) => {
                if dividend_opens_on_its_pair(expr, b, text) {
                    (expr, computed_member_object)
                } else {
                    walk(b.left, text, false)
                }
            }
            // A non-LHS target that takes its own pair (`(function () {}) = 1`, `(a + b)
            // = 1`) already prints `(` first, so the walk stops there, as at an IIFE
            // callee — descending would wrap a leading function a second time. A cast
            // target's pair is older and prettier's: prettier still descends through it
            // and wraps the leftmost node inside (`(({}).x as T) = 1`), so for the three
            // cast kinds the walk goes on, matching it.
            ExpressionKind::AssignmentExpression(a) => {
                if !is_type_assertion(a.left)
                    && needs_parens(
                        a.left,
                        ParenContext::AssignmentTarget {
                            operator: a.operator,
                        },
                        false,
                    )
                {
                    (expr, computed_member_object)
                } else {
                    walk(a.left, text, false)
                }
            }
            ExpressionKind::MemberExpression(m) => walk(m.object, text, m.computed && !m.optional),
            ExpressionKind::ConditionalExpression(c) => walk(c.test, text, false),
            ExpressionKind::SequenceExpression(s) => s
                .expressions
                .first()
                .map_or((expr, computed_member_object), |first| {
                    walk(first, text, false)
                }),
            // IIFEs (`(function () {})()` / `` (function () {})`x` ``) are already
            // parenthesized by their callee/tag, so prettier stops the walk there.
            ExpressionKind::CallExpression(call) => {
                if matches!(call.callee.kind, ExpressionKind::FunctionExpression(_)) {
                    (expr, computed_member_object)
                } else {
                    walk(call.callee, text, false)
                }
            }
            ExpressionKind::TaggedTemplateExpression(t) => {
                if matches!(t.tag.kind, ExpressionKind::FunctionExpression(_)) {
                    (expr, computed_member_object)
                } else {
                    walk(t.tag, text, false)
                }
            }
            // Postfix update (`x++`) prints its argument first; prefix (`++x`) does not.
            ExpressionKind::UpdateExpression(u) if !u.prefix => walk(u.argument, text, false),
            ExpressionKind::TSAsExpression(e) => walk(e.expression, text, false),
            ExpressionKind::TSSatisfiesExpression(e) => walk(e.expression, text, false),
            ExpressionKind::TSNonNullExpression(e) => walk(e.expression, text, false),
            ExpressionKind::TSInstantiationExpression(e) => walk(e.expression, text, false),
            _ => (expr, computed_member_object),
        }
    }
    walk(expr, text, false)
}

/// Whether `export default <expr>;` wraps the expression in parens — for either of two
/// independent reasons.
///
/// **Semantic:** its first *printed* token would be a bare `function`/`class` keyword,
/// which the grammar reads as a (hoisted) declaration, leaving the rest of the expression
/// (`.m()`, `= 1`, `as T`) dangling and unreparseable. Mirrors prettier's
/// `startsWithNoLookaheadToken(expr, isFunctionOrClass)` (parentheses/needs-parentheses.js).
///
/// **Clarity:** the value is an assignment ([`assignment_value_needs_parens`]) — the same
/// answer every other value position gives. The two overlap only on an assignment whose
/// leftmost token is a class/function, and either reason alone emits the one pair.
///
/// ⚠️ A `true` here does **not** mean the pair encloses the value's trailing comment gap:
/// it closes at the expression, so `export default`'s terminator split passes
/// `operand_parens_printed: false` (see `build_export_default_value_doc`).
///
/// Unlike the shared `leftmost_no_lookahead`, the callee/tag/object descent is
/// **paren-aware**: it stops when that child is itself parenthesized (a binary
/// tag `(f(){}+x)`, a lower-precedence callee, …), because the printed form then
/// starts with `(` and needs no outer paren. That avoids the double-wrap the raw
/// walk hits — prettier's own util docstring flags it as "overzealous if there
/// already are necessary grouping parentheses". So does the binary-left descent at a
/// function or class dividend, which takes its own pair (`export default (function () {})
/// / 2;`) — in the text the value prints as (`text`, [`LeftmostText`]: a frozen value's
/// verbatim slice has the pair only where the author wrote it). The other descents
/// (conditional test, cast operand, …) print the keyword bare, so they recurse
/// unconditionally like `leftmost_no_lookahead`.
pub(crate) fn export_default_needs_parens(expr: &Expression<'_>, text: LeftmostText) -> bool {
    // Two independent reasons for one pair — the value-position assignment rule below,
    // and the leftmost-token rule this function is named for.
    assignment_value_needs_parens(expr)
        || matches!(
            export_default_leftmost(expr, text).kind,
            ExpressionKind::FunctionExpression(_) | ExpressionKind::ClassExpression(_)
        )
}

/// An assignment used as a VALUE takes clarity parens (`const x = (y = z);`).
///
/// Prettier's rule for `AssignmentExpression` is default-TRUE with a short exemption
/// list, so this is the whole answer at every position that asks only it — the
/// `needs_parens` arm naming them, plus `export default`. The exemptions it does grant —
/// a C-style `for` header's own init/update clause, an expression statement, a chained
/// assignment's RHS, an object-pattern property value — are each answered by *their*
/// context arm returning false, never here.
///
/// One predicate and one arm rather than a `matches!` per position: spelled per
/// position, a position that never asked was a silent miss (`for (let i = (a = b); ;)`,
/// `for (const x of (a = b))`, `export = (a = b)` and an enum member's `A = (a = b)` all
/// dropped the pair).
fn assignment_value_needs_parens(expr: &Expression<'_>) -> bool {
    matches!(expr.kind, ExpressionKind::AssignmentExpression(_))
}

fn export_default_leftmost<'a>(expr: &'a Expression<'a>, text: LeftmostText) -> &'a Expression<'a> {
    match &expr.kind {
        // A division whose function or class dividend opens on its own pair starts with
        // that `(`, like the paren-aware descents below: the walk stops at the division.
        ExpressionKind::BinaryExpression(b) if dividend_opens_on_its_pair(expr, b, text) => expr,
        ExpressionKind::BinaryExpression(b) => export_default_leftmost(b.left, text),
        ExpressionKind::AssignmentExpression(a) => export_default_leftmost(a.left, text),
        ExpressionKind::ConditionalExpression(c) => export_default_leftmost(c.test, text),
        // A `SequenceExpression` self-parenthesizes in `build_sequence_doc` (its printed
        // form always starts with `(`), so — like the paren-aware member/call/tag descents
        // below — the walk stops here instead of recursing to the leftmost operand.
        // Recursing would double-wrap a class/function-leftmost sequence:
        // `export default ((class {}, x))` instead of prettier's `(class {}, x)`.
        ExpressionKind::SequenceExpression(_) => expr,
        ExpressionKind::UpdateExpression(u) if !u.prefix => {
            export_default_leftmost(u.argument, text)
        }
        ExpressionKind::TSAsExpression(e) => export_default_leftmost(e.expression, text),
        ExpressionKind::TSSatisfiesExpression(e) => export_default_leftmost(e.expression, text),
        ExpressionKind::TSNonNullExpression(e) => export_default_leftmost(e.expression, text),
        ExpressionKind::TSInstantiationExpression(e) => export_default_leftmost(e.expression, text),
        // Descents that cross a would-be-parenthesized child: stop there, since its
        // leading `(` already guards any inner keyword.
        ExpressionKind::MemberExpression(m)
            if !needs_parens(m.object, ParenContext::ChainBase, false) =>
        {
            export_default_leftmost(m.object, text)
        }
        ExpressionKind::CallExpression(call)
            if !needs_parens(call.callee, ParenContext::Callee, false) =>
        {
            export_default_leftmost(call.callee, text)
        }
        ExpressionKind::TaggedTemplateExpression(t)
            if !needs_parens(t.tag, ParenContext::TaggedTemplateTag, false) =>
        {
            export_default_leftmost(t.tag, text)
        }
        _ => expr,
    }
}

/// Whether the `<` region of a relational chain would open on a `(` in the PRINTED form —
/// the second disjunct of the relational arm in [`needs_parens_binary_operand`], and the
/// half `BinaryExpression::relexes_as_type_arguments` cannot answer.
///
/// That flag is a byte scan the parser ran over the SOURCE, and its `(` head arm looks
/// THROUGH a shell, because the shell the arm was written for is one the printer STRIPS:
/// `a < (arr[b - 1]) > c` prints as `a < arr[b - 1] > c`, so the two authorings of that one
/// document owe one verdict (`deno task paren:audit`'s `< > operand` class enumerates
/// exactly that pair). Where the printer KEEPS the shell the look-through grades the wrong
/// text — the content is no type, so the scan declines the pair, while the `(` it looked
/// through is still in the output, and a `(`-headed region IS a type-argument list to a
/// reader that grades bracket matching and the follow token rather than the body. Both of
/// the resulting spellings are documents the printer itself would emit:
/// `x < (a = b) > (t, u)` is rejected flat, and `x < (a = b) > c` the moment a width puts a
/// line terminator past the `>`.
///
/// The question is about the region's FIRST PRINTED BYTE, not about the operand NODE, so
/// this walks the operand's leftmost printed spine. A `(` inherited from a descendant opens
/// the region exactly as the operand's own would: `x < (a = b)[0] > (t, u)` prints the
/// assignment's required pair and the member prints nothing ahead of it, so the region still
/// opens on `(` — and a bare spelling there is a document tsv cannot reparse.
///
/// Two nodes carry a pair no position asks for, and both are tested at every step rather
/// than at the root alone:
///
/// - a [`JsdocCast`](crate::ast::internal::JsdocCast), whose parens are semantically
///   required — strip them and the cast stops being one — so its own doc always prints
///   them. A `/** @type {T} */` the author glued to the shell of a `<` operand is the same
///   document as the shell without it, and the region reads alike either way: the head scan
///   steps over a comment;
/// - a `SequenceExpression`, which supplies its own grouping pair in `build_sequence_doc`.
///   (The region-keyed reading already commits on a sequence — the `,` spells the argument
///   separator — so this is belt-and-braces, and it is here because the rule is about the
///   printed `(`, not about which reading happens to reach it.)
///
/// **Which `(` opens the region, and what may stand between the root and it.** The walk
/// goes down the printed left spine to the first child that prints in a pair
/// ([`printed_left_spine_step`]), and the node directly above that child decides
/// ([`RegionHop`]):
///
/// - a plain **computed** member, whose `[` is the one postfix the type grammar gives a
///   parenthesized type. The reading of the region then carries on past the `]`, so the
///   pair is owed only where every node above is such a member or a list hop: any other
///   hop puts a token there that no reading continues past — `.` (a qualified name needs
///   an identifier head, never a `)`), a plain call's `(`, a plain template's backtick, an
///   operator, a `?` — and an OPTIONAL member or call prints `?.` first, whose `?` is not
///   a type token either;
/// - an **argument list** — an instantiation's, a generic call's, a generic tagged
///   template's. The parser's own lookahead keeps a region claimed where a `<` stands
///   behind a shell that heads it, and reads nothing past that list (the parser's
///   `OperandEnd::RegionHeadShell`), so whatever stands ABOVE the list on the spine is
///   beside the point: `x < (typeof a)<C>(e).m > c` owes the pair that
///   `x < (typeof a)<C>(e) > c` does. The pair is owed there on the shells the recorded
///   flag does not already answer for ([`shell_content_may_read_as_a_type`]);
/// - anything else closes the region at the `)`, and no pair is owed.
///
/// A hop that could put a type SEPARATOR behind the `)` instead (`,`, `|`, `&`) cannot be
/// reached without the operand ROOT itself taking a pair first, since each of those binds
/// looser than `<` and a sequence self-parenthesizes — so the root test above has already
/// answered. The one other token a type operand may be followed by, `extends`, is no
/// expression token, so no printed `)` is ever followed by it.
///
/// ⚠️ The one token in that list whose answer is a tsv POSITION rather than a grammar fact
/// is the non-null `!`: it is tsc's `JSDocNonNullableType`, so a compiler reading
/// `(a = b)! ` would carry on, while tsv's own parse follows acorn-typescript and stops —
/// the drop-in contract. A parse that ever admitted `!` after a `)` would put the non-null
/// hop in this walk.
///
/// Over-approximating within the admitted hops is free: a pair around the `>`'s left operand
/// ends the region on a `)`, which continues no type-argument list, so a pair this adds
/// where the bare form would have re-parsed is noise and never a hazard. The arm stays
/// LAYOUT-BLIND with the rest of the family — whether the `>` ends a line is not knowable
/// where parens are decided, and the follow token past the `)` is read the same way at every
/// width — so the pair stands at every width.
fn relational_region_opens_on_a_kept_shell(operand: &Expression<'_>, in_for_init: bool) -> bool {
    // The root's own pair is the one its POSITION derives — the same `needs_parens` call
    // `Printer::build_binary_operand_doc` makes for a `<`'s right operand.
    let mut node = operand;
    if prints_its_own_paren_pair(node)
        || needs_parens(
            node,
            ParenContext::BinaryRight {
                parent_op: BinaryOperator::LessThan,
            },
            in_for_init,
        )
    {
        return true;
    }
    // Then down the printed left spine to the first child that prints in a pair — the
    // `(` the region opens on — each step asked through the context its own builder passes
    // for that child ([`printed_left_spine_step`]), so the two cannot drift.
    let mut open = true;
    loop {
        let Some((child, parenthesized)) = printed_left_spine_step(node, in_for_init) else {
            return false;
        };
        let hop = RegionHop::of(node);
        if prints_its_own_paren_pair(child) || parenthesized {
            return match hop {
                RegionHop::Index => open,
                // A `<` behind the shell keeps the region claimed by the parser's own
                // lookahead whatever stands above the list, and the recorded flag has
                // already answered for every content but one kind — so the pair is owed
                // on those shells alone ([`shell_content_may_read_as_a_type`]), and on a
                // self-parenthesizing child as it is at the root.
                RegionHop::List => {
                    shell_content_may_read_as_a_type(child) || prints_its_own_paren_pair(child)
                }
                RegionHop::Closing => false,
            };
        }
        open &= hop != RegionHop::Closing;
        node = child;
    }
}

/// Whether a paren shell around `expr`, kept ahead of an argument list, is one the chain's
/// pair is owed on — a shell the parser's type-argument lookahead reads as a
/// parenthesized type with a list behind it.
///
/// The recorded flag (`BinaryExpression::relexes_as_type_arguments`) answers for most of
/// them by itself: its reading looks through the shell at the content, and a content it
/// claims — an arrow function, a sequence, an instantiation — sets the flag whether the
/// shell is kept or not. What it cannot answer is the content that reads as a type NAME:
/// behind a name the reading steps over the list as the name's own and grades what
/// follows it, which is right where the printer strips the shell (`(A)<C>(e)` prints as
/// `A<C>(e)`, a generic call no type spells) and wrong where it keeps one, since the
/// printed `(`…`)<C>` is a parenthesized type the lookahead claims whatever follows the
/// list. The shells the printer keeps there are an instantiation's unary operand —
/// `(typeof a)<C>`, `(!a)<C>`, `(-1)<C>` — and a union or intersection's own pair
/// (`(a | b)<C>`).
///
/// A unary operand answers only where its own operand ends as a type operand does, and a
/// union or intersection only where every member does ([`ends_like_a_type_operand`]):
/// one that holds a call is no type to the parser's grade of the shell, which then makes
/// no claim behind it.
///
/// **Known gap.** The parser's claim behind a shell that heads the region asks nothing of
/// what the shell holds, so it also covers kept shells this filter omits — a conditional,
/// an assignment, a relational or shift chain, an angle-bracket assertion, a bare-name
/// arrow function, a unary operand that ends on a call (`(a ? b : c)<C>`, `(a = b)<C>`,
/// `(a < b)<C>`, `(<B>a)<C>`, `(typeof a())<C>`). Those chains print bare, and a width
/// that ends a line on their `>` prints a region the parser then claims and rejects.
fn shell_content_may_read_as_a_type(expr: &Expression<'_>) -> bool {
    match &expr.kind {
        ExpressionKind::UnaryExpression(unary) => match unary.operator {
            UnaryOperator::Typeof | UnaryOperator::Bang => ends_like_a_type_operand(unary.argument),
            UnaryOperator::Minus => is_numeral(unary.argument),
            UnaryOperator::Plus
            | UnaryOperator::Void
            | UnaryOperator::Delete
            | UnaryOperator::Tilde => false,
        },
        ExpressionKind::BinaryExpression(_) => ends_like_a_type_operand(expr),
        _ => false,
    }
}

/// Whether `expr` prints ending on an operand a type-argument list may stand behind in
/// the lookahead's reading: a name or qualified name, `this`, a literal, an object or
/// array, a group, a member of one, or one of those under `typeof` / `!` or ahead of a
/// non-null `!`.
fn ends_like_a_type_operand(expr: &Expression<'_>) -> bool {
    match &expr.kind {
        ExpressionKind::Identifier(_)
        | ExpressionKind::ThisExpression(_)
        | ExpressionKind::Literal(_)
        | ExpressionKind::ObjectExpression(_)
        | ExpressionKind::ArrayExpression(_)
        | ExpressionKind::SequenceExpression(_)
        | ExpressionKind::ImportExpression(_) => true,
        ExpressionKind::MemberExpression(member) => {
            !member.optional && ends_like_a_type_operand(member.object)
        }
        ExpressionKind::TSNonNullExpression(non_null) => {
            ends_like_a_type_operand(non_null.expression)
        }
        ExpressionKind::UnaryExpression(unary) => match unary.operator {
            UnaryOperator::Typeof | UnaryOperator::Bang => ends_like_a_type_operand(unary.argument),
            UnaryOperator::Minus => is_numeral(unary.argument),
            _ => false,
        },
        ExpressionKind::BinaryExpression(binary) => {
            matches!(
                binary.operator,
                BinaryOperator::Pipe | BinaryOperator::Ampersand
            ) && ends_like_a_type_operand(binary.left)
                && ends_like_a_type_operand(binary.right)
        }
        _ => false,
    }
}

/// Whether `expr` is a numeric or BigInt literal — the operand a `-` makes a negative
/// literal TYPE of.
fn is_numeral(expr: &Expression<'_>) -> bool {
    matches!(
        &expr.kind,
        ExpressionKind::Literal(literal)
            if matches!(literal.value, LiteralValue::Number(_) | LiteralValue::BigInt)
    )
}

/// What a node prints directly behind the child that opens its printed form, as the
/// reading of a type-argument region that opened on that child's `(` takes it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RegionHop {
    /// A plain computed member's `[` — the one postfix the type grammar gives a
    /// parenthesized type. The region stays open through it.
    Index,
    /// The `<` of an argument list — an instantiation's, a generic call's, a generic
    /// tagged template's. Behind a shell the parser's lookahead keeps the region claimed
    /// there, reading nothing past the list.
    List,
    /// Anything else, each a token neither reading continues past
    /// ([`relational_region_opens_on_a_kept_shell`] names them). An OPTIONAL member or
    /// generic call is one: it prints `?.` first.
    Closing,
}

impl RegionHop {
    fn of(node: &Expression<'_>) -> Self {
        match &node.kind {
            ExpressionKind::MemberExpression(member) if member.computed && !member.optional => {
                Self::Index
            }
            ExpressionKind::CallExpression(call)
                if call.type_arguments.is_some() && !call.optional =>
            {
                Self::List
            }
            ExpressionKind::TaggedTemplateExpression(tagged) if tagged.type_arguments.is_some() => {
                Self::List
            }
            ExpressionKind::TSInstantiationExpression(_) => Self::List,
            _ => Self::Closing,
        }
    }
}

/// A node whose own doc prints a paren pair whatever position it sits in, so no
/// context-keyed question reaches it — see [`relational_region_opens_on_a_kept_shell`],
/// which names why each one is on that list.
fn prints_its_own_paren_pair(expr: &Expression<'_>) -> bool {
    matches!(
        expr.kind,
        ExpressionKind::JsdocCast(_) | ExpressionKind::SequenceExpression(_)
    )
}

/// Whether `expr`, the LEFT operand of `parent_op`, is a function or class expression divided
/// — the operand [`needs_parens_binary_operand`] always parenthesizes, since acorn reads the
/// `/` after a bare one's body as a regex wherever its tokenizer takes the keyword for a
/// statement's.
fn is_function_dividend(expr: &Expression<'_>, parent_op: BinaryOperator) -> bool {
    parent_op == BinaryOperator::Slash
        && matches!(
            expr.kind,
            ExpressionKind::FunctionExpression(_) | ExpressionKind::ClassExpression(_)
        )
}

/// Whether the division `expr` opens, in the text a leftmost walk reads ([`LeftmostText`]),
/// on the `(` of a pair around its function or class dividend ([`is_function_dividend`]) —
/// so the walk stops at the division, as at an IIFE callee, where descending to the keyword
/// would wrap it a second time (`((function () {})) / 2;`, `export default ((function () {})
/// / 2);`).
///
/// The printed form always has that pair. A frozen slice has it where the author wrote it,
/// which the spans say: the division's begins at the `(` the parser erased from its left
/// operand, so a dividend starting later than its division sits inside one.
fn dividend_opens_on_its_pair(
    expr: &Expression<'_>,
    binary: &BinaryExpression<'_>,
    text: LeftmostText,
) -> bool {
    is_function_dividend(binary.left, binary.operator)
        && match text {
            LeftmostText::Printed => true,
            LeftmostText::Frozen => binary.left.span().start > expr.span().start,
        }
}

/// Why a binary operand prints in a paren pair ([`binary_operand_pair`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::printer) enum OperandPair {
    /// It does not.
    None,
    /// The tree, or a join of two tokens, needs it: stripped, the operand reads as
    /// another program or as none.
    Owed,
    /// For the reader alone: the bare spelling parses to the same tree, and no token
    /// on either side of the pair is one it keeps apart. `a && (await b)`,
    /// `(a % b) + c`, `(a * b) / c`, `(a << b) << c`, `(!a) in b`.
    ///
    /// The one position that withholds it is the operand behind a `>` that may close a
    /// type-argument region, where a `(` commits the list
    /// ([`clarity_pairs_behind_region_close`]) — which reads one pair of another
    /// position as this kind too, a called or tagging function expression's
    /// (`(function () {})()`).
    Clarity,
}

/// Binary operand: `<expr> op y` or `x op <expr>`. `in_for_init` is the ambient
/// for-init flag, read only by the instantiation-tail walk below (it asks the
/// printed shape of the operand's own children).
fn needs_parens_binary_operand(
    expr: &Expression<'_>,
    parent_op: BinaryOperator,
    is_right: bool,
    in_for_init: bool,
) -> bool {
    binary_operand_pair(expr, parent_op, is_right, in_for_init) != OperandPair::None
}

/// [`needs_parens_binary_operand`], with the reason the pair is there.
///
/// Every [`OperandPair::Owed`] arm is asked ahead of every [`OperandPair::Clarity`] one,
/// so an operand that takes a pair for two reasons answers with the one that cannot be
/// withheld: `(await f<T>) + 1` is owed by the `>` its `await` operand ends on, though
/// the `await` alone would take a clarity pair there.
pub(in crate::printer) fn binary_operand_pair(
    expr: &Expression<'_>,
    parent_op: BinaryOperator,
    is_right: bool,
    in_for_init: bool,
) -> OperandPair {
    // These expressions need parens when used as operands of binary expressions: each
    // binds looser than any binary operator.
    // e.g., `a && (b ? c : d)` - without parens it becomes `(a && b) ? c : d`
    // e.g., `(x as string) in obj` - without parens it becomes `x as (string in obj)`
    // e.g., `b || ((fn) => fn)` - without parens it becomes `(b || fn) => fn` (syntax error)
    // (`await` binds tighter than all of them; its pair is the clarity arm further down.)
    if matches!(
        expr.kind,
        ExpressionKind::ConditionalExpression(_)
            | ExpressionKind::AssignmentExpression(_)
            | ExpressionKind::TSAsExpression(_)
            | ExpressionKind::TSSatisfiesExpression(_)
            | ExpressionKind::ArrowFunctionExpression(_)
            | ExpressionKind::YieldExpression(_)
    ) {
        return OperandPair::Owed;
    }

    // Unary expressions as left operand of ** require parens (ES2016+ syntax rule)
    // `-2 ** 3` is a syntax error; must be `(-2) ** 3` or `-(2 ** 3)` — and an `await`
    // operand is a `UnaryExpression` to that rule (`await a ** 2`). An angle-bracket
    // assertion is the TypeScript twin: tsc rejects `<T>x ** 3` with the same diagnostic
    // family, and acorn-typescript reads it as `<T>(x ** 3)` — a different tree. Prettier
    // strips this pair, a cataloged ◆prettier_bug.
    if !is_right
        && parent_op == BinaryOperator::StarStar
        && matches!(
            expr.kind,
            ExpressionKind::UnaryExpression(_)
                | ExpressionKind::TSTypeAssertion(_)
                | ExpressionKind::AwaitExpression(_)
        )
    {
        return OperandPair::Owed;
    }

    // A function or class expression as the left operand of `/` keeps its pair — `(function
    // () {}) / 2`, `(class {}) / 2` — in every position, async, generator, named and decorated
    // forms included. The grammar reads a bare `function () {} / 2` as a division, but acorn
    // (the parser Svelte runs over every `<script>` and template expression) decides each `/`
    // by a token-context heuristic: where its tokenizer takes the keyword for a statement's —
    // among them a template expression's first token, an anonymous `async function`, a
    // decorated class, a `yield` or `await` operand, a conditional's alternate outside any
    // `(…)`, object literal or `${…}` — it reads the `/` after the body as a regex, and the
    // document no longer parses. One rule over every function and class expression rather
    // than acorn's positions: the pair is right by the grammar wherever it lands, and only a
    // model of acorn's context stack could say where it is not needed. (acorn-typescript has
    // a misread of its own the pair can meet — an empty object type before a `)`, `(function
    // (): {} {}) / 2` — tracked apart from this rule.) Prettier strips the pair everywhere, a
    // cataloged divergence. This is the direct case of one invariant — no function or class
    // body's `}` prints directly before a `/` — whose indirect case, a body the left operand
    // ENDS on (`!function () {} / 2`), is `Printer::mark_dividend_tail`.
    if !is_right && is_function_dividend(expr, parent_op) {
        return OperandPair::Owed;
    }

    // A left operand whose LAST printed token is an instantiation's closing `>` takes
    // the pair ahead of an operator whose first token joins that `>`
    // ([`joins_a_trailing_angle_bracket`], the per-operator table the frozen `new X<T>`
    // slice reads too): `+` and `-` continue it as a relational chain (`f<T> + 1` re-lexes
    // as `f < T > +1`, a different program) and `<`, `>`, `>=`, `<<`, `>>` and `>>>` leave a
    // form tsc or acorn-typescript rejects (`f<T> >= 1` does not parse). The axis is the JOIN
    // of the operand's last token and the operator's first, not the operand's node or
    // precedence — `a * fn<T> + 1` and `-fn<T> + 1` rebind exactly as `fn<T> + 1` does,
    // so the walk follows the rightmost printed child (`ends_with_instantiation_close`).
    // Every other binary operator follows a bare instantiation (`f<T> * 1`, `f<T> <= 1`).
    // Prettier strips the pair at every one of these, a cataloged ◆prettier_bug.
    if !is_right
        && joins_a_trailing_angle_bracket(parent_op)
        && ends_with_instantiation_close(expr, in_for_init)
    {
        return OperandPair::Owed;
    }

    // The DUAL of the arm above, read off the same `canFollowTypeArgumentsInExpression`
    // seam from the other side. There the operand IS an instantiation and the operator's
    // first token re-lexes its closing `>`; here the operand is a relational `<` chain
    // whose own printed `<`…`>` region would BE a type-argument list, and the enclosing
    // `>` is the close. `(fn < A[T]) > (t, u)` and `(x < y) > { a: 1 }` are the same tree
    // in the spelling every parser reads alike; bare, both re-parse as something else —
    // a `CallExpression` with type arguments where the printer folded the author's break
    // away, an instantiation plus a free-standing statement where it added one of its own
    // (past a line break tsv's own parse commits the list ahead of any expression, so
    // width alone reaches it; which followers commit, for each parser, is stated in the
    // catalog entry). Prettier strips the pair at both, a cataloged ◆prettier_bug.
    //
    // The axis is the JOIN of two tokens, not the operand's node — the same doctrine — so
    // the question was answered where the tokens still exist: the parser recorded it on
    // the `<` node (`BinaryExpression::relexes_as_type_arguments`), at the `>` that closes
    // the region. It is deliberately LAYOUT-BLIND: whether the `>` ends a line is
    // unknowable here, so the pair stands at every width, the ones that never break
    // included. And it is REGION-keyed rather than shape-keyed: an operand that is no type
    // keeps the chain bare, whatever it is parenthesized with.
    //
    // The recorded flag answers for every region the parser and the printer read as one
    // text. The second disjunct is the rest of them — the region whose head the PRINTER
    // writes, a `(` the operand keeps ([`relational_region_opens_on_a_kept_shell`]).
    if !is_right
        && parent_op == BinaryOperator::GreaterThan
        && matches!(
            &expr.kind,
            ExpressionKind::BinaryExpression(child)
                if child.operator == BinaryOperator::LessThan
                    && (child.relexes_as_type_arguments
                        || relational_region_opens_on_a_kept_shell(child.right, in_for_init))
        )
    {
        return OperandPair::Owed;
    }

    // A unary left operand of `in` / `instanceof` keeps CLARITY parens (prettier's
    // `parentheses/needs-parentheses.js`, the `UnaryExpression` → `BinaryExpression` arm).
    // The parse is unambiguous either way — `!` binds tighter than a relational operator —
    // but `!a in b` reads as `!(a in b)` to a human, so the parens say which one the author
    // wrote. An UpdateExpression is deliberately NOT covered: it falls through to this same
    // arm in prettier, which keys the rule on `node.type === "UnaryExpression"`, so
    // `a++ in b` stays bare.
    if !is_right
        && matches!(parent_op, BinaryOperator::In | BinaryOperator::Instanceof)
        && matches!(expr.kind, ExpressionKind::UnaryExpression(_))
    {
        return OperandPair::Clarity;
    }

    // `await` binds tighter than every binary operator, so `a && await b` is the tree
    // `a && (await b)` is: the pair is for clarity (Prettier style). Asked BELOW the arms
    // that owe one — the `**` rule and the instantiation join above — which an `await`
    // operand can meet too.
    if matches!(expr.kind, ExpressionKind::AwaitExpression(_)) {
        return OperandPair::Clarity;
    }

    let ExpressionKind::BinaryExpression(child) = &expr.kind else {
        return OperandPair::None;
    };
    let child_op = child.operator;

    // Special case: Logical operators (&&, ||, ??) mixing requires parens
    if parent_op.is_logical() && child_op.is_logical() && parent_op != child_op {
        return OperandPair::Owed;
    }

    let parent_prec = parent_op.precedence();
    let child_prec = child_op.precedence();

    // 1. Child has weaker precedence
    if child_prec < parent_prec {
        return OperandPair::Owed;
    }

    // 2. Right operand with same precedence - preserve programmer's grouping
    if is_right && child_prec == parent_prec {
        return OperandPair::Owed;
    }

    // 3. Same precedence but can't flatten. A LEFT operand by now, so the bare spelling
    // re-associates onto it by itself — except under `**`, the one right-associative
    // operator, where `(a ** b) ** c` is not `a ** b ** c`.
    if child_prec == parent_prec && !parent_op.can_flatten_with(child_op) {
        return if parent_op == BinaryOperator::StarStar {
            OperandPair::Owed
        } else {
            OperandPair::Clarity
        };
    }

    // 4. Special handling for modulo (the child binds tighter: clarity)
    if parent_prec < child_prec && child_op == BinaryOperator::Percent {
        return if matches!(parent_op, BinaryOperator::Plus | BinaryOperator::Minus)
            || parent_op.is_bitwise()
        {
            OperandPair::Clarity
        } else {
            OperandPair::None
        };
    }

    // 5. Bitwise operators with different precedence (the child binds tighter: clarity)
    if parent_op.is_bitwise() && child_prec != parent_prec {
        return OperandPair::Clarity;
    }

    OperandPair::None
}

/// The clarity pairs that would stand between `close` — a `>`, `>>` or `>>>` that may
/// close a type-argument region (`BinaryExpression::may_close_type_arguments`) — and the
/// first token of its right operand: the spans of the nodes that print BARE there.
///
/// A `(` directly behind such a token commits the list on any line, for every parser:
/// `fn(a < b, c > (await d))` is the generic call `a<b, c>(await d)`, and
/// `fn(a < b, c > (d % e) + f)` the call `a<b, c>(d % e)` plus `f`. Printed from
/// `fn(a < b, c > await d)` that `(` is the printer's own invention, so two comparisons
/// become one call's type arguments in a single pass — and nothing between the `<` and
/// the `>` can be reworded to prevent it (the family's other half, the pair around a
/// chain's `<` operand, ends a region of the `>`'s OWN left operand and does not reach a
/// sibling's). What CAN be left out is the pair: a clarity pair is one the tree does not
/// need ([`OperandPair::Clarity`]), so the operand prints as the author's bare spelling
/// would, the same tree to every parser.
///
/// The walk runs from the right operand down its printed left spine, since the `(` in
/// question is whichever one prints first: `c > ((await d) % e) + f` opens on the `%`
/// operand's pair and then on the `await`'s, and both are withheld. One pair on that
/// spine is no binary operand's and is a clarity pair all the same: the one around a
/// function expression that is called or used as a tag (`(function () {})()`,
/// `` (function () {})`t` ``), which a statement's first token needs and no other
/// position does — behind an operator `function () {}()` is the same call to every
/// parser. The walk ends at the first token that is no clarity pair of the printer's own:
///
/// - **a pair the AUTHOR wrote** (`source_holds_pair`) stays, with everything inside it.
///   Behind a region that reads as a list that `(` is no pair at all but the generic
///   call's argument list, so the parser never built this comparison; where one WAS
///   built, the region reads as no list to the parser that built it (an operator or a
///   call stands inside it, or one oracle alone reads a type there), and the author's
///   spelling is the one each of them read. Both authorings are therefore fixed points
///   where the pair is harmless: `a < b && c > (await d)` and `a < b && c > await d`.
/// - **a pair the tree owes, or one whose `)` keeps two tokens apart**, ends the walk
///   with nothing withheld at all — the operand still opens on a `(`, so leaving out the
///   pairs above it would change the output and not what it reads as. The second kind is
///   a clarity pair in name only: a binary operand that ends on an instantiation's `>` or
///   on a function or class body's `}` reads differently, or not at all, once the token
///   past the `)` stands directly behind it (`c > (await f<T>)`,
///   `(await function () {}) / 2` — [`ends_with_instantiation_close`],
///   `Printer::mark_dividend_tail`). A called function's pair is not asked: what follows
///   its `)` is the call's own `(`, `?.`, `<` or template, never an operator.
///
/// It is LAYOUT-BLIND with the rest of the family, and keyed on the same SUPERSET: which
/// regions are open is the parser's answer, and no model of what each parser's list
/// grammar takes — so the pair is also withheld where it would have been harmless.
// TODO: the pairs the PRINTER adds that this walk stops at still print behind such a
// token and commit the list — a function or class dividend's
// (`fn(a < b, c > function () {} / 2)` prints `c > (function () {}) / 2`), an
// instantiation's ahead of a token that would re-lex its `>` (`c > g<T>⏎+ 1` prints
// `c > (g<T>) + 1`), a numeric literal's as a member object (`c > 0..toString()` prints
// `c > (0).toString()`), and the clarity pair around an operand that ends on one of the
// first two. Each is a pair the printed tokens need as they stand (the dividend rule asks
// no position, the instantiation's bare spelling is a line break, and the literal's is
// one the number printer does not emit), so the answer there is a pair on the `<` side,
// which needs the `<` node to know about a `>` in a later sibling.
pub(in crate::printer) fn clarity_pairs_behind_region_close(
    close: &BinaryExpression<'_>,
    in_for_init: bool,
    source_holds_pair: impl Fn(&Expression<'_>) -> bool,
) -> SmallVec<[Span; 2]> {
    let mut withheld = SmallVec::new();
    let mut node = close.right;
    if prints_its_own_paren_pair(node) {
        return SmallVec::new();
    }
    // The pair `node` takes where it stands, and whether that position is a binary
    // operand's — the one whose `)` an operator follows.
    let mut pair = binary_operand_pair(node, close.operator, true, in_for_init);
    let mut is_operand = true;
    loop {
        match pair {
            OperandPair::None => {}
            OperandPair::Owed => return SmallVec::new(),
            OperandPair::Clarity => {
                if source_holds_pair(node) {
                    break;
                }
                if is_operand
                    && (ends_with_instantiation_close(node, in_for_init)
                        || ends_on_a_function_or_class_body(node, in_for_init))
                {
                    return SmallVec::new();
                }
                withheld.push(node.span());
            }
        }
        let Some((child, parenthesized)) = printed_left_spine_step(node, in_for_init) else {
            break;
        };
        if prints_its_own_paren_pair(child) {
            return SmallVec::new();
        }
        (pair, is_operand) = match &node.kind {
            _ if !parenthesized => (OperandPair::None, false),
            ExpressionKind::BinaryExpression(parent) => (
                binary_operand_pair(child, parent.operator, false, in_for_init),
                true,
            ),
            ExpressionKind::CallExpression(_) | ExpressionKind::TaggedTemplateExpression(_)
                if is_called_function(child) =>
            {
                (OperandPair::Clarity, false)
            }
            // A pair some other position asks for — a callee's, a member object's.
            _ => (OperandPair::Owed, false),
        };
        node = child;
    }
    withheld
}

/// Whether `expr`, a call's callee or a tagged template's tag, is the one kind whose
/// pair there is for the reader alone — a function expression. An arrow's is owed (bare,
/// its body takes the call), and so is every other kind's the position parenthesizes.
pub(in crate::printer) fn is_called_function(expr: &Expression<'_>) -> bool {
    matches!(expr.kind, ExpressionKind::FunctionExpression(_))
}

/// Whether the last token `expr` prints is the `}` of a function or class expression's
/// body — the operand [`clarity_pairs_behind_region_close`] leaves in its pair, since a
/// `/` directly behind that `}` is a regex to acorn wherever its tokenizer took the
/// keyword for a statement's (`Printer::mark_dividend_tail`, whose walk this mirrors
/// through `await`, the one kind that walk never meets bare).
fn ends_on_a_function_or_class_body(expr: &Expression<'_>, in_for_init: bool) -> bool {
    let mut operand = expr;
    loop {
        let (child, ctx) = match &operand.kind {
            ExpressionKind::FunctionExpression(_) | ExpressionKind::ClassExpression(_) => {
                return true;
            }
            ExpressionKind::AwaitExpression(await_expr) => {
                (await_expr.argument, ParenContext::AwaitArgument)
            }
            ExpressionKind::UnaryExpression(unary) => (
                unary.argument,
                ParenContext::UnaryArgument {
                    parent_op: unary.operator,
                },
            ),
            ExpressionKind::BinaryExpression(binary) => (
                binary.rebalanced_right(),
                ParenContext::BinaryRight {
                    parent_op: binary.operator,
                },
            ),
            ExpressionKind::TSTypeAssertion(assertion) => {
                (assertion.expression, ParenContext::AngleBracketAssertion)
            }
            _ => return false,
        };
        // An operand inside a pair ends on its `)`.
        if needs_parens(child, ctx, in_for_init) {
            return false;
        }
        operand = child;
    }
}
