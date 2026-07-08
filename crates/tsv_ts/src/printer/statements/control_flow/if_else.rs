// if/else statement printing
//
// Entry point (`build_if_statement_doc`) plus the wrapping and
// comment-handling variants, and else-clause layout helpers.

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
    /// Append an else body to parts, dispatching on statement type.
    ///
    /// When `comment_forced` is true, the layout was already determined by a preceding comment,
    /// so non-block bodies are emitted directly. When false, non-block/non-inline bodies get
    /// indented (Prettier's adjustClause behavior).
    fn append_else_body_doc(
        &self,
        parts: &mut DocBuf,
        alternate: &Statement<'_>,
        comment_forced: bool,
    ) {
        if let Statement::BlockStatement(block) = alternate {
            parts.push(self.build_block_statement_expand_empty_doc(block));
        } else if comment_forced || is_inline_alternate(alternate) {
            parts.push(self.build_statement_doc(alternate));
        } else {
            let d = self.d();
            parts.push(d.indent(d.concat(&[d.hardline(), self.build_statement_doc(alternate)])));
        }
    }

    /// Append `else` clause on a new line for non-block/empty-statement consequent paths.
    ///
    /// Handles EmptyStatement alternate (`else;`), inline alternate (`else expr;`),
    /// block alternate (`else { ... }`), and non-inline alternate (indented).
    fn append_newline_else_clause(&self, parts: &mut DocBuf, alternate: &Statement<'_>) {
        let d = self.d();
        parts.push(d.hardline());
        if matches!(alternate, Statement::EmptyStatement(_)) {
            parts.push(d.text("else;"));
        } else {
            parts.push(d.text("else "));
            if is_inline_alternate(alternate) {
                if let Statement::BlockStatement(block) = alternate {
                    parts.push(self.build_block_statement_expand_empty_doc(block));
                } else {
                    parts.push(self.build_statement_doc(alternate));
                }
            } else {
                parts.push(d.hardline());
                parts.push(d.indent(self.build_statement_doc(alternate)));
            }
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
                && self.has_comments_between(else_end, alt_start)
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
            && self.has_comments_between(else_end, alt_start)
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
                let body_doc = self.build_statement_doc(alternate);
                parts.push(d.indent(d.concat(&[d.hardline(), body_doc])));
            } else if has_line {
                parts.push(d.hardline());
                self.append_else_body_doc(parts, alternate, true);
            } else {
                parts.push(d.text(" "));
                self.append_else_body_doc(parts, alternate, true);
            }
        } else {
            parts.push(d.text(" else "));
            self.append_else_body_doc(parts, alternate, false);
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
            if bytes[i] == b'e' && &self.source[i..i + "else".len()] == "else" {
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
    fn build_if_statement_with_wrapping_doc(&self, stmt: &internal::IfStatement<'_>) -> DocId {
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
            let mut parts = smallvec![d.text("if")];
            if let Some(kc) = keyword_comments {
                parts.push(kc);
            }
            parts.push(d.text(" ("));
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

            let mut empty_parts = smallvec![d.text("if")];
            if let Some(kc) = keyword_comments {
                empty_parts.push(kc);
            }
            empty_parts.push(d.text(" ("));
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
            let consequent_doc = self.build_statement_doc(stmt.consequent);

            let mut head_parts: DocBuf = smallvec![d.text("if")];
            if let Some(kc) = keyword_comments {
                head_parts.push(kc);
            }
            head_parts.push(d.text(" ("));
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
            self.has_comments_between(consequent_end, alternate_start)
        });

        if has_if_else_comments {
            // Build doc with inline comments between } and else
            self.build_if_statement_with_comments_doc(stmt)
        } else {
            // Delegate to the sophisticated version that handles width-based wrapping
            self.build_if_statement_with_wrapping_doc(stmt)
        }
    }

    /// Build if statement doc with comments between consequent and alternate
    fn build_if_statement_with_comments_doc(&self, stmt: &internal::IfStatement<'_>) -> DocId {
        let d = self.d();
        // Build condition group (same as build_if_statement_with_wrapping_doc)
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));
        let if_keyword_end = stmt.span.start + "if".len() as u32;
        let keyword_comments = self.build_keyword_paren_comments(if_keyword_end, open_paren);
        let condition_group =
            self.build_statement_condition_doc(&stmt.test, open_paren, close_paren);

        let mut parts = smallvec![d.text("if")];
        if let Some(kc) = keyword_comments {
            parts.push(kc);
        }
        parts.push(d.text(" ("));
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
            let consequent_doc = self.build_statement_doc(stmt.consequent);

            if self.has_comments_between(paren_end, body_start) {
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
            if self.has_comments_between(consequent_end, before_else_end) {
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
                    && self.has_comments_between(else_e, alternate_start)
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
                && self.has_comments_between(else_e, alternate_start)
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
                    let body_doc = self.build_statement_doc(alternate);
                    parts.push(d.indent(d.concat(&[d.hardline(), body_doc])));
                } else if has_line {
                    parts.push(d.hardline());
                    self.append_else_body_doc(&mut parts, alternate, true);
                } else {
                    parts.push(d.text(" "));
                    self.append_else_body_doc(&mut parts, alternate, true);
                }
            } else {
                parts.push(d.text("else "));
                self.append_else_body_doc(&mut parts, alternate, false);
            }
        }

        d.concat(&parts)
    }
}
