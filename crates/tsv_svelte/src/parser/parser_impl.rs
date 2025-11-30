// SvelteParser struct and helper methods

use crate::ast::internal::FragmentNode;
use crate::lexer::{Lexer, TokenKind};
use std::cell::RefCell;
use std::rc::Rc;
use string_interner::{DefaultStringInterner, DefaultSymbol};
use tsv_lang::ParseError;

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

        Ok(self
            .peek_cache
            .as_ref()
            .is_some_and(|p| p.kind == kind))
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
}
