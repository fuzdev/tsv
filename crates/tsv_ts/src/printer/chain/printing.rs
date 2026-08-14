// Chain node and group printing for TypeScript member chain formatting
//
// This module handles rendering chain nodes and groups to Docs:
// - print_node: Basic node printing (with optional expansion flag)
// - print_group: Group printing (with optional expansion and comment skipping)

use super::types::{ChainGroup, ChainNode, NonNullGap, is_numeric_index};
use crate::ast::internal::{self, Expression};
use crate::printer::{LeadingGlue, ParenContext, Printer, needs_parens};
use tsv_lang::doc::{
    DocBuf,
    arena::{DocArena, DocId},
};
use tsv_lang::printing::has_blank_line_between_strict;

//
// Node Printing
//

/// Print a single chain node
///
/// Parameters:
/// - `node`: The chain node to print
/// - `printer`: The printer implementation
/// - `expanded`: If true, use forced call expansion (hardlines)
/// - `skip_comments`: If true, skip comments for member nodes (used when comments
///   are handled separately by add_comments_and_break)
pub(crate) fn print_node_inner<'a>(
    node: &ChainNode<'a>,
    printer: &Printer<'_>,
    expanded: bool,
    skip_comments: bool,
) -> DocId {
    let d = printer.arena();
    match node {
        ChainNode::Base {
            expr,
            needs_parens,
            paren_comment_end,
            followed_by_non_null,
        } => {
            if *needs_parens {
                // Asked BEFORE any base doc is built — building one can re-mark the
                // target from a nested value position inside the ternary's own branches.
                let base_ternary = printer.chain_base_ternary(expr);
                // The plain expression doc, memoized lazily: every arm consumes it EXCEPT
                // a binary base's common path, which builds the chain-for-parens operand
                // doc instead — eager, a parenthesized binary base paid two full doc
                // builds for one rendered doc. The rare paren-comment branch still
                // consumes it (the line-comment layout's broken body), which is why it is
                // a memo rather than moved into the arms that use it.
                let mut inner_memo = None;
                let mut inner = || -> DocId {
                    *inner_memo.get_or_insert_with(|| printer.build_expression_doc(expr))
                };
                // The parens stay bare outside so the chain's conditionalGroup drives
                // breaking; `build_expanding_parens_body_doc` is the shape every base kind
                // below takes — await, binary, and everything else alike.
                let hang = |content: DocId| printer.build_expanding_parens_body_doc(content);
                // A ternary base is decided before the kind match: it can never be an
                // arrow / function / binary, and its three shapes read better as one
                // question than as a guard clause wedged among the kind arms.
                let inner_group = if let Some(ternary) = base_ternary.filter(|t| !t.expands) {
                    // Not one of prettier's `ancestorNameMap` value positions
                    // (`shouldExtraIndentForConditionalExpression`) — a call argument, an
                    // array element, a property value — so the `?`/`:` arms hang under the
                    // `(` as the bare ternary does. Only when the member is the ternary's
                    // DIRECT parent does the `)` additionally drop to its own line
                    // (prettier's `breakClosingParen`); a `!` in between is the parent
                    // instead, and there the `)` stays welded to the last arm.
                    let body = inner();
                    if ternary.direct && !*followed_by_non_null {
                        d.group(d.concat(&[body, d.softline()]))
                    } else {
                        body
                    }
                } else {
                    match expr {
                        Expression::ArrowFunctionExpression(_)
                        | Expression::FunctionExpression(_) => {
                            // IIFE / function callee or arrow member-object: the parens
                            // hug the function — its own body drives breaking, prettier
                            // never breaks after the `(` here. `(() => {...})().catch()`,
                            // `(function () {})().p`. Matches the bare-callee path
                            // (`call_formatting.rs`), which wraps with hugging parens.
                            inner()
                        }
                        Expression::BinaryExpression(binary) => {
                            // The chain-for-parens operand doc, so the whole operand chain is
                            // what stays flat. Every operator family — arithmetic, logical
                            // (`&&`/`||`), and nullish (`??`) — is laid out identically. A
                            // logical base used to skip this wrapper and break at its own
                            // operators instead, welding the closing `).member` onto the last
                            // operand — a third layout matching neither tsv's arithmetic shape
                            // nor prettier's. See conformance_prettier_ts.md §TypeScript
                            // (Parenthesized binary member base).
                            hang(printer.build_binary_chain_for_parens(binary))
                        }
                        // Every other base kind, and a ternary base that DOES expand.
                        _ => hang(inner()),
                    }
                };
                // Preserve a comment from the stripped grouping parens inside them,
                // before `)` (`(x + y /* c */)!.foo`) — prettier relocates it past
                // `)`; tsv keeps it where the author wrote it. The `!` itself is the
                // next node, so this doc closes at the `)`.
                if let Some(end) = *paren_comment_end
                    && let Some(doc) = printer.build_non_null_paren_operand_doc(
                        expr.span().end,
                        end,
                        inner_group,
                        inner(),
                        ")",
                    )
                {
                    return doc;
                }
                d.parens(inner_group)
            } else {
                printer.build_expression_doc(expr)
            }
        }

        ChainNode::Call { call, optional } => {
            if expanded {
                printer.print_call_args_expanded(call, *optional)
            } else {
                printer.print_call_args(call, *optional)
            }
        }

        ChainNode::Member {
            property,
            optional,
            object_end,
            property_start,
        } => print_member_access(
            printer,
            *property,
            MemberSpans {
                name_start: *property_start,
                object_end: *object_end,
                property_start: *property_start,
            },
            *optional,
            false,
            skip_comments,
        ),

        ChainNode::PrivateMember {
            property,
            optional,
            object_end,
            property_start,
            name_start,
        } => print_member_access(
            printer,
            *property,
            MemberSpans {
                name_start: *name_start,
                object_end: *object_end,
                property_start: *property_start,
            },
            *optional,
            true,
            skip_comments,
        ),

        ChainNode::ComputedMember {
            expr,
            optional,
            object_end,
            bracket_end,
        } => {
            // A computed member index is `[ Expression ]` — an assignment expression
            // there is parenthesized for clarity (`arr[(x = 0)]`, `obj?.[(x = 0)]`),
            // matching Prettier and the object/class computed-key paths. Inside a
            // for-header init, an `in` binary is wrapped too (`for (x = arr[(a in b)];
            // …)`) — the brackets make the bare `in` reachable here, unlike a chain
            // base/callee whose parens are structurally mandatory. A sequence
            // self-parenthesizes via `build_sequence_doc`.
            let raw_inner = printer.build_expression_doc(expr);
            let inner = if needs_parens(
                expr,
                ParenContext::ComputedPropertyKey,
                printer.in_for_init(),
            ) {
                d.parens(raw_inner)
            } else {
                raw_inner
            };

            let open = if *optional { "?.[" } else { "[" };
            // A numeric index keeps its brackets flat (prettier's `isNumericLiteral`
            // carve-out); every other index gets a breakable group.
            let breakable = !is_numeric_index(expr);

            // Chain-level zero-comment gate: a comment-free chain span has no comment in
            // or around these brackets, so emit the `[…]` / `?.[…]` directly — skipping
            // find_bracket_position (a source scan) and every pre/inside-bracket comment
            // classification. Those paths all collapse to the same bracket doc around
            // `inner` when the range is comment-free.
            if !printer.chain_has_comments() {
                return computed_lookup_doc(printer, open, inner, breakable);
            }

            let prop_span = printer.get_property_span(expr);

            // Find the opening bracket position by scanning from object_end,
            // skipping over comments to find the actual `[` (or `?.[` for optional)
            let bracket_open_pos = find_bracket_position(printer, *object_end, prop_span.start);

            let inside_start = bracket_open_pos + if *optional { 3 } else { 1 };

            // A line comment inside the brackets (before the index, or after it before
            // `]`) forces the whole bracket to break so the `//` can't swallow the index
            // or `]` — the block-only paths below would drop it (content loss). `[` is
            // at `inside_start - 1` (the char before the index region, past `?.` for the
            // optional form). Prettier relocates the comment before the expression instead.
            let bracket_doc = if let Some(broken) = printer
                .build_computed_member_line_comment_bracket(
                    open,
                    inside_start,
                    prop_span.start,
                    prop_span.end,
                    *bracket_end,
                    inner,
                ) {
                broken
            } else {
                // Block comments inside brackets: [/* c */ key] and [key /* c */]
                // No same-line filter because when the input is already broken across
                // lines, the comment may be on a different line from the bracket opening
                // (e.g., `?.[` on one line, `/** @type {string} */ d` on the next).
                let leading_comments_doc = printer.build_chain_block_comments_doc(
                    inside_start,
                    prop_span.start,
                    crate::printer::CommentSpacing::Trailing,
                    false,
                );
                let trailing_comments_doc = printer.build_chain_block_comments_doc(
                    prop_span.end,
                    *bracket_end,
                    crate::printer::CommentSpacing::Leading,
                    false,
                );

                let inner_with_comments =
                    d.concat(&[leading_comments_doc, inner, trailing_comments_doc]);

                // Block comments inside the brackets (e.g. `?.[/** @type {string} */ d]`)
                // must be able to break onto their own line, so they override the numeric
                // carve-out — a bare `[0]` stays flat, but `[/* c */ 0]` gets the group:
                //   obj.chain?.[
                //       /** @type {string} */ d
                //   ]
                let has_inside_comments = printer
                    .has_comments_on_page_between(inside_start, prop_span.start)
                    || printer.has_comments_on_page_between(prop_span.end, *bracket_end);
                computed_lookup_doc(
                    printer,
                    open,
                    inner_with_comments,
                    breakable || has_inside_comments,
                )
            };

            // The chain builder owns the gap between the object and the `[`
            // (`node_comment_gap`) and has already emitted its comments ahead of this
            // node's line break, so emitting them again here would duplicate them.
            if skip_comments {
                return bracket_doc;
            }

            // Comments between object and `[` stay OUTSIDE brackets
            // (comments between `[` and the index went INSIDE, above)
            let pre_bracket = printer.classify_comments(*object_end, bracket_open_pos);

            // A LINE comment in this gap must end its line. Reaching here means no chain
            // builder owned the gap (`skip_comments` would be set): a computed member with
            // a numeric-literal index is glued into the preceding call's group rather than
            // starting one (prettier's `printMemberChain` grouping), so it is the one node
            // kind that can hang mid-group. Emit through the shared forced-break path, the
            // single definition of how a chain gap renders — the same one the group and
            // member-only paths use. The `line_suffix` route below (correct for a block
            // comment, which can sit inline) would instead defer the `//` to end of line:
            // `a.b()[0]; // c`, relocating it past the brackets AND the `;`, and merging
            // consecutive ones onto one line where the first `//` swallows the rest.
            if pre_bracket.has_line_comments() {
                let mut parts = DocBuf::new();
                push_gap_comments_and_break(&mut parts, printer, *object_end, bracket_open_pos);
                parts.push(bracket_doc);
                return d.concat(&parts);
            }

            // Block comments before the bracket (e.g., `a /* c */[0]`) stay inline.
            let pre_bracket_block_doc = printer.build_chain_block_comments_doc(
                *object_end,
                bracket_open_pos,
                crate::printer::CommentSpacing::Leading,
                true,
            );

            d.concat(&[pre_bracket_block_doc, bracket_doc])
        }

        // A comment between the operand and the `!` (`aaa /* c */!.bbb`) is this node's
        // to print — nothing else scans the region, and the chain builder can't take it
        // the way it takes a member's gap, since a `!` may never start a line
        // (`[no LineTerminator here]`). Prettier keeps it here too, in the same slot the
        // non-chain arm of `build_ts_non_null_doc` already emits (`p?.q /* c */!`).
        // Block-only, and a stripped shell is where that bound is wrong — see
        // `NonNullGap::Bang`.
        ChainNode::NonNull {
            gap:
                NonNullGap::Bang {
                    operand_end,
                    bang_end,
                },
        } if printer.chain_has_comments() => {
            let comments = printer.build_chain_block_comments_doc(
                *operand_end,
                *bang_end,
                crate::printer::CommentSpacing::Leading,
                false,
            );
            d.concat(&[comments, d.text("!")])
        }

        ChainNode::NonNull { .. } => d.text("!"),
    }
}

