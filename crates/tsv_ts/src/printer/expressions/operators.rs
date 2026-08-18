// Operator expression printing for TypeScript
//
// Handles printing of unary and binary expressions with:
// - Operator precedence and parenthesization
// - Clarity-based parens (mixing logical operators, etc.)

use crate::ast::internal::{self, BinaryOperator, Expression};
use crate::printer::comments::{CommentSpacing, KeywordOperandGap};
use crate::printer::{CommentVec, ParenContext, Printer, RunLeadingBlank};
use smallvec::{SmallVec, smallvec};
use tsv_lang::Span;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::{DocBuf, arena::DocId};

/// Holds information about an operand in a binary expression chain
/// Used to track position information for comment placement
struct ChainOperand {
    doc: DocId,
    span: Span,
}

/// Stack buffers for a flattened binary chain's operands / operators, collected
/// once per binary expression. The common 2–3 operand chain stays inline
/// (`ChainOperand` is 12 bytes, `BinaryOperator` 1 byte); longer chains spill.
type OperandBuf = SmallVec<[ChainOperand; 8]>;
pub(super) type OperatorBuf = SmallVec<[BinaryOperator; 8]>;

/// Style for building binary expression chain docs
#[derive(Clone, Copy)]
enum BinaryChainStyle {
    /// Wrapped in a group, flat structure (for standalone binary expressions)
    Grouped,
    /// No group wrapper, flat structure (for contexts where parent controls breaking)
    Ungrouped,
    /// Like Ungrouped, but also suppresses shouldGroup for logical operators.
    /// Used only for condition parentheses (if/while/for/do-while/switch) where
    /// Prettier's `isInsideParenthesis` is true. In these contexts, logical chain
    /// breaks must be controlled by the parent condition group, not a sub-group.
    UngroupedCondition,
    /// First operand at base indent, continuation lines indented — the positions
    /// where prettier's shouldNotIndent chain yields false (nested binary
    /// operands, a type assertion's operand, an Embedded expression root,
    /// ternary tests under return/throw/call/new).
    ContinuationIndent,
    /// `ContinuationIndent` without the outer group wrapper, for contexts where
    /// the caller controls grouping (the comment-aware twin for inline-paren
    /// chains).
    ContinuationIndentUngrouped,
}

/// How a `SequenceExpression`'s own grouping parens are emitted. One 3-valued mode
/// rather than two independent bools — the fourth combination (bare *and*
/// keep-trailing-inside) is impossible (a bare sequence has no `)` to keep a comment
/// before), so an enum makes it unrepresentable.
#[derive(Clone, Copy)]
enum SeqParens {
    /// Self-parenthesize; the last operand's trailing comment floats OUT after `)`
    /// (statement / throw / call-argument position).
    FloatOut,
    /// Self-parenthesize; the last operand's trailing comment stays INSIDE before `)`
    /// (value position — return arg / var init / assignment RHS; prettier #19263).
    KeepInside,
    /// No parens — bare comma-joined operands; an enclosing construct supplies the
    /// grouping and owns the edge-comment gaps (a leading-comment-forced hanging paren).
    Bare,
}

/// Where a `SequenceExpression`'s operands sit once the list breaks — prettier's
/// `printSequenceExpression` (`print/sequence-expression.js`), whose three arms are keyed on
/// the sequence's PARENT.
///
/// Orthogonal to [`SeqParens`]: that names what the parens do with an edge comment, this names
/// the geometry between them. tsv has no parent pointer, so each caller names the layout its
/// position takes — the same way it already names the paren mode.
#[derive(Clone, Copy)]
pub(in crate::printer) enum SeqLayout {
    /// `group(join([",", line]))` — every operand at the caller's own indent. Prettier's
    /// default arm, and so the common one: a call argument, a variable init, an assignment
    /// RHS, a `yield` argument, a nested operand.
    Aligned,
    /// `group([first, ",", indent([line, rest…])])` — operands after the first take one
    /// indent level, prettier's `ExpressionStatement` / `ForStatement` arm (the two positions
    /// where a sequence appears unparenthesized in source). ⚠️ The indent starts AFTER the
    /// first operand and not around the whole run: an operand that breaks internally keeps
    /// its own lines at the base column.
    Indented,
    /// `group(indent(softline + join) + softline)` — the whole list hangs inside the parens,
    /// `)` on its own line. Prettier's `shouldIndentSequenceExpression`: a `return`/`throw`
    /// argument and an arrow-function body. (Prettier writes it as an `ifBreak` whose flat arm
    /// is the bare join; the softlines already collapse flat, so the arms coincide.)
    Hanging,
}

/// Operator position in source, used for comment splitting
#[derive(Clone, Copy)]
struct OperatorPosition {
    /// Start position of operator in source
    start: u32,
    /// End position of operator in source (start + operator length)
    end: u32,
}

impl<'a> Printer<'a> {
    /// Build a Doc for an update expression
    pub(in crate::printer) fn build_update_doc(
        &self,
        update: &internal::UpdateExpression<'_>,
    ) -> DocId {
        let d = self.d();
        let operator_doc = d.text(update.operator.as_str());
        let operator_len = update.operator.as_str().len() as u32;

        // Postfix `x++` / `x--`: the operand→operator gap is ASI-sensitive — `a // c⏎++`
        // parses as `a;` then `++;`, so the operator may not start a line.
        //
        // That does not make the gap block-only, though. A grouping paren shell holds the
        // line terminator off the gap, so `(a // c⏎)++` really is a postfix update
        // carrying a `//` — and inlining it swallowed the `++;` into the comment. Such a
        // comment keeps the shell (`asi_gap_needs_parens`), which also supplies the
        // `(a as T)++` parens, so nothing below adds a second pair.
        //
        // Asked BEFORE the operand's doc is built: the shell builds its own operand doc,
        // so building one here first would be discarded — and `build_expression_doc` is
        // not side-effect-free (it consumes the expression-statement paren target), which
        // makes a discarded build more than wasted work.
        if !update.prefix
            && let Some(shell) = self.build_asi_operand_shell_doc(
                update.span.start,
                update.argument,
                update.span.end - operator_len,
            )
        {
            return d.concat(&[shell, operator_doc]);
        }

        let argument_doc = self.build_expression_doc(update.argument);
        // A type-assertion operand keeps its parens: `(a as T)++` (bare
        // `a as T++` binds `++` to `T`).
        let argument_doc = if self.needs_parens(update.argument, ParenContext::UpdateArgument) {
            d.parens(argument_doc)
        } else {
            argument_doc
        };

        if update.prefix {
            // Prefix: ++x, --x. The operator→operand gap has no other emitter, so a
            // comment authored there is dropped unless this builder claims it. Emit-axis,
            // so a comment the operand OWNS (a glued block, an annotation, a JSDoc cast)
            // returns `None` and keeps printing from the operand's own doc — claiming it
            // here too would double-print it.
            //
            // `Adjacent`, not the `AdjacentGlued` of the assignment/call pull-up: an
            // update operand is not a value gap, so prettier does not reflow the author's
            // break after a glued block — `++/* c */⏎x` keeps the break. Gluing pulled the
            // operand up to `++/* c */ x` instead.
            let operator_end = update.span.start + operator_len;
            match self.build_rhs_comments_opt(operator_end, update.argument.span().start) {
                Some(comments) => d.concat(&[operator_doc, comments, argument_doc]),
                None => d.concat(&[operator_doc, argument_doc]),
            }
        } else {
            // Postfix, no shell needed (see above): the same unclaimed gap on the other
            // side, emitted **inline** rather than hanging the operator — a break would
            // rewrite the program. Only a single-line block reaches here (`x /* c */++`);
            // anything spanning lines took the shell.
            let operator_start = update.span.end - operator_len;
            let comments =
                self.build_inline_comments_between_doc(update.argument.span().end, operator_start);
            d.concat(&[argument_doc, comments, operator_doc])
        }
    }

