// Helper utilities for node formatting
//
// Fragment-node classification predicates plus the pattern/expression doc
// builders shared by the block and tag builders, and source position tracking
// used in inline run grouping and multiline formatting decisions.

use crate::ast::internal::FragmentNode;
use crate::printer::Printer;
use tsv_lang::TAB_WIDTH;
use tsv_lang::doc::arena::DocId;

/// Check if a fragment node is a control flow block (if/each/await/key/snippet).
///
/// Control flow blocks can hug adjacent inline content when directly adjacent,
/// unlike HTML block elements (`<div>`, `<p>`) which get their own lines.
pub(crate) fn is_control_flow_block(node: &FragmentNode) -> bool {
    matches!(
        node,
        FragmentNode::IfBlock(_)
            | FragmentNode::EachBlock(_)
            | FragmentNode::AwaitBlock(_)
            | FragmentNode::KeyBlock(_)
            | FragmentNode::SnippetBlock(_)
    )
}

/// Check if a fragment node is a control flow block that forces block elements to expand.
///
/// Only if/each/key blocks force expansion. Await blocks do NOT - they stay inline
/// in block elements (e.g., `<div>{#await promise}loading{/await}</div>` stays inline).
pub(crate) fn is_expanding_control_flow_block(node: &FragmentNode) -> bool {
    matches!(
        node,
        FragmentNode::IfBlock(_) | FragmentNode::EachBlock(_) | FragmentNode::KeyBlock(_)
    )
}

/// Check if nodes contain any expanding blocks, either directly or nested in await blocks.
///
/// This is a convenience function combining `is_expanding_control_flow_block` and
/// `has_expanding_block_in_await` checks that are commonly used together.
pub(crate) fn has_any_expanding_blocks(nodes: &[FragmentNode]) -> bool {
    nodes.iter().any(is_expanding_control_flow_block) || has_expanding_block_in_await(nodes)
}

/// Whether a fragment node is inline content (a non-text node that participates in fill).
///
/// This is NOT the same as `!tsv_html::is_block_element` (HTML classification): a block
/// element like `<div>` is still inline *content* here. The set is elements / components /
/// expression-or-html-or-render tags — the nodes that can break before a control-flow
/// block. Plain text, comments, and the control-flow blocks themselves are not.
///
/// Single source of truth for the `has_preceding_breakable` test in `fragment_doc`'s node
/// loops and for [`has_control_flow_after_sibling`]'s breakable-sibling gate.
pub(crate) fn is_inline_content(node: &FragmentNode) -> bool {
    matches!(
        node,
        FragmentNode::Element(_)
            | FragmentNode::SpecialElement(_)
            | FragmentNode::ExpressionTag(_)
            | FragmentNode::RenderTag(_)
            | FragmentNode::HtmlTag(_)
    )
}

/// Whether a control-flow block is preceded by a **breakable** sibling — one that is
/// [`is_inline_content`], the same set that sets `has_preceding_breakable`.
///
/// `{#await}` / `{#snippet}` don't force their parent multiline on their own (a lone
/// `<div>{#await p}x{/await}</div>` stays inline, matching prettier), unlike if/each/key
/// (`has_any_expanding_blocks`). They need a force only when a preceding **breakable**
/// sibling *suppresses* the body-drop: there, forcing the parent multiline lets the
/// body-drop (`can_wrap`), the inline-element closing-`>` dangle
/// (`try_block_sibling_gt_dangle`), and a block-element sibling's own-line separation all
/// resolve in one pass. A non-breakable preceding sibling (plain text, a comment) does
/// **not** suppress the body-drop — await/snippet already drop on their own — so forcing
/// there only diverges from prettier (which keeps the short construct inline); such
/// siblings are skipped. (Block elements are `Element`, hence breakable, so their
/// separation still fires.) The force is also gated on `kind.is_block()` at the call site,
/// so it only applies to block-element parents.
pub(crate) fn has_control_flow_after_sibling(nodes: &[FragmentNode]) -> bool {
    let mut seen_breakable = false;
    for node in nodes {
        if node.is_whitespace_only_text() {
            continue;
        }
        if seen_breakable && is_control_flow_block(node) {
            return true;
        }
        if is_inline_content(node) {
            seen_breakable = true;
        }
    }
    false
}

