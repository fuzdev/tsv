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
    AssignmentOperator, BinaryOperator, Expression, ExpressionKind, LiteralValue, TSKeywordKind,
    TSType, TSTypeParameterInstantiation, UnaryOperator, UpdateOperator,
};
use crate::printer::class_expr_has_decorators;
use crate::printer::comments::{
    left_side_child_is_parenthesized, next_significant_byte, paren_shell_close_after,
};

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

    /// Expression in TSInstantiationExpression: `<expr><T>`. `second_list_pair` is the
    /// printer's answer for an instantiation `<expr>` — whether it keeps a pair ahead of
    /// this second list ([`instantiation_keeps_pair_before_type_args`]), which reads the
    /// source and whether this node's own owner continues its chain, facts the printer
    /// holds and the node alone does not.
    InstantiationExpression { second_list_pair: bool },

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
        if !matches!(inner.kind, ExpressionKind::TSInstantiationExpression(_))
            || paren_shell_close_after(source, inner.span().end).is_some()
        {
            return first_list_reads_as_expression(source, &inst.type_arguments);
        }
        if !asserts_one_type(source, &inst.type_arguments) {
            return false;
        }
        level = inner;
    }
    true
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
        TSType::IndexedAccess(access) => never(access.object_type) || never(access.index_type),
        TSType::TypeReference(reference) => reference
            .type_arguments
            .as_ref()
            .is_some_and(|list| list.params.iter().any(|t| never(t))),
        _ => false,
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
                // A `new` callee containing a call needs parens so the arguments
                // bind to the `new`, not to the inner call: `new (f())()`,
                // `new (a.b())()`, `new (f().C)()`, `new (a?.b())()`. Without them
                // `new f()()` parses as `(new f())()` — different semantics.
                if new_callee_has_call(expr) {
                    return true;
                }
            }
            // A `new` callee or template tag may NOT be an (unsealed) optional chain
            // per spec — `new a?.b()` / `` a?.b`x` `` are syntax errors. So the parens
            // are *always* required (unlike the boundary-dependent member/call/non-null
            // cases, which depend on what follows the chain). The plain call `Callee`
            // context is excluded: `(a?.b)()` strips to the valid `a?.b()`. A non-null
            // assertion that seals the chain (`(a?.b)!`) is handled by the sealed-base
            // rendering, not here (`has_optional_in_chain` returns false for it).
            if matches!(
                ctx,
                ParenContext::NewCallee | ParenContext::TaggedTemplateTag
            ) && expr.has_optional_in_chain()
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
        ParenContext::TypeAssertion => {
            is_await_or_yield(expr)
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
        // rightmost printed token, `ends_with_instantiation_close` — is stated once at
        // the binary rule, the one place it is live.
        ParenContext::UpdateArgument { postfix } => {
            is_lower_precedence(expr)
                || is_unary_or_update(expr)
                || matches!(expr.kind, ExpressionKind::ArrowFunctionExpression(_))
                || (postfix && matches!(expr.kind, ExpressionKind::TSInstantiationExpression(_)))
        }

        // Instantiation: `(<T>() => {})<U>`, `(x as A)<T>`, `(<T>x)<U>`, `(await x)<T>`, `(a = b)<T>`
        // Ternary/binary/assignment need parens to preserve semantics:
        // `(a ? b : c)<T>` vs `a ? b : c<T>` (different - ternary result vs alternate instantiated)
        // An instantiation instantiated again takes the pair the printer decided.
        ParenContext::InstantiationExpression { second_list_pair } => {
            second_list_pair
                || is_await_or_yield(expr)
                || is_type_assertion(expr)
                || is_function_like(expr)
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
/// rightmost child prints bare at its end (a binary's right operand, a prefix
/// operator's argument, an angle-bracket assertion's operand). A child that takes its
/// own pair BY PRECEDENCE in that position ends the operand on a `)` instead, so the
/// walk stops there; every other node ends on a token of its own (`!`, `)`, `]`, a
/// name, a literal). A shell the PRINTER retains for a comment is invisible here —
/// the walk asks `needs_parens`, which does not see that decision — so such an
/// operand takes a redundant outer pair, at this arm and at every other alike. The
/// `new X<T>` spelling ends on the `()` the printer always emits, so it is not in
/// the class, and `as` / `satisfies` end on a TYPE rather than on an expression, so
/// nothing can re-lex their tail.
fn ends_with_instantiation_close(expr: &Expression<'_>, in_for_init: bool) -> bool {
    match &expr.kind {
        ExpressionKind::TSInstantiationExpression(_) => true,
        ExpressionKind::BinaryExpression(binary) => {
            let ctx = ParenContext::BinaryRight {
                parent_op: binary.operator,
            };
            !needs_parens(binary.right, ctx, in_for_init)
                && ends_with_instantiation_close(binary.right, in_for_init)
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

/// Whether a `new` callee contains a call expression in its leftmost
/// member/non-null chain. Prettier parenthesizes such a callee so the `new`
/// arguments bind to the `new` rather than the inner call: `new (f())()`,
/// `new (a.b())()`, `new (f().C)()`, `new (a?.b())()`. Mirrors prettier's
/// `NewExpression` callee rule (`parentheses/needs-parentheses.js`). Member access walks the
/// object (the call must be to the left of `new`'s argument list to be
/// captured), so `new a[b]()` — no inner call — stays unparenthesized.
fn new_callee_has_call(expr: &Expression<'_>) -> bool {
    match &expr.kind {
        ExpressionKind::CallExpression(_) => true,
        ExpressionKind::MemberExpression(member) => new_callee_has_call(member.object),
        ExpressionKind::TSNonNullExpression(non_null) => new_callee_has_call(non_null.expression),
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
    leftmost_no_lookahead_reached(expr).0
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
) -> (&'a Expression<'a>, bool) {
    fn walk<'a>(
        expr: &'a Expression<'a>,
        computed_member_object: bool,
    ) -> (&'a Expression<'a>, bool) {
        match &expr.kind {
            // Binary and logical share `BinaryExpression` here — recurse into `.left`.
            ExpressionKind::BinaryExpression(b) => walk(b.left, false),
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
                    walk(a.left, false)
                }
            }
            ExpressionKind::MemberExpression(m) => walk(m.object, m.computed && !m.optional),
            ExpressionKind::ConditionalExpression(c) => walk(c.test, false),
            ExpressionKind::SequenceExpression(s) => s
                .expressions
                .first()
                .map_or((expr, computed_member_object), |first| walk(first, false)),
            // IIFEs (`(function () {})()` / `` (function () {})`x` ``) are already
            // parenthesized by their callee/tag, so prettier stops the walk there.
            ExpressionKind::CallExpression(call) => {
                if matches!(call.callee.kind, ExpressionKind::FunctionExpression(_)) {
                    (expr, computed_member_object)
                } else {
                    walk(call.callee, false)
                }
            }
            ExpressionKind::TaggedTemplateExpression(t) => {
                if matches!(t.tag.kind, ExpressionKind::FunctionExpression(_)) {
                    (expr, computed_member_object)
                } else {
                    walk(t.tag, false)
                }
            }
            // Postfix update (`x++`) prints its argument first; prefix (`++x`) does not.
            ExpressionKind::UpdateExpression(u) if !u.prefix => walk(u.argument, false),
            ExpressionKind::TSAsExpression(e) => walk(e.expression, false),
            ExpressionKind::TSSatisfiesExpression(e) => walk(e.expression, false),
            ExpressionKind::TSNonNullExpression(e) => walk(e.expression, false),
            ExpressionKind::TSInstantiationExpression(e) => walk(e.expression, false),
            _ => (expr, computed_member_object),
        }
    }
    walk(expr, false)
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
/// already are necessary grouping parentheses". The other descents (binary left,
/// conditional test, cast operand, …) print the keyword bare, so they recurse
/// unconditionally like `leftmost_no_lookahead`.
pub(crate) fn export_default_needs_parens(expr: &Expression<'_>) -> bool {
    // Two independent reasons for one pair — the value-position assignment rule below,
    // and the leftmost-token rule this function is named for.
    assignment_value_needs_parens(expr)
        || matches!(
            export_default_leftmost(expr).kind,
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

fn export_default_leftmost<'a>(expr: &'a Expression<'a>) -> &'a Expression<'a> {
    match &expr.kind {
        ExpressionKind::BinaryExpression(b) => export_default_leftmost(b.left),
        ExpressionKind::AssignmentExpression(a) => export_default_leftmost(a.left),
        ExpressionKind::ConditionalExpression(c) => export_default_leftmost(c.test),
        // A `SequenceExpression` self-parenthesizes in `build_sequence_doc` (its printed
        // form always starts with `(`), so — like the paren-aware member/call/tag descents
        // below — the walk stops here instead of recursing to the leftmost operand.
        // Recursing would double-wrap a class/function-leftmost sequence:
        // `export default ((class {}, x))` instead of prettier's `(class {}, x)`.
        ExpressionKind::SequenceExpression(_) => expr,
        ExpressionKind::UpdateExpression(u) if !u.prefix => export_default_leftmost(u.argument),
        ExpressionKind::TSAsExpression(e) => export_default_leftmost(e.expression),
        ExpressionKind::TSSatisfiesExpression(e) => export_default_leftmost(e.expression),
        ExpressionKind::TSNonNullExpression(e) => export_default_leftmost(e.expression),
        ExpressionKind::TSInstantiationExpression(e) => export_default_leftmost(e.expression),
        // Descents that cross a would-be-parenthesized child: stop there, since its
        // leading `(` already guards any inner keyword.
        ExpressionKind::MemberExpression(m)
            if !needs_parens(m.object, ParenContext::ChainBase, false) =>
        {
            export_default_leftmost(m.object)
        }
        ExpressionKind::CallExpression(call)
            if !needs_parens(call.callee, ParenContext::Callee, false) =>
        {
            export_default_leftmost(call.callee)
        }
        ExpressionKind::TaggedTemplateExpression(t)
            if !needs_parens(t.tag, ParenContext::TaggedTemplateTag, false) =>
        {
            export_default_leftmost(t.tag)
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
/// A preserved grouping pair is neither: `ParenthesizedExpression` is layout-transparent
/// (`Printer::build_preserved_paren_doc` renders the inner, which re-derives whatever parens
/// it needs), so it is peeled and the question is the inner's.
///
/// **Where the spine STOPS, and why that is a grammar fact rather than an accident.** A `(`
/// only opens a *type-argument* region if the type grammar can carry on past its matching
/// `)`, and the only postfix a parenthesized type takes is `[`…`]`. So the walk descends
/// exactly one hop kind: the OBJECT of a plain **computed** member, the one node that prints
/// `[` and nothing else between its object and the rest of the region. Every other spine hop
/// puts a token there that no type continues — `.` (a qualified name needs an identifier
/// head, never a `)`), `(`, a template's backtick, an operator, a `?` — and an OPTIONAL
/// computed member prints `?.[`, whose `?` is not a type token either. A hop that could put
/// a type SEPARATOR there instead (`,`, `|`, `&`, a nested `<`) cannot be reached without
/// the operand ROOT itself taking a pair first, since each of those binds looser than `<`
/// and a sequence self-parenthesizes — so the root test above has already answered. The one
/// other token a type operand may be followed by, `extends`, is no expression token, so no
/// printed `)` is ever followed by it.
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
    let mut node = peel_preserved_parens(operand);
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
    // Then the one hop that leaves the region open: a plain computed member's OBJECT, asked
    // through the shared position table so the context cannot drift from the one the chain
    // builder passes for that child.
    while let ExpressionKind::MemberExpression(member) = &node.kind {
        if !member.computed || member.optional {
            return false;
        }
        let child = peel_preserved_parens(member.object);
        if prints_its_own_paren_pair(child)
            || left_side_child_is_parenthesized(node, child, in_for_init)
        {
            return true;
        }
        node = child;
    }
    false
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

/// Step past every preserved grouping pair, which the printer renders through rather than
/// as a pair of its own (`Printer::build_preserved_paren_doc`).
fn peel_preserved_parens<'a>(expr: &'a Expression<'a>) -> &'a Expression<'a> {
    let mut expr = expr;
    while let ExpressionKind::ParenthesizedExpression(paren) = &expr.kind {
        expr = paren.expression;
    }
    expr
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
    // These expressions need parens when used as operands of binary expressions.
    // Some have lower precedence, others are for clarity (await/yield).
    // e.g., `a && (b ? c : d)` - without parens it becomes `(a && b) ? c : d`
    // e.g., `(x as string) in obj` - without parens it becomes `x as (string in obj)`
    // e.g., `b || ((fn) => fn)` - without parens it becomes `(b || fn) => fn` (syntax error)
    // e.g., `a && (await b)` - parens for clarity (Prettier style)
    if matches!(
        expr.kind,
        ExpressionKind::ConditionalExpression(_)
            | ExpressionKind::AssignmentExpression(_)
            | ExpressionKind::TSAsExpression(_)
            | ExpressionKind::TSSatisfiesExpression(_)
            | ExpressionKind::ArrowFunctionExpression(_)
            | ExpressionKind::AwaitExpression(_)
            | ExpressionKind::YieldExpression(_)
    ) {
        return true;
    }

    // Unary expressions as left operand of ** require parens (ES2016+ syntax rule)
    // `-2 ** 3` is a syntax error; must be `(-2) ** 3` or `-(2 ** 3)`. An angle-bracket
    // assertion is the TypeScript twin: tsc rejects `<T>x ** 3` with the same diagnostic
    // family, and acorn-typescript reads it as `<T>(x ** 3)` — a different tree. Prettier
    // strips this pair, a cataloged ◆prettier_bug.
    if !is_right
        && parent_op == BinaryOperator::StarStar
        && matches!(
            expr.kind,
            ExpressionKind::UnaryExpression(_) | ExpressionKind::TSTypeAssertion(_)
        )
    {
        return true;
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
        return true;
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
        return true;
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
        return true;
    }

    let ExpressionKind::BinaryExpression(child) = &expr.kind else {
        return false;
    };
    let child_op = child.operator;

    // Special case: Logical operators (&&, ||, ??) mixing requires parens
    if parent_op.is_logical() && child_op.is_logical() && parent_op != child_op {
        return true;
    }

    let parent_prec = parent_op.precedence();
    let child_prec = child_op.precedence();

    // 1. Child has weaker precedence
    if child_prec < parent_prec {
        return true;
    }

    // 2. Right operand with same precedence - preserve programmer's grouping
    if is_right && child_prec == parent_prec {
        return true;
    }

    // 3. Same precedence but can't flatten
    if child_prec == parent_prec && !parent_op.can_flatten_with(child_op) {
        return true;
    }

    // 4. Special handling for modulo
    if parent_prec < child_prec && child_op == BinaryOperator::Percent {
        return matches!(parent_op, BinaryOperator::Plus | BinaryOperator::Minus)
            || parent_op.is_bitwise();
    }

    // 5. Bitwise operators with different precedence
    if parent_op.is_bitwise() && child_prec != parent_prec {
        return true;
    }

    false
}
