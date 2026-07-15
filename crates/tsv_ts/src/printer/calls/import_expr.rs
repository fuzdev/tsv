// Import expression and meta property printing
//
// Handles:
// - Dynamic import: `import('module')`, `import('module', options)`
// - Meta properties: `import.meta`, `new.target`

use super::super::{CommentSpacing, Printer};
use super::arg_comments::{
    PartitionedComments, has_blank_line_between_args, should_force_expansion_for_comments,
};
use super::arg_predicates::is_expandable_object;
use crate::ast::internal;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::{DocArena, DocId};

/// The opening token of a (possibly phased) dynamic import: `import(`,
/// `import.source(`, or `import.defer(`.
fn import_open(phase: internal::ImportPhase) -> &'static str {
    match phase {
        internal::ImportPhase::None => "import(",
        internal::ImportPhase::Source => "import.source(",
        internal::ImportPhase::Defer => "import.defer(",
    }
}

/// Wrap import args in a breakable group: `<open>` + softline-indented `inner` +
/// softline + `)`. Stays inline when it fits, breaks each side onto its own line
/// otherwise. The shared shell for every block-comment / no-line-comment layout.
fn wrap_import_group(d: &DocArena, open: &'static str, inner: DocId) -> DocId {
    d.group(d.concat(&[
        d.text(open),
        d.indent_softline(inner),
        d.softline(),
        d.text(")"),
    ]))
}

/// Wrap import args in a forced-multiline layout: `<open>` + hardline-indented
/// `inner` + hardline + `)`. Used whenever a line comment (which runs to EOL) or an
/// own-line comment forces the parens open.
fn wrap_import_hardline(d: &DocArena, open: &'static str, inner: DocId) -> DocId {
    d.concat(&[
        d.text(open),
        d.indent(d.concat(&[d.hardline(), inner])),
        d.hardline(),
        d.text(")"),
    ])
}