/// Check if any await block contains expanding blocks (if/each/key) in its content.
///
/// Prettier treats expanding blocks inside await blocks as if they were directly
/// in the parent element, forcing multiline. For example:
/// `<a>{#await p}{#if c}text{/if}{/await}</a>` breaks because the if block
/// is effectively inside the inline element.
///
/// This function recursively checks nested await blocks, so deeply nested
/// structures like `{#await p1}{#await p2}{#if c}...{/if}{/await}{/await}`
/// are also detected.
fn has_expanding_block_in_await(nodes: &[FragmentNode]) -> bool {
    nodes.iter().any(|n| {
        if let FragmentNode::AwaitBlock(block) = n {
            // Check all branches of the await block for expanding blocks
            // or recursively for nested awaits containing expanding blocks
            let check_fragment = |f: &crate::ast::internal::Fragment| {
                f.nodes.iter().any(is_expanding_control_flow_block)
                    || has_expanding_block_in_await(&f.nodes)
            };
            let has_in_pending = block.pending.as_ref().is_some_and(check_fragment);
            let has_in_then = block.then.as_ref().is_some_and(check_fragment);
            let has_in_catch = block.catch.as_ref().is_some_and(check_fragment);
            has_in_pending || has_in_then || has_in_catch
        } else {
            false
        }
    })
}

/// Check if any child element contains block flow (if/each/etc).
///
/// Used to detect when a parent element will go multiline due to
/// nested content forcing line breaks.
pub(crate) fn has_nested_block_flow(nodes: &[FragmentNode]) -> bool {
    nodes.iter().any(|n| {
        if let FragmentNode::Element(child) = n {
            child.fragment.nodes.iter().any(is_control_flow_block)
        } else {
            false
        }
    })
}

/// Helper to wrap body content in indent(), with optional hardline for leading whitespace.
///
/// This pattern is used consistently across all block types (if, each, await, snippet, key)
/// to ensure proper indentation of nested content when it breaks across lines.
pub(super) fn indent_body(printer: &Printer<'_>, body_doc: DocId, has_leading_ws: bool) -> DocId {
    if has_leading_ws {
        let hardline = printer.d().hardline();
        let inner = printer.d().concat(&[hardline, body_doc]);
        printer.d().indent(inner)
    } else {
        printer.d().indent(body_doc)
    }
}

impl<'a> Printer<'a> {
    /// Extract source range as string slice
    pub(super) fn extract_source_range(&self, start: usize, end: usize) -> &str {
        &self.source[start..end]
    }

    /// Build a doc for a pattern (destructuring context)
    ///
    /// Patterns use specific whitespace rules:
    /// - Object patterns: `{a, b}` (hugged braces, `bracketSpacing: false`)
    /// - Array patterns: `[a, b]` (no spaces inside brackets)
    ///
    /// Used for `{#each ... as pattern}`, `{#await ... then pattern}`,
    /// `{:then pattern}`, and `{:catch pattern}` binding contexts. Object braces
    /// hug uniformly with `bracketSpacing: false` (like `{@const}` and every
    /// other object tsv emits); prettier-plugin-svelte keeps the spaced form here.
    /// Literal **default values** likewise normalize through the TS printer
    /// (string quotes + numeric form), where prettier-plugin-svelte preserves the
    /// author's source token — both deliberate divergences (see
    /// conformance_prettier.md §Svelte: destructuring literal normalization).
    /// Build a doc for a non-shorthand object-pattern property key.
    ///
    /// A **computed** key (`{[expr]: v}`) keeps its `[ ]` brackets so it reads the
    /// same property — dropping them would change semantics (and a non-identifier
    /// key like a template literal would become invalid syntax). Mirrors the
    /// TypeScript pattern printer's computed-key handling
    /// (`tsv_ts`'s `build_object_pattern_property_doc`); this path is comment-free,
    /// so the bracket wrapping is plain text rather than the comment-aware
    /// `build_computed_key_bracket_doc`.
    fn build_pattern_property_key_doc(&self, computed: bool, key: &tsv_ts::Expression) -> DocId {
        let d = self.d();
        let key_doc = self.build_ts_expression_doc_no_comments(key);
        if computed {
            d.brackets(key_doc)
        } else {
            key_doc
        }
    }

