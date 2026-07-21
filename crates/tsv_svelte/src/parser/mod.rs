// Svelte parser - main entry point for parsing .svelte files

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use crate::parser::element::ParsedElement;
use tsv_lang::source_scan::{TriviaProfile, skip_template_literal, skip_trivia};
use tsv_lang::{ParseError, Span};

// Module declarations
mod attribute;
mod block;
mod element;
mod expression_tag;
mod fragment;
mod parser_impl;
mod script;
mod style;
mod tag;

// Re-export parser implementation
use parser_impl::SvelteParser;

/// Parse a Svelte file and return a Root AST node.
///
/// `arena` owns the entire parsed graph (the template AST plus the embedded TS
/// `<script>`/`{expr}` ASTs, which share this one `Bump`); the returned
/// `Root<'arena>` borrows from it.
pub fn parse_svelte<'arena>(
    source: &str,
    arena: &'arena bumpalo::Bump,
) -> Result<Root<'arena>, ParseError> {
    let mut parser = SvelteParser::new(source, arena)?;
    parser.parse_root()
}

impl<'a, 'arena> SvelteParser<'a, 'arena> {
    /// Parse the root node of a Svelte file
    ///
    /// Script and style tags can appear in any order, before/after/between markup.
    /// This parser handles all orderings by parsing linearly and categorizing nodes.
    pub(crate) fn parse_root(&mut self) -> Result<Root<'arena>, ParseError> {
        let mut instance = None;
        let mut module = None;
        let mut css = None;
        let mut options = None;
        let mut fragment_nodes = self.bvec();
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
                        module = Some(self.alloc(script));
                    }
                    ScriptContext::Default => {
                        if instance.is_some() {
                            return Err(self.error_duplicate("instance script"));
                        }
                        instance = Some(self.alloc(script));
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
                css = Some(self.alloc(style));
            } else {
                // Regular markup: capture text and parse elements/expressions/comments

                // Capture any leading text
                self.capture_text_if_gap(last_end, &mut fragment_nodes)?;

                if self.check(TokenKind::Comment) {
                    let comment = self.parse_comment()?;
                    last_end = comment.span.end_usize();
                    fragment_nodes.push(FragmentNode::Comment(comment));
                } else if self.check(TokenKind::LeftAngle) {
                    match self.parse_element_or_special()? {
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
            nodes: fragment_nodes.into_bump_slice(),
        };

        // Root span calculation: Skip leading/trailing whitespace-only text nodes
        //
        // Whitespace-only text at root level is formatting (blank lines, indentation), not content.
        // root.span semantically covers meaningful content; full fidelity is in fragment.nodes.
        // This matches Svelte's parser exactly and aligns with JS AST conventions.

        // root.start: First fragment node (whitespace-only text → skip, content/element/comment → include)
        if let Some(first_node) = fragment.nodes.first() {
            root_start = Some(match first_node {
                FragmentNode::Text(text) if text.data(self.source).trim().is_empty() => {
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
                FragmentNode::Text(text) if text.data(self.source).trim().is_empty() => {
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
        if let Some(script) = instance {
            comments.extend_from_slice(script.content.comments);
        }
        if let Some(script) = module {
            comments.extend_from_slice(script.content.comments);
        }
        // Add expression comments collected during template parsing
        // Currently extracted from: {@debug} tags (intentional divergence from prettier)
        // Future: could extend to other template tags if needed
        comments.append(&mut self.expression_comments);
        // Sort by position for consistent lookup via comments_to_emit_in_range()
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
        })
    }

    /// Parse `<svelte:options ... />` tag
    ///
    /// svelte:options is always self-closing and has no children.
    /// It configures component behavior via attributes like `runes`, `customElement`, etc.
    fn parse_svelte_options(&mut self) -> Result<SvelteOptions<'arena>, ParseError> {
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
            attributes: attributes.into_bump_slice(),
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

/// Find the closing tag of a raw-text `<script>` / `<style>` at or after `from` — the
/// byte offset of the `<` in `</tag…>`, or `None` if none exists. `tag` is the bare
/// lowercase name (e.g. `b"script"`), matched as a full token: the byte after it must
/// be whitespace or `>`, so `</scripts>` never matches.
///
/// A raw byte scan — like Svelte's `read_until(regex_closing_script_tag)` it matches the
/// first *textual* `</tag…>` regardless of string/comment context, so a literal
/// `"</script>"` in the body closes the block for both parsers (the shared, documented
/// raw-text limitation).
///
/// Two variants mirror Svelte's own two code paths — **do not collapse them:**
///
/// - `find_raw_text_close` tolerates whitespace before `>` (`/<\/tag\s*>/`), for the
///   **top-level** component `<script>` / `<style>` Svelte reads via `read_script` /
///   `read_style` — so `</script  >` and `</style\n>` parse.
/// - `find_exact_tag_close` requires an exact `</tag>`, for a **nested** `<script>` /
///   `<style>` in markup, which Svelte reads through its generic element parser: it
///   rejects `</script  >` there, and tsv matches that (verified parity). Migrating it
///   to the whitespace-tolerant scan would introduce a divergence.
pub(crate) fn find_raw_text_close(bytes: &[u8], from: usize, tag: &[u8]) -> Option<usize> {
    find_tag_close(bytes, from, tag, true)
}

/// Exact-`</tag>` sibling of `find_raw_text_close` (no whitespace before `>`); see there
/// for the whitespace-tolerant-vs-exact split and why both exist.
pub(crate) fn find_exact_tag_close(bytes: &[u8], from: usize, tag: &[u8]) -> Option<usize> {
    find_tag_close(bytes, from, tag, false)
}

/// If a whitespace/attribute-tolerant closing tag `</tag…>` starts **exactly** at byte
/// `i`, return `(lt, gt)` — the `<` offset (`== i`) and the closing `>` offset;
/// otherwise `None`.
///
/// Ports Svelte's RCDATA close `regex_closing_textarea_tag = /<\/textarea(\s[^>]*)?>/iy`
/// (`1-parse/state/element.js`): sticky at `i`, case-insensitive on the tag name, then
/// either `>` immediately or **one** whitespace followed by any run of non-`>` up to the
/// first `>`. The RCDATA reader calls it at every content byte (the sticky-at-each-index
/// model of Svelte's `read_sequence` `done()` predicate).
///
/// Distinct from the raw-text finders above on three counts, so it can't reuse them: it
/// tolerates attributes on the close (`</textarea data-x >`, not just `\s*>`), it matches
/// the tag name case-insensitively (`</TEXTAREA>`), and it reports the `>` offset (the
/// reader needs it for the element end).
pub(crate) fn rcdata_close_at(bytes: &[u8], i: usize, tag: &[u8]) -> Option<(usize, usize)> {
    if bytes.get(i) != Some(&b'<') || bytes.get(i + 1) != Some(&b'/') {
        return None;
    }
    let name_start = i + 2;
    let rest = bytes.get(name_start..)?;
    if rest.len() < tag.len() || !rest[..tag.len()].eq_ignore_ascii_case(tag) {
        return None;
    }
    let after_name = name_start + tag.len();
    match bytes.get(after_name) {
        // `</textarea>` — closes directly.
        Some(b'>') => Some((i, after_name)),
        // `</textarea\s[^>]*>` — the required whitespace, then non-`>`* to the first `>`.
        Some(b) if b.is_ascii_whitespace() => {
            let mut j = after_name + 1;
            while let Some(&c) = bytes.get(j) {
                if c == b'>' {
                    return Some((i, j));
                }
                j += 1;
            }
            None
        }
        // Any other trailing byte (`</textareax`, `</textarea/`) is not a close.
        _ => None,
    }
}

/// Shared core of `find_raw_text_close` / `find_exact_tag_close`; `allow_ws_before_gt`
/// selects the whitespace-tolerant (`\s*>`) vs exact (`>`) close. Callers pick a named
/// wrapper so the boolean never reaches a call site.
fn find_tag_close(
    bytes: &[u8],
    from: usize,
    tag: &[u8],
    allow_ws_before_gt: bool,
) -> Option<usize> {
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'<'
            && bytes.get(i + 1) == Some(&b'/')
            && bytes.get(i + 2..).is_some_and(|rest| rest.starts_with(tag))
        {
            let mut j = i + 2 + tag.len();
            if allow_ws_before_gt {
                while bytes.get(j).is_some_and(u8::is_ascii_whitespace) {
                    j += 1;
                }
            }
            if bytes.get(j) == Some(&b'>') {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Find the first `target` byte in `bytes[start..end]` that sits at bracket depth 0
/// (outside `()`/`[]`/`{}`) and outside trivia (comments + strings per `profile`),
/// returning its byte offset or `None`.
///
/// The trivia-aware replacement for the hand-rolled top-level scans over Svelte
/// binding/declaration strings (`{@const}` declarator `=`/`,`, snippet param `,`):
/// a `target` glyph inside a comment or string can't mis-anchor the scan. `target`
/// must not itself be a bracket or a trivia-introducing byte (`/`, `'`, `"`, `` ` ``).
pub(crate) fn find_top_level_delim(
    bytes: &[u8],
    start: usize,
    end: usize,
    target: u8,
    profile: TriviaProfile,
) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut i = start;
    while i < end {
        if let Some(past) = skip_trivia(bytes, i, end, profile) {
            i = past;
            continue;
        }
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b if b == target && depth == 0 => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// Find the byte offset of the bracket matching the `open` byte at `bytes[open_pos]`,
/// scanning forward past nested `open`/`close` pairs and skipping trivia (comments +
/// strings per `profile`). Returns the matching `close` offset, or `None` if the
/// brackets never balance before `end`.
///
/// The trivia-aware core shared by the Svelte binding bracket matchers
/// (`{ }`/`[ ]` destructuring patterns, `< >` snippet generics, `( )` snippet
/// params) — a `close` inside a comment or string can't end the match early, and
/// the cursor's escape-correct string handling fixes the former `ends_with('\\')`
/// escape bug. Template literals (in a pattern default like `{ a = `…` }`) are
/// intercepted and skipped interpolation-aware via `skip_template_literal`, since
/// `skip_trivia`'s opaque `` ` ``-to-`` ` `` scan mis-pairs across a nested template.
pub(crate) fn match_bracket(
    bytes: &[u8],
    open_pos: usize,
    end: usize,
    open: u8,
    close: u8,
    profile: TriviaProfile,
) -> Option<usize> {
    debug_assert_eq!(
        bytes.get(open_pos),
        Some(&open),
        "match_bracket must start at the opening bracket"
    );
    let mut depth: u32 = 1;
    let mut i = open_pos + 1;
    while i < end {
        // Template literal — skip it whole, interpolation-aware. `skip_trivia`'s
        // opaque quote-to-quote handling mis-pairs backticks across a nested
        // template (`` `${`x`}` ``), so a pattern default like `{ a = `${`"`}` }`
        // would swallow past the closing bracket. Gated on `profile.strings` (the
        // JS binding-pattern callers), matching where `skip_trivia` treats `` ` ``
        // as a string.
        if profile.strings && bytes[i] == b'`' {
            i = skip_template_literal(bytes, i, end);
            continue;
        }
        if let Some(past) = skip_trivia(bytes, i, end, profile) {
            i = past;
            continue;
        }
        let b = bytes[i];
        if b == open {
            depth += 1;
        } else if b == close {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}
