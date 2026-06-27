// Attribute parsing

use bumpalo::collections::Vec as BumpVec;

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};
use tsv_ts::ast::internal::{Expression, Identifier};

use super::expression_tag::scan_to_matching_brace;
use super::parser_impl::SvelteParser;

/// Does a `{` at byte `pos` begin an expression tag, vs a block/tag construct?
///
/// `{#`, `{:`, `{@` (block/tag opens) and `{/word}` (block close) are not
/// expression tags; `{/*` and `{//` are comments, which are. Mirrors the lexer's
/// brace dispatch — shared by the quoted and unquoted attribute-value readers so
/// the two can't drift apart.
fn brace_starts_expression(bytes: &[u8], pos: usize) -> bool {
    match bytes.get(pos + 1) {
        Some(b'#' | b':' | b'@') => false,
        Some(b'/') => matches!(bytes.get(pos + 2), Some(b'*' | b'/')),
        _ => true,
    }
}

/// Directive prefix types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectiveType {
    On,
    Bind,
    Class,
    Style,
    Use,
    Transition,
    In,
    Out,
    Animate,
    Let,
}

impl DirectiveType {
    fn from_prefix(prefix: &str) -> Option<Self> {
        match prefix {
            "on" => Some(Self::On),
            "bind" => Some(Self::Bind),
            "class" => Some(Self::Class),
            "style" => Some(Self::Style),
            "use" => Some(Self::Use),
            "transition" => Some(Self::Transition),
            "in" => Some(Self::In),
            "out" => Some(Self::Out),
            "animate" => Some(Self::Animate),
            "let" => Some(Self::Let),
            _ => None,
        }
    }
}

impl<'a, 'arena> SvelteParser<'a, 'arena> {
    /// Return `end + 1` if the byte at `end` is a quote character, else `end`.
    ///
    /// Used when the last value part of a quoted attribute is an ExpressionTag:
    /// the `}` is included in the ExpressionTag span but the closing `"` is not.
    fn end_past_optional_quote(&self, end: usize) -> usize {
        if end < self.source.len() && matches!(self.source.as_bytes()[end], b'"' | b'\'') {
            end + 1
        } else {
            end
        }
    }

    /// Parse attribute list (e.g., `lang="ts" class="foo"`)
    /// Consumes tokens until we hit `>` or `/>`
    ///
    /// Supports:
    /// - Standard attributes: `name="value"` or `name={expr}`
    /// - Boolean attributes: `disabled`
    /// - Directives: `on:click`, `bind:value`, `class:class1`, etc.
    /// - Attach tags: `{@attach expr}` (Svelte 5.29+)
    /// - Spread attributes: `{...obj}` (Svelte 3+)
    /// - Shorthand attributes: `{name}` (equivalent to `name={name}`)
    pub(crate) fn parse_attributes(
        &mut self,
    ) -> Result<BumpVec<'arena, AttributeNode<'arena>>, ParseError> {
        self.parse_attributes_inner(true)
    }

    /// Parse attribute list for script/style tags where expressions are NOT parsed in quoted values
    ///
    /// Script and style tags use plain text attribute values - `{a: A}` is literal text,
    /// not an expression tag.
    pub(crate) fn parse_attributes_literal(
        &mut self,
    ) -> Result<BumpVec<'arena, AttributeNode<'arena>>, ParseError> {
        self.parse_attributes_inner(false)
    }

    fn parse_attributes_inner(
        &mut self,
        parse_expressions: bool,
    ) -> Result<BumpVec<'arena, AttributeNode<'arena>>, ParseError> {
        let mut attributes = self.bvec();

        loop {
            // Skip JS comments (// and /* */) between attributes
            while self.check(TokenKind::Slash) {
                if !self.try_read_js_comment()? {
                    break; // Regular slash (self-closing />)
                }
            }

            // Stop at > or />
            if self.check(TokenKind::RightAngle) || self.check(TokenKind::Slash) {
                break;
            }

            if self.check(TokenKind::Identifier) {
                attributes.push(self.parse_attribute_or_directive(parse_expressions)?);
            } else if self.check(TokenKind::TagOpen) {
                // {@ token - check if it's @attach
                attributes.push(AttributeNode::AttachTag(self.parse_attach_tag()?));
            } else if self.check(TokenKind::LeftBrace) {
                // { token - could be spread {...obj} or shorthand {name}
                // Peek ahead to determine which
                let next_char = self.peek_char_after_brace();
                if next_char == Some('.') {
                    // {...} - spread attribute
                    attributes.push(AttributeNode::SpreadAttribute(
                        self.parse_spread_attribute()?,
                    ));
                } else {
                    // {identifier} - shorthand attribute
                    attributes.push(AttributeNode::Attribute(self.parse_shorthand_attribute()?));
                }
            } else {
                return Err(self.error_expected_found("attribute name or '>'"));
            }
        }

        Ok(attributes)
    }

