// Destructuring pattern printing for TypeScript
//
// This module handles all destructuring patterns:
// - Object patterns: `{a, b}` with width-based expansion
// - Array patterns: `[a, b]`
// - Assignment patterns: `a = 1`
// - Assignment expressions: `a = b` with width-based wrapping and chain detection
// - Rest elements: `...rest`

use super::assignment::RhsCommentInfo;
use crate::ast::internal::{self, ArrowFunctionBody, Expression, ObjectPatternProperty};
use crate::printer::layout::hang_after_operator;
use crate::printer::{
    CommentVec, ParenContext, PatternContext, Printer, object_pattern_should_expand,
};
use smallvec::{SmallVec, smallvec};
use tsv_lang::Span;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

/// Context for assignment expression printing (chain detection)
///
/// Matches prettier's `path.match()` logic for determining when to use chain formatting.
/// Chain formatting is ONLY used when the parent is another assignment expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssignmentContext {
    /// Default context - parent is unknown or non-assignment
    None,
    /// Parent is ExpressionStatement or VariableDeclaration
    /// → Do NOT use chain formatting (use regular grouped layout)
    TopLevel,
    /// Parent is an assignment expression
    /// → Use chain formatting (ungrouped with line elements)
    Chain,
}

/// Check if an arrow function has a nested arrow function as its body
/// Used for chain-tail-arrow-chain detection: `(x) => (y) => x + y`
fn is_nested_arrow_function(expr: &Expression<'_>) -> bool {
    if let Expression::ArrowFunctionExpression(arrow) = expr
        && let ArrowFunctionBody::Expression(body_expr) = &arrow.body
    {
        return matches!(body_expr, Expression::ArrowFunctionExpression(_));
    }
    false
}

/// Layout role of an assignment-chain segment's right side (`a = b = value`).
///
/// Collapses the former `is_tail` / `is_arrow_chain` bool pair — `is_arrow_chain`
/// was only ever consulted when `is_tail` held, so the two flags encode three states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChainSegment {
    /// Chain middle (`a =` in `a = b = value`) — soft line break, no indent.
    Middle,
    /// Chain tail — indent the final value onto its own line when it breaks.
    Tail,
    /// Chain tail whose value is a nested arrow (`(x) => (y) => …`) — space, no indent.
    ArrowChainTail,
}

/// Build chain formatting doc: [group(left), op, ...right parts]
fn build_chain_doc(
    d: &tsv_lang::doc::arena::DocArena,
    left_doc: DocId,
    operator: &'static str,
    right_doc: DocId,
    segment: ChainSegment,
) -> DocId {
    let mut parts: DocBuf = smallvec![d.group(left_doc), d.text(operator)];

    match segment {
        ChainSegment::ArrowChainTail => {
            // Chain-tail-arrow-chain: (x) => (y) => x + y
            parts.push(d.text(" "));
            parts.push(right_doc);
        }
        ChainSegment::Tail => {
            // Standard chain tail: indent the final value
            parts.push(d.indent_line(right_doc));
        }
        ChainSegment::Middle => {
            // Chain middle: soft line break, no indent
            parts.push(d.line());
            parts.push(right_doc);
        }
    }

    d.concat(&parts)
}

impl<'a> Printer<'a> {
    /// Build a Doc for an assignment expression
    pub(super) fn build_assignment_doc(
        &self,
        assign: &internal::AssignmentExpression<'_>,
    ) -> DocId {
        // Determine initial context based on whether we're at top level
        let initial_context = if self.in_top_level_assignment.get() {
            AssignmentContext::TopLevel
        } else {
            AssignmentContext::None
        };
        self.build_assignment_doc_with_context(assign, initial_context)
    }

