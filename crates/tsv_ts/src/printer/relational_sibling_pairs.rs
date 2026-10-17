//! Comma-list owners keep the source's pairs between a comparison and a later sibling's
//! possible type-argument closer: an authored pair around the comparison, and the pairs the
//! printer's style decides inside the region the two bound.

use super::Printer;
use crate::ast::internal::{
    ArrowFunctionBody, ArrowFunctionExpression, BinaryExpression, BinaryOperator, Decorator,
    Expression, ExpressionKind, ObjectPatternProperty, ObjectProperty, UnaryExpression,
    UnaryOperator,
};
use tsv_lang::Span;
use tsv_lang::doc::arena::DocId;

/// The position an expression child stands at, as far as the region rules ask
/// ([`Printer::mark_region_source_pairs`]): the positions where the printer's style puts a
/// pair around an assignment, and the one where it puts a pair around a conditional.
#[derive(Clone, Copy)]
enum ChildSlot {
    /// A call, `new` or `import()` argument (`list_end` is the call's own end, just past
    /// its `)`), an array element or a sequence operand (`list_end` is `None`).
    Element { list_end: Option<u32> },
    /// An object literal property's value.
    PropertyValue,
    /// A template literal's `${…}` substitution, tagged or not.
    TemplateExpression,
    /// An arrow function's expression body.
    ArrowBody,
    /// Every other position.
    Other,
}

impl ChildSlot {
    /// Whether an assignment standing here takes the pair the printer's style puts around
    /// it — every position but `Other` — so that, in a region, the pair follows the source
    /// ([`Printer::mark_region_source_pairs`]) and is the assignment's own
    /// ([`supplies_its_own_pair`]).
    fn assignment_pair_follows_source(self) -> bool {
        !matches!(self, Self::Other)
    }
}

