// if/else statement printing
//
// Entry point (`build_if_statement_doc`) plus the wrapping and
// comment-handling variants, and else-clause layout helpers.

use super::ControlFlowGap;
use crate::ast::internal::{self, Statement};
use crate::printer::Printer;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::skip_comment;

/// Check if a statement can be printed inline after `if (cond)` without a newline.
///
/// Block, expression, break, continue, return, throw, and empty statements stay inline.
/// Other statements (if, for, while, etc.) go on a new line with indent.
fn is_inline_consequent(stmt: &Statement<'_>) -> bool {
    matches!(
        stmt,
        Statement::BlockStatement(_)
            | Statement::ExpressionStatement(_)
            | Statement::BreakStatement(_)
            | Statement::ContinueStatement(_)
            | Statement::ReturnStatement(_)
            | Statement::ThrowStatement(_)
            | Statement::EmptyStatement(_)
    )
}

/// Check if a statement can be printed inline after `else` without a newline.
///
/// Same as `is_inline_consequent` but also allows IfStatement for else-if chains.
fn is_inline_alternate(stmt: &Statement<'_>) -> bool {
    is_inline_consequent(stmt) || matches!(stmt, Statement::IfStatement(_))
}

impl<'a> Printer<'a> {
    /// Build a non-block, non-inline else body via Prettier's `adjustClause`
    /// (`group(indent([line, clause]))`): `else while (x) g();` stays inline when it fits
    /// and breaks to `else⏎↹while (x) g();` when it doesn't — the same soft-line layout the
    /// consequent uses (see `build_adjust_clause_with_comments`), never a bare hardline. The
    /// caller emits the bare `else` (no trailing space); the leading `line` supplies the
    /// separator when flat.
    fn build_else_adjust_clause(&self, alternate: &Statement<'_>) -> DocId {
        let d = self.d();
        d.group(d.indent_line(self.build_statement_doc(alternate, false)))
    }

    /// Append a **block or inline** else body to parts. A non-block, non-inline alternate
    /// without a forcing comment is emitted by [`Self::build_else_adjust_clause`] at the
    /// call sites instead; here a non-block alternate is always emitted inline (the caller
    /// reaches this only with an inline alternate, or a comment that forces inline layout).
    fn append_else_body_doc(&self, parts: &mut DocBuf, alternate: &Statement<'_>) {
        if let Statement::BlockStatement(block) = alternate {
            parts.push(self.build_block_statement_expand_empty_doc(block));
        } else {
            parts.push(self.build_statement_doc(alternate, false));
        }
    }

    /// Append the `else` keyword and its alternate body, choosing the layout: an inline
    /// alternate (block / expression / else-if) prints `else <body>`; a non-block, non-inline
    /// alternate uses `adjustClause` (`else` + [`Self::build_else_adjust_clause`]) so it stays
    /// inline when it fits and breaks to `else⏎↹clause` otherwise. `leading_space` prefixes
    /// ` else` — set when `else` abuts a preceding `}` on the same line (`} else …`), cleared
    /// when it starts its own line after a `hardline`. (EmptyStatement and comment-bearing
    /// alternates are handled by the callers, not here.)
    fn append_else_keyword_body(
        &self,
        parts: &mut DocBuf,
        alternate: &Statement<'_>,
        leading_space: bool,
    ) {
        let d = self.d();
        if is_inline_alternate(alternate) {
            parts.push(d.text(if leading_space { " else " } else { "else " }));
            self.append_else_body_doc(parts, alternate);
        } else {
            parts.push(d.text(if leading_space { " else" } else { "else" }));
            parts.push(self.build_else_adjust_clause(alternate));
        }
    }

