// Import expression and meta property printing
//
// Handles:
// - Dynamic import: `import('module')`, `import('module', options)`
// - Meta properties: `import.meta`, `new.target`

use super::super::Printer;
use super::super::utils::is_expandable_object;
use super::arg_comments::PartitionedComments;
use crate::ast::internal;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::{self, Doc};

/// Build a Doc for a dynamic import expression: `import('module')` or `import('module', options)`
///
/// Uses "expand last arg" pattern when options is an object:
/// - First arg stays on same line as `import(`
/// - Only the options object expands with its properties indented
pub(super) fn build_import_expression_doc(
    printer: &Printer,
    import_expr: &internal::ImportExpression,
) -> Doc {
    let source_doc = printer.build_expression_doc(&import_expr.source);

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
                return doc::concat(vec![
                    doc::text("import("),
                    doc::indent(doc::concat(vec![doc::hardline(), doc::concat(parts)])),
                    doc::hardline(),
                    doc::text(")"),
                ]);
            }

            // Block comments only - keep inline
            let mut parts = vec![source_doc];
            pc.emit_trailing_comments(&mut parts, printer);
            return doc::concat(vec![
                doc::text("import("),
                doc::concat(parts),
                doc::text(")"),
            ]);
        }

        return doc::concat(vec![doc::text("import"), doc::parens(source_doc)]);
    };

    let options_doc = printer.build_expression_doc(options);
    let options_end = options.span().end;
    let paren_close = import_expr.span.end;

    // Check for trailing comments after the options arg
    let has_trailing_comments = printer.has_comments_between(options_end, paren_close);
    let has_trailing_line_comments = printer.has_line_comments_between(options_end, paren_close);

    // "Expand last arg" pattern for objects: keep first arg inline, only expand the object
    // Result: import(source, {\n\twith: {...},\n})
    if is_expandable_object(options) {
        if has_trailing_line_comments {
            // Line comments force multiline expansion
            // import(\n\t'./a',\n\t{opts} // comment\n)
            let pc = PartitionedComments::new(
                printer.comments,
                printer.line_breaks,
                options_end,
                paren_close,
            );

            let mut parts = vec![options_doc];
            pc.emit_trailing_comments(&mut parts, printer);

            doc::concat(vec![
                doc::text("import("),
                doc::indent(doc::concat(vec![
                    doc::hardline(),
                    source_doc,
                    doc::text(","),
                    doc::hardline(),
                    doc::concat(parts),
                ])),
                doc::hardline(),
                doc::text(")"),
            ])
        } else if has_trailing_comments {
            // Block comments only - keep inline
            let pc = PartitionedComments::new(
                printer.comments,
                printer.line_breaks,
                options_end,
                paren_close,
            );

            let mut parts = vec![source_doc, doc::text(", "), options_doc];
            pc.emit_trailing_comments(&mut parts, printer);

            doc::concat(vec![
                doc::text("import("),
                doc::concat(parts),
                doc::text(")"),
            ])
        } else {
            doc::concat(vec![
                doc::text("import"),
                doc::text("("),
                source_doc,
                doc::text(", "),
                options_doc,
                doc::text(")"),
            ])
        }
    } else {
        // Standard wrapping for non-object options
        if has_trailing_line_comments {
            // Line comments force multiline expansion
            let pc = PartitionedComments::new(
                printer.comments,
                printer.line_breaks,
                options_end,
                paren_close,
            );

            let mut parts = vec![options_doc];
            pc.emit_trailing_comments(&mut parts, printer);

            doc::concat(vec![
                doc::text("import("),
                doc::indent(doc::concat(vec![
                    doc::hardline(),
                    source_doc,
                    doc::text(","),
                    doc::hardline(),
                    doc::concat(parts),
                ])),
                doc::hardline(),
                doc::text(")"),
            ])
        } else if has_trailing_comments {
            let pc = PartitionedComments::new(
                printer.comments,
                printer.line_breaks,
                options_end,
                paren_close,
            );

            let mut parts = vec![options_doc];
            pc.emit_trailing_comments(&mut parts, printer);

            let arg_parts = doc::join_doc([source_doc, doc::concat(parts)], doc::comma_line());
            doc::group(doc::concat(vec![
                doc::text("import"),
                doc::text("("),
                doc::indent_softline(arg_parts),
                doc::softline(),
                doc::text(")"),
            ]))
        } else {
            let arg_parts = doc::join_doc([source_doc, options_doc], doc::comma_line());
            doc::group(doc::concat(vec![
                doc::text("import"),
                doc::text("("),
                doc::indent_softline(arg_parts),
                doc::softline(),
                doc::text(")"),
            ]))
        }
    }
}

/// Build a Doc for a meta property: `import.meta`, `new.target`
pub(super) fn build_meta_property_doc(printer: &Printer, meta: &internal::MetaProperty) -> Doc {
    let meta_name = printer.resolve_symbol(meta.meta.name);
    let prop_name = printer.resolve_symbol(meta.property.name);
    doc::text_owned(format!("{meta_name}.{prop_name}"))
}
