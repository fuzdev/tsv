// Attribute parsing

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};
use tsv_ts::ast::internal::{Expression, Identifier};

use super::parser_impl::SvelteParser;

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

impl<'a> SvelteParser<'a> {
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
    pub(crate) fn parse_attributes(&mut self) -> Result<Vec<AttributeNode>, ParseError> {
        self.parse_attributes_inner(true)
    }

    /// Parse attribute list for script/style tags where expressions are NOT parsed in quoted values
    ///
    /// Script and style tags use plain text attribute values - `{a: A}` is literal text,
    /// not an expression tag.
    pub(crate) fn parse_attributes_literal(&mut self) -> Result<Vec<AttributeNode>, ParseError> {
        self.parse_attributes_inner(false)
    }

    fn parse_attributes_inner(
        &mut self,
        parse_expressions: bool,
    ) -> Result<Vec<AttributeNode>, ParseError> {
        let mut attributes = Vec::new();

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
    ) -> Result<AttributeNode, ParseError> {
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
    ) -> Result<AttributeNode, ParseError> {
        let start = self.current_start;
        let name_end = self.current_end;
        let name_span = Span {
            start: start as u32,
            end: name_end as u32,
        };

        // Extract directive name and modifiers from: prefix:name|mod1|mod2
        let after_colon = &full_name[colon_idx + 1..];
        let mut parts = after_colon.split('|');
        let directive_name = parts.next().unwrap_or("").to_string();
        let modifiers: Vec<String> = parts.map(str::to_string).collect();

        if directive_name.is_empty() {
            return Err(self.error_msg_at(
                &format!("Directive '{}' is missing a name", &full_name[..=colon_idx]),
                start,
            ));
        }

        self.advance()?; // consume the identifier

        // Style directives accept expression OR string values, handle separately
        if directive_type == DirectiveType::Style {
            return self.parse_style_directive(
                directive_name,
                modifiers,
                start,
                name_end,
                name_span,
            );
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
                name: directive_name,
                expression,
                modifiers,
                span,
                name_span,
                expression_tag_span,
            })),
            DirectiveType::Bind => {
                // Bind directive always has an expression (auto-generated for shorthand)
                let expr = expression.unwrap_or_else(|| {
                    self.make_shorthand_identifier(&directive_name, colon_idx + 1 + start, name_end)
                });
                Ok(AttributeNode::BindDirective(BindDirective {
                    name: directive_name,
                    expression: expr,
                    span,
                    name_span,
                    expression_tag_span,
                }))
            }
            DirectiveType::Class => {
                // Class directive always has an expression (auto-generated for shorthand)
                let expr = expression.unwrap_or_else(|| {
                    self.make_shorthand_identifier(&directive_name, colon_idx + 1 + start, name_end)
                });
                Ok(AttributeNode::ClassDirective(ClassDirective {
                    name: directive_name,
                    expression: expr,
                    span,
                    name_span,
                    expression_tag_span,
                }))
            }
            DirectiveType::Style => unreachable!("handled above"),
            DirectiveType::Use => Ok(AttributeNode::UseDirective(UseDirective {
                name: directive_name,
                expression,
                span,
                name_span,
                expression_tag_span,
            })),
            DirectiveType::Transition => {
                Ok(AttributeNode::TransitionDirective(TransitionDirective {
                    name: directive_name,
                    expression,
                    modifiers,
                    direction: TransitionDirection::Both,
                    span,
                    name_span,
                    expression_tag_span,
                }))
            }
            DirectiveType::In => Ok(AttributeNode::TransitionDirective(TransitionDirective {
                name: directive_name,
                expression,
                modifiers,
                direction: TransitionDirection::In,
                span,
                name_span,
                expression_tag_span,
            })),
            DirectiveType::Out => Ok(AttributeNode::TransitionDirective(TransitionDirective {
                name: directive_name,
                expression,
                modifiers,
                direction: TransitionDirection::Out,
                span,
                name_span,
                expression_tag_span,
            })),
            DirectiveType::Animate => Ok(AttributeNode::AnimateDirective(AnimateDirective {
                name: directive_name,
                expression,
                span,
                name_span,
                expression_tag_span,
            })),
            DirectiveType::Let => Ok(AttributeNode::LetDirective(LetDirective {
                name: directive_name,
                expression,
                span,
                name_span,
                expression_tag_span,
            })),
        }
    }

    /// Parse directive expression (the part after `=`)
    /// Returns the expression and the span of the expression tag (for comment lookup)
    ///
    /// Accepts both `{expr}` and `"{expr}"` (quoted mustache) forms.
    /// Svelte's parser accepts quoted expressions in directives; prettier strips the quotes.
    fn parse_directive_expression(&mut self) -> Result<(Expression, Span), ParseError> {
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
    fn make_shorthand_identifier(&self, name: &str, start: usize, end: usize) -> Expression {
        let symbol = self.interner.borrow_mut().get_or_intern(name);
        Expression::Identifier(Identifier {
            name: symbol,
            optional: false,
            type_annotation: None,
            decorators: None,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }

    /// Parse a style directive (style:property={value} or style:property="value")
    /// Style directives can have expression values OR string values
    fn parse_style_directive(
        &mut self,
        directive_name: String,
        modifiers: Vec<String>,
        start: usize,
        name_end: usize,
        name_span: Span,
    ) -> Result<AttributeNode, ParseError> {
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
                    _ => StyleDirectiveValue::Parts(parts),
                }
            } else if self.check(TokenKind::Identifier) {
                // Unquoted value: style:background=green
                let parts = self.parse_unquoted_attribute_value()?;
                StyleDirectiveValue::Parts(parts)
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
            name: directive_name,
            value,
            modifiers,
            span,
            name_span,
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
    pub(crate) fn parse_attach_tag(&mut self) -> Result<AttachTag, ParseError> {
        let start = self.current_start;

        // We're at '{@', scan forward to find the closing '}'
        // The content is: {@attach expr}
        let brace_start = self.current_start;

        // Find the closing brace by scanning forward (handles nested braces)
        let content_start = brace_start + 2; // Skip "{@"
        let mut depth = 1;
        let mut pos = content_start;
        let source_bytes = self.source.as_bytes();

        while pos < self.source.len() && depth > 0 {
            match source_bytes[pos] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            if depth > 0 {
                pos += 1;
            }
        }

        if depth != 0 {
            return Err(self.error_unclosed_at("{@attach} tag", start));
        }

        // pos is now at the closing '}'
        let content_end = pos;
        let end = pos + 1; // Include the closing '}'

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
    fn parse_spread_attribute(&mut self) -> Result<SpreadAttribute, ParseError> {
        let start = self.current_start;

        // We're at '{', scan forward to find the closing '}'
        let brace_start = self.current_start;

        // Find the closing brace by scanning forward (handles nested braces)
        let content_start = brace_start + 1; // Skip "{"
        let mut depth = 1;
        let mut pos = content_start;
        let source_bytes = self.source.as_bytes();

        while pos < self.source.len() && depth > 0 {
            match source_bytes[pos] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            if depth > 0 {
                pos += 1;
            }
        }

        if depth != 0 {
            return Err(self.error_unclosed_at("spread attribute", start));
        }

        // pos is now at the closing '}'
        let content_end = pos;
        let end = pos + 1; // Include the closing '}'

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
        let expr_offset = content_start + leading_ws + 3; // Skip whitespace + "..."

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
    fn parse_shorthand_attribute(&mut self) -> Result<Attribute, ParseError> {
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
        let identifier = Identifier {
            name,
            optional: false,
            type_annotation: None,
            decorators: None,
            span: Span {
                start: content_start as u32,
                end: content_end as u32,
            },
        };

        let expression_tag = ExpressionTag {
            expression: Expression::Identifier(identifier),
            span: Span {
                start: content_start as u32,
                end: content_end as u32,
            },
        };

        // Advance the lexer past the entire {name} construct
        self.advance_to_position(end)?;

        Ok(Attribute {
            name,
            value: Some(vec![AttributeValue::ExpressionTag(expression_tag)]),
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

    fn parse_attribute_inner(&mut self, parse_expressions: bool) -> Result<Attribute, ParseError> {
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
                value: Some(value),
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
    pub(crate) fn parse_attribute_value(&mut self) -> Result<Vec<AttributeValue>, ParseError> {
        self.parse_attribute_value_inner(true)
    }

    fn parse_attribute_value_inner(
        &mut self,
        parse_expressions: bool,
    ) -> Result<Vec<AttributeValue>, ParseError> {
        let mut parts = Vec::new();

        // Check for expression attribute {expr}
        if self.check(TokenKind::LeftBrace) {
            let expr_tag = self.parse_expression_tag()?;
            parts.push(AttributeValue::ExpressionTag(expr_tag));
            return Ok(parts);
        }

        // Check for unquoted attribute value
        // HTML allows unquoted attribute values: any chars except whitespace, ", ', =, <, >, `
        // This handles simple identifiers (data-attr=value) and URLs (href=https://example.com)
        if self.check(TokenKind::Identifier) {
            return self.parse_unquoted_attribute_value();
        }

        // Otherwise expect string value
        if !self.check(TokenKind::String) {
            return Err(self.error_expected_found("string or expression value"));
        }

        // Extract string content (without quotes)
        let (token_start, token_end) = self.current_pos();

        // Remove quotes: "ts" -> ts
        let content_start = token_start + 1;
        let content_end = token_end - 1;

        // Advance past the string token now, before we start parsing expression tags
        self.advance()?;

        // For script/style tag attributes, don't parse expressions - treat as literal text
        if !parse_expressions {
            let text_content = self.source[content_start..content_end].to_string();
            parts.push(AttributeValue::Text(Text {
                raw: text_content,
                decoding: TextDecoding::AttributeValue,
                span: Span {
                    start: content_start as u32,
                    end: content_end as u32,
                },
            }));
            return Ok(parts);
        }

        // Scan for expression tags within the quoted value
        // Example: "delete {'\"'}" contains text "delete " and expression {'\"'}
        let mut pos = content_start;
        let source_bytes = self.source.as_bytes();

        while pos < content_end {
            // Scan for the start of an expression tag
            let text_start = pos;
            while pos < content_end && source_bytes[pos] != b'{' {
                pos += 1;
            }

            // If we found text before the expression tag (or we're at the end), create a text part
            if pos > text_start {
                let text_content = self.source[text_start..pos].to_string();
                parts.push(AttributeValue::Text(Text {
                    raw: text_content,
                    decoding: TextDecoding::AttributeValue,
                    span: Span {
                        start: text_start as u32,
                        end: pos as u32,
                    },
                }));
            }

            // If we're at an expression tag, parse it
            if pos < content_end && source_bytes[pos] == b'{' {
                // Check if this is really an expression tag (not a block tag)
                // Expression tags start with { followed by anything except # / : @
                let next_char = source_bytes.get(pos + 1);
                let is_expression_tag =
                    !matches!(next_char, Some(b'#') | Some(b'/') | Some(b':') | Some(b'@'));

                if is_expression_tag {
                    // Manually extract the expression content by scanning for the matching }
                    // NOTE: Similar brace/string tracking logic exists in the lexer (lexer.rs).
                    // The lexer tokenizes the whole string; we extract expression boundaries here.
                    let expr_start = pos + 1; // Skip the opening {
                    let mut brace_depth = 1;
                    let mut expr_end = expr_start;
                    let mut in_string = false;
                    let mut string_char = '\0';
                    let mut escape_next = false;

                    let mut i = expr_start;
                    while i < content_end && brace_depth > 0 {
                        let ch = source_bytes[i] as char;

                        if in_string && escape_next {
                            escape_next = false;
                            i += 1;
                            continue;
                        }

                        if in_string && ch == '\\' {
                            escape_next = true;
                            i += 1;
                            continue;
                        }

                        if in_string {
                            if ch == string_char {
                                in_string = false;
                            }
                        } else if ch == '"' || ch == '\'' || ch == '`' {
                            in_string = true;
                            string_char = ch;
                        } else if ch == '{' {
                            brace_depth += 1;
                        } else if ch == '}' {
                            brace_depth -= 1;
                            if brace_depth == 0 {
                                expr_end = i;
                                break;
                            }
                        }

                        i += 1;
                    }

                    if brace_depth != 0 {
                        return Err(ParseError::InvalidSyntax {
                            message: "Unclosed expression tag in attribute value".to_string(),
                            position: pos,
                            context: None,
                        });
                    }

                    // Parse the expression content
                    let expr_content = &self.source[expr_start..expr_end];
                    let (expression, comments) = tsv_ts::parse_expression_with_comments(
                        expr_content,
                        expr_start,
                        std::rc::Rc::clone(&self.interner),
                    )?;

                    // Add expression comments to the parser's collection
                    self.expression_comments.extend(comments);

                    // Create the expression tag
                    let tag_end = expr_end + 1; // Include the closing }
                    parts.push(AttributeValue::ExpressionTag(ExpressionTag {
                        expression,
                        span: Span {
                            start: pos as u32,
                            end: tag_end as u32,
                        },
                    }));

                    // Move past the expression tag
                    pos = tag_end;
                } else {
                    // Not an expression tag, treat { as literal text
                    pos += 1;
                }
            }
        }

        // If no parts were created (empty string or quote mismatch), create empty text
        if parts.is_empty() {
            parts.push(AttributeValue::Text(Text {
                raw: String::new(),
                decoding: TextDecoding::AttributeValue,
                span: Span {
                    start: content_start as u32,
                    end: content_end as u32,
                },
            }));
        }

        Ok(parts)
    }

    /// Parse an unquoted attribute value by scanning raw bytes.
    ///
    /// HTML spec: unquoted values are any chars except whitespace, `"`, `'`, `=`, `<`, `>`, `` ` ``.
    /// The lexer's identifier token only covers alphanumeric and a few special chars (`:`, `.`, `-`),
    /// so URLs like `https://example.com/path` would be split across tokens. Instead, we start from
    /// the current token position and scan raw bytes for the full unquoted value.
    pub(crate) fn parse_unquoted_attribute_value(
        &mut self,
    ) -> Result<Vec<AttributeValue>, ParseError> {
        let start = self.current_start;
        let source_bytes = self.source.as_bytes();
        let mut pos = start;

        while pos < source_bytes.len() {
            match source_bytes[pos] {
                b' ' | b'\t' | b'\n' | b'\r' | b'\x0C' // whitespace
                | b'"' | b'\'' | b'=' | b'<' | b'>' | b'`' => break,
                _ => pos += 1,
            }
        }

        if pos == start {
            return Err(self.error_msg("Expected attribute value"));
        }

        // TODO: Svelte decodes unquoted attribute values with attribute-context
        // rules (element.js read_attribute_value decodes the unquoted match too);
        // tsv has never decoded here, so `data` keeps entities (e.g. `a&amp;b`).
        // Latent public-AST divergence, unpinned by any fixture — fix fixture-first.
        let text_content = self.source[start..pos].to_string();
        let text = Text {
            raw: text_content,
            decoding: TextDecoding::Raw,
            span: Span {
                start: start as u32,
                end: pos as u32,
            },
        };

        // Advance the lexer past the raw-scanned value
        self.advance_to_position(pos)?;

        Ok(vec![AttributeValue::Text(text)])
    }
}
