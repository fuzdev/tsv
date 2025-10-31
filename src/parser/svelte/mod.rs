// Svelte parser - main entry point for parsing .svelte files

use crate::ast::internal::*;
use crate::error::ParseError;
use crate::lexer::svelte::TokenKind;
use crate::span::Span;

// Module declarations
mod attribute;
mod element;
mod expression_tag;
mod fragment;
mod parser_impl;
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
    ///
    /// Script and style tags can appear in any order, before/after/between markup.
    /// This parser handles all orderings by parsing linearly and categorizing nodes.
    pub(crate) fn parse_root(&mut self) -> Result<Root, ParseError> {
        let mut instance = None;
        let mut module = None;
        let mut css = None;
        let mut fragment_nodes = Vec::new();
        let mut last_end = 0;
        let mut root_start = None;

        // Parse the entire file linearly
        while !self.check(TokenKind::Eof) {
            // Check for script or style tags
            if self.check(TokenKind::LeftAngle) && self.is_next_tag("script")? {
                // Capture any text before the script tag
                self.capture_text_if_gap(last_end, &mut fragment_nodes)?;

                // Parse script tag
                let script = self.parse_script_tag()?;
                last_end = script.span.end as usize;

                // Assign to instance or module based on script context
                // Valid script configurations:
                //   - 0 scripts
                //   - 1 instance script
                //   - 1 module script
                //   - 2 scripts: exactly 1 instance + 1 module (in any order)
                // Invalid: 2 instance scripts, 2 module scripts, 3+ scripts
                match script.context {
                    ScriptContext::Module => {
                        if module.is_some() {
                            return Err(ParseError::InvalidSyntax {
                                message: "Duplicate module script found".to_string(),
                                position: self.current_start,
                                context: None,
                            });
                        }
                        module = Some(Box::new(script));
                    }
                    ScriptContext::Default => {
                        if instance.is_some() {
                            return Err(ParseError::InvalidSyntax {
                                message: "Duplicate instance script found".to_string(),
                                position: self.current_start,
                                context: None,
                            });
                        }
                        instance = Some(Box::new(script));
                    }
                }
            } else if self.check(TokenKind::LeftAngle) && self.is_next_tag("style")? {
                // Capture any text before the style tag
                self.capture_text_if_gap(last_end, &mut fragment_nodes)?;

                // Parse style tag
                let style = self.parse_style_tag()?;
                last_end = style.span.end as usize;

                if css.is_some() {
                    return Err(ParseError::InvalidSyntax {
                        message: "More than one style tag found".to_string(),
                        position: self.current_start,
                        context: None,
                    });
                }
                css = Some(Box::new(style));
            } else {
                // Regular markup: capture text and parse elements/expressions

                // Capture any leading text
                self.capture_text_if_gap(last_end, &mut fragment_nodes)?;

                if self.check(TokenKind::LeftAngle) {
                    let element = self.parse_element()?;
                    last_end = element.span.end as usize;
                    fragment_nodes.push(FragmentNode::Element(element));
                } else if self.check(TokenKind::LeftBrace) {
                    let expression_tag = self.parse_expression_tag()?;
                    last_end = expression_tag.span.end as usize;
                    fragment_nodes.push(FragmentNode::ExpressionTag(expression_tag));
                } else {
                    return Err(ParseError::InvalidSyntax {
                        message: format!("Unexpected token in markup: {}", self.current_kind),
                        position: self.current_start,
                        context: None,
                    });
                }
            }
        }

        let fragment = Fragment {
            nodes: fragment_nodes,
        };

        // root.start is determined by the first fragment node (if present):
        // - If first node is Text: use text.end (point after whitespace)
        // - If first node is Element/Expression: use node.start (point to element)
        // This overrides the complex "leading script doesn't count" logic when markup is present
        if let Some(first_node) = fragment.nodes.first() {
            match first_node {
                FragmentNode::Text(text) => {
                    root_start = Some(text.span.end as usize);
                }
                _ => {
                    root_start = Some(first_node.span().start as usize);
                }
            }
        }

        // Calculate end position following Svelte's rules:
        // - If last fragment node is Text: use text.start (exclude trailing whitespace)
        // - If last fragment node is non-Text: use node.end (include it)
        // - If no fragment nodes: use max of script/module/style
        let end = if let Some(last_node) = fragment.nodes.last() {
            match last_node {
                FragmentNode::Text(text) => text.span.start,
                _ => last_node.span().end,
            }
        } else {
            // No fragment nodes - use max of all top-level items
            let mut max_end = 0;
            if let Some(script) = &instance {
                max_end = max_end.max(script.span.end);
            }
            if let Some(script) = &module {
                max_end = max_end.max(script.span.end);
            }
            if let Some(style) = &css {
                max_end = max_end.max(style.span.end);
            }
            max_end
        };

        // Use calculated root_start (from first fragment node), or 0 if no fragments
        let start = root_start.unwrap_or(0) as u32;

        Ok(Root {
            fragment,
            instance,
            module,
            css,
            span: Span { start, end },
            interner: self.interner.clone(),
        })
    }
}
