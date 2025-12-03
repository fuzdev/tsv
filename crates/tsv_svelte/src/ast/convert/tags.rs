// Svelte tag conversions
//
// Converts internal tag nodes to public format:
// - HtmlTag: {@html expression}
// - ConstTag: {@const x = value}
// - DebugTag: {@debug x, y}
// - RenderTag: {@render snippet(args)}

use crate::ast::{internal, public};
use string_interner::DefaultStringInterner;
use tsv_lang::LocationTracker;

pub(super) fn convert_html_tag(
    tag: &internal::HtmlTag,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::HtmlTag {
    let expression =
        tsv_ts::ast::convert::convert_expression(&tag.expression, source, loc, interner, 0);

    public::HtmlTag {
        node_type: "HtmlTag".to_string(),
        start: tag.span.start,
        end: tag.span.end,
        expression,
    }
}

pub(super) fn convert_const_tag(
    tag: &internal::ConstTag,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::ConstTag {
    // Convert id and init to public expressions
    let id = tsv_ts::ast::convert::convert_expression(&tag.id, source, loc, interner, 0);
    let init = tsv_ts::ast::convert::convert_expression(&tag.init, source, loc, interner, 0);

    // Calculate declarator span (from id start to init end)
    let declarator_start = tag.id.span().start;
    let declarator_end = tag.init.span().end;

    // Construct a proper VariableDeclaration structure
    let declaration = serde_json::json!({
        "type": "VariableDeclaration",
        "kind": "const",
        "declarations": [{
            "type": "VariableDeclarator",
            "id": id,
            "init": init,
            "start": declarator_start,
            "end": declarator_end
        }],
        "start": declarator_start,
        "end": declarator_end
    });

    public::ConstTag {
        node_type: "ConstTag".to_string(),
        start: tag.span.start,
        end: tag.span.end,
        declaration,
    }
}

pub(super) fn convert_debug_tag(
    tag: &internal::DebugTag,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::DebugTag {
    let identifiers = tag
        .identifiers
        .iter()
        .map(|id| tsv_ts::ast::convert::convert_expression(id, source, loc, interner, 0))
        .collect();

    public::DebugTag {
        node_type: "DebugTag".to_string(),
        start: tag.span.start,
        end: tag.span.end,
        identifiers,
    }
}

pub(super) fn convert_render_tag(
    tag: &internal::RenderTag,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::RenderTag {
    let expression =
        tsv_ts::ast::convert::convert_expression(&tag.expression, source, loc, interner, 0);

    public::RenderTag {
        node_type: "RenderTag".to_string(),
        start: tag.span.start,
        end: tag.span.end,
        expression,
    }
}
