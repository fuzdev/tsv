// Template tag parsing
//
// Handles: {@html}, {@const}, {@debug}, {@render} tags and {const}/{let} declaration tags

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use crate::whitespace::is_svelte_ws;
use tsv_lang::source_scan::TriviaProfile;
use tsv_lang::{ParseError, Span};
use tsv_ts::{Expression, ExpressionKind};

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
    ) -> Result<(&'arena Expression<'arena>, Span), ParseError> {
        let tag_content_start = self.current_end;
        let (tag_content, after_close) = self.scan_block_tag_content(tag_content_start)?;

        // Svelte requires whitespace after the keyword. Leading whitespace only — the
        // trailing run may be a line comment's own text (`Parser::parse_ts_expression`).
        let expr_str = self
            .strip_block_keyword(tag_content, keyword, tag_content_start)?
            .trim_start_matches(is_svelte_ws);

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
            .trim_start_matches(is_svelte_ws);

        let decl_offset = tag_content_start + subslice_offset(tag_content, decl_str);
        let (id, init) = self.parse_declarator(decl_str, decl_offset)?;

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
        let kw_start = after_brace + (rest.len() - rest.trim_start_matches(is_svelte_ws).len());
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

        let tsv_ts::StatementKind::VariableDeclaration(declaration) = self
            .parse_ts_statement(tag_content, tag_content_start)?
            .kind
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

    /// Read a `{@const}` declarator — `decl_str`, whose first byte is at source offset
    /// `decl_offset` — into its (id pattern, init expression), the way Svelte's `tag()` does:
    /// `read_pattern`, then `allow_whitespace` + `eat('=', true)`, then `read_expression` over
    /// the rest of the tag.
    ///
    /// The binding is [`Self::parse_block_pattern`]'s, the reader every `read_pattern`
    /// position shares, so its head gate and bounded extent hold here too: `(a)`, `(a): T`
    /// and `([a])` fail at the head (`expected_pattern`), and a binding that would continue
    /// as an expression (`a.b`, `a?.b`, `[a][0]`, `{ a }.b`, `[a] [b]`, `a, b`) stops at its
    /// own end, where the `=` is then missing. The declarator's `=` is therefore the first
    /// non-whitespace byte past the binding — never found by a scan, which cannot see TS
    /// type syntax (a function type's `=>`, a type argument list's `>`). A comment on either
    /// side of the binding is rejected by the same two reads, as canonical's `allow_whitespace`
    /// rejects it; one inside a destructure or inside the annotation is acorn's and stays.
    ///
    /// `{@const}`-only. The `{const}`/`{let}` declaration tags hand their whole content to
    /// `parse_ts_statement` instead: acorn takes a comment at either binding gap and Svelte's
    /// `read_pattern` does not, so the two tags genuinely disagree.
    fn parse_declarator(
        &mut self,
        decl_str: &'a str,
        decl_offset: usize,
    ) -> Result<(&'arena Expression<'arena>, &'arena Expression<'arena>), ParseError> {
        let (id, binding_end) = self.parse_block_pattern(decl_str, decl_offset)?;
        let after_binding = &decl_str[binding_end - decl_offset..];
        let at_eq = after_binding.trim_start_matches(is_svelte_ws);
        let eq_offset = binding_end + (after_binding.len() - at_eq.len());
        let Some(after_eq) = at_eq.strip_prefix('=') else {
            return Err(self.error_msg_at("Expected token =", eq_offset));
        };
        // The init ends at the tag's `}`, and its trailing run may be a line comment's own
        // text, so only the LEADING run is trimmed (`Parser::parse_ts_expression`).
        let init_str = after_eq.trim_start_matches(is_svelte_ws);
        let init_offset = eq_offset + 1 + (after_eq.len() - init_str.len());
        let init = self.parse_ts_expression(init_str, init_offset)?;
        self.reject_multi_declarator(init, init_str, init_offset)?;
        Ok((id, init))
    }

    /// Reject a `{@const}` whose init is a **bare** sequence — the second declarator of
    /// `{@const a = 1, b = 2}`, which Svelte refuses (`const_tag_invalid_expression`)
    /// while allowing the parenthesized `{@const a = (1, 2)}`.
    ///
    /// The two halves of the question have two different oracles, and asking either one
    /// alone is a bug:
    ///
    /// - *Is there a bare top-level comma at all?* Only the **node** knows — a scan over
    ///   the head cannot, for the reason [`super::find_top_level_delim`] states: it reads
    ///   a type argument's `,` as a separator and rejects `{@const a: Map<A, B> = expr}` /
    ///   `{@const a = fn<A, B>(expr)}`.
    /// - *Did the author parenthesize it?* Only the **source** knows. Both parsers drop
    ///   the parens — tsv has no `ParenthesizedExpression` and Svelte runs
    ///   `remove_parens` — so `(1, 2)` and `1, 2` reach here as the same node. Svelte
    ///   reads the source for a `(` between the init's start and the node's own start;
    ///   the depth scan below answers it the other way round, since a comma the author
    ///   parenthesized is not at depth 0. The two agree on the boundary shapes —
    ///   `(1), 2` and `(1, 2), (3, 4)` reject, `((1), 2)` and `((1, 2), 3)` do not.
    ///
    /// Gating the scan on the node is what makes it sound: a `SequenceExpression`'s own
    /// separators are top-level commas by construction, so the untracked `<`/`>` interior
    /// can no longer produce a false positive.
    fn reject_multi_declarator(
        &self,
        init: &Expression<'arena>,
        init_str: &str,
        init_offset: usize,
    ) -> Result<(), ParseError> {
        if !matches!(init.kind, ExpressionKind::SequenceExpression(_)) {
            return Ok(());
        }
        match super::find_top_level_delim(
            init_str.as_bytes(),
            0,
            init_str.len(),
            b',',
            TriviaProfile::JS,
        ) {
            Some(comma) => Err(self.error_msg_at(
                "{@const ...} must consist of a single variable declaration",
                init_offset + comma,
            )),
            None => Ok(()),
        }
    }

    /// Parse a debug tag: {@debug} or {@debug x, y, z}
    ///
    /// Mirrors Svelte's `1-parse/state/tag.js`: the whole content after `debug`
    /// is parsed as one expression (`read_expression`), a top-level comma
    /// `SequenceExpression` is flattened into the identifier list, and every
    /// element must be a plain `Identifier` (`debug_tag_invalid_arguments`) —
    /// through any JSDoc cast, which Svelte's post-`remove_parens` check never
    /// sees (a cast element stays a cast in the stored list; a cast around the
    /// whole list stays ONE entry, its sequence flattened only on the wire).
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
        //
        // Leading whitespace only — the trailing run may be a line comment's own text
        // (`Parser::parse_ts_expression`) — and that is also the whitespace-only test, since
        // a region of nothing but whitespace trims to empty from the front alone.
        let expr_str = rest.trim_start_matches(is_svelte_ws);
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
            match &expr.kind {
                ExpressionKind::SequenceExpression(seq) => {
                    for element in seq.expressions {
                        self.require_debug_identifier(element)?;
                        identifiers.push((*element).clone());
                    }
                }
                _ => {
                    // A JSDoc cast around the whole comma list validates per
                    // element — Svelte's check runs after `remove_parens`, which
                    // sees only parens + a comment here, so the sequence it
                    // uncovers flattens exactly as the bare one above. The STORED
                    // entry keeps the wrapper as one element for the whole cast:
                    // the printer emits identifiers by source span, so flattening
                    // through the cast would drop its parens from the output
                    // (breaking idempotency and the cast's binding).
                    if let ExpressionKind::SequenceExpression(seq) = &expr.unwrap_jsdoc_casts().kind
                    {
                        for element in seq.expressions {
                            self.require_debug_identifier(element)?;
                        }
                    } else {
                        self.require_debug_identifier(expr)?;
                    }
                    identifiers.push(expr.clone());
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
    /// unreparseable output. The check looks through JSDoc casts
    /// (`unwrap_jsdoc_casts`): Svelte validates after `remove_parens`, to which a
    /// cast is nothing but parens + a comment, so `{@debug /** @type {A} */ (a)}`
    /// is an identifier to both parsers.
    fn require_debug_identifier(&self, expr: &Expression<'arena>) -> Result<(), ParseError> {
        if matches!(
            expr.unwrap_jsdoc_casts().kind,
            ExpressionKind::Identifier(_)
        ) {
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
    ///
    /// The check looks through JSDoc casts (`unwrap_jsdoc_casts`): Svelte
    /// validates after `remove_parens`, to which a cast is nothing but parens +
    /// a comment, so `{@render /** @type {A} */ (fn())}` is a call to both
    /// parsers. The stored expression keeps the wrapper — the printer reproduces
    /// the authored cast from it.
    fn parse_render_tag(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        let (expression, span) = self.parse_keyword_expression_tag(start, "render")?;

        if !matches!(
            expression.unwrap_jsdoc_casts().kind,
            ExpressionKind::CallExpression(_)
        ) {
            return Err(self.error_msg_at(
                "{@render ...} tags can only contain call expressions",
                expression.span().start as usize,
            ));
        }

        Ok(FragmentNode::RenderTag(RenderTag { expression, span }))
    }
}
