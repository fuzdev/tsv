//! Comma-list owners keep the source's pairs between a comparison and a later sibling's
//! possible type-argument closer: an authored pair around the comparison, and the pairs the
//! printer's style decides inside the region the two bound.

use super::Printer;
use super::expressions::functions::has_leftmost_arrow_body_parens;
use crate::ast::AstView;
use crate::ast::{
    ArrowFunctionBody, ArrowFunctionExpression, BinaryExpression, BinaryOperator, Decorator,
    Expression, ExpressionKind, ObjectPatternProperty, ObjectProperty, UnaryExpression,
    UnaryOperator,
};
use smallvec::{SmallVec, smallvec};
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
    /// ([`Printer::mark_region_source_pairs`]).
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
    view: AstView<'e>,
    expr: &'e Expression<'e>,
    mut visit: impl FnMut(&'e Expression<'e>, ChildSlot) -> bool,
) -> bool {
    use ChildSlot::{ArrowBody, Element, Other, PropertyValue, TemplateExpression};
    match &expr.kind {
        ExpressionKind::BinaryExpression(binary) => {
            visit(view.expr(binary.left), Other) || visit(view.expr(binary.right), Other)
        }
        ExpressionKind::AssignmentExpression(assign) => {
            visit(view.expr(assign.left), Other) || visit(view.expr(assign.right), Other)
        }
        ExpressionKind::AssignmentPattern(assign) => {
            visit(view.expr(assign.left), Other)
                || visit(view.expr(assign.right), Other)
                || decorators_any(view, view.opt_decorators(assign.decorators), &mut visit)
        }
        ExpressionKind::ObjectExpression(object) => {
            view.obj_props(object.properties)
                .iter()
                .any(|property| match property {
                    ObjectProperty::Property(property) => {
                        visit(view.expr(property.key), Other)
                            || visit(view.expr(property.value), PropertyValue)
                    }
                    ObjectProperty::SpreadElement(spread) => {
                        visit(view.expr(spread.argument), Other)
                    }
                })
        }
        ExpressionKind::ObjectPattern(object) => {
            decorators_any(view, view.opt_decorators(object.decorators), &mut visit)
                || view
                    .pat_props(object.properties)
                    .iter()
                    .any(|property| match property {
                        ObjectPatternProperty::Property(property) => {
                            visit(view.expr(property.key), Other)
                                || visit(view.expr(property.value), Other)
                        }
                        ObjectPatternProperty::RestElement(rest) => {
                            visit(view.expr(rest.argument), Other)
                        }
                    })
        }
        ExpressionKind::ArrayExpression(array) => view
            .opt_list(array.elements)
            .iter()
            .flatten()
            .any(|child| visit(view.expr(*child), Element { list_end: None })),
        ExpressionKind::ArrayPattern(array) => {
            decorators_any(view, view.opt_decorators(array.decorators), &mut visit)
                || view
                    .opt_list(array.elements)
                    .iter()
                    .flatten()
                    .any(|child| visit(view.expr(*child), Other))
        }
        ExpressionKind::CallExpression(call) => {
            let list_end = Some(expr.span.end);
            visit(view.expr(call.callee), Other)
                || view
                    .list(call.arguments)
                    .iter()
                    .any(|child| visit(view.expr(*child), Element { list_end }))
        }
        ExpressionKind::NewExpression(new) => {
            let list_end = Some(expr.span.end);
            visit(view.expr(new.callee), Other)
                || view
                    .list(new.arguments)
                    .iter()
                    .any(|child| visit(view.expr(*child), Element { list_end }))
        }
        ExpressionKind::MemberExpression(member) => {
            visit(view.expr(member.object), Other) || visit(view.expr(member.property), Other)
        }
        ExpressionKind::ConditionalExpression(cond) => {
            visit(view.expr(cond.test), Other)
                || visit(view.expr(cond.consequent), Other)
                || visit(view.expr(cond.alternate), Other)
        }
        &ExpressionKind::ArrowFunctionExpression(arrow) => {
            let arrow = view.arrow_fn(arrow);
            view.run(arrow.params)
                .iter()
                .any(|child| visit(child, Other))
                || match &arrow.body {
                    ArrowFunctionBody::Expression(body) => visit(view.expr(*body), ArrowBody),
                    ArrowFunctionBody::BlockStatement(_) => false,
                }
        }
        &ExpressionKind::FunctionExpression(function) => view
            .run(view.fn_expr(function).params)
            .iter()
            .any(|child| visit(child, Other)),
        &ExpressionKind::ClassExpression(class) => {
            let class = view.class_expr(class);
            decorators_any(view, view.opt_decorators(class.decorators), &mut visit)
                || class
                    .super_class
                    .is_some_and(|child| visit(view.expr(child), Other))
        }
        ExpressionKind::UnaryExpression(unary) => visit(view.expr(unary.argument), Other),
        ExpressionKind::UpdateExpression(update) => visit(view.expr(update.argument), Other),
        ExpressionKind::SpreadElement(spread) => visit(view.expr(spread.argument), Other),
        ExpressionKind::RestElement(rest) => visit(view.expr(rest.argument), Other),
        ExpressionKind::AwaitExpression(await_expr) => visit(view.expr(await_expr.argument), Other),
        ExpressionKind::YieldExpression(yield_expr) => yield_expr
            .argument
            .is_some_and(|child| visit(view.expr(child), Other)),
        ExpressionKind::SequenceExpression(sequence) => view
            .list(sequence.expressions)
            .iter()
            .any(|child| visit(view.expr(*child), Element { list_end: None })),
        ExpressionKind::TemplateLiteral(template) => view
            .list(template.expressions)
            .iter()
            .any(|child| visit(view.expr(*child), TemplateExpression)),
        &ExpressionKind::TaggedTemplateExpression(tagged) => {
            let tagged = view.tagged_tpl(tagged);
            visit(view.expr(tagged.tag), Other)
                || view
                    .list(tagged.quasi.expressions)
                    .iter()
                    .any(|child| visit(view.expr(*child), TemplateExpression))
        }
        ExpressionKind::TSTypeAssertion(assertion) => visit(view.expr(assertion.expression), Other),
        ExpressionKind::TSAsExpression(assertion) => visit(view.expr(assertion.expression), Other),
        ExpressionKind::TSSatisfiesExpression(assertion) => {
            visit(view.expr(assertion.expression), Other)
        }
        ExpressionKind::TSInstantiationExpression(instantiation) => {
            visit(view.expr(instantiation.expression), Other)
        }
        ExpressionKind::TSNonNullExpression(non_null) => {
            visit(view.expr(non_null.expression), Other)
        }
        ExpressionKind::TSParameterProperty(property) => {
            visit(view.expr(property.parameter), Other)
        }
        ExpressionKind::ImportExpression(import) => {
            let list_end = Some(expr.span.end);
            visit(view.expr(import.source), Element { list_end })
                || import
                    .options
                    .is_some_and(|options| visit(view.expr(options), Element { list_end }))
        }
        ExpressionKind::CommentBoundPair(cast) => visit(view.expr(cast.inner), Other),
        ExpressionKind::ParenthesizedExpression(paren) => visit(view.expr(paren.expression), Other),
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
    view: AstView<'e>,
    decorators: Option<&'e [Decorator]>,
    visit: &mut impl FnMut(&'e Expression<'e>, ChildSlot) -> bool,
) -> bool {
    decorators.is_some_and(|items| {
        items
            .iter()
            .any(|item| visit(view.expr(item.expression), ChildSlot::Other))
    })
}

/// Whether an expression or one of its expression children ([`any_expression_child`])
/// holds a binary satisfying `predicate`.
///
/// The walk keeps its own stack of the children still to visit rather than recursing: an
/// operand chain's spine is thousands of links deep at the depth limits, and a frame per
/// link spends an engine's main-thread stack whatever the frame (`docs/cli.md` §The other
/// surfaces). The answer is a pure predicate over the subtree, so the visiting order is
/// free.
fn expression_holds_binary<'e>(
    view: AstView<'e>,
    expr: &'e Expression<'e>,
    predicate: impl Fn(&BinaryExpression) -> bool,
) -> bool {
    let mut pending: SmallVec<[&'e Expression<'e>; 16]> = smallvec![expr];
    while let Some(expr) = pending.pop() {
        if let ExpressionKind::BinaryExpression(binary) = &expr.kind
            && predicate(binary)
        {
            return true;
        }
        any_expression_child(view, expr, |child, _| {
            pending.push(child);
            false
        });
    }
    false
}

