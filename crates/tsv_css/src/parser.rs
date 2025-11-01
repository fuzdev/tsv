// CSS parser - parse CSS content from <style> tags
//
// PERFORMANCE CONSIDERATIONS:
//
// TODO: Future optimization opportunities (Phase 4+ from CSS_SPEC.md):
//
// 1. Pre-compile regex patterns (like Svelte does)
//    - Currently we match character-by-character in lexer
//    - Could use regex for faster identifier/number matching
//    - Trade-off: regex overhead vs simpler code
//
// 2. String slicing over allocation
//    - Currently allocating String for selectors, properties, values
//    - Could use string slices (&str) with lifetime management
//    - Trade-off: memory vs complexity
//
// 3. Single-pass parsing (inline tokenization like Svelte)
//    - Currently two-pass: lex then parse
//    - Could collapse into single pass
//    - Trade-off: performance vs debuggability (see lexer.rs TODO)
//
// 4. Arena allocation for AST nodes
//    - Currently using Vec and individual allocations
//    - Could use typed-arena or bumpalo for better cache locality
//    - Trade-off: speed vs memory control
//
// Recommendation: Implement features first, optimize when proven necessary.
// Profile real-world CSS files (10k+ lines) before optimizing.

use crate::ast::internal::*;
use crate::lexer::{Lexer, TokenKind};
use tsv_lang::{ParseError, Span};

#[derive(Debug)]
struct PeekData<K> {
    kind: K,
    start: usize,
    end: usize,
}

pub(crate) struct CssParser<'a> {
    source: &'a str,
    lexer: Lexer<'a>,
    current_kind: TokenKind,
    current_start: usize,
    current_end: usize,
    peek_cache: Option<PeekData<TokenKind>>,
    base_offset: usize, // Offset in full source (when parsing embedded CSS)
}

impl<'a> CssParser<'a> {
    pub(crate) fn new(source: &'a str, base_offset: usize) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(source);
        let (kind, start, end) = {
            let token = lexer.next_token()?;
            (token.kind, token.start, token.end)
        };
        Ok(Self {
            source,
            lexer,
            current_kind: kind,
            current_start: start,
            current_end: end,
            peek_cache: None,
            base_offset,
        })
    }

    fn advance(&mut self) -> Result<(), ParseError> {
        if let Some(peek) = self.peek_cache.take() {
            self.current_kind = peek.kind;
            self.current_start = peek.start;
            self.current_end = peek.end;
        } else {
            let token = self.lexer.next_token()?;
            self.current_kind = token.kind;
            self.current_start = token.start;
            self.current_end = token.end;
        }
        Ok(())
    }

    fn check(&self, kind: &TokenKind) -> bool {
        &self.current_kind == kind
    }

