// SvelteParser struct and helper methods

use crate::ast::internal::FragmentNode;
use crate::lexer::{Lexer, TokenKind};
use std::cell::RefCell;
use std::rc::Rc;
use string_interner::{DefaultStringInterner, DefaultSymbol};
use tsv_lang::{Comment, ParseError, Span};

use super::PeekData;

pub(crate) struct SvelteParser<'a> {
    pub(crate) source: &'a str, // Full original source
    pub(crate) lexer: Lexer<'a>,
    pub(crate) current_kind: TokenKind,
    pub(crate) current_start: usize, // Global position in full source
    pub(crate) current_end: usize,   // Global position in full source
    pub(crate) peek_cache: Option<PeekData<TokenKind>>,
    pub(crate) interner: Rc<RefCell<DefaultStringInterner>>,
    pub(crate) base_offset: usize, // Offset of lexer's source in full source
    /// TS comments collected from template expressions (e.g., {@debug /* comment */ a})
    pub(crate) expression_comments: Vec<Comment>,
}

impl<'a> SvelteParser<'a> {
    pub(crate) fn new(source: &'a str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(source);
        // Extract token data immediately to avoid keeping token alive
        let (kind, start, end) = {
            let token = lexer.next_token()?;
            (token.kind, token.start, token.end)
        };
        let interner = Rc::new(RefCell::new(DefaultStringInterner::new()));
        Ok(Self {
            source,
            lexer,
            current_kind: kind,
            current_start: start,
            current_end: end,
            peek_cache: None,
            interner,
            base_offset: 0,
            expression_comments: Vec::new(),
        })
    }

    pub(crate) fn advance(&mut self) -> Result<(), ParseError> {
        if let Some(peek) = self.peek_cache.take() {
            self.current_kind = peek.kind;
            self.current_start = peek.start;
            self.current_end = peek.end;
        } else {
            let token = self.lexer.next_token()?;
            self.current_kind = token.kind;
            self.current_start = self.base_offset + token.start;
            self.current_end = self.base_offset + token.end;
        }
        Ok(())
    }

    pub(crate) fn intern(&self, s: &str) -> DefaultSymbol {
        self.interner.borrow_mut().get_or_intern(s)
    }

    pub(crate) fn current_pos(&self) -> (usize, usize) {
        (self.current_start, self.current_end)
    }

    pub(crate) fn current_value(&self) -> &str {
        // current_start/end are global, so use them directly
        &self.source[self.current_start..self.current_end]
    }

    pub(crate) fn check(&self, kind: TokenKind) -> bool {
        self.current_kind == kind
    }

