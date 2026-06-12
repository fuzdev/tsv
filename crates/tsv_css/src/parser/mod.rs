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

use crate::ast::internal::{Comment, CssNode, CssStyleSheet};
use crate::lexer::{Lexer, TokenKind};
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
    pub(crate) comments: Vec<Comment>,
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
            comments: Vec::new(),
        })
    }

    /// Add a comment to the comments Vec
    pub(crate) fn add_comment(&mut self, comment: Comment) {
        self.comments.push(comment);
    }

    /// Register the current token as a comment.
    /// Assumes current token is a Comment. Extracts content without `/* */` delimiters.
    pub(crate) fn register_current_comment(&mut self) {
        debug_assert!(matches!(self.current_kind, TokenKind::Comment));
        let comment_start = self.base_offset + self.current_start;
        let comment_end = self.base_offset + self.current_end;
        // Extract content without /* */ delimiters
        let content = self.source[self.current_start + 2..self.current_end - 2].to_string();
        self.add_comment(Comment {
            content,
            is_block: true,
            span: Span {
                start: comment_start as u32,
                end: comment_end as u32,
            },
            emit_character_field: false,
        });
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
                TokenKind::Whitespace | TokenKind::Comment => continue,
                _ => return Ok(token.kind),
            }
        }
    }

    pub(crate) fn check(&self, kind: TokenKind) -> bool {
        self.current_kind == kind
    }

    pub(crate) fn expect(&mut self, kind: TokenKind) -> Result<(), ParseError> {
        if !self.check(kind) {
            return Err(self.error_expected_found(&format!("{kind:?}")));
        }
        self.advance()
    }

    /// Expect a token and capture its end position before advancing.
    /// Used for nodes whose span should end at the delimiter token.
    pub(crate) fn expect_and_capture(&mut self, kind: TokenKind) -> Result<u32, ParseError> {
        if !self.check(kind) {
            return Err(self.error_expected_found(&format!("{kind:?}")));
        }
        let end = (self.base_offset + self.current_end) as u32;
        self.advance()?;
        Ok(end)
    }

    pub(crate) fn skip_whitespace(&mut self) -> Result<(), ParseError> {
        while self.check(TokenKind::Whitespace) {
            self.advance()?;
        }
        Ok(())
    }

    /// Skip whitespace and comments (comments are not included in AST)
    pub(crate) fn skip_whitespace_and_comments(&mut self) -> Result<(), ParseError> {
        loop {
            if self.check(TokenKind::Whitespace) || matches!(&self.current_kind, TokenKind::Comment)
            {
                self.advance()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    /// Skip whitespace and **register** any comments encountered into `self.comments`.
    ///
    /// Used by structured preludes (e.g. `@import`) where comments are valid between
    /// the parsed tokens and must survive for the printer to reconstruct, even though
    /// they're stripped from the public-AST prelude string (matching Svelte). Unlike
    /// `skip_whitespace_and_comments`, this preserves the comments rather than dropping
    /// them.
    pub(crate) fn skip_whitespace_registering_comments(&mut self) -> Result<(), ParseError> {
        loop {
            if self.check(TokenKind::Whitespace) {
                self.advance()?;
            } else if matches!(&self.current_kind, TokenKind::Comment) {
                self.register_current_comment();
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

    /// Get current position (base_offset + current_start)
    #[inline]
    pub(crate) fn current_pos(&self) -> usize {
        self.base_offset + self.current_start
    }

    /// Parse the current comment token into a `Comment` and advance past it.
    /// Caller must verify `current_kind` is `TokenKind::Comment` before calling.
    pub(crate) fn parse_block_comment(&mut self) -> Result<Comment, ParseError> {
        let comment_start = self.base_offset + self.current_start;
        let comment_end = self.base_offset + self.current_end;
        let content = self.source[self.current_start + 2..self.current_end - 2].to_string();
        self.advance()?;
        self.skip_whitespace()?;
        Ok(Comment {
            content,
            is_block: true,
            span: Span {
                start: comment_start as u32,
                end: comment_end as u32,
            },
            emit_character_field: false,
        })
    }

    // Error Helpers

    /// Create an error with custom message at current position
    pub(crate) fn error_msg(&self, message: &str) -> ParseError {
        ParseError::InvalidSyntax {
            message: message.to_string(),
            position: self.current_pos(),
            context: None,
        }
    }

    /// Create an error with custom message at custom position
    pub(crate) fn error_msg_at(&self, message: &str, position: usize) -> ParseError {
        ParseError::InvalidSyntax {
            message: message.to_string(),
            position,
            context: None,
        }
    }

    /// Create an error: "Expected X"
    pub(crate) fn error_expected(&self, what: &str) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Expected {what}"),
            position: self.current_pos(),
            context: None,
        }
    }

    /// Create an error: "Expected X" at custom position
    pub(crate) fn error_expected_at(&self, what: &str, position: usize) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Expected {what}"),
            position,
            context: None,
        }
    }

    /// Create an error: "Expected X, found Y"
    pub(crate) fn error_expected_found(&self, what: &str) -> ParseError {
        let kind = &self.current_kind;
        ParseError::InvalidSyntax {
            message: format!("Expected {what}, found {kind}"),
            position: self.current_pos(),
            context: None,
        }
    }

    /// Create an error: "Expected X after 'Y'"
    pub(crate) fn error_expected_after(&self, what: &str, after: &str) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Expected {what} after '{after}'"),
            position: self.current_pos(),
            context: None,
        }
    }

    /// Create an error: "Unexpected X"
    pub(crate) fn error_unexpected(&self, what: &str) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Unexpected {what}"),
            position: self.current_pos(),
            context: None,
        }
    }

    pub(crate) fn parse(&mut self) -> Result<CssStyleSheet, ParseError> {
        let mut nodes = Vec::new();

        self.skip_whitespace()?;

        while !self.check(TokenKind::Eof) {
            // Handle comments at top level - add to comments Vec
            if matches!(&self.current_kind, TokenKind::Comment) {
                self.register_current_comment();
                self.advance()?;
                self.skip_whitespace()?;
                continue;
            }

            // Handle at-rules (@media, @keyframes, etc.)
            if self.check(TokenKind::AtSign) {
                // Top-level at-rules are not nested in rules
                let atrule = atrules::parse_atrule(self, false)?;
                nodes.push(CssNode::Atrule(atrule));
                self.skip_whitespace()?;
                continue;
            }

            // Parse rules (selector { declarations })
            let node = declarations::parse_rule(self, false)?;
            nodes.push(CssNode::Rule(node));

            self.skip_whitespace()?;
        }

        // Comments are already sorted by span.start since we add them in order during parsing

        // Build line breaks table for O(log n) line boundary lookups
        // Must add base_offset to each position since AST spans use global positions
        let base_offset_u32 = self.base_offset as u32;
        let line_breaks: Vec<u32> = tsv_lang::printing::build_line_breaks(self.source)
            .into_iter()
            .map(|pos| pos + base_offset_u32)
            .collect();

        Ok(CssStyleSheet {
            nodes,
            comments: std::mem::take(&mut self.comments),
            line_breaks,
        })
    }
}

/// Parse CSS source into AST nodes
/// base_offset is the position of the CSS source in a larger file (for embedded CSS)
pub fn parse_css(source: &str, base_offset: usize) -> Result<CssStyleSheet, ParseError> {
    let mut parser = CssParser::new(source, base_offset)?;
    parser.parse()
}
