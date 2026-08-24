//! CSS declaration printing and wrapping logic
//!
//! Handles:
//! - Declaration printing (property: value;)
//! - Multiline wrapping decisions
//! - Width-based wrapping for long lists
//! - Doc building for width calculations

use super::boundary_ws::trim_regenerated_separator;
use super::{Printer, value_normalization};
use crate::ast::internal::{self, CssValue};
use tsv_lang::Span;
use tsv_lang::doc::{DocBuf, DocContext, arena::DocId};
use tsv_lang::printing::{format_string_literal, visual_width};
use tsv_lang::{PRINT_WIDTH, TAB_WIDTH};

/// How a declaration's broken comma list lays out, decided once.
///
/// Its existence *is* the break decision: `multiline_plan` returns `Some` exactly when
/// `comma_list_should_break` does. The routing question and the answers the printing needs
/// are one computation, so they cannot disagree — a split between them would reintroduce
/// the glued-vs-spaced 2-cycle — and a comment-bearing declaration lexes its value once
/// rather than at both sites.
struct MultilinePlan<'v> {
    /// The leading comment run to emit on the colon's line — postcss `raws.between`
    /// material (`leading_value_comment_run`).
    hoisted: Option<Span>,
    /// Element 0's own content members once the hoisted run is cut away. `None` when
    /// nothing was hoisted, and then element 0 prints whole.
    first_members: Option<&'v [CssValue<'v>]>,
}

/// Is this comma-list element the **empty** one — the nothing between two top-level
/// commas (`transition: a,,b`, `,a`)?
///
/// The value parser emits it as the zero-width empty-identifier sentinel (see
/// `CssValue::Identifier`), which prints as the nothing it is. It is a real list entry —
/// CSS Syntax 3's comma-list algorithm produces a group for it — so it is never dropped;
/// what it *does* need is a layout rule of its own, since every other break trigger is
/// phrased in terms of an element holding more than one node and this one holds none.
pub(super) fn is_empty_element(value: &CssValue<'_>) -> bool {
    matches!(value, CssValue::Identifier { span } if span.start >= span.end)
}

