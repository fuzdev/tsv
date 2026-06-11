// Import expression and meta property printing
//
// Handles:
// - Dynamic import: `import('module')`, `import('module', options)`
// - Meta properties: `import.meta`, `new.target`

use super::super::Printer;
use super::arg_comments::PartitionedComments;
use super::arg_predicates::is_expandable_object;
use crate::ast::internal;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::arena::DocId;

/// Build a Doc for a dynamic import expression: `import('module')` or `import('module', options)`
///
/// Uses "expand last arg" pattern when options is an object:
/// - First arg stays on same line as `import(`
/// - Only the options object expands with its properties indented
pub(super) fn build_import_expression_doc(
    printer: &Printer,
    import_expr: &internal::ImportExpression,
) -> DocId {
    let d = printer.d();

    // Preserve comments between `import(` and the source expression, e.g.
    // import(/* @vite-ignore */ expr) — they would otherwise be lost. Own-line
    // comments force the parens to break; `leading_forces_break` drives that below.
    let open_paren_end = import_expr.span.start + 7; // "import(" is 7 chars
    let source_start = import_expr.source.span().start;

    let raw_source_doc = printer.build_expression_doc(&import_expr.source);
    let (source_doc, leading_forces_break) =
        printer.build_paren_leading_value_doc(open_paren_end, source_start, raw_source_doc);

    // If no options, check for trailing comments on the source arg
    let Some(options) = &import_expr.options else {
        // Check for trailing comments (line OR block) between source and closing paren
        let source_end = import_expr.source.span().end;
        let paren_close = import_expr.span.end;

        if printer.has_comments_between(source_end, paren_close) {
            // Force multi-line format with trailing comment (no trailing comma)
            // Prettier: import(\n\t'path' // comment\n)
            // For block comments: import('path' /* comment */); stays inline
            let pc = PartitionedComments::new(
                printer.comments,
                printer.line_breaks,
                source_end,
                paren_close,
            );

            // Check if we have line comments (force multiline) or only block comments (keep inline)
            if !pc.trailing_line.is_empty() {
                let mut parts = vec![source_doc];
                pc.emit_trailing_comments(&mut parts, printer);

                // Wrap with hardlines for line comments
                // Note: NOT using isolated_group because it causes indent issues
                // Instead, variable.rs handles preventing assignment break via special casing
                return d.concat(&[
                    d.text("import("),
                    d.indent(d.concat(&[d.hardline(), d.concat(&parts)])),
                    d.hardline(),
                    d.text(")"),
                ]);
            }

            // Block comments only - wrap in group so import() can break
            let mut parts = vec![source_doc];
            pc.emit_trailing_comments(&mut parts, printer);
            let inner = d.concat(&parts);
            return d.group(d.concat(&[
                d.text("import("),
                d.indent(d.concat(&[d.softline(), inner])),
                d.softline(),
                d.text(")"),
            ]));
        }

        // Own-line leading comment: force hardline layout to preserve comment position.
        // Prettier's printLeadingComment() keeps own-line comments on their own line.
        if leading_forces_break {
            return d.concat(&[
                d.text("import("),
                d.indent(d.concat(&[d.hardline(), source_doc])),
                d.hardline(),
                d.text(")"),
            ]);
        }

        // Wrap in group with softline break points so the outer import()
        // can break when the line exceeds print width, matching Prettier's
        // call-arg expansion behavior. Without this, only the inner arg's
        // groups can break (e.g., `import(fn(\n  'long',\n))` instead of
        // the correct `import(\n  fn('long')\n)`).
        return d.group(d.concat(&[
            d.text("import("),
            d.indent(d.concat(&[d.softline(), source_doc])),
            d.softline(),
            d.text(")"),
        ]));
    };

    let options_doc = printer.build_expression_doc(options);
    let options_end = options.span().end;
    let paren_close = import_expr.span.end;

    // Check for trailing comments after the options arg
    let has_trailing_line_comments = printer.has_line_comments_between(options_end, paren_close);
    let has_trailing_comments =
        has_trailing_line_comments || printer.has_comments_between(options_end, paren_close);

    // Comment paths are the same regardless of whether options is an expandable object.
    // The is_expandable_object check only matters for the no-comment expand-last-arg pattern.
    if has_trailing_line_comments || leading_forces_break {
        // Line comments or own-line leading comments force hardline layout
        let mut opts_parts = vec![options_doc];
        if has_trailing_comments {
            let pc = PartitionedComments::new(
                printer.comments,
                printer.line_breaks,
                options_end,
                paren_close,
            );
            pc.emit_trailing_comments(&mut opts_parts, printer);
        }

        d.concat(&[
            d.text("import("),
            d.indent(d.concat(&[
                d.hardline(),
                source_doc,
                d.text(","),
                d.hardline(),
                d.concat(&opts_parts),
            ])),
            d.hardline(),
            d.text(")"),
        ])
    } else if has_trailing_comments {
        // Block comments — standard group wrapping
        let pc = PartitionedComments::new(
            printer.comments,
            printer.line_breaks,
            options_end,
            paren_close,
        );

        let mut opts_parts = vec![options_doc];
        pc.emit_trailing_comments(&mut opts_parts, printer);
        let opts_with_comment = d.concat(&opts_parts);

        let arg_parts = d.join_doc([source_doc, opts_with_comment], d.comma_line());
        d.group(d.concat(&[
            d.text("import("),
            d.indent_softline(arg_parts),
            d.softline(),
            d.text(")"),
        ]))
    } else if is_expandable_object(options) {
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
        d.group(d.concat(&[
            d.text("import("),
            d.indent_softline(arg_parts),
            d.softline(),
            d.text(")"),
        ]))
    }
}

/// Build a Doc for a meta property: `import.meta`, `new.target`
pub(super) fn build_meta_property_doc(printer: &Printer, meta: &internal::MetaProperty) -> DocId {
    let d = printer.d();
    let meta_name = printer.resolve_symbol(meta.meta.name);
    let prop_name = printer.resolve_symbol(meta.property.name);
    d.text_owned(format!("{meta_name}.{prop_name}"))
}