    pub(crate) fn expect(&mut self, kind: TokenKind) -> Result<(), ParseError> {
        if !self.check(kind) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected {}, found {}", kind, self.current_kind),
                position: self.current_start,
                context: None,
            });
        }
        self.advance()
    }

    /// Check if the next tag matches the given name (e.g., "script", "style")
    /// Returns true if we're at `<tagname`, false otherwise
    /// Does not allocate - compares directly against source
    pub(crate) fn is_next_tag(&mut self, tag_name: &str) -> Result<bool, ParseError> {
        if !self.check(TokenKind::LeftAngle) {
            return Ok(false);
        }

        // Peek at next token
        if self.peek_cache.is_none() {
            let token = self.lexer.next_token()?;
            self.peek_cache = Some(PeekData::new(
                token.kind,
                self.base_offset + token.start,
                self.base_offset + token.end,
            ));
        }

        if let Some(peek) = &self.peek_cache
            && peek.kind == TokenKind::Identifier
        {
            // Compare directly without allocating
            let value = &self.source[peek.start..peek.end];
            return Ok(value == tag_name);
        }

        Ok(false)
    }

    /// Peek at the next token to check if it matches the given kind
    /// Does not consume current token or advance parser
    /// Returns true if next token matches kind, false otherwise
    pub(crate) fn is_next_token(&mut self, kind: TokenKind) -> Result<bool, ParseError> {
        // Populate peek cache if not already cached
        if self.peek_cache.is_none() {
            let token = self.lexer.next_token()?;
            self.peek_cache = Some(PeekData::new(
                token.kind,
                self.base_offset + token.start,
                self.base_offset + token.end,
            ));
        }

        Ok(self.peek_cache.as_ref().is_some_and(|p| p.kind == kind))
    }

    /// Parse a text node if there's a gap between the last position and current position.
    /// The Svelte lexer skips whitespace, so gaps represent text/whitespace content.
    pub(crate) fn capture_text_if_gap(
        &self,
        last_end: usize,
        nodes: &mut Vec<FragmentNode>,
    ) -> Result<(), ParseError> {
        if self.current_start > last_end {
            let text = self.parse_text(last_end, self.current_start)?;
            nodes.push(FragmentNode::Text(text));
        }
        Ok(())
    }

    /// Advance the lexer to a specific position in the source.
    /// Used when we've manually scanned ahead (e.g., for {@attach} parsing).
    /// Preserves the current `inside_tag` state for correct tokenization.
    pub(crate) fn advance_to_position(&mut self, pos: usize) -> Result<(), ParseError> {
        // Save the inside_tag state before creating new lexer
        let was_inside_tag = self.lexer.inside_tag;

        // Reset the lexer to start from the new position
        self.lexer = Lexer::new_at(&self.source[pos..], pos);
        self.base_offset = pos;
        self.peek_cache = None;

        // Restore inside_tag state
        self.lexer.inside_tag = was_inside_tag;

        // Get the next token at the new position
        let token = self.lexer.next_token()?;
        self.current_kind = token.kind;
        self.current_start = self.base_offset + token.start;
        self.current_end = self.base_offset + token.end;

        Ok(())
    }

    /// Extract TS comments from content and add them to expression_comments.
    ///
    /// Scans for `/* ... */` block comments and `// ...` line comments.
    /// Returns content with comments replaced by spaces (preserving positions).
    ///
    /// # Arguments
    /// * `content` - The content to scan for comments
    /// * `base_offset` - Offset in the full source where this content starts
    ///
    /// # Returns
    /// Content with comments replaced by equivalent whitespace
    pub(crate) fn extract_ts_comments(&mut self, content: &str, base_offset: usize) -> String {
        let mut result = content.to_string();
        let bytes = content.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                // Block comment: /* ... */
                let start = i;
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 < bytes.len() {
                    i += 2; // Skip */
                }
                let end = i;

                // Extract comment content (without /* */)
                let comment_content = &content[start + 2..end.saturating_sub(2)];
                self.expression_comments.push(Comment {
                    content: comment_content.to_string(),
                    is_block: true,
                    span: Span {
                        start: (base_offset + start) as u32,
                        end: (base_offset + end) as u32,
                    },
                });

                // Replace comment with spaces in result
                result.replace_range(start..end, &" ".repeat(end - start));
            } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
                // Line comment: // ...
                let start = i;
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                let end = i;

                // Extract comment content (without //)
                let comment_content = &content[start + 2..end];
                self.expression_comments.push(Comment {
                    content: comment_content.to_string(),
                    is_block: false,
                    span: Span {
                        start: (base_offset + start) as u32,
                        end: (base_offset + end) as u32,
                    },
                });

                // Replace comment with spaces in result
                result.replace_range(start..end, &" ".repeat(end - start));
            } else if bytes[i] == b'"' || bytes[i] == b'\'' || bytes[i] == b'`' {
                // Skip strings to avoid matching // or /* inside them
                let quote = bytes[i];
                i += 1;
                while i < bytes.len() && bytes[i] != quote {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        i += 2; // Skip escaped char
                    } else {
                        i += 1;
                    }
                }
                if i < bytes.len() {
                    i += 1; // Skip closing quote
                }
            } else {
                i += 1;
            }
        }

        result
    }
}
