// Import expression and meta property printing
//
// Handles:
// - Dynamic import: `import('module')`, `import('module', options)`
// - Meta properties: `import.meta`, `new.target`

use super::super::Printer;
use super::arg_comments::{
    PartitionedComments, has_blank_line_between_args, should_force_expansion_for_comments,
};
use super::arg_predicates::is_expandable_object;
use crate::ast::internal;
use smallvec::smallvec;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::{DocArena, DocId};

/// Wrap import args in a breakable group: `import(` + softline-indented `inner` +
/// softline + `)`. Stays inline when it fits, breaks each side onto its own line
/// otherwise. The shared shell for every block-comment / no-line-comment layout.
fn wrap_import_group(d: &DocArena, inner: DocId) -> DocId {
    d.group(d.concat(&[
        d.text("import("),
        d.indent_softline(inner),
        d.softline(),
        d.text(")"),
    ]))
}

/// Wrap import args in a forced-multiline layout: `import(` + hardline-indented
/// `inner` + hardline + `)`. Used whenever a line comment (which runs to EOL) or an
/// own-line comment forces the parens open.
fn wrap_import_hardline(d: &DocArena, inner: DocId) -> DocId {
    d.concat(&[
        d.text("import("),
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

    // Preserve comments between `import(` and the source expression, e.g.
    // import(/* @vite-ignore */ expr) — they would otherwise be lost. Own-line
    // comments force the parens to break; `leading_forces_break` drives that below.
    let open_paren_end = import_expr.span.start + "import(".len() as u32;
    let source_start = import_expr.source.span().start;

    let raw_source_doc = printer.build_expression_doc(import_expr.source);
    let (source_doc, leading_forces_break) =
        printer.build_paren_leading_value_doc(open_paren_end, source_start, raw_source_doc);

    let source_end = import_expr.source.span().end;
    let paren_close = import_expr.span.end;

    // If no options, check for trailing comments on the sole source arg.
    let Some(options) = &import_expr.options else {
        if printer.has_comments_between(source_end, paren_close) {
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
            // block stays inline and breaks only on width. (NOT isolated_group — it
            // causes indent issues; variable.rs special-cases the assignment break.)
            if pc.has_trailing_line() || !pc.leading.is_empty() || leading_forces_break {
                return wrap_import_hardline(d, inner);
            }
            return wrap_import_group(d, inner);
        }

        // Own-line leading comment: force hardline layout to preserve comment position.
        // Prettier's printLeadingComment() keeps own-line comments on their own line.
        if leading_forces_break {
            return wrap_import_hardline(d, source_doc);
        }

        // Group with softline break points so the outer import() can break when the
        // line exceeds print width, matching Prettier's call-arg expansion. Without
        // this, only the inner arg's groups can break (e.g., `import(fn(\n  'long',\n))`
        // instead of the correct `import(\n  fn('long')\n)`).
        return wrap_import_group(d, source_doc);
    };

    let options_doc = printer.build_expression_doc(options);
    let options_end = options.span().end;
    let options_start = options.span().start;

    let has_inter_comments = printer.has_comments_between(source_end, options_start);
    let has_trailing_comments = printer.has_comments_between(options_end, paren_close);
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
        inter.route_after_comma_hugging_to_leading(printer, source_end, options_start);

        // Source arg + comma: before-comma blocks trail the source; stranded after-comma
        // blocks and line comments follow the comma.
        let mut head = smallvec![source_doc];
        inter.emit_trailing_comments_around_comma(&mut head, printer, source_end, options_start);

        // Blank line in the gap, comment-aware once routed (so a comment's own newlines
        // don't read as a blank line).
        let inter_blank = inter_blank_no_comments
            || (has_inter_comments
                && inter.has_blank_line_in_gap(
                    printer.source,
                    printer.line_breaks,
                    source_end,
                    options_start,
                ));

        // Leading comments (own-line + hugged after-comma) lead the options arg; its
        // trailing region follows: same-line block/line comments inline, then own-line
        // comments each on their own line (dangling — import takes no trailing comma).
        let mut tail = DocBuf::new();
        inter.emit_leading_comments_inline_aware(&mut tail, printer, options_start);
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
            return wrap_import_hardline(d, body);
        }
        return wrap_import_group(d, body);
    }

    if is_expandable_object(options) {
        // Three-state conditional group matching Prettier's expand-last-arg:
        // State 0: all flat — import('source', {with: {type: 'json'}})
        // State 1: expand-last — import('source', {\n\twith: ...\n})
        // State 2: expand-all — import(\n\t'source',\n\t{...}\n)
        let state_flat = d.concat(&[
            d.text("import("),
            source_doc,
            d.text(", "),
            options_doc,
            d.text(")"),
        ]);

        let expanded_options = d.group_break(options_doc);
        let state_expand_last = d.concat(&[
            d.text("import("),
            source_doc,
            d.text(", "),
            expanded_options,
            d.text(")"),
        ]);

        let arg_parts = d.join_doc([source_doc, options_doc], d.comma_line());
        let state_expand_all = d.concat(&[
            d.text("import("),
            d.indent_softline(arg_parts),
            d.softline(),
            d.text(")"),
        ]);

        d.conditional_group(&[state_flat, state_expand_last, state_expand_all])
    } else {
        // Standard group wrapping for non-expandable options
        let arg_parts = d.join_doc([source_doc, options_doc], d.comma_line());
        wrap_import_group(d, arg_parts)
    }
}

/// Build a Doc for a meta property: `import.meta`, `new.target`
pub(super) fn build_meta_property_doc(
    printer: &Printer<'_>,
    meta: &internal::MetaProperty<'_>,
) -> DocId {
    let d = printer.d();
    let meta_name = printer.resolve_symbol(meta.meta.name);
    let prop_name = printer.resolve_symbol(meta.property.name);
    d.text_owned(format!("{meta_name}.{prop_name}"))
}
