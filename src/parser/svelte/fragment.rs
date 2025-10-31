// Fragment and text parsing

use crate::ast::internal::*;
use crate::error::ParseError;
use crate::span::Span;

use super::parser_impl::SvelteParser;

impl<'a> SvelteParser<'a> {
    /// Parse text content between nodes
    ///
    /// TODO(performance): Text node allocates twice (raw + data fields are identical).
    /// See TODO_PERF.md "P1: Text Node Dual Storage" and ast/internal.rs:225 for details.
    /// Options: 1) Use Rc<str>, 2) Store only Span and extract from source during conversion,
    /// 3) Add entity decoding immediately so fields differ. Current approach trades memory
    /// for simpler conversion logic. Revisit if text nodes show up in profiling.
    pub(crate) fn parse_text(&mut self, start: usize, end: usize) -> Result<Text, ParseError> {
        let text_content = self.source[start..end].to_string();
        Ok(Text {
            raw: text_content.clone(),
            data: text_content,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }
}
