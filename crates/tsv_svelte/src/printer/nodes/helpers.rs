// Helper utilities for node formatting
//
// Fragment-node classification predicates plus the pattern/expression doc
// builders shared by the block and tag builders, and source position tracking
// used in inline run grouping and multiline formatting decisions.

use crate::ast::internal::{EachBlock, FragmentNode};
use crate::printer::Printer;
use smallvec::{SmallVec, smallvec};
use tsv_lang::Span;
use tsv_lang::TAB_WIDTH;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;
use tsv_ts::Expression;

/// Trailing-comment range end for an `{#each}` head expression: the `as`-pattern
/// start when present, else the head end (`{#each ` … `}` minus the closing `}`).
///
/// Narrowing to the pattern start keeps a comment authored inside the pattern
/// (`{#each items /* c */ as item}`) in place rather than relocating it to trail
/// the collection expression. Shared by the standard and whitespace-sensitive each
/// builders so the two can't drift.
pub(crate) fn each_expr_comment_end(block: &EachBlock<'_>) -> u32 {
    block
        .context
        .as_ref()
        .map_or(block.opening_tag_span.end - 1, |c| c.span().start)
}

/// Check if a fragment node is a control flow block (if/each/await/key/snippet).
///
/// Control flow blocks can hug adjacent inline content when directly adjacent,
/// unlike HTML block elements (`<div>`, `<p>`) which get their own lines.
pub(crate) fn is_control_flow_block(node: &FragmentNode<'_>) -> bool {
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
pub(crate) fn is_expanding_control_flow_block(node: &FragmentNode<'_>) -> bool {
    matches!(
        node,
        FragmentNode::IfBlock(_) | FragmentNode::EachBlock(_) | FragmentNode::KeyBlock(_)
    )
}

/// Check if nodes contain any expanding blocks, either directly or nested in await blocks.
///
/// This is a convenience function combining `is_expanding_control_flow_block` and
/// `has_expanding_block_in_await` checks that are commonly used together.
pub(crate) fn has_any_expanding_blocks(nodes: &[FragmentNode<'_>]) -> bool {
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
pub(crate) fn is_inline_content(node: &FragmentNode<'_>) -> bool {
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
pub(crate) fn has_control_flow_after_sibling(nodes: &[FragmentNode<'_>]) -> bool {
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
fn has_expanding_block_in_await(nodes: &[FragmentNode<'_>]) -> bool {
    nodes.iter().any(|n| {
        if let FragmentNode::AwaitBlock(block) = n {
            // Check all branches of the await block for expanding blocks
            // or recursively for nested awaits containing expanding blocks
            let check_fragment = |f: &crate::ast::internal::Fragment<'_>| {
                f.nodes.iter().any(is_expanding_control_flow_block)
                    || has_expanding_block_in_await(f.nodes)
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
pub(crate) fn has_nested_block_flow(nodes: &[FragmentNode<'_>]) -> bool {
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
    /// Emit every comment in `[start, end)` in **leading** style: a block comment as
    /// `/* … */ ` (inline, trailing space); a line comment as `// …` + `hardline` (a `//`
    /// runs to end of line, so the following token drops to the next line to avoid
    /// swallowing it). Empty doc when the range holds no comments.
    fn build_pattern_leading_comments(&self, start: u32, end: u32) -> DocId {
        let docs: DocBuf = comments_to_emit_in_range(self.comments, start, end)
            .map(|c| self.build_leading_js_comment_doc(c))
            .collect();
        self.d().concat(&docs)
    }

    /// Emit every comment in `[start, end)` in **trailing** style: a block comment as
    /// ` /* … */` (inline, leading space); a line comment as ` // …` + `hardline`. Empty
    /// doc when the range holds no comments.
    fn build_pattern_trailing_comments(&self, start: u32, end: u32) -> DocId {
        let docs: DocBuf = comments_to_emit_in_range(self.comments, start, end)
            .map(|c| self.build_trailing_js_comment_doc(c))
            .collect();
        self.d().concat(&docs)
    }

    /// Build a two-sided delimiter gap (`,` / `:` / `=`) between two pattern pieces,
    /// keeping each comment on the side of the delimiter the author wrote it: comments
    /// before `delim` trail the left piece, comments after lead the right piece — so an
    /// association like `a /* c */ = 1` (comment on the binding) is never flipped onto the
    /// value. `delim_text` is the rendered separator (`", "`, `": "`, `" = "`). Falls back
    /// to the bare separator when the delimiter isn't found (defensive — the parser
    /// guarantees one in every gap this is called on).
    fn build_pattern_delim_gap(
        &self,
        left_end: u32,
        right_start: u32,
        delim: u8,
        delim_text: &'static str,
    ) -> DocId {
        let d = self.d();
        match find_char_skipping_comments(
            self.source.as_bytes(),
            left_end as usize,
            right_start as usize,
            delim,
        ) {
            Some(pos) => {
                let pos = pos as u32;
                let before = self.build_pattern_trailing_comments(left_end, pos);
                let after = self.build_pattern_leading_comments(pos + 1, right_start);
                d.concat(&[before, d.text(delim_text), after])
            }
            None => d.text(delim_text),
        }
    }

    /// Build a `...rest` binding, threading any comment in the `...`→binding gap
    /// (`.../* c */ rest`). Shared by the array-pattern and object-pattern rest arms.
    fn build_rest_pattern_doc(&self, rest_span_start: u32, argument: &Expression<'_>) -> DocId {
        let d = self.d();
        let dots_end = rest_span_start + 3; // past "..."
        let lead = self.build_pattern_leading_comments(dots_end, argument.span().start);
        d.concat(&[d.text("..."), lead, self.build_pattern_doc(argument)])
    }

    /// Build a property key for a non-shorthand object-pattern property, returning
    /// `(doc, key_region_end)` where `key_region_end` is the source position just past the
    /// key (after `]` for a computed key) — the lower bound for the following colon gap. A
    /// computed key threads comments inside its `[ … ]` brackets.
    fn build_pattern_key_doc(
        &self,
        computed: bool,
        key: &Expression<'_>,
        prop_start: u32,
        value_start: u32,
    ) -> (DocId, u32) {
        let d = self.d();
        if computed {
            let key_start = key.span().start;
            let key_end = key.span().end;
            let close = find_char_skipping_comments(
                self.source.as_bytes(),
                key_end as usize,
                value_start as usize,
                b']',
            )
            .map_or(key_end, |p| p as u32);
            let lead = self.build_pattern_leading_comments(prop_start + 1, key_start);
            // Comment-aware so an owned leading comment glued to the key (`[/* c */ k]`)
            // is claimed by the key's own doc — the `[`→key gap emitter above skips it
            // (owned comments are off the positional axis), so nothing else would print it.
            let key_doc = self.build_ts_expression_doc(key);
            let trail = self.build_pattern_trailing_comments(key_end, close);
            let doc = d.concat(&[d.text("["), lead, key_doc, trail, d.text("]")]);
            (doc, close + 1)
        } else {
            (
                self.build_ts_expression_doc_no_comments(key),
                key.span().end,
            )
        }
    }

    /// Build a single object property (`key: value` / shorthand `key`), threading comments
    /// through the computed-key brackets and the `key`→`value` colon gap. A shorthand
    /// property routes its value (an `AssignmentPattern` for a default) back through
    /// `build_pattern_doc`, whose arms carry any interior comments. Shared by the binding
    /// (`ObjectPattern`) and the default-value (`ObjectExpression`) property dispatchers,
    /// which differ only in their enum types — both wrap the same `Property` fields.
    fn build_pattern_property_kv(
        &self,
        shorthand: bool,
        computed: bool,
        key: &Expression<'_>,
        value: &Expression<'_>,
        prop_start: u32,
    ) -> DocId {
        let d = self.d();
        if shorthand {
            self.build_pattern_doc(value)
        } else {
            let value_start = value.span().start;
            let (key_doc, key_region_end) =
                self.build_pattern_key_doc(computed, key, prop_start, value_start);
            let colon_gap = self.build_pattern_delim_gap(key_region_end, value_start, b':', ": ");
            d.concat(&[key_doc, colon_gap, self.build_pattern_doc(value)])
        }
    }

    /// Property dispatcher for a binding `ObjectPattern`.
    fn build_object_pattern_property_doc(&self, prop: &tsv_ts::ObjectPatternProperty<'_>) -> DocId {
        match prop {
            tsv_ts::ObjectPatternProperty::Property(p) => self.build_pattern_property_kv(
                p.shorthand,
                p.computed,
                &p.key,
                &p.value,
                p.span.start,
            ),
            tsv_ts::ObjectPatternProperty::RestElement(r) => {
                self.build_rest_pattern_doc(r.span.start, r.argument)
            }
        }
    }

    /// Property dispatcher for an `ObjectExpression` reached as an assignment-pattern
    /// **default value** (`{ a = { b: /* c */ 1 } }`). Same shape as the binding
    /// dispatcher; only the enum (`ObjectProperty` / `SpreadElement`) differs.
    fn build_object_expr_property_doc(&self, prop: &tsv_ts::ObjectProperty<'_>) -> DocId {
        match prop {
            tsv_ts::ObjectProperty::Property(p) => self.build_pattern_property_kv(
                p.shorthand,
                p.computed,
                &p.key,
                &p.value,
                p.span.start,
            ),
            tsv_ts::ObjectProperty::SpreadElement(s) => {
                self.build_rest_pattern_doc(s.span.start, s.argument)
            }
        }
    }

    /// Build comment-aware object braces (`{ … }`) from pre-built property entries
    /// `(span_start, span_end, doc)`. `bracketSpacing: true` pads non-empty braces;
    /// comments thread through every gap (after `{`, around each `,`, before `}`) and a
    /// dangling comment in an empty pattern is preserved. Shared by the `ObjectPattern`
    /// (binding) and `ObjectExpression` (default-value) arms.
    fn build_object_braces(
        &self,
        span_start: u32,
        span_end: u32,
        entries: &[(u32, u32, DocId)],
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = smallvec![d.text("{")];
        if entries.is_empty() {
            // Empty pattern stays tight (`{}`), but preserve a dangling comment.
            parts.push(self.build_pattern_leading_comments(span_start + 1, span_end - 1));
        } else {
            parts.push(d.text(" "));
            let mut prev_end = span_start + 1; // past `{`
            for (i, &(estart, eend, edoc)) in entries.iter().enumerate() {
                if i == 0 {
                    parts.push(self.build_pattern_leading_comments(prev_end, estart));
                } else {
                    parts.push(self.build_pattern_delim_gap(prev_end, estart, b',', ", "));
                }
                parts.push(edoc);
                prev_end = eend;
            }
            parts.push(self.build_pattern_trailing_comments(prev_end, span_end - 1));
            parts.push(d.text(" "));
        }
        parts.push(d.text("}"));
        d.concat(&parts)
    }

    /// Build comment-aware array brackets (`[ … ]`) — no bracket spacing in either
    /// formatter. Comments thread through every gap (after `[`, around each `,`, before
    /// `]`); a hole (`[a, , b]`) has no element span to anchor against, so its separator
    /// stays a bare comma. Shared by the `ArrayPattern` (binding) and `ArrayExpression`
    /// (default-value) arms, which carry identical `Vec<Option<Expression>>` elements.
    fn build_array_brackets(
        &self,
        elements: &[Option<Expression<'_>>],
        span_start: u32,
        span_end: u32,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = smallvec![d.text("[")];
        let mut prev_end = span_start + 1; // past `[`
        for (i, elem) in elements.iter().enumerate() {
            if i == 0 {
                if let Some(e) = elem {
                    parts.push(self.build_pattern_leading_comments(prev_end, e.span().start));
                }
            } else if let Some(e) = elem {
                parts.push(self.build_pattern_delim_gap(prev_end, e.span().start, b',', ", "));
            } else {
                parts.push(d.text(", "));
            }
            if let Some(e) = elem {
                parts.push(self.build_pattern_doc(e));
                prev_end = e.span().end;
            }
        }
        parts.push(self.build_pattern_trailing_comments(prev_end, span_end - 1));
        parts.push(d.text("]"));
        d.concat(&parts)
    }

    /// Build a doc for a pattern (destructuring context).
    ///
    /// Patterns use specific whitespace rules:
    /// - Object patterns: `{ a, b }` (inner-padded braces, `bracketSpacing: true`)
    /// - Array patterns: `[a, b]` (no spaces inside brackets — `bracketSpacing`
    ///   governs object braces, not array brackets)
    ///
    /// Used for `{#each ... as pattern}`, `{#await ... then pattern}`,
    /// `{:then pattern}`, and `{:catch pattern}` binding contexts. Non-empty object
    /// braces carry an inner space (`{ a, b }`) under the project-wide
    /// `bracketSpacing: true`, consistent with every other object tsv emits and
    /// matching prettier-plugin-svelte; an empty `{}` stays tight.
    ///
    /// **Comments are preserved in place.** A comment in any pattern position (after a
    /// brace/bracket, around a `,` / `:` / `=`, inside a property, before the close) is
    /// threaded through via `comments_to_emit_in_range` and kept where the author wrote it —
    /// block comments inline, line comments line-safely (`//` + `hardline`, so the tail
    /// drops to the next line without swallow). prettier-plugin-svelte prints these
    /// patterns from a comment-blind path and drops them, so this is a
    /// `_svelte_prettier_divergence` (see conformance_prettier.md §Svelte: destructuring
    /// binding-pattern comments).
    ///
    /// Literal **default values** normalize through the TS printer (string quotes +
    /// numeric form), where prettier-plugin-svelte preserves the author's source
    /// token — a separate deliberate divergence (see conformance_prettier.md §Svelte:
    /// destructuring literal normalization).
    /// Append a destructuring pattern's `: T` tail (`{#each xs as { a }: T}`).
    ///
    /// A binding **identifier** carries its annotation through the TypeScript
    /// pattern printer, which `build_pattern_doc`'s default arm routes to. A
    /// destructuring pattern does not: its braces/brackets are built here, on the
    /// Svelte side's own comment-preserving path, so the tail has to be appended
    /// explicitly or it prints nowhere — and an annotation that prints nowhere is
    /// content loss on a format round-trip, not a formatting choice.
    fn append_pattern_type_annotation(
        &self,
        pattern: DocId,
        type_annotation: Option<&tsv_ts::ast::internal::TSTypeAnnotation<'_>>,
    ) -> DocId {
        let Some(annotation) = type_annotation else {
            return pattern;
        };
        let d = self.d();
        let tail = tsv_ts::build_type_annotation_doc_with_comments(
            d,
            annotation,
            &self.ts_inputs(),
            &self.embed,
        );
        d.concat(&[pattern, tail])
    }

    pub(super) fn build_pattern_doc(&self, expr: &Expression<'_>) -> DocId {
        match expr {
            // Comments thread through every gap so a comment in any pattern position is
            // preserved in place — a `_svelte_prettier_divergence` from prettier-plugin-svelte,
            // which drops it. The `*Expression` variants are reached as assignment-pattern
            // **default values** (`{ a = { … } }` / `{ a = [ … ] }`), which prettier likewise
            // keeps inline (so they share the always-inline binding shape, not the breakable
            // expression printer); they just need the same comment-awareness.
            Expression::ObjectPattern(obj) => {
                let entries: SmallVec<[(u32, u32, DocId); 8]> = obj
                    .properties
                    .iter()
                    .map(|p| {
                        let s = p.span();
                        (s.start, s.end, self.build_object_pattern_property_doc(p))
                    })
                    .collect();
                let braces = self.build_object_braces(obj.span.start, obj.span.end, &entries);
                self.append_pattern_type_annotation(braces, obj.type_annotation.as_ref())
            }
            Expression::ObjectExpression(obj) => {
                let entries: SmallVec<[(u32, u32, DocId); 8]> = obj
                    .properties
                    .iter()
                    .map(|p| {
                        let s = p.span();
                        (s.start, s.end, self.build_object_expr_property_doc(p))
                    })
                    .collect();
                self.build_object_braces(obj.span.start, obj.span.end, &entries)
            }
            Expression::ArrayPattern(arr) => {
                let brackets =
                    self.build_array_brackets(arr.elements, arr.span.start, arr.span.end);
                self.append_pattern_type_annotation(brackets, arr.type_annotation.as_ref())
            }
            Expression::ArrayExpression(arr) => {
                self.build_array_brackets(arr.elements, arr.span.start, arr.span.end)
            }
            Expression::RestElement(rest) => {
                self.build_rest_pattern_doc(rest.span.start, rest.argument)
            }
            // Comments around the `=` stay on the side the author wrote them
            // (`a /* c */ = 1` vs `a = /* c */ 1`). The `Expression` variant is the
            // default-value form of the same `=`.
            Expression::AssignmentPattern(assign) => {
                self.build_pattern_assignment(assign.left, assign.right)
            }
            Expression::AssignmentExpression(assign) => {
                self.build_pattern_assignment(assign.left, assign.right)
            }
            // Default: build doc through the comment-aware TS builder. Literals route
            // here too, so string and numeric defaults normalize through the TS printer
            // (single quotes + escaping, lowercase hex/exponent, leading/trailing zeros) —
            // identical to `{@const}` and every other literal tsv emits.
            // prettier-plugin-svelte instead prints these binding patterns from raw
            // source, preserving the author's quote style and numeric form; tsv
            // normalizes uniformly (a deliberate divergence — see conformance_prettier.md
            // §Svelte: destructuring literal normalization).
            //
            // Comment-aware (not `_no_comments`) so an owned leading comment glued to the
            // leaf — a default value (`{ a = /* c */ 1 }`), a rename value (`{ g: /* c */ h }`),
            // an array element (`[/* c */ m]`), a rest argument (`.../* c */ rest`) — is
            // claimed by the leaf's own doc via `prepend_owned_leading_comment`. Every gap
            // emitter around it skips owned comments (the to-emit axis), so without this the
            // comment reaches no printer and is dropped. The surrounding positional gap
            // comments stay with the pattern printer: `build_expression_doc` prepends only
            // the owned leading comment, never a positional one, so there is no double-emit.
            _ => self.build_ts_expression_doc(expr),
        }
    }

    /// Build a doc for a binding assignment (`left = right`) — the shared body of
    /// the `AssignmentPattern` (default-value binding) and `AssignmentExpression`
    /// pattern arms. Comments around the `=` stay on the side the author wrote
    /// them (`a /* c */ = 1` vs `a = /* c */ 1`) via `build_pattern_delim_gap`.
    fn build_pattern_assignment(&self, left: &Expression<'_>, right: &Expression<'_>) -> DocId {
        let d = self.d();
        let left_doc = self.build_pattern_doc(left);
        let eq = self.build_pattern_delim_gap(left.span().end, right.span().start, b'=', " = ");
        let right_doc = self.build_pattern_doc(right);
        d.concat(&[left_doc, eq, right_doc])
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
        expr: &Expression<'_>,
        span_start: u32,
        span_end: u32,
    ) -> DocId {
        let d = self.d();
        let expr_start = expr.span().start;
        let expr_end = expr.span().end;

        // Build docs for leading comments (between span_start and expression start)
        let leading_docs: DocBuf = comments_to_emit_in_range(self.comments, span_start, expr_start)
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
        let trailing_docs: DocBuf = comments_to_emit_in_range(self.comments, expr_end, span_end)
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
        expr: &Expression<'_>,
        span_start: u32,
        span_end: u32,
        opening_offset: usize,
        in_multiline_context: bool,
    ) -> DocId {
        let d = self.d();
        let expr_start = expr.span().start;
        let expr_end = expr.span().end;

        // Build docs for leading comments
        let leading_docs: DocBuf = comments_to_emit_in_range(self.comments, span_start, expr_start)
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
        let expr_doc = if matches!(expr, Expression::AssignmentExpression(_)) {
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
        let trailing_docs: DocBuf = comments_to_emit_in_range(self.comments, expr_end, span_end)
            .map(|c| self.build_trailing_js_comment_doc(c))
            .collect();

        self.concat_with_surrounding_comments(leading_docs, expr_doc, trailing_docs)
    }

    /// Build a block head's expression doc, deriving both the comment-scan start offset
    /// and the width-estimation `opening_offset` from the opening literal `open` — so the
    /// two can't drift from the emitted text (the same invariant the `*_BLOCK_OPEN` const
    /// comment guards). `comment_end` bounds the leading/trailing-comment scan (the head
    /// end for `{#if}`/`{:else if}`/`{#key}`, or the pattern-start-narrowed end for
    /// `{#each}`/`{#await}` via `each_expr_comment_end` / the await shorthand). `wrapping`
    /// is `in_multiline_context` (always false inside a whitespace-sensitive element).
    ///
    /// Wraps [`Printer::build_expression_doc_for_block`] for the standard block heads; the
    /// parenthesized `{#each (key)}` offset is derived from `key_span`, not `open`, so those
    /// call `build_expression_doc_for_block` directly.
    pub(super) fn build_block_head_expr(
        &self,
        open: &'static str,
        opening_tag_span: Span,
        expr: &Expression<'_>,
        comment_end: u32,
        wrapping: bool,
    ) -> DocId {
        self.build_expression_doc_for_block(
            expr,
            opening_tag_span.start + open.len() as u32,
            comment_end,
            open.len(),
            wrapping,
        )
    }

    /// Assemble `[leading…, expr, trailing…]` into one doc, returning `expr` unchanged
    /// when there are no surrounding comments. Shared tail of the expression+comment
    /// builders (`build_expression_with_comments_doc`, `build_expression_doc_for_block`,
    /// `build_const_init_doc`).
    pub(super) fn concat_with_surrounding_comments(
        &self,
        leading_docs: DocBuf,
        expr_doc: DocId,
        trailing_docs: DocBuf,
    ) -> DocId {
        if leading_docs.is_empty() && trailing_docs.is_empty() {
            expr_doc
        } else {
            let mut parts: DocBuf =
                DocBuf::with_capacity(leading_docs.len() + 1 + trailing_docs.len());
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

    /// Parse a Svelte template and return its top-level fragment nodes. The
    /// nodes borrow the caller-owned `arena`, so each test holds the `Bump`.
    fn parse_nodes<'arena>(
        arena: &'arena bumpalo::Bump,
        src: &str,
    ) -> &'arena [FragmentNode<'arena>] {
        crate::parse(src, arena)
            .expect("template should parse")
            .fragment
            .nodes
    }

    #[test]
    fn control_flow_classification_await_is_not_expanding() {
        let arena = bumpalo::Bump::new();
        let if_nodes = parse_nodes(&arena, "{#if c}x{/if}");
        assert!(is_control_flow_block(&if_nodes[0]));
        assert!(is_expanding_control_flow_block(&if_nodes[0]));

        let await_nodes = parse_nodes(&arena, "{#await p}x{/await}");
        assert!(is_control_flow_block(&await_nodes[0]));
        // Await blocks are control-flow but do NOT force expansion.
        assert!(!is_expanding_control_flow_block(&await_nodes[0]));
    }

    #[test]
    fn expanding_block_detected_through_nested_awaits() {
        let arena = bumpalo::Bump::new();
        // An if directly inside an await is detected.
        assert!(has_any_expanding_blocks(parse_nodes(
            &arena,
            "{#await p}{#if c}x{/if}{/await}"
        )));
        // ...and through a second level of await nesting (recursion).
        assert!(has_any_expanding_blocks(parse_nodes(
            &arena,
            "{#await p}{#await q}{#if c}x{/if}{/await}{/await}"
        )));
        // An await with only inline/element content does NOT expand.
        assert!(!has_any_expanding_blocks(parse_nodes(
            &arena,
            "{#await p}<span>x</span>{/await}"
        )));
    }
}