/// The key a chain link is recorded under in `Printer::relational_chain_walked_links`: the
/// source its operands cover. A binary node holds no span of its own, and no two binaries
/// cover the same operand source — an ancestor's reaches past its descendant's on one side.
fn chain_link_key(view: AstView<'_>, node: &BinaryExpression) -> Span {
    Span::new(
        view.expr(node.left).span().start,
        view.expr(node.right).span().end,
    )
}

/// Whether `binary` is a `<` / `<<`, the operators that open a type-argument region.
fn opens_region(binary: &BinaryExpression) -> bool {
    matches!(
        binary.operator,
        BinaryOperator::LessThan | BinaryOperator::LeftShift
    )
}

/// Whether `expr` holds a binary `<` / `<<` ([`opens_region`]).
fn holds_region_opener(view: AstView<'_>, expr: &Expression<'_>) -> bool {
    expression_holds_binary(view, expr, opens_region)
}

/// Whether `expr` holds a binary the parser flagged as one that may close an open region
/// (`BinaryExpression::may_close_type_arguments`).
fn holds_region_closer(view: AstView<'_>, expr: &Expression<'_>) -> bool {
    expression_holds_binary(view, expr, |binary| binary.may_close_type_arguments)
}

/// Whether `expr` prints a pair of its own that the pair found around it would double: a
/// sequence, whose own pair is the one the walk finds (`fn((a < b, c), d > (e, f))` would
/// print `((a < b, c))`), and a JSDoc cast, whose own pair holds the comparison, so a
/// redundant one around the cast is left out as prettier leaves it.
///
/// An assignment is not one: its authored pair is kept like any other node's, since a
/// carrier that prints its operands itself — a `for` header's clause — prints no pair of
/// the assignment's style, and stripped there `(x = a < b), e > (f, g)` reads as one
/// generic call.
fn supplies_its_own_pair(expr: &Expression<'_>) -> bool {
    matches!(
        expr.kind,
        ExpressionKind::SequenceExpression(_) | ExpressionKind::CommentBoundPair(_)
    )
}

