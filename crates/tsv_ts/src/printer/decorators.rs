// Decorator printing for TypeScript
//
// Class-level decorators (always own-line), class-member decorators, and
// parameter decorators (both inline vs own-line preserved from source),
// including comment placement between decorators and the decorated member.

use crate::ast::internal;
use crate::printer::comments::CommentVec;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::{Comment, CommentPosition, Span, classify_comment, doc::arena::DocId};

use super::Printer;

/// One decorator list, walked into the joinable items all three decorator printers lay
/// out, plus the two facts that decide whether that layout can stay flat.
///
/// Each decorator — with the comments that hug it and the author blank that ends its gap
/// — is one item, and each own-line comment run in a gap is the item after it. A printer's
/// whole remaining job is to pick the separator and the tail.
struct DecoratorItems {
    /// The decorator and own-line-run items, in source order.
    items: DocBuf,
    /// A `//` sits in some decorator gap — it runs to end-of-line, so nothing that
    /// follows can share that line.
    has_line_comment: bool,
    /// Some gap holds an own-line comment run — an item whose own line is the author's,
    /// which collapsing onto the decorator's line would destroy.
    has_own_line_run: bool,
}

impl<'a> Printer<'a> {
    /// Build a Doc for a decorator's expression, parenthesizing when the bare
    /// decorator grammar doesn't cover it.
    ///
    /// Bare form: an identifier / non-computed non-optional member chain, or one
    /// non-optional call on such a chain — anything else (`@(fn().fn1())`,
    /// `@(a?.b)`, `@(a[b])`) keeps parens. Prettier ref:
    /// `canDecoratorExpressionUnparenthesized` in parentheses/parent-needs-parentheses.js.
    pub(in crate::printer) fn build_decorator_expression_doc(
        &self,
        decorator: &internal::Decorator<'_>,
    ) -> DocId {
        let d = self.d();
        let expr_doc = self.build_expression_doc(&decorator.expression);
        if can_decorator_expression_unparenthesized(&decorator.expression) {
            expr_doc
        } else {
            d.parens(expr_doc)
        }
    }

    /// Push a decorator's "head" — `@`, any comment authored between `@` and the
    /// decorator expression (`@/* c */ dec`, including inside stripped parens
    /// `@(/* c */ dec)`; dropping it is content loss, and it hugs the expression
    /// inline or drops to its own line per the author), the expression itself, and any
    /// comment authored between the expression and the decorator's end (`@(dec
    /// /* c */)`) — into `parts`. One item's head, pushed by
    /// [`Self::collect_decorator_items`]; the gap behind it and the separator toward the
    /// next item are that walk's, and the layout around the finished items is the three
    /// printers'.
    ///
    /// The trailing run lands **outside** the printed parens, so an author who wrote
    /// the comment inside them converges on the fixed point an author who wrote it
    /// outside already had: `@(a + b /* c */)` and `@(a + b) /* c */` both print as the
    /// latter. That is one fixed point rather than two, it needs no forced-open shell
    /// for a `//` (the decorator's own trailing break ends the line), and the parens are
    /// the printer's to synthesize — `build_decorator_expression_doc` adds or omits them
    /// from the expression's shape, never from the author's spelling, so a position
    /// defined relative to them carries no authorial signal to preserve.
    ///
    /// The range is empty for a decorator the printer leaves bare (`@fn`, `@fn()`),
    /// where `decorator.span` ends at the expression — so a comment the author wrote
    /// *after* the decorator stays the following gap's to emit, and cannot be
    /// double-printed here.
    fn push_decorator_head(&self, parts: &mut DocBuf, decorator: &internal::Decorator<'_>) {
        let d = self.d();
        parts.push(d.text("@"));
        if let Some(c) =
            self.build_rhs_comments_opt(decorator.span.start + 1, decorator.expression.span().start)
        {
            parts.push(c);
        }
        parts.push(self.build_decorator_expression_doc(decorator));
        self.push_trailing_comments_in_range(
            parts,
            decorator.expression.span().end,
            decorator.span.end,
        );
    }

