// Template literal parsing: `\`hello ${name}\`` (simple and interpolated), plus
// the single `TemplateElement` constructor every template site builds through —
// expression AND type templates, head/middle/tail/no-substitution alike.

use crate::ast::internal::{Expression, TemplateCooked, TemplateElement, TemplateLiteral};
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::Parser;

/// The delimiter-stripped content of a template token, and its source span.
///
/// All eight construction sites reduce to two shapes, selected by `tail`: a
/// token that CLOSES the template (`` `x` ``, `` }x` ``) sheds one delimiter
/// byte at each end, while one that OPENS an interpolation (`` `x${ ``,
/// `` }x${ ``) sheds one at the front and two at the back — a middle/tail
/// token's leading delimiter being the `}` it starts at.
///
/// The length guard is the reason this is one function rather than eight
/// slicings: the lexer's shortest tokens are `` `` `` (2 bytes) and `` `${ ``
/// (3), so a well-formed token always satisfies it, and the arithmetic below
/// would panic on anything shorter.
#[inline]
fn template_token_content(raw: &str, span: Span, tail: bool) -> (&str, Span) {
    let closing = if tail { 1 } else { 2 };
    if raw.len() < 1 + closing {
        return ("", Span::new(span.start, span.start));
    }
    (
        &raw[1..raw.len() - closing],
        Span::new(span.start + 1, span.end - closing as u32),
    )
}