/// Print a single chain node (normal mode)
pub(crate) fn print_node<'a>(node: &ChainNode<'a>, printer: &Printer<'_>) -> DocId {
    print_node_inner(node, printer, false, false)
}

//
// Group Printing
//

/// Print a single chain group
pub(crate) fn print_group<'a>(group: &ChainGroup<'a>, printer: &Printer<'_>) -> DocId {
    print_group_inner(group, printer, false, false)
}

/// Print a chain group with forced call expansion
pub(crate) fn print_group_expanded<'a>(group: &ChainGroup<'a>, printer: &Printer<'_>) -> DocId {
    print_group_inner(group, printer, true, false)
}

/// Print a chain group with standard forced call expansion (no arrow hugging)
///
/// Like `print_group_expanded`, but uses `(\n  args,\n)` instead of `(sig =>\n  body,\n)`
/// for single-arg arrows with breakable bodies. Used in short chain states where the
/// chain doesn't break between groups.
pub(crate) fn print_group_standard_expanded<'a>(
    group: &ChainGroup<'a>,
    printer: &Printer<'_>,
) -> DocId {
    let d = printer.arena();
    let docs: DocBuf = group
        .nodes
        .iter()
        .map(|n| match n {
            ChainNode::Call { call, optional } => {
                printer.print_call_args_standard_expanded(call, *optional)
            }
            _ => print_node_inner(n, printer, false, false),
        })
        .collect();
    d.concat(&docs)
}