    /// Build a Doc for a unary expression
    pub(in crate::printer) fn build_unary_doc(
        &self,
        unary: &internal::UnaryExpression<'_>,
    ) -> DocId {
        let d = self.d();

        // Check for comments between operator and argument.
        // When grouping parens containing a JSDoc comment are stripped by the parser,
        // the comment ends up in the gap between operator and argument span.
        // Re-add parens to preserve the comment: `!(/** @type {T} */ expr.prop)`
        let operator_end = unary.span.start + unary.operator.as_str().len() as u32;
        let argument_start = unary.argument.span().start;
        let argument_end = unary.argument.span().end;
        // Unary argument — an `ancestorNameMap` value position. A BARE ternary operand
        // records nothing (prettier's `child === node` guard), which is why `!(c ? a : b)`
        // keeps the hanging form while `!((c ? a : b) as T)` expands.
        self.mark_ternary_extra_indent(unary.argument);
        // The shared leading-comment emitter, in its plain `Adjacent` mode — prettier's
        // `printLeadingComment` exactly. Its third separator is what makes the shell below
        // a width decision rather than a comment one: a single-line block glued to the
        // operator (`!/* c */⏎x`) takes the soft `line`, so the operand pulls up to
        // `!(/* c */ x)` while it fits and drops below the comment once it doesn't. The
        // gluing variant (`build_rhs_comments_glued_opt`) spends that `line` on an
        // unconditional space and can only ever reach the first of those two forms.
        // The opening-delimiter rule at the comment-holder `(`: a `//` the author glued to
        // it keeps that line ([`Printer::split_open_delimiter_glued_run`]), as at `return (`
        // / `throw (`, a retained type paren shell, every bracket and every statement
        // header. This shell was the value family's last holdout — prettier un-glues here
        // and tsv followed, which is the one-position-agreeing shape that froze the rule
        // everywhere else it was found (`docs/comments.md` §The delimiter-line question).
        //
        // The gap opens at the OPERATOR's end because the parser strips the source `(` — so
        // the region spans it, and "no newline before the comment" is exactly "the author
        // wrote it on the line this shell's re-added `(` will sit on".
        let (paren_glued, leading_run_start) =
            self.split_open_delimiter_glued_run(operator_end, argument_start);
        let leading_comments_opt = self.build_rhs_comments_opt(leading_run_start, argument_start);
        // Whether a leading comment is *present* — the gate for re-adding the parens — as
        // opposed to whether this emitter has to print it. A **forward-binding** comment (a
        // bundler annotation, a JSDoc cast) is `owned_by_node`, so the operand's own doc
        // prints it and `leading_comments_opt` is `None` — but the parens are still wanted,
        // for the same reason an ordinary gap comment wants them: bare,
        // `!/* @__PURE__ */ f()` reads as annotating the *operator* rather than the operand.
        // Counting the owned comment here is what keeps the wrap, and it is the ONLY thing
        // that does — `needs_parens` deliberately doesn't (it would double-wrap).
        //
        // This is the `has_comments_on_page_in_range` / `has_comments_to_emit_in_range` split: an
        // emit-decision skips owned comments, a layout/semantic gate counts them.
        //
        // An operand that already prints its own value-position pair is the exception:
        // that pair encloses the owned comment on the plain needs-parens path below
        // (`!(/** @type {A} */ (x), y)`, `!(/* c */ x = y)`, `!(/* c */ cond ? b : c)`),
        // so counting the owned comment here would drive the comment-holder wrap and
        // double the parens (`!((/* c */ x = y))`). Skipping it routes the operand to that
        // plain path, where its own single pair encloses the owned comment (prepended by
        // `build_expression_doc`). That's any `needs_parens` operand — assignment, a
        // conditional, an arrow, await/yield, a type assertion — plus a sequence, whose
        // own printer supplies the pair (`needs_parens` reports false for it). Binary is
        // deliberately NOT here: its plain path renders through
        // `build_binary_chain_doc_ungrouped`, which does not prepend the owned comment, so
        // binary must take the comment-holder path (where `needs_paren_wrap` excludes it —
        // one pair either way). A *trailing* comment on such an operand still keeps both
        // pairs (the comment-holder path via `has_trailing_comments`) — that is the
        // deliberate `!((x = y) /* c */)` form, pinned by `operand_paren_comment`.
        // Asked three times below (the exception here, the comment-holder inner wrap,
        // and the plain path), always with the same operand + context — compute once.
        let arg_needs_parens = self.needs_parens(
            unary.argument,
            ParenContext::UnaryArgument {
                parent_op: unary.operator,
            },
        );
        let operand_encloses_owned_comment =
            matches!(unary.argument, Expression::SequenceExpression(_))
                || (arg_needs_parens && !matches!(unary.argument, Expression::BinaryExpression(_)));
        // Anchored at `leading_run_start`, not `operator_end`: the glued `//` above already
        // left the run, and counting it here would make the name a lie (it is never owned —
        // `owned ⇒ is_block`). Inert for the wrap, which ORs the two, but this flag is the
        // reason the wrap survives a run the emitter prints nothing for, so it has to mean
        // what it says.
        let owned_leading_comment = leading_comments_opt.is_none()
            && !operand_encloses_owned_comment
            && tsv_lang::has_comments_on_page_in_range(
                self.comments,
                leading_run_start,
                argument_start,
            );
        let has_leading_comments =
            paren_glued.is_some() || leading_comments_opt.is_some() || owned_leading_comment;

        // Check for trailing comments after the argument but inside the original parens.
        // When the parser strips grouping parens from `!(x /* c */)`, the comment
        // between argument end and unary span end is lost if we don't re-add parens.
        let has_trailing_comments = self.has_comments_to_emit_between(argument_end, unary.span.end);

        let argument_doc = if has_leading_comments || has_trailing_comments {
            // Comments inside grouping parens — must wrap in parens to preserve them.
            let inner = self.build_expression_doc(unary.argument);
            // The outer comment-holder parens already group the operand, so the inner
            // needs_parens layer is redundant for a binary/logical operand — prettier
            // strips it (`!(x + y /* c */)`). Assignment/ternary operands keep their
            // parens for clarity in both formatters, so leave those untouched.
            let needs_paren_wrap =
                arg_needs_parens && !matches!(unary.argument, Expression::BinaryExpression(_));
            let inner = if needs_paren_wrap {
                d.parens(inner)
            } else {
                inner
            };
            // Prettier's `UnaryExpression` shell (inline in `print/estree.js`) verbatim
            // (`print/estree.js`: `group(["(", indent([softline, argumentDoc]), softline,
            // ")"])`), so the two questions stay separate: the GROUP decides flat vs
            // broken on width, and the comment runs inside it decide their own
            // separators. Nothing here asks whether a comment "owns its line" — a gate
            // that did (`comment_cannot_glue_to_operator` over the leading gap) answered
            // a width question with a comment answer, and read the FIRST comment of a run
            // the author glued onto one line as owning it, which it does not
            // (`docs/comments.md` §Own-line-ness is a SOURCE question).
            //
            // Everything that genuinely must break still does, through the run's own
            // hardline and `DocArena::will_break`: an own-line comment, a `//`, a
            // multiline block. Those reach the group as a forced break instead of as a
            // pre-empted layout, which is the whole difference.
            let mut parts: DocBuf = smallvec![d.softline()];
            if let Some(leading) = leading_comments_opt {
                parts.push(leading);
            }
            parts.push(inner);
            // The trailing run, one separator BEFORE each comment — the shared
            // anchored-run emitter, so this gap answers the glue question the way every
            // other trailing run does. A fixed `argument_end` anchor re-asked per comment
            // read the *second* half of a pair the author glued (`x⏎/* c1 */ /* c2 */`)
            // as own-line and split it, and asking the comment's KIND instead welded an
            // own-line `//` onto whatever preceded it (`docs/comments.md` §Trailing and
            // dangling runs).
            self.push_anchored_trailing_run(
                &mut parts,
                argument_end,
                unary.span.end,
                RunLeadingBlank::Keep,
            );
            // The one break the group cannot see for itself. A trailing **line** comment
            // is deferred through a `line_suffix` (`build_trailing_line_comment_doc`), so
            // `will_break` reports nothing and a flat shell would carry the `//` out past
            // its own `)` and `;` — the deferred run escaping the construct it was written
            // in, which is content loss rather than a layout choice
            // (`docs/comments.md` §Trailing and dangling runs). Every other break-forcing
            // comment reaches the group as a real hardline: an own-line trailing comment
            // through `push_trailing_run_separator`, a `//` or own-line block in the
            // LEADING run through `push_leading_comment_run`'s third separator.
            let mut shell_parts: DocBuf = smallvec![d.text("(")];
            // The glued `//` rides on the `(`'s own line, ahead of the `indent` — no break
            // precedes it, so the indent would have nothing to act on anyway.
            shell_parts.extend(paren_glued);
            shell_parts.push(d.indent(d.concat(&parts)));
            shell_parts.push(d.softline());
            shell_parts.push(d.text(")"));
            let shell = d.concat(&shell_parts);
            // The second break the group cannot see for itself. A `//` in the leading run
            // reaches it as a real `hardline` through `push_leading_comment_run` — but the
            // glued one has LEFT that run, so nothing inside the group forces it open any
            // more, and a flat shell would put `x)` on the `//`'s own line and swallow it.
            // Claiming the delimiter's line is therefore a break obligation, not just a
            // placement: the shell rendered the same way before, the comment merely sat one
            // line lower.
            if paren_glued.is_some() || self.has_line_comments_between(argument_end, unary.span.end)
            {
                d.group_break(shell)
            } else {
                d.group(shell)
            }
        } else if arg_needs_parens {
            // Binary expressions need parens - grouping lets the parens expand when the arg is long
            if let Expression::BinaryExpression(binary) = unary.argument {
                // Wrap any binaryish arg (logical or not) in a single paren group.
                // Matches Prettier's `parent.type === "UnaryExpression"` path
                // (binaryish.js:88-91): `group([indent([softline, ...parts]), softline])`.
                // The chain's shouldGroup is computed normally: 2-operand chains
                // get a sub-group (can stay flat at inner indent when paren group
                // breaks), 3+ chained operands break together with the paren group.
                let inner = self.build_binary_chain_doc_ungrouped(binary);
                d.group(d.concat(&[
                    d.text("("),
                    d.indent_softline(inner),
                    d.softline(),
                    d.text(")"),
                ]))
            } else {
                // Non-binary that needs parens (e.g., ternary or assignment in unary/assertion)
                d.concat(&[
                    d.text("("),
                    self.build_expression_doc(unary.argument),
                    d.text(")"),
                ])
            }
        } else {
            self.build_expression_doc(unary.argument)
        };

        // Keyword operators need a space before the operand
        if unary.operator.is_keyword_operator() {
            d.concat(&[d.text(unary.operator.as_str()), d.text(" "), argument_doc])
        } else {
            d.concat(&[d.text(unary.operator.as_str()), argument_doc])
        }
    }