    /// Build a Doc for an assignment expression with chain context
    fn build_assignment_doc_with_context(
        &self,
        assign: &internal::AssignmentExpression<'_>,
        context: AssignmentContext,
    ) -> DocId {
        let d = self.d();
        let rhs_is_assignment = matches!(assign.right, Expression::AssignmentExpression(_));
        // A type-assertion target (`as` / `satisfies` / `<T>`) must be parenthesized
        // to round-trip (`(x as T) = …`); non-null `x!` stays bare. The cast is kept
        // in the internal AST so the formatter reproduces prettier's output, even
        // though the public AST drops it from a simple `=` left.
        let left_doc = if self.needs_parens(assign.left, ParenContext::AssignmentTarget) {
            d.concat(&[
                d.text("("),
                self.build_expression_doc(assign.left),
                d.text(")"),
            ])
        } else {
            self.build_expression_doc(assign.left)
        };

        // Extract inline comments between operator and RHS
        // Uses line-comment-safe spacing: block comments get trailing space,
        // line comments get hardline to prevent content absorption.
        let rhs_comment_start = assign.left.span().end;
        let rhs_comment_end = assign.right.span().start;

        // Promote comments that appear before the operator to the LHS.
        // e.g., `a /* comment */ = b` → comment stays before `=`, not after.
        let (left_doc, effective_rhs_start) = if let Some((promoted, new_start)) = self
            .promote_comments_before_operator(
                rhs_comment_start,
                rhs_comment_end,
                assign.operator.as_str(),
            ) {
            (d.concat(&[left_doc, promoted]), new_start)
        } else {
            (left_doc, rhs_comment_start)
        };

        let rhs_has_line_comment =
            self.has_line_comments_between(effective_rhs_start, rhs_comment_end);
        // A single-line block glued to the operator hugs the value even across a
        // source newline (`x = /* c */⏎v` → `x = /* c */ v`), matching prettier's
        // assignment layout.
        let rhs_comments = self.build_rhs_comments_glued_opt(effective_rhs_start, rhs_comment_end);
        // The `=`→RHS head: an own-line directive there freezes the whole RHS — including a
        // nested assignment, so a chain freezes from the directive down.
        let rhs_frozen = self.value_head_frozen_span(effective_rhs_start, assign.right.span());

        // For 2-segment chains at top level (a = b = value), use unified assignment layout.
        // Prettier only uses chain formatting for 3+ segments (assignment.js:113-125).
        // A 2-segment chain has rhs_is_assignment=true but the inner RHS is NOT an assignment.
        if !matches!(context, AssignmentContext::Chain)
            && rhs_is_assignment
            && !matches!(assign.left, Expression::ObjectPattern(_))
            && let Expression::AssignmentExpression(inner) = assign.right
        {
            let inner_rhs_is_assignment =
                matches!(inner.right, Expression::AssignmentExpression(_));
            if !inner_rhs_is_assignment {
                return self.build_assignment_layout(
                    left_doc,
                    assign.operator.as_str_with_leading_space(),
                    assign.right,
                    false,
                    RhsCommentInfo {
                        comments: rhs_comments,
                        has_line_comment: rhs_has_line_comment,
                        boundary: Some(assign.span.end),
                        frozen: rhs_frozen,
                    },
                );
            }
        }

        // Use unified assignment layout for simple (non-chain, non-pattern) cases.
        // build_assignment_layout builds right_doc internally and handles rhs_comments.
        if !matches!(context, AssignmentContext::Chain)
            && !matches!(assign.left, Expression::ObjectPattern(_))
            && !rhs_is_assignment
        {
            return self.build_assignment_layout(
                left_doc,
                assign.operator.as_str_with_leading_space(),
                assign.right,
                false,
                RhsCommentInfo {
                    comments: rhs_comments,
                    has_line_comment: rhs_has_line_comment,
                    boundary: Some(assign.span.end),
                    frozen: rhs_frozen,
                },
            );
        }

        // Build right doc for paths that handle layout directly
        let right_doc = if let Some(frozen) = rhs_frozen {
            self.build_frozen_expression_doc(assign.right, frozen)
        } else if let Expression::AssignmentExpression(rhs_assign) = assign.right {
            self.build_assignment_doc_with_context(rhs_assign, AssignmentContext::Chain)
        } else {
            self.build_expression_doc_with_paren_comments(assign.right, assign.span.end)
        };

        // Prepend inline comments to right doc if present
        let right_doc = if let Some(comments_doc) = rhs_comments {
            d.concat(&[comments_doc, right_doc])
        } else {
            right_doc
        };

        if matches!(context, AssignmentContext::Chain) {
            // Chain formatting - parent is an assignment
            let segment = if rhs_is_assignment {
                ChainSegment::Middle
            } else if is_nested_arrow_function(assign.right) {
                ChainSegment::ArrowChainTail
            } else {
                ChainSegment::Tail
            };
            build_chain_doc(
                d,
                left_doc,
                assign.operator.as_str_with_leading_space(),
                right_doc,
                segment,
            )
        } else if matches!(assign.left, Expression::ObjectPattern(_)) {
            // Object patterns on LHS - never break after operator, EXCEPT under an honored
            // directive: gluing the run to the operator would leave the directive trailing
            // `=`, an inert placement, so the freeze would die on the second pass. Hang the
            // value instead, as every other assignment host does.
            if rhs_frozen.is_some() {
                self.build_frozen_assignment_shape(
                    left_doc,
                    assign.operator.as_str_with_leading_space(),
                    right_doc,
                )
            } else {
                d.concat(&[
                    left_doc,
                    d.text(assign.operator.as_str_with_leading_space()),
                    d.text(" "),
                    right_doc,
                ])
            }
        } else {
            // RHS is a chain - group + indent (chain formatting from recursive call)
            d.group(d.concat(&[
                left_doc,
                d.text(assign.operator.as_str_with_leading_space()),
                d.indent_line(right_doc),
            ]))
        }
    }

