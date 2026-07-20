// Import expression and meta property printing
//
// Handles:
// - Dynamic import: `import('module')`, `import('module', options)`
// - Meta properties: `import.meta`, `new.target`

use super::super::Printer;
use super::arg_comments::{PartitionedComments, should_force_expansion_for_comments};
use super::arg_predicates::is_expandable_object;
use crate::ast::internal;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::{DocArena, DocId};

/// The phase keyword of a phased dynamic import (`source` / `defer`), or `None` for a
/// plain `import(`.
fn import_phase_word(phase: internal::ImportPhase) -> Option<&'static str> {
    match phase {
        internal::ImportPhase::None => None,
        internal::ImportPhase::Source => Some("source"),
        internal::ImportPhase::Defer => Some("defer"),
    }
}

/// The whole opening of a (possibly phased) dynamic import — `import`, the phase
/// (`.source` / `.defer`) when there is one, and the `(` — plus the offset where the
/// caller's leading-comment scan must begin.
///
/// A phased import is a dotted pair (`import` `.` `source`), so both gaps around its dot
/// are positions an author can comment in; the shared printer emits them. Baking the
/// pair into one fixed text (`"import.source("`) scans neither and drops what's there.
///
/// The returned offset is the **head's end** — just past `import`, or past the phase
/// word — deliberately *before* the `(` rather than after it. The caller scans from
/// there to the first argument, so that one range covers both the head→`(` gap and the
/// `(`→argument gap, and every comment in the opening reaches the caller's emitter
/// exactly as before. `span.start + import_open(phase).len()` could not: it assumes the
/// opening is contiguous, so a comment inside it shifts the real `(` past that offset
/// and the scan starts mid-comment, missing it. That was the drop.
fn build_import_open_doc(
    printer: &Printer<'_>,
    import_expr: &internal::ImportExpression<'_>,
) -> (DocId, u32) {
    let d = printer.d();
    let start = import_expr.span.start;
    let import_end = start + "import".len() as u32;
    // Bounds every scan below: the first argument is the next real token, and no `(` or
    // phase word of this import's own lies past it.
    let source_start = import_expr.source.span().start;

    // The head (`import`, plus `.source` / `.defer`) and where it ends in source.
    let (head, head_end) = match import_phase_word(import_expr.phase) {
        None => (d.text("import"), import_end),
        Some(word) => match printer.find_keyword_in_range(import_end, source_start, word) {
            Some(word_start) => (
                printer.build_dotted_pair_doc(
                    d.text("import"),
                    d.text(word),
                    import_end,
                    word_start,
                ),
                word_start + word.len() as u32,
            ),
            None => {
                // Only reachable on a synthetic span: the parser set the phase by
                // reading this very word out of the source.
                debug_assert!(
                    false,
                    "phased import has no `{word}` in source[{import_end}..{source_start}]"
                );
                (d.text("import"), import_end)
            }
        },
    };

    (d.concat(&[head, d.text("(")]), head_end)
}

/// Wrap import args in a breakable group: `<open>` + softline-indented `inner` +
/// softline + `)`. Stays inline when it fits, breaks each side onto its own line
/// otherwise. The shared shell for every block-comment / no-line-comment layout.
fn wrap_import_group(d: &DocArena, open: DocId, inner: DocId) -> DocId {
    d.group(d.concat(&[open, d.indent_softline(inner), d.softline(), d.text(")")]))
}

/// Wrap import args in a forced-multiline layout: `<open>` + hardline-indented
/// `inner` + hardline + `)`. Used whenever a line comment (which runs to EOL) or an
/// own-line comment forces the parens open.
fn wrap_import_hardline(d: &DocArena, open: DocId, inner: DocId) -> DocId {
    d.concat(&[
        open,
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

    // The opening — `import`, any phase (`.source` / `.defer`), and the `(` — built once
    // and threaded into every wrapper. It also reports where the leading-comment scan
    // starts: the head's end, so the scan spans the `(` and catches a comment on either
    // side of it (`import /* c */ ('m')`, `import(/* @vite-ignore */ m)`). Own-line
    // comments force the parens to break; `leading_forces_break` drives that below.
    let (open, leading_scan_start) = build_import_open_doc(printer, import_expr);
    let source_start = import_expr.source.span().start;

    // Parenthesize an `in` argument inside a for-header init (`for (a = import(m,
    // (b in c));…)`); a no-op elsewhere. Dynamic-import args are hand-rolled here
    // rather than routed through `needs_parens(Argument)`, so apply the rule directly.
    let raw_source_doc = printer.wrap_for_init_in(
        import_expr.source,
        printer.build_expression_doc(import_expr.source),
    );
    let (source_doc, leading_forces_break) =
        printer.build_paren_leading_value_doc(leading_scan_start, source_start, raw_source_doc);

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
    let inter_blank_no_comments =
        !has_inter_comments && printer.is_next_line_empty(source_end, options_start);

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
        let state_flat = d.concat(&[open, source_doc, d.text(", "), options_doc, d.text(")")]);

        let expanded_options = d.group_break(options_doc);
        let state_expand_last = d.concat(&[
            open,
            source_doc,
            d.text(", "),
            expanded_options,
            d.text(")"),
        ]);

        let arg_parts = d.join_doc([source_doc, options_doc], d.comma_line());
        let state_expand_all = d.concat(&[
            open,
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
/// A meta property is a dotted pair of names, so it delegates to the shared
/// [`Printer::build_dotted_pair_doc`] — which emits both gaps around the `.`, the
/// positions an author can comment in (`new /* c */.target`, `new./* c */ target`).
/// A qualified name (`ns.Type`) is the same shape and shares that printer.
pub(super) fn build_meta_property_doc(
    printer: &Printer<'_>,
    meta: &internal::MetaProperty<'_>,
) -> DocId {
    printer.build_dotted_pair_doc(
        printer.identifier_name_doc(&meta.meta),
        printer.identifier_name_doc(&meta.property),
        meta.meta.span.end,
        meta.property.span.start,
    )
}