impl<'a> Printer<'a> {
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
    pub(crate) fn mark_relational_sibling_authored_pairs(
        &self,
        elements: impl Clone
        + DoubleEndedIterator<Item = Option<crate::ast::ExprId>>
        + ExactSizeIterator,
    ) {
        let view = self.view;
        let elements = elements.map(move |element| view.opt(element));
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
            .position(|element| element.is_some_and(|expr| holds_region_opener(view, expr)))
        else {
            return;
        };
        let Some(closer) = elements
            .clone()
            .enumerate()
            .skip(opener + 1)
            .rev()
            .find_map(|(i, element)| {
                element
                    .is_some_and(|expr| holds_region_closer(view, expr))
                    .then_some(i)
            })
        else {
            return;
        };
        for expr in elements.skip(opener).take(closer - opener + 1).flatten() {
            if !self.region_walked_elements.is_marked(expr.span()) {
                self.mark_region_source_pairs(expr, ChildSlot::Element { list_end: None });
            }
        }
    }

    /// The chain twin of [`Self::mark_relational_sibling_authored_pairs`]: a `<` whose left
    /// operand holds a `<` / `<<` and whose right operand holds a binary the parser flagged
    /// `may_close_type_arguments` bounds a region inside one comparison chain, and the
    /// pairs the printer would decide inside both operands follow the source
    /// ([`Self::mark_region_source_pairs`]). Stripped, the author's pair around the left
    /// comparison (``(f < A) < C >> `t` ``) or a `typeof` operand's shell
    /// (``f < typeof (b) < C >> `t` ``) leaves a `<`…`>>` run every parser reads as a
    /// type-argument list. Called by both chain collectors down the chain's left spine,
    /// ahead of any operand's build and ahead of the flattening question, which asks the
    /// marks.
    ///
    /// Answers whether the spine below `node` is covered, so the collector stops asking:
    /// `node` marked its operands, or an earlier walk covered `node`
    /// ([`Self::relational_chain_walked_links`]) — the same chain re-entered by another
    /// layout candidate, or a chain anywhere inside a region a walk already visited, a
    /// comma list's included. Every walk ([`Self::mark_region_source_pairs`]) visits a
    /// binary's two operands with `ChildSlot::Other`, the slot this call gives them, so a
    /// covered link's marks are the ones this call would set; and a deeper link of the same
    /// spine lies inside a covered link's left operand. Each binary is walked once.
    pub(in crate::printer) fn mark_relational_chain_region(&self, node: &BinaryExpression) -> bool {
        if node.operator != BinaryOperator::LessThan {
            return false;
        }
        if self.has_relational_chain_walks.get()
            && self
                .relational_chain_walked_links
                .is_marked(chain_link_key(self.view, node))
        {
            return true;
        }
        // The source preflight: the right operand of an ordinary `<` holds no `>` byte.
        let right = self.view.expr(node.right).span();
        if !self
            .source
            .as_bytes()
            .get(right.start as usize..right.end as usize)
            .is_some_and(|text| text.contains(&b'>'))
        {
            return false;
        }
        self.relational_chain_region_marked(node)
    }