/// Visit each expression child of `expr` with the position it stands at, stopping at the
/// first `visit` answers `true` for (and answering whether one did). Function, class and
/// block bodies are the walk's boundary: their statements and members are not expression
/// children. Parameters, decorators, a superclass and an arrow's expression body are
/// children and are visited; types contain no value expressions.
///
/// The one notion of an expression child the comma-list owner's questions share, so the
/// opener, the closer and the region marks cannot drift. Every wrapper is explicit; a new
/// expression kind must settle its own children.
fn any_expression_child<'e>(
    expr: &'e Expression<'e>,
    mut visit: impl FnMut(&'e Expression<'e>, ChildSlot) -> bool,
) -> bool {
    use ChildSlot::{ArrowBody, Element, Other, PropertyValue, TemplateExpression};
    match &expr.kind {
        ExpressionKind::BinaryExpression(binary) => {
            visit(binary.left, Other) || visit(binary.right, Other)
        }
        ExpressionKind::AssignmentExpression(assign) => {
            visit(assign.left, Other) || visit(assign.right, Other)
        }
        ExpressionKind::AssignmentPattern(assign) => {
            visit(assign.left, Other)
                || visit(assign.right, Other)
                || decorators_any(assign.decorators, &mut visit)
        }
        ExpressionKind::ObjectExpression(object) => {
            object.properties.iter().any(|property| match property {
                ObjectProperty::Property(property) => {
                    visit(property.key, Other) || visit(property.value, PropertyValue)
                }
                ObjectProperty::SpreadElement(spread) => visit(spread.argument, Other),
            })
        }
        ExpressionKind::ObjectPattern(object) => {
            decorators_any(object.decorators, &mut visit)
                || object.properties.iter().any(|property| match property {
                    ObjectPatternProperty::Property(property) => {
                        visit(property.key, Other) || visit(property.value, Other)
                    }
                    ObjectPatternProperty::RestElement(rest) => visit(rest.argument, Other),
                })
        }
        ExpressionKind::ArrayExpression(array) => array
            .elements
            .iter()
            .flatten()
            .any(|child| visit(child, Element { list_end: None })),
        ExpressionKind::ArrayPattern(array) => {
            decorators_any(array.decorators, &mut visit)
                || array
                    .elements
                    .iter()
                    .flatten()
                    .any(|child| visit(child, Other))
        }
        ExpressionKind::CallExpression(call) => {
            let list_end = Some(expr.span.end);
            visit(call.callee, Other)
                || call
                    .arguments
                    .iter()
                    .any(|child| visit(child, Element { list_end }))
        }
        ExpressionKind::NewExpression(new) => {
            let list_end = Some(expr.span.end);
            visit(new.callee, Other)
                || new
                    .arguments
                    .iter()
                    .any(|child| visit(child, Element { list_end }))
        }
        ExpressionKind::MemberExpression(member) => {
            visit(member.object, Other) || visit(member.property, Other)
        }
        ExpressionKind::ConditionalExpression(cond) => {
            visit(cond.test, Other) || visit(cond.consequent, Other) || visit(cond.alternate, Other)
        }
        ExpressionKind::ArrowFunctionExpression(arrow) => {
            arrow.params.iter().any(|child| visit(child, Other))
                || match &arrow.body {
                    ArrowFunctionBody::Expression(body) => visit(body, ArrowBody),
                    ArrowFunctionBody::BlockStatement(_) => false,
                }
        }
        ExpressionKind::FunctionExpression(function) => {
            function.params.iter().any(|child| visit(child, Other))
        }
        ExpressionKind::ClassExpression(class) => {
            decorators_any(class.decorators, &mut visit)
                || class.super_class.is_some_and(|child| visit(child, Other))
        }
        ExpressionKind::UnaryExpression(unary) => visit(unary.argument, Other),
        ExpressionKind::UpdateExpression(update) => visit(update.argument, Other),
        ExpressionKind::SpreadElement(spread) => visit(spread.argument, Other),
        ExpressionKind::RestElement(rest) => visit(rest.argument, Other),
        ExpressionKind::AwaitExpression(await_expr) => visit(await_expr.argument, Other),
        ExpressionKind::YieldExpression(yield_expr) => {
            yield_expr.argument.is_some_and(|child| visit(child, Other))
        }
        ExpressionKind::SequenceExpression(sequence) => sequence
            .expressions
            .iter()
            .any(|child| visit(child, Element { list_end: None })),
        ExpressionKind::TemplateLiteral(template) => template
            .expressions
            .iter()
            .any(|child| visit(child, TemplateExpression)),
        ExpressionKind::TaggedTemplateExpression(tagged) => {
            visit(tagged.tag, Other)
                || tagged
                    .quasi
                    .expressions
                    .iter()
                    .any(|child| visit(child, TemplateExpression))
        }
        ExpressionKind::TSTypeAssertion(assertion) => visit(assertion.expression, Other),
        ExpressionKind::TSAsExpression(assertion) => visit(assertion.expression, Other),
        ExpressionKind::TSSatisfiesExpression(assertion) => visit(assertion.expression, Other),
        ExpressionKind::TSInstantiationExpression(instantiation) => {
            visit(instantiation.expression, Other)
        }
        ExpressionKind::TSNonNullExpression(non_null) => visit(non_null.expression, Other),
        ExpressionKind::TSParameterProperty(property) => visit(property.parameter, Other),
        ExpressionKind::ImportExpression(import) => {
            let list_end = Some(expr.span.end);
            visit(import.source, Element { list_end })
                || import
                    .options
                    .is_some_and(|options| visit(options, Element { list_end }))
        }
        ExpressionKind::JsdocCast(cast) => visit(cast.inner, Other),
        ExpressionKind::ParenthesizedExpression(paren) => visit(paren.expression, Other),
        ExpressionKind::Literal(_)
        | ExpressionKind::Identifier(_)
        | ExpressionKind::PrivateIdentifier(_)
        | ExpressionKind::RegexLiteral(_)
        | ExpressionKind::ThisExpression(_)
        | ExpressionKind::Super(_)
        | ExpressionKind::MetaProperty(_) => false,
    }
}

/// Whether any decorator's expression satisfies `visit` — a decorator is an `Other` child.
fn decorators_any<'e>(
    decorators: Option<&'e [Decorator<'e>]>,
    visit: &mut impl FnMut(&'e Expression<'e>, ChildSlot) -> bool,
) -> bool {
    decorators.is_some_and(|items| {
        items
            .iter()
            .any(|item| visit(&item.expression, ChildSlot::Other))
    })
}

/// Whether an expression or one of its expression children ([`any_expression_child`])
/// holds a binary satisfying `predicate`.
fn expression_holds_binary<'e>(
    expr: &'e Expression<'e>,
    predicate: impl Copy + Fn(&BinaryExpression<'_>) -> bool,
) -> bool {
    if let ExpressionKind::BinaryExpression(binary) = &expr.kind
        && predicate(binary)
    {
        return true;
    }
    any_expression_child(expr, |child, _| expression_holds_binary(child, predicate))
}