    /// Append `else` clause on a new line for non-block/empty-statement consequent paths.
    ///
    /// Handles EmptyStatement alternate (`else;`) and delegates the block/inline/non-inline
    /// body layout to [`Self::append_else_keyword_body`].
    fn append_newline_else_clause(&self, parts: &mut DocBuf, alternate: &Statement<'_>) {
        let d = self.d();
        parts.push(d.hardline());
        if matches!(alternate, Statement::EmptyStatement(_)) {
            parts.push(d.text("else;"));
        } else {
            self.append_else_keyword_body(parts, alternate, false);
        }
    }

    /// Build else clause with comment extraction between `else` keyword and body.
    ///
    /// Handles block comments staying inline: `} else /* c */ {`
    /// and line comments forcing a break before the body.
    fn build_head_body_else_clause(
        &self,
        parts: &mut DocBuf,
        alternate: &Statement<'_>,
        consequent_end: u32,
    ) {
        let d = self.d();
        let alt_start = alternate.span().start;

        // Find "else" keyword by scanning forward from consequent end, skipping comments
        let else_end = self.find_else_keyword_end_between(consequent_end, alt_start);

        if matches!(alternate, Statement::EmptyStatement(_)) {
            // Empty alternate: `} else;`, `} else /* c */ ;`, or `} else // c\n;`
            if let Some(else_end) = else_end
                && self.has_comments_to_emit_between(else_end, alt_start)
            {
                let has_line = self.has_line_comments_between(else_end, alt_start);
                parts.push(d.text(" else"));
                parts.push(self.build_inline_comments_between_doc(else_end, alt_start));
                if has_line {
                    // Line comment: `} else // c\n;`
                    parts.push(d.hardline());
                    parts.push(d.text(";"));
                } else {
                    // Block comment: `} else /* c */ ;`
                    parts.push(d.text(" ;"));
                }
            } else {
                parts.push(d.text(" else;"));
            }
        } else if let Some(else_end) = else_end
            && self.has_comments_to_emit_between(else_end, alt_start)
        {
            // Comments between `else` and body
            let has_line = self.has_line_comments_between(else_end, alt_start);
            let is_non_block_non_if = !matches!(
                alternate,
                Statement::BlockStatement(_) | Statement::IfStatement(_)
            );
            parts.push(d.text(" else"));
            parts.push(self.build_inline_comments_between_doc(else_end, alt_start));
            if has_line && is_non_block_non_if {
                // Line comment + non-block body: comment stays on else line, body indented
                // } else // c\n\texpr;
                let body_doc = self.build_statement_doc(alternate, false);
                parts.push(d.indent(d.concat(&[d.hardline(), body_doc])));
            } else if has_line {
                parts.push(d.hardline());
                self.append_else_body_doc(parts, alternate);
            } else {
                parts.push(d.text(" "));
                self.append_else_body_doc(parts, alternate);
            }
        } else {
            self.append_else_keyword_body(parts, alternate, true);
        }
    }

    /// Find the end position of the "else" keyword between two positions.
    ///
    /// Scans forward from `from` to `to`, skipping comment content so that
    /// "else" inside comments (e.g., `} else /* or else */ {`) is not matched.
    fn find_else_keyword_end_between(&self, from: u32, to: u32) -> Option<u32> {
        let bytes = self.source.as_bytes();
        let mut i = from as usize;
        let end = to as usize;
        while i + "else".len() <= end {
            if let Some(new_i) = skip_comment(bytes, i, end) {
                // Clamp in case a block comment was unterminated
                i = new_i.min(end);
                continue;
            }
            // `str::get` (not `source[i..i + 4]`): `bytes[i] == b'e'` proves `i` is a char
            // boundary, but `i + "else".len()` is not — an `e` followed within 3 bytes by a
            // multibyte char lands the slice end mid-codepoint and panics. `get` returns
            // `None` there, so a doomed scan over minted multibyte text simply finds no `else`.
            if bytes[i] == b'e' && self.source.get(i..i + "else".len()) == Some("else") {
                return Some((i + "else".len()) as u32);
            }
            i += 1;
        }
        None
    }