    /// Peek at the first non-whitespace character after the opening brace
    fn peek_char_after_brace(&self) -> Option<char> {
        let pos = self.current_start + 1; // Skip the '{'
        self.source.get(pos..)?.chars().find(|c| !c.is_whitespace())
    }

    /// Parse an attribute or directive
    ///
    /// Detects if the attribute name contains a colon (`:`) indicating a directive,
    /// and routes to the appropriate parser.
    fn parse_attribute_or_directive(
        &mut self,
        parse_expressions: bool,
    ) -> Result<AttributeNode<'arena>, ParseError> {
        let name_str = self.current_value().to_string();

        // Check if this is a directive (contains colon)
        if let Some(colon_idx) = name_str.find(':') {
            let prefix = &name_str[..colon_idx];
            if let Some(directive_type) = DirectiveType::from_prefix(prefix) {
                return self.parse_directive(directive_type, &name_str, colon_idx);
            }
        }

        // Not a directive, parse as regular attribute
        Ok(AttributeNode::Attribute(
            self.parse_attribute_inner(parse_expressions)?,
        ))
    }

    /// Parse a directive (on:, bind:, class:, style:, use:, transition:, in:, out:, animate:, let:)
    fn parse_directive(
        &mut self,
        directive_type: DirectiveType,
        full_name: &str,
        colon_idx: usize,
    ) -> Result<AttributeNode<'arena>, ParseError> {
        let start = self.current_start;
        let name_end = self.current_end;
        let head_span = Span {
            start: start as u32,
            end: name_end as u32,
        };

        // Extract directive name and modifiers from: prefix:name|mod1|mod2
        let after_colon = &full_name[colon_idx + 1..];
        let mut parts = after_colon.split('|');
        // The name is a verbatim source slice (HTML/Svelte attribute names are never
        // entity-decoded), so it's stored as a span — not an arena copy. `start` is the
        // attribute-name token start and `full_name` is that raw token, so the name occupies
        // `source[start + colon_idx + 1 .. + name.len()]`. The borrow lives only for this
        // method (the AST stores `name_span`, not the string).
        let directive_name: &str = parts.next().unwrap_or("");
        let name_start = start + colon_idx + 1;
        let name_span = Span {
            start: name_start as u32,
            end: (name_start + directive_name.len()) as u32,
        };
        let mut modifiers_vec = self.bvec();
        for m in parts {
            let m: &'arena str = self.alloc_str_in(m);
            modifiers_vec.push(m);
        }
        let modifiers: &'arena [&'arena str] = modifiers_vec.into_bump_slice();

        if directive_name.is_empty() {
            return Err(self.error_msg_at(
                &format!("Directive '{}' is missing a name", &full_name[..=colon_idx]),
                start,
            ));
        }

        self.advance()?; // consume the identifier

        // Style directives accept expression OR string values, handle separately
        if directive_type == DirectiveType::Style {
            return self.parse_style_directive(name_span, modifiers, start, name_end, head_span);
        }

        // Check for = (directive with value)
        let (expression, expression_tag_span) = if self.check(TokenKind::Equals) {
            self.advance()?; // consume =
            let (expr, tag_span) = self.parse_directive_expression()?;
            (Some(expr), Some(tag_span))
        } else {
            (None, None)
        };

        // Calculate end position
        // For the quoted mustache form ("{expr}") the tag span ends at `}` but the
        // directive includes the closing quote (matching Svelte)
        let end = if let Some(tag_span) = &expression_tag_span {
            self.end_past_optional_quote(tag_span.end as usize)
        } else {
            name_end
        };

        let span = Span {
            start: start as u32,
            end: end as u32,
        };

        // Create the directive
        match directive_type {
            DirectiveType::On => Ok(AttributeNode::OnDirective(OnDirective {
                name_span,
                expression,
                modifiers,
                span,
                head_span,
                expression_tag_span,
            })),
            DirectiveType::Bind => {
                // Bind directive always has an expression (auto-generated for shorthand)
                let expr = expression.unwrap_or_else(|| {
                    self.make_shorthand_identifier(directive_name, colon_idx + 1 + start, name_end)
                });
                Ok(AttributeNode::BindDirective(BindDirective {
                    name_span,
                    expression: expr,
                    modifiers,
                    span,
                    head_span,
                    expression_tag_span,
                }))
            }
            DirectiveType::Class => {
                // Class directive always has an expression (auto-generated for shorthand)
                let expr = expression.unwrap_or_else(|| {
                    self.make_shorthand_identifier(directive_name, colon_idx + 1 + start, name_end)
                });
                Ok(AttributeNode::ClassDirective(ClassDirective {
                    name_span,
                    expression: expr,
                    modifiers,
                    span,
                    head_span,
                    expression_tag_span,
                }))
            }
            DirectiveType::Style => unreachable!("handled above"),
            DirectiveType::Use => Ok(AttributeNode::UseDirective(UseDirective {
                name_span,
                expression,
                modifiers,
                span,
                head_span,
                expression_tag_span,
            })),
            DirectiveType::Transition => {
                Ok(AttributeNode::TransitionDirective(TransitionDirective {
                    name_span,
                    expression,
                    modifiers,
                    direction: TransitionDirection::Both,
                    span,
                    head_span,
                    expression_tag_span,
                }))
            }
            DirectiveType::In => Ok(AttributeNode::TransitionDirective(TransitionDirective {
                name_span,
                expression,
                modifiers,
                direction: TransitionDirection::In,
                span,
                head_span,
                expression_tag_span,
            })),
            DirectiveType::Out => Ok(AttributeNode::TransitionDirective(TransitionDirective {
                name_span,
                expression,
                modifiers,
                direction: TransitionDirection::Out,
                span,
                head_span,
                expression_tag_span,
            })),
            DirectiveType::Animate => Ok(AttributeNode::AnimateDirective(AnimateDirective {
                name_span,
                expression,
                modifiers,
                span,
                head_span,
                expression_tag_span,
            })),
            DirectiveType::Let => Ok(AttributeNode::LetDirective(LetDirective {
                name_span,
                expression,
                modifiers,
                span,
                head_span,
                expression_tag_span,
            })),
        }
    }

    /// Parse directive expression (the part after `=`)
    /// Returns the expression and the span of the expression tag (for comment lookup)
    ///
    /// Accepts both `{expr}` and `"{expr}"` (quoted mustache) forms.
    /// Svelte's parser accepts quoted expressions in directives; prettier strips the quotes.
    fn parse_directive_expression(&mut self) -> Result<(Expression<'arena>, Span), ParseError> {
        if self.check(TokenKind::LeftBrace) {
            // Standard form: {expr}
            let expr_tag = self.parse_expression_tag()?;
            Ok((expr_tag.expression, expr_tag.span))
        } else if self.check(TokenKind::String) {
            // Quoted mustache form: "{expr}"
            let mut parts = self.parse_attribute_value()?;
            // Must be exactly one ExpressionTag with no text parts
            match parts.as_mut_slice() {
                [AttributeValue::ExpressionTag(_)] => {
                    // Safety: slice match above confirmed exactly one ExpressionTag element
                    let Some(AttributeValue::ExpressionTag(expr_tag)) = parts.pop() else {
                        unreachable!("matched single ExpressionTag element")
                    };
                    Ok((expr_tag.expression, expr_tag.span))
                }
                _ => Err(self.error_msg(
                    "Quoted directive value must contain a single expression, e.g. \"{expr}\"",
                )),
            }
        } else {
            Err(self.error_msg("Directive value must be an expression wrapped in {}"))
        }
    }

    /// Create an identifier expression for shorthand directives (bind:value, class:class1)
    fn make_shorthand_identifier(
        &self,
        name: &str,
        start: usize,
        end: usize,
    ) -> Expression<'arena> {
        let symbol = self.interner.borrow_mut().get_or_intern(name);
        Expression::Identifier(Identifier::simple(
            symbol,
            Span {
                start: start as u32,
                end: end as u32,
            },
        ))
    }

    /// Parse a style directive (style:property={value} or style:property="value")
    /// Style directives can have expression values OR string values
    fn parse_style_directive(
        &mut self,
        name_span: Span,
        modifiers: &'arena [&'arena str],
        start: usize,
        name_end: usize,
        head_span: Span,
    ) -> Result<AttributeNode<'arena>, ParseError> {
        // Check for = (directive with value)
        let value = if self.check(TokenKind::Equals) {
            self.advance()?; // consume =

            // Style directive can have either expression {value} or string "value"
            if self.check(TokenKind::LeftBrace) {
                let expr_tag = self.parse_expression_tag()?;
                StyleDirectiveValue::ExpressionTag(expr_tag)
            } else if self.check(TokenKind::String) {
                // Parse string value like "red" or quoted mustache like "{value}"
                let mut parts = self.parse_attribute_value()?;
                // Quoted mustache "{expr}" → ExpressionTag (quotes stripped)
                match parts.as_mut_slice() {
                    [AttributeValue::ExpressionTag(_)] => {
                        // Safety: slice match above confirmed exactly one ExpressionTag element
                        let Some(AttributeValue::ExpressionTag(expr_tag)) = parts.pop() else {
                            unreachable!("matched single ExpressionTag element")
                        };
                        StyleDirectiveValue::ExpressionTag(expr_tag)
                    }
                    _ => StyleDirectiveValue::Parts(parts.into_bump_slice()),
                }
            } else if self.check(TokenKind::Identifier) {
                // Unquoted value: style:background=green
                let parts = self.parse_unquoted_attribute_value(true)?;
                StyleDirectiveValue::Parts(parts.into_bump_slice())
            } else {
                return Err(
                    self.error_msg("Style directive value must be an expression or quoted string")
                );
            }
        } else {
            // Shorthand: style:color (no value, uses variable with same name)
            StyleDirectiveValue::True
        };

        // Calculate end position
        // For ExpressionTag from quoted mustache ("{expr}"), skip past the closing quote
        let end = match &value {
            StyleDirectiveValue::ExpressionTag(et) => {
                self.end_past_optional_quote(et.span.end_usize())
            }
            StyleDirectiveValue::Parts(parts) => parts.last().map_or(name_end, |p| match p {
                AttributeValue::Text(t) => self.end_past_optional_quote(t.span.end_usize()),
                AttributeValue::ExpressionTag(et) => {
                    self.end_past_optional_quote(et.span.end_usize())
                }
            }),
            StyleDirectiveValue::True => name_end,
        };

        let span = Span {
            start: start as u32,
            end: end as u32,
        };

        Ok(AttributeNode::StyleDirective(StyleDirective {
            name_span,
            value,
            modifiers,
            span,
            head_span,
        }))
    }

    /// Parse an {@attach expr} tag inside element attributes
    ///
    /// Syntax: {@attach expression}
    ///
    /// The expression can be:
    /// - An identifier: {@attach fn}
    /// - A call expression: {@attach tooltip("hi")}
    /// - A conditional: {@attach a ? fn1 : fn2}
    /// - An arrow function: {@attach (el) => el.focus()}
    pub(crate) fn parse_attach_tag(&mut self) -> Result<AttachTag<'arena>, ParseError> {
        let start = self.current_start;

        // We're at '{@', scan forward to find the closing '}'
        // The content is: {@attach expr}
        let brace_start = self.current_start;

        let content_start = brace_start + 2; // Skip "{@"

        // Find the matching closing `}` (skips strings/comments/regex).
        let Some(content_end) = scan_to_matching_brace(self.source.as_bytes(), content_start)
        else {
            return Err(self.error_unclosed_at("{@attach} tag", start));
        };
        let end = content_end + 1; // Include the closing '}'

        // Extract content: "attach expr"
        let content = &self.source[content_start..content_end];

        // Parse: "attach expr"
        let Some(after_attach) = content.strip_prefix("attach ") else {
            return Err(self.error_expected_at("'attach' keyword", content_start));
        };
        let expr_str = after_attach.trim();

        if expr_str.is_empty() {
            return Err(self.error_msg_at("{@attach} requires an expression", content_start));
        }

        // Calculate the offset of the expression in the source
        let expr_offset = content_start + super::subslice_offset(content, expr_str);

        // Parse the expression using the TypeScript parser
        let expression = self.parse_ts_expression(expr_str, expr_offset)?;

        // Advance the lexer past the entire {@attach ...} construct
        // We need to update the lexer position to after the closing '}'
        self.advance_to_position(end)?;

        Ok(AttachTag {
            expression,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }

    /// Parse a spread attribute: {...expr}
    ///
    /// Syntax: {...expression}
    ///
    /// The expression can be:
    /// - An identifier: {...obj}
    /// - A call expression: {...getProps()}
    /// - A member expression: {...obj.nested}
    fn parse_spread_attribute(&mut self) -> Result<SpreadAttribute<'arena>, ParseError> {
        let start = self.current_start;

        // We're at '{', scan forward to find the closing '}'
        let brace_start = self.current_start;

        let content_start = brace_start + 1; // Skip "{"

        // Find the matching closing `}` (skips strings/comments/regex).
        let Some(content_end) = scan_to_matching_brace(self.source.as_bytes(), content_start)
        else {
            return Err(self.error_unclosed_at("spread attribute", start));
        };
        let end = content_end + 1; // Include the closing '}'

        // Extract content: "...expr" or " ...expr " (with whitespace)
        let content = &self.source[content_start..content_end];
        let trimmed = content.trim_start();

        // Parse: "...expr"
        let Some(after_dots) = trimmed.strip_prefix("...") else {
            return Err(self.error_expected_at("'...' in spread attribute", content_start));
        };
        let expr_str = after_dots.trim();

        if expr_str.is_empty() {
            return Err(self.error_msg_at("Spread attribute requires an expression", content_start));
        }

        // Calculate the offset of the expression in the source
        // Skip leading whitespace + "..."
        let leading_ws = content.len() - trimmed.len();
        let expr_offset = content_start + leading_ws + "...".len();

        // Parse the expression using the TypeScript parser
        let expression = self.parse_ts_expression(expr_str, expr_offset)?;

        // Advance the lexer past the entire {...} construct
        self.advance_to_position(end)?;

        Ok(SpreadAttribute {
            expression,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }

    /// Parse a shorthand attribute: {name}
    ///
    /// Syntax: {identifier}
    /// Equivalent to: name={name}
    ///
    /// The content must be a valid identifier.
    fn parse_shorthand_attribute(&mut self) -> Result<Attribute<'arena>, ParseError> {
        let start = self.current_start;

        // We're at '{', scan forward to find the closing '}'
        let brace_start = self.current_start;

        // Find the closing brace
        let content_start = brace_start + 1; // Skip "{"
        let mut pos = content_start;
        let source_bytes = self.source.as_bytes();

        // For shorthand, we don't expect nested braces - just find the closing one
        while pos < self.source.len() && source_bytes[pos] != b'}' {
            pos += 1;
        }

        if pos >= self.source.len() {
            return Err(self.error_unclosed_at("shorthand attribute", start));
        }

        // pos is now at the closing '}'
        let content_end = pos;
        let end = pos + 1; // Include the closing '}'

        // Extract content: the identifier name
        let name_str = self.source[content_start..content_end].trim();

        if name_str.is_empty() {
            return Err(
                self.error_msg_at("Shorthand attribute requires an identifier", content_start)
            );
        }

        // Validate it's a valid identifier (simple check - no spaces or special chars)
        if !name_str
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        {
            return Err(self.error_msg_at(
                &format!("Invalid shorthand attribute: '{name_str}'"),
                content_start,
            ));
        }

        // Intern the name
        let name = self.intern(name_str);

        // Create the value as an ExpressionTag containing an Identifier
        // The identifier has the same name as the attribute
        let identifier = Identifier::simple(
            name,
            Span {
                start: content_start as u32,
                end: content_end as u32,
            },
        );

        let expression_tag = ExpressionTag {
            expression: Expression::Identifier(identifier),
            span: Span {
                start: content_start as u32,
                end: content_end as u32,
            },
        };

        // Advance the lexer past the entire {name} construct
        self.advance_to_position(end)?;

        let mut value_vec = self.bvec();
        value_vec.push(AttributeValue::ExpressionTag(expression_tag));

        Ok(Attribute {
            name,
            value: Some(value_vec.into_bump_slice()),
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            name_span: Span {
                start: content_start as u32,
                end: content_end as u32,
            },
        })
    }

    fn parse_attribute_inner(
        &mut self,
        parse_expressions: bool,
    ) -> Result<Attribute<'arena>, ParseError> {
        let start = self.current_start;

        // Parse attribute name
        if !self.check(TokenKind::Identifier) {
            return Err(self.error_expected_found("attribute name"));
        }

        let name_str = self.current_value().to_string();
        let name = self.intern(&name_str);
        let name_end = self.current_end; // Save end position of name token
        self.advance()?;

        // Check for = (attribute with value)
        if self.check(TokenKind::Equals) {
            self.advance()?; // consume =

            // Parse attribute value (string or expression)
            let value = self.parse_attribute_value_inner(parse_expressions)?;

            // Find the end position from the last value part
            let value_end = if let Some(last_part) = value.last() {
                match last_part {
                    AttributeValue::Text(text) => {
                        // For quoted strings, Text span covers content only (without quotes),
                        // so skip past the closing quote. For unquoted values, the span
                        // already covers the full value (no quote to skip).
                        self.end_past_optional_quote(text.span.end_usize())
                    }
                    AttributeValue::ExpressionTag(tag) => {
                        self.end_past_optional_quote(tag.span.end_usize())
                    }
                }
            } else {
                return Err(self.error_msg("Attribute value is empty"));
            };

            Ok(Attribute {
                name,
                value: Some(value.into_bump_slice()),
                span: Span {
                    start: start as u32,
                    end: value_end as u32,
                },
                name_span: Span {
                    start: start as u32,
                    end: name_end as u32,
                },
            })
        } else {
            // Boolean attribute (no value) - ends where the name ends
            Ok(Attribute {
                name,
                value: None,
                span: Span {
                    start: start as u32,
                    end: name_end as u32,
                },
                name_span: Span {
                    start: start as u32,
                    end: name_end as u32,
                },
            })
        }
    }

    /// Parse attribute value (e.g., `"ts"`, `{expr}`, or unquoted `value`)
    /// Returns a Vec<AttributeValue> to support mixed text/expressions
    pub(crate) fn parse_attribute_value(
        &mut self,
    ) -> Result<BumpVec<'arena, AttributeValue<'arena>>, ParseError> {
        self.parse_attribute_value_inner(true)
    }

    fn parse_attribute_value_inner(
        &mut self,
        parse_expressions: bool,
    ) -> Result<BumpVec<'arena, AttributeValue<'arena>>, ParseError> {
        // Any value not starting with a quote is unquoted, read as a Svelte
        // `read_sequence` — a run of Text + {expr} chunks to the terminator regex.
        // Covers a bare identifier (`data-attr=value`), a single expression
        // (`prop={a}`), concatenations (`prop={a}{b}`, `src={a}//cdn`), and
        // slash-led paths (`href=/path`).
        if !self.check(TokenKind::String) {
            return self.parse_unquoted_attribute_value(parse_expressions);
        }

        let mut parts = self.bvec();

        // Extract string content (without quotes)
        let (token_start, token_end) = self.current_pos();

        // Remove quotes: "ts" -> ts
        let content_start = token_start + 1;
        let content_end = token_end - 1;

        // Advance past the string token now, before we start parsing expression tags
        self.advance()?;

        // For script/style tag attributes, don't parse expressions - treat as literal text
        if !parse_expressions {
            let span = Span {
                start: content_start as u32,
                end: content_end as u32,
            };
            parts.push(AttributeValue::Text(Text::new(
                span,
                TextDecoding::AttributeValue,
                span,
                self.source,
            )));
            return Ok(parts);
        }

        // Scan the quoted value as a sequence of Text and {expr} chunks. Each
        // `{expr}` is parsed by the shared `parse_expression_tag_at`, which skips
        // nested braces, strings, comments, and regex literals — so a `}` inside one
        // (`"{/* } */ x}"`, `"{f(/[}]/)}"`) doesn't desync brace matching.
        // Example: "delete {'\"'}" contains text "delete " and expression {'\"'}.
        let mut pos = content_start;
        let source_bytes = self.source.as_bytes();

        while pos < content_end {
            // Accumulate text up to the next `{`.
            let text_start = pos;
            while pos < content_end && source_bytes[pos] != b'{' {
                pos += 1;
            }
            if pos > text_start {
                let span = Span {
                    start: text_start as u32,
                    end: pos as u32,
                };
                parts.push(AttributeValue::Text(Text::new(
                    span,
                    TextDecoding::AttributeValue,
                    span,
                    self.source,
                )));
            }

            if pos < content_end && source_bytes[pos] == b'{' {
                if brace_starts_expression(source_bytes, pos) {
                    let tag = self.parse_expression_tag_at(pos)?;
                    pos = tag.span.end as usize;
                    parts.push(AttributeValue::ExpressionTag(tag));
                } else {
                    pos += 1; // literal `{`
                }
            }
        }

        // If no parts were created (empty string or quote mismatch), create empty text.
        // `raw` is empty here even when the node span covers a stray byte (e.g. a
        // literal `{`), so `raw_span` is an empty span, not the node span.
        if parts.is_empty() {
            parts.push(AttributeValue::Text(Text::new(
                Span {
                    start: content_start as u32,
                    end: content_start as u32,
                },
                TextDecoding::AttributeValue,
                Span {
                    start: content_start as u32,
                    end: content_end as u32,
                },
                self.source,
            )));
        }

        Ok(parts)
    }

    /// Parse an unquoted attribute value as a Svelte `read_sequence`.
    ///
    /// An unquoted value is a run of `Text` and `{expr}` chunks terminated by
    /// `regex_invalid_unquoted_attribute_value` — `/>` or one of whitespace, `"`,
    /// `'`, `=`, `<`, `>`, `` ` ``. So `prop={a}{b}` is one value `[{a}, {b}]`,
    /// `src={a}//cdn` is `[{a}, "//cdn"]`, and `href=/path` is `["/path"]`. A bare
    /// `/` (only `/>`) does not terminate, so protocol-relative and root-relative
    /// URLs read as plain text.
    ///
    /// We scan raw bytes because the lexer's identifier token doesn't span `/`,
    /// `:`, and the like. `Text` chunks decode with attribute-context rules to
    /// match Svelte (`decode_character_references(raw, true)`).
    ///
    /// `parse_expressions` is `false` for `<script>` / `<style>` tag attributes,
    /// where `{` is literal text and the whole value is a single `Text` chunk.
    pub(crate) fn parse_unquoted_attribute_value(
        &mut self,
        parse_expressions: bool,
    ) -> Result<BumpVec<'arena, AttributeValue<'arena>>, ParseError> {
        // `src`/`bytes` borrow the source data (lifetime `'a`), so they stay valid
        // across the `&mut self` `parse_expression_tag_at` call below.
        let src = self.source;
        let bytes = src.as_bytes();
        let start = self.current_start;
        let mut parts: BumpVec<'arena, AttributeValue<'arena>> = self.bvec();
        let mut text_start = start;
        let mut pos = start;

        let flush_text =
            |parts: &mut BumpVec<'arena, AttributeValue<'arena>>, from: usize, to: usize| {
                if to > from {
                    let span = Span {
                        start: from as u32,
                        end: to as u32,
                    };
                    parts.push(AttributeValue::Text(Text::new(
                        span,
                        TextDecoding::AttributeValue,
                        span,
                        src,
                    )));
                }
            };

        loop {
            // Terminator regex: `/>` or one of whitespace " ' = < > `
            let terminated = match bytes.get(pos).copied() {
                None => true,
                Some(b'/') => bytes.get(pos + 1) == Some(&b'>'),
                Some(
                    b' ' | b'\t' | b'\n' | b'\r' | b'\x0C' | b'"' | b'\'' | b'=' | b'<' | b'>'
                    | b'`',
                ) => true,
                Some(_) => false,
            };
            if terminated {
                flush_text(&mut parts, text_start, pos);
                break;
            }

            // An `{expr}` chunk starts a new part. Every `{` is literal text when
            // expressions are disabled (script/style tag attributes).
            if parse_expressions && bytes[pos] == b'{' && brace_starts_expression(bytes, pos) {
                flush_text(&mut parts, text_start, pos);
                // Parse the `{expr}` without disturbing the lexer (it handles nested
                // braces, strings, comments, and regex that a raw byte scan cannot);
                // we own the cursor and sync the lexer once below.
                let tag = self.parse_expression_tag_at(pos)?;
                pos = tag.span.end as usize;
                text_start = pos;
                parts.push(AttributeValue::ExpressionTag(tag));
                continue;
            }

            pos += 1;
        }

        if pos == start {
            return Err(self.error_msg("Expected attribute value"));
        }

        // Sync the lexer to the value terminator for the element parser. The loop
        // never touched the lexer, so `inside_tag` is still set (we're inside the
        // tag) and `advance_to_position` re-lexes the terminator in tag mode.
        self.advance_to_position(pos)?;

        Ok(parts)
    }
}
