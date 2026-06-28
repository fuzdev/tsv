// Svelte tag conversions
//
// Converts internal tag nodes to public format:
// - HtmlTag: {@html expression}
// - ConstTag: {@const x = value}
// - DeclarationTag: {const x = value} / {let x = value} / {let x}
// - DebugTag: {@debug x, y}
// - RenderTag: {@render snippet(args)}

use crate::ast::{internal, public};
use string_interner::DefaultStringInterner;
use tsv_lang::LocationTracker;
use tsv_ts::ast::convert::convert_expression;

use super::convert_pattern_expression;

pub(super) fn convert_html_tag<'src>(
    tag: &internal::HtmlTag<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::HtmlTag<'src> {
    let expression = convert_expression(&tag.expression, source, loc, interner, 0);

    public::HtmlTag {
        node_type: "HtmlTag",
        start: tag.span.start,
        end: tag.span.end,
        expression,
    }
}

pub(super) fn convert_const_tag(
    tag: &internal::ConstTag<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::ConstTag {
    let id_value = convert_pattern_expression(&tag.id, source, loc, interner);
    let init = convert_expression(&tag.init, source, loc, interner, 0);

    let declarator_start = tag.id.span().start;
    let declarator_end = tag.init.span().end;
    let declaration_start = tag.span.start + 2; // skip `{@`

    let declaration = serde_json::json!({
        "type": "VariableDeclaration",
        "kind": "const",
        "declarations": [{
            "type": "VariableDeclarator",
            "id": id_value,
            "init": init,
            "start": declarator_start,
            "end": declarator_end
        }],
        "start": declaration_start,
        "end": declarator_end
    });

    public::ConstTag {
        node_type: "ConstTag",
        start: tag.span.start,
        end: tag.span.end,
        declaration,
    }
}

pub(super) fn convert_declaration_tag(
    tag: &internal::DeclarationTag<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::DeclarationTag {
    // The body is a TS variable declaration; reuse tsv_ts's converter so the
    // public AST matches a standalone declaration exactly. Positions are byte-based
    // here; `translate_byte_to_char_offsets` (run over the whole tree in
    // `convert_ast_json`) makes them char-based.
    let declaration = super::to_json_value(&tsv_ts::ast::convert::convert_variable_declaration(
        &tag.declaration,
        source,
        loc,
        interner,
        0,
    ));

    public::DeclarationTag {
        node_type: "DeclarationTag",
        start: tag.span.start,
        end: tag.span.end,
        declaration,
    }
}

pub(super) fn convert_debug_tag<'src>(
    tag: &internal::DebugTag<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::DebugTag<'src> {
    let identifiers = tag
        .identifiers
        .iter()
        .map(|id| convert_expression(id, source, loc, interner, 0))
        .collect();

    public::DebugTag {
        node_type: "DebugTag",
        start: tag.span.start,
        end: tag.span.end,
        identifiers,
    }
}

pub(super) fn convert_render_tag<'src>(
    tag: &internal::RenderTag<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::RenderTag<'src> {
    let expression = convert_expression(&tag.expression, source, loc, interner, 0);

    public::RenderTag {
        node_type: "RenderTag",
        start: tag.span.start,
        end: tag.span.end,
        expression,
    }
}
