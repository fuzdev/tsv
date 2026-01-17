// Style tag parsing

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::parser_impl::SvelteParser;

impl<'a> SvelteParser<'a> {
    pub(crate) fn parse_style_tag(&mut self) -> Result<Style, ParseError> {
        let start = self.current_start;

        // Expect <
        self.expect(TokenKind::LeftAngle)?;

        // Expect identifier "style"
        if !self.check(TokenKind::Identifier) || self.current_value() != "style" {
            return Err(self.error_expected_found("'style'"));
        }
        self.advance()?;

        // Parse attributes (e.g., lang="scss")
        // Use literal parsing - style attributes don't have expression syntax
        let attributes = self.parse_attributes_literal()?;

        // Verify we're at > and save position for content start
        if !self.check(TokenKind::RightAngle) {
            return Err(self.error_expected_found("'>'"));
        }

        // Content starts right after the >
        let content_start = self.current_end;

        // Find closing </style> tag
        let closing_pattern = b"</style>";
        let source_bytes = self.source.as_bytes();
        let mut content_end = content_start;
        let mut found_close = false;

        for i in content_start..source_bytes.len() {
            if i + closing_pattern.len() <= source_bytes.len()
                && &source_bytes[i..i + closing_pattern.len()] == closing_pattern
            {
                content_end = i;
                found_close = true;
                break;
            }
        }

        if !found_close {
            return Err(self.error_msg_at("Unterminated style tag", start));
        }

        // Recreate lexer starting from the closing tag position
        let remaining_source = &self.source[content_end..];
        let mut new_lexer = crate::lexer::Lexer::new(remaining_source);

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

        // Verify it's the closing tag: </style>
        if !self.check(TokenKind::LeftAngle) {
            return Err(self.error_expected_found("'</style>'"));
        }
        self.advance()?; // consume <

        if !self.check(TokenKind::Slash) {
            return Err(self.error_expected_found("'/'"));
        }
        self.advance()?; // consume /

        if !self.check(TokenKind::Identifier) || self.current_value() != "style" {
            return Err(self.error_expected_found("'style'"));
        }
        self.advance()?; // consume style

        // Save end position before consuming >
        let end = self.current_end;
        self.expect(TokenKind::RightAngle)?; // consume >

        // Parse CSS content
        let css_content = &self.source[content_start..content_end];
        let css_stylesheet = tsv_css::parse_embedded(css_content, content_start)?;

        Ok(Style {
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            content_span: Span {
                start: content_start as u32,
                end: content_end as u32,
            },
            attributes,
            css_stylesheet,
        })
    }
}
