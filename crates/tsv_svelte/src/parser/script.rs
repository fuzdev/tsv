// Script tag parsing

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use std::rc::Rc;
use tsv_lang::{InfallibleResolve, ParseError, SharedInterner, Span};

use super::parser_impl::SvelteParser;

impl<'a> SvelteParser<'a> {
    /// Parse a script tag: `<script lang="ts">...</script>`
    pub(crate) fn parse_script_tag(&mut self) -> Result<Script, ParseError> {
        let start = self.current_start;

        // Expect <
        self.expect(TokenKind::LeftAngle)?;

        // Expect identifier "script"
        if !self.check(TokenKind::Identifier) || self.current_value() != "script" {
            return Err(self.error_expected_found("'script'"));
        }
        self.advance()?;

        // Parse attributes (e.g., lang="ts")
        // Use literal parsing - script attributes don't have expression syntax
        let attributes = self.parse_attributes_literal()?;

        // Verify we're at > and save position for content start
        if !self.check(TokenKind::RightAngle) {
            return Err(self.error_expected_found("'>'"));
        }

        // Content starts right after the >
        // Don't advance() here because the Svelte lexer can't tokenize script content
        let content_start = self.current_end;

        // TODO(future): This is a simple pattern matching approach that doesn't handle:
        // - Nested <script> in string literals or comments: `const a = "</script>";`
        // - Template strings with </script>: `const a = \`</script>\`;`
        // For proper implementation, could use TypeScript lexer to tokenize and track
        // string/comment contexts.
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
            return Err(self.error_msg_at("Unterminated script tag", start));
        }

        // Extract script content
        let content = &self.source[content_start..content_end];

        // Parse content with TypeScript parser (shared interner + base offset)
        let program =
            tsv_ts::parse_with_interner(content, content_start, Rc::clone(&self.interner))?;

        // Reposition the lexer to the closing `</script>` tag (resumes at `<`).
        self.advance_to_position(content_end)?;

        // Verify it's the closing tag: </script>
        if !self.check(TokenKind::LeftAngle) {
            return Err(self.error_expected_found("'</script>'"));
        }
        self.advance()?; // consume <

        if !self.check(TokenKind::Slash) {
            return Err(self.error_expected_found("'/'"));
        }
        self.advance()?; // consume /

        if !self.check(TokenKind::Identifier) || self.current_value() != "script" {
            return Err(self.error_expected_found("'script'"));
        }
        self.advance()?; // consume script

        // Save end position before consuming >
        let end = self.current_end;
        self.expect(TokenKind::RightAngle)?; // consume >

        // Detect script context from attributes
        // Module scripts can be specified as:
        //   - <script module> (boolean attribute)
        //   - <script context="module"> (string attribute)
        let context = Self::detect_script_context(&attributes, self.source, &self.interner);

        Ok(Script {
            content: program,
            attributes,
            context,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }

    /// Detect whether a script is a module script based on its attributes
    fn detect_script_context(
        attributes: &[AttributeNode],
        source: &str,
        interner: &SharedInterner,
    ) -> ScriptContext {
        for attr_node in attributes {
            // Only process Attribute nodes (not AttachTag or directives)
            let attr = match attr_node {
                AttributeNode::Attribute(attr) => attr,
                AttributeNode::SpreadAttribute(_)
                | AttributeNode::AttachTag(_)
                | AttributeNode::OnDirective(_)
                | AttributeNode::BindDirective(_)
                | AttributeNode::ClassDirective(_)
                | AttributeNode::StyleDirective(_)
                | AttributeNode::UseDirective(_)
                | AttributeNode::TransitionDirective(_)
                | AttributeNode::AnimateDirective(_)
                | AttributeNode::LetDirective(_) => continue,
            };

            // Resolve attribute name to string
            let name = interner.borrow().resolve_infallible(attr.name).to_string();

            // Check for boolean module attribute: <script module>
            if name == "module" && attr.value.is_none() {
                return ScriptContext::Module;
            }

            // Check for context="module": <script context="module">
            if name == "context"
                && let Some(values) = &attr.value
                && let Some(AttributeValue::Text(text)) = values.first()
                && text.data(source) == "module"
            {
                return ScriptContext::Module;
            }
        }

        ScriptContext::Default
    }
}