/// Print a chain group, skipping block comments for the first member node
///
/// Used by ChainPartsBuilder in expanded path where `add_comments_and_break`
/// already handles comments for the first member (emitting before the line break).
pub(crate) fn print_group_skip_first_comments<'a>(
    group: &ChainGroup<'a>,
    printer: &Printer<'_>,
) -> DocId {
    print_group_inner(group, printer, false, true)
}

/// Print a chain group with forced call expansion, skipping block comments for the first member
pub(crate) fn print_group_expanded_skip_first_comments<'a>(
    group: &ChainGroup<'a>,
    printer: &Printer<'_>,
) -> DocId {
    print_group_inner(group, printer, true, true)
}

/// Internal implementation for printing a chain group
fn print_group_inner<'a>(
    group: &ChainGroup<'a>,
    printer: &Printer<'_>,
    expanded: bool,
    skip_first_comments: bool,
) -> DocId {
    let d = printer.arena();
    let docs: DocBuf = group
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            // Skip comments only for the first member node
            let skip_comments = skip_first_comments && i == 0 && n.is_member();
            print_node_inner(n, printer, expanded, skip_comments)
        })
        .collect();
    d.concat(&docs)
}

//
// Member Access Printing
//

/// Wrap a plain `.prop` lookup in its own break point — Prettier's
/// `printMemberExpression` (member.js): `group(indent([softline, lookup]))`.
///
/// The lookup hugs the previous line's end when it fits there and drops to its
/// own indented line otherwise, each lookup wrapping only its own break point —
/// so a base that breaks internally still keeps its lookups glued to the `)`/`}`
/// it ends on. The per-member counterpart of [`computed_lookup_doc`], which
/// breaks *inside* its brackets instead.
pub(crate) fn member_lookup_group(d: &DocArena, lookup: DocId) -> DocId {
    d.group(d.indent(d.concat(&[d.softline(), lookup])))
}

