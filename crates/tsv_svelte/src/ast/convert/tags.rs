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

use super::convert_pattern_expression;

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
    let id_value = convert_pattern_expression(&tag.id, source, loc, interner);
    let init = tsv_ts::ast::convert::convert_expression(&tag.init, source, loc, interner, 0);

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
        node_type: "ConstTag".to_string(),
        start: tag.span.start,
        end: tag.span.end,
        declaration,
    }
}

pub(super) fn convert_declaration_tag(
    tag: &internal::DeclarationTag,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::DeclarationTag {
    // Unlike `{@const}` (Svelte builds the VariableDeclaration by hand, no `loc`,
    // and parses the pattern via `read_pattern` — hence the `character`/+1-column
    // quirks), `{const}`/`{let}` are acorn-parsed: the id/init use plain
    // expression conversion and the wrapper carries `loc`. Positions are emitted
    // byte-based; `translate_byte_to_char_offsets` (run over the whole tree in
    // `convert_ast_json`) converts them to char-based and never adds `character`.
    let id_value = super::to_json_value(&tsv_ts::ast::convert::convert_expression(
        &tag.id, source, loc, interner, 0,
    ));
    let init_value = match &tag.init {
        Some(init) => super::to_json_value(&tsv_ts::ast::convert::convert_expression(
            init, source, loc, interner, 0,
        )),
        None => serde_json::Value::Null,
    };

    let declarator_start = tag.id.span().start;
    let declarator_end = match &tag.init {
        Some(init) => init.span().end,
        None => tag.id.span().end,
    };
    let declaration_start = tag.span.start + 1; // skip `{`
    // The VariableDeclaration ends at the `}` (Svelte's `parser.index - 1`), not
    // at the last declarator — they coincide for `{const a = 1}` but not for a
    // binding-less `{let a;}`, whose `;` sits between the id and the `}`.
    let declaration_end = tag.span.end - 1;

    let loc_json = |start: u32, end: u32| {
        let s = loc.offset_to_position(start as usize);
        let e = loc.offset_to_position(end as usize);
        serde_json::json!({
            "start": {"line": s.line, "column": s.column},
            "end": {"line": e.line, "column": e.column},
        })
    };

    let declaration = serde_json::json!({
        "type": "VariableDeclaration",
        "start": declaration_start,
        "end": declaration_end,
        "loc": loc_json(declaration_start, declaration_end),
        "declarations": [{
            "type": "VariableDeclarator",
            "start": declarator_start,
            "end": declarator_end,
            "loc": loc_json(declarator_start, declarator_end),
            "id": id_value,
            "init": init_value,
        }],
        "kind": tag.kind.keyword(),
    });

    public::DeclarationTag {
        node_type: "DeclarationTag".to_string(),
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