    /// Push the author's blank line before a decorator's own-line comment run — the
    /// `literalline` half of the blank-preserving separator, left for the caller's own
    /// break (the member and parameter paths' item `line`, the class-level path's
    /// `hardline`) to supply the other half. A `literalline` reports `LAYOUT_BREAKS_FORCED`
    /// like a `hardline` does, so it forces every enclosing group open on its own: a pushed
    /// blank can never land in a flat render, whatever the caller's own break turns out to
    /// be.
    ///
    /// **Unconditional** — the one rule this emitter states: an author blank before an
    /// own-line decorator-gap comment run survives. It reaches every decorator gap that
    /// exists, class-level, member and parameter alike, because
    /// [`Self::collect_decorator_items`] is the only walk over them.
    /// The null control is what makes it a *comment run's* rule rather than a decorator
    /// gap's: with NO comment in the gap both formatters drop a blank between two
    /// decorators byte-identically, so nothing here preserves a blank the run does not
    /// carry.
    ///
    /// Prettier reaches the same blank through its **comment** printer instead, and only
    /// where one of three attachment handlers happens to fire — so one authoring gets four
    /// verdicts there. tsv inherits none of them: it prints the run where the author wrote
    /// it at every gap, so the blank in front of it is authoring signal at every gap too.
    /// The principle is `docs/comments.md` §Trailing and dangling runs; the gap-by-gap
    /// table lives once, in the fixture
    /// `tests/fixtures/typescript/statements/class/decorator_comment_blank_prettier_divergence`.
    ///
    /// ⚠️ The scan is the **in-source** question, so it opens at
    /// [`blank_scan_start`](Printer::blank_scan_start): a comment already emitted inline
    /// after the decorator (`@fn /* t */⏎⏎// c`) still occupies its bytes, and a
    /// multi-line one would otherwise hand its OWN newlines to the scan as an author
    /// blank (`docs/comments.md` §Trailing and dangling runs).
    fn push_decorator_run_blank(
        &self,
        parts: &mut DocBuf,
        decorator: &internal::Decorator<'_>,
        own_line_run: &[&Comment],
    ) {
        let Some(first) = own_line_run.first() else {
            return;
        };
        let from = self.blank_scan_start(decorator.span.end, first.span.start);
        if self.has_blank_line_between(from, first.span.start) {
            parts.push(self.d().literalline());
        }
    }

