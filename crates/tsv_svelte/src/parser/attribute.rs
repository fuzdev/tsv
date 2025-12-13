// Attribute parsing

use std::rc::Rc;

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
    /// Parse attribute list (e.g., `lang="ts" class="foo"`)
    /// Consumes tokens until we hit `>` or `/>`
    ///
    /// Supports:
    /// - Standard attributes: `name="value"` or `name={expr}`
    /// - Boolean attributes: `disabled`
    /// - Directives: `on:click`, `bind:value`, `class:active`, etc.
    /// - Attach tags: `{@attach expr}` (Svelte 5.29+)
    /// - Spread attributes: `{...obj}` (Svelte 3+)
    /// - Shorthand attributes: `{name}` (equivalent to `name={name}`)
    pub(crate) fn parse_attributes(&mut self) -> Result<Vec<AttributeNode>, ParseError> {
        let mut attributes = Vec::new();

        // Parse attributes until we hit > or />
        while !self.check(TokenKind::RightAngle) && !self.check(TokenKind::Slash) {
            if self.check(TokenKind::Identifier) {
                attributes.push(self.parse_attribute_or_directive()?);
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
                return Err(ParseError::InvalidSyntax {
                    message: format!(
                        "Expected attribute name or '>', found {}",
                        self.current_kind
                    ),
                    position: self.current_start,
                    context: None,
                });
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
    fn parse_attribute_or_directive(&mut self) -> Result<AttributeNode, ParseError> {
        let name_str = self.current_value().to_string();

        // Check if this is a directive (contains colon)
        if let Some(colon_idx) = name_str.find(':') {
            let prefix = &name_str[..colon_idx];
            if let Some(directive_type) = DirectiveType::from_prefix(prefix) {
                return self.parse_directive(directive_type, &name_str, colon_idx);
            }
        }

        // Not a directive, parse as regular attribute
        Ok(AttributeNode::Attribute(self.parse_attribute()?))
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

        // Extract directive name and modifiers from: prefix:name|mod1|mod2
        let after_colon = &full_name[colon_idx + 1..];
        let mut parts = after_colon.split('|');
        let directive_name = parts.next().unwrap_or("").to_string();
        let modifiers: Vec<String> = parts.map(str::to_string).collect();

        if directive_name.is_empty() {
            return Err(ParseError::InvalidSyntax {
                message: format!("Directive '{}' is missing a name", &full_name[..=colon_idx]),
                position: start,
                context: None,
            });
        }

        self.advance()?; // consume the identifier

        // Style directives accept expression OR string values, handle separately
        if directive_type == DirectiveType::Style {
            return self.parse_style_directive(directive_name, modifiers, start, name_end);
        }

        // Check for = (directive with value)
        let expression = if self.check(TokenKind::Equals) {
            self.advance()?; // consume =
            Some(self.parse_directive_expression()?)
        } else {
            None
        };

        // Calculate end position
        let end = if let Some(expr) = &expression {
            expr.span().end as usize
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
            })),
            DirectiveType::Bind => {
                // Bind directive always has an expression (auto-generated for shorthand)
                let expr = expression.unwrap_or_else(|| {
                    self.make_shorthand_identifier(&directive_name, colon_idx + 1 + start, name_end)
                });
                Ok(AttributeNode::BindDirective(BindDirective {
                    name: directive_name,
                    expression: expr,
                    modifiers,
                    span,
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
                    modifiers,
                    span,
                }))
            }
            DirectiveType::Style => unreachable!("handled above"),
            DirectiveType::Use => Ok(AttributeNode::UseDirective(UseDirective {
                name: directive_name,
                expression,
                modifiers,
                span,
            })),
            DirectiveType::Transition => {
                Ok(AttributeNode::TransitionDirective(TransitionDirective {
                    name: directive_name,
                    expression,
                    modifiers,
                    direction: TransitionDirection::Both,
                    span,
                }))
            }
            DirectiveType::In => Ok(AttributeNode::TransitionDirective(TransitionDirective {
                name: directive_name,
                expression,
                modifiers,
                direction: TransitionDirection::In,
                span,
            })),
            DirectiveType::Out => Ok(AttributeNode::TransitionDirective(TransitionDirective {
                name: directive_name,
                expression,
                modifiers,
                direction: TransitionDirection::Out,
                span,
            })),
            DirectiveType::Animate => Ok(AttributeNode::AnimateDirective(AnimateDirective {
                name: directive_name,
                expression,
                modifiers,
                span,
            })),
            DirectiveType::Let => Ok(AttributeNode::LetDirective(LetDirective {
                name: directive_name,
                expression,
                modifiers,
                span,
            })),
        }
    }

    /// Parse directive expression (the part after `=`)
    fn parse_directive_expression(&mut self) -> Result<Expression, ParseError> {
        // Expect { for expression
        if !self.check(TokenKind::LeftBrace) {
            return Err(ParseError::InvalidSyntax {
                message: "Directive value must be an expression wrapped in {}".to_string(),
                position: self.current_start,
                context: None,
            });
        }

        // Parse as expression tag and extract the expression
        let expr_tag = self.parse_expression_tag()?;
        Ok(expr_tag.expression)
    }

    /// Create an identifier expression for shorthand directives (bind:value, class:active)
    fn make_shorthand_identifier(&self, name: &str, start: usize, end: usize) -> Expression {
        let symbol = self.interner.borrow_mut().get_or_intern(name);
        Expression::Identifier(Identifier {
            name: symbol,
            optional: false,
            type_annotation: None,
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
    ) -> Result<AttributeNode, ParseError> {
        // Check for = (directive with value)
        let value = if self.check(TokenKind::Equals) {
            self.advance()?; // consume =

            // Style directive can have either expression {value} or string "value"
            if self.check(TokenKind::LeftBrace) {
                let expr_tag = self.parse_expression_tag()?;
                StyleDirectiveValue::ExpressionTag(expr_tag)
            } else if self.check(TokenKind::String) {
                // Parse string value like "red"
                let parts = self.parse_attribute_value()?;
                StyleDirectiveValue::Parts(parts)
            } else {
                return Err(ParseError::InvalidSyntax {
                    message: "Style directive value must be an expression or quoted string"
                        .to_string(),
                    position: self.current_start,
                    context: None,
                });
            }
        } else {
            // Shorthand: style:color (no value, uses variable with same name)
            StyleDirectiveValue::True
        };

        // Calculate end position
        let end = match &value {
            StyleDirectiveValue::ExpressionTag(et) => et.span.end as usize,
            StyleDirectiveValue::Parts(parts) => parts.last().map_or(name_end, |p| match p {
                AttributeValue::Text(t) => t.span.end as usize,
                AttributeValue::ExpressionTag(et) => et.span.end as usize,
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
            return Err(ParseError::InvalidSyntax {
                message: "Unclosed {@attach} tag".to_string(),
                position: start,
                context: None,
            });
        }

        // pos is now at the closing '}'
        let content_end = pos;
        let end = pos + 1; // Include the closing '}'

        // Extract content: "attach expr"
        let content = &self.source[content_start..content_end];

        // Parse: "attach expr"
        let expr_str = content
            .strip_prefix("attach ")
            .ok_or_else(|| ParseError::InvalidSyntax {
                message: "Expected 'attach' keyword".to_string(),
                position: content_start,
                context: None,
            })?
            .trim();

        if expr_str.is_empty() {
            return Err(ParseError::InvalidSyntax {
                message: "{@attach} requires an expression".to_string(),
                position: content_start,
                context: None,
            });
        }

        // Calculate the offset of the expression in the source
        let expr_offset = content_start + content.find(expr_str).unwrap_or(0);

        // Parse the expression using the TypeScript parser
        let expression =
            tsv_ts::parse_expression(expr_str, expr_offset, Rc::clone(&self.interner))?;

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
            return Err(ParseError::InvalidSyntax {
                message: "Unclosed spread attribute".to_string(),
                position: start,
                context: None,
            });
        }

        // pos is now at the closing '}'
        let content_end = pos;
        let end = pos + 1; // Include the closing '}'

        // Extract content: "...expr" or " ...expr " (with whitespace)
        let content = &self.source[content_start..content_end];
        let trimmed = content.trim_start();

        // Parse: "...expr"
        let after_dots = trimmed
            .strip_prefix("...")
            .ok_or_else(|| ParseError::InvalidSyntax {
                message: "Expected '...' in spread attribute".to_string(),
                position: content_start,
                context: None,
            })?;
        let expr_str = after_dots.trim();

        if expr_str.is_empty() {
            return Err(ParseError::InvalidSyntax {
                message: "Spread attribute requires an expression".to_string(),
                position: content_start,
                context: None,
            });
        }

        // Calculate the offset of the expression in the source
        // Skip leading whitespace + "..."
        let leading_ws = content.len() - trimmed.len();
        let expr_offset = content_start + leading_ws + 3; // Skip whitespace + "..."

        // Parse the expression using the TypeScript parser
        let expression =
            tsv_ts::parse_expression(expr_str, expr_offset, Rc::clone(&self.interner))?;

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
            return Err(ParseError::InvalidSyntax {
                message: "Unclosed shorthand attribute".to_string(),
                position: start,
                context: None,
            });
        }

        // pos is now at the closing '}'
        let content_end = pos;
        let end = pos + 1; // Include the closing '}'

        // Extract content: the identifier name
        let name_str = self.source[content_start..content_end].trim();

        if name_str.is_empty() {
            return Err(ParseError::InvalidSyntax {
                message: "Shorthand attribute requires an identifier".to_string(),
                position: content_start,
                context: None,
            });
        }

        // Validate it's a valid identifier (simple check - no spaces or special chars)
        if !name_str
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        {
            return Err(ParseError::InvalidSyntax {
                message: format!("Invalid shorthand attribute: '{name_str}'"),
                position: content_start,
                context: None,
            });
        }

        // Intern the name
        let name = self.intern(name_str);

        // Create the value as an ExpressionTag containing an Identifier
        // The identifier has the same name as the attribute
        let identifier = Identifier {
            name,
            optional: false,
            type_annotation: None,
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
        })
    }

    /// Parse a single attribute (e.g., `lang="ts"`)
    pub(crate) fn parse_attribute(&mut self) -> Result<Attribute, ParseError> {
        let start = self.current_start;

        // Parse attribute name
        if !self.check(TokenKind::Identifier) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected attribute name, found {}", self.current_kind),
                position: self.current_start,
                context: None,
            });
        }

        let name_str = self.current_value().to_string();
        let name = self.intern(&name_str);
        let name_end = self.current_end; // Save end position of name token
        self.advance()?;

        // Check for = (attribute with value)
        if self.check(TokenKind::Equals) {
            self.advance()?; // consume =

            // Parse attribute value (string or expression)
            let value = self.parse_attribute_value()?;

            // Find the end position from the last value part
            let value_end = if let Some(last_part) = value.last() {
                match last_part {
                    AttributeValue::Text(text) => {
                        // For string values, the Text span covers content only (without quotes)
                        // The attribute span must include the closing quote, so add 1 to content_end
                        // Example: type="text" → Text span is "text" (positions 13-17), token is "text" (positions 12-18)
                        text.span.end as usize + 1
                    }
                    AttributeValue::ExpressionTag(tag) => {
                        // Expression span already includes the closing } so use as-is
                        tag.span.end as usize
                    }
                }
            } else {
                return Err(ParseError::InvalidSyntax {
                    message: "Attribute value is empty".to_string(),
                    position: self.current_start,
                    context: None,
                });
            };

            Ok(Attribute {
                name,
                value: Some(value),
                span: Span {
                    start: start as u32,
                    end: value_end as u32,
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
            })
        }
    }

    /// Parse attribute value (e.g., `"ts"`, `{expr}`, or unquoted `value`)
    /// Returns a Vec<AttributeValue> to support mixed text/expressions
    pub(crate) fn parse_attribute_value(&mut self) -> Result<Vec<AttributeValue>, ParseError> {
        let mut parts = Vec::new();

        // Check for expression attribute {expr}
        if self.check(TokenKind::LeftBrace) {
            let expr_tag = self.parse_expression_tag()?;
            parts.push(AttributeValue::ExpressionTag(expr_tag));
            return Ok(parts);
        }

        // Check for unquoted attribute value (identifier)
        // HTML allows unquoted attribute values that don't contain whitespace or special chars
        if self.check(TokenKind::Identifier) {
            let (token_start, token_end) = self.current_pos();
            let text_content = self.source[token_start..token_end].to_string();

            let text = Text {
                raw: text_content.clone(),
                data: text_content,
                span: Span {
                    start: token_start as u32,
                    end: token_end as u32,
                },
            };

            self.advance()?;
            parts.push(AttributeValue::Text(text));
            return Ok(parts);
        }

        // Otherwise expect string value
        if !self.check(TokenKind::String) {
            return Err(ParseError::InvalidSyntax {
                message: format!(
                    "Expected string or expression value, found {}",
                    self.current_kind
                ),
                position: self.current_start,
                context: None,
            });
        }

        // Extract string content (without quotes)
        let (token_start, token_end) = self.current_pos();

        // Remove quotes: "ts" -> ts
        let content_start = token_start + 1;
        let content_end = token_end - 1;

        // Extract the actual text content from source
        let text_content = self.source[content_start..content_end].to_string();

        // Decode HTML entities in attribute values (is_attribute_value=true)
        let decoded = tsv_html::decode_character_references(&text_content, true);

        let text = Text {
            raw: text_content,
            data: decoded,
            span: Span {
                start: content_start as u32,
                end: content_end as u32,
            },
        };

        self.advance()?;

        parts.push(AttributeValue::Text(text));
        Ok(parts)
    }
}