    /// Build a doc for a chain of binary operators with line wrapping support
    ///
    /// The default binary layout — what a `BinaryExpression` gets from the generic
    /// `build_expression_doc` dispatch, in every context (standalone and embedded
    /// alike; the one root-position exception is `build_root_expression_doc`).
    /// Contexts wanting a different style key it at the call site, mirroring
    /// prettier's parent-keyed shouldNotIndent chain (binaryish.js:97): call args
    /// and array elements take `build_binary_chain_doc_indented`, return/throw and
    /// conditions the ungrouped variants, nested binary operands and a type
    /// assertion's operand the continuation style.
    ///
    /// ⚠️ This default is the INVERSE of prettier's. There, the indent bucket is the
    /// *fall-through* — a parent indents unless it is named in `shouldNotIndent` or
    /// `shouldIndentIfInlining` — so a position nobody thought about lands on indent.
    /// Here it lands on flat, which is the wrong answer for every parent outside those
    /// two lists. A new binary-holding position therefore has to opt in explicitly;
    /// check the parent against binaryish.js:96-115 rather than inheriting this default
    /// by omission (the cast operand was flat for exactly that reason).
    ///
    /// When the chain exceeds print width, breaks after operators:
    /// ```text
    /// a +
    /// b +
    /// c
    /// ```
    ///
    /// Implements prettier's "add parens for clarity" behavior where mixing certain
    /// operators requires parentheses for readability (`a && b || c` →
    /// `(a && b) || c`, chained equality, right-side same-precedence parens).
    /// Flattens same-precedence operators that can be chained (e.g., a + b + c)
    /// but preserves parentheses where needed for clarity (e.g., a * b / c).
    ///
    /// Note: The binary expression doc itself does NOT add indent for continuations.
    /// Indentation comes from the parent context (e.g., assignment adds indent,
    /// function call args add indent, etc.).
    ///
    /// Handles comments between operands (Prettier 3.7 #17723):
    /// - Line comments force a line break
    /// - Block comments are printed inline
    pub(in crate::printer) fn build_binary_chain_doc(
        &self,
        binary: &internal::BinaryExpression<'_>,
    ) -> DocId {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::Grouped)
    }

    /// Build a binary chain doc WITHOUT the outer group wrapper
    ///
    /// Use in contexts where the parent group should control breaking
    /// (e.g., !!(), new expression callee, return/throw).
    /// The line() elements will break with the parent group, but shouldGroup
    /// is computed normally — 2-operand chains get a sub-group.
    pub(in crate::printer) fn build_binary_chain_doc_ungrouped(
        &self,
        binary: &internal::BinaryExpression<'_>,
    ) -> DocId {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::Ungrouped)
    }

    /// Build a binary chain doc for condition parentheses (if/while/for/do-while/switch)
    ///
    /// Like ungrouped, but also suppresses shouldGroup for logical operators so that
    /// logical chain breaks are controlled by the parent condition group.
    /// Matches Prettier's `isInsideParenthesis` behavior (binaryish.js:331).
    pub(in crate::printer) fn build_binary_chain_doc_ungrouped_condition(
        &self,
        binary: &internal::BinaryExpression<'_>,
    ) -> DocId {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::UngroupedCondition)
    }

    /// Build a binary chain doc with continuation indent
    ///
    /// When the chain breaks, continuation lines are indented relative to the first:
    /// ```text
    /// first &&
    ///   second &&
    ///   third
    /// ```
    ///
    /// The style for the positions where prettier's shouldNotIndent chain yields
    /// false: a nested binary operand (parenthesized or not,
    /// `build_binary_operand_doc`), a type assertion's operand
    /// (`build_continuation_indent_expression_doc` — `(a ??\n\tb) as T`), an Embedded expression root
    /// (`build_root_expression_doc`), and a ternary test under
    /// return/throw/call/new.
    pub(in crate::printer) fn build_binary_chain_doc_with_continuation_indent(
        &self,
        binary: &internal::BinaryExpression<'_>,
    ) -> DocId {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::ContinuationIndent)
    }

    /// Build binary chain with continuation indent WITHOUT the outer group wrapper
    ///
    /// Use this when the caller controls grouping: the one caller is
    /// `build_binary_chain_parts_indented`'s comment-aware twin hand-off
    /// (`prepare_binary_chain_layout`, inline-paren contexts).
    pub(in crate::printer) fn build_binary_chain_parts_with_continuation_indent(
        &self,
        binary: &internal::BinaryExpression<'_>,
    ) -> DocId {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::ContinuationIndentUngrouped)
    }

    /// Core implementation for binary chain doc building
    ///
    /// One body for every `BinaryChainStyle` (the variants document themselves):
    /// the flat styles route through `build_binary_chain_flat`, the
    /// continuation-indent pair through
    /// `build_binary_chain_continuation_indent{,_parts}`.
    fn build_binary_chain_doc_core(
        &self,
        binary: &internal::BinaryExpression<'_>,
        style: BinaryChainStyle,
    ) -> DocId {
        // Collect all operands (with spans) and operators in the chain
        let mut operands: OperandBuf = OperandBuf::new();
        let mut operators: OperatorBuf = OperatorBuf::new();
        self.collect_binary_chain_with_spans(binary, &mut operands, &mut operators);

        if operands.len() <= 1 {
            // Single operand, shouldn't happen but handle gracefully
            return self.build_expression_doc(binary.left);
        }

        // Compute shouldGroup from the original binary expression.
        // This matches Prettier's shouldGroup in printBinaryishExpressions:
        // the continuation gets its own group only when both operand types
        // differ from the current node type (BinaryExpression vs LogicalExpression).
        //
        // In UngroupedCondition mode (if/while/for/do-while/switch conditions),
        // logical operators (&&, ||, ??) must NOT get a sub-group — the parent
        // condition group controls their breaking. This matches Prettier's
        // `isInsideParenthesis` suppression (binaryish.js:331).
        // Without this, `while (a < b && c === d)` keeps the chain flat when
        // the condition group breaks, because the sub-group evaluates fit
        // independently.
        //
        // In plain Ungrouped mode (!!(), new, return/throw), shouldGroup is
        // computed normally — 2-operand chains get a sub-group so they can
        // stay flat when the parent's paren group breaks.
        let should_group = if matches!(style, BinaryChainStyle::UngroupedCondition)
            && binary.operator.is_logical()
        {
            false
        } else {
            Self::should_group_binary_continuation(binary)
        };

        // shouldInlineLogicalExpression: when the outermost logical has a non-empty
        // object/array on the right, keep operator and RHS on the same line.
        // Prettier ref: binaryish.js:275, 361
        let should_inline_last = super::assignment::should_inline_logical_expression(binary);

        // For the continuation-indent styles, we separate first operand from the rest
        // For other styles, we build a flat parts list
        let chain = match style {
            BinaryChainStyle::ContinuationIndent => self.build_binary_chain_continuation_indent(
                &operands,
                &operators,
                should_inline_last,
                should_group,
            ),
            BinaryChainStyle::ContinuationIndentUngrouped => self
                .build_binary_chain_continuation_indent_parts(
                    &operands,
                    &operators,
                    should_inline_last,
                    should_group,
                ),
            _ => self.build_binary_chain_flat(
                &operands,
                &operators,
                style,
                should_group,
                should_inline_last,
            ),
        };
        self.wrap_chain_with_paren_comments(binary, &operands, chain)
    }

    /// Wrap a binary chain doc with comments from stripped grouping parens.
    ///
    /// When the parser strips parens like `(/* l */ a + b /* t */)`, the
    /// comments are orphaned in the gaps between `binary.span` and the outer
    /// operand spans. Without this, those comments are silently dropped — a
    /// SAFETY violation.
    ///
    /// Leading comments (between `binary.span.start` and the leftmost operand)
    /// are prepended via `prepend_removed_paren_comments`. Trailing comments
    /// (between the rightmost operand and `binary.span.end`) emit inline for
    /// same-line blocks (` /* t */`) and via `line_suffix` for line/own-line
    /// comments (so they defer past any enclosing semicolon).
    fn wrap_chain_with_paren_comments(
        &self,
        binary: &internal::BinaryExpression<'_>,
        operands: &[ChainOperand],
        chain: DocId,
    ) -> DocId {
        let Some(leftmost_start) = operands.first().map(|o| o.span.start) else {
            return chain;
        };
        let Some(rightmost_end) = operands.last().map(|o| o.span.end) else {
            return chain;
        };

        let with_leading =
            self.prepend_removed_paren_comments(binary.span.start, leftmost_start, chain);

        if rightmost_end >= binary.span.end {
            return with_leading;
        }
        let mut parts = smallvec![with_leading];
        self.append_trailing_paren_comments(&mut parts, rightmost_end, binary.span.end);
        // `concat` short-circuits the no-trailing-comment case (`[with_leading]`).
        self.d().concat(&parts)
    }

    /// Check if the binary continuation should be wrapped in its own group.
    ///
    /// Matches Prettier's `shouldGroup` in `printBinaryishExpressions`:
    /// - Returns true when both left and right operands are a different AST type
    ///   category than the current node (BinaryExpression vs LogicalExpression).
    /// - In ESTree, `+`, `*`, etc. are BinaryExpression while `&&`, `||`, `??`
    ///   are LogicalExpression. We use `is_logical()` to distinguish these categories.
    ///
    /// When shouldGroup is true, the continuation gets its own group, allowing it
    /// to independently evaluate whether it fits on the current line when the outer
    /// group breaks (e.g., due to a multi-line parenthesized left operand).
    pub(in crate::printer) fn should_group_binary_continuation(
        binary: &internal::BinaryExpression<'_>,
    ) -> bool {
        let current_is_logical = binary.operator.is_logical();

        // Check if left operand is same AST type category
        let left_is_same_category = matches!(
            binary.left,
            Expression::BinaryExpression(inner) if inner.operator.is_logical() == current_is_logical
        );

        // Check if right operand is same AST type category
        let right_is_same_category = matches!(
            binary.right,
            Expression::BinaryExpression(inner) if inner.operator.is_logical() == current_is_logical
        );

        // shouldGroup when NEITHER operand is the same category
        !left_is_same_category && !right_is_same_category
    }

    /// Common logic for building binary chain (shared by flat and continuation indent styles)
    ///
    /// Returns (head_parts, continuation_parts) where head includes first operand + operator.
    ///
    /// When `should_inline_last` is true (shouldInlineLogicalExpression), the last operand
    /// uses a space instead of `line()`, keeping operator and RHS on the same line so the
    /// object/array can self-expand. Prettier ref: binaryish.js:275, 361
    fn build_binary_chain_parts(
        &self,
        operands: &[ChainOperand],
        operators: &[BinaryOperator],
        should_inline_last: bool,
    ) -> (DocBuf, DocBuf) {
        if operands.is_empty() || operands.len() == 1 {
            // Edge cases handled by callers
            return (DocBuf::new(), DocBuf::new());
        }

        // Whole-chain comment presence gate (idiom 8): one on-page lookup over the chain's
        // whole span lets the per-gap emitters below skip their per-gap comment scans for the
        // ~all chains that hold no comment anywhere. A *presence* flag (on-page counts owned),
        // so it fails open — it can only add work on a commented chain, never suppress a
        // comment (the perf80 hazard). Every operand→operator / operator→operand gap the
        // emitters scan lies within `[first operand start, last operand end]`.
        let chain_has_comments = self.has_comments_on_page_between(
            operands[0].span.start,
            operands[operands.len() - 1].span.end,
        );

        // First operand + first operator (stays at base indent)
        let mut head_parts: DocBuf = smallvec![operands[0].doc];

        let first_op_str = operators[0].as_str();

        let first_op_pos =
            self.find_operator_position(operands[0].span.end, operands[1].span.start, first_op_str);

        // operand[0] → first operator. A line comment in this gap would swallow the
        // operator if emitted inline; the helper keeps it trailing the operand and
        // reports whether it forced the operator onto the next line.
        let mut prev_forced_break = self.push_operand_operator_gap(
            &mut head_parts,
            operands[0].span.end,
            first_op_pos.start,
            first_op_str,
            chain_has_comments,
        );

        // Build continuation parts
        let mut continuation_parts: DocBuf = DocBuf::new();

        // The operand[i-1]→operand[i] operator gap is located once and carried across
        // iterations: iteration i's leading gap is either the first gap (i == 1) or the
        // trailing gap the previous iteration already scanned, so it is never re-scanned.
        let mut op_pos = first_op_pos;

        for i in 1..operands.len() {
            let operand = &operands[i];

            // shouldInlineLogicalExpression: the last operand (non-empty object/array)
            // uses a space instead of line(), keeping operator and RHS on the same line.
            let allow_breaks = !(i == operands.len() - 1 && should_inline_last);

            // When the previous operand→operator gap forced a break, the operator now
            // leads this operand on the same line, so hug it with a space (not a line).
            self.append_post_operator_parts(
                &mut continuation_parts,
                op_pos.end,
                operand,
                allow_breaks,
                prev_forced_break,
                chain_has_comments,
            );

            // operand[i] → next operator (if not last operand)
            if i < operands.len() - 1 {
                let next_op_str = operators[i].as_str();
                let next_op_pos = self.find_operator_position(
                    operand.span.end,
                    operands[i + 1].span.start,
                    next_op_str,
                );

                prev_forced_break = self.push_operand_operator_gap(
                    &mut continuation_parts,
                    operand.span.end,
                    next_op_pos.start,
                    next_op_str,
                    chain_has_comments,
                );

                // Carry this trailing gap forward as the next iteration's leading gap.
                op_pos = next_op_pos;
            }
        }

        (head_parts, continuation_parts)
    }

    /// Build a flat binary chain (Grouped or Ungrouped style)
    ///
    /// Matches Prettier's binaryish.js structure:
    /// - First operand + first operator at base indent (head)
    /// - Continuation (line + remaining operands) optionally in a sub-group
    ///
    /// When `should_group` is true (operand types differ from current node,
    /// e.g., `(LogicalExpr) + d`), the continuation gets its own group so it
    /// can independently evaluate fit when the outer group breaks due to a
    /// multi-line left operand. When false (same category, e.g., `(a+b)*c`),
    /// continuation breaks with the outer group.
    fn build_binary_chain_flat(
        &self,
        operands: &[ChainOperand],
        operators: &[BinaryOperator],
        style: BinaryChainStyle,
        should_group: bool,
        should_inline_last: bool,
    ) -> DocId {
        let d = self.d();
        if operands.is_empty() {
            return d.empty();
        }

        if operands.len() == 1 {
            return operands[0].doc;
        }

        let (mut head_parts, continuation_parts) =
            self.build_binary_chain_parts(operands, operators, should_inline_last);

        if !continuation_parts.is_empty() {
            if should_group {
                // Sub-group: continuation evaluates fit independently
                head_parts.push(d.group(d.concat(&continuation_parts)));
            } else {
                // No sub-group: continuation breaks with outer group
                head_parts.extend(continuation_parts);
            }
        }

        match style {
            BinaryChainStyle::Grouped => d.group(d.concat(&head_parts)),
            _ => d.concat(&head_parts),
        }
    }

    /// Build a binary chain with continuation indent
    ///
    /// When flat: "first && second && third"
    /// When broken:
    /// "first &&
    ///   second &&
    ///   third"
    fn build_binary_chain_continuation_indent(
        &self,
        operands: &[ChainOperand],
        operators: &[BinaryOperator],
        should_inline_last: bool,
        should_group: bool,
    ) -> DocId {
        let d = self.d();
        d.group(self.build_binary_chain_continuation_indent_parts(
            operands,
            operators,
            should_inline_last,
            should_group,
        ))
    }

    /// Build binary chain continuation indent parts WITHOUT group wrapper.
    ///
    /// Returns the concat of first_parts + indent(continuation_parts) without
    /// wrapping in a group. Used in Svelte template expressions and when the
    /// caller controls grouping.
    ///
    /// When `should_group` is true, wraps the continuation in a sub-group so it
    /// can independently evaluate fit (bypassing the renderer's `will_break` check
    /// on the outer group).
    fn build_binary_chain_continuation_indent_parts(
        &self,
        operands: &[ChainOperand],
        operators: &[BinaryOperator],
        should_inline_last: bool,
        should_group: bool,
    ) -> DocId {
        let d = self.d();
        let (first_parts, continuation_parts) =
            self.build_binary_chain_parts(operands, operators, should_inline_last);

        // When should_group is true, wrap the continuation in its own group so it
        // can independently evaluate fit. Without this, the renderer's will_break()
        // check on the outer group sees hardlines in the left operand (e.g., a
        // multi-line call expression) and forces the entire group to Break mode,
        // even when the continuation (e.g., `?? 'text'`) fits on the closing line.
        //
        // When should_inline_last is true, skip indent entirely — matching prettier's
        // early return of group(parts) with no indent wrapper (binaryish.js:131-134).
        // The inlined last operand (object/array) handles its own indentation.
        let continuation_doc = if should_inline_last {
            d.concat(&continuation_parts)
        } else {
            d.indent(d.concat(&continuation_parts))
        };
        let continuation_doc = if should_group {
            d.group(continuation_doc)
        } else {
            continuation_doc
        };

        d.concat(&[d.concat(&first_parts), continuation_doc])
    }

    /// Emit a binary chain's operand→operator gap, returning whether a line comment
    /// in the gap forced the operator onto the next line.
    ///
    /// Without a line comment the gap renders inline as it always has
    /// (`operand <inline block comments> operator`). With a line comment, emitting it
    /// inline would run to end-of-line and **swallow the operator**
    /// (`1 // c⏎+ 2` → `1 // c + 2`, the `+ 2` absorbed into the comment — content
    /// loss). Instead the comment is kept trailing the operand where the author wrote
    /// it — the first, on the operand's own line, via `line_suffix` (zero width); any
    /// later ones on their own line — and a hardline then forces the operator down to
    /// hug its right operand (`1 // c⏎+ 2`). Returns `true` in that case so the caller
    /// hugs the following operand with a space rather than a breakable line (avoiding
    /// the `1 // c⏎+⏎2` over-break). Prettier instead relocates the comment past the
    /// operator; see conformance_prettier_ts_comments.md §Comment relocation.
    fn push_operand_operator_gap(
        &self,
        parts: &mut DocBuf,
        operand_end: u32,
        op_start: u32,
        op_str: &'static str,
        chain_has_comments: bool,
    ) -> bool {
        let d = self.d();

        // Zero-comment fast path: the operand→operator gap holds no comment (the
        // ubiquitous case), so emit just the operator — no empty comment node in the
        // parts concat, and no per-gap comment scan at all. Byte-identical: the gap is
        // comment-free, so the general path below would build `empty()` here (renders to
        // nothing). The gap ⊆ the binary span, so this can only skip work, never a comment.
        // The whole-chain gate short-circuits the per-gap scan when the chain is
        // comment-free (`chain_has_comments` false ⇒ this gap holds none to emit either).
        if !chain_has_comments || !self.has_comments_to_emit_between(operand_end, op_start) {
            parts.push(d.text(" "));
            parts.push(d.text(op_str));
            return false;
        }

        if !self.has_line_comments_between(operand_end, op_start) {
            // No line comment — inline gap (block comments stay inline, as before).
            parts.push(self.build_inline_comments_between_doc(operand_end, op_start));
            parts.push(d.text(" "));
            parts.push(d.text(op_str));
            return false;
        }

        // Keep each comment where the author wrote it, then break before the operator —
        // the shared anchored-run emitter ([`Printer::push_anchored_trailing_run`]).
        self.push_anchored_trailing_run(parts, operand_end, op_start, RunLeadingBlank::Keep);

        parts.push(d.hardline());
        parts.push(d.text(op_str));
        true
    }

    /// Append post-operator parts (comments and line breaks) to a parts vector
    ///
    /// Handles line comments vs block comments appropriately.
    /// When `allow_breaks` is true, uses `line()` (space when flat, newline when broken).
    /// When `lead_with_space` is true, the leading separator is a hard space instead of a
    /// breakable line — used when the previous operand→operator gap forced a break, so the
    /// operator now leads this operand on the same line (`1 // c⏎+ 2`, not `1 // c⏎+⏎2`).
    ///
    /// Handles multiple consecutive comments by preserving their line structure:
    /// - `a && // comment1\n// comment2\nb` keeps each comment on its own line
    fn append_post_operator_parts(
        &self,
        parts: &mut DocBuf,
        op_end: u32,
        operand: &ChainOperand,
        allow_breaks: bool,
        lead_with_space: bool,
        chain_has_comments: bool,
    ) {
        let d = self.d();
        // Zero-comment fast path, the same shape as the operand→operator gap's: the
        // whole-chain gate short-circuits the per-gap scan for the ~all chains with no
        // comment (`chain_has_comments` false ⇒ this gap, ⊆ the chain span, holds none to
        // emit). A *presence* flag, so a false gate proves this gap emits nothing.
        if !chain_has_comments || !self.has_comments_to_emit_between(op_end, operand.span.start) {
            // No comments - simple case
            if allow_breaks && !lead_with_space {
                parts.push(d.line());
            } else {
                parts.push(d.text(" "));
            }
            parts.push(operand.doc);
            return;
        }

        // Whether the run forces the operand onto its own (broken) line — a **line**
        // comment, or a block the author gave a line of its own. Anything else is
        // inline-leading and lets the chain's group decide on width.
        let forces_own_line = self.comment_hangs_binary_operand(op_end, operand.span.start);

        if !forces_own_line {
            // Only inline-leading block comments - place as leading on RHS operand.
            // In flat mode: `a || /* comment */ b` (space from line(), comment+trailing space, operand)
            // In break mode: `a ||\n<indent>/* comment */ b` (comment leads continuation line)
            let comments_doc =
                self.build_comments_between(op_end, operand.span.start, CommentSpacing::Trailing);
            if allow_breaks && !lead_with_space {
                parts.push(d.line());
            } else {
                parts.push(d.text(" "));
            }
            parts.push(comments_doc);
            parts.push(operand.doc);
            return;
        }

        // An own-line (or line) comment forces the chain to break. Each comment keeps
        // its line; authored blank lines are preserved. A trailing comment glued to the
        // operator (no newline after it) stays inline-leading it. The shared
        // anchored-run emitter ([`Printer::push_anchored_trailing_run`]), which hands
        // back the cursor the operand separator below reads.
        let pos = self.push_anchored_trailing_run(
            parts,
            op_end,
            operand.span.start,
            RunLeadingBlank::Drop,
        );

        // Operand: on its own line when the last comment has a newline after it
        // (preserving an author blank line), else glued inline (`/* c */ operand`).
        // Comment-adjacency read (real even in canonical mode): a line comment always
        // has a source newline before the operand, and gluing the operand after its
        // `line_suffix` would swallow it at flush (inside `${…}` this even makes the
        // output unparseable).
        if self.comment_has_newline_between(pos, operand.span.start) {
            // ⚠️ The blank scan takes the in-source ceiling: the operand is an EXPRESSION
            // start, so a block comment the author glued to it is OWNED — physically in
            // this gap, printed by the operand's own doc, and skipped by the run above.
            // Scanning across it reads its interior newlines as an author blank, which
            // this gap already keeps for real (`RunLeadingBlank::Keep`, the sanctioned
            // `expressions/binary/operator_trailing_comment_blank`) — so the fabrication
            // wears the sanction's own output and nothing tells them apart.
            self.push_blank_preserving_hardline(
                parts,
                pos,
                self.blank_scan_end(pos, operand.span.start),
            );
        } else {
            parts.push(d.text(" "));
        }
        parts.push(operand.doc);
    }

    /// Find operator position between two operands in source
    ///
    /// Returns the start and end positions of the operator string in the source,
    /// which is used to correctly split comments before/after the operator.
    /// Skips over comments to avoid matching operators inside them.
    fn find_operator_position(
        &self,
        prev_span_end: u32,
        next_span_start: u32,
        op_str: &str,
    ) -> OperatorPosition {
        let range_start = prev_span_end as usize;
        let range_end = next_span_start as usize;
        let bytes = self.source.as_bytes();
        let op_bytes = op_str.as_bytes();
        let op_len = op_bytes.len();
        let mut i = range_start;

        while i + op_len <= range_end {
            // Skip comments
            if let Some(new_i) = tsv_lang::source_scan::skip_comment(bytes, i, range_end) {
                i = new_i;
                continue;
            }
            // Check for operator match
            if &bytes[i..i + op_len] == op_bytes {
                return OperatorPosition {
                    start: i as u32,
                    end: (i + op_len) as u32,
                };
            }
            i += 1;
        }
        // Fallback (shouldn't happen in valid code)
        OperatorPosition {
            start: prev_span_end,
            end: prev_span_end + op_str.len() as u32,
        }
    }

    /// Collect all operands (with spans) and operators from a chain of binary expressions
    ///
    /// Uses `should_flatten()` to determine which operators can be chained together.
    /// Flattens both left and right sides when operators are compatible (e.g., `&&`, `||`).
    fn collect_binary_chain_with_spans(
        &self,
        expr: &internal::BinaryExpression<'_>,
        operands: &mut OperandBuf,
        operators: &mut OperatorBuf,
    ) {
        // Recursively flatten left side if it can be chained with current operator
        if let Expression::BinaryExpression(left_binary) = expr.left {
            if expr.operator.can_flatten_with(left_binary.operator) {
                self.collect_binary_chain_with_spans(left_binary, operands, operators);
            } else {
                operands.push(ChainOperand {
                    doc: self.build_binary_operand_doc(expr.left, expr.operator, false),
                    span: expr.left.span(),
                });
            }
        } else {
            operands.push(ChainOperand {
                doc: self.build_binary_operand_doc(expr.left, expr.operator, false),
                span: expr.left.span(),
            });
        }

        // Add current operator
        operators.push(expr.operator);

        // Also flatten right side for truly associative operators (removes redundant parens)
        // e.g., `a && (b && c)` becomes `a && b && c`
        // Only logical operators are truly associative; arithmetic preserves right-side parens
        if let Expression::BinaryExpression(right_binary) = expr.right
            && expr.operator.can_flatten_with(right_binary.operator)
            && expr.operator.is_logical()
            && right_binary.operator.is_logical()
        {
            self.collect_binary_chain_with_spans(right_binary, operands, operators);
            return;
        }

        // Right operand can't be flattened - add as-is
        operands.push(ChainOperand {
            doc: self.build_binary_operand_doc(expr.right, expr.operator, true),
            span: expr.right.span(),
        });
    }

    /// Build operand with parens if needed for clarity
    pub(in crate::printer) fn build_binary_operand_doc(
        &self,
        operand: &Expression<'_>,
        parent_op: BinaryOperator,
        is_right: bool,
    ) -> DocId {
        let d = self.d();
        let ctx = if is_right {
            ParenContext::BinaryRight { parent_op }
        } else {
            ParenContext::BinaryLeft { parent_op }
        };

        // A nested binary sub-expression uses continuation indent, parenthesized or
        // not: prettier's shouldNotIndent (binaryish.js:96-115) evaluates to false
        // when the parent is BinaryExpression (none of the conditions match), so the
        // inner chain gets indent(rest) — when it breaks, its continuation lines are
        // indented (`(first &&\n\t\tsecond)` not `(first &&\n\tsecond)`; likewise
        // `0.5 * a(...) * b(...)` inside `... + 1.0` indents the `*` continuation
        // lines relative to `0.5`).
        //
        // One shape for every context. A `group(parens(parts))` twin — parens inside
        // the group so the fit reserved `)` — used to be the Standalone parenthesized
        // arm; the renderer's lookahead fits (`rest_commands`) measures the `)`
        // either way now, and the two shapes are output-identical over the full
        // fixture corpus, so the mode split retired with the depth-blind
        // `is_embedded()` keying.
        let operand_doc = if let Expression::BinaryExpression(inner_binary) = operand {
            self.build_binary_chain_doc_with_continuation_indent(inner_binary)
        } else {
            self.build_chain_aware_operand_doc(operand)
        };
        if self.needs_parens(operand, ctx) {
            d.parens(operand_doc)
        } else {
            operand_doc
        }
    }

    /// Build a binary operand's doc, routing a curried arrow-chain operand
    /// (`cond ?? ((a) => (b) => …)`) through the progressive call-arg/binaryish
    /// chain layout. Mirrors prettier's `isBinaryish(parent)` reaching
    /// `printArrowFunctionSignatures`. Every curried operand is routed; a head triggering
    /// `arrow_chain_should_break` gets the break INSIDE that layout rather than being turned
    /// away from it. `should_use_arrow_chain_layout` still gates on every comment sitting in
    /// a region the chain doc emits, so an operand whose comment lands in a gap that layout
    /// has no emitter for falls through to the default path.
    fn build_chain_aware_operand_doc(&self, operand: &Expression<'_>) -> DocId {
        if crate::printer::is_curried_arrow_chain(operand) {
            self.build_with_arrow_chain_context(
                crate::printer::ArrowChainContext::CallArgOrBinaryish,
                || self.build_expression_doc(operand),
            )
        } else {
            self.build_expression_doc(operand)
        }
    }

    /// Build a Doc for an await expression
    pub(in crate::printer) fn build_await_doc(
        &self,
        await_expr: &internal::AwaitExpression<'_>,
    ) -> DocId {
        let d = self.d();

        // Preserve comments from stripped grouping parens: `await (/** @type {T} */ expr)`
        let keyword_end = await_expr.span.start + "await".len() as u32;
        // `await` argument — an `ancestorNameMap` value position.
        self.mark_ternary_extra_indent(await_expr.argument);
        let argument_start = await_expr.argument.span().start;
        let argument_end = await_expr.argument.span().end;
        // Trailing comments from stripped grouping parens: `await (x /* c */)` → `await x /* c */`
        let has_trailing_comments =
            self.has_comments_to_emit_between(argument_end, await_expr.span.end);

        // A block run the author broke AFTER, before an argument that WILL BREAK,
        // keeps its break — prettier's `printLeadingComment` newline-after `line`,
        // materialized by the argument's own break (`await /* c */⏎fn({…})` stays
        // broken, the argument opening un-indented on the next line; `await` is not a
        // restricted production, so the break is layout, not ASI — contrast `yield`).
        // The gate is the shared `breaking_value_leading_run`; an argument that FITS
        // declines into the glued pull-up below. Mirrors `build_spread_doc`; a
        // parenthesized argument keeps the glued path (the compound is unprobed).
        if !self.needs_parens(await_expr.argument, ParenContext::AwaitArgument)
            && let Some((run, arg_doc)) =
                self.breaking_value_leading_run(keyword_end, argument_start, || {
                    self.build_expression_doc(await_expr.argument)
                })
        {
            let mut parts: DocBuf = smallvec![d.text("await ")];
            self.push_leading_run_before_breaking_value(&mut parts, &run, argument_start);
            parts.push(arg_doc);
            self.append_trailing_paren_comments(&mut parts, argument_end, await_expr.span.end);
            return d.concat(&parts);
        }

        // The keyword→operand gap, shared with `new`→callee. The run is emitted OUTSIDE
        // any parens the operand needs — the gap belongs to the keyword, not to a pair
        // the printer must emit — so it answers one way whether or not the operand's
        // precedence happens to require one. A comment the author wrote INSIDE those
        // parens is glued to the operand and therefore owned, so it never reaches this
        // axis and keeps its place (the `grouped_operand_comment` divergence).
        let gap = self.keyword_operand_gap(keyword_end, argument_start);

        let argument_doc = if has_trailing_comments {
            // The grouping parens are required when the operand needs them (`await`
            // binds tighter than a binary/ternary operand, so `await x + y` is
            // `(await x) + y`) — and a comment in the operand→`)` gap RETAINS them even
            // where they were redundant, through the shared operand-shell emitter. The
            // comment stays INSIDE them where the author wrote it; prettier relocates it
            // past `)` and, on the next pass, past the `;`. That second pass is why the
            // shell is not optional here: `await`'s own span covers the `)`, so a stripped
            // form hands the comment to the enclosing terminator gap on reparse and the
            // authoring has no fixed point at all. Mirrors `build_spread_doc`.
            let inner = self.build_expression_doc(await_expr.argument);
            if let Some(shell) = self.build_paren_operand_comment_doc(
                argument_end,
                await_expr.span.end,
                inner,
                inner,
                ")",
            ) {
                shell
            } else if self.needs_parens(await_expr.argument, ParenContext::AwaitArgument) {
                d.parens(inner)
            } else {
                inner
            }
        } else if self.needs_parens(await_expr.argument, ParenContext::AwaitArgument) {
            d.concat(&[
                d.text("("),
                self.build_expression_doc(await_expr.argument),
                d.text(")"),
            ])
        } else {
            self.build_expression_doc(await_expr.argument)
        };

        match gap {
            KeywordOperandGap::Continuation => d.concat(&[
                d.text("await"),
                self.build_continuation_indent(keyword_end, argument_start, argument_doc),
            ]),
            KeywordOperandGap::Inline(Some(run)) => {
                d.concat(&[d.text("await "), run, argument_doc])
            }
            KeywordOperandGap::Inline(None) => d.concat(&[d.text("await "), argument_doc]),
        }
    }

    /// Build a Doc for a yield expression
    pub(in crate::printer) fn build_yield_doc(
        &self,
        yield_expr: &internal::YieldExpression<'_>,
    ) -> DocId {
        let d = self.d();
        let keyword = if yield_expr.delegate {
            "yield*"
        } else {
            "yield"
        };
        let Some(arg) = yield_expr.argument else {
            return d.text(keyword);
        };

        // The gap starts at the end of the bare `yield`, never past a computed `*`:
        // `yield*` is one fixed string in the OUTPUT, but in the source the `*` follows
        // whatever the author wrote between the two (`yield/* c */* x`), so measuring the
        // gap as `+ "yield*".len()` lands inside that comment and both scans below start
        // past it — dropping it. A keyword's own bytes can hold no comment, so its end
        // bounds the region exactly; the `*` then sits inside the gap, which is harmless
        // because both readers look for comments (and `(`) rather than slicing text.
        let keyword_end = yield_expr.span.start + "yield".len() as u32;
        // `yield` argument — an `ancestorNameMap` value position.
        self.mark_ternary_extra_indent(arg);
        let argument_start = arg.span().start;
        let argument_end = arg.span().end;

        // Trailing comments from stripped grouping parens: `yield (x /* c */)` → `yield x /* c */`
        let has_trailing_comments =
            self.has_comments_to_emit_between(argument_end, yield_expr.span.end);

        // A comment that forces the break takes the parenthesized form. `yield` is a
        // restricted production (`yield [no LineTerminator here] AssignmentExpression`,
        // ECMA-262 §15.5), so without the parens ASI ends the `yield` at the newline and
        // the operand becomes a separate expression statement — the `yield` silently
        // loses its argument. Same gate and same layout as its `return`/`throw` siblings;
        // see `build_hanging_paren_doc` for the shared rule, and
        // docs/conformance_prettier_ts_comments.md §Comment relocation for why prettier (whose own
        // retention is scoped to those two) diverges here.
        if self.argument_has_own_line_comment(yield_expr.span.start, arg) {
            // Shared with `return`/`throw` (`build_comment_paren_doc`), the three restricted
            // productions on one path: a same-line-as-`(` `//` comment trails the `(`, a
            // sequence operand renders bare, and a comment before the `)` stays inside. The
            // boundary is discarded — `yield` is an expression, so its enclosing statement
            // (not this doc) appends the `;`, and `yield_expr.span.end` is the `)`.
            let (hanging, _boundary) = self.build_restricted_production_paren_doc(
                keyword,
                keyword_end,
                arg,
                yield_expr.span.end,
            );
            return hanging;
        }

        let mut parts: DocBuf = smallvec![d.text(keyword), d.text(" ")];
        // Every remaining comment is glued to the keyword with the operand after it on
        // some line, so the operand is pulled up onto the comment's line rather than
        // keeping the author's break — the break would be ASI, not layout.
        let leading_comments_opt = self.build_rhs_comments_glued_opt(keyword_end, argument_start);

        if leading_comments_opt.is_some() || has_trailing_comments {
            let inner = self.build_expression_doc(arg);
            let body = match leading_comments_opt {
                Some(comments) => d.concat(&[comments, inner]),
                None => inner,
            };
            // The operand→`)` gap retains its shell, exactly as `await`'s does and for the
            // same reason: `yield`'s span covers the `)`, so a stripped form hands the
            // comment to the enclosing terminator gap on reparse — a block comment lands
            // past the `;` on pass 2, and a `//` merges with whatever already trails that
            // line. An assignment operand's clarity parens are the same pair.
            parts.push(
                self.build_paren_operand_comment_doc(
                    argument_end,
                    yield_expr.span.end,
                    body,
                    body,
                    ")",
                )
                .unwrap_or(body),
            );
        } else if self.needs_parens(arg, ParenContext::YieldArgument) {
            // Assignment needs parens: `yield (x ??= y)`
            parts.push(d.text("("));
            parts.push(self.build_expression_doc(arg));
            parts.push(d.text(")"));
        } else {
            parts.push(self.build_expression_doc(arg));
        }

        d.concat(&parts)
    }

    /// Build a Doc for a sequence expression
    ///
    /// Redundantly-parenthesized operand comments anchored to the sequence's
    /// outer *edges* float OUT of the sequence parens, matching prettier's fixed
    /// point: a leading comment on the first operand (`((/* c */ x), y)`) is
    /// emitted before the opening `(` (`/* c */ (x, y)`) and a trailing comment
    /// on the last operand (`(x, (y /* c */))`) after the closing `)`
    /// (`(x, y) /* c */`). Each floated comment keeps its source line-treatment —
    /// own-line (hardline) when a newline separates it from the operand, inline
    /// (space) otherwise. Preserving the line-treatment is what makes the float
    /// idempotent even when the sequence is nested inside surrounding comments (a
    /// naive always-inline float re-collapses on the second pass).
    /// See operand_edge_comment_prettier_divergence.
    ///
    /// Interior operand comments (between two operands) stay stripped + inline on
    /// the comma-gap path below and match prettier — see operand_comments.
    ///
    /// This is the statement/throw/call-argument default (the comment floats out).
    /// Value positions (return / variable init / assignment RHS) instead keep the
    /// last operand's trailing comment INSIDE the parens — see
    /// [`Self::build_sequence_doc_value`].
    ///
    /// `layout` is the orthogonal [`SeqLayout`] axis — the caller's position decides it,
    /// since tsv has no parent pointer to read it from.
    pub(in crate::printer) fn build_sequence_doc(
        &self,
        seq: &internal::SequenceExpression<'_>,
        layout: SeqLayout,
    ) -> DocId {
        // Float-out path: the last operand's trailing comment is the caller's job
        // (it lives in the stripped grouping-paren gap, outside `seq.span`), so the
        // in-sequence trailing scan stops at `seq.span.end`.
        self.build_sequence_doc_inner(seq, seq.span.end, SeqParens::FloatOut, layout)
    }

    /// Bare variant: the comma-joined operands **without** the sequence's own
    /// wrapping parens (and without the paren-gap edge-comment floats). Used where an
    /// enclosing construct already supplies the required grouping parens and owns the
    /// edge gaps — a restricted-production (`return` / `throw` / `yield` / `yield*`)
    /// argument forced into the hanging `kw (⏎ // c⏎ body⏎)` form by a leading own-line
    /// comment ([`Self::build_restricted_production_paren_doc`]). Self-parenthesizing
    /// there would double the parens (`return (⏎ (a, b)⏎)`); prettier keeps the sequence
    /// bare inside the one pair the comment break already needs.
    ///
    /// [`SeqLayout::Aligned`] is not a choice here: the enclosing construct has already
    /// emitted the hanging geometry (its `(`, indent and softlines), so the operands only
    /// have to join.
    pub(in crate::printer) fn build_sequence_doc_bare(
        &self,
        seq: &internal::SequenceExpression<'_>,
    ) -> DocId {
        self.build_sequence_doc_inner(seq, seq.span.end, SeqParens::Bare, SeqLayout::Aligned)
    }

    /// Value-position variant: a trailing comment on the last operand stays
    /// **inside** the parens (`return (a, b /* c */)` / `const x = (a, b // c)`)
    /// rather than floating out after `)`. Prettier keeps sequence/assignment
    /// trailing comments inside the added parens in value positions (return arg,
    /// variable init, assignment RHS) — its #19263 — while floating them out in
    /// statement / throw / call-argument positions. Callers in value positions use
    /// this; everything else uses [`Self::build_sequence_doc`].
    ///
    /// `trailing_end` is where the stripped grouping `)` sits (the comment between
    /// the last operand and it must be kept inside) — the caller finds it because
    /// it falls *outside* `seq.span` (the grouping parens aren't part of the node).
    pub(in crate::printer) fn build_sequence_doc_value(
        &self,
        seq: &internal::SequenceExpression<'_>,
        trailing_end: u32,
        layout: SeqLayout,
    ) -> DocId {
        self.build_sequence_doc_inner(seq, trailing_end, SeqParens::KeepInside, layout)
    }

    /// The doc for sequence operand `i` — the freeze-aware twin of the plain
    /// `build_expression_doc`, shared by both sequence layouts so neither spells the
    /// dispatch (nor the assignment-paren rule) itself.
    ///
    /// **Rule A** over the operand list: an own-line directive in the gap after the
    /// previous operand's comma freezes the FOLLOWING operand over its own node span, and
    /// the separating `,` stays parent-owned. The first operand has no such gap here — a
    /// directive before it leads the *sequence node*, which is the enclosing value head's
    /// question ([`Printer::value_head_frozen_span`]).
    ///
    /// The assignment clarity parens are the printer's, not the author's, so they land
    /// OUTSIDE the frozen slice, exactly as an argument's do
    /// ([`Printer::build_frozen_arg_doc`]); a nested sequence operand's own grouping parens
    /// are re-synthesized by [`Printer::build_frozen_expression_doc`], since slicing the
    /// node span alone would drop them and change what the operand means.
    fn build_sequence_operand_doc(
        &self,
        seq: &internal::SequenceExpression<'_>,
        i: usize,
    ) -> DocId {
        let d = self.d();
        let expr = &seq.expressions[i];
        let frozen = (i > 0)
            .then(|| self.gap_frozen_span(seq.expressions[i - 1].span().end, expr.span()))
            .flatten();
        let core = frozen.map_or_else(
            || self.build_expression_doc(expr),
            |frozen| self.build_frozen_expression_doc(expr, frozen),
        );
        if matches!(expr, Expression::AssignmentExpression(_)) {
            d.parens(core)
        } else {
            core
        }
    }

    /// Shape a comma-joined operand run per [`SeqLayout`] — prettier's three arms in one
    /// place, shared by the block-comment path and its line-comment twin so the two cannot
    /// answer the geometry differently. The parens (when there are any) sit outside the
    /// group, as prettier's do: they are added by the paren machinery, not by
    /// `printSequenceExpression`.
    ///
    /// `first_end` is where the FIRST operand's docs end in `inner` (the index of its `,`),
    /// and only `Indented` reads it — but it is why that arm cannot be spelled
    /// `group(indent(run))`. ⚠️ **An `indent` reaches every line inside it, including the
    /// ones an operand breaks ITSELF.** Prettier indents per continuation
    /// (`[first, ",", indent([line, next]), …]`), so a first operand that breaks internally
    /// — an assignment with a ternary RHS, `((a = b ? c : fn()), …)` — keeps its own lines at
    /// the base column. Wrapping the whole run instead pushed them one level in, which
    /// `prettier/tests/format/js/sequence-break/break.js` is the standing check for.
    fn build_sequence_layout_doc(
        &self,
        inner: &[DocId],
        first_end: usize,
        layout: SeqLayout,
    ) -> DocId {
        let d = self.d();
        match layout {
            SeqLayout::Aligned => d.group(d.concat(inner)),
            SeqLayout::Indented => {
                let (first, rest) = inner.split_at(first_end.min(inner.len()));
                d.group(d.concat(&[d.concat(first), d.indent(d.concat(rest))]))
            }
            // The expanding-parens body every other paren pair that drops its content onto
            // its own lines already uses — the shape, not a copy of it.
            SeqLayout::Hanging => self.build_expanding_parens_body_doc(d.concat(inner)),
        }
    }

    /// Wrap a built operand run in the sequence's own paren ENVELOPE: the first operand's
    /// leading-edge comments floated out before `(`, the run, `)`, and the last operand's
    /// trailing-edge comments floated out after it.
    ///
    /// The two builders differ only in how they assemble the run — the envelope around it is
    /// one question with one answer, so they share this rather than each spelling it. That is
    /// the standing hazard in a pair of parallel printers: the halves that look incidental are
    /// where they drift.
    ///
    /// `SeqParens::Bare` has no envelope at all: the enclosing construct supplies the parens
    /// and owns both edge gaps. `KeepInside` keeps the trailing comments inside the run (the
    /// caller has already appended them), so only the float-out mode reaches the epilogue.
    fn build_sequence_envelope_doc(
        &self,
        seq: &internal::SequenceExpression<'_>,
        body: DocId,
        parens: SeqParens,
    ) -> DocId {
        if matches!(parens, SeqParens::Bare) {
            return body;
        }
        let d = self.d();
        let mut parts = DocBuf::with_capacity(4);
        self.append_floated_leading_comments(
            &mut parts,
            seq.span.start,
            seq.expressions[0].span().start,
        );
        parts.push(d.text("("));
        parts.push(body);
        parts.push(d.text(")"));
        if !matches!(parens, SeqParens::KeepInside) {
            // Same-line block comments stay inline (`(x, y) /* c */`); own-line block
            // comments defer via `line_suffix` (`append_trailing_paren_comments`) so they
            // land past the enclosing comma/semicolon — where they re-parse to, keeping the
            // float idempotent.
            let last_end = seq.expressions[seq.expressions.len() - 1].span().end;
            self.append_trailing_paren_comments(&mut parts, last_end, seq.span.end);
        }
        d.concat(&parts)
    }

    fn build_sequence_doc_inner(
        &self,
        seq: &internal::SequenceExpression<'_>,
        trailing_end: u32,
        parens: SeqParens,
        layout: SeqLayout,
    ) -> DocId {
        let keep_trailing_inside = matches!(parens, SeqParens::KeepInside);
        // Line comments anywhere up to `trailing_end` (incl. the last operand's
        // trailing comment, which lives outside `seq.span` in value positions) need
        // break handling so the comment isn't swallowed by the following comma/operand
        // or the closing `)`.
        // Axis-free: the rule looks only at LINE comments, and ownership binds only a block
        // comment (`owned ⇒ is_block`), so skipping and counting give the same answer.
        // An honored directive routes here too, whatever its spelling: the flat path below
        // emits an inter-operand comment inline before its operand, which would glue an
        // own-line block directive onto the operand's line — an inert placement, so the
        // freeze would be lost on the second pass.
        if comments_to_emit_in_range(self.comments, seq.span.start, trailing_end)
            .any(|c| !c.is_block || self.is_honored_directive(c))
        {
            return self.build_sequence_doc_with_line_comments(seq, trailing_end, parens, layout);
        }

        let d = self.d();
        let n = seq.expressions.len();
        // The comma-joined operand run the layout shapes; the parens and the floated edge
        // comments around it are the envelope's (`build_sequence_envelope_doc`).
        let mut inner = DocBuf::with_capacity(n * 3);

        // Whole-sequence comment gate: the inter-operand gaps (after/before each comma)
        // all lie within `seq.span`, so with no comment there, every per-operand gap is
        // empty. Skip the per-operand comma scans + the `empty()` comment children on the
        // comment-free common path. Byte-identical (the line-comment path already branched
        // off above, so a present comment here is a block, handled by the full path).
        let seq_has_comments = self.has_comments_to_emit_between(seq.span.start, seq.span.end);

        // Where the first operand's docs end — the continuation indent starts here, never
        // at the run's own start (see `build_sequence_layout_doc`).
        let mut first_end = 0;
        for (i, expr) in seq.expressions.iter().enumerate() {
            let is_last = i + 1 == n;
            let expr_start = expr.span().start;
            let expr_end = expr.span().end;

            if i > 0 {
                if i == 1 {
                    first_end = inner.len();
                }
                // `,` + `line`, prettier's `join([",", line], …)`: the operands break one per
                // line once they no longer fit, which is what keeps a sequence inside the
                // print width. A flat `", "` here made every sequence unbreakable.
                inner.push(d.text(","));
                inner.push(d.line());
                // Leading comments of this operand: the gap after the previous comma.
                // Redundant operand parens are stripped, so a comment the user wrote
                // inside them (`(/* c */ b)`) is preserved inline before the operand.
                if seq_has_comments {
                    let prev_end = seq.expressions[i - 1].span().end;
                    if let Some(comma) = self.find_comma_after(prev_end) {
                        inner.push(self.build_comments_between(
                            comma + 1,
                            expr_start,
                            CommentSpacing::Trailing,
                        ));
                    }
                }
            }

            inner.push(self.build_sequence_operand_doc(seq, i));

            // Trailing comments of this operand: the gap before the next comma.
            if seq_has_comments
                && !is_last
                && let Some(comma) = self.find_comma_after(expr_end)
            {
                inner.push(self.build_comments_between(expr_end, comma, CommentSpacing::Leading));
            }
        }
        if keep_trailing_inside {
            // Value position: a same-line block comment stays INSIDE before `)`
            // (`(a, b /* c */)`). Block-only path, so the comments are blocks. The
            // comment lives between the last operand and the grouping `)`
            // (`trailing_end`), outside `seq.span`. Inside `inner`, so a hanging layout's
            // closing softline lands after it rather than between operand and comment.
            let last_end = seq.expressions[n - 1].span().end;
            for comment in comments_to_emit_in_range(self.comments, last_end, trailing_end) {
                inner.push(d.text(" "));
                inner.push(self.build_comment_doc(comment));
            }
        }

        let body = self.build_sequence_layout_doc(&inner, first_end, layout);
        self.build_sequence_envelope_doc(seq, body, parens)
    }

    /// Emit the first operand's leading-edge comments, floated out before the
    /// sequence's opening `(`, preserving each comment's source line-treatment:
    /// own-line (a newline before the operand) → hardline, inline → space. The
    /// spacing follows each comment and is sized by the gap to the next token (the
    /// following comment, else the operand at `operand_start`). On re-parse these
    /// land in the enclosing context's leading-comment domain, which emits the
    /// same own-line/inline treatment — so the float is idempotent.
    fn append_floated_leading_comments(&self, parts: &mut DocBuf, start: u32, operand_start: u32) {
        let d = self.d();
        let comments: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, start, operand_start).collect();
        for (i, comment) in comments.iter().enumerate() {
            parts.push(self.build_comment_doc(comment));
            let next = comments.get(i + 1).map_or(operand_start, |c| c.span.start);
            // Comment-adjacency read (real even in canonical mode): a line comment
            // always has a newline before the next token, and gluing content after
            // its inline emission would swallow it.
            if self.comment_has_newline_between(comment.span.end, next) {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
        }
    }

    /// Sequence layout used when the sequence contains a line comment, which forces
    /// a multiline break so the comment isn't swallowed by the following comma or
    /// operand. Mirrors prettier's `group(join([",", line], parts))`: each comma gap
    /// is partitioned by line — a comment with no newline before it *trails* the
    /// preceding operand (a same-line block stays inline before the comma; a line
    /// comment defers past the comma via `line_suffix`, rendering at end-of-line);
    /// an own-line comment *leads* the next operand on its own line. A `break_parent`
    /// forces the group (and any enclosing call/arg group) to break.
    ///
    /// The outer-edge comments — leading on the first operand, trailing on the last —
    /// still float OUT of the parens via the same helpers as the block-comment path
    /// (`append_floated_leading_comments` / `append_trailing_paren_comments`).
    ///
    /// The geometry is the caller's [`SeqLayout`], the same axis the block path takes — a
    /// forced break doesn't change WHICH layout the position is, only that it happens. Under
    /// `Aligned` a deferred trailing `//` rides its `line_suffix` past the `)` and the
    /// enclosing `;` (`(aa,⏎bb); // c`); under `Hanging` the closing softline flushes it
    /// inside (`return (⏎aa,⏎bb // c⏎);`) — both prettier's, from one rule.
    fn build_sequence_doc_with_line_comments(
        &self,
        seq: &internal::SequenceExpression<'_>,
        trailing_end: u32,
        parens: SeqParens,
        layout: SeqLayout,
    ) -> DocId {
        let keep_trailing_inside = matches!(parens, SeqParens::KeepInside);
        let d = self.d();
        let n = seq.expressions.len();

        // Build per-operand docs (own-line leading + core + same-line trailing),
        // joined by `,` + line inside a group forced to break. `first_end` is where the
        // first operand's docs end, for the layout's continuation indent.
        let mut inner: DocBuf = smallvec![d.break_parent()];
        let mut first_end = 0;
        for (i, expr) in seq.expressions.iter().enumerate() {
            let is_last = i + 1 == n;
            let expr_start = expr.span().start;
            let expr_end = expr.span().end;
            let mut od = DocBuf::new();

            // Own-line comments from the previous comma gap lead this operand.
            // The same-line prefix of that gap trails the previous operand (emitted
            // there), so skip it here; once a comment is own-line the rest follow.
            if i > 0 {
                let prev_end = seq.expressions[i - 1].span().end;
                let mut pos = prev_end;
                let mut in_trailing_run = true;
                for comment in comments_to_emit_in_range(self.comments, prev_end, expr_start) {
                    // Comment-adjacency read (real even in canonical mode).
                    let own_line = self.comment_has_newline_between(pos, comment.span.start);
                    // Once a comment is own-line (or the trailing run already ended),
                    // it and the rest lead the next operand.
                    if !in_trailing_run || own_line {
                        in_trailing_run = false;
                        od.push(self.build_comment_doc(comment));
                        od.push(d.hardline());
                    }
                    pos = comment.span.end;
                }
            }

            od.push(self.build_sequence_operand_doc(seq, i));

            // Same-line comments in the next comma gap trail this operand: a block
            // stays inline before the comma; a line comment defers via `line_suffix`
            // so it renders after the comma at end-of-line. Own-line comments belong
            // to the next operand (handled above), so stop at the first one.
            if !is_last {
                let next_start = seq.expressions[i + 1].span().start;
                let mut pos = expr_end;
                for comment in comments_to_emit_in_range(self.comments, expr_end, next_start) {
                    // Comment-adjacency read (real even in canonical mode): an
                    // own-line comment must lead the next operand, not merge into
                    // the previous operand's `line_suffix` trailing run.
                    if self.comment_has_newline_between(pos, comment.span.start) {
                        break;
                    }
                    // Same-line trailing comment: block inline before the comma, line
                    // comment deferred via `line_suffix` to render after the comma.
                    od.push(self.build_trailing_comment_doc(comment));
                    pos = comment.span.end;
                }
            } else if keep_trailing_inside {
                // Value position: the last operand's trailing comment trails the operand — a
                // block inline, a line comment via `line_suffix`. Whether the `//` lands
                // inside the parens is then the LAYOUT's answer, not this branch's: a
                // `Hanging` closing softline flushes it inside (`b // c` then `)` on its own
                // line), an `Aligned` one has no break left before `)`, so it rides out past
                // the `);` — which is prettier's split too. The comment lives up to the
                // grouping `)` (`trailing_end`), outside `seq.span`.
                for comment in comments_to_emit_in_range(self.comments, expr_end, trailing_end) {
                    od.push(self.build_trailing_comment_doc(comment));
                }
            }

            if i > 0 {
                if i == 1 {
                    first_end = inner.len();
                }
                inner.push(d.text(","));
                inner.push(d.line());
            }
            inner.push(d.concat(&od));
        }

        let body = self.build_sequence_layout_doc(&inner, first_end, layout);
        self.build_sequence_envelope_doc(seq, body, parens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run `should_group_binary_continuation` on a parsed binary expression.
    fn group(src: &str) -> bool {
        let arena = bumpalo::Bump::new();
        let expr = crate::parse_expression_with_comments(src, 0, &arena)
            .expect("expression should parse")
            .0;
        match expr {
            Expression::BinaryExpression(b) => Printer::should_group_binary_continuation(&b),
            other => panic!("expected a binary expression, got: {other:?}"),
        }
    }

    #[test]
    fn should_group_binary_continuation_by_category() {
        // A logical operand under an arithmetic parent — categories differ, so the
        // continuation gets its own group.
        assert!(group("(a && b) + c"));
        assert!(group("(a && b) * c"));
        // Flattened same-category chains do NOT group (the left is the same category).
        assert!(!group("a && b && c"));
        assert!(!group("a * b * c"));
    }
}