    /// [`Self::mark_relational_chain_region`] past its preflight, out of line: a `>` byte
    /// behind a `<` link is the rare case.
    #[cold]
    fn relational_chain_region_marked(&self, node: &BinaryExpression) -> bool {
        if !(holds_region_opener(self.view, self.view.expr(node.left))
            && holds_region_closer(self.view, self.view.expr(node.right)))
        {
            return false;
        }
        self.mark_region_source_pairs(self.view.expr(node.left), ChildSlot::Other);
        self.mark_region_source_pairs(self.view.expr(node.right), ChildSlot::Other);
        self.relational_chain_walked_links
            .mark(chain_link_key(self.view, node));
        self.has_relational_chain_walks.set(true);
        true
    }

    fn mark_relational_sibling_pair(&self, span: Span) {
        self.relational_sibling_pair_targets.mark(span);
        self.has_relational_sibling_pairs.set(true);
    }

    /// From the `<` holder to the closer ([`Self::mark_relational_sibling_authored_pairs`]),
    /// the pairs the printer otherwise decides print iff the author wrote them, for `expr`
    /// and every expression child of it ([`any_expression_child`], whose boundary —
    /// function, class and block bodies — is this walk's), each node's question whether it
    /// holds a `<` / `<<` ([`opens_region`]) answered bottom-up so the walk stays linear:
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
    ///
    /// The walk keeps its own stack rather than recursing, for the reason
    /// [`expression_holds_binary`] does. It enters the nodes in the order a recursion
    /// would, setting each node's marks on the way down ([`Self::mark_region_node`]), and
    /// decides each node's own pair on the way back up, once every child has answered
    /// whether it holds a `<` / `<<` — the one order that matters, since the marks are sets
    /// and nothing the walk asks reads them.
    fn mark_region_source_pairs<'e>(&self, expr: &'e Expression<'e>, slot: ChildSlot)
    where
        'a: 'e,
    {
        /// A step of the walk: enter a child of the node at `parent` (`None` for the root),
        /// or leave the node at an index into `visited` once its children are done.
        enum Step<'e> {
            Enter(&'e Expression<'e>, ChildSlot, Option<usize>),
            Leave(usize),
        }
        /// A node entered and not yet left: what its pair decision reads.
        struct Visited<'e> {
            expr: &'e Expression<'e>,
            list_end: Option<u32>,
            holds_opener: bool,
            parent: Option<usize>,
        }
        let mut steps: SmallVec<[Step<'e>; 16]> = smallvec![Step::Enter(expr, slot, None)];
        let mut visited: SmallVec<[Visited<'e>; 16]> = SmallVec::new();
        while let Some(step) = steps.pop() {
            match step {
                Step::Enter(expr, slot, parent) => {
                    let list_end = self.mark_region_node(expr, slot);
                    let index = visited.len();
                    visited.push(Visited {
                        expr,
                        list_end,
                        holds_opener: matches!(
                            &expr.kind,
                            ExpressionKind::BinaryExpression(binary) if opens_region(binary)
                        ),
                        parent,
                    });
                    steps.push(Step::Leave(index));
                    let first_child = steps.len();
                    any_expression_child(self.view, expr, |child, slot| {
                        steps.push(Step::Enter(child, slot, Some(index)));
                        false
                    });
                    // reversed, so the first child is entered first, as a recursion would
                    steps[first_child..].reverse();
                }
                Step::Leave(index) => {
                    let node = &visited[index];
                    let (holds_opener, parent) = (node.holds_opener, node.parent);
                    if holds_opener
                        && !supplies_its_own_pair(node.expr)
                        && self.holds_authored_pair(node.expr, node.list_end)
                    {
                        self.mark_relational_sibling_pair(node.expr.span());
                    }
                    if let Some(parent) = parent {
                        visited[parent].holds_opener |= holds_opener;
                    }
                }
            }
        }
    }

    /// The marks [`Self::mark_region_source_pairs`] sets on entering `expr` at `slot`,
    /// ahead of its children — answering the list end its authored-pair question reads.
    fn mark_region_node(&self, expr: &Expression<'_>, slot: ChildSlot) -> Option<u32> {
        // A `<` this walk covers is a chain link whose operands it walks with the slot a
        // direct chain-region walk gives them, so that chain need not walk them again
        // ([`Self::mark_relational_chain_region`]).
        if let ExpressionKind::BinaryExpression(binary) = &expr.kind
            && binary.operator == BinaryOperator::LessThan
        {
            self.relational_chain_walked_links
                .mark(chain_link_key(self.view, binary));
            self.has_relational_chain_walks.set(true);
        }
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
                if self.holds_authored_pair(self.view.expr(assign.left), None) {
                    self.region_kept_shell_targets
                        .mark(self.view.expr(assign.left).span());
                    self.has_region_source_marks.set(true);
                }
                if slot.assignment_pair_follows_source()
                    && !self.holds_authored_pair(expr, list_end)
                {
                    self.mark_region_bare(expr.span());
                }
            }
            &ExpressionKind::ArrowFunctionExpression(arrow) => {
                let arrow = self.view.arrow_fn(arrow);
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
                    match self.view.expr(*body).kind {
                        ExpressionKind::ConditionalExpression(_)
                            if !self.holds_authored_pair(self.view.expr(*body), None) =>
                        {
                            self.mark_region_bare(self.view.expr(*body).span());
                        }
                        // A curried chain: printed bare, a head's `=>` follows another
                        // inside the region, a line tsv's own parse can refuse
                        // (`fn(a < b, () => c => c > (d, e))`), so a pair the author wrote
                        // around the chain or around its next head stays.
                        ExpressionKind::ArrowFunctionExpression(_) => {
                            if self.holds_authored_pair(expr, list_end) {
                                self.mark_relational_sibling_pair(expr.span());
                            }
                            if self.holds_authored_pair(self.view.expr(*body), None) {
                                self.mark_relational_sibling_pair(self.view.expr(*body).span());
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
        list_end
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
    fn mark_prefix_operator_shells(&self, unary: &UnaryExpression) {
        let mut node = self.view.expr(unary.argument);
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
                ExpressionKind::MemberExpression(member) => self.view.expr(member.object),
                ExpressionKind::CallExpression(call) => self.view.expr(call.callee),
                &ExpressionKind::TaggedTemplateExpression(tagged) => {
                    self.view.expr(self.view.tagged_tpl(tagged).tag)
                }
                ExpressionKind::TSNonNullExpression(non_null) => {
                    self.view.expr(non_null.expression)
                }
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
    /// `shouldAddParensIfNotBreak`) — every conditional body but one whose leftmost token takes
    /// its own pair ([`has_leftmost_arrow_body_parens`]: `({}).x ? a : b` prints bare, the pair
    /// around the object alone) and one a region leaves bare.
    #[inline]
    pub(in crate::printer) fn arrow_body_takes_ternary_pair(&self, body: &Expression<'_>) -> bool {
        matches!(body.kind, ExpressionKind::ConditionalExpression(_))
            && !has_leftmost_arrow_body_parens(self.view, body)
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
    /// `crate::__private::build_bare_sequence_operand_docs`: the authored pair around a node
    /// holding a comparison ([`Self::wrap_relational_sibling_pair`]), and the authored pair around
    /// an assignment operand inside the region, which a sequence prints as the pair of its own
    /// style and the region rules leave to the source (`(x = a < b), c > (d, e)`;
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