    pub(super) fn build_pattern_doc(&self, expr: &tsv_ts::Expression) -> DocId {
        let d = self.d();
        match expr {
            tsv_ts::Expression::ObjectPattern(obj) => {
                let mut parts = vec![d.text("{")];
                for (i, prop) in obj.properties.iter().enumerate() {
                    if i > 0 {
                        parts.push(d.text(", "));
                    }
                    match prop {
                        tsv_ts::ObjectPatternProperty::Property(p) => {
                            if p.shorthand {
                                // Shorthand: `{ k }` or `{ k = 1 }`
                                // Use build_pattern_doc for the value to handle
                                // AssignmentPattern (defaults) and preserve quotes
                                parts.push(self.build_pattern_doc(&p.value));
                            } else {
                                parts.push(self.build_pattern_property_key_doc(p.computed, &p.key));
                                parts.push(d.text(": "));
                                parts.push(self.build_pattern_doc(&p.value));
                            }
                        }
                        tsv_ts::ObjectPatternProperty::RestElement(r) => {
                            parts.push(d.text("..."));
                            parts.push(self.build_pattern_doc(&r.argument));
                        }
                    }
                }
                parts.push(d.text("}"));
                d.concat(&parts)
            }
            tsv_ts::Expression::ObjectExpression(obj) => {
                // Legacy AST - treat same as ObjectPattern
                let mut parts = vec![d.text("{")];
                for (i, prop) in obj.properties.iter().enumerate() {
                    if i > 0 {
                        parts.push(d.text(", "));
                    }
                    match prop {
                        tsv_ts::ObjectProperty::Property(p) => {
                            if p.shorthand {
                                // Shorthand: `{ k }` or `{ k = 1 }`
                                parts.push(self.build_pattern_doc(&p.value));
                            } else {
                                parts.push(self.build_pattern_property_key_doc(p.computed, &p.key));
                                parts.push(d.text(": "));
                                parts.push(self.build_pattern_doc(&p.value));
                            }
                        }
                        tsv_ts::ObjectProperty::SpreadElement(s) => {
                            parts.push(d.text("..."));
                            parts.push(self.build_pattern_doc(&s.argument));
                        }
                    }
                }
                parts.push(d.text("}"));
                d.concat(&parts)
            }
            tsv_ts::Expression::ArrayPattern(arr) => {
                let mut parts = vec![d.text("[")];
                for (i, elem) in arr.elements.iter().enumerate() {
                    if i > 0 {
                        parts.push(d.text(", "));
                    }
                    if let Some(e) = elem {
                        parts.push(self.build_pattern_doc(e));
                    }
                }
                parts.push(d.text("]"));
                d.concat(&parts)
            }
            tsv_ts::Expression::ArrayExpression(arr) => {
                // Legacy AST - treat same as ArrayPattern
                let mut parts = vec![d.text("[")];
                for (i, elem) in arr.elements.iter().enumerate() {
                    if i > 0 {
                        parts.push(d.text(", "));
                    }
                    if let Some(e) = elem {
                        parts.push(self.build_pattern_doc(e));
                    }
                }
                parts.push(d.text("]"));
                d.concat(&parts)
            }
            tsv_ts::Expression::RestElement(rest) => {
                let dots = d.text("...");
                let arg = self.build_pattern_doc(&rest.argument);
                d.concat(&[dots, arg])
            }
            tsv_ts::Expression::AssignmentPattern(assign) => {
                let left = self.build_pattern_doc(&assign.left);
                let eq = d.text(" = ");
                let right = self.build_pattern_doc(&assign.right);
                d.concat(&[left, eq, right])
            }
            tsv_ts::Expression::AssignmentExpression(assign) => {
                // Legacy AST - treat same as AssignmentPattern
                let left = self.build_pattern_doc(&assign.left);
                let eq = d.text(" = ");
                let right = self.build_pattern_doc(&assign.right);
                d.concat(&[left, eq, right])
            }
            // Default: build doc directly in shared arena. Literals route here too,
            // so string and numeric defaults normalize through the TS printer
            // (single quotes + escaping, lowercase hex/exponent, leading/trailing zeros) —
            // identical to `{@const}` and every other literal tsv emits.
            // prettier-plugin-svelte instead prints these binding patterns from raw
            // source, preserving the author's quote style and numeric form; tsv
            // normalizes uniformly (a deliberate divergence — see conformance_prettier.md
            // §Svelte: destructuring literal normalization).
            _ => self.build_ts_expression_doc_no_comments(expr),
        }
    }

