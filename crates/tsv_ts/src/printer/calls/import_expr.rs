// Import expression and meta property printing
//
// Handles:
// - Dynamic import: `import('module')`, `import('module', options)`
// - Meta properties: `import.meta`, `new.target`

use super::super::Printer;
use super::super::utils::is_expandable_object;
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

    // If no options, simple case
    let Some(options) = &import_expr.options else {
        return doc::concat(vec![doc::text("import"), doc::parens(source_doc)]);
    };

    let options_doc = printer.build_expression_doc(options);

    // "Expand last arg" pattern for objects: keep first arg inline, only expand the object
    // Result: import(source, {\n\twith: {...},\n})
    if is_expandable_object(options) {
        doc::concat(vec![
            doc::text("import"),
            doc::text("("),
            source_doc,
            doc::text(", "),
            options_doc,
            doc::text(")"),
        ])
    } else {
        // Standard wrapping for non-object options
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

/// Build a Doc for a meta property: `import.meta`, `new.target`
pub(super) fn build_meta_property_doc(printer: &Printer, meta: &internal::MetaProperty) -> Doc {
    let meta_name = printer.resolve_symbol(meta.meta.name);
    let prop_name = printer.resolve_symbol(meta.property.name);
    doc::text_owned(format!("{meta_name}.{prop_name}"))
}