/// Build a Doc for a dynamic import expression: `import('module')` or `import('module', options)`
///
/// Uses "expand last arg" pattern when options is an object:
/// - First arg stays on same line as `import(`
/// - Only the options object expands with its properties indented
pub(super) fn build_import_expression_doc(
    printer: &Printer<'_>,
    import_expr: &internal::ImportExpression<'_>,
) -> DocId {
    let d = printer.d();

    // Opening token: `import(`, `import.source(`, or `import.defer(` (import-phase
    // proposals). Threaded into every wrapper and used to bound the leading-comment scan.
    let open = import_open(import_expr.phase);

    // Preserve comments between the `(` and the source expression, e.g.
    // import(/* @vite-ignore */ expr) — they would otherwise be lost. Own-line
    // comments force the parens to break; `leading_forces_break` drives that below.
    let open_paren_end = import_expr.span.start + open.len() as u32;
    let source_start = import_expr.source.span().start;

    // Parenthesize an `in` argument inside a for-header init (`for (a = import(m,
    // (b in c));…)`); a no-op elsewhere. Dynamic-import args are hand-rolled here
    // rather than routed through `needs_parens(Argument)`, so apply the rule directly.
    let raw_source_doc = printer.wrap_for_init_in(
        import_expr.source,
        printer.build_expression_doc(import_expr.source),
    );
    let (source_doc, leading_forces_break) =
        printer.build_paren_leading_value_doc(open_paren_end, source_start, raw_source_doc);

    let source_end = import_expr.source.span().end;
    let paren_close = import_expr.span.end;

    // If no options, check for trailing comments on the sole source arg.
    let Some(options) = &import_expr.options else {
        if printer.has_comments_to_emit_between(source_end, paren_close) {
            let pc = PartitionedComments::new(
                printer.comments,
                printer.line_breaks,
                source_end,
                paren_close,
            );

            // Trailing region after the arg: same-line block/line comments inline, then
            // own-line comments each on their own line (dangling — import takes no
            // trailing comma). Without the dangling pass, own-line comments are dropped.
            let mut parts = smallvec![source_doc];
            pc.emit_trailing_comments(&mut parts, printer);
            pc.emit_dangling_comments(&mut parts, printer);
            let inner = d.concat(&parts);

            // A line comment (runs to EOL), any own-line comment, or an own-line leading
            // comment before the source forces the multiline layout; a lone same-line
            // block stays inline and breaks only on width. (variable.rs special-cases
            // the assignment break.)
            if pc.has_trailing_line() || !pc.leading.is_empty() || leading_forces_break {
                return wrap_import_hardline(d, open, inner);
            }
            return wrap_import_group(d, open, inner);
        }

        // Own-line leading comment: force hardline layout to preserve comment position.
        // Prettier's printLeadingComment() keeps own-line comments on their own line.
        if leading_forces_break {
            return wrap_import_hardline(d, open, source_doc);
        }

        // Group with softline break points so the outer import() can break when the
        // line exceeds print width, matching Prettier's call-arg expansion. Without
        // this, only the inner arg's groups can break (e.g., `import(fn(\n  'long',\n))`
        // instead of the correct `import(\n  fn('long')\n)`).
        return wrap_import_group(d, open, source_doc);
    };

    let options_doc = printer.wrap_for_init_in(options, printer.build_expression_doc(options));
    let options_end = options.span().end;
    let options_start = options.span().start;

    let has_inter_comments = printer.has_comments_on_page_between(source_end, options_start);
    let has_trailing_comments = printer.has_comments_to_emit_between(options_end, paren_close);
    // A blank line in the source→options gap (with no comment there) is preserved like
    // every other argument gap; the comment case re-derives it comment-aware below.
    let inter_blank_no_comments = !has_inter_comments
        && has_blank_line_between_args(
            printer.source,
            printer.line_breaks,
            source_end,
            options_start,
        );

    // All comment cases — plus a blank-line gap — share one layout: a comment in the
    // inter-argument gap (source→options), which the rest of this function never
    // examines and would otherwise drop (content loss); a trailing comment after options;
    // or an own-line comment before the source (`leading_forces_break`). Route both gaps
    // through the unified argument-comment helpers, so the respect-the-newline rule (a
    // hugging block leads the next arg; a stranded block stays on the comma line) is
    // inherited rather than re-implemented, and bypass the expand-last-arg conditional
    // group below — matching prettier disabling shouldExpandLastArg whenever an argument
    // carries a comment.
    if leading_forces_break
        || has_inter_comments
        || has_trailing_comments
        || inter_blank_no_comments
    {
        let mut inter = PartitionedComments::new(
            printer.comments,
            printer.line_breaks,
            source_end,
            options_start,
        );
        inter.route_after_comma_hugging_to_leading(printer);

        // Source arg + comma: before-comma blocks trail the source; stranded after-comma
        // blocks and line comments follow the comma.
        let mut head = smallvec![source_doc];
        inter.emit_trailing_comments_around_comma(&mut head, printer);

        // Blank line in the gap, comment-aware once routed (so a comment's own newlines
        // don't read as a blank line).
        let inter_blank = inter_blank_no_comments
            || (has_inter_comments
                && inter.has_blank_line_in_gap(printer.source, printer.line_breaks));

        // Leading comments (own-line + hugged after-comma) lead the options arg; its
        // trailing region follows: same-line block/line comments inline, then own-line
        // comments each on their own line (dangling — import takes no trailing comma).
        let mut tail = DocBuf::new();
        inter.emit_leading_comments_inline_aware(&mut tail, printer);
        tail.push(options_doc);
        let trailing = PartitionedComments::new(
            printer.comments,
            printer.line_breaks,
            options_end,
            paren_close,
        );
        trailing.emit_trailing_comments(&mut tail, printer);
        trailing.emit_dangling_comments(&mut tail, printer);

        // A line comment (runs to EOL), an own-line comment (leading before source, in
        // the gap, or dangling after options), or a blank line forces the multiline
        // layout; inline blocks (hugging / before-comma / same-line trailing) leave the
        // group free to stay inline and break only on width.
        let force_break = leading_forces_break
            || inter_blank
            || trailing.has_trailing_line()
            || !trailing.leading.is_empty()
            || should_force_expansion_for_comments(printer, source_end, options_start);

        // The source→options separator: a blank line when the author left one (a blank
        // line always forces the break), else a hardline when broken, else a soft `line`.
        let sep = if inter_blank {
            d.concat(&[d.literalline(), d.hardline()])
        } else if force_break {
            d.hardline()
        } else {
            d.line()
        };

        let body = d.concat(&[d.concat(&head), sep, d.concat(&tail)]);
        if force_break {
            return wrap_import_hardline(d, open, body);
        }
        return wrap_import_group(d, open, body);
    }

    if is_expandable_object(options) {
        // Three-state conditional group matching Prettier's expand-last-arg:
        // State 0: all flat — import('source', {with: {type: 'json'}})
        // State 1: expand-last — import('source', {\n\twith: ...\n})
        // State 2: expand-all — import(\n\t'source',\n\t{...}\n)
        let state_flat = d.concat(&[
            d.text(open),
            source_doc,
            d.text(", "),
            options_doc,
            d.text(")"),
        ]);

        let expanded_options = d.group_break(options_doc);
        let state_expand_last = d.concat(&[
            d.text(open),
            source_doc,
            d.text(", "),
            expanded_options,
            d.text(")"),
        ]);

        let arg_parts = d.join_doc([source_doc, options_doc], d.comma_line());
        let state_expand_all = d.concat(&[
            d.text(open),
            d.indent_softline(arg_parts),
            d.softline(),
            d.text(")"),
        ]);

        d.conditional_group(&[state_flat, state_expand_last, state_expand_all])
    } else {
        // Standard group wrapping for non-expandable options
        let arg_parts = d.join_doc([source_doc, options_doc], d.comma_line());
        wrap_import_group(d, open, arg_parts)
    }
}