/// Did the author write a **closing** comma — a separator with nothing after it — at the
/// end of this comma list?
///
/// `values` are the list's elements and `list_end` the byte offset the list runs to (a
/// declaration value's span end, a function's `)`), so the answer is whatever text sits
/// between the last element and that bound: a comma there terminated no element, and one
/// must be written back. Every case falls out of the one question:
///
/// - `a,` → elements `[a]`, tail `,` — the source's last comma produced no element (CSS
///   Syntax 3 §"parse a comma-separated list of component values" stops once the input is
///   empty), so joining the elements would drop it.
/// - `a,,` → elements `[a, ""]`, tail `,` — the trailing **empty** element is real, and
///   joining N elements writes only N-1 commas, so its spelling still needs one more.
/// - `a, b` → tail empty. `a, b\,` → tail empty too: an escaped comma is content, so it
///   lives *inside* the last element's span and closes nothing.
///
/// Asking the source rather than the elements is what makes those one rule instead of
/// two. Reading it off the elements can only see the empty-element case (`values.last()`
/// empty), which left the plain trailing comma deleted — the parse is unchanged, but that
/// is not the question a declaration asks. css-values-4 §"Component value combinators"
/// requires a comma to be omitted when "all items following the comma have been omitted",
/// so `transition: a,` is invalid where `transition: a` is valid and the deletion turns a
/// dead declaration live; a **custom property** has no grammar to invalidate at all, and
/// its verbatim token sequence (css-variables-1) simply loses a token. Prettier deletes it
/// in every position — see `css/values/lists/comma_closing_prettier_divergence`.
///
/// The `<media-query-list>` is the one construct where the deletion *is* correct, because
/// mediaqueries-4 §Syntax delegates to the split itself; that path has its own reader
/// (`atrules::media_query_list`) and never comes here.
pub(super) fn list_has_closing_comma(
    source: &str,
    values: &[CssValue<'_>],
    list_end: usize,
) -> bool {
    let Some(last) = values.last() else {
        return false;
    };
    source
        .get(last.span().end_usize()..list_end)
        .is_some_and(|tail| crate::escapes::trim_end_preserving_escape(tail).ends_with(','))
}

impl<'a> Printer<'a> {
    /// Write the declaration ending: optional `!important` tail and the semicolon with newline.
    ///
    /// The value span ends before the `!important` region, so that region — and any
    /// comments around it (`blue /* a */ !important /* b */;`) — is invisible to the
    /// value printers. Re-emit it from source here with comments preserved in place
    /// (like prettier) and `!`/`important` normalized to a single ` !important`.
    /// Build the declaration's trailing text after the value: the `!important`
    /// keyword (normalized to ` !important`) plus any comments trailing it, or
    /// empty when the declaration isn't important.
    ///
    /// Single source of truth for the tail, shared by the emit
    /// (`write_declaration_end`) and the function paths' inline-vs-wrap width
    /// decisions — so the wrap check counts exactly the bytes the emit appends.
    /// A value carrying `!important` therefore reserves the tail and wraps when it
    /// would overrun the print width, matching prettier (the old measure pass
    /// omitted the tail and overran by its width).
    fn declaration_tail(&self, decl: &internal::CssDeclaration<'_>) -> String {
        if !decl.is_important() {
            return String::new();
        }
        let bytes = self.source.as_bytes();
        let mut i = decl.span.end_usize();
        let mut out = String::new();
        while i < bytes.len() {
            match bytes[i] {
                b';' | b'}' => break,
                b'/' if crate::comments::is_comment_start(bytes, i) => {
                    let end = crate::comments::comment_end(bytes, i);
                    out.push(' ');
                    out.push_str(&self.source[i..end]);
                    i = end;
                }
                b'!' => {
                    out.push_str(" !important");
                    i += 1;
                }
                c if c.is_ascii_alphabetic() => {
                    // the `important` keyword itself — already emitted at the `!`
                    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                }
                _ => i += 1,
            }
        }
        out
    }

    fn write_declaration_end(&mut self, decl: &internal::CssDeclaration<'_>) {
        let tail = self.declaration_tail(decl);
        self.write(&tail);
        self.write(";\n");
    }

    /// Emit a format-ignored declaration verbatim from source. The value span excludes
    /// the trailing `!important` region (and any comments inside it) plus the `;`, so
    /// `write_declaration_end` re-emits both — preserving a comment *after* the bang
    /// (`blue !important /* y */;`) that a hand-rolled synthetic ` !important` would drop.
    /// Shared by the rule and at-rule block loops.
    pub(super) fn write_format_ignore_declaration(&mut self, decl: &internal::CssDeclaration<'_>) {
        self.write_indent();
        self.write_verbatim_span(decl.span);
        self.write_declaration_end(decl);
    }

    /// Check if any value in a list requires one-per-line formatting
    ///
    /// This matches Prettier's `shouldBreakList` which checks:
    /// `node.groups.some((node) => node.type === "value-comma_group")`
    ///
    /// Returns true if any value is a space-separated list (box-shadow, text-shadow, etc.)
    /// or an **empty** element (`transition: a,,b`) — postcss-value-parser materializes the
    /// nothing between two commas as a group too, so prettier's predicate sees it and
    /// breaks; putting each element on its own line is also the layout that makes an empty
    /// one visible rather than hiding it in a run of commas.
    /// Functions are NOT checked here - they use doc-based wrapping with group/softline.
    fn any_value_needs_own_line(&self, values: &[CssValue<'_>]) -> bool {
        values
            .iter()
            .any(|v| matches!(v, CssValue::List { .. }) || is_empty_element(v))
    }

    /// This declaration's broken-comma-list layout, or `None` when the list does not break
    /// and takes one of the inline paths instead.
    ///
    /// The break is purely structural — prettier's `shouldBreakList`. tsv formerly also
    /// broke a list whose *source* put a newline after the colon, which made the output
    /// authoring-dependent on a shape nothing else in CSS honors (`margin:⏎1px 2px`,
    /// `color:⏎red` and `translate(⏎1px,⏎2px)` all collapse), and which no fixture relied
    /// on. Pinned by `comma_separated`'s `unformatted_authored_break` variant.
    fn multiline_plan<'v>(
        &self,
        decl: &'v internal::CssDeclaration<'v>,
    ) -> Option<MultilinePlan<'v>> {
        // Only comma-separated values with multiple items.
        let values = match &decl.value {
            CssValue::CommaSeparated { values, .. } if values.len() > 1 => *values,
            _ => return None,
        };

        // Custom properties skip structure-based multiline (one-per-line for List values).
        // They take the self-deciding width path via `print_decl_value_list` instead.
        // See fixture: declaration_long_multiline (continuation indent for space-separated items)
        if decl.property.starts_with("--") {
            return None;
        }

        let hoist = self.hoistable_leading_run(decl, values);
        // Element 0's node count starts where its own content does, past any hoisted run.
        let content_start = hoist.map(|(_, members)| members[0].span().start);
        if !self.comma_list_should_break(decl, values, content_start) {
            return None;
        }

        Some(MultilinePlan {
            hoisted: hoist.map(|(run, _)| run),
            first_members: hoist.map(|(_, members)| members),
        })
    }

    /// Whether this declaration's comma list breaks one element per line — prettier's
    /// `shouldBreakList`: some element holds more than one node.
    ///
    /// The single predicate `multiline_plan` derives the whole layout from — the break, the
    /// hoist, and the members element 0 emits — so they cannot disagree.
    ///
    /// A comment-free value answers from the AST alone: an element holds more than one
    /// node exactly when the value parser built it as a space-separated `List`, so the
    /// common path keeps its O(1) shape and pays nothing for the comment rules. Only a
    /// declaration that actually carries a `/* … */` (the O(1) `has_block_comment` gate)
    /// pays the per-element lex, and there a comment counts as a node — see
    /// `value_node_count` for why that is what makes the glued and spaced authorings
    /// converge. Element 0's own leading comment run is excluded via `content_start`: it
    /// is postcss `raws.between` material, not value content.
    fn comma_list_should_break(
        &self,
        decl: &internal::CssDeclaration<'_>,
        values: &[CssValue<'_>],
        content_start: Option<u32>,
    ) -> bool {
        if !decl.has_block_comment {
            return self.any_value_needs_own_line(values);
        }
        values.iter().enumerate().any(|(i, value)| {
            let from = if i == 0 { content_start } else { None };
            // An empty element holds *zero* nodes, so the count alone would miss it —
            // it is the one element kind whose break reason is its emptiness.
            is_empty_element(value) || self.value_node_count(value.span(), from) > 1
        })
    }

    /// The leading comment run `print_decl_multiline` hoists onto the colon's line, paired
    /// with element 0's own content members — the ones that print beneath it. `None` when
    /// there is nothing to hoist.
    ///
    /// Two things must hold, and both are about **element 0**. It has to be a
    /// space-separated `List` whose members the run can be cut from at a boundary
    /// (`content_members`), and content has to remain on the other side of that cut. A
    /// comment-only element 0 (`font-family: /* c */, b`) fails the second: hoisting would
    /// strand a bare `,` on its own line, which is what prettier does there and tsv
    /// declines — see the `comma_comment_only_element_prettier_divergence` fixture.
    ///
    /// Gated on the O(1) `has_block_comment`, so a comment-free comma list — the common
    /// case — never pays the lex.
    fn hoistable_leading_run<'v>(
        &self,
        decl: &internal::CssDeclaration<'_>,
        values: &'v [CssValue<'v>],
    ) -> Option<(Span, &'v [CssValue<'v>])> {
        if !decl.has_block_comment {
            return None;
        }
        let (run, content_start) = self.leading_value_comment_run(decl.value.span())?;
        let CssValue::List {
            values: members, ..
        } = values.first()?
        else {
            return None;
        };
        Some((run, Self::content_members(members, content_start)?))
    }

    /// Element 0's content members — those at or after `content_start` — or `None` when
    /// `content_start` does not land on a member boundary.
    ///
    /// That boundary is the hoist's precondition, and checking it here is what keeps the
    /// hoist lossless. A member that *contains* `content_start` holds the comment and the
    /// token after it fused into one: there is nothing to cut there, dropping the member
    /// whole would drop its content, and keeping it would print the comment twice. The
    /// value parser splits a leading comment run into its own member precisely so the
    /// boundary exists (`split_top_level`'s `comment_is_element`); this returns `None` for
    /// anything that still arrives fused, and the caller then declines to hoist rather
    /// than losing either the content or the print-once property.
    ///
    /// `None` also covers an all-comment element (no member is at or after
    /// `content_start`), which is the comment-only element 0 case above.
    fn content_members<'v>(
        members: &'v [CssValue<'v>],
        content_start: u32,
    ) -> Option<&'v [CssValue<'v>]> {
        let kept = members
            .iter()
            .position(|member| member.span().start >= content_start)?;
        (members[kept].span().start == content_start).then_some(&members[kept..])
    }

    /// Print a declaration whose comment-free value is a comma- or space-separated
    /// list, as one self-deciding doc: the renderer's own fit check chooses inline
    /// vs. wrapped, so the wrap decision and the emission are a single representation
    /// and cannot drift (the doc-first shape the at-rule prelude and the function /
    /// multiline-continuation paths already use). Replaces the former measure-then-emit
    /// pair (a discarded flat-join measured to decide, then a *different* fill emitted).
    ///
    /// The two list kinds wrap differently, so each builds its own shape:
    /// - **comma** breaks *after* the colon — flat `prop: a, b`, broken
    ///   `prop:\n\ta,\n\tb` — via `group(indent([line, comma_fill]))`. The group's
    ///   flat-fit reserves the `;` through `comma_fill`'s own `trailing_reserve`, while
    ///   the `line` covers the colon-space, so the boundary matches the old width check
    ///   (`indent + property + ": " + ";" + join <= PRINT_WIDTH`) exactly.
    /// - **space** keeps `: ` literal and wraps only the tail — flat `prop: a b c`,
    ///   broken `prop: a b\n\tc` — via `indent(space_fill)`. A `space_fill` that fits
    ///   renders byte-identical to the old flat join; its last-item `trailing_reserve`
    ///   reproduces the old measure pass's wrap decision (and never leaks into the
    ///   nested `var`/`calc`/`color-mix` groups, which render in the fill's forced flat
    ///   mode — the boundary the suffix mechanism would have broken wrongly).
    ///
    /// Value comments aren't in the CSS AST, so a comment-bearing list isn't routed
    /// here (the dispatch guard); it stays on the source-extracting comment path.
    fn print_decl_value_list(&mut self, decl: &internal::CssDeclaration<'_>) {
        let doc = match &decl.value {
            CssValue::CommaSeparated { values, span } => {
                let fill = self.build_comma_fill_doc(values, span.end_usize());
                let d = self.d();
                let body = d.group(d.indent(d.concat(&[d.line(), fill])));
                d.concat(&[d.text(":"), body])
            }
            CssValue::List { values, .. } => {
                let fill = self.build_space_fill_doc(values);
                let d = self.d();
                d.concat(&[d.text(": "), d.indent(fill)])
            }
            // The dispatch in `print_css_declaration` only routes comma/space lists here;
            // fall back to the plain `: value` form rather than panicking, matching the
            // crate's other defensive value guards.
            _ => {
                let value_doc = self.build_css_value_doc(&decl.value);
                let d = self.d();
                d.concat(&[d.text(": "), value_doc])
            }
        };
        self.write_arena_doc(doc);
        self.write_declaration_end(decl);
    }

    /// Format a CSS declaration (property: value;)
    pub(super) fn print_css_declaration(&mut self, decl: &internal::CssDeclaration<'_>) {
        self.write_indent();

        // The block-child juncture's boundary run (`a {<NBSP>color: red }`,
        // `color: red;<NBSP>top: 0`), which the parser skipped and this rebuilt head would
        // otherwise drop — the declaration spelling of the claim a rule child gets from its
        // selector and an at-rule from its `@`. Flush against the property, past the indent
        // this printer regenerates; the backward scan settles on the first non-whitespace
        // byte, which is the `{` or the previous declaration's `;`.
        self.write_head_boundary_ws(decl.span.start);

        // Extract property name from source to preserve escape sequences, then
        // lowercase it (property names are ASCII case-insensitive; prettier
        // lowercases — custom properties and escaped/comment-bearing names are
        // preserved by `lowercase_property_name`).
        let decl_source = decl.span.extract(self.source);
        let property_normalized = value_normalization::lowercase_property_name(
            value_normalization::extract_property_name(
                decl_source,
                decl.colon_pos(),
                decl.has_block_comment,
            ),
        );
        self.write(&property_normalized);

        // The property→colon gap's own run (`color<NBSP>: red`). Not an
        // `allow_whitespace()` juncture — `parseCss` reads the property raw and `.trim()`s
        // it, and JS's trim takes every one of these code points, so both parsers agree the
        // name is `color` — but the byte is still the author's, and this head is rebuilt from
        // the trimmed name. Floored inside the declaration so the scan cannot leave it, and
        // ASCII-trimmed like the combinator's: the separator that follows is regenerated, so
        // keeping the author's space before the colon would print one this formatter never
        // writes.
        let kept = trim_regenerated_separator(
            self.preserved_boundary_ws(decl.span.start, decl.span.start + decl.colon_pos() as u32),
        );
        if !kept.is_empty() {
            self.write(kept);
        }

        // A property carrying a block comment (`color /* c */`) takes a space before
        // the colon in tsv's normalized form (`color /* c */ : value`; fixture
        // `css/tokens/comments/in_property_value_before_colon_prettier_divergence`).
        // Emit it here, once, so every value-kind dispatch path below agrees on the
        // separator. Deciding it per-path (only `print_decl_default` did) left the
        // comma/space-list/string/function/value-comment paths emitting `: ` while the
        // single-value path emitted ` : ` — a symmetric-position inconsistency that
        // became a non-idempotency when a leading-comma value (`,ed`) drops its comma
        // across passes and flips CommaSeparated→Identifier, oscillating the separator.
        // See `tests/css_property_comment_colon_idempotent.rs`.
        if self.property_colon_needs_leading_space(decl, &property_normalized) {
            self.write(" ");
        }

        // Dispatch to appropriate handler based on value type and formatting needs
        if self.is_grid_multirow_value(decl) {
            self.print_decl_grid_multirow(decl);
        } else if let Some(plan) = self.multiline_plan(decl) {
            self.print_decl_multiline(decl, plan);
        } else if matches!(
            &decl.value,
            CssValue::CommaSeparated { .. } | CssValue::List { .. }
        ) && !self.has_value_comments_in_decl(decl)
        {
            self.print_decl_value_list(decl);
        } else if let CssValue::Function { name, args, span } = &decl.value {
            self.print_decl_function(decl, decl_source, name, args, *span);
        } else if self.has_value_comments_in_decl(decl) {
            self.print_decl_with_comments(decl, decl_source);
        } else if matches!(&decl.value, CssValue::String { .. }) {
            self.print_decl_string(decl, decl_source);
        } else {
            self.print_decl_default(decl);
        }
    }

    /// Print declaration with multiline formatting, per the layout `multiline_plan`
    /// already decided.
    fn print_decl_multiline<'v>(
        &mut self,
        decl: &'v internal::CssDeclaration<'v>,
        plan: MultilinePlan<'v>,
    ) {
        self.write(":");
        // A leading comment run is `raws.between` material, so it stays on the colon's
        // line and the value breaks beneath it. Emitted through the same whitespace
        // normalizer every other comment-bearing value path uses, so a run of them is
        // joined single-spaced.
        if let Some(run) = plan.hoisted {
            self.write(" ");
            let text = value_normalization::normalize_css_whitespace(run.extract(self.source));
            self.write(&text);
        }
        self.write("\n");
        self.indent_level += 1;
        self.print_css_value_multiline(&decl.value, plan.first_members);
        self.indent_level -= 1;
        self.write_declaration_end(decl);
    }

    /// Print declaration with function value.
    ///
    /// A comment-free value renders through the shared `build_value_function_doc`
    /// group: the renderer's own fit check — with the trailing `;` reserved via
    /// `write_arena_doc_reserving` — decides flat-vs-wrapped, so the wrap decision
    /// and the emission are a single doc and cannot drift. A value with comments
    /// stays on the imperative source-extraction path, since CSS value comments
    /// aren't stored in the AST and so can't be expressed as a doc.
    fn print_decl_function(
        &mut self,
        decl: &internal::CssDeclaration<'_>,
        decl_source: &str,
        name: &str,
        args: &[CssValue<'_>],
        span: Span,
    ) {
        if self.has_value_comments_in_decl(decl) {
            self.print_decl_function_with_comments(decl, decl_source, name, args, span);
        } else {
            self.write(": ");
            let doc = self.build_value_function_doc(name, args, span);
            // Reserve the trailing `;` plus any ` !important` tail for the OUTERMOST
            // function group's fit decision (the property + `: ` + tail + `;` boundary).
            // Counting the tail makes an `!important` function wrap when the keyword
            // would push it past the print width, instead of overrunning — matching
            // prettier. The tail comes from `declaration_tail`, the same string the
            // emit appends, so measure and emit can't drift.
            let tail_width = visual_width(&self.declaration_tail(decl), TAB_WIDTH);
            self.write_arena_doc_reserving(doc, 1 + tail_width);
        }
        self.write_declaration_end(decl);
    }

    /// Render a value doc, reserving `reserve` columns of trailing punctuation
    /// (the declaration's `;`) for the **outermost** group's fit decision only.
    ///
    /// Unlike `write_arena_doc_with_suffix` — whose `EmbedContext::suffix_width`
    /// every group's fit check subtracts — this appends a measurement-only trailing
    /// node (it renders nothing) after the doc. The outermost group's flat line
    /// reaches that node and counts it, but a nested group (a nested `calc`, a paren
    /// group) is separated from it by the outermost group's softline break, so its
    /// lookahead stops there and it never reserves the column. That keeps prettier's
    /// exact-width-boundary layout for nested groups, which a global suffix would
    /// wrongly break (e.g. a 100-column nested paren group).
    fn write_arena_doc_reserving(&mut self, doc: DocId, reserve: usize) {
        let reserved = {
            let d = self.d();
            let marker = d.with_context(d.empty(), DocContext::reserving(reserve));
            d.concat(&[doc, marker])
        };
        self.write_arena_doc(reserved);
    }

    /// Print a function-valued declaration whose value contains comments.
    ///
    /// CSS value comments aren't stored in the AST, so the value is reconstructed
    /// from source text: a wrapped function splits its args from source (preserving
    /// the comments in place), an inline one re-emits the normalized value verbatim.
    fn print_decl_function_with_comments(
        &mut self,
        decl: &internal::CssDeclaration<'_>,
        decl_source: &str,
        name: &str,
        args: &[CssValue<'_>],
        span: Span,
    ) {
        // Width check uses the NORMALIZED source length (comments included), since the
        // comments aren't in the doc and the value must round-trip verbatim.
        let func_source = span.extract(self.source);
        let normalized = value_normalization::normalize_css_whitespace(func_source);
        // Visual width (not byte length) of `property: value !important;`. The multibyte
        // comment's byte inflation is excluded by `visual_width`; the ` !important` tail is
        // counted via `declaration_tail` (the same string the emit appends) so an important
        // value wraps rather than overrunning the print width. `: ` is 2 cols, `;` is 1.
        let inline_len = visual_width(decl.property, TAB_WIDTH)
            + 2
            + visual_width(&normalized, TAB_WIDTH)
            + visual_width(&self.declaration_tail(decl), TAB_WIDTH)
            + 1;
        let needs_wrap = self.indent_width() + inline_len > PRINT_WIDTH;

        self.write(": ");
        if needs_wrap {
            // Wrapped: func(\n\targ1,\n\targ2\n)
            self.write(name);
            self.write("(\n");
            self.indent_level += 1;
            self.print_function_args_from_source(span, args);
            self.indent_level -= 1;
            self.write("\n");
            self.write_indent();
            self.write(")");
        } else {
            let normalized =
                value_normalization::extract_value_with_comments(decl_source, decl.colon_pos());
            self.write(&normalized);
        }
    }

    /// Print declaration with comments in value (non-function)
    fn print_decl_with_comments(&mut self, decl: &internal::CssDeclaration<'_>, decl_source: &str) {
        self.write(": ");
        let normalized =
            value_normalization::extract_value_with_comments(decl_source, decl.colon_pos());
        self.write(&normalized);
        self.write_declaration_end(decl);
    }

    /// Print declaration with string value
    fn print_decl_string(&mut self, decl: &internal::CssDeclaration<'_>, decl_source: &str) {
        // The original quote is the first byte of the string value's span (recovered
        // from source, not stored).
        let quote = self.source.as_bytes()[decl.value.span().start_usize()] as char;
        self.write(": ");
        if let Some(formatted) =
            value_normalization::extract_string_value(decl_source, decl.colon_pos(), quote)
        {
            self.write(&formatted);
        } else {
            let formatted = format_string_literal("", quote);
            self.write(&formatted);
        }
        self.write_declaration_end(decl);
    }

    /// Whether the property→colon gap takes a space before the colon.
    ///
    /// tsv's normalized form puts one after a property that carries a block comment
    /// (`color /* c */ : value`) — the parser-recorded `has_block_comment` is false
    /// iff the declaration holds no `/* … */` anywhere, so a false value proves the
    /// property text has none and the substring scan is skipped. Consulted once in
    /// `print_css_declaration`, before the value-kind dispatch, so every path agrees;
    /// it is the single predicate that keeps the separator from oscillating across a
    /// value-kind flip (see `tests/css_property_comment_colon_idempotent.rs`).
    fn property_colon_needs_leading_space(
        &self,
        decl: &internal::CssDeclaration<'_>,
        property: &str,
    ) -> bool {
        decl.has_block_comment && property.contains("/*")
    }

    /// Print declaration with default formatting
    fn print_decl_default(&mut self, decl: &internal::CssDeclaration<'_>) {
        // The property→colon separator's leading space (for a comment-bearing property)
        // is emitted once in `print_css_declaration`; every path writes the bare `: `.
        self.write(": ");
        // Empty custom-property value carrying !important (`--a: !important;`): the `: `
        // separator already supplies the single space, so emit `!important` without the
        // extra leading space `write_declaration_end` adds — avoids `--a:  !important;`.
        if decl.is_important()
            && matches!(&decl.value, CssValue::Identifier { span } if span.extract(self.source).trim().is_empty())
        {
            self.write("!important;\n");
            return;
        }
        self.print_css_value(&decl.value);
        self.write_declaration_end(decl);
    }

    /// Check if this is a grid property with multiple row string values
    /// where consecutive values are on different source lines.
    ///
    /// Matches Prettier's source-position-dependent grid formatting
    /// (comma-separated-value-group.js lines 421-436): if consecutive values
    /// are on different source lines, wrap each to its own line.
    /// Properties: `grid-template-areas`, `grid-template*`, `grid`
    fn is_grid_multirow_value(&self, decl: &internal::CssDeclaration<'_>) -> bool {
        let prop = decl.property;
        let is_grid_prop = prop == "grid" || prop.starts_with("grid-template");
        if !is_grid_prop {
            return false;
        }
        let values = match &decl.value {
            CssValue::List { values, .. }
                if values.len() >= 2
                    && values.iter().all(|v| matches!(v, CssValue::String { .. })) =>
            {
                values
            }
            _ => return false,
        };
        // Check source positions: are consecutive values on different lines?
        let source_bytes = self.source.as_bytes();
        for pair in values.windows(2) {
            let end = pair[0].span().end_usize();
            let start = pair[1].span().start_usize();
            if end <= start && source_bytes[end..start].contains(&b'\n') {
                return true;
            }
        }
        false
    }

    /// Print grid property with multiple row strings, one per line
    ///
    /// Format: `property:\n\t'row1'\n\t'row2'\n\t'row3';`
    fn print_decl_grid_multirow(&mut self, decl: &internal::CssDeclaration<'_>) {
        self.write(":\n");
        if let CssValue::List { values, .. } = &decl.value {
            self.indent_level += 1;
            for (i, val) in values.iter().enumerate() {
                self.write_indent();
                self.print_css_value(val);
                if i < values.len() - 1 {
                    self.write("\n");
                }
            }
            self.indent_level -= 1;
        }
        self.write_declaration_end(decl);
    }

    /// Emit a broken comma list, one element per line.
    ///
    /// Reached only through `multiline_plan`, which returns `Some` exactly when
    /// `comma_list_should_break` does — so there is one arm, not a choice between
    /// one-per-line and greedy packing. A list that does *not* break is not routed here at
    /// all: it takes `print_decl_value_list`'s self-deciding doc (or, with comments,
    /// `print_decl_with_comments`), which does its own width-based wrapping.
    fn print_css_value_multiline<'v>(
        &mut self,
        value: &'v CssValue<'v>,
        first_members: Option<&'v [CssValue<'v>]>,
    ) {
        let CssValue::CommaSeparated { values, span } = value else {
            // Unreachable via `multiline_plan` (which matches on `CommaSeparated`); fall
            // back rather than panicking, like the crate's other defensive value guards.
            self.print_nested_value(value);
            return;
        };

        let closing_comma = list_has_closing_comma(self.source, values, span.end_usize());
        for (i, val) in values.iter().enumerate() {
            self.write_indent();
            // Every item reserves what its own line ends with, so a wrappable item breaks
            // that many columns early rather than letting the terminator push the line to
            // 101 (matching prettier and tsv's hard-print-width stance). That is the `,` a
            // non-final item takes, or the `;` a final one does — except when a closing
            // comma is coming, and the last line ends `,;`, two columns. A space-separated
            // List value self-wraps via `build_space_fill_value_doc`'s
            // `group(indent(fill))`; a non-List value (e.g. a gradient function) wraps via
            // its own value group.
            let reserve = if closing_comma && i == values.len() - 1 {
                2
            } else {
                1
            };
            let doc = if let CssValue::List {
                values: list_values,
                ..
            } = val
            {
                // Element 0 prints only its own content: the leading comment run
                // `print_decl_multiline` hoisted onto the colon's line is already
                // gone from `first_members`, so the run prints exactly once.
                let members = match first_members {
                    Some(members) if i == 0 => members,
                    _ => list_values,
                };
                self.build_space_fill_value_doc(members)
            } else {
                self.build_css_value_doc(val)
            };
            self.write_arena_doc_reserving(doc, reserve);
            if i < values.len() - 1 {
                self.write(",\n");
            }
        }
        if closing_comma {
            self.write(",");
        }
    }

    /// Build a fill doc for comma-separated values
    ///
    /// Creates a doc that packs values greedily:
    /// - In flat mode: `item1, item2, item3`
    /// - When broken: `item1, item2,\n  item3, item4,\n  item5`
    ///
    /// For space-separated items (CssValue::List), each item is wrapped as
    /// `group(indent(fill([sub1, line, sub2, ...])))` so fill can break within
    /// items with continuation indent. This matches prettier's
    /// `printCommaSeparatedValueGroup` which returns `group(indent(fill(parts)))`.
    ///
    /// `list_end` bounds the list in the source so an authored **closing** comma is spelled
    /// back (`list_has_closing_comma`). It rides on the last element's part rather than
    /// becoming a part of its own: `fill` reads the parts as alternating content and
    /// separators, so a bare `,` appended after the final content lands in separator
    /// position and could be left stranded on its own line when the fill breaks.
    fn build_comma_fill_doc(&self, values: &[CssValue<'_>], list_end: usize) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        for (i, val) in values.iter().enumerate() {
            if let CssValue::List {
                values: list_values,
                ..
            } = val
            {
                // Space-separated values: build as group(indent(fill([sub1, line, sub2])))
                // so fill can break within items with continuation indent
                let sub_parts = self.build_space_fill_parts(list_values);
                let sub_fill = d.fill(&sub_parts);
                let sub_indented = d.indent(sub_fill);
                parts.push(d.group(sub_indented));
            } else {
                parts.push(self.build_css_value_doc(val));
            }
            if i < values.len() - 1 {
                // Separator: ", " in flat mode, ",\n" when broken
                let comma = d.text(",");
                let line = d.line();
                parts.push(d.concat(&[comma, line]));
            }
        }
        if list_has_closing_comma(self.source, values, list_end)
            && let Some(last) = parts.pop()
        {
            parts.push(d.concat(&[last, d.text(",")]));
        }

        // Reserve 1 char for trailing semicolon to prevent fill from packing
        // to exactly printWidth and then exceeding when ';' is added
        let context = DocContext::reserving(1);
        let fill = d.fill(&parts);
        d.with_context(fill, context)
    }

    /// Build fill parts for space-separated values (shared helper)
    ///
    /// Returns `[val1, line, val2, line, val3]` — suitable for `d.fill()`.
    /// Used by both declaration wrapping and function arg wrapping.
    pub(super) fn build_space_fill_parts(&self, values: &[CssValue<'_>]) -> DocBuf {
        let d = self.d();
        let mut parts = DocBuf::with_capacity(values.len() * 2);
        for (i, val) in values.iter().enumerate() {
            parts.push(self.build_css_value_doc(val));
            if i < values.len() - 1 {
                parts.push(d.line());
            }
        }
        parts
    }

    /// Build a fill doc for space-separated values
    ///
    /// Creates a doc that packs values greedily:
    /// - In flat mode: `item1 item2 item3`
    /// - When broken: `item1 item2\n  item3 item4\n  item5`
    ///
    /// The whole list is the declaration's value, so what follows it is the `;` — one
    /// column, reserved so the fill breaks rather than letting the terminator push the
    /// line to 101.
    fn build_space_fill_doc(&self, values: &[CssValue<'_>]) -> DocId {
        let d = self.d();
        let parts = self.build_space_fill_parts(values);
        let context = DocContext::reserving(1);
        let fill = d.fill(&parts);
        d.with_context(fill, context)
    }

    /// Print function arguments from source, preserving comments
    ///
    /// Used when a function has comments in its arguments and needs wrapping.
    /// Extracts each argument from the source string to preserve comments.
    fn print_function_args_from_source(&mut self, span: Span, args: &[CssValue<'_>]) {
        // Extract function args content from the function's OWN span, or fall back to
        // semantic printing.
        let Some(args_content) = self.function_args_source(span) else {
            self.print_function_args_semantic(args);
            return;
        };

        // Split by top-level commas and print each normalized arg. The trailing trim is
        // what lets `has_closing_comma` see a closing comma the author padded (`…, )`) —
        // untrimmed, that space becomes a whitespace-only final part, which both hides the
        // comma and prints as a blank argument line. It is the escape-aware trim, so an
        // argument legitimately ending in an escaped space (`calc(1px\ )`) keeps it.
        let args_content = crate::escapes::trim_end_preserving_escape(args_content);
        let arg_strs = value_normalization::split_args_by_comma(args_content);
        for (i, arg_str) in arg_strs.iter().enumerate() {
            self.write_indent();
            let normalized = value_normalization::normalize_css_whitespace(arg_str);

            // Check if this arg has space-separated values that would exceed width
            // Split by top-level spaces (not inside parens) to get individual values
            let space_parts = value_normalization::split_by_space_preserving_parens(&normalized);
            if space_parts.len() > 1 && self.arg_string_exceeds_width(&normalized) {
                // Use fill wrapping with continuation indent
                self.print_space_separated_with_fill(&space_parts);
            } else {
                self.write(&normalized);
            }

            if i < arg_strs.len() - 1 {
                self.write(",\n");
            }
        }
        // A comma in final position produced no part, so it can only be written back here
        // — the same closing comma the doc paths spell via `list_has_closing_comma`. It
        // joins the last argument's line rather than taking one of its own.
        if value_normalization::has_closing_comma(args_content, &arg_strs) {
            self.write(",");
        }
    }

    /// The text between a function value's parentheses, sliced from the function's own
    /// span.
    ///
    /// The span is the exact bound: `extract_function_parts` accepts a value as a function
    /// only when the matching close paren is its **last byte**, and a function name cannot
    /// contain a `(`, so the first one in the span opens the arguments. Locating the
    /// function by *name* instead — searching the whole declaration text for `name(` — is
    /// what this replaces: the name is routinely also part of the property
    /// (`--linear-gradient: linear-gradient(…)`), and the search then measures from the
    /// wrong occurrence, which lands on the right paren only because nothing between them
    /// can be a `(` — until a comment in the property→colon gap is (`--linear-gradient
    /// /* ( */: linear-gradient(…)`), and the whole extraction fails to the semantic
    /// fallback, dropping every argument comment and the closing comma.
    ///
    /// `None` only if the span is out of bounds or holds no `(` — neither reachable for a
    /// parsed function, so the caller's fallback is defensive, like the crate's other
    /// value guards.
    fn function_args_source(&self, span: Span) -> Option<&'a str> {
        if span.end_usize() > self.source.len() {
            return None;
        }
        let raw = span.extract(self.source);
        let open = raw.as_bytes().iter().position(|&b| b == b'(')?;
        raw.get(open + 1..raw.len().checked_sub(1)?)
    }

    /// Check if an arg string would exceed width when printed at current position
    ///
    /// Width is the arg's **visual** width (`visual_width`), not its byte length —
    /// a multibyte arg (e.g. an accented comment) is narrower than `str::len()`
    /// reports, and byte math would wrongly route it to the wrapping path.
    fn arg_string_exceeds_width(&self, arg: &str) -> bool {
        self.indent_width() + visual_width(arg, TAB_WIDTH) > PRINT_WIDTH
    }

    /// Print space-separated values with fill wrapping
    ///
    /// Uses continuation indent for wrapped lines. When the first part is a comment
    /// that fills the line, the comment is printed separately at base indent, then
    /// the value parts use continuation indent.
    fn print_space_separated_with_fill(&mut self, parts: &[&str]) {
        if parts.len() < 2 {
            if let Some(part) = parts.first() {
                self.write(part);
            }
            return;
        }

        // Check if first part is a comment that fills the line. Widths are visual
        // (`visual_width`), not byte length: a multibyte first part (e.g. an accented
        // comment) is narrower than `str::len()`, and byte math would wrongly split it
        // onto its own line where it actually fits inline alongside the next value.
        let first_is_comment = parts[0].trim().starts_with("/*");
        let first_len = visual_width(parts[0], TAB_WIDTH);
        let second_len = visual_width(parts[1], TAB_WIDTH);
        let first_fills_line = self.indent_width() + first_len + 1 + second_len > PRINT_WIDTH;

        // When comment fills line: print it separately, then handle values with continuation
        let (value_parts, use_continuation) =
            if first_is_comment && first_fills_line && parts.len() > 2 {
                self.write(parts[0]);
                self.write("\n");
                self.write_indent();

                // Check if value parts need continuation indent (visual width)
                let val1_len = visual_width(parts[1], TAB_WIDTH);
                let val2_len = visual_width(parts[2], TAB_WIDTH);
                let needs_wrap = self.indent_width() + val1_len + 1 + val2_len > PRINT_WIDTH;
                (&parts[1..], needs_wrap)
            } else {
                // Normal case: check if first two items fit together
                let both_fit = self.indent_width() + first_len + 1 + second_len <= PRINT_WIDTH;
                (parts, both_fit)
            };

        // Build and write fill doc
        let fill_doc = self.build_fill_parts_from_strings(value_parts);
        if use_continuation {
            self.indent_level += 1;
        }
        self.write_arena_doc(fill_doc);
        if use_continuation {
            self.indent_level -= 1;
        }
    }

    /// Build fill doc parts from string slices
    fn build_fill_parts_from_strings(&self, parts: &[&str]) -> DocId {
        let d = self.d();
        let mut doc_parts = DocBuf::with_capacity(parts.len() * 2);
        for (i, part) in parts.iter().enumerate() {
            doc_parts.push(d.text_pooled(part));
            if i < parts.len() - 1 {
                doc_parts.push(d.line());
            }
        }
        d.fill(&doc_parts)
    }

    /// Print function arguments semantically — the defensive arm for when
    /// `function_args_source` can't slice the arguments out of the function's span.
    ///
    /// ⚠️ Lossy by construction, unlike the crate's other defensive guards: it prints from
    /// the AST, which holds neither the closing comma nor any argument comment that isn't
    /// glued into an argument's own span. That is tolerable only because it is
    /// **unreachable** for a parsed function — the span is in bounds and a function value
    /// always holds its `(`. It exists so an impossible state degrades instead of
    /// panicking; if a shape ever *does* reach it, the fix is to make the slice total, not
    /// to improve this.
    fn print_function_args_semantic(&mut self, args: &[CssValue<'_>]) {
        for (i, arg) in args.iter().enumerate() {
            self.write_indent();
            self.print_nested_value(arg);
            if i < args.len() - 1 {
                self.write(",\n");
            }
        }
    }
}
