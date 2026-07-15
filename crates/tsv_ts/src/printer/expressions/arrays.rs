// Array expression printing for TypeScript
//
// Handles printing of array expressions with:
// - Width-based wrapping
// - Fill mode for number-only arrays
// - Forced expansion for multiline content
// - Comment preservation

use crate::ast::internal::{self, Expression, LiteralValue};
use crate::printer::{CommentVec, Printer, has_multiline_content};
use smallvec::{SmallVec, smallvec};
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::{comments_to_emit_in_range, has_multiline_block_comments_on_page_in_range};

impl<'a> Printer<'a> {
    /// Check if array should force break based on Prettier's heuristic
    ///
    /// Returns true when:
    /// - More than 1 element
    /// - ALL elements are arrays (or ALL are objects - no mixing)
    /// - EACH inner array/object has more than 1 item
    ///
    /// This matches prettier's shouldBreak logic in array.js:89-106
    fn should_break_nested_array(&self, arr: &internal::ArrayExpression<'_>) -> bool {
        if arr.elements.len() <= 1 {
            return false;
        }

        let mut expect_arrays: Option<bool> = None;

        for elem in arr.elements {
            let Some(expr) = elem else { return false };

            let (is_array, inner_len) = match expr {
                Expression::ArrayExpression(inner) => (true, inner.elements.len()),
                Expression::ObjectExpression(inner) => (false, inner.properties.len()),
                _ => return false,
            };

            // All elements must be same type (all arrays or all objects)
            if expect_arrays.is_some_and(|expected| expected != is_array) {
                return false;
            }
            expect_arrays = Some(is_array);

            // Each inner must have more than 1 item
            if inner_len <= 1 {
                return false;
            }
        }

        true
    }

    /// Calculate the boundary position for the next element (or array end)
    ///
    /// Used to find the range for trailing comments after an element.
    /// Returns the start position of the next element, or the closing bracket if this is the last element.
    fn next_element_boundary(
        &self,
        arr: &internal::ArrayExpression<'_>,
        current_index: usize,
    ) -> u32 {
        arr.elements[current_index + 1..]
            .iter()
            .find_map(|e| e.as_ref().map(|e| e.span().start))
            .unwrap_or(arr.span.end - 1)
    }

    /// Blank-line rule for the gap after array slot `i` — prettier's
    /// `node && isLineAfterElementEmpty(node)` (`print/array.js`, `printArrayElements`).
    ///
    /// Both terms are load-bearing for elisions:
    ///
    /// - A **hole carries no blank line after it** (prettier's `node &&`): it has no node
    ///   to anchor the scan on, and its own line break is structure, not authorship.
    /// - The scan stops at **slot `i + 1`**, not at the next real element. A hole is empty
    ///   — its slot is the point just before the comma that terminates it — so when slot
    ///   `i + 1` is a hole the next real element lies past that comma, and a scan running
    ///   that far reads the hole's own line break as an author's blank line.
    ///
    /// The scan stops where slot `i + 1`'s printed content begins, so a blank line the
    /// author left ahead of that content is inside the measured range. `upper_override`
    /// supplies that position for a caller that already knows it — the expanding printer,
    /// which stops at slot `i + 1`'s first leading comment. With `None` the position is
    /// derived here, and it is **not** simply the next element's start: an OWNED comment
    /// prints ahead of the element's first token from inside its own doc, so the content
    /// begins at the comment. Bounding past it puts the author's blank line — which lies
    /// *before* the comment — outside the range, where it is silently dropped.
    ///
    /// The derivation lives here rather than at the call sites because a glued block
    /// comment is not itself an expansion trigger (it is neither a line, a multi-line, nor
    /// an own-line comment), so it reaches the width-wrapping and multiline-content
    /// printers too — every caller needs this, and one that forgets it loses blank lines
    /// silently.
    ///
    /// The hole guard runs first, so the scan always anchors on a real element's end —
    /// which also keeps a nested element's commas (`[[1, 2], , x]`) behind it.
    fn has_blank_line_after_slot(
        &self,
        arr: &internal::ArrayExpression<'_>,
        i: usize,
        upper_override: Option<u32>,
    ) -> bool {
        let Some(elem) = arr.elements[i].as_ref() else {
            return false;
        };
        let elem_end = elem.span().end;
        let upper = upper_override.unwrap_or_else(|| {
            // The next real element's start, or the closing `]` — slot `i + 1`'s own
            // boundary unless a hole sits in front of it.
            let next_real = self.next_element_boundary(arr, i);
            if matches!(arr.elements.get(i + 1), Some(None)) {
                // A hole terminates at its own comma: the second past this element, the
                // first being this element's own separator.
                return self
                    .find_comma_in_range(elem_end, next_real)
                    .and_then(|comma| self.find_comma_in_range(comma + 1, next_real))
                    .unwrap_or(next_real);
            }
            // Slot `i + 1`'s owned comment, when it has one — guarded to this gap, since
            // the lookup is keyed on the element's own span start.
            arr.elements
                .get(i + 1)
                .and_then(|e| e.as_ref())
                .and_then(|e| self.owned_leading_comment_start(e))
                .filter(|&p| p > elem_end && p < next_real)
                .unwrap_or(next_real)
        });
        self.has_blank_line_after_comma(elem_end, upper)
    }

