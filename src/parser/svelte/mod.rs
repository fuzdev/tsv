// Svelte parser - main entry point for parsing .svelte files

use crate::ast::internal::*;
use crate::error::ParseError;
use crate::span::Span;

// Module declarations
mod parser_impl;
mod attribute;
mod element;
mod expression_tag;
mod fragment;
mod script;
mod style;

// Re-export parser implementation
use parser_impl::SvelteParser;

/// Parse a Svelte file and return a Root AST node
pub fn parse_svelte(source: &str) -> Result<Root, ParseError> {
    let mut parser = SvelteParser::new(source)?;
    parser.parse_root()
}

/// PeekData struct used by parser helpers
pub(crate) struct PeekData<T> {
    pub(crate) kind: T,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl<'a> SvelteParser<'a> {
    /// Parse the root node of a Svelte file
    pub(crate) fn parse_root(&mut self) -> Result<Root, ParseError> {
        let start = 0;
        let mut fragment_nodes = Vec::new();

        // Check for script tag first
        let instance = if self.is_next_tag("script")? {
            Some(Box::new(self.parse_script_tag()?))
        } else {
            None
        };

        // After script, capture any text before style tag
        let fragment_start_1 = if let Some(script) = &instance {
            script.span.end as usize
        } else {
            0
        };

        // Check for style tag and capture text between script and style
        let css = if self.is_next_tag("style")? {
            // Capture text between script end and style start
            if self.current_start > fragment_start_1 {
                let text = self.parse_text(fragment_start_1, self.current_start)?;
                fragment_nodes.push(FragmentNode::Text(text));
            }
            Some(Box::new(self.parse_style_tag()?))
        } else {
            None
        };

        // Determine where the main fragment starts (after script and/or style)
        let fragment_start = if let Some(style) = &css {
            style.span.end as usize
        } else {
            fragment_start_1
        };

        // Parse the template fragment (after style, or after script if no style)
        let mut rest_of_fragment = self.parse_fragment(fragment_start)?;
        fragment_nodes.append(&mut rest_of_fragment.nodes);

        let fragment = Fragment { nodes: fragment_nodes };

        // Calculate end position
        let end = if let Some(last_node) = fragment.nodes.last() {
            last_node.span().end
        } else if let Some(style) = &css {
            style.span.end
        } else if let Some(script) = &instance {
            script.span.end
        } else {
            0 // Empty file
        };

        Ok(Root {
            fragment,
            instance,
            module: None,
            css,
            span: Span { start, end },
            interner: self.interner.clone(),
        })
    }
}