/// Build a computed member lookup — Prettier's `printMemberLookup` (member.js):
/// `group([open, indent([softline, index]), softline, "]"])`.
///
/// The brackets are where a computed access sheds width. Prettier never places a break
/// point *before* the `[` (`printMemberExpression`'s `shouldInline` includes
/// `node.computed`), so callers must keep the lookup glued to the object — see
/// `starts_segment` in the member-only builder. A non-breakable lookup (`breakable ==
/// false`, the numeric-index carve-out) has no break point at all, and so overflows the
/// print width rather than splitting.
fn computed_lookup_doc(
    printer: &Printer<'_>,
    open: &'static str,
    index: DocId,
    breakable: bool,
) -> DocId {
    let d = printer.arena();
    if !breakable {
        return d.concat(&[d.text(open), index, d.text("]")]);
    }
    let indented = d.indent(d.concat(&[d.softline(), index]));
    d.group(d.concat(&[d.text(open), indented, d.softline(), d.text("]")]))
}

/// Print a member access (shared logic for Member and PrivateMember)
///
/// Emits comments before the member access:
/// - Block comments inline (e.g., `a /* comment */.b`)
/// - Line comments via line_suffix (moved to end of line, matching Prettier)
///
/// The `skip_comments` flag is used by the expanded path where `add_comments_and_break`
/// already handles comments for the first member of rest groups.
/// Source positions for a member-access chain node.
struct MemberSpans {
    /// Start of the property name (a `#`-private name anchors past the `#`).
    name_start: u32,
    /// End of the object expression (start of the object→property gap).
    object_end: u32,
    /// Start of the property (end of the object→property gap).
    property_start: u32,
}