    /// Build a doc for an expression with leading and trailing comments
    ///
    /// Looks up comments in the range [span_start, span_end] and includes them:
    /// - Leading comments: between span_start and expr.span().start
    /// - Expression doc
    /// - Trailing comments: between expr.span().end and span_end
    ///
    /// Builds the expression doc directly in the shared arena using
    /// `build_expression_doc_with_comments` with `LayoutMode::Embedded`
    /// so binary chains use ContinuationIndent style. The surrounding Svelte doc tree
    /// (e.g., the closing `}`) provides natural lookahead for fits checks — no
    /// `suffix_width` estimation needed.
    pub(super) fn build_expression_with_comments_doc(
        &self,
        expr: &tsv_ts::Expression,
        span_start: u32,
        span_end: u32,
    ) -> DocId {
        let d = self.d();
        let expr_start = expr.span().start;
        let expr_end = expr.span().end;

        // Build docs for leading comments (between span_start and expression start)
        let leading_docs: Vec<DocId> =
            tsv_lang::comments_in_range(self.comments, span_start, expr_start)
                .map(|c| self.build_leading_js_comment_doc(c))
                .collect();

        // Embed for embedded expression context: binary chains use ContinuationIndent style.
        // first_line_offset estimates the column position for width calculations.
        let context_indent = TAB_WIDTH;
        let opening_offset = 5; // typical tag prefix, e.g. `{#if `
        let first_line_offset = context_indent + opening_offset;
        let embed = tsv_lang::EmbedContext {
            first_line_offset,
            mode: tsv_lang::LayoutMode::Embedded,
            ..self.embed
        };

        // Build expression doc directly in the shared arena.
        // No suffix_width needed — the surrounding doc tree (closing `}`, etc.)
        // provides natural lookahead via arena_fits_with_lookahead's rest_commands.
        let expr_doc =
            tsv_ts::build_expression_doc_with_comments(d, expr, &self.ts_inputs(), &embed);

        // Build docs for trailing comments (between expression end and span_end)
        let trailing_docs: Vec<DocId> =
            tsv_lang::comments_in_range(self.comments, expr_end, span_end)
                .map(|c| self.build_trailing_js_comment_doc(c))
                .collect();

        self.concat_with_surrounding_comments(leading_docs, expr_doc, trailing_docs)
    }