    /// Build a doc for an if statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture from estree.js:
    /// ```js
    /// group([
    ///   "if (",
    ///   group([indent([softline, test]), softline]),  // inner group for condition
    ///   ")",
    ///   adjustClause(consequent),  // body handling
    /// ])
    /// ```
    fn build_if_statement_plain_doc(&self, stmt: &internal::IfStatement<'_>) -> DocId {
        let d = self.d();
        // Find paren positions for comment handling
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));

        // Preserve comments between `if` keyword and `(` in place:
        //   if/* c */(a){} → if /* c */ (a) {}
        let if_keyword_end = stmt.span.start + "if".len() as u32;
        let keyword_comments = self.build_keyword_paren_comments(if_keyword_end, open_paren);

        // Build condition group (handles breaking within condition and comments,
        // and the `!(logical)` inline-negation hug).
        let condition_group =
            self.build_statement_condition_doc(&stmt.test, open_paren, close_paren);

        if let Statement::BlockStatement(block) = stmt.consequent {
            // Block consequent: group(["if (" + condition + ") " + block])
            // Outer group controls whether the whole if statement breaks
            let mut parts: DocBuf = DocBuf::new();
            self.push_keyword_open_paren(&mut parts, "if", keyword_comments);
            parts.push(condition_group);

            // Check for comments between ) and block body
            let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
            self.append_close_paren_with_comments(&mut parts, paren_end, block.span.start);

            parts.push(self.build_block_statement_expand_empty_doc(block));

            // Handle else clause
            if let Some(alternate) = &stmt.alternate {
                self.build_head_body_else_clause(&mut parts, alternate, block.span.end);
            }

            // Outer group for the whole if statement
            d.group(d.concat(&parts))
        } else if matches!(stmt.consequent, Statement::EmptyStatement(_)) {
            // Empty statement: `if (cond);` or `if (cond) /* comment */ ;`
            let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
            let empty_start = stmt.consequent.span().start;

            let mut empty_parts: DocBuf = DocBuf::new();
            self.push_keyword_open_paren(&mut empty_parts, "if", keyword_comments);
            empty_parts.push(condition_group);
            self.append_close_paren_empty_stmt_with_comments(
                &mut empty_parts,
                paren_end,
                empty_start,
            );

            // Handle else clause for empty-statement consequent
            if let Some(alternate) = &stmt.alternate {
                self.append_newline_else_clause(&mut empty_parts, alternate);
            }

            d.group(d.concat(&empty_parts))
        } else {
            // Non-block consequent: use adjustClause equivalent
            // Prettier's adjustClause returns: indent([line, clause])
            // - When flat: line becomes space -> `if (cond) a;`
            // - When broken: line becomes newline + indent -> `if (cond)\n\ta;`
            let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
            let body_start = stmt.consequent.span().start;
            let consequent_doc = self.build_statement_doc(stmt.consequent, false);

            let mut head_parts: DocBuf = DocBuf::new();
            self.push_keyword_open_paren(&mut head_parts, "if", keyword_comments);
            head_parts.push(condition_group);
            let head_and_body = self.build_adjust_clause_with_comments(
                &head_parts,
                paren_end,
                body_start,
                consequent_doc,
            );

            let mut parts = smallvec![head_and_body];

            // Handle else clause for non-block consequent
            if let Some(alternate) = &stmt.alternate {
                self.append_newline_else_clause(&mut parts, alternate);
            }

            d.concat(&parts)
        }
    }

    pub(in crate::printer::statements) fn build_if_statement_doc(
        &self,
        stmt: &internal::IfStatement<'_>,
    ) -> DocId {
        // Check for comments between consequent and alternate that need special handling
        let has_if_else_comments = stmt.alternate.as_ref().is_some_and(|alt| {
            let consequent_end = stmt.consequent.span().end;
            let alternate_start = alt.span().start;
            self.has_comments_to_emit_between(consequent_end, alternate_start)
        });

        // A comment in the consequent→alternate gap is the only axis here; both paths
        // group and wrap the condition identically.
        if has_if_else_comments {
            self.build_if_statement_with_comments_doc(stmt)
        } else {
            self.build_if_statement_plain_doc(stmt)
        }
    }

    /// Build if statement doc with comments between consequent and alternate
    fn build_if_statement_with_comments_doc(&self, stmt: &internal::IfStatement<'_>) -> DocId {
        let d = self.d();
        // Build condition group (same as build_if_statement_plain_doc)
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));
        let if_keyword_end = stmt.span.start + "if".len() as u32;
        let keyword_comments = self.build_keyword_paren_comments(if_keyword_end, open_paren);
        let condition_group =
            self.build_statement_condition_doc(&stmt.test, open_paren, close_paren);

        let mut parts: DocBuf = DocBuf::new();
        self.push_keyword_open_paren(&mut parts, "if", keyword_comments);
        parts.push(condition_group);

        // Build consequent (with head-body comment extraction)
        let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
        if let Statement::BlockStatement(block) = stmt.consequent {
            self.append_close_paren_with_comments(&mut parts, paren_end, block.span.start);
            parts.push(self.build_block_statement_expand_empty_doc(block));
        } else if matches!(stmt.consequent, Statement::EmptyStatement(_)) {
            let empty_start = stmt.consequent.span().start;
            self.append_close_paren_empty_stmt_with_comments(&mut parts, paren_end, empty_start);
        } else {
            // Non-block consequent: handle head-body comments between ) and body
            let body_start = stmt.consequent.span().start;
            let consequent_doc = self.build_statement_doc(stmt.consequent, false);

            if self.has_comments_to_emit_between(paren_end, body_start) {
                let has_line = self.has_line_comments_between(paren_end, body_start);
                let comment_doc =
                    self.build_inline_comments_between_doc_no_leading_space(paren_end, body_start);
                parts.push(d.text(")"));
                if has_line {
                    // Line comment forces break: if (cond)\n\t// comment\n\tbody;
                    parts.push(d.indent(d.concat(&[
                        d.hardline(),
                        comment_doc,
                        d.hardline(),
                        consequent_doc,
                    ])));
                } else {
                    // Block comment stays inline: if (cond) /* c */ body;
                    parts.push(d.text(" "));
                    parts.push(comment_doc);
                    parts.push(d.text(" "));
                    parts.push(consequent_doc);
                }
            } else if is_inline_consequent(stmt.consequent) {
                parts.push(d.text(") "));
                parts.push(consequent_doc);
            } else {
                parts.push(d.text(")"));
                parts.push(d.indent(d.concat(&[d.hardline(), consequent_doc])));
            }
        }

        // Handle else with comments
        if let Some(alternate) = &stmt.alternate {
            let consequent_end = stmt.consequent.span().end;
            let alternate_start = alternate.span().start;

            // Find "else" keyword to split comments into before-else and after-else
            let else_end = self.find_else_keyword_end_between(consequent_end, alternate_start);
            let else_start = else_end.map(|e| e - "else".len() as u32);

            // Comments between } and "else"
            let before_else_end = else_start.unwrap_or(alternate_start);
            if self.has_comments_to_emit_between(consequent_end, before_else_end) {
                let (inline_prev, own_line, inline_next) =
                    self.partition_comments_by_line(consequent_end, before_else_end);

                // Merge inline_next (comments on same line as `else`) into own_line
                // so they're emitted before the `else` keyword rather than dropped.
                // e.g. `} \n /* b */ else {` → `}\n/* b */\nelse {`
                let mut all_own_line = own_line;
                all_own_line.extend(inline_next);

                self.build_comments_between_parts(
                    &mut parts,
                    &inline_prev,
                    &all_own_line,
                    consequent_end,
                    ControlFlowGap::BlockToKeyword,
                );

                let has_inline_line_comment = inline_prev.iter().any(|c| !c.is_block);
                let is_block_consequent = matches!(stmt.consequent, Statement::BlockStatement(_));
                if is_block_consequent && all_own_line.is_empty() && !has_inline_line_comment {
                    parts.push(d.text(" "));
                } else {
                    parts.push(d.hardline());
                }
            } else if matches!(stmt.consequent, Statement::BlockStatement(_)) {
                // Block body: `} else` on same line
                parts.push(d.text(" "));
            } else {
                // Empty statement or non-block body: `else` on new line
                parts.push(d.hardline());
            }

            // Comments between "else" and alternate body
            if matches!(alternate, Statement::EmptyStatement(_)) {
                // Empty alternate: `else;`, `else /* c */ ;`, or `else // c\n;`
                if let Some(else_e) = else_end
                    && self.has_comments_to_emit_between(else_e, alternate_start)
                {
                    let has_line = self.has_line_comments_between(else_e, alternate_start);
                    parts.push(d.text("else"));
                    parts.push(self.build_inline_comments_between_doc(else_e, alternate_start));
                    if has_line {
                        // Line comment: `else // c\n;`
                        parts.push(d.hardline());
                        parts.push(d.text(";"));
                    } else {
                        // Block comment: `else /* c */ ;`
                        parts.push(d.text(" ;"));
                    }
                } else {
                    parts.push(d.text("else;"));
                }
            } else if let Some(else_e) = else_end
                && self.has_comments_to_emit_between(else_e, alternate_start)
            {
                let has_line = self.has_line_comments_between(else_e, alternate_start);
                let is_non_block_non_if = !matches!(
                    alternate,
                    Statement::BlockStatement(_) | Statement::IfStatement(_)
                );
                parts.push(d.text("else"));
                parts.push(self.build_inline_comments_between_doc(else_e, alternate_start));
                if has_line && is_non_block_non_if {
                    // Line comment + non-block body: comment stays on else line, body indented
                    // else // c\n\texpr;
                    let body_doc = self.build_statement_doc(alternate, false);
                    parts.push(d.indent(d.concat(&[d.hardline(), body_doc])));
                } else if has_line {
                    parts.push(d.hardline());
                    self.append_else_body_doc(&mut parts, alternate);
                } else {
                    parts.push(d.text(" "));
                    self.append_else_body_doc(&mut parts, alternate);
                }
            } else {
                self.append_else_keyword_body(&mut parts, alternate, false);
            }
        }

        d.concat(&parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PrinterInputs;
    use std::cell::RefCell;
    use std::rc::Rc;
    use string_interner::DefaultStringInterner;
    use tsv_lang::EmbedContext;
    use tsv_lang::doc::arena::DocArena;

    /// A synthetic if/else region (as `tsv_svelte_compile` mints) can bracket arbitrary
    /// multibyte template text. Here an `e` byte is immediately followed by a multibyte
    /// char whose bytes straddle `i + "else".len()`, so an unchecked `source[i..i + 4]`
    /// slice would panic on a non-char-boundary. The `str::get` form must instead find no
    /// `else` and return `None` without panicking (prod WASM is `panic = "abort"`).
    #[test]
    fn find_else_keyword_end_between_multibyte_is_panic_free() {
        // bytes: `e`, `x`, then the 3-byte em-dash `—` at 2,3,4 — so index 4 (the slice
        // end for an `e` at index 0) falls inside the em-dash.
        let source = "ex—";
        let arena = DocArena::new();
        let interner = Rc::new(RefCell::new(DefaultStringInterner::new()));
        let inputs = PrinterInputs {
            source,
            interner,
            comments: &[],
            line_breaks: &[],
            has_owned_comments: false,
            has_format_ignore: false,
        };
        let printer = Printer::with_context(&arena, &inputs, EmbedContext::default(), 0);
        assert_eq!(
            printer.find_else_keyword_end_between(0, source.len() as u32),
            None
        );
    }
}