fn print_member_access(
    printer: &Printer<'_>,
    property: internal::IdentName<'_>,
    spans: MemberSpans,
    optional: bool,
    is_private: bool,
    skip_comments: bool,
) -> DocId {
    let d = printer.arena();
    // Build member doc without format! allocation — span-identity source slice
    // (or the escaped name's arena string)
    let prop_doc = printer.ident_doc(property, spans.name_start);
    let member_doc = match (optional, is_private) {
        (false, false) => d.concat(&[d.text("."), prop_doc]),
        (true, false) => d.concat(&[d.text("?."), prop_doc]),
        (false, true) => d.concat(&[d.text(".#"), prop_doc]),
        (true, true) => d.concat(&[d.text("?.#"), prop_doc]),
    };

    if skip_comments {
        return member_doc;
    }

    // Chain-level zero-comment gate: when the whole chain span is comment-free (the
    // common case), no member gap can carry a comment — each gap (object_end,
    // property_start) lies within the chain span — so skip the per-member
    // classification and its 5-child comment concat entirely. Byte-identical: an empty
    // classification renders every comment slot to nothing, leaving just member_doc.
    if !printer.chain_has_comments() {
        return member_doc;
    }

    // Classify all comments in the range between object and property
    let classified = printer.classify_comments(spans.object_end, spans.property_start);

    // Trailing block comments: same line as previous element (e.g., `method() /* c */.prop`)
    let trailing_block = printer.build_trailing_block_doc(&classified.trailing_block);

    // Line comments (both trailing and leading) get moved to end of line via line_suffix.
    // This matches Prettier's behavior of hoisting mid-chain line comments.
    let trailing_line = printer.build_deferred_line_comments_doc(&classified.trailing_line);
    let leading_line = printer.build_deferred_line_comments_doc(&classified.leading_line);

    // Leading block comments on their own line - emit inline (rare case)
    let leading_block = printer.build_trailing_block_doc(&classified.leading_block);

    d.concat(&[
        trailing_block,
        trailing_line,
        leading_block,
        leading_line,
        member_doc,
    ])
}

