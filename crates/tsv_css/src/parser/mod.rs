// CSS parser - parse CSS content from <style> tags
//
// PERFORMANCE CONSIDERATIONS:
//
// TODO: Future optimization opportunities:
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

mod atrules;
mod attributes;
mod declarations;
mod pseudo;
mod selectors;
mod value;

use crate::ast::internal::{CssComment, CssNode, CssStyleSheet};
use crate::lexer::{Lexer, TokenKind};
use std::collections::HashMap;
use tsv_lang::{ParseError, PeekData, Span};

pub(crate) struct CssParser<'a> {
    source: &'a str,
    lexer: Lexer<'a>,
    pub(crate) current_kind: TokenKind,
    pub(crate) current_start: usize,
    pub(crate) current_end: usize,
    current_decoded: Option<String>, // Decoded value for current token (e.g., identifier escapes)
    peek_cache: Option<PeekData<TokenKind>>,
    base_offset: usize, // Offset in full source (when parsing embedded CSS)
    pub(crate) value_comments: HashMap<u32, Vec<CssComment>>, // Side table for property value comments
}

impl<'a> CssParser<'a> {
    pub(crate) fn new(source: &'a str, base_offset: usize) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(source);
        let (kind, start, end, decoded) = {
            let token = lexer.next_token()?;
            (token.kind, token.start, token.end, token.decoded)
        };
        Ok(Self {
            source,
            lexer,
            current_kind: kind,
            current_start: start,
            current_end: end,
            current_decoded: decoded,
            peek_cache: None,
            base_offset,
            value_comments: HashMap::new(),
        })
    }

    pub(crate) fn advance(&mut self) -> Result<(), ParseError> {
        if let Some(peek) = self.peek_cache.take() {
            self.current_kind = peek.kind;
            self.current_start = peek.start;
            self.current_end = peek.end;
            self.current_decoded = None; // Peek cache doesn't store decoded (not needed yet)
        } else {
            let token = self.lexer.next_token()?;
            self.current_kind = token.kind;
            self.current_start = token.start;
            self.current_end = token.end;
            self.current_decoded = token.decoded;
        }
        Ok(())
    }

    /// Peek at the next token without consuming it.
    /// Result is cached so repeated peeks are efficient.
    pub(crate) fn peek(&mut self) -> Result<&TokenKind, ParseError> {
        if self.peek_cache.is_none() {
            let token = self.lexer.next_token()?;
            self.peek_cache = Some(PeekData::new(token.kind, token.start, token.end));
        }
        // peek_cache is guaranteed Some after the if block above
        match &self.peek_cache {
            Some(data) => Ok(&data.kind),
            None => unreachable!("peek_cache was just populated"),
        }
    }

    /// Peek past whitespace and comments to find the next significant token.
    /// This creates a temporary lexer to look ahead without modifying parser state.
    /// Used for disambiguating declarations vs nested rules.
    pub(crate) fn peek_past_whitespace(&self) -> Result<TokenKind, ParseError> {
        // Create a temporary lexer from current position
        let remaining = &self.source()[self.current_end..];
        let mut temp_lexer = Lexer::new(remaining);

        // Skip whitespace and comments
        loop {
            let token = temp_lexer.next_token()?;
            match &token.kind {
                TokenKind::Whitespace | TokenKind::Comment(_) => continue,
                _ => return Ok(token.kind),
            }
        }
    }

    pub(crate) fn check(&self, kind: &TokenKind) -> bool {
        &self.current_kind == kind
    }

    pub(crate) fn expect(&mut self, kind: &TokenKind) -> Result<(), ParseError> {
        if !self.check(kind) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected {:?}, found {:?}", kind, self.current_kind),
                position: self.base_offset + self.current_start,
                context: None,
            });
        }
        self.advance()
    }

    /// Expect a token and capture its end position before advancing.
    /// Used for nodes whose span should end at the delimiter token.
    pub(crate) fn expect_and_capture(&mut self, kind: &TokenKind) -> Result<u32, ParseError> {
        if !self.check(kind) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected {:?}, found {:?}", kind, self.current_kind),
                position: self.base_offset + self.current_start,
                context: None,
            });
        }
        let end = (self.base_offset + self.current_end) as u32;
        self.advance()?;
        Ok(end)
    }

    pub(crate) fn skip_whitespace(&mut self) -> Result<(), ParseError> {
        while self.check(&TokenKind::Whitespace) {
            self.advance()?;
        }
        Ok(())
    }

    /// Skip whitespace and comments (comments are not included in AST)
    pub(crate) fn skip_whitespace_and_comments(&mut self) -> Result<(), ParseError> {
        loop {
            if self.check(&TokenKind::Whitespace)
                || matches!(&self.current_kind, TokenKind::Comment(_))
            {
                self.advance()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    /// Get the current token's value from source (for most tokens)
    pub(crate) fn current_value(&self) -> &str {
        &self.source[self.current_start..self.current_end]
    }

    /// Get the decoded identifier value (for Identifier tokens only)
    /// Returns None if not an identifier or no decoded value available
    pub(crate) fn current_identifier(&self) -> Option<&str> {
        self.current_decoded.as_deref()
    }

    pub(crate) fn current_start(&self) -> usize {
        self.current_start
    }

    pub(crate) fn base_offset(&self) -> usize {
        self.base_offset
    }

    pub(crate) fn source(&self) -> &'a str {
        self.source
    }

    pub(crate) fn parse(&mut self) -> Result<CssStyleSheet, ParseError> {
        let mut nodes = Vec::new();

        self.skip_whitespace()?;

        while !self.check(&TokenKind::Eof) {
            // Handle comments at top level
            if let TokenKind::Comment(content) = &self.current_kind {
                let comment_start = self.base_offset() + self.current_start;
                let comment_end = self.base_offset() + self.current_end;
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

            // Handle at-rules (@media, @keyframes, etc.)
            if self.check(&TokenKind::AtSign) {
                // Top-level at-rules are not nested in rules
                let atrule = atrules::parse_atrule(self, false)?;
                nodes.push(CssNode::Atrule(atrule));
                self.skip_whitespace()?;
                continue;
            }

            // Parse rules (selector { declarations })
            let node = declarations::parse_rule(self)?;
            nodes.push(CssNode::Rule(node));

            self.skip_whitespace()?;
        }

        Ok(CssStyleSheet {
            nodes,
            value_comments: self.value_comments.clone(),
        })
    }
}

/// Parse CSS source into AST nodes
/// base_offset is the position of the CSS source in a larger file (for embedded CSS)
pub fn parse_css(source: &str, base_offset: usize) -> Result<CssStyleSheet, ParseError> {
    let mut parser = CssParser::new(source, base_offset)?;
    parser.parse()
}
