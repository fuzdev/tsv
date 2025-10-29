// Fragment and text parsing

use crate::ast::internal::*;
use crate::error::ParseError;
use crate::lexer::svelte::TokenKind;
use crate::span::Span;

use super::parser_impl::SvelteParser;

impl<'a> SvelteParser<'a> {
    /// Parse a fragment (list of template nodes)
    pub(crate) fn parse_fragment(&mut self, fragment_start: usize) -> Result<Fragment, ParseError> {
        let mut nodes = Vec::new();
        let mut last_end = fragment_start;

        // Parse template nodes until EOF or closing tag
        while !self.check(TokenKind::Eof) {
            // Check for text content between last_end and current_start
            if self.current_start > last_end {
                let text = self.parse_text(last_end, self.current_start)?;
                nodes.push(FragmentNode::Text(text));
            }

            if self.check(TokenKind::LeftAngle) {
                let element = self.parse_element()?;
                last_end = element.span.end as usize;
                nodes.push(FragmentNode::Element(element));
            } else if self.check(TokenKind::LeftBrace) {
                let expression_tag = self.parse_expression_tag()?;
                last_end = expression_tag.span.end as usize;
                nodes.push(FragmentNode::ExpressionTag(expression_tag));
            } else {
                // Unexpected token in fragment
                return Err(ParseError::InvalidSyntax {
                    message: format!("Expected element or expression tag, found {}", self.current_kind),
                    position: self.current_start,
                });
            }
        }

        Ok(Fragment { nodes })
    }

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
            span: Span { start: start as u32, end: end as u32 },
        })
    }
}
