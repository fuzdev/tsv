// Doc builders for Svelte control-flow blocks
//
// {#if}/{:else if}/{:else}, {#each}, {#await}, {#key}, and {#snippet} —
// opening/closing tag layout, branch flattening, and section bodies.

// Allow Svelte block syntax like `{:else}`, `{:then}`, `{:catch}` which
// look like Rust format args but are valid Svelte template syntax.
#![allow(clippy::literal_string_with_formatting_args)]

use crate::ast::internal::{self, Fragment, FragmentNode};
use crate::printer::Printer;
use tsv_lang::doc::arena::DocId;

use super::helpers::indent_body;

// Opening-tag literals for control-flow blocks. Every offset that locates the
// embedded expression past the opening tag derives from `.len()` of these, so
// the emitted text and the scan offset cannot drift apart. Shared with the
// inline / whitespace-sensitive builders in `element_doc.rs`.
pub(crate) const IF_BLOCK_OPEN: &str = "{#if ";
pub(crate) const ELSE_IF_BLOCK_OPEN: &str = "{:else if ";
pub(crate) const EACH_BLOCK_OPEN: &str = "{#each ";
pub(crate) const AWAIT_BLOCK_OPEN: &str = "{#await ";
pub(crate) const KEY_BLOCK_OPEN: &str = "{#key ";

/// Build an await block section body with newline-based whitespace detection.
///
/// Returns `(body_doc, has_trailing)` — the indented body doc and whether the
/// fragment had trailing whitespace (needed for section separator logic).
fn build_await_section_body(printer: &Printer, fragment: &Fragment) -> (DocId, bool) {
    let has_leading = printer.fragment_has_leading_ws(fragment);
    let has_trailing = printer.fragment_has_trailing_ws(fragment);
    let force_break = printer.fragment_should_force_break_content(&fragment.nodes);
    let is_inline = !has_leading && !has_trailing && !force_break;
    let body_doc = if is_inline {
        printer.build_fragment_doc(fragment)
    } else {
        printer.build_nodes_doc_multiline(&fragment.nodes)
    };
    (indent_body(printer, body_doc, has_leading), has_trailing)
}

/// Build `indent([line, body_doc])` for space-only await blocks.
///
/// In flat mode (fits): ` body_doc` (space + content)
/// In break mode (exceeds print width): newline + indent + body_doc
fn indent_body_soft(printer: &Printer, body_doc: DocId) -> DocId {
    let line = printer.d().line();
    let inner = printer.d().concat(&[line, body_doc]);
    printer.d().indent(inner)
}