    fn expect(&mut self, kind: &TokenKind) -> Result<(), ParseError> {
        if !self.check(kind) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected {:?}, found {:?}", kind, self.current_kind),
                position: self.base_offset + self.current_start,
                context: None,
            });
        }
        self.advance()
    }

    fn skip_whitespace(&mut self) -> Result<(), ParseError> {
        while self.check(&TokenKind::Whitespace) {
            self.advance()?;
        }
        Ok(())
    }

    fn current_value(&self) -> &str {
        &self.source[self.current_start..self.current_end]
    }

    /// Parse CSS content into a list of CSS nodes
    fn parse(&mut self) -> Result<Vec<CssNode>, ParseError> {
        let mut nodes = Vec::new();

        self.skip_whitespace()?;

        while !self.check(&TokenKind::Eof) {
            // Handle comments at top level
            if let TokenKind::Comment(content) = &self.current_kind {
                let comment_start = self.base_offset + self.current_start;
                let comment_end = self.base_offset + self.current_end;
                let content = content.clone();

                self.advance()?;
                self.skip_whitespace()?;

                nodes.push(CssNode::Comment(CssComment {
                    content,
                    span: Span {
                        start: comment_start as u32,
                        end: comment_end as u32,
                    },
                }));
                continue;
            }

            // For now, only parse rules (selector { declarations })
            let node = self.parse_rule()?;
            nodes.push(CssNode::Rule(node));

            self.skip_whitespace()?;
        }

        Ok(nodes)
    }

    /// Parse a CSS rule: `selector { property: value; }`
    fn parse_rule(&mut self) -> Result<CssRule, ParseError> {
        let start = self.base_offset + self.current_start;
        let selector_start_in_source = self.current_start;

        // Advance past selector tokens until we hit {
        // Phase 2: TODO - parse selectors into structured AST (see CSS_SPEC.md)
        // For now, just preserve the raw selector text
        while !self.check(&TokenKind::LeftBrace) && !self.check(&TokenKind::Eof) {
            self.advance()?;
        }

        // Extract raw selector text from source (trimmed)
        let selector_end_in_source = self.current_start;
        let selector = self.source[selector_start_in_source..selector_end_in_source]
            .trim()
            .to_string();

        if selector.is_empty() {
            return Err(ParseError::InvalidSyntax {
                message: "Empty CSS selector".to_string(),
                position: start,
                context: None,
            });
        }

        let selector_end = start + selector.len();

        // Expect { and capture its start
        let block_start = self.base_offset + self.current_start;
        self.expect(&TokenKind::LeftBrace)?;
        self.skip_whitespace()?;

        // Parse declarations
        let mut declarations = Vec::new();
        while !self.check(&TokenKind::RightBrace) && !self.check(&TokenKind::Eof) {
            if self.check(&TokenKind::Identifier) {
                let decl = self.parse_declaration()?;
                declarations.push(decl);
            } else {
                // Skip unexpected tokens
                self.advance()?;
            }
            self.skip_whitespace()?;
        }

        // Expect } and capture its end position
        if !self.check(&TokenKind::RightBrace) {
            return Err(ParseError::InvalidSyntax {
                message: "Expected '}'".to_string(),
                position: self.base_offset + self.current_start,
                context: None,
            });
        }
        let block_end = self.base_offset + self.current_end;
        let end = block_end;
        self.advance()?; // consume }

        Ok(CssRule {
            selector,
            selector_span: Span {
                start: start as u32,
                end: selector_end as u32,
            },
            block_span: Span {
                start: block_start as u32,
                end: block_end as u32,
            },
            declarations,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }

    /// Parse a CSS declaration: `property: value;`
    fn parse_declaration(&mut self) -> Result<CssDeclaration, ParseError> {
        let start = self.base_offset + self.current_start;

        // Parse property
        if !self.check(&TokenKind::Identifier) {
            return Err(ParseError::InvalidSyntax {
                message: "Expected property name".to_string(),
                position: start,
                context: None,
            });
        }
        let property = self.current_value().to_string();
        self.advance()?;

        self.skip_whitespace()?;

        // Expect :
        self.expect(&TokenKind::Colon)?;
        self.skip_whitespace()?;

        // Parse value (collect tokens until ; or })
        let mut value_parts = Vec::new();
        let mut value_end = start;
        while !self.check(&TokenKind::Semicolon)
            && !self.check(&TokenKind::RightBrace)
            && !self.check(&TokenKind::Eof)
        {
            // Convert token to string representation for value
            let value_str = match &self.current_kind {
                TokenKind::Identifier => self.current_value().to_string(),
                TokenKind::String { content, quote } => format!("{}{}{}", quote, content, quote),
                TokenKind::Number(n) => n.to_string(),
                TokenKind::Percentage(n) => format!("{}%", n),
                TokenKind::Dimension(n, unit) => format!("{}{}", n, unit),
                TokenKind::Whitespace => {
                    self.advance()?;
                    continue;
                }
                TokenKind::Comment(_) => {
                    // Skip comments in values
                    self.advance()?;
                    continue;
                }
                _ => {
                    // Other tokens - include them as-is from source
                    self.current_value().to_string()
                }
            };

            value_parts.push(value_str);
            value_end = self.base_offset + self.current_end;
            self.advance()?;
        }

        let value = value_parts.join(" ");

        if value.is_empty() {
            return Err(ParseError::InvalidSyntax {
                message: "Empty CSS value".to_string(),
                position: start,
                context: None,
            });
        }

        // Declaration ends after the value, NOT including the semicolon
        let end = value_end;

        // Optionally consume semicolon (but don't include it in the declaration span)
        if self.check(&TokenKind::Semicolon) {
            self.advance()?;
        }

        Ok(CssDeclaration {
            property,
            value,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }
}

/// Parse CSS source into AST nodes
/// base_offset is the position of the CSS source in a larger file (for embedded CSS)
pub fn parse_css(source: &str, base_offset: usize) -> Result<Vec<CssNode>, ParseError> {
    let mut parser = CssParser::new(source, base_offset)?;
    parser.parse()
}
