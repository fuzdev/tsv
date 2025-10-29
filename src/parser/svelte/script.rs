// Script tag parsing

use crate::ast::internal::*;
use crate::error::ParseError;
use crate::lexer::svelte::TokenKind;
use crate::span::Span;

use super::parser_impl::SvelteParser;

impl<'a> SvelteParser<'a> {
    /// Parse a script tag: `<script lang="ts">...</script>`
    pub(crate) fn parse_script_tag(&mut self) -> Result<Script, ParseError> {
        let start = self.current_start;

        // Expect <
        self.expect(TokenKind::LeftAngle)?;

        // Expect identifier "script"
        if !self.check(TokenKind::Identifier) || self.current_value() != "script" {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected 'script', found {}", self.current_kind),
                position: self.current_start,
            });
        }
        self.advance()?;

        // Parse attributes (e.g., lang="ts")
        let attributes = self.parse_attributes()?;

        // Verify we're at > and save position for content start
        if !self.check(TokenKind::RightAngle) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected '>', found {}", self.current_kind),
                position: self.current_start,
            });
        }

        // Content starts right after the >
        // Don't advance() here because the Svelte lexer can't tokenize script content
        let content_start = self.current_end;

        // TODO(future): This is a simple pattern matching approach that doesn't handle:
        // - Nested <script> in string literals or comments: `const x = "</script>";`
        // - Template strings with </script>: `const x = \`</script>\`;`
        // For proper implementation, could use TypeScript lexer to tokenize and track
        // string/comment contexts. For POC, simple pattern matching is acceptable.
        let closing_pattern = b"</script>";
        let source_bytes = self.source.as_bytes();
        let mut content_end = content_start;
        let mut found_close = false;

        // Scan for closing tag pattern
        for i in content_start..source_bytes.len() {
            // Check if we found the pattern
            if i + closing_pattern.len() <= source_bytes.len()
                && &source_bytes[i..i + closing_pattern.len()] == closing_pattern
            {
                content_end = i;
                found_close = true;
                break;
            }
        }

        if !found_close {
            return Err(ParseError::InvalidSyntax {
                message: "Unterminated script tag".to_string(),
                position: start,
            });
        }

        // Extract script content
        let content = &self.source[content_start..content_end];

        // Parse content with TypeScript parser (shared interner + base offset)
        let mut ts_parser = crate::parser::typescript::Parser::with_interner(
            content,
            content_start,
            self.interner.clone(),
        )?;
        let program = ts_parser.parse()?;

        // Recreate lexer starting from the closing tag position
        // (same pattern as expression tags - see Sprint 6 for rationale)
        let remaining_source = &self.source[content_end..];
        let mut new_lexer = crate::lexer::svelte::Lexer::new(remaining_source);

        let (token_kind, token_start, token_end) = {
            let token = new_lexer.next_token()?;
            (token.kind, token.start, token.end)
        };

        self.lexer = new_lexer;
        self.base_offset = content_end;
        self.current_kind = token_kind;
        self.current_start = content_end + token_start;
        self.current_end = content_end + token_end;
        self.peek_cache = None;

        // Verify it's the closing tag: </script>
        if !self.check(TokenKind::LeftAngle) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected '</script>', found {}", self.current_kind),
                position: self.current_start,
            });
        }
        self.advance()?; // consume <

        if !self.check(TokenKind::Slash) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected '/', found {}", self.current_kind),
                position: self.current_start,
            });
        }
        self.advance()?; // consume /

        if !self.check(TokenKind::Identifier) || self.current_value() != "script" {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected 'script', found {}", self.current_kind),
                position: self.current_start,
            });
        }
        self.advance()?; // consume script

        // Save end position before consuming >
        let (_, end_after_angle) = self.current_pos();
        self.expect(TokenKind::RightAngle)?; // consume >

        let end = end_after_angle;

        // TODO(future): Detect script context from attributes
        // For now, always use Default. Module scripts (`<script context="module">`) deferred.
        let context = ScriptContext::Default;

        Ok(Script {
            content: program,
            attributes,
            context,
            span: Span { start: start as u32, end: end as u32 },
        })
    }
}