/// Normalize a template segment's `<CR>` line terminators to `<LF>`, or `None`
/// if there are none — the ECMAScript rule for both the TRV and the TV of a
/// `LineTerminatorSequence` inside a template (§12.9.6): `<CR><LF>` and a lone
/// `<CR>` each become one `<LF>`.
///
/// ⚠️ **`text` must be the RAW SOURCE**, never a decoded value. The rule covers a
/// literal terminator in the template body and nothing else, so an author's
/// `\r` **escape** must survive — and after decoding the two are the same
/// character, indistinguishable. Running this over a cooked value rewrites
/// `` `GET / HTTP/1.1\r\nHost: x` `` (an HTTP fixture, and real code) into
/// something that no longer means what it says. In the raw text the escape is
/// the two characters `\` `r`, which this scan cannot match.
///
/// ⚠️ Deliberately **narrower** than the `LineTerminatorSequence` class itself
/// (`tsv_lang::printing::line_terminator_len`, the right predicate for "where
/// does a line end"): `<LS>` and `<PS>` are terminators that map to *themselves*
/// here, so folding them in would rewrite a literal's value. The two questions
/// share a name and not an answer.
///
/// Returns `None` for the overwhelmingly common no-`<CR>` case so callers keep
/// the allocation-free source-slice path.
fn normalize_template_cr(text: &str) -> Option<String> {
    if !text.contains('\r') {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut parts = text.split('\r');
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for part in parts {
        // Each split point WAS a `<CR>`, and becomes the `<LF>`. A `<CR><LF>` is
        // ONE sequence, so an `<LF>` leading the part behind it was that pair's
        // second half — dropped rather than emitted twice.
        out.push('\n');
        out.push_str(part.strip_prefix('\n').unwrap_or(part));
    }
    Some(out)
}

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Build the cooked value for the current template token.
    ///
    /// The lexer reports `decoded == None` both for a segment with no escapes and
    /// for one whose escape sequence is invalid — the latter is deferred here
    /// (the lexer can't know whether the template is tagged). A backslash in the
    /// raw `content` distinguishes the invalid case: per the ES2018 template-
    /// literals revision an invalid escape is allowed in a **tagged** template
    /// (cooked value `null` → `TemplateCooked::Invalid`), but is a syntax error in
    /// an untagged template or a template-literal type.
    pub(super) fn template_cooked(
        &self,
        content: &str,
        tagged: bool,
    ) -> Result<TemplateCooked<'arena>, ParseError> {
        match self.current_decoded {
            Some(decoded) => Ok(TemplateCooked::Decoded(decoded)),
            None if content.contains('\\') => {
                if tagged {
                    Ok(TemplateCooked::Invalid)
                } else {
                    // Re-run the decode to surface the precise escape error the
                    // lexer swallowed to defer the tagged/untagged decision.
                    Err(crate::lexer::escapes::decode_string_escapes(content)
                        .err()
                        .unwrap_or_else(|| {
                            self.error_msg("Invalid escape sequence in template literal")
                        }))
                }
            }
            None => Ok(TemplateCooked::Verbatim),
        }
    }

    /// Build the `TemplateElement` for the current template token.
    ///
    /// The one constructor for all eight sites (expression and type templates ×
    /// no-substitution/head/middle/tail), so the delimiter arithmetic, the TRV
    /// normalization, the cooked decision and the `has_newline` precompute
    /// cannot drift between them.
    ///
    /// `span` is the element's span AND the token's: a middle/tail token starts
    /// at the prior `}` (`Lexer::continue_template_from_brace` stamps
    /// `start: brace_start`), which is exactly where the element starts, so the
    /// two are never distinct. The type-template path used to thread a separate
    /// `brace_start` for this and it was always the same value.
    ///
    /// Call BEFORE `advance()` — this reads the current token's text and its
    /// decoded value.
    pub(super) fn template_element(
        &self,
        span: Span,
        tail: bool,
        tagged: bool,
    ) -> Result<TemplateElement<'arena>, ParseError> {
        let arena = self.arena;
        let (content, raw_span) = template_token_content(self.current_value(), span, tail);
        let cooked = self.template_cooked(content, tagged)?;
        // `has_newline` asks about the SOURCE bytes — the printer walks
        // `raw_span` — so it stays on `content` under either arm below.
        let has_newline = content.contains('\n');

        let Some(trv) = normalize_template_cr(content) else {
            return Ok(TemplateElement {
                raw_span,
                raw_trv: None,
                cooked,
                has_newline,
                tail,
                span,
            });
        };

        // The TV normalizes with the TRV, so a decoded value must be decoded FROM
        // the normalized raw. Normalizing the DECODED text instead is the trap: by
        // then an author's `\r` escape is the same character as a literal `<CR>`,
        // so the pass rewrites `` `… HTTP/1.1\r\nHost: …` `` — real code, and the
        // corpus caught it. This decodes with the lexer's own function
        // (`decode_string_escapes_into` is the template path in `lexer/core.rs`),
        // so it is a re-run, not a second implementation.
        let cooked = match cooked {
            TemplateCooked::Decoded(_) => TemplateCooked::Decoded(
                arena.alloc_str(&crate::lexer::escapes::decode_string_escapes(&trv)?),
            ),
            verbatim_or_invalid => verbatim_or_invalid,
        };

        Ok(TemplateElement {
            raw_span,
            raw_trv: Some(arena.alloc_str(&trv)),
            cooked,
            has_newline,
            tail,
            span,
        })
    }

    /// Parse template literal: `hello ${name}`
    ///
    /// Handles both simple templates (no interpolation) and templates with expressions.
    /// `tagged` is true when this template is the quasi of a tagged-template
    /// expression — it relaxes invalid-escape handling per ES2018 (see
    /// `template_cooked`). See also `parse_template_literal_type()` in types.rs.
    pub(super) fn parse_template_literal(
        &mut self,
        tagged: bool,
    ) -> Result<Expression<'arena>, ParseError> {
        let (start, _) = self.current_pos();
        let mut quasis = self.bvec();
        let mut expressions = self.bvec();

        match self.current_kind() {
            TokenKind::NoSubstitutionTemplate => {
                // Simple template with no interpolation: `hello world`
                let (elem_start, elem_end) = self.current_pos();
                let token = Span::new(elem_start as u32, elem_end as u32);
                let element = self.template_element(token, true, tagged)?;

                self.advance()?;

                quasis.push(element);

                Ok(Expression::TemplateLiteral(TemplateLiteral {
                    quasis: quasis.into_bump_slice(),
                    expressions: expressions.into_bump_slice(),
                    span: Span::new(start as u32, elem_end as u32),
                }))
            }
            TokenKind::TemplateHead => {
                // Template with interpolation: `hello ${name}...`
                let (elem_start, elem_end) = self.current_pos();
                let token = Span::new(elem_start as u32, elem_end as u32);
                let element = self.template_element(token, false, tagged)?;

                self.advance()?;

                quasis.push(element);

                self.enter_grouping();

                // Parse expressions and remaining template parts
                loop {
                    // Parse the interpolated expression
                    let expr = self.parse_expression()?;
                    expressions.push(expr);

                    // Expect closing } of the interpolation
                    let (brace_start, _) = self.current_pos();
                    if !self.check(&TokenKind::BraceClose) {
                        return Err(self.error_expected_found_at(
                            "'}' at end of template interpolation",
                            brace_start,
                        ));
                    }

                    // Get the raw end position (without base_offset) for the lexer
                    let raw_brace_end = self.current_raw_end();

                    // Skip the } in the lexer without getting next token normally
                    // (calling advance() would try to lex ` as a new token)
                    // Instead, tell the lexer to skip past the } and read template content
                    let next_token = self.lexer.continue_template_from_brace(raw_brace_end)?;
                    self.update_current(next_token);

                    let (elem_start, elem_end) = self.current_pos();

                    match *self.current_kind() {
                        TokenKind::TemplateMiddle => {
                            // More interpolations to come: }content${
                            let token = Span::new(elem_start as u32, elem_end as u32);
                            let element = self.template_element(token, false, tagged)?;

                            self.advance()?;

                            quasis.push(element);
                        }
                        TokenKind::TemplateTail => {
                            // End of template: }content`
                            let token = Span::new(elem_start as u32, elem_end as u32);
                            let element = self.template_element(token, true, tagged)?;

                            self.advance()?;

                            quasis.push(element);

                            break;
                        }
                        _ => {
                            return Err(
                                self.error_expected_found_at("template middle or tail", elem_start)
                            );
                        }
                    }
                }

                self.exit_grouping();

                let end = quasis.last().map_or(start as u32, |q| q.span.end);

                Ok(Expression::TemplateLiteral(TemplateLiteral {
                    quasis: quasis.into_bump_slice(),
                    expressions: expressions.into_bump_slice(),
                    span: Span::new(start as u32, end),
                }))
            }
            _ => Err(self.error_expected_found_at("template literal", start)),
        }
    }
}
