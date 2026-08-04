// Template tag parsing
//
// Handles: {@html}, {@const}, {@debug}, {@render} tags and {const}/{let} declaration tags

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use crate::whitespace::is_svelte_ws;
use tsv_lang::source_scan::{TriviaProfile, skip_comment, skip_trivia};
use tsv_lang::{ParseError, Span};
use tsv_ts::Expression;

use super::parser_impl::SvelteParser;
use super::subslice_offset;

impl<'a, 'arena> SvelteParser<'a, 'arena> {
    /// Parse a template tag starting with {@
    ///
    /// Dispatches to specific tag parsers based on the keyword.
    pub(crate) fn parse_template_tag(&mut self) -> Result<FragmentNode<'arena>, ParseError> {
        let start = self.current_start;

        // We're at {@, consume it
        if !self.check(TokenKind::TagOpen) {
            return Err(self.error_expected_found("'{@'"));
        }

        // After {@ we expect a tag keyword: html, const, debug, render
        let keyword = self.keyword_at(self.current_end);

        match keyword {
            "html" => self.parse_html_tag(start),
            "const" => self.parse_const_tag(start),
            "debug" => self.parse_debug_tag(start),
            "render" => self.parse_render_tag(start),
            _ => Err(self.error_unknown_at("template tag", &format!("{{@{keyword}}}"), start)),
        }
    }

    /// Parse a keyword-prefixed single-expression tag — `{@html expr}` /
    /// `{@render expr}`. Both require whitespace after the keyword
    /// (`strip_block_keyword`), parse the remaining content as one TS expression
    /// (which collects comments and enforces end-of-input), and span the whole
    /// `{@…}`. The caller wraps the `(expression, span)` in its node type.
    fn parse_keyword_expression_tag(
        &mut self,
        start: usize,
        keyword: &str,
    ) -> Result<(Expression<'arena>, Span), ParseError> {
        let tag_content_start = self.current_end;
        let (tag_content, after_close) = self.scan_block_tag_content(tag_content_start)?;

        // Svelte requires whitespace after the keyword.
        let expr_str = self
            .strip_block_keyword(tag_content, keyword, tag_content_start)?
            .trim();

        let expr_offset = tag_content_start + subslice_offset(tag_content, expr_str);
        let expression = self.parse_ts_expression(expr_str, expr_offset)?;

        // Span runs from `{@` (start) to just after the closing `}` (after_close).
        let span = Span {
            start: start as u32,
            end: after_close as u32,
        };
        Ok((expression, span))
    }

    /// Parse an html tag: {@html expression}
    fn parse_html_tag(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        let (expression, span) = self.parse_keyword_expression_tag(start, "html")?;
        Ok(FragmentNode::HtmlTag(HtmlTag { expression, span }))
    }

    /// Parse a const tag: {@const name = expression}
    fn parse_const_tag(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        let tag_content_start = self.current_end;
        let (tag_content, after_close) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "const name = expression" — Svelte requires whitespace after the keyword.
        let decl_str = self
            .strip_block_keyword(tag_content, "const", tag_content_start)?
            .trim();

        let decl_offset = tag_content_start + subslice_offset(tag_content, decl_str);

        // `{@const}` must be a single declarator (unlike the bare `{const}`/`{let}`
        // tags) — Svelte rejects `{@const a = 1, b = 2}`. A top-level `,` is the
        // multi-declarator signal; a `,` inside a comment or string is not.
        if super::find_top_level_delim(
            decl_str.as_bytes(),
            0,
            decl_str.len(),
            b',',
            TriviaProfile::JS,
        )
        .is_some()
        {
            return Err(self.error_msg_at(
                "{@const ...} must consist of a single variable declaration",
                decl_offset,
            ));
        }

        // Find the top-level `=` (not the nested `=` of a destructuring default)
        // separating id from init, then parse both sides.
        let eq_pos = self.find_top_level_equals(decl_str)?;
        let (id, init) = self.parse_declarator(decl_str, decl_offset, eq_pos)?;

        Ok(FragmentNode::ConstTag(ConstTag {
            id,
            init,
            span: Span {
                start: start as u32,
                end: after_close as u32,
            },
        }))
    }

    /// When positioned at a `{` (`LeftBrace`), detect whether it opens a
    /// `{const …}` / `{let …}` declaration tag rather than an ordinary `{expr}`
    /// mustache. Leading whitespace after `{` is skipped to match Svelte's
    /// `allow_whitespace`. Only the exact keywords `const`/`let` match (the
    /// alphabetic-run read gives the `\b` word boundary for free, so identifiers
    /// like `constant`/`letter` and expressions like `cond ? …` fall through).
    pub(crate) fn opens_declaration_tag(&self) -> bool {
        let after_brace = self.current_end;
        let rest = &self.source[after_brace..];
        let kw_start = after_brace + (rest.len() - rest.trim_start().len());
        matches!(self.keyword_at(kw_start), "const" | "let")
    }

    /// At a `{` (`LeftBrace`), parse either a `{const}`/`{let}` declaration tag
    /// or an ordinary `{expr}` mustache, whichever the lookahead indicates.
    /// Shared by the root, element-children, and block-children fragment loops.
    pub(crate) fn parse_brace_tag(&mut self) -> Result<FragmentNode<'arena>, ParseError> {
        if self.opens_declaration_tag() {
            self.parse_declaration_tag(self.current_start)
        } else {
            Ok(FragmentNode::ExpressionTag(self.parse_expression_tag()?))
        }
    }

    /// Parse a declaration tag: `{const …}` / `{let …}`. The body is a TS variable
    /// declaration — `tsv_ts` parses it natively (declarators, comments, brackets,
    /// strings), so this delegates and rejects only a comment trailing the
    /// declaration before `}`, which Svelte does not allow. `{@const}` keeps its
    /// own `parse_const_tag`.
    pub(crate) fn parse_declaration_tag(
        &mut self,
        start: usize,
    ) -> Result<FragmentNode<'arena>, ParseError> {
        let tag_content_start = self.current_end;
        let (tag_content, after_close) = self.scan_block_tag_content(tag_content_start)?;

        let tsv_ts::Statement::VariableDeclaration(declaration) =
            self.parse_ts_statement(tag_content, tag_content_start)?
        else {
            return Err(
                self.error_msg_at("expected a `const` or `let` declaration", tag_content_start)
            );
        };

        // Svelte (like acorn) rejects a `const` declarator with no initializer;
        // tsv_ts's parser is more permissive, so enforce it here.
        if declaration.kind.as_str() == "const"
            && declaration
                .declarations
                .iter()
                .any(|decl| decl.init.is_none())
        {
            return Err(self.error_msg_at(
                "`const` declarations require an initializer",
                tag_content_start,
            ));
        }

        // Svelte rejects a comment trailing the declaration before `}`
        // (`{const x = v /* c */}`); only whitespace and an optional `;` may follow.
        let decl_end = declaration.span.end as usize;
        let close_brace = after_close - 1;
        // The declaration was parsed from the content the closing `}` bounds, so it
        // ends at or before the brace; assert it so a malformed span surfaces in
        // tests rather than as an opaque slice-index panic (no release-build cost).
        debug_assert!(
            decl_end <= close_brace,
            "declaration end must not pass the closing brace"
        );
        if !self.source[decl_end..close_brace]
            .trim_matches(|c: char| is_svelte_ws(c) || c == ';')
            .is_empty()
        {
            return Err(self.error_msg_at("unexpected content after declaration", decl_end));
        }

        Ok(FragmentNode::DeclarationTag(DeclarationTag {
            declaration,
            span: Span {
                start: start as u32,
                end: after_close as u32,
            },
        }))
    }

    /// Split a declarator string on the top-level `=` at `eq_pos` into a parsed
    /// (id pattern, init expression). `decl_offset` is the byte offset of
    /// `decl_str` in the source, used to recover each side's span.
    ///
    /// `{@const}`-only. The `{const}`/`{let}` declaration tags hand their whole content to
    /// `parse_ts_statement` instead, which is why the binding rule below is enforced here
    /// rather than at the tag: acorn takes a comment at either binding gap and Svelte's
    /// `read_pattern` does not, so the two tags genuinely disagree.
    fn parse_declarator(
        &mut self,
        decl_str: &'a str,
        decl_offset: usize,
        eq_pos: usize,
    ) -> Result<(Expression<'arena>, Expression<'arena>), ParseError> {
        let id_str = decl_str[..eq_pos].trim();
        let init_str = decl_str[eq_pos + 1..].trim();

        let id_offset = decl_offset + subslice_offset(decl_str, id_str);
        let init_offset =
            decl_offset + eq_pos + 1 + (decl_str[eq_pos + 1..].len() - init_str.len());

        // Taken off the id side derived just above, so the rule and the parse cannot
        // disagree about where the binding is.
        self.reject_binding_comments(id_str, id_offset)?;

        // id is a pattern (identifier or destructuring — `parse_ts_pattern`
        // converts ObjectExpression/ArrayExpression to patterns), init an expression.
        let id = self.parse_ts_pattern(id_str, id_offset)?;
        let init = self.parse_ts_expression(init_str, init_offset)?;
        Ok((id, init))
    }

    /// Reject a comment in the `{@const}` **binding** — the gaps Svelte's reader cannot
    /// cross. `read_pattern` starts at the first non-whitespace byte, so a comment before
    /// the pattern is `expected_pattern`; it then reads an optional `: T` (TS only) and the
    /// caller does `allow_whitespace()` + `eat('=')`, so a comment anywhere else outside the
    /// annotation is `expected_token`. Only the annotation's **interior** is comment-bearing,
    /// because acorn parses it: `y: A /* c */ | B` is legal, `y /* c */: T` and `y: T /* c */`
    /// are not.
    ///
    /// ⚠️ Reached only from [`Self::parse_declarator`], the `{@const}`-only splitter — the
    /// unprefixed `{const}`/`{let}` tags take a comment at either gap and must keep doing so.
    ///
    /// Without this the head over-accepts and the printer has no emitter for the gap: a line
    /// comment is DROPPED, and one before the `=` is RELOCATED across it onto the init.
    ///
    /// `id_str` is the binding side of the declarator, whitespace-trimmed; `id_offset` its
    /// byte offset in the source, for error positions.
    fn reject_binding_comments(&self, id_str: &str, id_offset: usize) -> Result<(), ParseError> {
        let bytes = id_str.as_bytes();
        let end = bytes.len();

        // Depth keeps a pattern's interior out of it — a comment inside `{ a }` or `[ a ]` is
        // acorn's, not Svelte's. Deliberately NOT tracking `<`/`>`: a type argument's interior
        // is already covered by the annotation rule below, while a relational `>` in a
        // destructuring default (`{ a = b > c }`) would decrement a depth it never opened.
        // `annot_colon` is the annotation opener
        // (an object pattern's `:` is nested, so a top-level one can only be that), and
        // `pending_annotation_comment` holds a comment that is legal *so far* and becomes a
        // violation only if nothing significant follows it.
        let mut depth = 0usize;
        let mut annot_colon: Option<usize> = None;
        let mut seen_significant = false;
        let mut pending_annotation_comment: Option<usize> = None;
        let mut i = 0;

        while i < end {
            if let Some(next) = skip_comment(bytes, i, end) {
                if depth == 0 {
                    if !seen_significant {
                        return Err(self.error_msg_at(
                            "Expected identifier or destructure pattern",
                            id_offset + i,
                        ));
                    }
                    match annot_colon {
                        // Before the annotation opener (or with no annotation at all) the
                        // comment sits in a gap Svelte crosses with `allow_whitespace`.
                        None => return Err(self.error_msg_at("Expected token =", id_offset + i)),
                        Some(_) => pending_annotation_comment = Some(i),
                    }
                }
                i = next;
                continue;
            }
            // Strings only — comments are handled above.
            if let Some(next) = skip_trivia(bytes, i, end, TriviaProfile::JS) {
                seen_significant = true;
                pending_annotation_comment = None;
                i = next;
                continue;
            }

            let b = bytes[i];
            match b {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                b':' if depth == 0 && annot_colon.is_none() => annot_colon = Some(i),
                _ => {}
            }
            if !b.is_ascii_whitespace() {
                seen_significant = true;
                pending_annotation_comment = None;
            }
            i += 1;
        }

        // A comment the annotation opened for but never closed over — it trails the type,
        // where Svelte is already looking for the `=`.
        if let Some(pos) = pending_annotation_comment {
            return Err(self.error_msg_at("Expected token =", id_offset + pos));
        }
        Ok(())
    }

    /// Find the top-level `=` in a declaration string (not inside brackets/braces,
    /// strings, or comments) — the one separating the binding from its initializer.
    fn find_top_level_equals(&self, s: &str) -> Result<usize, ParseError> {
        super::find_top_level_delim(s.as_bytes(), 0, s.len(), b'=', TriviaProfile::JS)
            .ok_or_else(|| ParseError::invalid_syntax("Expected '=' in declaration".to_string(), 0))
    }

    /// Parse a debug tag: {@debug} or {@debug x, y, z}
    ///
    /// Mirrors Svelte's `1-parse/state/tag.js`: the whole content after `debug`
    /// is parsed as one expression (`read_expression`), a top-level comma
    /// `SequenceExpression` is flattened into the identifier list, and every
    /// element must be a plain `Identifier` (`debug_tag_invalid_arguments`).
    /// `{@debug}` — only whitespace before `}` — is "debug all" (an empty list).
    ///
    /// Unlike Prettier (which strips comments), we preserve TS comments in debug
    /// tags; parsing via `parse_ts_expression` collects them into `Root.comments`
    /// for lookup by span.
    fn parse_debug_tag(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        let tag_content_start = self.current_end;
        let (tag_content, after_close) = self.scan_block_tag_content(tag_content_start)?;

        // Content after the `debug` keyword. Svelte does not require whitespace
        // after `debug` — `{@debug}`, `{@debug(a,b)}` are both valid — so this
        // strips only the keyword (the tag dispatch guarantees the prefix).
        debug_assert!(
            tag_content.starts_with("debug"),
            "debug tag dispatch guarantees the `debug` keyword prefix"
        );
        let rest = &tag_content["debug".len()..];
        let rest_offset = tag_content_start + "debug".len();

        let mut identifiers = self.bvec();

        // `{@debug}` — only whitespace before `}` — means "debug all" (no
        // identifiers). Svelte's `regex_whitespace_with_closing_curly_brace`
        // (`/\s*}/y`); a comment is not whitespace, so `{@debug /* c */}` falls
        // through to the parse below, which rejects (there is no expression).
        let expr_str = rest.trim();
        if !expr_str.is_empty() {
            let expr_offset = rest_offset + subslice_offset(rest, expr_str);

            // Parse the whole argument list as one expression (Svelte's
            // `read_expression`). `parse_ts_expression` enforces end-of-input,
            // so a trailing/leading/empty-slot comma (`{@debug a,}` /
            // `{@debug ,a}` / `{@debug a, , b}`) or a trailing token
            // (`{@debug a b}`) is a parse error, matching Svelte's
            // `eat('}', true)`. Comments are collected into `Root.comments`.
            let expr = self.parse_ts_expression(expr_str, expr_offset)?;

            // Flatten a top-level comma sequence — `{@debug a, b}` and
            // `{@debug (a, b)}` both yield `[a, b]` (a comma inside `()` is not a
            // top-level separator, so the parenthesized form is one
            // `SequenceExpression`); a single expression is a one-element list.
            match expr {
                Expression::SequenceExpression(seq) => {
                    for element in seq.expressions {
                        self.require_debug_identifier(element)?;
                        identifiers.push(element.clone());
                    }
                }
                _ => {
                    self.require_debug_identifier(&expr)?;
                    identifiers.push(expr);
                }
            }
        }

        Ok(FragmentNode::DebugTag(DebugTag {
            identifiers: identifiers.into_bump_slice(),
            span: Span {
                start: start as u32,
                end: after_close as u32,
            },
        }))
    }

    /// Every `{@debug}` argument must be a plain `Identifier`
    /// (`1-parse/state/tag.js`: `debug_tag_invalid_arguments`). tsv parses the
    /// argument list as a full TS expression, so reject the non-identifier forms
    /// (regex/member/call/binary/`this`/literal) here to match Svelte — and, for
    /// a regex literal, to avoid re-emitting a `/…*/` source span glued to `}` as
    /// unreparseable output.
    fn require_debug_identifier(&self, expr: &Expression<'arena>) -> Result<(), ParseError> {
        if matches!(expr, Expression::Identifier(_)) {
            Ok(())
        } else {
            Err(self.error_msg_at(
                "{@debug ...} arguments must be identifiers, not arbitrary expressions",
                expr.span().start as usize,
            ))
        }
    }

    /// Parse a render tag: {@render fn()} or {@render fn?.()}
    ///
    /// Svelte requires the expression to be a `CallExpression`, or a
    /// `ChainExpression` whose inner `.expression` is a `CallExpression`
    /// (`1-parse/state/tag.js`: `render_tag_invalid_expression`). tsv has no
    /// distinct `ChainExpression` node — an optional chain folds into the call
    /// node it wraps (`Expression::has_optional_in_chain` drives the wire wrap),
    /// so `foo()` and `foo?.()` both surface here as a top-level `CallExpression`
    /// and Svelte's two-branch check collapses to one: the expression must be a
    /// `CallExpression`. A non-call form (`{@render foo}`, `{@render a?.b}`) is
    /// rejected, mirroring `require_debug_identifier`.
    fn parse_render_tag(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        let (expression, span) = self.parse_keyword_expression_tag(start, "render")?;

        if !matches!(expression, Expression::CallExpression(_)) {
            return Err(self.error_msg_at(
                "{@render ...} tags can only contain call expressions",
                expression.span().start as usize,
            ));
        }

        Ok(FragmentNode::RenderTag(RenderTag { expression, span }))
    }
}