/// Split a raw parameter string at top-level commas, returning trimmed param strings.
///
/// Handles nesting for `()`, `[]`, `{}`, `<>`, and string literals (`'...'`, `"..."`).
/// E.g., `"a: A | 'x', b: B<C, D>"` → `["a: A | 'x'", "b: B<C, D>"]`.
fn split_raw_params_at_commas(raw: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' => {
                let quote = bytes[i];
                i += 1;
                while i < bytes.len() && bytes[i] != quote {
                    if bytes[i] == b'\\' {
                        i += 1; // skip escaped char
                    }
                    i += 1;
                }
            }
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => depth -= 1,
            b',' if depth == 0 => {
                result.push(raw[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    let last = raw[start..].trim();
    if !last.is_empty() {
        result.push(last);
    }
    result
}

impl<'a> Printer<'a> {
    /// Build a doc for an if block
    ///
    /// For inline blocks (no leading/trailing whitespace in body), hugs content directly:
    ///   {#if cond}content{/if}
    ///
    /// For multiline blocks (has whitespace boundaries), uses hardlines:
    ///   {#if cond}\n  content\n{/if}
    ///
    /// Note: Body is always wrapped in indent() so any internal breaks (like component
    /// attr wrapping) get proper indentation relative to the if block.
    pub(crate) fn build_if_block_doc(&self, block: &internal::IfBlock) -> DocId {
        self.build_if_block_doc_with_context(block, false)
    }

    /// Build if block doc with multiline context awareness.
    ///
    /// When `in_multiline_context` is true, blocks with symmetric spaces expand.
    pub(crate) fn build_if_block_doc_with_context(
        &self,
        block: &internal::IfBlock,
        in_multiline_context: bool,
    ) -> DocId {
        self.build_if_block_doc_with_full_context(block, in_multiline_context, false)
    }

    /// Build if block doc with full context (multiline + preceding content).
    ///
    /// `has_preceding_breakable`: If true, there's breakable content before this block,
    /// so use remove_lines() to ensure that content breaks first.
    pub(super) fn build_if_block_doc_with_full_context(
        &self,
        block: &internal::IfBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> DocId {
        let d = self.d();
        // Build expression doc with context-dependent behavior
        // Use remove_lines only if there's preceding breakable content (so it breaks first).
        // Otherwise, allow natural wrapping to respect print_width.
        let allow_wrapping = !has_preceding_breakable;
        let expr_doc = self.build_expression_doc_for_block(
            &block.test,
            block.opening_tag_span.start + IF_BLOCK_OPEN.len() as u32,
            block.opening_tag_span.end - 1,
            IF_BLOCK_OPEN.len(),
            allow_wrapping || in_multiline_context,
        );

        // Check leading/trailing whitespace, considering multiline context.
        // Space-only whitespace (no newlines) also triggers expansion to match prettier.
        // E.g., `{#if a} content {/if}` or `{#if a} content{/if}` → expand to multiline.
        let (has_leading, has_trailing) =
            self.fragment_ws_status(&block.consequent, in_multiline_context);
        // Force non-inline when block elements among multiple children
        // (matches prettier's forceBreakContent + breakParent)
        let force_break = self.fragment_should_force_break_content(&block.consequent.nodes);
        let is_inline = !has_leading && !has_trailing && !force_break;

        // For inline: use regular fragment doc (preserves spaces)
        // For multiline: use multiline doc (preserves line structure with hardlines)
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.consequent)
        } else {
            self.build_nodes_doc_multiline(&block.consequent.nodes)
        };

        // Always wrap body in indent() for proper internal break indentation
        let indented_body = indent_body(self, body_doc, has_leading);

        let mut parts = vec![d.text(IF_BLOCK_OPEN), expr_doc, d.text("}"), indented_body];

        // Handle alternate (else/else-if) and determine final trailing status
        let final_has_trailing = if let Some(alt) = &block.alternate {
            // Add break before alternate only if consequent has trailing ws
            if has_trailing {
                parts.push(d.hardline());
            }
            parts.push(self.build_if_alternate_doc(
                alt,
                has_leading,
                has_trailing,
                in_multiline_context,
            ));
            // Get trailing status from the final branch
            self.get_final_branch_trailing(block, in_multiline_context)
        } else {
            has_trailing
        };

        // Add endline before {/if} only if final branch has trailing whitespace
        if final_has_trailing {
            parts.push(d.hardline());
        }

        parts.push(d.text("{/if}"));
        d.concat(&parts)
    }

    /// Check if a fragment can be flattened to an else-if.
    ///
    /// Returns the inner IfBlock only when the fragment is exactly one IfBlock
    /// (plus optional whitespace) AND the user authored it as `{:else if}`
    /// (Svelte's `elseif: true` flag). Returns None for multiple IfBlocks, other
    /// content, or a block-form `{:else}{#if}{/if}` (`elseif: false`): that form is
    /// preserved verbatim rather than collapsed — matching prettier, which keeps the
    /// two distinct (collapsing would be information loss).
    pub(super) fn get_flattenable_else_if(alt: &Fragment) -> Option<&internal::IfBlock> {
        let mut if_block: Option<&internal::IfBlock> = None;

        for node in &alt.nodes {
            match node {
                FragmentNode::IfBlock(b) => {
                    if if_block.is_some() {
                        // Multiple IfBlocks - can't flatten
                        return None;
                    }
                    if_block = Some(b);
                }
                FragmentNode::Text(t) if t.raw.trim().is_empty() => {
                    // Whitespace-only text is OK
                }
                _ => {
                    // Non-whitespace content - can't flatten
                    return None;
                }
            }
        }

        // Block-form `{:else}{#if}{/if}` (elseif: false) does not flatten — see fn doc.
        if_block.filter(|b| b.elseif)
    }

    /// Build the condition-expression doc for a flattened `{:else if}` block.
    ///
    /// Shared by the normal and whitespace-sensitive alternate printers.
    /// `get_flattenable_else_if` only returns genuine `{:else if}` blocks, so the
    /// opening is always the literal `{:else if ` and the expression starts that many
    /// chars past the opening-tag span.
    pub(super) fn build_else_if_expr_doc(
        &self,
        else_if: &internal::IfBlock,
        in_multiline_context: bool,
    ) -> DocId {
        self.build_expression_doc_for_block(
            &else_if.test,
            else_if.opening_tag_span.start + ELSE_IF_BLOCK_OPEN.len() as u32,
            else_if.opening_tag_span.end - 1,
            ELSE_IF_BLOCK_OPEN.len(),
            in_multiline_context,
        )
    }

    /// Build doc for if block alternate (else or else-if)
    ///
    /// Uses separate leading/trailing whitespace handling for proper hugging.
    /// `parent_has_leading` - whether parent had leading ws (break after opening)
    /// `parent_has_trailing` - whether parent had trailing ws (break before this alternate)
    /// `in_multiline_context` - whether we're in a multiline parent context
    ///
    /// Returns (doc, final_has_trailing) where final_has_trailing indicates whether
    /// the last branch of this alternate chain has trailing whitespace.
    fn build_if_alternate_doc(
        &self,
        alt: &Fragment,
        parent_has_leading: bool,
        parent_has_trailing: bool,
        in_multiline_context: bool,
    ) -> DocId {
        let d = self.d();
        // Check if this can be flattened to {:else if ...}
        if let Some(else_if) = Self::get_flattenable_else_if(alt) {
            // {:else if condition}
            let expr_doc = self.build_else_if_expr_doc(else_if, in_multiline_context);

            // Check this branch's own leading/trailing whitespace
            let (has_leading, has_trailing) =
                self.fragment_ws_status(&else_if.consequent, in_multiline_context);
            let force_break = self.fragment_should_force_break_content(&else_if.consequent.nodes);
            let is_inline = !has_leading && !has_trailing && !force_break;
            let parent_inline = !parent_has_leading && !parent_has_trailing;
            let is_both_inline = is_inline && parent_inline;

            // For inline: use regular fragment doc (preserves spaces)
            // For multiline: use multiline doc (preserves line structure)
            let body_doc = if is_both_inline {
                self.build_fragment_doc(&else_if.consequent)
            } else {
                self.build_nodes_doc_multiline(&else_if.consequent.nodes)
            };

            let indented_body = indent_body(self, body_doc, has_leading);

            let mut parts = vec![
                d.text(ELSE_IF_BLOCK_OPEN),
                expr_doc,
                d.text("}"),
                indented_body,
            ];

            // Handle nested alternate or trailing
            if let Some(nested_alt) = &else_if.alternate {
                // Add break before next alternate only if this branch has trailing ws
                if has_trailing {
                    parts.push(d.hardline());
                }
                parts.push(self.build_if_alternate_doc(
                    nested_alt,
                    has_leading,
                    has_trailing,
                    in_multiline_context,
                ));
            }

            return d.concat(&parts);
        }

        // Plain {:else}
        let (has_leading, has_trailing) = self.fragment_ws_status(alt, in_multiline_context);
        let force_break = self.fragment_should_force_break_content(&alt.nodes);
        let is_inline = !has_leading && !has_trailing && !force_break;
        let parent_inline = !parent_has_leading && !parent_has_trailing;
        let is_both_inline = is_inline && parent_inline;

        // For inline: use regular fragment doc (preserves spaces)
        // For multiline: use multiline doc
        let body_doc = if is_both_inline {
            self.build_nodes_doc(&alt.nodes)
        } else {
            self.build_nodes_doc_multiline(&alt.nodes)
        };

        let indented_body = indent_body(self, body_doc, has_leading);

        d.concat(&[d.text("{:else}"), indented_body])
    }

    /// Get the trailing whitespace status of the final branch in an if-block.
    ///
    /// This walks the alternate chain to find the last branch and returns
    /// whether it has trailing whitespace (for placing `{/if}`).
    fn get_final_branch_trailing(
        &self,
        block: &internal::IfBlock,
        in_multiline_context: bool,
    ) -> bool {
        // If no alternate, use the consequent's trailing
        let Some(alt) = &block.alternate else {
            let (_, has_trailing) =
                self.fragment_ws_status(&block.consequent, in_multiline_context);
            return has_trailing;
        };

        // Check if this is an else-if chain
        if let Some(else_if) = Self::get_flattenable_else_if(alt) {
            // Recurse into else-if
            return self.get_final_branch_trailing(else_if, in_multiline_context);
        }

        // Plain {:else} - use its trailing
        let (_, has_trailing) = self.fragment_ws_status(alt, in_multiline_context);
        has_trailing
    }

    /// Build a doc for an each block
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_each_block_doc(&self, block: &internal::EachBlock) -> DocId {
        self.build_each_block_doc_with_context(block, false)
    }

    /// Build each block doc with multiline context awareness.
    pub(crate) fn build_each_block_doc_with_context(
        &self,
        block: &internal::EachBlock,
        in_multiline_context: bool,
    ) -> DocId {
        self.build_each_block_doc_with_full_context(block, in_multiline_context, false)
    }

    /// Build each block doc with full context (multiline + preceding content).
    pub(super) fn build_each_block_doc_with_full_context(
        &self,
        block: &internal::EachBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> DocId {
        let d = self.d();
        // Build expression doc with context-dependent behavior
        // Comment range: after "{#each " to before "as" keyword (or end if no context)
        let allow_wrapping = !has_preceding_breakable;
        let expr_comment_end = block
            .context
            .as_ref()
            .map_or(block.opening_tag_span.end - 1, |c| c.span().start);
        let expr_doc = self.build_expression_doc_for_block(
            &block.expression,
            block.opening_tag_span.start + EACH_BLOCK_OPEN.len() as u32,
            expr_comment_end,
            EACH_BLOCK_OPEN.len(),
            allow_wrapping || in_multiline_context,
        );

        let mut opening = vec![d.text(EACH_BLOCK_OPEN), expr_doc];

        // Pattern (context) - only add " as " when there's a context or index
        if let Some(context) = &block.context {
            opening.push(d.text(" as "));
            // Format pattern through TypeScript formatter for proper whitespace normalization
            let pattern_doc = self.build_pattern_doc(context);
            opening.push(pattern_doc);
            if let Some(index) = &block.index {
                opening.push(d.text(", "));
                opening.push(d.text_owned(index.clone()));
            }
        } else if let Some(index) = &block.index {
            // No context but has index: ", i" pattern
            opening.push(d.text(", "));
            opening.push(d.text_owned(index.clone()));
        }

        if let Some(key) = &block.key {
            // Build key doc with context-dependent behavior
            // The key expression is inside parens, so opening offset accounts for that
            let key_doc = if let Some(key_span) = block.key_span {
                self.build_expression_doc_for_block(
                    key,
                    key_span.start + 1, // after "("
                    key_span.end - 1,   // before ")"
                    1,                  // "(" = 1 char (key is inside parens)
                    allow_wrapping || in_multiline_context,
                )
            } else {
                // No key_span: build doc directly
                self.build_ts_expression_doc(key)
            };
            opening.push(d.text(" ("));
            opening.push(key_doc);
            opening.push(d.text(")"));
        }

        opening.push(d.text("}"));

        // Check leading/trailing whitespace, considering multiline context.
        // Space-only whitespace (no newlines) also triggers expansion to match prettier.
        let (has_leading, has_trailing) =
            self.fragment_ws_status(&block.body, in_multiline_context);
        // Force non-inline when block elements among multiple children
        let force_break = self.fragment_should_force_break_content(&block.body.nodes);
        let is_inline = !has_leading && !has_trailing && !force_break;

        // For inline: use regular fragment doc (preserves inline spacing)
        // For multiline: use multiline doc (preserves line structure with hardlines)
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.body)
        } else {
            self.build_nodes_doc_multiline(&block.body.nodes)
        };

        let indented_body = indent_body(self, body_doc, has_leading);

        let opening_concat = d.concat(&opening);
        let mut parts = vec![opening_concat, indented_body];

        // Determine final trailing status (from body or fallback if present)
        let final_has_trailing = if let Some(fallback) = &block.fallback {
            // Add break before {:else} only if body has trailing ws
            if has_trailing {
                parts.push(d.hardline());
            }

            let (fallback_has_leading, fallback_has_trailing) =
                self.fragment_ws_status(fallback, in_multiline_context);
            let fallback_force_break = self.fragment_should_force_break_content(&fallback.nodes);
            let fallback_inline =
                !fallback_has_leading && !fallback_has_trailing && !fallback_force_break;
            let is_both_inline = fallback_inline && is_inline;

            parts.push(d.text("{:else}"));

            // For inline: use regular fragment doc
            // For multiline: use multiline doc
            let fallback_doc = if is_both_inline {
                self.build_fragment_doc(fallback)
            } else {
                self.build_nodes_doc_multiline(&fallback.nodes)
            };

            let indented_fallback =
                indent_body(self, fallback_doc, fallback_has_leading || has_leading);
            parts.push(indented_fallback);

            fallback_has_trailing
        } else {
            has_trailing
        };

        // Add endline before {/each} only if final has trailing whitespace
        if final_has_trailing {
            parts.push(d.hardline());
        }

        parts.push(d.text("{/each}"));
        d.concat(&parts)
    }

    /// Build a doc for an await block
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_await_block_doc(&self, block: &internal::AwaitBlock) -> DocId {
        self.build_await_block_doc_with_context(block, false)
    }

    /// Build await block doc with multiline context awareness.
    pub(crate) fn build_await_block_doc_with_context(
        &self,
        block: &internal::AwaitBlock,
        in_multiline_context: bool,
    ) -> DocId {
        self.build_await_block_doc_with_full_context(block, in_multiline_context, false)
    }

    /// Build await block doc with full context (multiline + preceding content).
    pub(super) fn build_await_block_doc_with_full_context(
        &self,
        block: &internal::AwaitBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> DocId {
        let d = self.d();
        // Build expression doc with context-dependent behavior
        let allow_wrapping = !has_preceding_breakable;
        let expr_doc = self.build_expression_doc_for_block(
            &block.expression,
            block.opening_tag_span.start + AWAIT_BLOCK_OPEN.len() as u32,
            block.opening_tag_span.end - 1,
            AWAIT_BLOCK_OPEN.len(),
            allow_wrapping || in_multiline_context,
        );

        let mut parts = vec![d.text(AWAIT_BLOCK_OPEN), expr_doc];

        // Shorthand: {#await expr then value}...{/await}
        // Also handles: {#await expr then value}...{:catch error}...{/await}
        if let (Some(value), None) = (&block.value, &block.pending) {
            parts.push(d.text(" then "));
            parts.push(self.build_pattern_doc(value));
            parts.push(d.text("}"));

            // Check if any section has space-only whitespace
            let has_space_only = block
                .then
                .as_ref()
                .is_some_and(|f| self.fragment_has_space_only_ws(f))
                || block
                    .catch
                    .as_ref()
                    .is_some_and(|f| self.fragment_has_space_only_ws(f));

            if has_space_only {
                if let Some(then_block) = &block.then {
                    let body_doc = self.build_nodes_doc_multiline(&then_block.nodes);
                    parts.push(indent_body_soft(self, body_doc));
                }
                if let Some(error) = &block.error {
                    parts.push(d.line());
                    parts.push(d.text("{:catch "));
                    parts.push(self.build_pattern_doc(error));
                    parts.push(d.text("}"));
                } else if block.catch.as_ref().is_some_and(|c| !c.nodes.is_empty()) {
                    parts.push(d.line());
                    parts.push(d.text("{:catch}"));
                }
                if let Some(catch_block) = &block.catch {
                    let body_doc = self.build_nodes_doc_multiline(&catch_block.nodes);
                    parts.push(indent_body_soft(self, body_doc));
                }
                parts.push(d.line());
                parts.push(d.text("{/await}"));
                let concat = d.concat(&parts);
                return d.group(concat);
            }

            let mut prev_has_trailing = false;
            if let Some(then_block) = &block.then {
                let (body, trailing) = build_await_section_body(self, then_block);
                parts.push(body);
                prev_has_trailing = trailing;
            }

            // Optional {:catch} continuation after then-shorthand
            if block.catch.is_some() {
                if let Some(error) = &block.error {
                    if prev_has_trailing {
                        parts.push(d.hardline());
                    }
                    parts.push(d.text("{:catch "));
                    parts.push(self.build_pattern_doc(error));
                    parts.push(d.text("}"));
                } else if block.catch.as_ref().is_some_and(|c| !c.nodes.is_empty()) {
                    if prev_has_trailing {
                        parts.push(d.hardline());
                    }
                    parts.push(d.text("{:catch}"));
                }
                if let Some(catch_block) = &block.catch {
                    let (body, trailing) = build_await_section_body(self, catch_block);
                    parts.push(body);
                    prev_has_trailing = trailing;
                }
            }

            if prev_has_trailing {
                parts.push(d.hardline());
            }
            parts.push(d.text("{/await}"));
            return d.concat(&parts);
        }

        // Shorthand: {#await expr catch error}
        if block.pending.is_none()
            && block.value.is_none()
            && let Some(error) = &block.error
        {
            parts.push(d.text(" catch "));
            parts.push(self.build_pattern_doc(error));
            parts.push(d.text("}"));

            // Check if any section has space-only whitespace
            let has_space_only = block
                .catch
                .as_ref()
                .is_some_and(|f| self.fragment_has_space_only_ws(f));

            if has_space_only {
                if let Some(catch_block) = &block.catch {
                    let body_doc = self.build_nodes_doc_multiline(&catch_block.nodes);
                    parts.push(indent_body_soft(self, body_doc));
                }
                parts.push(d.line());
                parts.push(d.text("{/await}"));
                let concat = d.concat(&parts);
                return d.group(concat);
            }

            if let Some(catch_block) = &block.catch {
                let (body, has_trailing) = build_await_section_body(self, catch_block);
                parts.push(body);
                if has_trailing {
                    parts.push(d.hardline());
                }
            }
            parts.push(d.text("{/await}"));
            return d.concat(&parts);
        }

        parts.push(d.text("}"));

        // Check if any section has space-only whitespace (spaces, no newlines).
        // Space-only await blocks stay inline when short but break when exceeding
        // print width. Use group+line so the renderer decides based on width.
        let has_space_only = [&block.pending, &block.then, &block.catch].iter().any(|f| {
            f.as_ref()
                .is_some_and(|f| self.fragment_has_space_only_ws(f))
        });

        if has_space_only {
            // Build all sections with line() docs — space in flat, newline in break.
            // All sections break together as a unit via the outer group.
            if let Some(pending) = &block.pending {
                let body_doc = self.build_nodes_doc_multiline(&pending.nodes);
                parts.push(indent_body_soft(self, body_doc));
            }

            if let Some(value) = &block.value {
                parts.push(d.line());
                parts.push(d.text("{:then "));
                parts.push(self.build_pattern_doc(value));
                parts.push(d.text("}"));
            } else if block.then.as_ref().is_some_and(|t| !t.nodes.is_empty()) {
                parts.push(d.line());
                parts.push(d.text("{:then}"));
            }
            if let Some(then_block) = &block.then {
                let body_doc = self.build_nodes_doc_multiline(&then_block.nodes);
                parts.push(indent_body_soft(self, body_doc));
            }

            if let Some(error) = &block.error {
                parts.push(d.line());
                parts.push(d.text("{:catch "));
                parts.push(self.build_pattern_doc(error));
                parts.push(d.text("}"));
            } else if block.catch.as_ref().is_some_and(|c| !c.nodes.is_empty()) {
                parts.push(d.line());
                parts.push(d.text("{:catch}"));
            }
            if let Some(catch_block) = &block.catch {
                let body_doc = self.build_nodes_doc_multiline(&catch_block.nodes);
                parts.push(indent_body_soft(self, body_doc));
            }

            parts.push(d.line());
            parts.push(d.text("{/await}"));
            let concat = d.concat(&parts);
            return d.group(concat);
        }

        // Track whitespace status for each section
        // The final section's trailing determines break before {/await}
        let mut final_has_trailing = false;
        let mut prev_has_trailing = false;

        // Pending - newline-based detection only (space-only handled above via group)
        if let Some(pending) = &block.pending {
            let (body, has_trailing) = build_await_section_body(self, pending);
            parts.push(body);
            final_has_trailing = has_trailing;
            prev_has_trailing = has_trailing;
        }

        // Then
        if let Some(value) = &block.value {
            if prev_has_trailing {
                parts.push(d.hardline());
            }
            parts.push(d.text("{:then "));
            parts.push(self.build_pattern_doc(value));
            parts.push(d.text("}"));
        } else if block.then.as_ref().is_some_and(|t| !t.nodes.is_empty()) {
            if prev_has_trailing {
                parts.push(d.hardline());
            }
            parts.push(d.text("{:then}"));
        }
        if let Some(then_block) = &block.then {
            let (body, has_trailing) = build_await_section_body(self, then_block);
            parts.push(body);
            final_has_trailing = has_trailing;
            prev_has_trailing = has_trailing;
        }

        // Catch
        if let Some(error) = &block.error {
            if prev_has_trailing {
                parts.push(d.hardline());
            }
            parts.push(d.text("{:catch "));
            parts.push(self.build_pattern_doc(error));
            parts.push(d.text("}"));
        } else if block.catch.as_ref().is_some_and(|c| !c.nodes.is_empty()) {
            if prev_has_trailing {
                parts.push(d.hardline());
            }
            parts.push(d.text("{:catch}"));
        }
        if let Some(catch_block) = &block.catch {
            let (body, has_trailing) = build_await_section_body(self, catch_block);
            parts.push(body);
            final_has_trailing = has_trailing;
        }

        // Add endline before {/await} only if final section has trailing whitespace
        if final_has_trailing {
            parts.push(d.hardline());
        }

        parts.push(d.text("{/await}"));
        d.concat(&parts)
    }

    /// Build a doc for a key block
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_key_block_doc(&self, block: &internal::KeyBlock) -> DocId {
        self.build_key_block_doc_with_context(block, false)
    }

    /// Build key block doc with multiline context awareness.
    pub(crate) fn build_key_block_doc_with_context(
        &self,
        block: &internal::KeyBlock,
        in_multiline_context: bool,
    ) -> DocId {
        self.build_key_block_doc_with_full_context(block, in_multiline_context, false)
    }

    /// Build key block doc with full context (multiline + preceding content).
    pub(super) fn build_key_block_doc_with_full_context(
        &self,
        block: &internal::KeyBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> DocId {
        let d = self.d();
        // Build expression doc with context-dependent behavior
        let allow_wrapping = !has_preceding_breakable;
        let expr_doc = self.build_expression_doc_for_block(
            &block.expression,
            block.opening_tag_span.start + KEY_BLOCK_OPEN.len() as u32,
            block.opening_tag_span.end - 1,
            KEY_BLOCK_OPEN.len(),
            allow_wrapping || in_multiline_context,
        );

        // Check leading/trailing whitespace, considering space-only patterns.
        // Space-only whitespace (no newlines) also triggers expansion to match prettier.
        let (has_leading, has_trailing) = self.fragment_ws_status(&block.fragment, false);
        // Force non-inline when block elements among multiple children
        let force_break = self.fragment_should_force_break_content(&block.fragment.nodes);
        let is_inline = !has_leading && !has_trailing && !force_break;

        // For inline: use regular fragment doc (preserves inline spacing)
        // For multiline: use multiline doc (preserves line structure with hardlines)
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.fragment)
        } else {
            self.build_nodes_doc_multiline(&block.fragment.nodes)
        };

        let indented_body = indent_body(self, body_doc, has_leading);

        let mut parts = vec![d.text(KEY_BLOCK_OPEN), expr_doc, d.text("}"), indented_body];

        // Add endline before {/key} only if trailing whitespace exists
        if has_trailing {
            parts.push(d.hardline());
        }

        parts.push(d.text("{/key}"));
        d.concat(&parts)
    }

    /// Build a doc for a snippet block
    ///
    /// Uses same inline/multiline pattern as if blocks.
    /// Opening tag uses group() for parameter wrapping when they exceed print width.
    pub(crate) fn build_snippet_block_doc(&self, block: &internal::SnippetBlock) -> DocId {
        let d = self.d();
        // Extract snippet name from the identifier expression
        let name = self.extract_source_range(
            block.expression.span().start_usize(),
            block.expression.span().end_usize(),
        );

        // Check leading/trailing whitespace, considering space-only patterns.
        // Space-only whitespace (no newlines) also triggers expansion to match prettier.
        let (has_leading, has_trailing) = self.fragment_ws_status(&block.body, false);
        let force_break = self.fragment_should_force_break_content(&block.body.nodes);
        let is_inline = !has_leading && !has_trailing && !force_break;

        // Type parameters (generics)
        let type_params_part = block.type_parameters.as_ref().map_or_else(
            || d.empty(),
            |tp| d.concat(&[d.text("<"), d.text_owned(tp.clone()), d.text(">")]),
        );

        // Parameters: use raw_parameters if available (preserves TypeScript types),
        // otherwise format individual params.
        // Split raw_parameters at top-level commas so each param gets its own
        // line when the group breaks (matching prettier's per-param wrapping).
        let params_docs: Vec<DocId> = if let Some(raw) = &block.raw_parameters {
            split_raw_params_at_commas(raw)
                .iter()
                .map(|s| d.text_owned(s.to_string()))
                .collect()
        } else {
            block
                .parameters
                .iter()
                .map(|p| {
                    // Format parameter through TypeScript formatter for proper normalization
                    self.build_ts_expression_doc_no_comments(p)
                })
                .collect()
        };

        // Build opening tag with group for parameter wrapping
        // When fits: {#snippet name(a, b, c)}
        // When wraps: {#snippet name(\n\ta,\n\tb,\n\tc,\n)}
        // Empty params: {#snippet name()} - no wrapping structure
        let opening_doc = if params_docs.is_empty() {
            // No params - simple structure that won't break incorrectly
            d.concat(&[
                d.text("{#snippet "),
                d.text_owned(name.to_string()),
                type_params_part,
                d.text("()}"),
            ])
        } else {
            // Build params doc with line() separators for wrapping
            // Pre-allocate: each param + separator (except first)
            let mut parts = Vec::with_capacity(params_docs.len() * 3);
            for (i, param_doc) in params_docs.into_iter().enumerate() {
                if i > 0 {
                    parts.push(d.text(","));
                    parts.push(d.line());
                }
                parts.push(param_doc);
            }
            let params_doc = d.concat(&parts);

            let indent_sl = d.indent_softline(params_doc);
            let trailing = d.trailing_comma();
            let softline = d.softline();
            let inner = d.concat(&[
                d.text("{#snippet "),
                d.text_owned(name.to_string()),
                type_params_part,
                d.text("("),
                indent_sl,
                trailing,
                softline,
                d.text(")}"),
            ]);
            d.group(inner)
        };

        let mut parts = vec![opening_doc];

        // Body: inline hugs directly, multiline uses hardlines
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.body)
        } else {
            self.build_nodes_doc_multiline(&block.body.nodes)
        };

        parts.push(indent_body(self, body_doc, has_leading));

        // Add endline before {/snippet} only if trailing whitespace exists
        if has_trailing {
            parts.push(d.hardline());
        }

        parts.push(d.text("{/snippet}"));
        d.concat(&parts)
    }
}
