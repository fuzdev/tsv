// Doc builders for Svelte control-flow blocks
//
// {#if}/{:else if}/{:else}, {#each}, {#await}, {#key}, and {#snippet} —
// opening/closing tag layout, branch flattening, and section bodies.

// Allow Svelte block syntax like `{:else}`, `{:then}`, `{:catch}` which
// look like Rust format args but are valid Svelte template syntax.
#![allow(clippy::literal_string_with_formatting_args)]

use crate::ast::internal::{self, Fragment, FragmentNode};
use crate::printer::Printer;
use smallvec::smallvec;
use tsv_lang::doc::arena::DocId;
use tsv_lang::doc::{DocBuf, GroupId};

use super::helpers::{each_expr_comment_end, indent_body};

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
fn build_await_section_body(printer: &Printer<'_>, fragment: &Fragment) -> (DocId, bool) {
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
fn indent_body_soft(printer: &Printer<'_>, body_doc: DocId) -> DocId {
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

/// Whether a wrapped block head's clause + `}` should hug the expression's last
/// line (`) as item}`, `)}`) rather than dropping to their own line.
///
/// True only for a single call/`new` whose callee contains no other call: when its
/// arguments wrap, the `)` dedents to the tag's base indent, so the rendered last
/// line starts with `)` and (per the layout rule) must not be broken — the clause +
/// `}` continue on it. A binary/logical chain (last line is an operand), a
/// multi-segment member chain (last line is a `.method(...)` segment, not a bare
/// `)`), or anything else breaks the clause + `}` to their own line at base.
fn clause_hugs_expr(expr: &tsv_ts::Expression) -> bool {
    use tsv_ts::Expression as E;
    // A callee with no nested call means the only `(` belongs to this call, so its
    // `)` lands at the tag base when the args wrap (vs. a chain, whose segments indent).
    fn callee_has_no_call(e: &E) -> bool {
        match e {
            E::Identifier(_) | E::ThisExpression(_) | E::Super(_) => true,
            E::MemberExpression(m) => callee_has_no_call(&m.object),
            _ => false,
        }
    }
    match expr {
        E::CallExpression(c) => callee_has_no_call(&c.callee),
        E::NewExpression(n) => callee_has_no_call(&n.callee),
        _ => false,
    }
}

/// How an `{#await}` carries its first section in the head, keyed on the **absence of a
/// pending body** (the binding is optional). Single classification shared by the head-clause
/// builder and the tail builders (which skip the head-carried keyword) — so the two can't drift.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AwaitShorthand {
    /// `{#await x then v}` / bare `{#await x then}` — no pending body, a `then` section.
    Then,
    /// `{#await x catch e}` / bare `{#await x catch}` — no pending, no `then`, a `catch` section.
    Catch,
    /// Full form (`{#await x}…{:then}…{:catch}…`) — the head carries no section clause.
    None,
}

/// Classify an await block's head shorthand. See [`AwaitShorthand`].
fn await_shorthand(block: &internal::AwaitBlock) -> AwaitShorthand {
    if block.pending.is_some() {
        AwaitShorthand::None
    } else if block.then.is_some() {
        AwaitShorthand::Then
    } else if block.error.is_some() || block.catch.is_some() {
        AwaitShorthand::Catch
    } else {
        AwaitShorthand::None
    }
}

impl<'a> Printer<'a> {
    /// Whether the trailing comments in `[start, end)` end with a line (`//`) comment.
    ///
    /// A trailing line comment is emitted (by `build_trailing_js_comment_doc`) with a
    /// closing `hardline` — a `//` runs to end of line, so the following clause + `}`
    /// already drop to the next line. The head dangle (and the flat hug) must then not
    /// add their own break, or a spurious blank line / leading space appears. A trailing
    /// *block* comment carries no `hardline`, so it does not suppress the dangle.
    pub(super) fn head_trailing_line_comment(&self, start: u32, end: u32) -> bool {
        tsv_lang::comments_in_range(self.comments, start, end)
            .last()
            .is_some_and(|c| !c.is_block)
    }

    /// Whether a wrapped block head may dangle its `}` (and expand its body) here. The
    /// head expression is allowed to break (`allow_wrapping` or a multiline context) AND
    /// the context permits the dangle — false only inside a whitespace-significant element
    /// (`<pre>` / `<textarea>`), gated by [`Printer::block_dangle_allowed`]. Gating it off
    /// only hugs the `}`; the expression still wraps to respect printWidth either way.
    fn block_head_can_wrap(&self, allow_wrapping: bool, in_multiline_context: bool) -> bool {
        (allow_wrapping || in_multiline_context) && self.block_dangle_allowed()
    }

    /// Wrap a block-tag head so its closing `}` dangles on its own line when the head wraps.
    ///
    /// `open` is the opening literal (`{#if `, `{#each `, …) and `head_inner` is everything
    /// between it and the closing `}` — the breakable expression plus any clause (` as item`,
    /// ` then value`). When `can_wrap` is set (the same condition under which the expression is
    /// allowed to break), `[head_inner, softline]` is grouped so the trailing softline — and thus
    /// `}` — drops to its own line at the tag's base indent whenever the head exceeds print width
    /// (a deliberate `_prettier_divergence`, consistent with tsv's JS `if (⏎…⏎) {` and broken-element
    /// `>`). When the head fits, the softline collapses and `}` hugs the head, byte-identical to the
    /// inline form. When `can_wrap` is false (inline context, `remove_lines` applied) the head is
    /// emitted flat with `}` hugged, unchanged.
    ///
    /// `expr_ends_with_line_comment` (from `head_trailing_line_comment`) short-circuits both
    /// paths: the comment's own `hardline` already drops the clause + `}` to the next line, so
    /// the dangle/hug break is skipped to avoid a spurious blank line.
    pub(super) fn build_block_head_doc(
        &self,
        open: &'static str,
        expr_doc: DocId,
        clause: Option<DocId>,
        can_wrap: bool,
        hug: bool,
        expr_ends_with_line_comment: bool,
    ) -> DocId {
        let d = self.d();
        let open_doc = d.text(open);
        let close = d.text("}");
        if expr_ends_with_line_comment {
            // The trailing line comment already emitted a `hardline` that drops the
            // clause + `}` to the next line at base indent. Emit them directly with no
            // dangle/hug break. Still group the expression on the wrapping path so the
            // body-expand keyed to `BlockHead` sees the (comment-forced) break.
            let head = if can_wrap {
                d.group_with_id(expr_doc, GroupId::BlockHead)
            } else {
                expr_doc
            };
            return match clause {
                Some(c) => d.concat(&[open_doc, head, c, close]),
                None => d.concat(&[open_doc, head, close]),
            };
        }
        if can_wrap {
            // Key the breakable expression to `GroupId::BlockHead`. The head group's
            // `fits()` counts the trailing clause + `}` (they sit in rest-commands /
            // resolve flat during the fits test), so the head breaks at the right
            // boundary; reading anything keyed to `BlockHead` immediately after the
            // group resolves keeps the shared id nesting-safe.
            let grouped = d.group_with_id(expr_doc, GroupId::BlockHead);
            if hug {
                // The expression renders ending with `)` on its own line at base (a
                // single call whose args wrapped). Per the layout rule, don't break a
                // line that starts with `)` — the clause + `}` continue on it
                // (`) as item}`, `)}`), in both the flat and broken head.
                match clause {
                    Some(c) => {
                        let space = d.text(" ");
                        d.concat(&[open_doc, grouped, space, c, close])
                    }
                    None => d.concat(&[open_doc, grouped, close]),
                }
            } else {
                // Binary chain / member chain / etc.: the clause + `}` drop to their
                // own line at the tag's base indent when the head wraps (`expr⏎as item}`,
                // `expr⏎}`); when it fits they hug inline (`expr as item}`).
                let hardline = d.hardline();
                let (break_tail, flat_tail) = match clause {
                    Some(c) => {
                        let space = d.text(" ");
                        (d.concat(&[hardline, c]), d.concat(&[space, c]))
                    }
                    None => (hardline, d.empty()),
                };
                let dangle = d.if_break_with_id(break_tail, flat_tail, GroupId::BlockHead);
                d.concat(&[open_doc, grouped, dangle, close])
            }
        } else {
            match clause {
                Some(c) => {
                    let space = d.text(" ");
                    d.concat(&[open_doc, expr_doc, space, c, close])
                }
                None => d.concat(&[open_doc, expr_doc, close]),
            }
        }
    }

    /// The shared block-head tail every block builder ends with: detect whether the
    /// head expression's trailing comments (over `[expr.end, comment_end)`) end with a
    /// line comment, then build the head doc via [`Printer::build_block_head_doc`].
    ///
    /// `expr` is the head expression — used for its span and the `clause_hugs_expr`
    /// classification; `expr_doc` is the already-built expression doc (the `{#each}`
    /// degenerate index/key form passes a concat of the expression plus its tail here,
    /// so the two are distinct). `clause` is the optional ` as …` / ` then …` / ` catch …`
    /// tail (without leading space), and `comment_end` bounds the trailing-comment scan
    /// (the head end for `{#if}`/`{:else if}`/`{#key}`, or the pattern-start-narrowed
    /// end for `{#each}`/`{#await}`). `can_wrap` stays caller-computed — its sources
    /// differ across builders and several reuse it afterward for the body-drop.
    fn build_block_head(
        &self,
        open: &'static str,
        expr: &tsv_ts::Expression,
        expr_doc: DocId,
        clause: Option<DocId>,
        comment_end: u32,
        can_wrap: bool,
    ) -> DocId {
        let elc = self.head_trailing_line_comment(expr.span().end, comment_end);
        self.build_block_head_doc(
            open,
            expr_doc,
            clause,
            can_wrap,
            clause_hugs_expr(expr),
            elc,
        )
    }

    /// Build a **section-free** block (key, plain each/if/await, snippet) whose body
    /// is inline-authored, expanding the body + `{/tag}` onto their own lines when the
    /// head goes multiline.
    ///
    /// `head_doc` is the opening tag through its `}` (including the `BlockHead`
    /// head-wrap group + dangle); `body_doc` / `close` are the inline body and the
    /// `{/tag}` close.
    ///
    /// A `conditional_group` chooses in one pass among (1) fully inline, (2) flat head +
    /// expanded body (the construct overflows but the head fits alone — the expanded
    /// body's leading `hardline` ends the head's `fits()` lookahead, so the head
    /// measures *head-alone*), and (3) wrapped head + expanded body (the head alone
    /// overflows, so it wraps and its `}` dangles). Decoupling head-wrap (head-alone
    /// width) from body-expand (head+body width) keeps every layout a one-pass fixed
    /// point: a one-line input in the "middle zone" (head fits alone, head+body
    /// doesn't) converges directly to layout 2 instead of wrapping then un-wrapping
    /// across two passes. This holds for **every** body shape — text, expression,
    /// void, and element/component — which all drop to their own line on overflow.
    fn build_expanding_block(
        &self,
        head_doc: DocId,
        body_doc: DocId,
        close: DocId,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        let lead = d.hardline();
        let trail = d.hardline();
        let multiline_tail = d.concat(&[d.indent(d.concat(&[lead, body_doc])), trail, close]);
        // Inline tail keeps the body's own `indent()` wrapper (so a body that breaks
        // internally still indents); the close hugs the body.
        let inline_tail = d.concat(&[d.indent(body_doc), close]);
        self.build_expanding_construct(head_doc, inline_tail, multiline_tail, gt_prefix)
    }

    /// Prepend a split-off preceding sibling's closing `>` (`gt_prefix`) to a block
    /// candidate: hugged in the inline candidate (`>{#…}`) and dangled onto its own line
    /// in a multiline candidate (`⏎>{#…}`), so the `>` tracks the block's own
    /// inline-vs-multiline choice. `None` leaves the candidate untouched. See the axis-3
    /// sibling-`>` dangle in `build_inline_element_omit_close_gt`.
    fn fold_gt(&self, gt_prefix: Option<DocId>, dangle: bool, body: DocId) -> DocId {
        let d = self.d();
        match gt_prefix {
            Some(gt) if dangle => d.concat(&[d.hardline(), gt, body]),
            Some(gt) => d.concat(&[gt, body]),
            None => body,
        }
    }

    /// Core of the expand-when-the-construct-overflows layout, over a precomputed
    /// `inline_tail` (everything after the head's `}` hugged onto one line) and
    /// `multiline_tail` (the same content with each body/section/branch on its own
    /// line). Shared by the section-free blocks (via `build_expanding_block`),
    /// `{#if}`/`{#each}` with `{:else}`/`{:else if}` alternates, and `{#await}`
    /// (multiple sections, via `build_await_tail`).
    ///
    /// A `conditional_group` picks fully-inline / flat-head + expanded / wrapped-head +
    /// expanded in one pass, decoupling head-wrap (head-alone width) from body-expand
    /// (head+tail width) so every layout is a one-pass fixed point (no two-pass
    /// wrap-then-unwrap in the middle zone). The body **always drops to its own line**
    /// when the construct overflows — uniformly across text, expression, void, and
    /// element/component bodies (a deliberate divergence from prettier, which hugs the
    /// `}` and breaks an element body internally; see
    /// `conformance_prettier.md` §Svelte: Blocks). The `conditional_group` fits-tests
    /// each candidate in flat mode, so an element body's inline candidate does not
    /// "fit by breaking internally" — it falls through to the expanded (drop) state.
    ///
    /// **Head forced multiline** (a trailing line comment) → expand directly; its
    /// hardline would otherwise short-circuit the `conditional_group`'s `fits()` to
    /// "fits" and wrongly hug the tail.
    fn build_expanding_construct(
        &self,
        head_doc: DocId,
        inline_tail: DocId,
        multiline_tail: DocId,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        if d.will_break(head_doc) {
            return self.fold_gt(gt_prefix, true, d.concat(&[head_doc, multiline_tail]));
        }
        let inline = self.fold_gt(gt_prefix, false, d.concat(&[head_doc, inline_tail]));
        let expanded = self.fold_gt(gt_prefix, true, d.concat(&[head_doc, multiline_tail]));
        d.conditional_group(&[inline, expanded])
    }

    /// Whether every if-block branch (consequent, each `{:else if}` consequent,
    /// `{:else}`) is inline-authored — the precondition for the body-expand fast path.
    fn if_branches_all_inline(&self, block: &internal::IfBlock, imc: bool) -> bool {
        let mut all_inline = self.fragment_inline_authored(&block.consequent, imc);
        let mut alt = block.alternate.as_ref();
        while let Some(a) = alt {
            if let Some(else_if) = Self::get_flattenable_else_if(a) {
                all_inline &= self.fragment_inline_authored(&else_if.consequent, imc);
                alt = else_if.alternate.as_ref();
            } else {
                all_inline &= self.fragment_inline_authored(a, imc);
                alt = None;
            }
        }
        all_inline
    }

    /// Indent a section body, dropping it to its own line (leading `hardline`) when
    /// `multiline`; otherwise indent it in place (so a body that breaks internally still
    /// indents). The shared body-expand primitive for every block tail's body / branch /
    /// section / fallback.
    fn indent_body_expand(&self, body: DocId, multiline: bool) -> DocId {
        let d = self.d();
        if multiline {
            d.indent(d.concat(&[d.hardline(), body]))
        } else {
            d.indent(body)
        }
    }

    /// Build the if-block tail (consequent body + alternate branches + `{/if}`) in
    /// inline (`multiline = false`) or expanded (`multiline = true`) form, for
    /// `build_expanding_construct`. Each `{:else if}` keeps its own head (with dangle),
    /// so a long else-if head still wraps within the expanded form.
    fn build_if_tail(&self, block: &internal::IfBlock, multiline: bool) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = DocBuf::new();
        let cons = self.build_fragment_doc(&block.consequent);
        parts.push(self.indent_body_expand(cons, multiline));
        if let Some(alt) = &block.alternate {
            parts.push(self.build_if_alternate_tail(alt, multiline));
        }
        if multiline {
            parts.push(d.hardline());
        }
        parts.push(d.text("{/if}"));
        d.concat(&parts)
    }

    /// Build the alternate part of an if tail (`{:else if …}` chains / `{:else}`),
    /// recursively, in inline or expanded form. Excludes the closing `{/if}` (added by
    /// `build_if_tail`).
    fn build_if_alternate_tail(&self, alt: &Fragment, multiline: bool) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = DocBuf::new();
        if let Some(else_if) = Self::get_flattenable_else_if(alt) {
            // Build the else-if head with wrapping enabled so it can dangle within the
            // expanded form; in the inline form `BlockHead` resolves flat (no dangle).
            let expr_doc = self.build_else_if_expr_doc(else_if, true);
            let head_doc = self.build_block_head(
                ELSE_IF_BLOCK_OPEN,
                &else_if.test,
                expr_doc,
                None,
                else_if.opening_tag_span.end - 1,
                self.block_dangle_allowed(),
            );
            if multiline {
                parts.push(d.hardline());
            }
            parts.push(head_doc);
            let body = self.build_fragment_doc(&else_if.consequent);
            parts.push(self.indent_body_expand(body, multiline));
            if let Some(nested) = &else_if.alternate {
                parts.push(self.build_if_alternate_tail(nested, multiline));
            }
        } else {
            if multiline {
                parts.push(d.hardline());
            }
            parts.push(d.text("{:else}"));
            let body = self.build_nodes_doc(&alt.nodes);
            parts.push(self.indent_body_expand(body, multiline));
        }
        d.concat(&parts)
    }

    /// Build if block doc with full context (multiline + preceding content).
    ///
    /// `has_preceding_breakable`: If true, there's breakable content before this block,
    /// so use remove_lines() to ensure that content breaks first.
    ///
    /// `gt_prefix`: a preceding inline-element sibling's split-off closing `>` to fold
    /// into the block's inline-vs-multiline decision (axis-3 sibling-`>` dangle). Only
    /// passed for the expanding path (see `block_sibling_gt_dangle_eligible`).
    pub(super) fn build_if_block_doc_with_full_context(
        &self,
        block: &internal::IfBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        // Build expression doc with context-dependent behavior
        // Use remove_lines only if there's preceding breakable content (so it breaks first).
        // Otherwise, allow natural wrapping to respect print_width.
        let allow_wrapping = !has_preceding_breakable;
        let expr_doc = self.build_block_head_expr(
            IF_BLOCK_OPEN,
            block.opening_tag_span,
            &block.test,
            block.opening_tag_span.end - 1,
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

        let can_wrap = self.block_head_can_wrap(allow_wrapping, in_multiline_context);
        let head_doc = self.build_block_head(
            IF_BLOCK_OPEN,
            &block.test,
            expr_doc,
            None,
            block.opening_tag_span.end - 1,
            can_wrap,
        );

        // Inline-authored block (consequent + every alternate branch): expand the
        // whole block — bodies, `{:else if}`/`{:else}` sections, and `{/if}` — onto
        // their own lines when the head wraps (or the construct overflows).
        if self.if_branches_all_inline(block, in_multiline_context) && can_wrap {
            let inline_tail = self.build_if_tail(block, false);
            let multiline_tail = self.build_if_tail(block, true);
            return self.build_expanding_construct(
                head_doc,
                inline_tail,
                multiline_tail,
                gt_prefix,
            );
        }

        let mut parts: DocBuf = smallvec![head_doc, indented_body];

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
        self.build_block_head_expr(
            ELSE_IF_BLOCK_OPEN,
            else_if.opening_tag_span,
            &else_if.test,
            else_if.opening_tag_span.end - 1,
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

            // `build_else_if_expr_doc` builds the condition with `in_multiline_context`
            // as its wrapping flag, so the dangle keys on the same condition.
            let head_doc = self.build_block_head(
                ELSE_IF_BLOCK_OPEN,
                &else_if.test,
                expr_doc,
                None,
                else_if.opening_tag_span.end - 1,
                in_multiline_context && self.block_dangle_allowed(),
            );
            let mut parts: DocBuf = smallvec![head_doc, indented_body];

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

    /// Whether the each block's body and its optional `{:else}` fallback are both
    /// inline-authored — the precondition for the body-expand fast path.
    fn each_branches_all_inline(&self, block: &internal::EachBlock, imc: bool) -> bool {
        let mut all_inline = self.fragment_inline_authored(&block.body, imc);
        if let Some(fallback) = &block.fallback {
            all_inline &= self.fragment_inline_authored(fallback, imc);
        }
        all_inline
    }

    /// Build the each-block tail (body + optional `{:else}` fallback + `{/each}`) in
    /// inline or expanded form, for `build_expanding_construct`.
    fn build_each_tail(&self, block: &internal::EachBlock, multiline: bool) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = DocBuf::new();
        let body = self.build_fragment_doc(&block.body);
        parts.push(self.indent_body_expand(body, multiline));
        if let Some(fallback) = &block.fallback {
            if multiline {
                parts.push(d.hardline());
            }
            parts.push(d.text("{:else}"));
            let fb = self.build_fragment_doc(fallback);
            parts.push(self.indent_body_expand(fb, multiline));
        }
        if multiline {
            parts.push(d.hardline());
        }
        parts.push(d.text("{/each}"));
        d.concat(&parts)
    }

    /// Build each block doc with full context (multiline + preceding content).
    ///
    /// `gt_prefix`: see `build_if_block_doc_with_full_context`.
    pub(super) fn build_each_block_doc_with_full_context(
        &self,
        block: &internal::EachBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        // Build expression doc with context-dependent behavior
        let allow_wrapping = !has_preceding_breakable;
        let expr_comment_end = each_expr_comment_end(block);
        let expr_doc = self.build_block_head_expr(
            EACH_BLOCK_OPEN,
            block.opening_tag_span,
            &block.expression,
            expr_comment_end,
            allow_wrapping || in_multiline_context,
        );

        // Build the optional key doc (shared between the clause and degenerate paths).
        let key_doc = block.key.as_ref().map(|key| {
            // The key expression is inside parens, so the offset accounts for that.
            if let Some(key_span) = block.key_span {
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
            }
        });

        // Separate the breakable expression from its clause so the clause + `}` can
        // dedent together onto their own line when the head wraps. `clause` is the
        // `as pattern[, index][ (key)]` tail WITHOUT its leading space (added by
        // `build_block_head_doc`); the degenerate index/key-without-`as` cases (not
        // valid Svelte) keep hugging the expression unchanged.
        let (head_expr, clause) = if let Some(context) = &block.context {
            let mut clause_parts: DocBuf = smallvec![d.text("as ")];
            clause_parts.push(self.build_pattern_doc(context));
            if let Some(index) = &block.index {
                clause_parts.push(d.text(", "));
                clause_parts.push(d.text_owned(index.clone()));
            }
            if let Some(kd) = key_doc {
                clause_parts.push(d.text(" ("));
                clause_parts.push(kd);
                clause_parts.push(d.text(")"));
            }
            (expr_doc, Some(d.concat(&clause_parts)))
        } else {
            // No `as`: any index/key is degenerate — keep it hugging the expression.
            let mut e: DocBuf = smallvec![expr_doc];
            if let Some(index) = &block.index {
                e.push(d.text(", "));
                e.push(d.text_owned(index.clone()));
            }
            if let Some(kd) = key_doc {
                e.push(d.text(" ("));
                e.push(kd);
                e.push(d.text(")"));
            }
            (d.concat(&e), None)
        };

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

        let can_wrap = self.block_head_can_wrap(allow_wrapping, in_multiline_context);
        let head_doc = self.build_block_head(
            EACH_BLOCK_OPEN,
            &block.expression,
            head_expr,
            clause,
            expr_comment_end,
            can_wrap,
        );

        // Inline-authored block (body + `{:else}` fallback): expand the body,
        // `{:else}` section, and `{/each}` onto their own lines when the head wraps
        // (or the construct overflows).
        if self.each_branches_all_inline(block, in_multiline_context) && can_wrap {
            let inline_tail = self.build_each_tail(block, false);
            let multiline_tail = self.build_each_tail(block, true);
            return self.build_expanding_construct(
                head_doc,
                inline_tail,
                multiline_tail,
                gt_prefix,
            );
        }

        let mut parts: DocBuf = smallvec![head_doc, indented_body];

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

    /// Whether a fragment is inline-authored (no leading/trailing whitespace — incl.
    /// space-only — and no forced break) — the precondition for the body-expand fast
    /// path on `{#if}` / `{#each}` / `{#await}` bodies, branches, and sections. Uses
    /// the same `fragment_ws_status` the non-fast-path `is_inline` uses, so space-only
    /// / newline-authored fragments fall through to the existing whitespace-respecting
    /// paths.
    fn fragment_inline_authored(&self, frag: &Fragment, in_multiline_context: bool) -> bool {
        let (has_leading, has_trailing) = self.fragment_ws_status(frag, in_multiline_context);
        !has_leading && !has_trailing && !self.fragment_should_force_break_content(&frag.nodes)
    }

    /// Build one await section body, indented; in multiline mode it drops to its own
    /// line (leading `hardline`), matching the other blocks' body-expand.
    fn await_section_body_expand(&self, frag: &Fragment, multiline: bool) -> DocId {
        self.indent_body_expand(self.build_fragment_doc(frag), multiline)
    }

    /// The `{:then …}` keyword doc — `{:then value}` if a `then` value binds, else
    /// `{:then}` if the then-section has content, else `None`. Whether to emit it is the
    /// caller's decision: a `then`-shorthand carries it in the head instead.
    fn await_then_keyword(&self, block: &internal::AwaitBlock) -> Option<DocId> {
        let d = self.d();
        if let Some(value) = &block.value {
            Some(d.concat(&[
                d.text("{:then "),
                self.build_pattern_doc(value),
                d.text("}"),
            ]))
        } else if block.then.as_ref().is_some_and(|t| !t.nodes.is_empty()) {
            Some(d.text("{:then}"))
        } else {
            None
        }
    }

    /// The `{:catch …}` keyword doc — `{:catch error}` if an error binds, else `{:catch}`
    /// if the catch-section has content, else `None`. A `catch`-shorthand carries it in the
    /// head instead.
    fn await_catch_keyword(&self, block: &internal::AwaitBlock) -> Option<DocId> {
        let d = self.d();
        if let Some(error) = &block.error {
            Some(d.concat(&[
                d.text("{:catch "),
                self.build_pattern_doc(error),
                d.text("}"),
            ]))
        } else if block.catch.as_ref().is_some_and(|c| !c.nodes.is_empty()) {
            Some(d.text("{:catch}"))
        } else {
            None
        }
    }

    /// Which shorthand carries its clause in the head, so the tail omits that keyword:
    /// `(then-shorthand, catch-shorthand)`. Derived from [`await_shorthand`] so it can't drift
    /// from the head-clause builder.
    fn await_shorthand_flags(block: &internal::AwaitBlock) -> (bool, bool) {
        match await_shorthand(block) {
            AwaitShorthand::Then => (true, false),
            AwaitShorthand::Catch => (false, true),
            AwaitShorthand::None => (false, false),
        }
    }

    /// Build the await tail (everything after the head's `}`): the section bodies, the
    /// `{:then …}` / `{:catch …}` keywords (only the ones NOT carried in the head by a
    /// shorthand), and `{/await}`. In `multiline` mode every section + keyword +
    /// `{/await}` is on its own line; otherwise they hug. Feeds
    /// `build_expanding_construct`, so a wrapped await head expands all sections like
    /// the other blocks.
    fn build_await_tail(&self, block: &internal::AwaitBlock, multiline: bool) -> DocId {
        let d = self.d();
        let (is_then_shorthand, is_catch_shorthand) = Self::await_shorthand_flags(block);
        let mut parts: DocBuf = DocBuf::new();
        if let Some(pending) = &block.pending {
            parts.push(self.await_section_body_expand(pending, multiline));
        }
        if !is_then_shorthand && let Some(kw) = self.await_then_keyword(block) {
            if multiline {
                parts.push(d.hardline());
            }
            parts.push(kw);
        }
        if let Some(then_block) = &block.then {
            parts.push(self.await_section_body_expand(then_block, multiline));
        }
        if !is_catch_shorthand && let Some(kw) = self.await_catch_keyword(block) {
            if multiline {
                parts.push(d.hardline());
            }
            parts.push(kw);
        }
        if let Some(catch_block) = &block.catch {
            parts.push(self.await_section_body_expand(catch_block, multiline));
        }
        if multiline {
            parts.push(d.hardline());
        }
        parts.push(d.text("{/await}"));
        d.concat(&parts)
    }

    /// Build the await tail for the **space-only** layout: each present section body
    /// (`indent_body_soft`) and each un-shorthanded `{:then}` / `{:catch}` keyword are
    /// separated by `line()` docs, so the whole construct breaks together as a unit under
    /// the caller's `group`. Mirrors `build_await_tail`, but with `line()` separators and
    /// soft-indented bodies (the head is prepended + grouped by the caller).
    fn build_await_tail_space_only(&self, block: &internal::AwaitBlock) -> DocId {
        let d = self.d();
        let (is_then_shorthand, is_catch_shorthand) = Self::await_shorthand_flags(block);
        let mut parts: DocBuf = DocBuf::new();
        if let Some(pending) = &block.pending {
            let body = self.build_nodes_doc_multiline(&pending.nodes);
            parts.push(indent_body_soft(self, body));
        }
        if !is_then_shorthand && let Some(kw) = self.await_then_keyword(block) {
            parts.push(d.line());
            parts.push(kw);
        }
        if let Some(then_block) = &block.then {
            let body = self.build_nodes_doc_multiline(&then_block.nodes);
            parts.push(indent_body_soft(self, body));
        }
        if !is_catch_shorthand && let Some(kw) = self.await_catch_keyword(block) {
            parts.push(d.line());
            parts.push(kw);
        }
        if let Some(catch_block) = &block.catch {
            let body = self.build_nodes_doc_multiline(&catch_block.nodes);
            parts.push(indent_body_soft(self, body));
        }
        parts.push(d.line());
        parts.push(d.text("{/await}"));
        d.concat(&parts)
    }

    /// Build the await tail for the **newline-authored** layout: section bodies via
    /// `build_await_section_body` (which reports trailing whitespace), with a `hardline`
    /// before each keyword and before `{/await}` only when the preceding section had
    /// trailing whitespace. Mirrors `build_await_tail`, but respects authored trailing
    /// whitespace instead of a uniform `multiline` flag (the head is prepended by the
    /// caller).
    fn build_await_tail_newline(&self, block: &internal::AwaitBlock) -> DocId {
        let d = self.d();
        let (is_then_shorthand, is_catch_shorthand) = Self::await_shorthand_flags(block);
        let mut parts: DocBuf = DocBuf::new();
        let mut prev_has_trailing = false;
        if let Some(pending) = &block.pending {
            let (body, has_trailing) = build_await_section_body(self, pending);
            parts.push(body);
            prev_has_trailing = has_trailing;
        }
        if !is_then_shorthand && let Some(kw) = self.await_then_keyword(block) {
            if prev_has_trailing {
                parts.push(d.hardline());
            }
            parts.push(kw);
        }
        if let Some(then_block) = &block.then {
            let (body, has_trailing) = build_await_section_body(self, then_block);
            parts.push(body);
            prev_has_trailing = has_trailing;
        }
        if !is_catch_shorthand && let Some(kw) = self.await_catch_keyword(block) {
            if prev_has_trailing {
                parts.push(d.hardline());
            }
            parts.push(kw);
        }
        if let Some(catch_block) = &block.catch {
            let (body, has_trailing) = build_await_section_body(self, catch_block);
            parts.push(body);
            prev_has_trailing = has_trailing;
        }
        if prev_has_trailing {
            parts.push(d.hardline());
        }
        parts.push(d.text("{/await}"));
        d.concat(&parts)
    }

    /// Build a doc for an await block (no preceding context / sibling `>`).
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_await_block_doc(&self, block: &internal::AwaitBlock) -> DocId {
        self.build_await_block_doc_with_full_context(block, false, false, None)
    }

    /// Build await block doc with full context (multiline + preceding content).
    ///
    /// `gt_prefix`: a preceding inline-element sibling's split-off closing `>` to fold into
    /// the block's inline-vs-multiline decision (axis-3 sibling-`>` dangle). Only passed for
    /// the expanding path (see `build_block_node_doc_with_gt`).
    pub(super) fn build_await_block_doc_with_full_context(
        &self,
        block: &internal::AwaitBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        // Build expression doc with context-dependent behavior
        let allow_wrapping = !has_preceding_breakable;
        // When a `then`/`catch` shorthand carries its binding pattern in the head, bound
        // the awaited expression's trailing-comment range at the pattern start (mirroring
        // `{#each}`'s `context.span().start`) so a comment *inside* the pattern isn't
        // relocated out to trail the expression (`{#await p /* c */ then …}`); the
        // comment-aware `build_pattern_doc` preserves it in place instead. The full form
        // carries its patterns in `{:then}`/`{:catch}` outside the head, so it keeps the
        // head end.
        let head_end = block.opening_tag_span.end - 1;
        let expr_comment_end = match await_shorthand(block) {
            AwaitShorthand::Then => block.value.as_ref().map_or(head_end, |v| v.span().start),
            AwaitShorthand::Catch => block.error.as_ref().map_or(head_end, |e| e.span().start),
            AwaitShorthand::None => head_end,
        };
        let expr_doc = self.build_block_head_expr(
            AWAIT_BLOCK_OPEN,
            block.opening_tag_span,
            &block.expression,
            expr_comment_end,
            allow_wrapping || in_multiline_context,
        );

        let can_wrap = self.block_head_can_wrap(allow_wrapping, in_multiline_context);

        // Fast path: every present section is inline-authored → body-expand like the
        // other blocks. The head carries the `then v` / `catch e` clause; the section
        // bodies + `{:then}`/`{:catch}` keywords + `{/await}` all drop to their own
        // lines when the head wraps, chosen in one pass by `build_expanding_construct`.
        let sections = [&block.pending, &block.then, &block.catch];
        let has_section = sections
            .iter()
            .any(|f| f.as_ref().is_some_and(|f| !f.nodes.is_empty()));
        // `fragment_inline_authored` (via `fragment_ws_status`) already treats a
        // space-only section as non-inline, so space-only await blocks fall through to
        // the `has_space_only` group path below.
        let all_sections_inline = sections
            .iter()
            .filter_map(|f| f.as_ref())
            .all(|f| self.fragment_inline_authored(f, in_multiline_context));
        // Uniform body-drop: when every present section is inline-authored, the body +
        // `{:then}`/`{:catch}` keywords + `{/await}` drop to their own lines on overflow.
        // Keyed on `can_wrap` — the same gate `{#if}`/`{#each}` use — so the body hugs in the
        // inline-content/hug-both path (`can_wrap` false) but drops in the multiline-fragment
        // path. A block-parent sibling routes await through the multiline path via
        // `has_control_flow_after_sibling` (so `can_wrap` is true there); an inline parent
        // keeps `can_wrap` false and hugs, matching `{#if}`/`{#each}`.
        // Shorthand clause lives in the head: `then v` / bare `then`, or `catch e` / bare
        // `catch`; the full form has none. Built once, shared by the fast path, the
        // space-only tail, and the newline-authored tail. Classified by `await_shorthand`,
        // the same source `await_shorthand_flags` uses to skip the head-carried keyword.
        let clause = match await_shorthand(block) {
            AwaitShorthand::Then => Some(match &block.value {
                Some(value) => d.concat(&[d.text("then "), self.build_pattern_doc(value)]),
                None => d.text("then"),
            }),
            AwaitShorthand::Catch => Some(match &block.error {
                Some(error) => d.concat(&[d.text("catch "), self.build_pattern_doc(error)]),
                None => d.text("catch"),
            }),
            AwaitShorthand::None => None,
        };
        // `comment_end` is bound at `expr_comment_end` (not the head end) so a line
        // comment *inside* a shorthand pattern isn't mistaken for a trailing line comment
        // on the awaited expression — that would drop the space before the `then`/`catch`
        // clause.
        let head_doc = self.build_block_head(
            AWAIT_BLOCK_OPEN,
            &block.expression,
            expr_doc,
            clause,
            expr_comment_end,
            can_wrap,
        );

        // Fast path: every present section is inline-authored → body-expand like the other
        // blocks. The section bodies + `{:then}`/`{:catch}` keywords + `{/await}` all drop to
        // their own lines when the head wraps, chosen in one pass by `build_expanding_construct`.
        if can_wrap && has_section && all_sections_inline {
            let inline_tail = self.build_await_tail(block, false);
            let multiline_tail = self.build_await_tail(block, true);
            return self.build_expanding_construct(
                head_doc,
                inline_tail,
                multiline_tail,
                gt_prefix,
            );
        }

        // Space-only sections break together as one unit under a single `group` (the tail
        // uses `line()` separators); newline-authored sections key hardlines on authored
        // trailing whitespace. Both reuse the hoisted head + the shared tail builders.
        let has_space_only = [&block.pending, &block.then, &block.catch].iter().any(|f| {
            f.as_ref()
                .is_some_and(|f| self.fragment_has_space_only_ws(f))
        });
        if has_space_only {
            let tail = self.build_await_tail_space_only(block);
            return d.group(d.concat(&[head_doc, tail]));
        }

        let tail = self.build_await_tail_newline(block);
        d.concat(&[head_doc, tail])
    }

    /// Build a doc for a key block (no preceding context / sibling `>`).
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_key_block_doc(&self, block: &internal::KeyBlock) -> DocId {
        self.build_key_block_doc_with_full_context(block, false, false, None)
    }

    /// Build key block doc with full context (multiline + preceding content).
    ///
    /// `gt_prefix`: see `build_if_block_doc_with_full_context`.
    pub(super) fn build_key_block_doc_with_full_context(
        &self,
        block: &internal::KeyBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        // Build expression doc with context-dependent behavior
        let allow_wrapping = !has_preceding_breakable;
        let expr_doc = self.build_block_head_expr(
            KEY_BLOCK_OPEN,
            block.opening_tag_span,
            &block.expression,
            block.opening_tag_span.end - 1,
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

        let can_wrap = self.block_head_can_wrap(allow_wrapping, in_multiline_context);
        let head_doc = self.build_block_head(
            KEY_BLOCK_OPEN,
            &block.expression,
            expr_doc,
            None,
            block.opening_tag_span.end - 1,
            can_wrap,
        );
        let close = d.text("{/key}");
        if is_inline && can_wrap {
            // Inline-authored body: expand it + `{/key}` when the head wraps.
            return self.build_expanding_block(head_doc, body_doc, close, gt_prefix);
        }

        let mut parts: DocBuf = smallvec![head_doc, indented_body];

        // Add endline before {/key} only if trailing whitespace exists
        if has_trailing {
            parts.push(d.hardline());
        }

        parts.push(close);
        d.concat(&parts)
    }

    /// Build a doc for a snippet block (no preceding context / sibling `>`).
    pub(crate) fn build_snippet_block_doc(&self, block: &internal::SnippetBlock) -> DocId {
        self.build_snippet_block_doc_with_full_context(block, false, false, None)
    }

    /// Build a doc for a snippet block with full context (multiline + preceding content +
    /// optional sibling `>` fold).
    ///
    /// Uses same inline/multiline pattern as if blocks. Opening tag uses group() for
    /// parameter wrapping when they exceed print width. The body-drop keys on `can_wrap`
    /// (like `{#if}`/`{#each}`/`{#await}`): it hugs in the inline-content/hug-both path
    /// (`can_wrap` false) and drops in the multiline-fragment path.
    pub(crate) fn build_snippet_block_doc_with_full_context(
        &self,
        block: &internal::SnippetBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        let allow_wrapping = !has_preceding_breakable;
        let can_wrap = self.block_head_can_wrap(allow_wrapping, in_multiline_context);
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

        // Type parameters (generics). Parsed nodes route through tsv_ts's type-parameter
        // printer (constraints, defaults, modifiers, interior comments, width-based
        // wrapping of a long generic list — its own group, so it breaks independently of
        // the parameter list). The raw-text fallback (parse failure) emits the source
        // verbatim between `<` and `>`.
        let type_params_part = if let Some(decl) = &block.type_parameters {
            tsv_ts::build_type_parameters_doc_with_comments(d, decl, &self.ts_inputs(), &self.embed)
        } else if let Some(raw) = &block.type_params_raw {
            d.concat(&[d.text("<"), d.text_owned(raw.clone()), d.text(">")])
        } else {
            d.empty()
        };

        // Parameter list `(…)`. The parens fold so that when they wrap, `)` dedents to
        // base and `}` hugs it (`)}`) — no dangle (no trailing comma; trailingComma:
        // 'none').
        let params_inner = if let Some(raw) = &block.raw_parameters {
            // Parse-failure fallback: emit the raw parameter source (comments preserved
            // verbatim), split at top-level commas so a long list still wraps one-per-line.
            let params_docs: DocBuf = split_raw_params_at_commas(raw)
                .iter()
                .map(|s| d.text_owned(s.to_string()))
                .collect();
            let mut parts: DocBuf = DocBuf::with_capacity(params_docs.len() * 2);
            for (i, param_doc) in params_docs.into_iter().enumerate() {
                if i > 0 {
                    parts.push(d.text(","));
                    parts.push(d.line());
                }
                parts.push(param_doc);
            }
            d.concat(&[
                d.text("("),
                d.indent_softline(d.concat(&parts)),
                d.softline(),
                d.text(")"),
            ])
        } else {
            // Parsed parameters route through the same comment-aware,
            // `FunctionParameter`-context printer a real function signature uses, so
            // interior comments (`{ a = /* c */ 1 }`), boundary comments (`a /* c */, b`),
            // the single-pattern hug, and nesting-depth expansion all match a standalone
            // parameter list. `build_function_params_doc_with_comments` emits the `(…)`
            // with no group of its own — the `group` below drives the wrap.
            match block.params_paren {
                Some(paren) => tsv_ts::build_function_params_doc_with_comments(
                    d,
                    &block.parameters,
                    Some(paren.start),
                    Some(paren.end),
                    &self.ts_inputs(),
                    &self.embed,
                ),
                None => d.text("()"),
            }
        };
        // The parameter list gets its OWN group so it breaks independently of the
        // type-parameter group (mirroring a real function signature, where `<…>` and
        // `(…)` are sibling groups): a long generic list can wrap while short params stay
        // inline on the closing `>(…)}` line, and vice-versa. The outer `BlockHead` group
        // still governs the head as a whole.
        let params_doc = d.group(params_inner);

        // Opening tag `{#snippet name<T>(params)}`. Key the group to `BlockHead` so the
        // body can expand when the params wrap (below).
        //   When fits: {#snippet name(a, b, c)}
        //   When wraps: {#snippet name(\n\ta,\n\tb,\n\tc\n)}
        let opening_doc = d.group_with_id(
            d.concat(&[
                d.text("{#snippet "),
                d.text_owned(name.to_string()),
                type_params_part,
                params_doc,
                d.text("}"),
            ]),
            GroupId::BlockHead,
        );

        // Body: inline hugs directly, multiline uses hardlines
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.body)
        } else {
            self.build_nodes_doc_multiline(&block.body.nodes)
        };

        // Inline-authored body: expand the body + `{/snippet}` onto their own lines
        // when the construct overflows (params wrap, or head + body exceeds width) —
        // uniformly, including paramless snippets. Keyed to the opening group above.
        if is_inline && can_wrap {
            let close = d.text("{/snippet}");
            return self.build_expanding_block(opening_doc, body_doc, close, gt_prefix);
        }

        let mut parts: DocBuf = smallvec![opening_doc];
        parts.push(indent_body(self, body_doc, has_leading));

        // Add endline before {/snippet} only if trailing whitespace exists
        if has_trailing {
            parts.push(d.hardline());
        }

        parts.push(d.text("{/snippet}"));
        d.concat(&parts)
    }
}