    /// Build the doc tail that follows a destructuring-pattern parameter's
    /// closing `]`/`}`: the optional `?` marker (parameter position only), then a
    /// `: Type` annotation. Either may be absent — `[]`, `[]?`, `[]: T`, `[]?: T`.
    /// Shared by every object/array pattern path so the `?` lands between the
    /// brackets and the type uniformly.
    fn build_pattern_optional_type_tail(
        &self,
        optional: bool,
        type_annotation: Option<&internal::TSTypeAnnotation<'_>>,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        if optional {
            parts.push(d.text("?"));
        }
        if let Some(ta) = type_annotation {
            parts.push(self.build_type_annotation_doc(ta));
        }
        d.concat(&parts)
    }

    /// Build a Doc for an object pattern
    ///
    /// Prettier expands object patterns when:
    /// 1. Any property has a nested pattern value (always expand)
    /// 2. The pattern exceeds print width (width-based expansion)
    pub(super) fn build_object_pattern_doc(&self, obj: &internal::ObjectPattern<'_>) -> DocId {
        self.build_object_pattern_doc_with_context(obj, PatternContext::Standalone)
    }

    /// Build object pattern doc with explicit context
    pub(super) fn build_object_pattern_doc_with_context(
        &self,
        obj: &internal::ObjectPattern<'_>,
        context: PatternContext,
    ) -> DocId {
        let d = self.d();
        if obj.properties.is_empty() {
            self.build_empty_object_pattern_doc(obj)
        } else {
            // Expand if: nested patterns, line comments, blank lines, or own-line
            // block comments between/around properties. One whole-span comment
            // pre-check gates both comment-scanning hints (blank lines are
            // comment-independent, so `formatting_hints` still runs to detect them).
            let should_expand = object_pattern_should_expand(obj, context);
            let boundary = obj
                .type_annotation
                .as_ref()
                .map_or(obj.span.end, |t| t.span.start);
            let has_comments = self.has_comments_on_page_between(obj.span.start, boundary);
            let (has_line_comments, has_blank_lines) =
                self.object_pattern_formatting_hints(obj, has_comments);
            let has_own_line_block =
                has_comments && self.object_pattern_has_own_line_block_comments(obj);

            if should_expand || has_line_comments || has_blank_lines || has_own_line_block {
                self.build_expanded_object_pattern_doc(obj)
            } else {
                // Use group with line breaks for width-based expansion
                // Include type annotation in the group so its width is considered
                let mut parts = d.pooled_docbuf();

                // Track previous end for comment detection (start after `{`)
                let mut prev_end = obj.span.start + 1;

                for (i, prop) in obj.properties.iter().enumerate() {
                    // Check for leading comments before this property. Gated on the
                    // object-wide comment flag: with no comment in the pattern span, the
                    // per-property gap is empty too, so skip the `empty()` leading child
                    // (render + every fits pass would still walk it). Byte-identical.
                    let prop_start = prop.span().start;
                    if has_comments {
                        let leading_comments = self
                            .build_inline_comments_between_doc_trailing_space(prev_end, prop_start);
                        parts.push(leading_comments);
                    }

                    parts.push(self.build_object_pattern_property_doc(prop));

                    let prop_end = prop.span().end;
                    let is_last = i == obj.properties.len() - 1;

                    // Collect trailing comments (stop at next property or type annotation)
                    let upper_bound = obj
                        .properties
                        .get(i + 1)
                        .map(|next| next.span().start)
                        .or_else(|| obj.type_annotation.as_ref().map(|t| t.span.start))
                        .unwrap_or(obj.span.end);
                    let trailing = self.collect_trailing_comments(prop_end, upper_bound, is_last);

                    // Separator comma between properties; no trailing comma on the last
                    // property (trailingComma: 'none').
                    let comma = if !is_last { d.text(",") } else { d.empty() };
                    self.push_element_comma_trailing(&mut parts, &trailing, comma);

                    // Add line break between properties
                    if !is_last {
                        parts.push(d.line());
                    }

                    prev_end = trailing.end_pos;
                }

                // Whatever the last property's trailing run left unclaimed, before the
                // closing brace — e.g. `{a /*, b*/}`.
                let trailing = self.build_object_pattern_trailing_comments(obj, prev_end);
                parts.push(trailing);

                // Build group contents: { + properties + } with bracketSpacing
                // boundaries (space when flat `{ a }`, newline when broken).
                let mut group_parts: DocBuf = smallvec![
                    d.text("{"),
                    d.indent_line(d.concat(&parts)),
                    d.line(),
                    d.text("}"),
                ];

                // Include `?` + type annotation in the group for width calculation
                group_parts.push(
                    self.build_pattern_optional_type_tail(
                        obj.optional,
                        obj.type_annotation.as_ref(),
                    ),
                );

                d.group(d.concat(&group_parts))
            }
        }
    }

    /// Build trailing comments doc for the inline (single-line) object pattern
    /// path (between last property and `}`), e.g. `{a /*, b*/}`.
    ///
    /// `prev_end` is the loop's cursor — the last property's trailing-run end
    /// (`TrailingComments::end_pos`) — so everything still in `[prev_end, boundary)` is by
    /// construction unclaimed and emitted here. Re-deriving "not on the last property's line" instead would ask the gap a
    /// second, *different* question than the run that already answered it: the collector
    /// follows a multi-line block to its closing line, and a comment sitting on that line
    /// is neither on the property's line nor unclaimed. The expanded paths use
    /// `build_pattern_trailing_dangling_comments` instead, which puts each comment on its
    /// own line.
    fn build_object_pattern_trailing_comments(
        &self,
        obj: &internal::ObjectPattern<'_>,
        prev_end: u32,
    ) -> DocId {
        let d = self.d();
        let boundary = obj
            .type_annotation
            .as_ref()
            .map_or(obj.span.end, |t| t.span.start);

        let mut parts = DocBuf::new();
        for comment in comments_to_emit_in_range(self.comments, prev_end, boundary) {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }
        d.concat(&parts)
    }

    /// Build dangling comments after the last element of an *expanded* pattern
    /// (between the last element and the closing `}`/`]`).
    ///
    /// Each comment goes on its own line, with blank lines preserved. Same-line
    /// trailing comments are handled in the per-element loop, so they are
    /// skipped here. Mirrors the object/array expression printers' handling of
    /// trailing comments before the closing delimiter (`objects.rs`). Without
    /// this, expanded array patterns dropped these comments entirely (content
    /// loss) and expanded object patterns glued them onto the last property's
    /// line.
    ///
    /// ⚠️ `past_elision` says a trailing **hole** sits between `prev_end` and this run, and
    /// suppresses the blank line ahead of the FIRST comment. An elision's own line break is
    /// structure, not authorship — the rule `Printer::has_blank_line_after_slot` states for
    /// the array literal (prettier's `node &&`) — and a raw scan from the last binding's end
    /// counts it, so `const [a,⏎,⏎/* c */] = arr` grew a blank the first pass never wrote:
    /// not a fixed point. Blanks *between* two dangling comments are unaffected; only the
    /// one measured across the hole is.
    ///
    /// ⚠️ The run is measured from the last element's shell end
    /// ([`Printer::element_shell_end`]) — the same "structure is not authorship" rule in
    /// its *stripped paren* spelling. An array pattern's element span ends INSIDE its
    /// shell, so a raw scan from it reads the erased `)`'s line breaks as an author blank
    /// and fabricates one. The comment range still opens at `prev_end`: the peel steps over
    /// nothing but `)` and whitespace and stops at the first comment, so no interior
    /// comment is skipped.
    fn build_pattern_trailing_dangling_comments(
        &self,
        prev_end: u32,
        boundary: u32,
        past_elision: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        let mut last_pos = self.element_shell_end(prev_end, boundary);
        let mut first = true;
        for comment in comments_to_emit_in_range(self.comments, prev_end, boundary) {
            let blanks_measurable = !(first && past_elision);
            first = false;
            if self.is_same_line(last_pos, comment.span.start) {
                // Same source line as the preceding content, which here means the
                // preceding *comment*: the per-element loop's run already claimed
                // everything sharing a line with the element (and follows a multi-line
                // block to its closing line), so `prev_end` opens on a fresh line. The
                // test is keyed on the running `last_pos` rather than `prev_end` for
                // exactly that reason — it keeps two same-line dangling comments
                // together (`/* a */ // b`) instead of splitting them onto two lines.
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else {
                if blanks_measurable && self.has_blank_line_between(last_pos, comment.span.start) {
                    parts.push(d.literalline());
                }
                parts.push(d.hardline());
                parts.push(self.build_comment_doc(comment));
            }
            last_pos = comment.span.end;
        }
        d.concat(&parts)
    }

    /// Generic helper: Check for line comments and blank lines in a collection
    ///
    /// Returns (has_line_comments, has_blank_lines) in a single pass.
    /// Works for any collection with elements that have spans.
    ///
    /// `has_comments` is the caller's whole-span comment pre-check: when false,
    /// the per-element comment scans are skipped (no comment can be in any gap),
    /// leaving only the comment-independent blank-line detection.
    fn collection_formatting_hints<T>(
        &self,
        collection_start: u32,
        collection_end: u32,
        elements: &[T],
        has_comments: bool,
        get_span: impl Fn(&T) -> Span,
    ) -> (bool, bool) {
        let mut has_line_comments = false;
        let mut has_blank_lines = false;
        let mut prev_end = collection_start + 1; // After opening bracket/brace

        for elem in elements {
            let elem_start = get_span(elem).start;

            // Blank-line detection is comment-independent; when the collection has
            // no comments the first-comment lookup is a no-op (check_pos == elem_start).
            // **in source**: bounds a raw blank-line scan (see `blank_scan_end`).
            let check_pos = if has_comments {
                self.comments_in_source_between(prev_end, elem_start)
                    .next()
                    .map_or(elem_start, |c| c.span.start)
            } else {
                elem_start
            };
            if self.has_blank_line_between(prev_end, check_pos) {
                has_blank_lines = true;
            }

            if has_comments {
                // Check for line comments
                for comment in comments_to_emit_in_range(self.comments, prev_end, elem_start) {
                    if !comment.is_block {
                        has_line_comments = true;
                        break;
                    }
                }

                // Early exit if both found
                if has_line_comments && has_blank_lines {
                    return (true, true);
                }
            }

            prev_end = get_span(elem).end;
        }

        // Check comments after last element
        if has_comments {
            for comment in comments_to_emit_in_range(self.comments, prev_end, collection_end) {
                if !comment.is_block {
                    return (true, has_blank_lines);
                }
            }
        }

        (has_line_comments, has_blank_lines)
    }

    /// Check if object pattern has line comments or blank lines between properties
    ///
    /// Returns (has_line_comments, has_blank_lines) in a single pass.
    fn object_pattern_formatting_hints(
        &self,
        obj: &internal::ObjectPattern<'_>,
        has_comments: bool,
    ) -> (bool, bool) {
        let boundary = obj
            .type_annotation
            .as_ref()
            .map_or(obj.span.end, |t| t.span.start);
        self.collection_formatting_hints(
            obj.span.start,
            boundary,
            obj.properties,
            has_comments,
            ObjectPatternProperty::span,
        )
    }

    /// Check if object pattern has any own-line single-line block comments
    ///
    /// Mirrors `array_pattern_has_own_line_block_comments`: an own-line block
    /// comment (between, before, or after properties) forces expansion, matching
    /// prettier.
    fn object_pattern_has_own_line_block_comments(
        &self,
        obj: &internal::ObjectPattern<'_>,
    ) -> bool {
        let boundary = obj
            .type_annotation
            .as_ref()
            .map_or(obj.span.end, |t| t.span.start);
        let span = Span::new(obj.span.start, boundary);
        self.has_own_line_block_comments_in_bracket_list(
            span,
            obj.properties,
            ObjectPatternProperty::span,
        )
    }

    /// Build doc for empty object pattern: `{}` with optional `?` + type annotation
    fn build_empty_object_pattern_doc(&self, obj: &internal::ObjectPattern<'_>) -> DocId {
        let d = self.d();
        // Bound the comment scan to the braces (before any `?`/`: Type`), mirroring
        // `build_empty_array_pattern_doc`. Scanning the full span would pull a
        // comment out of the type annotation into the empty `{}` and duplicate it.
        let body_end = obj
            .type_annotation
            .as_ref()
            .map_or(obj.span.end, |t| t.span.start);
        let body_doc =
            self.build_empty_braces_inline_with_comments_doc(Span::new(obj.span.start, body_end));
        let tail =
            self.build_pattern_optional_type_tail(obj.optional, obj.type_annotation.as_ref());
        d.concat(&[body_doc, tail])
    }

    /// Build expanded doc for object pattern with hardlines (always multiline)
    fn build_expanded_object_pattern_doc(&self, obj: &internal::ObjectPattern<'_>) -> DocId {
        let d = self.d();

        // Zero-comment fast gate: one binary search over the whole object-pattern
        // window (up to any `: Type` annotation) decides whether the per-property
        // loop needs any comment work. This branch is reached on nesting depth or a
        // blank line alone (see `build_object_pattern_doc_with_context`), so a
        // deeply-nested but comment-free pattern lands here with nothing to scan —
        // the `{`-line prefix pull, the per-property leading-comment scan, the
        // format-ignore lookup, the between-property comment scan, and the trailing
        // dangling scan are all skipped. The gate is a general comment check (block
        // comments too, not just line comments): a same-line block comment can
        // reach this branch via nesting with no line comment, and the loop must
        // still emit it. The expansion decision itself runs earlier and is
        // unaffected. Canonical reference: build_params_doc_with_comments.
        let boundary = obj
            .type_annotation
            .as_ref()
            .map_or(obj.span.end, |t| t.span.start);
        let has_comments = self.has_comments_to_emit_between(obj.span.start, boundary);

        // A comment trailing the opening `{` on its own line is kept on the `{`
        // line when the pattern expands (divergence from prettier, which relocates
        // it to its own line as the first property's leading comment). See
        // conformance_prettier_ts_comments.md §Comment relocation (Object destructuring `{`).
        let first_prop_start = obj.properties[0].span().start;
        let (brace_line_prefix, brace_pull_pos) = if has_comments {
            self.delimiter_line_comment_prefix(obj.span.start, first_prop_start)
        } else {
            (DocBuf::new(), None)
        };

        // Track previous end for comment detection (start after `{`)
        let mut prev_end = obj.span.start + 1;

        let mut prop_parts = DocBuf::new();
        for (i, prop) in obj.properties.iter().enumerate() {
            // Handle leading comments before this property (with blank line preservation)
            let prop_start = prop.span().start;
            let leading_comments: CommentVec<'_> = if has_comments {
                self.collect_item_leading_comments(
                    prev_end,
                    prop_start,
                    (i == 0).then_some(brace_pull_pos).flatten(),
                )
            } else {
                CommentVec::new()
            };

            self.push_leading_comments_before(&mut prop_parts, &leading_comments, prop_start);

            // A preceding format-ignore directive keeps the property's source verbatim
            // (trailing comment/comma handled normally)
            if has_comments && self.member_gap_frozen(prev_end, prop_start) {
                prop_parts.push(self.raw_source_doc(prop.span()));
            } else {
                prop_parts.push(self.build_object_pattern_property_doc(prop));
            }

            let prop_end = prop.span().end;
            let is_last = i == obj.properties.len() - 1;

            // Collect trailing comments (stop at next property or type annotation)
            let upper_bound = obj
                .properties
                .get(i + 1)
                .map(|next| next.span().start)
                .or_else(|| obj.type_annotation.as_ref().map(|t| t.span.start))
                .unwrap_or(obj.span.end);
            let trailing = self.collect_trailing_comments(prop_end, upper_bound, is_last);

            // Separator comma between properties; no trailing comma on the last
            // property under `trailingComma: 'none'` (a rest element never takes one
            // either — it is a syntax error there).
            let comma = if !is_last { d.text(",") } else { d.empty() };
            self.push_element_comma_trailing(&mut prop_parts, &trailing, comma);

            if !is_last {
                // Blank line before the next property, on prettier's `isNextLineEmpty` —
                // the same question the object LITERAL asks, through the same helper,
                // because `printObject` prints `ObjectExpression` and `ObjectPattern`
                // through one path. The range-based predicate this used to ask cannot
                // express it: it counted the newlines in the gap, so a blank *after* a
                // comma the author pushed onto its own line (`a1⏎,⏎⏎b1`) read as an author
                // blank and was preserved, where prettier collapses it.
                let next_start = obj.properties[i + 1].span().start;
                self.push_item_blank_separator(&mut prop_parts, trailing.end_pos, next_start);
            }

            prev_end = trailing.end_pos;
        }

        // Check for dangling comments after the last property (before `}`)
        if has_comments {
            // An object pattern has no elisions, so the blank scan is never across one.
            prop_parts
                .push(self.build_pattern_trailing_dangling_comments(prev_end, boundary, false));
        }

        // Structure: { + brace-line prefix + indent(hardline + props) + hardline + } + type_annotation
        let mut result_parts: DocBuf = smallvec![
            d.text("{"),
            d.concat(&brace_line_prefix),
            d.indent(d.concat(&[d.hardline(), d.concat(&prop_parts)])),
            d.hardline(),
            d.text("}"),
        ];

        result_parts.push(
            self.build_pattern_optional_type_tail(obj.optional, obj.type_annotation.as_ref()),
        );

        d.concat(&result_parts)
    }

    /// Build a Doc for an object pattern property
    ///
    /// String keys that are valid identifiers are normalized to unquoted form:
    /// `{"key": value}` → `{key: value}`
    fn build_object_pattern_property_doc(&self, prop: &ObjectPatternProperty<'_>) -> DocId {
        let d = self.d();
        match prop {
            ObjectPatternProperty::Property(p) => {
                if p.shorthand {
                    // A shorthand default (`{ k = 1 }`) parses as
                    // `Property{shorthand, key: k, value: AssignmentPattern{left: k,
                    // right: 1}}`, so the *value* is already the whole `k = 1` binding
                    // and prints through `build_assignment_pattern_doc` — the same
                    // builder every other binding default uses (array element,
                    // non-shorthand property, standalone parameter). Delegating rather
                    // than re-deriving `key = default` here is load-bearing: this arm
                    // used to reach into `pat.right` directly and so never acquired
                    // that builder's `=`→value freeze (`value_head_frozen_span`) or its
                    // `ParenContext::DefaultValue` wrap, making the shorthand the one
                    // position where an own-line `prettier-ignore` was ignored and an
                    // `AssignmentExpression` default lost its required parens
                    // (`{ k = (a = 1) }`).
                    //
                    // `p.value` can never be a bare `AssignmentExpression` here:
                    // `ObjectPatternProperty::Property` is built at exactly one site
                    // (`object_property_to_pattern`), which routes the value through
                    // `to_assignable`, and that converts every `AssignmentExpression`
                    // into an `AssignmentPattern`.
                    debug_assert!(
                        !matches!(&p.value, Expression::AssignmentExpression(_)),
                        "a pattern property's value is converted to an AssignmentPattern"
                    );
                    match &p.value {
                        // Simple shorthand: `{k}` — the value repeats the key.
                        Expression::Identifier(_) => self.build_expression_doc(&p.key),
                        _ => self.build_expression_doc(&p.value),
                    }
                } else {
                    // Handle computed keys: {[key]: value}
                    // For regular keys, use property_key_doc to normalize string keys to identifiers
                    let key_region_end;
                    let key_doc = if p.computed {
                        let inner = self.build_expression_doc(&p.key);
                        let (doc, end) =
                            self.build_computed_key_bracket_doc(p.span.start, &p.key, inner);
                        key_region_end = end;
                        doc
                    } else {
                        key_region_end = p.key.span().end;
                        self.build_property_key_doc(&p.key)
                    };
                    // Comments between key and value, split at `:`
                    // e.g., `{[x] /* c1 */: /* c2 */ a}` → before `:` and after `:`
                    //
                    // The `: ` is static text, so the `:` byte scan exists only to split
                    // the two comment ranges — a comment-free `{a: b}` neither scans nor
                    // pushes an empty child.
                    let value_start = p.value.span().start;
                    let mut parts: DocBuf = smallvec![key_doc];
                    let mut tail: DocBuf = DocBuf::new();
                    // Set when a pre-`:` comment hangs what follows: the whole
                    // `: local` tail then drops to a continuation line.
                    let mut hang_range: Option<(u32, u32)> = None;
                    // Set when the after-`:` gap holds comments: the shared value-gap
                    // seam emits them and hangs the local when one forces a break.
                    let mut value_gap: Option<(u32, u32)> = None;
                    if self.has_comments_to_emit_between(key_region_end, value_start) {
                        #[allow(clippy::expect_used)]
                        // Parser guarantees `:` exists in destructuring property
                        let colon_pos = find_char_skipping_comments(
                            self.source.as_bytes(),
                            key_region_end as usize,
                            value_start as usize,
                            b':',
                        )
                        .expect(": not found in destructuring property")
                            as u32;
                        // Comment(s) before `:`: a glued block stays inline
                        // (`a /* c */: b`); a line comment — or a multiline block the
                        // author broke after — trails the key and drops `: local` to a
                        // continuation line indented one level (uniform
                        // forced-continuation indent; without the break a `//` would
                        // swallow `: b } = o` — content loss). tsv preserves the
                        // authored position — prettier hoists the comment to lead the
                        // whole property.
                        hang_range =
                            self.route_pre_separator_gap(&mut parts, key_region_end, colon_pos);
                        tail.push(d.text(": "));
                        // Comments after `:`: a glued block inlines onto the local; a
                        // line comment — or an own-line multiline block — hangs it,
                        // the local indented one level so it reads as this property's
                        // value, not a sibling (the same rule as the `=`→value gaps
                        // above). The shared seam owns that choice — the caller only
                        // says the gap holds comments.
                        if self.has_comments_to_emit_between(colon_pos + 1, value_start) {
                            value_gap = Some((colon_pos + 1, value_start));
                        }
                    } else {
                        tail.push(d.text(": "));
                    }
                    let value_doc = self.build_expression_doc(&p.value);
                    match value_gap {
                        Some((start, end)) => {
                            tail.push(self.build_pattern_value_gap_doc(start, end, value_doc));
                        }
                        None => tail.push(value_doc),
                    }
                    let tail_doc = d.concat(&tail);
                    match hang_range {
                        Some((start, end)) => {
                            parts.push(self.build_continuation_indent(start, end, tail_doc));
                        }
                        None => parts.push(tail_doc),
                    }
                    d.concat(&parts)
                }
            }
            ObjectPatternProperty::RestElement(r) => self.build_rest_element_doc(r),
        }
    }

    /// Build a Doc for an array pattern
    pub(super) fn build_array_pattern_doc(&self, arr: &internal::ArrayPattern<'_>) -> DocId {
        if arr.elements.is_empty() {
            return self.build_empty_array_pattern_doc(arr);
        }

        // Expand if line comments, multi-line block comments, or own-line block comments.
        // One whole-span comment pre-check gates the scans; array patterns don't
        // force-expand on blank lines, so a comment-free pattern skips the element scan
        // entirely.
        //
        // The three sub-questions must match the array EXPRESSION's gate
        // (`arrays.rs`'s `has_expanding_comments`) — this asked only two, and a comment
        // shape that expands `[a, b /* x⏎y */]` but not `const [a, b /* x⏎y */] = arr` is
        // a bug every time. It was: only the expanded path emits the dangling comments
        // after the last element, so a multi-line one landed on the grouped path and was
        // DROPPED outright (fixture `array_own_line_multiline_comment_expand`).
        // `has_own_line_block_comments_in_bracket_list` deliberately skips multi-line
        // comments — the multi-line question is this separate predicate's, not its.
        let boundary = arr
            .type_annotation
            .as_ref()
            .map_or(arr.span.end, |t| t.span.start);
        let has_comments = self.has_comments_on_page_between(arr.span.start, boundary);
        let (has_line_comments, has_multiline_block, has_own_line_block) = if has_comments {
            // Flatten once (skip holes) and share across the scans.
            let non_null: SmallVec<[_; 8]> = arr.elements.iter().flatten().collect();
            let has_line_comments = self
                .collection_formatting_hints(arr.span.start, boundary, &non_null, true, |elem| {
                    elem.span()
                })
                .0;
            let has_multiline_block =
                self.has_multiline_block_comments_on_page_between(arr.span.start, boundary);
            let has_own_line_block = self.has_own_line_block_comments_in_bracket_list(
                Span::new(arr.span.start, boundary),
                &non_null,
                |elem| elem.span(),
            );
            (has_line_comments, has_multiline_block, has_own_line_block)
        } else {
            (false, false, false)
        };

        if has_line_comments || has_multiline_block || has_own_line_block {
            self.build_expanded_array_pattern_doc(arr)
        } else {
            self.build_grouped_array_pattern_doc(arr, has_comments)
        }
    }

    /// Build doc for empty array pattern: `[]` with optional type annotation
    fn build_empty_array_pattern_doc(&self, arr: &internal::ArrayPattern<'_>) -> DocId {
        let d = self.d();
        // For array patterns with type annotations, the body ends before the annotation
        let body_end = arr
            .type_annotation
            .as_ref()
            .map_or(arr.span.end, |t| t.span.start);

        let body_doc =
            self.build_empty_brackets_inline_with_comments_doc_range(arr.span.start, body_end);

        let tail =
            self.build_pattern_optional_type_tail(arr.optional, arr.type_annotation.as_ref());
        d.concat(&[body_doc, tail])
    }

    /// Build grouped array pattern doc (width-based expansion). `has_comments` is the
    /// caller's whole-pattern comment pre-check (threaded to avoid re-scanning): when
    /// false, the per-element leading gap is empty, so the `empty()` leading child is
    /// skipped (render + fits would still walk it). Byte-identical.
    fn build_grouped_array_pattern_doc(
        &self,
        arr: &internal::ArrayPattern<'_>,
        has_comments: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = d.pooled_docbuf();
        let mut prev_end = arr.span.start + 1;

        for (i, elem) in arr.elements.iter().enumerate() {
            let is_last = i == arr.elements.len() - 1;

            if let Some(e) = elem {
                // Check for leading comments before this element
                let elem_start = e.span().start;
                if has_comments {
                    let leading_comments =
                        self.build_inline_comments_between_doc_trailing_space(prev_end, elem_start);
                    parts.push(leading_comments);
                }

                parts.push(self.build_expression_doc(e));

                let elem_end = e.span().end;

                // Collect trailing comments (stop at next element or type annotation).
                // The annotation link is load-bearing: an `ArrayPattern`'s span swallows its
                // `: T` tail, so without it the last element's trailing range runs past the
                // `]` and claims a comment the annotation's own doc also prints.
                let upper_bound = arr
                    .elements
                    .get(i + 1)
                    .and_then(|opt| opt.as_ref().map(|e| e.span().start))
                    .or_else(|| arr.type_annotation.as_ref().map(|t| t.span.start))
                    .unwrap_or(arr.span.end);
                let trailing = self.collect_trailing_comments(elem_end, upper_bound, is_last);

                // Block comments around the comma (line comments force the expanded
                // path, so `trailing.line` is empty here). Separator comma between
                // elements; no trailing comma on the last (trailingComma: 'none').
                let comma = if !is_last { d.text(",") } else { d.empty() };
                self.push_element_comma_trailing(&mut parts, &trailing, comma);
                if !is_last {
                    parts.push(d.line());
                }

                prev_end = trailing.end_pos;
            } else {
                // A hole's comma is syntactically significant and is ALWAYS emitted,
                // the last slot included: a trailing elision is a destructuring slot,
                // and dropping its comma turns `[a, ,]` into `[a, ]` — one binding
                // fewer, so the iterator advances one step fewer. The expanded twin
                // already emits it unconditionally; only this one asked `is_last`, and
                // the result was not even a fixed point (each pass ate another hole).
                parts.push(d.text(","));
                if !is_last {
                    parts.push(d.line());
                }
            }
        }

        // The trailing-elision region: a comment past the last binding has no next element
        // to lead, so it slides to the pattern's own trailing position — the array
        // literal's rule (`build_array_group_doc`'s `has_trailing_elision` arm), and this
        // is its only emitter, since no hole slot prints an element. `prev_end` is the last
        // binding's claim end (just inside `[` when there is no binding at all), so the two
        // sides partition the gap. A line comment routes to the expanded path, so only
        // blocks reach here.
        if has_comments && arr.elements.last().is_some_and(Option::is_none) {
            let boundary = arr
                .type_annotation
                .as_ref()
                .map_or(arr.span.end, |t| t.span.start);
            for comment in comments_to_emit_in_range(self.comments, prev_end, boundary) {
                if comment.is_block {
                    parts.push(self.build_comment_doc(comment));
                }
            }
        }

        // Build group for the array pattern brackets only
        // Type annotation is OUTSIDE the group so it breaks independently.
        // This ensures `[a, b]: [long_tuple]` breaks the tuple type, not the pattern.
        let group_parts: DocBuf = smallvec![
            d.text("["),
            d.indent_softline(d.concat(&parts)),
            d.softline(),
            d.text("]"),
        ];

        let group_doc = d.group(d.concat(&group_parts));

        let tail =
            self.build_pattern_optional_type_tail(arr.optional, arr.type_annotation.as_ref());
        d.concat(&[group_doc, tail])
    }

    /// Build expanded array pattern doc (always multiline)
    fn build_expanded_array_pattern_doc(&self, arr: &internal::ArrayPattern<'_>) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        let mut prev_end = arr.span.start + 1;

        // A comment trailing the opening `[` on its own line is kept on the `[`
        // line when the pattern expands (divergence from prettier, which relocates
        // it to its own line as the first element's leading comment). See
        // conformance_prettier_ts_comments.md §Comment relocation (Array destructuring `[`).
        // Only applies when the first element is present (a leading hole has no
        // span to anchor the range); otherwise the existing path handles comments.
        let (bracket_line_prefix, bracket_pull_pos) =
            match arr.elements.first().and_then(|opt| opt.as_ref()) {
                Some(first) => {
                    self.delimiter_line_comment_prefix(arr.span.start, first.span().start)
                }
                None => (DocBuf::new(), None),
            };

        for (i, elem) in arr.elements.iter().enumerate() {
            let is_last = i == arr.elements.len() - 1;

            if let Some(e) = elem {
                // Check for leading comments before this element (with blank line preservation)
                let elem_start = e.span().start;
                let leading_comments = self.collect_item_leading_comments(
                    prev_end,
                    elem_start,
                    (i == 0).then_some(bracket_pull_pos).flatten(),
                );
                // The element's leading run and the element form one group — see
                // `build_list_element_group` for why (prettier routes `ArrayPattern`
                // through the same `printArray` as an array literal). An own-line
                // format-ignore directive in the element's gap freezes it verbatim
                // (Rule A) — the array literal's expanding printer does the same, and
                // either spelling of an own-line comment routes the pattern here.
                let element_doc =
                    match self.element_frozen_span(arr.span.start + 1, arr.elements, i) {
                        Some(frozen) => self.build_frozen_arg_doc(e, frozen),
                        None => self.build_expression_doc(e),
                    };
                parts.push(self.build_list_element_group_from_comments(
                    leading_comments.iter().copied(),
                    elem_start,
                    element_doc,
                ));

                let elem_end = e.span().end;

                // Collect trailing comments (stop at next element or type annotation).
                // The annotation link is load-bearing: an `ArrayPattern`'s span swallows its
                // `: T` tail, so without it the last element's trailing range runs past the
                // `]` and claims a comment the annotation's own doc also prints.
                let upper_bound = arr
                    .elements
                    .get(i + 1)
                    .and_then(|opt| opt.as_ref().map(|e| e.span().start))
                    .or_else(|| arr.type_annotation.as_ref().map(|t| t.span.start))
                    .unwrap_or(arr.span.end);
                let trailing = self.collect_trailing_comments(elem_end, upper_bound, is_last);

                // Separator comma between elements; no trailing comma on the last
                // element under `trailingComma: 'none'` (a rest element never takes one
                // either — it is a syntax error there).
                let comma = if !is_last { d.text(",") } else { d.empty() };
                self.push_element_comma_trailing(&mut parts, &trailing, comma);

                if !is_last {
                    // Check for blank line before next element (or its leading comment)
                    let next_elem = arr.elements.get(i + 1).and_then(|opt| opt.as_ref());
                    if let Some(next) = next_elem {
                        let next_start = next.span().start;
                        // **in source**: bounds a raw blank-line scan (see `blank_scan_end`).
                        let check_pos = self
                            .comments_in_source_between(trailing.end_pos, next_start)
                            .next()
                            .map_or(next_start, |c| c.span.start);
                        if self.has_blank_line_between(trailing.end_pos, check_pos) {
                            parts.push(d.literalline());
                        }
                    }
                    parts.push(d.hardline());
                }

                prev_end = trailing.end_pos;
            } else {
                // Hole in array pattern
                parts.push(d.text(","));
                if !is_last {
                    parts.push(d.hardline());
                }
            }
        }

        // Check for dangling comments after the last element (before `]`)
        let boundary = arr
            .type_annotation
            .as_ref()
            .map_or(arr.span.end, |t| t.span.start);
        let past_elision = arr.elements.last().is_some_and(Option::is_none);
        parts.push(self.build_pattern_trailing_dangling_comments(prev_end, boundary, past_elision));

        // Structure: [ + bracket-line prefix + indent(hardline + elements) + hardline + ] + type_annotation
        let mut result_parts: DocBuf = smallvec![
            d.text("["),
            d.concat(&bracket_line_prefix),
            d.indent(d.concat(&[d.hardline(), d.concat(&parts)])),
            d.hardline(),
            d.text("]"),
        ];

        result_parts.push(
            self.build_pattern_optional_type_tail(arr.optional, arr.type_annotation.as_ref()),
        );

        d.concat(&result_parts)
    }

    /// Build a Doc for an assignment pattern
    pub(super) fn build_assignment_pattern_doc(
        &self,
        pattern: &internal::AssignmentPattern<'_>,
    ) -> DocId {
        let d = self.d();
        // A type-assertion target keeps its required parens (`{ a: (b as T) =
        // 1 }` — bare `b as T = 1` is a TS parse error); non-null `b!` stays
        // bare. Same rule as an assignment expression's left.
        let left_doc = if self.needs_parens(pattern.left, ParenContext::AssignmentTarget) {
            d.parens(self.build_expression_doc(pattern.left))
        } else if let Expression::ObjectPattern(obj) = pattern.left {
            // A destructuring default's left object pattern does NOT expand on nesting
            // — prettier's shouldBreak excludes an AssignmentPattern parent (object.js),
            // so `{ a: { b } = {} }` (and deeper) stays inline. Width-based breaking
            // still applies. (Array-pattern lefts don't expand on nesting.)
            self.build_object_pattern_doc_with_context(obj, PatternContext::AssignmentDefault)
        } else {
            self.build_expression_doc(pattern.left)
        };

        let left_end = pattern.left.span().end;
        let rhs_start = pattern.right.span().start;
        // Zero-comment fast gate: one binary search over the whole `left = right`
        // gap. On the common comment-free default the `=`-locating byte scan and
        // both per-side comment probes are skipped and it renders as `left = right`
        // (this builder is also the function-param default path `f(a = 1)`, so the
        // gate fires broadly). Canonical reference: build_params_doc_with_comments.
        let gap_has_comments = self.has_comments_on_page_between(left_end, rhs_start);
        let eq_pos = if gap_has_comments {
            self.find_equals_position(left_end, rhs_start)
        } else {
            rhs_start
        };

        // Comment(s) before `=` (e.g., `{a /* c */ = 1}`): a glued block stays inline;
        // a line comment — or a multiline block the author broke after — trails the
        // left and drops `= value` to a continuation line indented one level (uniform
        // forced-continuation indent), so the `=` can't be swallowed (`a // c⏎= 1`).
        // tsv preserves the authored position — prettier relocates the comment to
        // trail the whole binding. The shorthand default's key→`=` gap is the same
        // rule on the same seams (`build_object_pattern_property_doc`).
        let mut head: DocBuf = smallvec![left_doc];
        let mut tail: DocBuf = DocBuf::new();
        let pre_eq_hang = gap_has_comments
            .then(|| self.route_pre_separator_gap(&mut head, left_end, eq_pos))
            .flatten();

        // The `=`→value head: an own-line directive there freezes the whole default.
        // The directive also keeps its own line, unlike the ordinary own-line comment
        // below — the trailing placement that emitter produces is inert under the
        // placement floor, so following it would lose the freeze on the second pass.
        // Same rule, same reason, as at the declaration headers.
        //
        // Rides the zero-comment gate above: a directive is a comment, so a comment-free
        // gap provably holds none — and `eq_pos` is a real `=` position only there.
        let frozen = gap_has_comments
            .then(|| self.value_head_frozen_span(eq_pos + 1, pattern.right.span()))
            .flatten();
        if let Some(frozen) = frozen {
            let frozen_doc =
                self.build_frozen_value_doc(pattern.right, frozen, ParenContext::DefaultValue);
            let rhs = match self.build_rhs_comments_opt(eq_pos + 1, rhs_start) {
                Some(comments_doc) => d.concat(&[comments_doc, frozen_doc]),
                None => frozen_doc,
            };
            tail.push(d.text(if pre_eq_hang.is_some() { "=" } else { " =" }));
            tail.push(hang_after_operator(d, rhs));
        } else {
            // A block comment after `=` inlines onto the value (`a = /* c */ b`),
            // collapsing an own-line authoring; a line comment forces the value onto
            // the next line (it can't share the `//` line). Prettier instead moves a
            // block before `=` (`a /* c */ = b`) or floats a line past the value
            // (`a = b // c`) — see param_default_*_comment_prettier_divergence.
            let rhs_doc = self.build_expression_doc(pattern.right);
            let rhs_doc = if self.needs_parens(pattern.right, ParenContext::DefaultValue) {
                d.parens(rhs_doc)
            } else {
                rhs_doc
            };
            let value_doc =
                if gap_has_comments && self.has_comments_to_emit_between(eq_pos + 1, rhs_start) {
                    self.build_pattern_value_gap_doc(eq_pos + 1, rhs_start, rhs_doc)
                } else {
                    rhs_doc
                };

            tail.push(d.text(if pre_eq_hang.is_some() { "= " } else { " = " }));
            tail.push(value_doc);
        }
        let tail_doc = d.concat(&tail);
        // A hanging pre-`=` comment left `= value` on its own line; the shared
        // continuation seam emits the comment run and indents the tail so it reads as
        // this binding's continuation, not a sibling element.
        match pre_eq_hang {
            Some((start, end)) => head.push(self.build_continuation_indent(start, end, tail_doc)),
            None => head.push(tail_doc),
        }
        d.concat(&head)
    }

    /// Build a Doc for a rest element
    pub(super) fn build_rest_element_doc(&self, rest: &internal::RestElement<'_>) -> DocId {
        let d = self.d();
        // Comments between `...` and the argument (e.g., `.../* c */ args`). Skipped on
        // the comment-free path — rest/spread elements are everywhere (`f(...xs)`,
        // `{a, ...rest}`, `(...args) =>`) and a comment inside the `...` gap is not.
        let dots_end = rest.span.start + "...".len() as u32;
        let arg_start = rest.argument.span().start;
        let mut parts: DocBuf = smallvec![d.text("...")];
        if let Some(comments_doc) =
            self.build_inline_comments_between_doc_trailing_space_opt(dots_end, arg_start)
        {
            parts.push(comments_doc);
        }
        parts.push(self.build_expression_doc(rest.argument));
        // The optional `?` marker and the `: type` annotation, with the comment landings
        // both of their gaps need — `wrap` is false to match the value-side identifier
        // path this mirrors (`build_identifier_doc_inner`). See
        // `push_rest_element_tail_doc`, shared with the type-side rest arm
        // (`types/function_types.rs`).
        self.push_rest_element_tail_doc(&mut parts, rest, false);
        d.concat(&parts)
    }
}