    /// Build expression doc for block expressions (if, each, await, key).
    ///
    /// # Context-dependent behavior
    ///
    /// - **Inline context** (`in_multiline_context=false`): Applies `remove_lines()` to prevent
    ///   the block condition from breaking. When the line exceeds print_width, EARLIER content
    ///   should break instead. Example: `{expr}{#if cond}` - expr breaks, cond stays flat.
    ///
    /// - **Multiline context** (`in_multiline_context=true`): The condition is on its own line.
    ///   No `remove_lines()` is applied, allowing long chains to wrap naturally.
    ///   Uses `LayoutMode::Embedded` for proper continuation indent on wrapped binary expressions.
    ///
    /// # Parameters
    /// - `opening_offset` - Characters before the expression (e.g., 5 for `{#if `). Used to
    ///   calculate `first_line_offset` for width estimation.
    /// - `in_multiline_context` - Whether the block is on its own line (multiline) or inline
    pub(super) fn build_expression_doc_for_block(
        &self,
        expr: &tsv_ts::Expression,
        span_start: u32,
        span_end: u32,
        opening_offset: usize,
        in_multiline_context: bool,
    ) -> DocId {
        let d = self.d();
        let expr_start = expr.span().start;
        let expr_end = expr.span().end;

        // Build docs for leading comments
        let leading_docs: Vec<DocId> =
            tsv_lang::comments_in_range(self.comments, span_start, expr_start)
                .map(|c| self.build_leading_js_comment_doc(c))
                .collect();

        // In multiline contexts, set up embedded expression context so binary chains
        // use ContinuationIndent style. first_line_offset estimates the column position.
        let embed = if in_multiline_context {
            let context_indent = TAB_WIDTH;
            let first_line_offset = context_indent + opening_offset;
            tsv_lang::EmbedContext {
                first_line_offset,
                mode: tsv_lang::LayoutMode::Embedded,
                ..self.embed
            }
        } else {
            self.embed
        };

        // Build expression doc tree
        // Assignment expressions need parens in block conditions: {#if (a = b)}
        let expr_doc = if matches!(expr, tsv_ts::Expression::AssignmentExpression(_)) {
            let inner =
                tsv_ts::build_expression_doc_with_comments(d, expr, &self.ts_inputs(), &embed);
            d.parens(inner)
        } else {
            tsv_ts::build_expression_doc_with_comments(d, expr, &self.ts_inputs(), &embed)
        };

        // Apply remove_lines() only in INLINE contexts to prevent the condition
        // from being the first thing to break when there's other content on the line.
        // In multiline contexts, the condition is on its own line and can wrap naturally.
        let expr_doc = if in_multiline_context {
            expr_doc
        } else {
            d.remove_lines(expr_doc)
        };

        // Build docs for trailing comments
        let trailing_docs: Vec<DocId> =
            tsv_lang::comments_in_range(self.comments, expr_end, span_end)
                .map(|c| self.build_trailing_js_comment_doc(c))
                .collect();

        self.concat_with_surrounding_comments(leading_docs, expr_doc, trailing_docs)
    }

    /// Assemble `[leading…, expr, trailing…]` into one doc, returning `expr` unchanged
    /// when there are no surrounding comments. Shared tail of the expression+comment
    /// builders (`build_expression_with_comments_doc`, `build_expression_doc_for_block`,
    /// `build_const_init_doc`).
    pub(super) fn concat_with_surrounding_comments(
        &self,
        leading_docs: Vec<DocId>,
        expr_doc: DocId,
        trailing_docs: Vec<DocId>,
    ) -> DocId {
        if leading_docs.is_empty() && trailing_docs.is_empty() {
            expr_doc
        } else {
            let mut parts = Vec::with_capacity(leading_docs.len() + 1 + trailing_docs.len());
            parts.extend(leading_docs);
            parts.push(expr_doc);
            parts.extend(trailing_docs);
            self.d().concat(&parts)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{has_any_expanding_blocks, is_control_flow_block, is_expanding_control_flow_block};
    use crate::ast::internal::FragmentNode;

    /// Parse a Svelte template and return its top-level fragment nodes.
    fn parse_nodes(src: &str) -> Vec<FragmentNode> {
        crate::parse(src)
            .expect("template should parse")
            .fragment
            .nodes
    }

    #[test]
    fn control_flow_classification_await_is_not_expanding() {
        let if_nodes = parse_nodes("{#if c}x{/if}");
        assert!(is_control_flow_block(&if_nodes[0]));
        assert!(is_expanding_control_flow_block(&if_nodes[0]));

        let await_nodes = parse_nodes("{#await p}x{/await}");
        assert!(is_control_flow_block(&await_nodes[0]));
        // Await blocks are control-flow but do NOT force expansion.
        assert!(!is_expanding_control_flow_block(&await_nodes[0]));
    }

    #[test]
    fn expanding_block_detected_through_nested_awaits() {
        // An if directly inside an await is detected.
        assert!(has_any_expanding_blocks(&parse_nodes(
            "{#await p}{#if c}x{/if}{/await}"
        )));
        // ...and through a second level of await nesting (recursion).
        assert!(has_any_expanding_blocks(&parse_nodes(
            "{#await p}{#await q}{#if c}x{/if}{/await}{/await}"
        )));
        // An await with only inline/element content does NOT expand.
        assert!(!has_any_expanding_blocks(&parse_nodes(
            "{#await p}<span>x</span>{/await}"
        )));
    }
}
