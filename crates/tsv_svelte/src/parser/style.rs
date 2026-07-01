// Style tag parsing

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::find_raw_text_close;
use super::parser_impl::SvelteParser;

impl<'a, 'arena> SvelteParser<'a, 'arena> {
    pub(crate) fn parse_style_tag(&mut self) -> Result<Style<'arena>, ParseError> {
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

        // Find closing </style> tag (raw scan, `/<\/style\s*>/`; see
        // `find_raw_text_close` for the shared string-context limitation).
        let content_end = find_raw_text_close(self.source.as_bytes(), content_start, b"style")
            .ok_or_else(|| self.error_msg_at("Unterminated style tag", start))?;

        // Reposition the lexer to the closing `</style>` tag (resumes at `<`).
        self.advance_to_position(content_end)?;

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

        // Parse CSS content (shares the document's bump arena)
        let css_content = &self.source[content_start..content_end];
        let css_stylesheet = tsv_css::parse_embedded(css_content, content_start, self.arena)?;

        Ok(Style {
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            content_span: Span {
                start: content_start as u32,
                end: content_end as u32,
            },
            attributes: attributes.into_bump_slice(),
            css_stylesheet,
        })
    }
}
