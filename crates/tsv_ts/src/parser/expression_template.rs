// Template literal parsing: `\`hello ${name}\`` (simple and interpolated), plus
// the raw-content slicers for head/middle/tail/no-substitution template tokens.

use crate::ast::internal::{Expression, TemplateElement, TemplateLiteral};
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::Parser;

/// Extract content from template head: `content${ → "content"
#[inline]
fn extract_template_head_content(raw: &str) -> &str {
    if raw.len() >= 3 {
        &raw[1..raw.len() - 2]
    } else {
        ""
    }
}

/// Extract content from template tail: }content` → "content"
#[inline]
fn extract_template_tail_content(raw: &str) -> &str {
    if raw.len() >= 2 {
        &raw[1..raw.len() - 1]
    } else {
        ""
    }
}

/// Extract content from no-substitution template: `content` → "content"
#[inline]
fn extract_template_simple_content(raw: &str) -> &str {
    extract_template_tail_content(raw) // Same logic: strip first and last char
}

impl<'a> Parser<'a> {
    /// Parse template literal: `hello ${name}`
    ///
    /// Handles both simple templates (no interpolation) and templates with expressions.
    /// See also `parse_template_literal_type()` in statement.rs for type context version.
    pub(super) fn parse_template_literal(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();
        let mut quasis = Vec::new();
        let mut expressions = Vec::new();

        match self.current_kind() {
            TokenKind::NoSubstitutionTemplate => {
                // Simple template with no interpolation: `hello world`
                let (elem_start, elem_end) = self.current_pos();
                let content = extract_template_simple_content(self.current_value());
                let has_newline = content.contains('\n');
                let cooked = self.current_decoded().map_or_else(
                    || Some(content.to_string()),
                    |decoded| Some(decoded.to_string()),
                );
                // Content span: strip the opening and closing backticks.
                let raw_span = Span::new(elem_start as u32 + 1, elem_end as u32 - 1);

                self.advance()?;

                quasis.push(TemplateElement {
                    raw_span,
                    cooked,
                    has_newline,
                    tail: true,
                    span: Span::new(elem_start as u32, elem_end as u32),
                });

                Ok(Expression::TemplateLiteral(TemplateLiteral {
                    quasis,
                    expressions,
                    span: Span::new(start as u32, elem_end as u32),
                }))
            }
            TokenKind::TemplateHead => {
                // Template with interpolation: `hello ${name}...`
                let (elem_start, elem_end) = self.current_pos();
                let content = extract_template_head_content(self.current_value());
                let has_newline = content.contains('\n');
                let cooked = self.current_decoded().map_or_else(
                    || Some(content.to_string()),
                    |decoded| Some(decoded.to_string()),
                );
                // Content span: strip the opening backtick and trailing `${`.
                let raw_span = Span::new(elem_start as u32 + 1, elem_end as u32 - 2);

                self.advance()?;

                quasis.push(TemplateElement {
                    raw_span,
                    cooked,
                    has_newline,
                    tail: false,
                    span: Span::new(elem_start as u32, elem_end as u32),
                });

                self.grouping_depth += 1;

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
                            let content = extract_template_head_content(self.current_value());
                            let has_newline = content.contains('\n');
                            let cooked = self.current_decoded().map_or_else(
                                || Some(content.to_string()),
                                |decoded| Some(decoded.to_string()),
                            );
                            // Content span: strip the leading `}` and trailing `${`.
                            let raw_span = Span::new(elem_start as u32 + 1, elem_end as u32 - 2);

                            self.advance()?;

                            quasis.push(TemplateElement {
                                raw_span,
                                cooked,
                                has_newline,
                                tail: false,
                                span: Span::new(elem_start as u32, elem_end as u32),
                            });
                        }
                        TokenKind::TemplateTail => {
                            // End of template: }content`
                            let content = extract_template_tail_content(self.current_value());
                            let has_newline = content.contains('\n');
                            let cooked = self.current_decoded().map_or_else(
                                || Some(content.to_string()),
                                |decoded| Some(decoded.to_string()),
                            );
                            // Content span: strip the leading `}` and trailing backtick.
                            let raw_span = Span::new(elem_start as u32 + 1, elem_end as u32 - 1);

                            self.advance()?;

                            quasis.push(TemplateElement {
                                raw_span,
                                cooked,
                                has_newline,
                                tail: true,
                                span: Span::new(elem_start as u32, elem_end as u32),
                            });

                            break;
                        }
                        _ => {
                            return Err(
                                self.error_expected_found_at("template middle or tail", elem_start)
                            );
                        }
                    }
                }

                self.grouping_depth -= 1;

                let end = quasis.last().map_or(start as u32, |q| q.span.end);

                Ok(Expression::TemplateLiteral(TemplateLiteral {
                    quasis,
                    expressions,
                    span: Span::new(start as u32, end),
                }))
            }
            _ => Err(self.error_expected_found_at("template literal", start)),
        }
    }
}