    /// Format a block comment for inline use (with appropriate spacing)
    ///
    /// - `leading: true` for comments before elements → space after: `/*c*/ elem`
    /// - `leading: false` for comments after elements → space before: `elem /*c*/`
    fn format_inline_block_comment(&self, comment: &tsv_lang::Comment, leading: bool) -> DocId {
        let d = self.d();
        // One text node either way (the full span is the verbatim `/*content*/`,
        // delimiters included) — array fill items must not gain a separate
        // space node.
        let mut w = d.pool_writer();
        if leading {
            w.push_str(comment.span.extract(self.source));
            w.push(' ');
        } else {
            w.push(' ');
            w.push_str(comment.span.extract(self.source));
        }
        let doc = w.finish_text();
        // A comment emission that can't route through `build_comment_doc` (the space must
        // share the node), so it tags its own ledger node.
        #[cfg(feature = "comment_check")]
        d.tag_comment_doc(doc, comment.span, self.source);
        doc
    }

    /// The last real element before slot `i` — its end position and its index.
    ///
    /// A hole has no span, so this is the only anchor a source scan across earlier slots can
    /// start from. When every earlier slot is a hole the anchor is just inside `[`, paired
    /// with index 0 so a comma count from it still measures whole slots.
    fn prev_real_element_end(&self, arr: &internal::ArrayExpression<'_>, i: usize) -> (u32, usize) {
        arr.elements[..i]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(idx, e)| e.as_ref().map(|e| (e.span().end, idx)))
            .unwrap_or((arr.span.start + 1, 0))
    }

    /// Does this **block** comment trail the element before it, rather than lead the one after?
    /// The separator decides, and is the whole rule: before the comma the comment trails
    /// (`[A /* c */, B]`); past it, it leads the next element (`[A, /* c */ B]`).
    ///
    /// A newline after the comment does **not** carry it across the comma. Prettier classifies
    /// on newlines alone (`endOfLine`, `main/comments/attach.js` — a comma is not a node) and so
    /// rewrites `[A, /* c */⏎B]` to `[A /* c */, B]`, flipping the binding from `B` to `A`; tsv
    /// preserves the authored position. The comma is what carries the association, and unlike a
    /// `//` — which runs to end-of-line, so trailing it past the comma is the only rendering
    /// that exists (the sanctioned pure-separator trail) — a block comment renders fine either
    /// side, making the move unforced. See conformance_prettier.md §Comment relocation.
    ///
    /// `comma_pos` is the separator after `prev_end`. An own-line comment (a newline *before*
    /// it) forces the expanding path, so it never reaches here.
    fn block_comment_trails_prev_element(
        &self,
        prev_end: u32,
        comment: &tsv_lang::Comment,
        comma_pos: Option<u32>,
    ) -> bool {
        self.is_same_line(prev_end, comment.span.start)
            && comma_pos.is_none_or(|pos| comment.span.start < pos)
    }

    /// Emit block comments in `[search_start, elem_start)` as inline-leading
    /// (`/*c*/ elem`). Used by both the first-element and subsequent-element
    /// paths in the non-expanding array printers.
    ///
    /// No trailing-side filter is needed: `search_start` is already past slot `i - 1`'s
    /// comma ([`Self::leading_comment_search_start_for`]), and a block comment trails the
    /// previous element only from *before* that comma — so this range holds none of them,
    /// and the trailing emitter's own comma test excludes exactly what this one keeps.
    fn add_inline_leading_block_comments(
        &self,
        search_start: u32,
        elem_start: u32,
        parts: &mut DocBuf,
    ) {
        for comment in comments_to_emit_in_range(self.comments, search_start, elem_start) {
            if comment.is_block {
                parts.push(self.format_inline_block_comment(comment, true));
            }
        }
    }

    /// Where to start searching for the array element at `i`'s leading comments — just past
    /// the comma that terminates slot `i - 1`. `elem_start` is that element's own start, which
    /// bounds the comma scan (only a real element collects leading comments, so there is
    /// always one to bound it).
    ///
    /// A hole has no span to anchor on, so when holes sit before `i` that comma is located by
    /// **counting** from the last real element: its own separator is the first comma past the
    /// anchor, and each hole after it adds one more. Anchoring on the array's first comma
    /// instead would start the search before earlier elements and re-collect their comments.
    fn leading_comment_search_start_for(
        &self,
        arr: &internal::ArrayExpression<'_>,
        i: usize,
        elem_start: u32,
    ) -> u32 {
        if i == 0 {
            return arr.span.start + 1;
        }
        let (anchor, r) = self.prev_real_element_end(arr, i);
        let mut pos = anchor;
        for _ in 0..(i - r) {
            let Some(comma) = self.find_comma_in_range(pos, elem_start) else {
                break;
            };
            pos = comma + 1;
        }
        pos
    }

    /// Add trailing block comments for an array element — the ones
    /// [`Self::block_comment_trails_prev_element`] binds to it.
    fn add_trailing_array_comments(
        &self,
        arr: &internal::ArrayExpression<'_>,
        elem_end: u32,
        current_index: usize,
        parts: &mut DocBuf,
    ) {
        let next_boundary = self.next_element_boundary(arr, current_index);
        // Bounded at `next_boundary`: this element's separator, if it has one, lies before the
        // next element. Past the last element there is none — and every candidate comment is
        // in range, so `None` tie-breaks them all the same way a comma beyond the array would.
        let comma_pos = self.find_comma_in_range(elem_end, next_boundary);

        for comment in comments_to_emit_in_range(self.comments, elem_end, next_boundary) {
            if comment.is_block
                && self.block_comment_trails_prev_element(elem_end, comment, comma_pos)
            {
                parts.push(self.format_inline_block_comment(comment, false));
            }
        }
    }

    /// Build a Doc for an array expression, wrapping on width.
    ///
    /// The single entry point for every array position — top-level and nested alike — so
    /// multiline content triggers the same expansion everywhere.
    pub(in crate::printer) fn build_array_doc(&self, arr: &internal::ArrayExpression<'_>) -> DocId {
        if arr.elements.is_empty() {
            return self.build_empty_brackets_inline_with_comments_doc(arr.span);
        }

        // Whole-array comment-presence flag (one binary search over the `[…]` span).
        // A false gate is exact: every per-element comment sub-range — the
        // expanding-comment checks below, and the inline leading/trailing lookups in
        // the fill/group builders — lies within [span.start, span.end], so when the
        // array holds no comment, none can lie in any of them (canonical reference:
        // build_params_doc_with_comments).
        let has_comments = self.has_comments_on_page_between(arr.span.start, arr.span.end);

        // Check for comments that force expansion: line comments (can't be inline),
        // multi-line block comments (contain hardlines that must propagate),
        // or own-line single-line block comments (on a separate line from adjacent tokens).
        // The gate skips all three sub-queries — and sub-query 3's eager element
        // collect — on the comment-free common case.
        let has_expanding_comments = has_comments
            && (self.has_line_comments_between(arr.span.start, arr.span.end)
                || has_multiline_block_comments_on_page_in_range(
                    self.comments,
                    arr.span.start,
                    arr.span.end,
                )
                || self.has_own_line_block_comments_in_array(arr));

        if has_expanding_comments {
            return self.build_array_doc_with_expanding_comments(arr);
        }

        // Check if any element has multiline content (e.g., line continuation strings)
        // Prettier expands arrays containing multiline strings (recursively)
        let has_multiline = arr
            .elements
            .iter()
            .flatten()
            .any(|elem| has_multiline_content(elem, self.source));

        // Check if this is a "numbers-only" array (use fill) vs other (one-per-line)
        let is_numbers_only = self.is_numbers_only_array(arr);

        if has_multiline {
            // Force expansion with hardlines for multiline content
            self.build_array_group_doc_forced(arr)
        } else if is_numbers_only {
            // Use fill for greedy packing of numbers
            self.build_array_fill_doc(arr, has_comments)
        } else {
            // Use group with one-per-line for other content
            self.build_array_group_doc(arr, has_comments)
        }
    }

    /// Check if array contains own-line single-line block comments that force expansion.
    ///
    /// Delegates to the generic `has_own_line_block_comments_in_bracket_list` helper,
    /// filtering out holes (elisions) from the element list.
    fn has_own_line_block_comments_in_array(&self, arr: &internal::ArrayExpression<'_>) -> bool {
        let non_null: SmallVec<[_; 8]> = arr.elements.iter().flatten().collect();
        self.has_own_line_block_comments_in_bracket_list(arr.span, &non_null, |e| e.span())
    }

    /// Check if array contains only numeric literals (for fill behavior)
    fn is_numbers_only_array(&self, arr: &internal::ArrayExpression<'_>) -> bool {
        arr.elements.iter().all(|elem| match elem {
            Some(Expression::Literal(lit)) => {
                matches!(lit.value, LiteralValue::Number(_))
            }
            Some(Expression::UnaryExpression(unary)) => {
                // -1, +1 are also numeric
                matches!(
                    unary.operator,
                    internal::UnaryOperator::Minus | internal::UnaryOperator::Plus
                ) && matches!(
                    unary.argument,
                    Expression::Literal(lit) if matches!(lit.value, LiteralValue::Number(_))
                )
            }
            _ => false,
        })
    }

    /// Build fill doc for numbers-only arrays (greedy packing)
    ///
    /// Includes inline block comments between elements.
    /// Uses binary search to find comments: O(log n + k)
    fn build_array_fill_doc(
        &self,
        arr: &internal::ArrayExpression<'_>,
        has_comments: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        for (i, elem) in arr.elements.iter().enumerate() {
            // Handle comments and element (skip comment collection for elisions)
            if let Some(expr) = elem {
                // Zero-comment fast gate: with no comments anywhere in the array,
                // no comment can lie in this element's leading/trailing gap, so skip
                // the inline block-comment collection (and its comma scan).
                if has_comments {
                    let elem_start = expr.span().start;
                    let search_start = self.leading_comment_search_start_for(arr, i, elem_start);
                    self.add_inline_leading_block_comments(search_start, elem_start, &mut parts);
                }

                parts.push(self.build_arg_expression_doc(expr));

                if has_comments {
                    // Trailing block comments (before comma only)
                    self.add_trailing_array_comments(arr, expr.span().end, i, &mut parts);
                }
            }

            if i < arr.elements.len() - 1 {
                parts.push(d.comma_line());
            }
        }

        let inner = d.concat(&[d.softline(), d.fill(&parts)]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.softline());

        d.group(d.concat(&[d.text("["), indented_content, closing_line, d.text("]")]))
    }

    /// Build group doc for non-numeric arrays (one per line when broken)
    ///
    /// Includes inline block comments between elements.
    /// Uses binary search to find comments: O(log n + k)
    ///
    /// Note: Arrays with expanding comments use build_array_doc_with_expanding_comments instead.
    fn build_array_group_doc(
        &self,
        arr: &internal::ArrayExpression<'_>,
        has_comments: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = d.pooled_docbuf();

        // Check if last element is an elision (requires mandatory trailing comma)
        let has_trailing_elision = arr.elements.last().is_some_and(Option::is_none);

        // Check Prettier's shouldBreak heuristic for nested arrays/objects
        let mut should_break = self.should_break_nested_array(arr);

        for (i, elem) in arr.elements.iter().enumerate() {
            // Handle comments and element (skip comment collection for elisions)
            if let Some(expr) = elem {
                // Zero-comment fast gate: with no comments anywhere in the array, no
                // comment can lie in this element's leading/trailing gap, so skip the
                // inline block-comment collection (and its comma scan). The blank-line
                // detection below is comment-independent and stays outside the gate.
                if has_comments {
                    let elem_start = expr.span().start;
                    let search_start = self.leading_comment_search_start_for(arr, i, elem_start);
                    self.add_inline_leading_block_comments(search_start, elem_start, &mut parts);
                }

                parts.push(self.build_arg_expression_doc(expr));

                if has_comments {
                    // Trailing block comments (before comma only)
                    self.add_trailing_array_comments(arr, expr.span().end, i, &mut parts);
                }
            }

            let is_last = i == arr.elements.len() - 1;
            if !is_last {
                let has_blank_after = self.has_blank_line_after_slot(arr, i, None);

                // Separator comma between elements
                parts.push(d.text(","));

                // Own-line block comments from spread with stripped parens:
                // placed after the comma as siblings in the array.
                if let Some(expr) = elem {
                    let spread_comments = self.spread_own_line_block_comments(expr);
                    if !spread_comments.is_empty() {
                        for comment in &spread_comments {
                            parts.push(d.line());
                            parts.push(self.build_comment_doc(comment));
                        }
                        should_break = true;
                    }
                }

                if has_blank_after {
                    // Blank line preservation: empty line (no indent) then content line (with indent)
                    // Flat mode: just a space (blank line collapses)
                    // Break mode: literalline (empty) + hardline (indented)
                    parts.push(d.if_break(d.concat(&[d.literalline(), d.hardline()]), d.text(" ")));
                } else {
                    parts.push(d.line());
                }
            } else if has_trailing_elision {
                // Trailing comma for elision - MUST be preserved (semantically significant)
                parts.push(d.text(","));

                // Block comments past the trailing-elision comma (e.g., `[, , ,/* c */]`)
                // aren't picked up by add_leading/add_trailing, which only run for real
                // elements. Anchor on the last comma in the array, then emit any block
                // comments after it inline.
                let scan_start = arr
                    .elements
                    .iter()
                    .flatten()
                    .next_back()
                    .map_or(arr.span.start + 1, |e| e.span().end);
                if let Some(lc) = self.find_last_comma_before(scan_start, arr.span.end - 1) {
                    for comment in
                        comments_to_emit_in_range(self.comments, lc + 1, arr.span.end - 1)
                    {
                        if comment.is_block {
                            parts.push(self.build_comment_doc(comment));
                        }
                    }
                }
            }
        }

        // Own-line block comments before the closing bracket, emitted as siblings after the
        // last element and forcing the array to break. Only a spread's stripped-paren
        // comments (which `build_spread_doc` skips) actually reach here: any *other*
        // own-line block comment past the last element lies outside every element span, so
        // `has_own_line_block_comments_in_array` sees it and `build_array_doc` routes the
        // array to the expanding printer before this path runs. The collection stays
        // general — it costs nothing and the spread case shares its shape.
        let mut trailing_own_line_comments: CommentVec<'_> = smallvec![];
        // Same-line block comment past the LAST element's comma — a dangling comment with
        // no element after it to lead. Under `trailingComma: 'none'` the comma it followed
        // is dropped, so it renders directly against the element: `['a', 'b', /* c */]` →
        // `['a', 'b' /* c */]`. Prettier emits the same. Own-line comments are siblings
        // (above); this is not a relocation tsv chose, it is the only position left once
        // the separator the author wrote it against is gone.
        let mut trailing_same_line_after_comma: CommentVec<'_> = smallvec![];
        // Zero-comment fast gate: both lists collect nothing but comments, so with none
        // anywhere in the array the whole scan is a no-op.
        let last_elem_end = has_comments
            .then(|| arr.elements.last().and_then(|e| e.as_ref()))
            .flatten()
            .map(|e| {
                // For spread elements, also check inside the spread span for
                // comments from stripped parens (argument.end to spread.end)
                if let Expression::SpreadElement(spread) = e {
                    let has_inner = self
                        .has_comments_to_emit_between(spread.argument.span().end, spread.span.end);
                    if has_inner {
                        return spread.argument.span().end;
                    }
                }
                e.span().end
            });
        if let Some(search_start) = last_elem_end {
            // Bounded at `]`: under `trailingComma: 'none'` the last element has no
            // separator, so an unbounded probe would scan the rest of the file for a comma
            // that is not this array's. Every candidate is in range, so `None` tie-breaks
            // them all the same way a comma past the array would.
            let comma_pos = self.find_comma_in_range(search_start, arr.span.end - 1);
            for comment in comments_to_emit_in_range(self.comments, search_start, arr.span.end - 1)
            {
                if !comment.is_block {
                    continue;
                }
                if !self.is_same_line(search_start, comment.span.start) {
                    trailing_own_line_comments.push(comment);
                } else if comma_pos.is_some_and(|pos| comment.span.start > pos) {
                    trailing_same_line_after_comma.push(comment);
                }
            }
        }

        let mut inner_parts: DocBuf = smallvec![d.softline(), d.concat(&parts)];
        for comment in &trailing_same_line_after_comma {
            inner_parts.push(d.text(" "));
            inner_parts.push(self.build_comment_doc(comment));
        }
        if !trailing_own_line_comments.is_empty() {
            for comment in &trailing_own_line_comments {
                inner_parts.push(d.line());
                inner_parts.push(self.build_comment_doc(comment));
            }
            should_break = true;
        }

        let inner = d.concat(&inner_parts);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.softline());

        // Build group contents
        let group_contents = d.concat(&[d.text("["), indented_content, closing_line, d.text("]")]);

        // Use group_break() when shouldBreak heuristic matched or spread comments force it.
        // This sets shouldBreak on the GROUP ITSELF rather than using break_parent().
        // The difference: shouldBreak is local to this group, while break_parent()
        // propagates up and forces enclosing groups to break.
        // Prettier uses shouldBreak for this heuristic (array.js lines 89-106, 143).
        if should_break {
            d.group_break(group_contents)
        } else {
            d.group(group_contents)
        }
    }

    /// Build group doc for arrays with multiline content (forced expansion with hardlines)
    fn build_array_group_doc_forced(&self, arr: &internal::ArrayExpression<'_>) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        for (i, elem) in arr.elements.iter().enumerate() {
            if i > 0 {
                parts.push(d.text(","));

                // Check for blank line before this element (preserved when wrapped) — the
                // gap after the previous slot. Use literalline() for the blank line (no
                // trailing whitespace) then hardline() for the indented content line —
                // matches the non-forced path.
                if self.has_blank_line_after_slot(arr, i - 1, None) {
                    parts.push(d.literalline());
                }

                parts.push(d.hardline());
            }

            if let Some(expr) = elem {
                parts.push(self.build_arg_expression_doc(expr));
            }
        }

        // No trailing comma after the last element under `trailingComma: 'none'`, and no
        // trailing comment to place: an own-line block comment before the closing bracket
        // can't reach this path. `build_array_doc` routes an array to
        // `build_array_doc_with_expanding_comments` whenever one is present — a single-line
        // one via `has_own_line_block_comments_in_array` (which returns true for any
        // own-line comment past the last element), a multi-line one via the on-page check.
        let inner = d.concat(&[d.hardline(), d.concat(&parts)]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.hardline());
        d.concat(&[d.text("["), indented_content, closing_line, d.text("]")])
    }

    /// Build a Doc for an array with comments that force expansion.
    ///
    /// Used for arrays containing line comments (can't be inline) or multi-line
    /// block comments (hardlines must propagate). Always expands to multiline.
    fn build_array_doc_with_expanding_comments(
        &self,
        arr: &internal::ArrayExpression<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts = d.pooled_docbuf();

        // A comment trailing the opening `[` on its own line is kept on the `[`
        // line (divergence from prettier, which relocates it to its own line as the
        // first element's leading comment). See conformance_prettier.md §Comment
        // relocation (Array literal `[`).
        let first_elem_start = arr
            .elements
            .iter()
            .flatten()
            .next()
            .map_or(arr.span.end - 1, |e| e.span().start);
        let (bracket_line_prefix, bracket_pull_pos) =
            self.delimiter_line_comment_prefix(arr.span.start, first_elem_start);

        // End of the most recently emitted REAL element. Holes don't advance it;
        // this lets the next real element's leading-comment range walk back across
        // any intervening hole commas to claim the comments between them and the
        // previous real element. Also drives the post-loop trailing-comments scan.
        let mut last_real_emit_end = arr.span.start + 1;

        // End position of the last trailing-on-array comment emitted by the
        // trailing-hole iteration (when present). The post-loop scan starts here
        // to avoid re-emitting those comments.
        let mut trailing_hole_comments_end: Option<u32> = None;

        for (i, elem) in arr.elements.iter().enumerate() {
            // O(remaining elements) — compute once and reuse below.
            let next_boundary = self.next_element_boundary(arr, i);
            let (elem_start, elem_end) = match elem {
                Some(e) => (e.span().start, e.span().end),
                None => (next_boundary, next_boundary),
            };

            // Hole at the LAST element index: its leading comments are trailing
            // on the array as a whole (no future real element to attach to).
            // Collect them and emit inline after the hole's comma below.
            let is_trailing_hole = elem.is_none() && i + 1 == arr.elements.len();

            // For real elements: comments in (last_real_emit_end, elem_start).
            // For a trailing hole: same range, but extended to the closing `]`.
            // Other holes contribute nothing — their comments belong to the next
            // real element's leading-comment range.
            //
            // Filter rule: comments same-line with the previous real element are
            // trailing on it, EXCEPT block comments past its comma, which are
            // leading on this element.
            let leading_upper = if elem.is_some() {
                Some(elem_start)
            } else if is_trailing_hole {
                Some(arr.span.end - 1)
            } else {
                None
            };
            let leading_comments: CommentVec<'_> = if let Some(upper) = leading_upper {
                let prev_comma_pos = (i > 0)
                    .then(|| self.find_comma_in_range(last_real_emit_end, upper))
                    .flatten();
                comments_to_emit_in_range(self.comments, last_real_emit_end, upper)
                    .filter(|c| {
                        // Bracket-line comments pulled onto the `[` line above are
                        // emitted as the prefix, not as leading on the first element.
                        // (Only the first element's gap can be same-line as `[`.)
                        if let Some(dpos) = bracket_pull_pos
                            && self.comment_on_delimiter_line(dpos, c)
                        {
                            return false;
                        }
                        if i > 0 && self.is_same_line(last_real_emit_end, c.span.start) {
                            // The complement of the trailing filter below — same predicate,
                            // so each same-line block comment lands on exactly one side.
                            c.is_block
                                && !self.block_comment_trails_prev_element(
                                    last_real_emit_end,
                                    c,
                                    prev_comma_pos,
                                )
                        } else {
                            true
                        }
                    })
                    .collect()
            } else {
                smallvec![]
            };

            // The separator for the gap BEFORE this slot. It is emitted here, by the slot
            // that follows the gap, because only this slot knows where its own printed
            // content begins — and that position is what the blank-line scan must stop at,
            // so that a blank line the author left ahead of the content is inside the
            // measured range. It is one of:
            //
            // - a leading comment, when this gap emits one;
            // - else the element's OWNED comment, which prints ahead of its first token
            //   from inside the element's doc — invisible to the **to emit** axis the
            //   leading list rides, so a bound taken from that list alone lands past the
            //   comment, and the blank line before it falls outside the range and is lost;
            // - else `None`, letting `has_blank_line_after_slot` take this slot's own
            //   boundary — for a hole its comma, rather than the next real element past it.
            if i > 0 {
                let content_start = leading_comments.first().map(|c| c.span.start).or_else(|| {
                    elem.as_ref()
                        .and_then(|e| self.owned_leading_comment_start(e))
                });
                if self.has_blank_line_after_slot(arr, i - 1, content_start) {
                    parts.push(d.literalline());
                }
                parts.push(d.hardline());
            }

            // Emit leading comments BEFORE real element.
            if elem.is_some() {
                for (ci, comment) in leading_comments.iter().enumerate() {
                    if ci > 0 {
                        let prev_comment_end = leading_comments[ci - 1].span.end;
                        if self.has_blank_line_between(prev_comment_end, comment.span.start) {
                            parts.push(d.literalline());
                            parts.push(d.hardline());
                        }
                    }
                    parts.push(self.build_comment_doc(comment));
                    // If a blank line follows, the next iter's literalline+hardline
                    // handles the separator — emit nothing here.
                    let next_is_separated_by_blank =
                        leading_comments.get(ci + 1).is_some_and(|next| {
                            self.has_blank_line_between(comment.span.end, next.span.start)
                        });
                    if !next_is_separated_by_blank {
                        let inline_block =
                            comment.is_block && self.is_same_line(comment.span.end, elem_start);
                        if inline_block {
                            parts.push(d.text(" "));
                        } else {
                            // Last comment before the element: preserve a blank line
                            // the author left between the comment and the element.
                            if leading_comments.get(ci + 1).is_none()
                                && self.has_blank_line_between(comment.span.end, elem_start)
                            {
                                parts.push(d.literalline());
                            }
                            parts.push(d.hardline());
                        }
                    }
                }
            }

            if let Some(e) = elem {
                parts.push(self.build_arg_expression_doc(e));
            }

            // Same-line trailing comments (real elements only).
            let trailing: CommentVec<'_> = if elem.is_some() {
                let comma_pos = self.find_comma_in_range(elem_end, next_boundary);
                comments_to_emit_in_range(self.comments, elem_end, next_boundary)
                    .filter(|c| self.is_same_line(elem_end, c.span.start))
                    .filter(|c| {
                        if c.is_block {
                            self.block_comment_trails_prev_element(elem_end, c, comma_pos)
                        } else {
                            // A same-line line comment always trails: nothing can follow it
                            // on its line.
                            true
                        }
                    })
                    .collect()
            } else {
                smallvec![]
            };

            for comment in trailing.iter().filter(|c| c.is_block) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }

            // Separator comma between elements; under `trailingComma: 'none'` the last
            // REAL element gets no trailing comma, but a trailing-elision hole keeps its
            // (syntactically significant) comma.
            let is_last = i + 1 == arr.elements.len();
            if !is_last || elem.is_none() {
                parts.push(d.text(","));
            }

            for comment in trailing.iter().filter(|c| !c.is_block) {
                parts.push(self.build_trailing_line_comment_doc(comment));
            }

            // Trailing-hole iter: emit collected trailing-on-array comments inline
            // after this hole's comma. First same-line block comment hugs the comma
            // (no separator); subsequent or own-line comments use hardline.
            if is_trailing_hole && !leading_comments.is_empty() {
                // Source position of the LAST comma before `]` (the comma we just
                // emitted for this hole). Used as the same-line anchor for the
                // first comment.
                let last_comma = self.find_last_comma_before(last_real_emit_end, arr.span.end - 1);

                for (ci, comment) in leading_comments.iter().enumerate() {
                    let same_line_inline = if ci == 0 {
                        comment.is_block
                            && last_comma.is_some_and(|c| self.is_same_line(c, comment.span.start))
                    } else {
                        let prev_comment_end = leading_comments[ci - 1].span.end;
                        comment.is_block
                            && self.is_same_line(prev_comment_end, comment.span.start)
                            && !self.has_blank_line_between(prev_comment_end, comment.span.start)
                    };
                    if same_line_inline {
                        parts.push(self.build_comment_doc(comment));
                    } else {
                        if ci > 0 {
                            let prev_comment_end = leading_comments[ci - 1].span.end;
                            if self.has_blank_line_between(prev_comment_end, comment.span.start) {
                                parts.push(d.literalline());
                            }
                        }
                        parts.push(d.hardline());
                        parts.push(self.build_comment_doc(comment));
                    }
                }
                trailing_hole_comments_end = leading_comments.last().map(|c| c.span.end);
            }

            // No separator is emitted here: the gap after this slot belongs to the slot that
            // follows it, which emits it on the way in. That slot is the only one that knows
            // where its own printed content begins — a fact this end-of-iteration position
            // could only *predict*, by re-deriving the next slot's leading comments before
            // they are collected. Two derivations of one fact drift: the prediction read the
            // **in source** axis while the collection read **to emit**, so a glued (owned)
            // comment bounded one and not the other, and the separator went missing on
            // exactly the arrays that had one.
            if elem.is_some() {
                last_real_emit_end = elem_end;

                // Spread elements with own-line trailing comments from stripped parens:
                // expose them to subsequent leading-comment searches and the final scan.
                if let Some(Expression::SpreadElement(spread)) = elem {
                    let arg_end = spread.argument.span().end;
                    let has_own_line = self
                        .comments_on_page_between(arg_end, spread.span.end)
                        .any(|c| c.is_block && self.has_newline_between(arg_end, c.span.start));
                    if has_own_line {
                        last_real_emit_end = arg_end;
                    }
                }
            }
        }

        // Final comments before closing bracket. Skip what trailing-hole emission
        // already handled.
        let final_scan_start = trailing_hole_comments_end.unwrap_or(last_real_emit_end);
        let mut prev_end = final_scan_start;
        for comment in comments_to_emit_in_range(self.comments, final_scan_start, arr.span.end - 1)
        {
            if !self.is_same_line(final_scan_start, comment.span.start) {
                // Preserve an author blank line before an own-line trailing comment.
                self.push_blank_preserving_hardline(&mut parts, prev_end, comment.span.start);
                parts.push(self.build_comment_doc(comment));
                prev_end = comment.span.end;
            }
        }

        let inner = d.concat(&[d.hardline(), d.concat(&parts)]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.hardline());

        d.concat(&[
            d.text("["),
            d.concat(&bracket_line_prefix),
            indented_content,
            closing_line,
            d.text("]"),
        ])
    }

    /// Build a Doc for an array expression with forced expansion (hardlines).
    ///
    /// Used by chain arg formatting when we need the array to expand internally
    /// with hardlines so fits() can correctly measure the first line.
    /// Produces: `[\n  elem,\n]` with actual hardlines.
    pub(in crate::printer) fn build_array_doc_expanded(
        &self,
        arr: &internal::ArrayExpression<'_>,
    ) -> DocId {
        let d = self.d();
        if arr.elements.is_empty() {
            return d.text("[]");
        }

        let mut parts = DocBuf::new();
        for (i, elem) in arr.elements.iter().enumerate() {
            // Elements are Option<Expression> where None = hole/elision
            if let Some(expr) = elem {
                parts.push(self.build_arg_expression_doc(expr));
            }
            // Holes are represented by just a comma (no element content)

            if i < arr.elements.len() - 1 {
                parts.push(d.text(","));
                parts.push(d.hardline());
            } else if elem.is_none() {
                // Trailing-elision hole keeps its (syntactically significant) comma;
                // a real last element gets no trailing comma under `trailingComma: 'none'`.
                parts.push(d.text(","));
            }
        }

        d.concat(&[
            d.text("["),
            d.indent(d.concat(&[d.hardline(), d.concat(&parts)])),
            d.hardline(),
            d.text("]"),
        ])
    }
}
