// Svelte parser - main entry point for parsing .svelte files

use std::rc::Rc;

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use crate::parser::element::ParsedElement;
use tsv_lang::{ParseError, PeekData, Span};

// Module declarations
mod attribute;
mod block;
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

impl<'a> SvelteParser<'a> {
    /// Parse the root node of a Svelte file
    ///
    /// Script and style tags can appear in any order, before/after/between markup.
    /// This parser handles all orderings by parsing linearly and categorizing nodes.
    pub(crate) fn parse_root(&mut self) -> Result<Root, ParseError> {
        let mut instance = None;
        let mut module = None;
        let mut css = None;
        let mut options = None;
        let mut fragment_nodes = Vec::new();
        // Start gap tracking at lexer's initial position (accounts for BOM skip)
        let mut last_end = self.initial_position();
        let mut root_start = None;

        // Parse the entire file linearly
        while !self.check(TokenKind::Eof) {
            // Check for svelte:options tag (must come first, before other special handling)
            if self.check(TokenKind::LeftAngle) && self.is_next_tag("svelte:options")? {
                // Capture any text before the options tag
                self.capture_text_if_gap(last_end, &mut fragment_nodes)?;

                // Parse svelte:options tag
                let svelte_options = self.parse_svelte_options()?;
                last_end = svelte_options.span.end_usize();

                if options.is_some() {
                    return Err(self.error_duplicate("<svelte:options>"));
                }
                options = Some(svelte_options);
            // Check for script or style tags
            } else if self.check(TokenKind::LeftAngle) && self.is_next_tag("script")? {
                // Capture any text before the script tag
                self.capture_text_if_gap(last_end, &mut fragment_nodes)?;

                // Parse script tag
                let script = self.parse_script_tag()?;
                last_end = script.span.end_usize();

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
                            return Err(self.error_duplicate("module script"));
                        }
                        module = Some(Box::new(script));
                    }
                    ScriptContext::Default => {
                        if instance.is_some() {
                            return Err(self.error_duplicate("instance script"));
                        }
                        instance = Some(Box::new(script));
                    }
                }
            } else if self.check(TokenKind::LeftAngle) && self.is_next_tag("style")? {
                // Capture any text before the style tag
                self.capture_text_if_gap(last_end, &mut fragment_nodes)?;

                // Parse style tag
                let style = self.parse_style_tag()?;
                last_end = style.span.end_usize();

                if css.is_some() {
                    return Err(self.error_duplicate("style tag"));
                }
                css = Some(Box::new(style));
            } else {
                // Regular markup: capture text and parse elements/expressions/comments

                // Capture any leading text
                self.capture_text_if_gap(last_end, &mut fragment_nodes)?;

                if self.check(TokenKind::Comment) {
                    let comment = self.parse_comment()?;
                    last_end = comment.span.end_usize();
                    fragment_nodes.push(FragmentNode::Comment(comment));
                } else if self.check(TokenKind::LeftAngle) {
                    match self.parse_element_or_special(false)? {
                        ParsedElement::Element(elem) => {
                            last_end = elem.span.end_usize();
                            fragment_nodes.push(FragmentNode::Element(elem));
                        }
                        ParsedElement::SpecialElement(elem) => {
                            last_end = elem.span.end_usize();
                            fragment_nodes.push(FragmentNode::SpecialElement(elem));
                        }
                    }
                } else if self.check(TokenKind::LeftBrace) {
                    let tag = self.parse_brace_tag()?;
                    last_end = tag.span().end_usize();
                    fragment_nodes.push(tag);
                } else if self.check(TokenKind::BlockOpen) {
                    let block = self.parse_block()?;
                    last_end = block.span().end_usize();
                    fragment_nodes.push(block);
                } else if self.check(TokenKind::TagOpen) {
                    let tag = self.parse_template_tag()?;
                    last_end = tag.span().end_usize();
                    fragment_nodes.push(tag);
                } else {
                    return Err(self.error_msg(&format!(
                        "Unexpected token in markup: {}",
                        self.current_kind
                    )));
                }
            }
        }

        // Capture any trailing text after the last element
        // Svelte's behavior: skip trailing whitespace entirely
        if self.current_start > last_end {
            let trailing_text = &self.source[last_end..self.current_start];
            let trimmed = trailing_text.trim_end();
            if !trimmed.is_empty() {
                // Only capture up to the end of non-whitespace content
                let end_pos = last_end + trimmed.len();
                let text = self.parse_text(last_end, end_pos)?;
                fragment_nodes.push(FragmentNode::Text(text));
            }
        }

        let fragment = Fragment {
            nodes: fragment_nodes,
        };

        // Root span calculation: Skip leading/trailing whitespace-only text nodes
        //
        // Whitespace-only text at root level is formatting (blank lines, indentation), not content.
        // root.span semantically covers meaningful content; full fidelity is in fragment.nodes.
        // This matches Svelte's parser exactly and aligns with JS AST conventions.

        // root.start: First fragment node (whitespace-only text → skip, content/element/comment → include)
        if let Some(first_node) = fragment.nodes.first() {
            root_start = Some(match first_node {
                FragmentNode::Text(text) if text.data().trim().is_empty() => {
                    // Whitespace-only: skip it (start after the whitespace)
                    text.span.end_usize()
                }
                // Any node with content: include it
                _ => first_node.span().start_usize(),
            });
        }

        // root.end: Last fragment node (whitespace-only text → exclude, content/element/comment → include)
        let end = if let Some(last_node) = fragment.nodes.last() {
            match last_node {
                FragmentNode::Text(text) if text.data().trim().is_empty() => {
                    // Whitespace-only: exclude it (end before the whitespace)
                    text.span.start
                }
                // Any node with content: include it
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

        // Collect all comments from scripts and template expressions
        let mut comments = Vec::new();
        if let Some(ref script) = instance {
            for ts_comment in &script.content.comments {
                comments.push(Comment {
                    content: ts_comment.content.clone(),
                    is_block: ts_comment.is_block,
                    span: ts_comment.span,
                    emit_character_field: ts_comment.emit_character_field,
                });
            }
        }
        if let Some(ref script) = module {
            for ts_comment in &script.content.comments {
                comments.push(Comment {
                    content: ts_comment.content.clone(),
                    is_block: ts_comment.is_block,
                    span: ts_comment.span,
                    emit_character_field: ts_comment.emit_character_field,
                });
            }
        }
        // Add expression comments collected during template parsing
        // Currently extracted from: {@debug} tags (intentional divergence from prettier)
        // Future: could extend to other template tags if needed
        comments.append(&mut self.expression_comments);
        // Sort by position for consistent lookup via comments_in_range()
        comments.sort_by_key(|c| c.span.start);
        // TODO: Consider extracting CSS comments if needed for public AST

        Ok(Root {
            fragment,
            instance,
            module,
            css,
            options,
            comments,
            span: Span { start, end },
            interner: Rc::clone(&self.interner),
        })
    }

    /// Parse `<svelte:options ... />` tag
    ///
    /// svelte:options is always self-closing and has no children.
    /// It configures component behavior via attributes like `runes`, `customElement`, etc.
    fn parse_svelte_options(&mut self) -> Result<SvelteOptions, ParseError> {
        let start = self.current_start;

        // Parse opening: <svelte:options
        self.expect(TokenKind::LeftAngle)?;
        self.expect(TokenKind::Identifier)?; // "svelte:options"

        // Parse attributes
        let attributes = self.parse_attributes()?;

        // Check for self-closing: />
        let self_closing = self.check(TokenKind::Slash);
        if self_closing {
            self.advance()?; // consume /
        }

        let end = self.current_end as u32;
        self.expect(TokenKind::RightAngle)?;

        // If not self-closing, expect closing tag
        if !self_closing {
            self.expect(TokenKind::LeftAngle)?;
            self.expect(TokenKind::Slash)?;
            if !self.check(TokenKind::Identifier) || self.current_value() != "svelte:options" {
                return Err(self.error_expected("</svelte:options>"));
            }
            self.advance()?;
            self.expect(TokenKind::RightAngle)?;
        }

        Ok(SvelteOptions {
            attributes,
            span: Span {
                start: start as u32,
                end,
            },
        })
    }
}

/// Byte offset of `inner` within `outer`, derived from pointer identity.
///
/// `inner` MUST be a subslice of `outer` (the product of `trim`, `strip_prefix`, or
/// range slicing — all zero-copy). Searching by content (`str::find`) misattributes
/// the position whenever the text also occurs earlier in `outer` — `{@html html}`
/// resolved the expression to the `html` inside the keyword.
pub(crate) fn subslice_offset(outer: &str, inner: &str) -> usize {
    debug_assert!(
        (inner.as_ptr() as usize) >= (outer.as_ptr() as usize)
            && (inner.as_ptr() as usize) + inner.len() <= (outer.as_ptr() as usize) + outer.len(),
        "inner is not a subslice of outer"
    );
    (inner.as_ptr() as usize) - (outer.as_ptr() as usize)
}