/// Check if a computed member node has block comments inside its brackets.
///
/// Used by the chain builder to route chains with inside-bracket comments
/// through the conditional_group path (instead of fill), so the bracket
/// content can break internally.
pub(crate) fn has_inside_bracket_comments<'a>(node: &ChainNode<'a>, printer: &Printer<'_>) -> bool {
    if let ChainNode::ComputedMember {
        expr,
        optional,
        object_end,
        bracket_end,
    } = node
    {
        let prop_span = printer.get_property_span(expr);
        let bracket_open_pos = find_bracket_position(printer, *object_end, prop_span.start);
        let inside_start = bracket_open_pos + if *optional { 3 } else { 1 };
        printer.has_comments_on_page_between(inside_start, prop_span.start)
            || printer.has_comments_on_page_between(prop_span.end, *bracket_end)
    } else {
        false
    }
}

/// Find the position of `[` (or `?.[` for optional) in the source,
/// skipping over comments to avoid matching `[` inside comments.
///
/// For `?.[`, returns the position of `?` (the start of the optional chain syntax).
fn find_bracket_position(printer: &Printer<'_>, start: u32, end: u32) -> u32 {
    let source = printer.get_source();
    let bytes = source.as_bytes();
    let start_pos = start as usize;
    let end_pos = end as usize;
    let mut i = start_pos;

    while i < end_pos {
        if let Some(new_i) = tsv_lang::source_scan::skip_comment(bytes, i, end_pos) {
            i = new_i;
            continue;
        }
        // Check for `?.[` first (returns position of `?`)
        if bytes[i] == b'?' && i + 2 < end_pos && bytes[i + 1] == b'.' && bytes[i + 2] == b'[' {
            return i as u32;
        }
        // Check for plain `[`
        if bytes[i] == b'[' {
            return i as u32;
        }
        i += 1;
    }
    start // Fallback
}

//
// Helper Functions
//

/// Emit a chain gap's comments and the line break into `parts`, for the gap
/// between `object_end` and `property_start` (i.e. before a `.member`).
///
/// Order: trailing block comments (`prev /* c */`), trailing line comments (same
/// line, via `line_suffix`), the line break (blank-line aware), then the gap's
/// leading run in authored order on the shared leading-run emitter
/// (`push_leading_comment_run` — glue, inter-comment blanks and the break toward
/// the property are its separators; the blank ABOVE the run is this gap's own, a
/// between-segments blank). Uses single-pass classification (one binary search).
///
/// This is the single definition of "how a forced chain break renders the comments
/// in its gap", shared by the call-chain group path (the `ChainPartsBuilder` group path) and the
/// member-only breaking path, so the two cannot drift (the historical member-only
/// `line_suffix`-everything approach was exactly such a drift — it merged/reversed
/// consecutive mid-chain line comments).
// The same-line/later-line classification (`classify_comments` →
// `tsv_lang::ClassifiedComments`) is shared with `conditional.rs`
// split_pre_operator_comments and `calls/arg_comments.rs` PartitionedComments, so the
// "same-line trails, later-line breaks, never merge" rule lives in one place. Only the
// emission differs per shape — dot (here) / operator / comma — which is intentional
// (this dot path also owns the blank-line preservation ABOVE the leading run).
pub(crate) fn push_gap_comments_and_break(
    parts: &mut DocBuf,
    printer: &Printer<'_>,
    object_end: u32,
    property_start: u32,
) {
    // Chain-level zero-comment gate: a comment-free chain span has no comment in this
    // gap (gap ⊆ span), so the only non-empty part below is the line break — the
    // classification and its four empty comment pushes collapse to nothing. Emit just
    // the break. Byte-identical (empty comment slots render to nothing).
    if !printer.chain_has_comments() {
        parts.push(build_chain_line_break(printer, object_end, property_start));
        return;
    }

    // Classify all comments in one pass (single binary search)
    let classified = printer.classify_comments(object_end, property_start);

    // Trailing block comments (same line as previous element): `method() /* c */`
    parts.push(printer.build_trailing_block_doc(&classified.trailing_block));
    // Trailing line comments (same line as previous element), via line_suffix. No
    // boundary: the break below always fires (`build_chain_line_break` is a hardline
    // at minimum) and flushes the suffix itself. A boundary would flush AND end the
    // line, so the break that follows would land a blank line after the comment.
    parts.push(printer.build_deferred_line_comments_doc(&classified.trailing_line));
    // Line break with blank line preservation
    parts.push(build_chain_line_break(printer, object_end, property_start));

    // The gap's own-line run, in AUTHORED order, on the single leading-run emitter.
    // The kind-split buckets exist for the trailing half above (a block trails inline,
    // a `//` defers); emitted kind-by-kind here they regrouped an interleaved
    // authoring (`// c⏎/* d */` → `/* d */⏎// c`), split authored glue
    // (`/* d */ // e`, `/* d */ .member`) and dropped the author blank between two
    // comments — all separator questions `push_leading_comment_run` owns, including
    // the blank-preserving break toward the property.
    let leading = classified.leading_in_source_order();
    if let Some(first) = leading.first() {
        // The blank ABOVE the run's first comment is NOT the leading-run rule
        // (`RunLeadingBlank::Drop` — prettier's `printLeadingComment` has no emitter
        // for it): here it is a blank between chain SEGMENTS, which prettier's
        // member-chain group separator preserves, so this gap keeps it —
        // `build_chain_line_break` skipped blank detection because comments exist
        // (`calls/chained/blank_line_before_comment` pins it).
        if has_blank_line_between_strict(printer.get_source(), object_end, first.span.start) {
            parts.push(printer.arena().hardline());
        }
        printer.push_leading_comment_run(
            parts,
            leading.iter().copied(),
            property_start,
            LeadingGlue::Adjacent,
            printer.arena().empty(),
        );
    }
}

