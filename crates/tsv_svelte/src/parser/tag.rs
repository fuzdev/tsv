// Template tag parsing
//
// Handles: {@html}, {@const}, {@debug}, {@render} tags and {const}/{let} declaration tags

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::parser_impl::SvelteParser;

/// Iterate the `(byte_offset, char)` of the chars in a declaration string that sit
/// at bracket depth 0 and outside string literals — the structurally "top-level"
/// positions where a declarator `,` separator or a binding/init `=` appears.
/// Brackets (`()`/`[]`/`{}`) with their contents, and string literals, are skipped.
///
/// NOTE: escape handling is simplified — a `\` before a closing quote isn't treated
/// as an escaped backslash (`"a\\"` would misparse). Unlikely in real declarations.
fn top_level_chars(s: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut string_char = '"';
    s.char_indices().filter(move |&(i, c)| {
        if in_string {
            if c == string_char && !s[..i].ends_with('\\') {
                in_string = false;
            }
            false
        } else {
            match c {
                '"' | '\'' | '`' => {
                    in_string = true;
                    string_char = c;
                    false
                }
                '(' | '[' | '{' => {
                    depth += 1;
                    false
                }
                ')' | ']' | '}' => {
                    depth -= 1;
                    false
                }
                _ => depth == 0,
            }
        }
    })
}

impl<'a> SvelteParser<'a> {
    /// Parse a template tag starting with {@
    ///
    /// Dispatches to specific tag parsers based on the keyword.
    pub(crate) fn parse_template_tag(&mut self) -> Result<FragmentNode, ParseError> {
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

    /// Parse an html tag: {@html expression}
    fn parse_html_tag(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
        let tag_content_start = self.current_end;
        let (tag_content, after_close) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "html expression" — Svelte requires whitespace after the keyword.
        let expr_str = self
            .strip_block_keyword(tag_content, "html", tag_content_start)?
            .trim();

        let expr_offset = tag_content_start + super::subslice_offset(tag_content, expr_str);
        let expression = self.parse_ts_expression(expr_str, expr_offset)?;

        // End is right after the closing }
        let end = after_close;

        Ok(FragmentNode::HtmlTag(HtmlTag {
            expression,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        }))
    }

    /// Parse a const tag: {@const name = expression}
    fn parse_const_tag(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
        let tag_content_start = self.current_end;
        let (tag_content, after_close) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "const name = expression" — Svelte requires whitespace after the keyword.
        let decl_str = self
            .strip_block_keyword(tag_content, "const", tag_content_start)?
            .trim();

        let decl_offset = tag_content_start + super::subslice_offset(tag_content, decl_str);

        // `{@const}` must be a single declarator (unlike the bare `{const}`/`{let}`
        // tags) — Svelte rejects `{@const a = 1, b = 2}`.
        if top_level_chars(decl_str).any(|(_, c)| c == ',') {
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
    pub(crate) fn parse_brace_tag(&mut self) -> Result<FragmentNode, ParseError> {
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
    ) -> Result<FragmentNode, ParseError> {
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
        if !self.source[decl_end..close_brace]
            .trim_matches(|c: char| c.is_whitespace() || c == ';')
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
    /// `decl_str` in the source, used to recover each side's span. Shared by the
    /// `{@const}` and the with-init `{const}`/`{let}` paths.
    fn parse_declarator(
        &mut self,
        decl_str: &'a str,
        decl_offset: usize,
        eq_pos: usize,
    ) -> Result<(tsv_ts::Expression, tsv_ts::Expression), ParseError> {
        let id_str = decl_str[..eq_pos].trim();
        let init_str = decl_str[eq_pos + 1..].trim();

        let id_offset = decl_offset + super::subslice_offset(decl_str, id_str);
        let init_offset =
            decl_offset + eq_pos + 1 + (decl_str[eq_pos + 1..].len() - init_str.len());

        // id is a pattern (identifier or destructuring — `parse_ts_pattern`
        // converts ObjectExpression/ArrayExpression to patterns), init an expression.
        let id = self.parse_ts_pattern(id_str, id_offset)?;
        let init = self.parse_ts_expression(init_str, init_offset)?;
        Ok((id, init))
    }

    /// Find the top-level `=` in a declaration string (not inside brackets/braces
    /// or strings) — the one separating the binding from its initializer.
    fn find_top_level_equals(&self, s: &str) -> Result<usize, ParseError> {
        top_level_chars(s)
            .find(|&(_, c)| c == '=')
            .map(|(i, _)| i)
            .ok_or_else(|| ParseError::InvalidSyntax {
                message: "Expected '=' in declaration".to_string(),
                position: 0,
                context: None,
            })
    }

    /// Parse a debug tag: {@debug} or {@debug x, y, z}
    ///
    /// Unlike Prettier (which strips comments), we preserve TS comments in debug tags.
    /// Comments are extracted and stored in Root.comments for lookup by span.
    fn parse_debug_tag(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
        let tag_content_start = self.current_end;
        let (tag_content, after_close) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "debug" or "debug x, y, z"
        // First get the part after "debug" keyword
        let (idents_str, idents_offset) = if let Some(stripped) = tag_content.strip_prefix("debug ")
        {
            let offset = tag_content_start + "debug ".len();
            (stripped, offset)
        } else if let Some(stripped) = tag_content.strip_prefix("debug") {
            let offset = tag_content_start + "debug".len();
            (stripped, offset)
        } else {
            ("", tag_content_start)
        };

        // Extract TS comments from the identifiers portion (preserves in Root.comments)
        // Returns content with comments replaced by spaces (positions preserved)
        let cleaned_idents = self.extract_ts_comments(idents_str, idents_offset);

        let mut identifiers = Vec::new();
        if !cleaned_idents.trim().is_empty() {
            // Parse comma-separated identifiers from the cleaned content
            // Since comments are replaced with equal-length spaces, positions are preserved
            let mut pos = 0;
            for chunk in cleaned_idents.split(',') {
                let trimmed = chunk.trim();
                if !trimmed.is_empty() {
                    // Find where trimmed content starts within this chunk
                    let trim_offset = super::subslice_offset(chunk, trimmed);
                    let ident_offset = idents_offset + pos + trim_offset;
                    let expr = self.parse_ts_expression(trimmed, ident_offset)?;
                    identifiers.push(expr);
                }
                pos += chunk.len() + 1; // +1 for the comma
            }
        }

        let end = after_close;

        Ok(FragmentNode::DebugTag(DebugTag {
            identifiers,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        }))
    }

    /// Parse a render tag: {@render fn()} or {@render fn?.()}
    fn parse_render_tag(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
        let tag_content_start = self.current_end;
        let (tag_content, after_close) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "render expression" where expression must be a call — Svelte
        // requires whitespace after the keyword.
        let expr_str = self
            .strip_block_keyword(tag_content, "render", tag_content_start)?
            .trim();

        let expr_offset = tag_content_start + super::subslice_offset(tag_content, expr_str);
        let expression = self.parse_ts_expression(expr_str, expr_offset)?;

        let end = after_close;

        Ok(FragmentNode::RenderTag(RenderTag {
            expression,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        }))
    }
}