/// Build a Doc for a meta property: `import.meta`, `new.target`
///
/// Both gaps around the `.` are real source positions an author can comment in
/// (`new /* c */.target`, `new./* c */ target`), so each is located and emitted.
/// Concatenating the three pieces scans neither gap and drops whatever is in it — the
/// same class as a comment inside a multi-word keyword (see `build_keyword_words_doc`
/// in `printer/comments/declarations.rs`), and the reason that class's usual detector
/// (a `d.text` literal with an *interior* space) is only a proxy: here the pieces are
/// joined by `"."`, which has no space to find.
///
/// Each side stays where it was authored, which is also what prettier prints: the
/// comment hugs the `.` and keeps its space on the identifier's side. A *line* comment
/// before the `.` ends its line, so `.property` continues one level down
/// (`new // c⏎\t.target`) — the shape a member access already takes.
pub(super) fn build_meta_property_doc(
    printer: &Printer<'_>,
    meta: &internal::MetaProperty<'_>,
) -> DocId {
    let d = printer.d();
    let meta_doc = printer.identifier_name_doc(&meta.meta);
    let prop_doc = printer.identifier_name_doc(&meta.property);
    let gap_start = meta.meta.span.end;
    let gap_end = meta.property.span.start;

    // `new.target` / `import.meta` with both gaps empty — every ordinary occurrence.
    if !printer.has_comments_to_emit_between(gap_start, gap_end) {
        return d.concat(&[meta_doc, d.text("."), prop_doc]);
    }

    let Some(dot) = printer.find_char_outside_comments(gap_start, gap_end, b'.') else {
        debug_assert!(
            false,
            "a meta property always spells a `.` between its two names"
        );
        return d.concat(&[meta_doc, d.text("."), prop_doc]);
    };

    // `.`→property: a block comment keeps its space on the property's side
    // (`./* c */ target`); a line comment ends the line, so the property drops a level.
    let after_dot = if !printer.has_comments_to_emit_between(dot + 1, gap_end) {
        prop_doc
    } else if printer.has_line_comments_between(dot + 1, gap_end) {
        printer.build_continuation_indent(dot + 1, gap_end, prop_doc)
    } else {
        d.concat(&[
            printer.build_comments_between(dot + 1, gap_end, CommentSpacing::Trailing),
            prop_doc,
        ])
    };
    let tail = d.concat(&[d.text("."), after_dot]);

    // meta→`.`: a line comment takes the whole `.property` onto the next line with it.
    if printer.has_line_comments_between(gap_start, dot) {
        return d.concat(&[
            meta_doc,
            printer.build_continuation_indent(gap_start, dot, tail),
        ]);
    }
    if printer.has_comments_to_emit_between(gap_start, dot) {
        return d.concat(&[
            meta_doc,
            printer.build_comments_between(gap_start, dot, CommentSpacing::Leading),
            tail,
        ]);
    }
    d.concat(&[meta_doc, tail])
}