/// Whether `binary` is a `<` / `<<`, the operators that open a type-argument region.
fn opens_region(binary: &BinaryExpression<'_>) -> bool {
    matches!(
        binary.operator,
        BinaryOperator::LessThan | BinaryOperator::LeftShift
    )
}

/// Whether `expr` holds a binary `<` / `<<` ([`opens_region`]).
fn holds_region_opener(expr: &Expression<'_>) -> bool {
    expression_holds_binary(expr, opens_region)
}

/// Whether `expr` holds a binary the parser flagged as one that may close an open region
/// (`BinaryExpression::may_close_type_arguments`).
fn holds_region_closer(expr: &Expression<'_>) -> bool {
    expression_holds_binary(expr, |binary| binary.may_close_type_arguments)
}

/// Whether the pair the author wrote around `expr` at `slot` is one the printer decides on
/// its own terms — a sequence's or a JSDoc cast's, which it always prints, or an
/// assignment's at a position where the region rules already make that pair follow the
/// source ([`Printer::mark_region_source_pairs`]) — so keeping it again would wrap twice.
fn supplies_its_own_pair(expr: &Expression<'_>, slot: ChildSlot) -> bool {
    match expr.kind {
        ExpressionKind::SequenceExpression(_) | ExpressionKind::JsdocCast(_) => true,
        ExpressionKind::AssignmentExpression(_) => slot.assignment_pair_follows_source(),
        _ => false,
    }
}

