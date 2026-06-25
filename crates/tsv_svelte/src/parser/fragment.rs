// Fragment and text parsing

use crate::ast::internal::*;
use tsv_lang::{ParseError, Span};

use super::parser_impl::SvelteParser;

impl<'a, 'arena> SvelteParser<'a, 'arena> {
    /// Parse text content between nodes
    ///
    /// Stores the original source text; HTML entities decode lazily via
    /// `Text::data` with text-content rules (`TextDecoding::Fragment`).
    pub(crate) fn parse_text(&self, start: usize, end: usize) -> Result<Text, ParseError> {
        let span = Span {
            start: start as u32,
            end: end as u32,
        };
        Ok(Text {
            raw_span: span,
            decoding: TextDecoding::Fragment,
            span,
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

        // Content span: text between <!-- and --> (strip "<!--" = 4, "-->" = 3),
        // a pure source sub-slice recovered via `HtmlComment::content`.
        let content_span = if token_value.len() >= 7 {
            Span {
                start: (start + 4) as u32,
                end: (end - 3) as u32,
            }
        } else {
            Span {
                start: start as u32,
                end: start as u32,
            }
        };

        self.advance()?;

        Ok(HtmlComment {
            content_span,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }
}
