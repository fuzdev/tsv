// Fragment and text parsing

use crate::ast::internal::*;
use tsv_lang::{ParseError, Span};

use super::parser_impl::SvelteParser;

impl<'a> SvelteParser<'a> {
    /// Parse text content between nodes
    ///
    /// Stores the original source text; HTML entities decode lazily via
    /// `Text::data` with text-content rules (`TextDecoding::Fragment`).
    pub(crate) fn parse_text(&self, start: usize, end: usize) -> Result<Text, ParseError> {
        let raw = self.source[start..end].to_string();
        Ok(Text {
            raw,
            decoding: TextDecoding::Fragment,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }

    /// Parse an HTML comment: <!-- content -->
    ///
    /// The current token is TokenKind::Comment, which includes the full
    /// <!-- ... --> delimiters. We extract just the content field.
    pub(crate) fn parse_comment(&mut self) -> Result<HtmlComment, ParseError> {
        let start = self.current_start;
        let end = self.current_end;

        // Token value is the full comment including <!-- and -->
        let token_value = self.current_value();

        // Extract content: text between <!-- and -->
        let content = if token_value.len() >= 7 {
            // Remove "<!--" (4 chars) from start and "-->" (3 chars) from end
            token_value[4..token_value.len() - 3].to_string()
        } else {
            String::new()
        };

        self.advance()?;

        Ok(HtmlComment {
            content,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }
}