impl Printer<'_> {
    /// The comma-list owner's region rules, asked before any element doc is built,
    /// including alternate layouts, so a shared argument doc cannot predate its marks.
    ///
    /// A region runs from the first non-last element holding a binary `<` / `<<` to the
    /// last later element holding a binary the parser flagged `may_close_type_arguments`.
    /// The closer is the parser's flag, never a guess from its operator or follower;
    /// grouping pairs re-anchor the region, so a pair kept or left out by these rules
    /// cannot change that flag on the next pass. Inside the `<` holder and every later
    /// element, the closer's own included, the pairs the printer would decide follow the
    /// source ([`Self::mark_region_source_pairs`]): an authored pair around a node holding
    /// a `<` / `<<` is kept, the element itself included, as is one around a curried chain
    /// or its next head, and none of the printer's style is added.
    ///
    /// No pair is ever synthesized where the source has none. Array holes remain list
    /// positions. The source preflight — a `<` byte ahead of the last element — keeps the
    /// expression walks off every list that cannot open a region; the opener is found
    /// once from the left and the closer once from the right, never per candidate, and
    /// an element an enclosing list's walk already covered is not walked again
    /// ([`Self::region_walked_elements`]).
    pub(crate) fn mark_relational_sibling_authored_pairs<'e>(
        &self,
        elements: impl Clone
        + DoubleEndedIterator<Item = Option<&'e Expression<'e>>>
        + ExactSizeIterator,
    ) {
        let len = elements.len();
        if len < 2 {
            return;
        }
        // The source the elements ahead of the last one cover. A tree built for printing
        // need not hold its elements in source order, so the window is their hull, and a
        // span outside the source skips nothing.
        let (start, end) = elements
            .clone()
            .take(len - 1)
            .flatten()
            .fold((u32::MAX, 0), |(start, end), expr| {
                (start.min(expr.span().start), end.max(expr.span().end))
            });
        if start < end
            && self
                .source
                .as_bytes()
                .get(start as usize..end as usize)
                .is_some_and(|heads| !heads.contains(&b'<'))
        {
            return;
        }
        let Some(opener) = elements
            .clone()
            .take(len - 1)
            .position(|element| element.is_some_and(holds_region_opener))
        else {
            return;
        };
        let Some(closer) = elements
            .clone()
            .enumerate()
            .skip(opener + 1)
            .rev()
            .find_map(|(i, element)| element.is_some_and(holds_region_closer).then_some(i))
        else {
            return;
        };
        for expr in elements.skip(opener).take(closer - opener + 1).flatten() {
            if !self.region_walked_elements.is_marked(expr.span()) {
                self.mark_region_source_pairs(expr, ChildSlot::Element { list_end: None });
            }
        }
    }

    fn mark_relational_sibling_pair(&self, span: Span) {
        self.relational_sibling_pair_targets.mark(span);
        self.has_relational_sibling_pairs.set(true);
    }

    /// From the `<` holder to the closer ([`Self::mark_relational_sibling_authored_pairs`]),
    /// the pairs the printer otherwise decides print iff the author wrote them, for `expr`
    /// and every expression child of it ([`any_expression_child`], whose boundary —
    /// function, class and block bodies — is this walk's), answering whether `expr` holds a
    /// `<` / `<<` ([`opens_region`]), bottom-up so the walk stays linear:
    ///
    /// - a pair around a node holding a `<` / `<<`, at whatever position, unless the node
    ///   prints a pair of its own there ([`supplies_its_own_pair`])
    ///   ([`Self::relational_sibling_pair_is_kept`]), and one around a curried chain or
    ///   around its next head, whatever the head holds;
    /// - an assignment at a position whose pair is style — an argument, an array element,
    ///   a sequence operand, a property value, a template substitution, an arrow body —
    ///   and an arrow function's conditional body ([`Self::region_withholds_pair`]);
    /// - an arrow function's lone parameter ([`Self::region_param_is_bare`]);
    /// - the pair directly behind `typeof` or a unary `-`, and an assignment target's own
    ///   shell (`(c) = d`) ([`Self::region_keeps_shell`]).
    ///
    /// A pair the printer adds there opens on text tsc's recovering type-argument parse
    /// completes — `(c ? d : e)` and `(c = d)` as a function type's parameters, `(x) => x`
    /// as a function type — and a pair it strips there leaves a type behind (`typeof c`,
    /// `-1`) or a comparison whose operands run on past the element's comma, into a nested
    /// list or a `>>` read as two `>`s (`x => c < d`, `c << d < e`), so between the `<`
    /// and the `>` a line the author wrote as two comparisons would read as one generic
    /// call; a curried chain stripped of its pair puts a head's `=>` past another, a line
    /// tsv's own parse can refuse. Following the source changes no token there, so
    /// every parser reads the output as it read the input, and each authoring is its own
    /// fixed point. The tree decides what is marked and the source only whether the pair
    /// was written; what the tree requires (`(c = d) + 1`, `() => ({})`, a sequence's own
    /// pair) is never asked.
    fn mark_region_source_pairs(&self, expr: &Expression<'_>, slot: ChildSlot) -> bool {
        let list_end = match slot {
            ChildSlot::Element { list_end } => {
                self.region_walked_elements.mark(expr.span());
                list_end
            }
            ChildSlot::PropertyValue
            | ChildSlot::TemplateExpression
            | ChildSlot::ArrowBody
            | ChildSlot::Other => None,
        };
        match &expr.kind {
            ExpressionKind::AssignmentExpression(assign) => {
                // The target's own shell (`(c) = d`): with the assignment's pair gone, a
                // stripped one would leave `c = d` where the author's `(` stood, which
                // inside an authored sequence pair (`(y, c = d)`) tsc's recovery reads as
                // a function type's parameters.
                if self.holds_authored_pair(assign.left, None) {
                    self.region_kept_shell_targets.mark(assign.left.span());
                    self.has_region_source_marks.set(true);
                }
                if slot.assignment_pair_follows_source()
                    && !self.holds_authored_pair(expr, list_end)
                {
                    self.mark_region_bare(expr.span());
                }
            }
            ExpressionKind::ArrowFunctionExpression(arrow) => {
                // `params_start` is the parameter list's `(`: absent, the lone parameter
                // was written bare.
                if arrow.params_start.is_none() {
                    self.mark_region_bare(expr.span());
                }
                // TODO: an arrow's conditional body takes its pair only while it stays on the
                // `=>` line, a kept one included, so an authored pair around a conditional
                // whose own tail ends on a comparison is lost once the body breaks
                // (`(x) =>⏎a ? b : c < d,`), and the line reads as a generic call on the next
                // pass.
                if let ArrowFunctionBody::Expression(body) = &arrow.body {
                    match body.kind {
                        ExpressionKind::ConditionalExpression(_)
                            if !self.holds_authored_pair(body, None) =>
                        {
                            self.mark_region_bare(body.span());
                        }
                        // A curried chain: printed bare, a head's `=>` follows another
                        // inside the region, a line tsv's own parse can refuse
                        // (`fn(a < b, () => c => c > (d, e))`), so a pair the author wrote
                        // around the chain or around its next head stays.
                        ExpressionKind::ArrowFunctionExpression(_) => {
                            if self.holds_authored_pair(expr, list_end) {
                                self.mark_relational_sibling_pair(expr.span());
                            }
                            if self.holds_authored_pair(body, None) {
                                self.mark_relational_sibling_pair(body.span());
                            }
                        }
                        _ => {}
                    }
                }
            }
            ExpressionKind::UnaryExpression(unary)
                if matches!(unary.operator, UnaryOperator::Typeof | UnaryOperator::Minus) =>
            {
                self.mark_prefix_operator_shells(unary);
            }
            _ => {}
        }
        let mut holds_opener =
            matches!(&expr.kind, ExpressionKind::BinaryExpression(binary) if opens_region(binary));
        any_expression_child(expr, |child, slot| {
            holds_opener |= self.mark_region_source_pairs(child, slot);
            false
        });
        if holds_opener
            && !supplies_its_own_pair(expr, slot)
            && self.holds_authored_pair(expr, list_end)
        {
            self.mark_relational_sibling_pair(expr.span());
        }
        holds_opener
    }

    fn mark_region_bare(&self, span: Span) {
        self.region_bare_targets.mark(span);
        self.has_region_source_marks.set(true);
    }

    /// Whether the author wrote a grouping pair directly around `expr`: a `(` is the token
    /// before it and a `)` the token after it, whitespace and comments stepped over. Asked
    /// only at a position whose own delimiters are no `(` — except a call argument's, whose
    /// list `(` is the token before its first argument: `list_end`, just past the call's
    /// `)`, tells that `)` from a pair inside it.
    fn holds_authored_pair(&self, expr: &Expression<'_>, list_end: Option<u32>) -> bool {
        self.paren_shell_close_after(expr.span().end)
            .is_some_and(|close| list_end.is_none_or(|end| close < end))
            && self.opens_behind_paren(expr)
    }

    /// Whether the token before `expr` is a `(`, whitespace and comments stepped over. A
    /// span outside the source — a tree a tool built for printing — answers no.
    fn opens_behind_paren(&self, expr: &Expression<'_>) -> bool {
        let start = expr.span().start;
        self.source.is_char_boundary(start as usize)
            && self.prev_significant_byte(start) == Some(b'(')
    }

    /// Mark the pairs the author wrote around `unary`'s operand and the heads it opens on: the
    /// pair directly behind the operator around the operand itself (`typeof (c)`, `-(1)`) or
    /// around the head of a longer operand whose shell holds only that head (`typeof (c)[d]`,
    /// `typeof (c.d).e`, `typeof (c)()`, `` typeof (c)`x` ``, `typeof (c)!`) — the member
    /// object, callee, tag or non-null operand its printed form opens on — and, inside a
    /// pair that is marked, each further one around the head it opens on in turn
    /// (`typeof ((c)())`). A span that opens on a `(` the node's own token does not belong
    /// to is one whose head carries the shell.
    ///
    /// The nested pair is what keeps a comment between the operator and a head's shell a
    /// fixed point: the comment prints inside a pair of its own (`typeof (/* j */ (c)())`),
    /// which is the operand's shell on the next pass, and the head's shell inside it is
    /// still the author's.
    fn mark_prefix_operator_shells(&self, unary: &UnaryExpression<'_>) {
        let mut node = unary.argument;
        // Where the last marked pair's content starts: a head starting there too stands
        // behind that same `(`, which is not a pair of its own.
        let mut marked_start = None;
        loop {
            if marked_start != Some(node.span().start) && self.opens_behind_paren(node) {
                if self.paren_shell_close_after(node.span().end).is_none() {
                    return;
                }
                self.region_kept_shell_targets.mark(node.span());
                self.has_region_source_marks.set(true);
                marked_start = Some(node.span().start);
            }
            if self.source.as_bytes().get(node.span().start as usize) != Some(&b'(') {
                return;
            }
            node = match &node.kind {
                ExpressionKind::MemberExpression(member) => member.object,
                ExpressionKind::CallExpression(call) => call.callee,
                ExpressionKind::TaggedTemplateExpression(tagged) => tagged.tag,
                ExpressionKind::TSNonNullExpression(non_null) => non_null.expression,
                _ => return,
            };
        }
    }

    /// Whether the region rules leave out the pair the printer's style puts around `expr`
    /// — an assignment at a style position, or an arrow function's conditional body
    /// ([`Self::mark_region_source_pairs`]). A document-level fast gate keeps every
    /// ordinary expression off the set.
    #[inline]
    pub(in crate::printer) fn region_withholds_pair(&self, expr: &Expression<'_>) -> bool {
        self.has_region_source_marks.get() && self.region_bare_targets.is_marked(expr.span())
    }

    /// Whether the arrow function spanning `arrow_span` prints its lone parameter without
    /// the pair the printer's style adds, as the author wrote it in a region.
    #[inline]
    pub(in crate::printer) fn region_param_is_bare(
        &self,
        arrow: &ArrowFunctionExpression<'_>,
        arrow_span: Span,
    ) -> bool {
        arrow.params_start.is_none()
            && self.has_region_source_marks.get()
            && self.region_bare_targets.is_marked(arrow_span)
    }

    /// Whether `expr` prints inside the pair the author wrote directly behind a `typeof`
    /// or a unary `-` in a region, or around an assignment's target there — a pair its
    /// position would otherwise strip.
    #[inline]
    pub(in crate::printer) fn region_keeps_shell(&self, expr: &Expression<'_>) -> bool {
        self.has_region_source_marks.get() && self.region_kept_shell_targets.is_marked(expr.span())
    }

    /// Whether an arrow function's expression body `body` takes the conditional's pair the
    /// arrow prints around it while it stays on the `=>` line (prettier's
    /// `shouldAddParensIfNotBreak`) — every conditional body but one a region leaves bare.
    #[inline]
    pub(in crate::printer) fn arrow_body_takes_ternary_pair(&self, body: &Expression<'_>) -> bool {
        matches!(body.kind, ExpressionKind::ConditionalExpression(_))
            && !self.region_withholds_pair(body)
    }

    /// `doc` inside the authored pair the list owner kept around `expr`
    /// ([`Self::relational_sibling_pair_is_kept`]), for a carrier that prints its elements
    /// itself rather than through `needs_parens(Argument)`. `already_parenthesized` is the
    /// position's own pair around the same element (a `for` header's `[~In]` pair), which
    /// is the kept pair when present: wrapping again would print it twice.
    pub(in crate::printer) fn wrap_relational_sibling_pair(
        &self,
        expr: &Expression<'_>,
        already_parenthesized: bool,
        doc: DocId,
    ) -> DocId {
        if !already_parenthesized && self.relational_sibling_pair_is_kept(expr) {
            self.d().parens(doc)
        } else {
            doc
        }
    }

    /// `doc` inside the pair a comma-list region keeps around a **bare** list's operand
    /// `expr` — an operand its host prints without a sequence's pair, through
    /// [`crate::build_bare_sequence_operand_docs`]: the authored pair around a node holding
    /// a comparison ([`Self::wrap_relational_sibling_pair`]), and the authored pair around
    /// an assignment operand inside the region, which a sequence prints as the pair of its
    /// own style and the region rules leave to the source (`(x = a < b), c > (d, e)`;
    /// [`Self::mark_region_source_pairs`]).
    pub(crate) fn wrap_bare_list_operand(&self, expr: &Expression<'_>, doc: DocId) -> DocId {
        if matches!(expr.kind, ExpressionKind::AssignmentExpression(_))
            && self.region_walked_elements.is_marked(expr.span())
            && !self.region_withholds_pair(expr)
        {
            self.d().parens(doc)
        } else {
            self.wrap_relational_sibling_pair(expr, false, doc)
        }
    }

    /// An assignment's value through the value shell
    /// ([`Self::build_expression_doc_with_paren_comments`]), inside the authored pair the
    /// list owner kept around it ([`Self::relational_sibling_pair_is_kept`]): the kept pair
    /// is the position's pair, so a comment in its gap stays inside it and the shell is
    /// never wrapped twice.
    pub(in crate::printer) fn build_value_doc_keeping_relational_sibling_pair(
        &self,
        expr: &Expression<'_>,
        boundary_end: u32,
    ) -> DocId {
        let kept = self.relational_sibling_pair_is_kept(expr);
        let inner = self.build_expression_doc_with_paren_comments(expr, boundary_end, kept);
        self.wrap_value_position_parens(expr, boundary_end, kept, inner)
    }

    /// The pair recorded by the list owner, with a document-level fast gate so an ordinary
    /// expression never probes the set. Marks are never consumed: every layout candidate
    /// asks the same question of the same span.
    #[inline]
    pub(in crate::printer) fn relational_sibling_pair_is_kept(
        &self,
        expr: &Expression<'_>,
    ) -> bool {
        self.has_relational_sibling_pairs.get()
            && self.relational_sibling_pair_targets.is_marked(expr.span())
    }
}