    /// Route one decorator's trailing gap.
    ///
    /// Each comment goes where [`Self::classify_decorator_gap_comment`] says: a same-line
    /// **trailing** one inline onto `buf` (behind the decorator just pushed), one the author
    /// left on the NEXT decorator's line into `pending` (drained at the top of the next
    /// iteration by [`Self::push_pending_decorator_prefix`]), and the **own-line** run
    /// handed back — it becomes an item of its own, which is the caller's to place.
    /// `has_line` rides along for the group-break gate the member and parameter layouts
    /// take.
    fn collect_decorator_gap_comments<'c>(
        &'c self,
        buf: &mut DocBuf,
        pending: &mut DocBuf,
        decorator: &internal::Decorator<'_>,
        boundary: u32,
        is_last: bool,
    ) -> (CommentVec<'c>, bool) {
        let mut own_line: CommentVec<'c> = CommentVec::new();
        let mut has_line = false;
        for comment in self.comments_to_emit_between(decorator.span.end, boundary) {
            has_line |= !comment.is_block;
            match self.classify_decorator_gap_comment(
                comment,
                decorator.span.end,
                boundary,
                is_last,
            ) {
                CommentPosition::Trailing => {
                    // A block inline, a `//` via `line_suffix` — the caller's join
                    // already breaks after a trailing `//` (`has_line`), so the suffix
                    // flushes there byte-identically. Deferring is what keeps it from
                    // landing AHEAD of a run the decorator's own doc already deferred
                    // (`@(a ? b : c // inj⏎) // c4` welded as `// c4 // inj`, reordered);
                    // as suffixes the two meet the flush in source order and the run
                    // separator breaks between them (`doc/arena_render_suffix.rs`).
                    buf.push(self.build_trailing_comment_doc(comment));
                }
                CommentPosition::LeadingInline => pending.push(self.build_comment_doc(comment)),
                CommentPosition::LeadingOwnLine => own_line.push(comment),
            }
        }
        (own_line, has_line)
    }

    /// Build the run of consecutive own-line leading comments authored between a
    /// decorator and the next token (the following decorator, or the decorated
    /// member / class / binding) as ONE doc. Two block comments the author left on
    /// the same source line stay together with a space (`/* c1 */ /* c2 */`); every
    /// other boundary drops to its own line via a blank-preserving `hardline` —
    /// prettier's leading-comment separator rule (`printLeadingComment`), the same
    /// rule `push_leading_comment_run` / `push_leading_comments_before` apply for an
    /// undecorated member. Emits NO trailing separator toward the token:
    /// the decorator layout's own item-join / trailing break connects to it. The run is
    /// built once, in [`Self::collect_decorator_items`], so a same-line comment pair
    /// renders identically at every decorator gap and whether or not it decorates.
    fn build_decorator_own_line_comment_run(&self, comments: &[&Comment]) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        let mut prev_end: Option<u32> = None;
        for comment in comments {
            if let Some(pe) = prev_end {
                if self.is_same_line(pe, comment.span.start) {
                    parts.push(d.text(" "));
                } else {
                    self.push_blank_preserving_hardline(&mut parts, pe, comment.span.start);
                }
            }
            parts.push(self.build_comment_doc(comment));
            prev_end = Some(comment.span.end);
        }
        d.concat(&parts)
    }

    /// Drain the `pending` run of LeadingInline comments — comments the author left
    /// on the *next* decorator's line — as an inline prefix on `buf` (each followed by
    /// a space), right before that decorator's head is pushed (`@a /* c */ @b`). A no-op
    /// when `pending` is empty.
    fn push_pending_decorator_prefix(&self, buf: &mut DocBuf, pending: &mut DocBuf) {
        let d = self.d();
        for leading in std::mem::take(pending) {
            buf.push(leading);
            buf.push(d.text(" "));
        }
    }

    /// Walk a decorator list into [`DecoratorItems`] — the loop all three decorator
    /// printers share. They differ only in what they do with the result: the separator
    /// they join the items with, and the tail they append (a `hardline` toward the class
    /// keyword, the member group's trailing break, the decorated binding).
    ///
    /// One walk rather than three copies: every rule that reaches a decorator gap has to
    /// reach all three printers, and while they each ran their own loop such a rule had to
    /// be added at three sites that only agreed because they were read side by side (the
    /// author-blank rule, [`Self::push_decorator_run_blank`], was the third in a row). The
    /// helpers below therefore each state ONE rule and are called from here alone — the
    /// agreement is the single loop, not a shared callee.
    fn collect_decorator_items(
        &self,
        decorators: &[internal::Decorator<'_>],
        next_token_start: u32,
    ) -> DecoratorItems {
        let d = self.d();
        let mut collected = DecoratorItems {
            items: DocBuf::new(),
            has_line_comment: false,
            has_own_line_run: false,
        };
        // A comment the author left on the *next* decorator's line prefixes that
        // decorator, carried across the iteration boundary.
        let mut pending: DocBuf = DocBuf::new();
        for (i, decorator) in decorators.iter().enumerate() {
            // The gap runs to the next decorator's `@`, or — past the last one — to
            // `next_token_start`, where the decorated thing itself begins (class keyword,
            // member key, parameter binding).
            let next = decorators.get(i + 1);
            let boundary = next.map_or(next_token_start, |n| n.span.start);
            let is_last = next.is_none();

            let mut item: DocBuf = DocBuf::new();
            self.push_pending_decorator_prefix(&mut item, &mut pending);
            self.push_decorator_head(&mut item, decorator);

            // Comments between this decorator and the next boundary: a same-line trailing
            // one hugs the decorator inline, one on the NEXT decorator's line goes to
            // `pending`, and the own-line run comes back to become the next item.
            let (own_line_refs, gap_has_line) = self.collect_decorator_gap_comments(
                &mut item,
                &mut pending,
                decorator,
                boundary,
                is_last,
            );
            collected.has_line_comment |= gap_has_line;
            collected.has_own_line_run |= !own_line_refs.is_empty();

            // The author blank rides the END of the decorator's own item, ahead of the
            // join's separator — the two render as `literalline` + break, the same order
            // `push_blank_preserving_hardline` emits.
            self.push_decorator_run_blank(&mut item, decorator, &own_line_refs);
            collected.items.push(d.concat(&item));
            // Consecutive own-line comments the author left on one source line stay
            // together as one item (`/* c1 */ /* c2 */`), not split across lines.
            if !own_line_refs.is_empty() {
                collected
                    .items
                    .push(self.build_decorator_own_line_comment_run(&own_line_refs));
            }
        }
        collected
    }

    /// Where a comment authored in a decorator's trailing gap goes — the one predicate
    /// behind every decorator gap, class-level, member and parameter alike.
    ///
    /// **Between two decorators**, and unlike the generic `classify_comment` (which
    /// prioritises *trailing* the previous node), the comment prefers to **lead the next
    /// decorator**: prettier attaches it to the following decorator (its tree walk visits
    /// the decorated key before the decorators), so a comment sharing the next decorator's
    /// line leads it inline even when it also touches the previous decorator's line
    /// (`@a /* c */ @b` → `@a⏎/* c */ @b`). Only a comment on the previous decorator's
    /// line that doesn't reach the next (`@a /* c */⏎@b`) trails; anything fully on its
    /// own line is own-line.
    ///
    /// **At the LAST boundary** the next token is the decorated thing, which prettier
    /// never inline-leads, so only `Trailing` survives and everything else drops to its
    /// own line. `LeadingInline` is therefore unreachable when `is_last` — which is what
    /// lets every caller match three arms with no guard. Writing that collapse
    /// twice (an explicit `match` at the class-level printer, a `LeadingInline if !is_last`
    /// guard at the member and parameter ones) is agreement by coincidence only.
    fn classify_decorator_gap_comment(
        &self,
        comment: &Comment,
        prev_end: u32,
        boundary: u32,
        is_last: bool,
    ) -> CommentPosition {
        if is_last {
            return match classify_comment(comment, prev_end, boundary, self.source) {
                CommentPosition::Trailing => CommentPosition::Trailing,
                _ => CommentPosition::LeadingOwnLine,
            };
        }
        if self.is_same_line(comment.span.end, boundary) {
            CommentPosition::LeadingInline
        } else if self.is_same_line(prev_end, comment.span.start) {
            CommentPosition::Trailing
        } else {
            CommentPosition::LeadingOwnLine
        }
    }

    /// prettier's `hasNewlineBetweenOrAfterDecorators`: true when any decorator is
    /// followed (skipping spaces/tabs) by a newline before the next token, so the
    /// decorators break onto their own lines. Comments between the decorator and
    /// the newline do NOT count — prettier only skips spaces/tabs, so `@fn /* c */\nb`
    /// yields false (the first non-space char is `/`). Drives both class-member
    /// decorators and parameter decorators.
    fn has_newline_after_any_decorator(&self, decorators: &[internal::Decorator<'_>]) -> bool {
        // A source newline after a decorator is authoring layout intent, erased in
        // the canonical reprint (decorators then stay inline unless width breaks them).
        if self.canonical {
            return false;
        }
        decorators.iter().any(|dec| {
            let end = dec.span.end as usize;
            self.source[end..]
                .bytes()
                .find(|&b| b != b' ' && b != b'\t')
                .is_some_and(|b| b == b'\n' || b == b'\r')
        })
    }

    /// The source position a decorated class member's own first token begins at — the
    /// boundary [`Self::build_class_member_decorators_doc`] scans its last decorator gap
    /// to. `span_start` for an undecorated member, whose span already starts at that token.
    ///
    /// Asked by the property and the method paths, which are otherwise separate builders:
    /// the boundary decides which gap a comment between the last decorator and the member
    /// falls in, so the two disagreeing about it would put the same comment in two places.
    pub(in crate::printer) fn member_decorator_boundary(
        &self,
        decorators: Option<&[internal::Decorator<'_>]>,
        span_start: u32,
    ) -> u32 {
        decorators
            .and_then(|decs| decs.last())
            .map_or(span_start, |dec| self.find_first_token_after(dec.span.end))
    }

    /// True when `expr` carries any parameter decorator — prettier's
    /// `hasNotParameterDecorator` (`print/function-parameters.js`), read positively.
    /// A decorated parameter never hugs at any of the three parameter builders: the
    /// decorator and its binding are one group of their own
    /// ([`Self::with_param_decorators`]), which a hug — `(` welded to the binding, the
    /// binding's own doc left to break — cannot express. The gate is the DECORATOR's
    /// presence, not its layout: an inline `@dec a: { … }` declines the hug exactly as an
    /// own-line one does, and the list breaks around the parameter instead.
    pub(in crate::printer) fn param_has_decorators(&self, expr: &internal::Expression<'_>) -> bool {
        param_decorators(expr).is_some()
    }

    /// The source position where a parameter's rendered form begins: its first
    /// decorator when it carries parameter decorators, else the binding itself.
    /// Decorators precede the binding but are stored *on* it, so the binding span
    /// alone skips them — measuring a blank line to it would miscount a decorator
    /// line as an author blank line.
    pub(in crate::printer) fn param_start_with_decorators(
        &self,
        expr: &internal::Expression<'_>,
    ) -> u32 {
        param_decorators(expr).map_or_else(|| expr.span().start, |decs| decs[0].span.start)
    }

    /// The source span a parameter renders from: its node span widened at the start to
    /// cover any parameter decorators (see [`Self::param_start_with_decorators`]). The
    /// span form of that position, and the slice a parameter freeze emits verbatim.
    pub(in crate::printer) fn param_render_span(&self, expr: &internal::Expression<'_>) -> Span {
        Span::new(self.param_start_with_decorators(expr), expr.span().end)
    }

    /// The span an alone-on-line format-ignore directive between a DECORATED parameter's
    /// decorators and its binding freezes (`@dec⏎// prettier-ignore⏎private a:   T`) — the
    /// binding, the node the directive precedes. `None` when the parameter carries no
    /// decorator or no directive leads its binding.
    ///
    /// This is the inner half of a decorated parameter's two freeze positions; the
    /// parameter-list gap ([`Printer::param_frozen_span`]) covers the outer half, where a
    /// directive before the FIRST decorator freezes the whole parameter, decorators
    /// included. Each freezes exactly what the directive precedes.
    ///
    /// Named separately from the doc builder below because the parameter list's comment
    /// seam asks it too: the freeze prints this span RAW, so it is also how far the
    /// parameter PRINTED, which is the seam's claim anchor
    /// ([`Printer::element_claim_anchor`]). Reading only the outer half there reports "not
    /// frozen" for a binding this half emitted verbatim, and the seam re-claims a comment
    /// inside it.
    ///
    /// Inlined for the same reason as every gated entry in `printer::ignore`: every
    /// parameter of every document asks this, so a directive-free document must pay the
    /// one predicted branch and never the call.
    #[inline]
    pub(in crate::printer) fn param_binding_frozen_span(
        &self,
        param: &internal::Expression<'_>,
    ) -> Option<Span> {
        if !self.has_format_ignore {
            return None;
        }
        let decorators = param_decorators(param)?;
        // The slice is non-empty by `param_decorators`' contract, so `last()` always
        // yields — the `?` on it is the compiler's price for saying so, not a third case.
        self.member_gap_frozen(decorators.last()?.span.end, param.span().start)
            .then(|| param.span())
    }

    /// The frozen doc for the binding [`Self::param_binding_frozen_span`] selects: the
    /// decorators print normally — with the directive among them, at the author's
    /// position — and the binding is emitted verbatim.
    #[inline]
    pub(in crate::printer) fn build_frozen_param_binding_doc(
        &self,
        param: &internal::Expression<'_>,
    ) -> Option<DocId> {
        let frozen = self.param_binding_frozen_span(param)?;
        Some(self.with_param_decorators(
            param_decorators(param),
            self.build_frozen_span_doc(frozen),
            frozen.start,
        ))
    }

    /// Prefix a parameter binding's doc with its parameter decorators, preserving
    /// any comments the author interleaved with them — prettier's generic
    /// `printDecorators` (`print/decorators.js`) plus the `group([decoratorsDoc, doc])`
    /// its caller wraps around the pair (`print/index.js`).
    ///
    /// **The decorators and their binding are ONE group joined by `line`s**, so the
    /// decorators keep the binding's line while the whole pair fits and each drops to its
    /// own line when it does not — the break is width-driven, not spelled. A newline
    /// after any decorator in the source (prettier's `hasNewlineBetweenOrAfterDecorators`)
    /// forces that break through a `break_parent`, which also carries it out to
    /// the enclosing parameter group and expands the list. This group is why a decorated
    /// parameter can never hug ([`Printer::param_has_decorators`]): the hug welds `(` to the
    /// binding and leaves the binding's own doc to break, which is a different shape from
    /// a pair that breaks between the decorator and the binding.
    ///
    /// A no-op when there are no decorators. Used for destructuring and
    /// default parameters (`@dec { a }: T`, `@dec a = 1`) — whose decorators acorn
    /// stores on the pattern / `AssignmentPattern` node — plus the parameter-property
    /// path. `inner_start` is the source position where the rendered binding begins
    /// (its first modifier for a parameter property, else the binding node); it
    /// bounds the scan for comments after the last decorator.
    pub(in crate::printer) fn with_param_decorators(
        &self,
        decorators: Option<&[internal::Decorator<'_>]>,
        inner: DocId,
        inner_start: u32,
    ) -> DocId {
        let Some(decorators) = non_empty_decorators(decorators) else {
            return inner;
        };
        // Common case — no comment interleaved with the decorators: emit the bare
        // `@expr <sep> … <sep> binding` flat, skipping the comment scans and the
        // per-decorator segment wrapping the comment-aware path needs.
        if !self.has_comments_to_emit_between(decorators[0].span.start, inner_start) {
            let d = self.d();
            let mut parts = DocBuf::new();
            if self.has_newline_after_any_decorator(decorators) {
                parts.push(d.break_parent());
            }
            let sep = d.line();
            for decorator in decorators {
                parts.push(d.text("@"));
                parts.push(self.build_decorator_expression_doc(decorator));
                parts.push(sep);
            }
            parts.push(inner);
            return d.group(d.concat(&parts));
        }
        self.build_param_decorators_doc(decorators, inner, inner_start)
    }

    /// The comment-aware core of `with_param_decorators` (decorators non-empty and
    /// at least one comment interleaved among them — the bare case takes the flat
    /// fast path). Emits each decorator plus any comment written between `@` and the
    /// decorator expression (`@/* c */ dec`), between two decorators
    /// (`@dec1 /* c */ @dec2`), or between the last decorator and the binding
    /// (`@dec /* c */ x`), at the author's position — the parameter analog of
    /// `build_class_member_decorators_doc`. The segments are
    /// [`Self::collect_decorator_items`]'s items with the binding appended, joined with
    /// `line`s inside the pair's group and forced open by a `break_parent` in the
    /// own-line / line-comment layout. This is why a comment interleaved with parameter
    /// decorators is NOT hoisted into the leading-comment run:
    /// `build_leading_param_comments` stops collecting at the first decorator.
    fn build_param_decorators_doc(
        &self,
        decorators: &[internal::Decorator<'_>],
        inner: DocId,
        inner_start: u32,
    ) -> DocId {
        let d = self.d();
        let collected = self.collect_decorator_items(decorators, inner_start);

        // Own-line layout, the same three reasons the member printer takes
        // ([`Self::build_class_member_decorators_doc`]'s `needs_break`), two of them read
        // off the walk rather than asked of the source a second time: a newline after any
        // decorator (prettier's `hasNewlineBetweenOrAfterDecorators`), a `//` in any gap
        // (it runs to end-of-line, so the next segment cannot share it), or an own-line
        // comment RUN (a segment of its own, whose line is the author's). The last is the
        // one a `has_newline` test alone misses — `@fn1 /* t */⏎/* c */⏎@fn2 p` puts a
        // `/*` immediately after the decorator, so no newline is *directly* after one, yet
        // the run is still own-line and both prettier and the member path keep it there.
        let own_line = self.has_newline_after_any_decorator(decorators)
            || collected.has_line_comment
            || collected.has_own_line_run;

        let mut segments = collected.items;
        segments.push(inner);
        let joined = d.join_doc(segments, d.line());
        let body = if own_line {
            d.concat(&[d.break_parent(), joined])
        } else {
            joined
        };
        d.group(body)
    }

    /// Build a Doc for a list of decorators, each on its own line
    ///
    /// Returns None if there are no decorators.
    /// Each decorator is formatted as `@expression` followed by hardline.
    /// Used for class-level decorators which always go on their own line.
    ///
    /// The only path that never groups: [`Self::collect_decorator_items`]'s items — the
    /// decorators and any own-line comment runs among them — take a `hardline` apiece, the
    /// last one carrying to the class keyword. So the layout facts the walk reports are
    /// moot here; there is no flat render for them to rule out.
    pub(in crate::printer) fn build_decorators_doc(
        &self,
        decorators: Option<&[internal::Decorator<'_>]>,
        next_token_start: u32,
    ) -> Option<DocId> {
        let decorators = non_empty_decorators(decorators)?;
        let d = self.d();
        let items = self
            .collect_decorator_items(decorators, next_token_start)
            .items;
        Some(d.concat(&[d.join_doc(items, d.hardline()), d.hardline()]))
    }

    /// Build a Doc for class member decorators (properties and methods)
    ///
    /// Returns None if there are no decorators.
    /// Prettier preserves the original formatting: if any decorator has a newline
    /// after it in the source, all decorators go on their own lines. Otherwise,
    /// decorators stay inline (separated by spaces).
    ///
    /// Comments between decorators and between the last decorator and the member are
    /// [`Self::collect_decorator_items`]'s to route: a trailing one hugs its decorator
    /// inline, one on the next decorator's line prefixes that decorator, and an own-line
    /// run becomes an item of its own.
    ///
    /// Prettier ref: `printClassMemberDecorators` in print/decorators.js
    /// uses `hasNewlineBetweenOrAfterDecorators` to decide `hardline` vs `line`.
    pub(in crate::printer) fn build_class_member_decorators_doc(
        &self,
        decorators: Option<&[internal::Decorator<'_>]>,
        next_token_start: u32,
    ) -> Option<DocId> {
        let decorators = non_empty_decorators(decorators)?;
        let d = self.d();

        // Own-line if any decorator has a newline between it and the next token
        // (prettier's hasNewlineBetweenOrAfterDecorators).
        let has_newline_after = self.has_newline_after_any_decorator(decorators);
        let collected = self.collect_decorator_items(decorators, next_token_start);

        // group([join(line, items), hardline_or_line])
        // Between items: `line` (space in flat, newline in break)
        // After last item: `hardline` if source has newlines (forces group
        // to break), `line` otherwise (stays flat if group fits)
        let trailing = if has_newline_after {
            d.hardline()
        } else {
            d.line()
        };
        // Line comments and own-line comment runs force the group to break: a `//` takes the
        // rest of the line, and a run is an item whose own line is the author's.
        let needs_break = collected.has_line_comment || collected.has_own_line_run;
        let joined = d.join_doc(collected.items, d.line());
        let mut group_parts: DocBuf = smallvec![joined, trailing];
        if needs_break {
            group_parts.push(d.break_parent());
        }
        Some(d.group(d.concat(&group_parts)))
    }
}

/// The parameter decorators attached to a binding form (identifier / object or
/// array pattern / assignment-pattern default), reaching inside a
/// `TSParameterProperty` onto its inner binding — matching where acorn stores a
/// parameter's decorators. Returns `None` for any non-parameter or undecorated
/// form.
///
/// Normalized through [`non_empty_decorators`], so an undecorated parameter and a
/// decorated one with an empty list are the same answer here as everywhere else.
fn param_decorators<'arena>(
    expr: &internal::Expression<'arena>,
) -> Option<&'arena [internal::Decorator<'arena>]> {
    let decorators = match expr {
        internal::Expression::Identifier(id) => id.decorators(),
        internal::Expression::ObjectPattern(obj) => obj.decorators,
        internal::Expression::ArrayPattern(arr) => arr.decorators,
        internal::Expression::AssignmentPattern(ap) => ap.decorators,
        internal::Expression::TSParameterProperty(pp) => param_decorators(pp.parameter),
        _ => None,
    };
    non_empty_decorators(decorators)
}

/// The decorator slice a node carries, or `None` when it carries none — **the one
/// spelling of "does this hold anything"** for every `Option<&[Decorator]>` in the
/// printer. The parser attaches a slice only behind an `is_empty` guard
/// (`Parser::attach_param_decorators`, and the class paths likewise), so the filter costs a
/// predicted branch; what it buys is that an absent list and an empty one are one answer at
/// every reader, rather than four hand-rolled tests that can each read the empty case
/// differently.
///
/// A caller that wants the LAST decorator asks `.and_then(<[_]>::last)` of the raw field
/// instead — that is a different question which happens to answer this one, and routing it
/// through here only to index back in reads worse.
pub(in crate::printer) fn non_empty_decorators<'d, 'arena>(
    decorators: Option<&'d [internal::Decorator<'arena>]>,
) -> Option<&'d [internal::Decorator<'arena>]> {
    decorators.filter(|decs| !decs.is_empty())
}

/// Whether a class expression carries decorators (`@dec class {}`).
///
/// A decorated class expression breaks after the assignment operator (each
/// decorator on its own line); an undecorated one stays on the operator's line
/// and expands its body in place. Prettier ref: shouldBreakAfterOperator
/// (assignment.js:228) `case "ClassExpression": isNonEmptyArray(decorators)`;
/// the never-break ClassExpression case (assignment.js:189) only applies once
/// that has ruled out a decorated class. That assignment seam is one of six askers —
/// `needs_parens`, the class-body, statement, variable-declarator and expression-dispatch
/// paths ask it too — which is why it lives here rather than beside any one of them.
pub(in crate::printer) fn class_expr_has_decorators(c: &internal::ClassExpression<'_>) -> bool {
    non_empty_decorators(c.decorators).is_some()
}

/// Whether `expr` is a bare-decorator member chain: an identifier, or a
/// non-computed, non-optional member chain of identifiers down to one.
fn is_decorator_member_expression(expr: &internal::Expression<'_>) -> bool {
    match expr {
        internal::Expression::Identifier(_) => true,
        internal::Expression::MemberExpression(member) => {
            !member.computed
                && !member.optional
                && matches!(member.property, internal::Expression::Identifier(_))
                && is_decorator_member_expression(member.object)
        }
        _ => false,
    }
}

/// Whether a decorator expression is valid without parens (see
/// `Printer::build_decorator_expression_doc`).
fn can_decorator_expression_unparenthesized(expr: &internal::Expression<'_>) -> bool {
    match expr {
        internal::Expression::CallExpression(call) => {
            !call.optional && is_decorator_member_expression(call.callee)
        }
        _ => is_decorator_member_expression(expr),
    }
}