/// The **chain-level** comment gap before a node — the source range whose comments the
/// chain builder owns, emitting them ahead of the line break it puts in front of the node.
///
/// For a computed member this stops at the `[` rather than running to the index: comments
/// *inside* the brackets belong to the bracket builder in [`print_node_inner`], so a chain
/// builder that scanned past the `[` would emit them a second time (and the arm's
/// boundary-less `line_suffix` copy would flush at the end of the line, merging consecutive
/// comments into one). The `[` scan is skipped on a comment-free chain, where every gap
/// query is empty regardless of the bound.
pub(crate) fn node_comment_gap(node: &ChainNode<'_>, printer: &Printer<'_>) -> Option<(u32, u32)> {
    let (object_end, property_start) = node.comment_range()?;
    if matches!(node, ChainNode::ComputedMember { .. }) && printer.chain_has_comments() {
        let bracket_open = find_bracket_position(printer, object_end, property_start);
        return Some((object_end, bracket_open));
    }
    Some((object_end, property_start))
}

/// The chain-level comment gap before a group's first node — see [`node_comment_gap`].
pub(crate) fn group_comment_gap(
    group: &ChainGroup<'_>,
    printer: &Printer<'_>,
) -> Option<(u32, u32)> {
    node_comment_gap(group.nodes.first()?, printer)
}

/// Build a line break doc with blank line preservation
///
/// Returns:
/// - `hardline` when no blank line in source
/// - `literalline + hardline` when blank line should be preserved
pub(crate) fn build_chain_line_break(
    printer: &Printer<'_>,
    object_end: u32,
    property_start: u32,
) -> DocId {
    let d = printer.arena();

    // Check for blank line preservation (only when no comments - comments handle their own spacing)
    // When there are comments between obj and property, the 2+ newlines (one before comment,
    // one after) should NOT be treated as a blank line.
    // Use strict check: verifies intermediate lines are truly blank (whitespace-only).
    // This avoids false positives when the parser strips grouping parens, leaving `)` between
    // newlines (e.g., `(fn({...}))\n.method()` → inner span ends before `)`, creating `\n)\n`).
    let source = printer.get_source();
    let has_comments = printer.has_comments_to_emit_between(object_end, property_start);

    if !has_comments && has_blank_line_between_strict(source, object_end, property_start) {
        // Preserve blank line: literalline (no indent) + hardline (with indent for next content)
        d.concat(&[d.literalline(), d.hardline()])
    } else {
        d.hardline()
    }
}
