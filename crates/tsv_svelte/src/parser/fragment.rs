// Fragment and text parsing

use crate::ast::internal::*;
use tsv_lang::{ParseError, Span};

use super::parser_impl::SvelteParser;

/// Decode HTML entities in text content
///
/// Uses the comprehensive HTML5 entity decoder from tsv_html which supports:
/// - All 2,231 named entities from HTML5 spec (&lt;, &copy;, &euro;, etc.)
/// - Numeric decimal entities (&#65;, &#8364;, &#128169;)
/// - Numeric hex entities (&#x41;, &#X41;, &#x1F4A9;)
///
/// The `is_attribute_value` parameter is `false` for text content, meaning all
/// valid entities are decoded (no special semicolon rules for attribute context).
fn decode_html_entities(raw: &str) -> String {
    tsv_html::decode_character_references(raw, false)
}

impl<'a> SvelteParser<'a> {
    /// Parse text content between nodes
    ///
    /// The `raw` field stores the original text from source.
    /// The `data` field stores the decoded text with HTML entities converted.
    pub(crate) fn parse_text(&self, start: usize, end: usize) -> Result<Text, ParseError> {
        let raw = self.source[start..end].to_string();
        let data = decode_html_entities(&raw);
        Ok(Text {
            raw,
            data,
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
