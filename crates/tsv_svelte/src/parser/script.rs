// Script tag parsing

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::find_raw_text_close;
use super::parser_impl::SvelteParser;

impl<'a, 'arena> SvelteParser<'a, 'arena> {
    /// Parse a script tag: `<script lang="ts">...</script>`
    pub(crate) fn parse_script_tag(&mut self) -> Result<Script<'arena>, ParseError> {
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

        // TODO(future): `find_raw_text_close` is a raw scan that doesn't handle:
        // - Nested <script> in string literals or comments: `const a = "</script>";`
        // - Template strings with </script>: `const a = \`</script>\`;`
        // For proper implementation, could use TypeScript lexer to tokenize and track
        // string/comment contexts. (Svelte's own `read_until` scan has the same gap.)
        let content_end = find_raw_text_close(self.source.as_bytes(), content_start, b"script")
            .ok_or_else(|| self.error_msg_at("Unterminated script tag", start))?;

        // Extract script content
        let content = &self.source[content_start..content_end];

        // Parse content with TypeScript parser (base offset); the embedded
        // program shares this document's arena.
        let program = tsv_ts::parse_embedded(content, content_start, self.arena)?;

        // Reposition the lexer to the closing `</script>` tag (resumes at `<`) and
        // consume it; `find_raw_text_close` already guaranteed it exists.
        self.advance_to_position(content_end)?;
        let end = self.parse_closing_tag("script")?;

        // Detect script context from attributes
        // Module scripts can be specified as:
        //   - <script module> (boolean attribute)
        //   - <script context="module"> (string attribute)
        let context = Self::detect_script_context(&attributes, self.source);

        Ok(Script {
            content: program,
            attributes: attributes.into_bump_slice(),
            context,
            span: Span {
                start: start as u32,
                end,
            },
        })
    }

    /// Detect whether a script is a module script based on its attributes
    fn detect_script_context(attributes: &[AttributeNode<'_>], source: &str) -> ScriptContext {
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

            // Span-identity attribute name (no owned copy)
            let name = attr.name(source);

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
