// Svelte special node conversions
//
// Converts internal special nodes to public format:
// - Script: <script> tags with TypeScript content
// - Style: <style> tags with CSS content
// - SvelteOptions: <svelte:options> configuration
// - SpecialElement: <svelte:*> elements (head, body, window, etc.)

use crate::ast::{internal, public};
use string_interner::DefaultStringInterner;
use tsv_lang::LocationTracker;

use super::{
    attach_comments_recursively, convert_attribute_node, convert_fragment,
    to_json_value, CommentAttachmentContext,
};

pub(super) fn convert_script(
    script: &internal::Script,
    source: &str,
    interner: &DefaultStringInterner,
) -> public::Script {
    let context = script.context.as_str();

    // Use full source LocationTracker for absolute line/column numbers everywhere
    let loc = LocationTracker::new(source);

    // Delegate to tsv_ts for program conversion
    let mut program = tsv_ts::ast::convert::convert_program(&script.content, source, &loc);

    // Svelte's quirk: loc.start is hardcoded to {line: 1, column: 0}
    program.loc.start = tsv_ts::ast::public::Position { line: 1, column: 0 };

    // Attach comments to all nodes (recursively)
    // Convert program to JSON so we can inject leadingComments/trailingComments
    //
    // Architecture note: We use JSON roundtrip instead of adding fields to AST structs
    // because it keeps the internal TypeScript AST clean (zero Svelte-specific pollution).
    // The trade-off is we lose type safety on Script.content (now serde_json::Value),
    // but this is acceptable for the public API layer.
    let mut program_json = to_json_value(&program);

    // Track which comments have been attached to prevent duplicates
    let mut attached_indices = std::collections::HashSet::new();

    // Create context for comment attachment
    let mut ctx = CommentAttachmentContext {
        all_comments: &script.content.comments,
        source,
        attached_indices: &mut attached_indices,
    };

    // Recursively attach comments to all nodes in the AST
    attach_comments_recursively(
        &mut program_json,
        &mut ctx,
        None, // No parent for root Program node
        None,
        false, // Root node is not in an array
    );

    // TODO: Consider handling orphaned comments (comments that didn't attach anywhere)
    // Currently, comments that don't attach to any node are silently ignored.
    // Svelte handles this by attaching remaining comments to the Program node's trailingComments
    // See: acorn.js - "Special case: Trailing comments after the root node"
    // Could add:
    // if attached_indices.len() < script.content.comments.len() {
    //     let orphaned: Vec<_> = script.content.comments.iter().enumerate()
    //         .filter(|(i, _)| !attached_indices.contains(i))
    //         .collect();
    //     // Attach orphaned to Program.trailingComments or log warning
    // }

    // Return program_json directly (preserves leadingComments/trailingComments)
    public::Script {
        node_type: "Script".to_string(),
        start: script.span.start,
        end: script.span.end,
        context: context.to_string(),
        content: program_json,
        attributes: script
            .attributes
            .iter()
            .map(|attr| convert_attribute_node(attr, source, &loc, interner))
            .collect(),
    }
}

/// Convert internal SvelteOptions to public format
pub(super) fn convert_svelte_options(
    options: &internal::SvelteOptions,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::SvelteOptions {
    use tsv_lang::InfallibleResolve;

    // Check for `runes` attribute (boolean shorthand or explicit)
    let runes = options.attributes.iter().find_map(|attr| {
        if let internal::AttributeNode::Attribute(attr) = attr {
            if interner.resolve_infallible(attr.name) == "runes" {
                // Boolean shorthand: `runes` (no value) -> true
                // Explicit: `runes={true}` or `runes={false}`
                match &attr.value {
                    None => Some(true), // Boolean shorthand
                    Some(values) => {
                        // Look for expression tag with boolean literal
                        values.iter().find_map(|v| {
                            if let internal::AttributeValue::ExpressionTag(expr) = v
                                && let tsv_ts::ast::internal::Expression::Literal(lit) =
                                    &expr.expression
                                && let tsv_ts::ast::internal::LiteralValue::Boolean(b) = lit.value
                            {
                                return Some(b);
                            }
                            None
                        })
                    }
                }
            } else {
                None
            }
        } else {
            None
        }
    });

    public::SvelteOptions {
        start: options.span.start,
        end: options.span.end,
        attributes: options
            .attributes
            .iter()
            .map(|attr| convert_attribute_node(attr, source, loc, interner))
            .collect(),
        runes,
    }
}

pub(super) fn convert_style(
    style: &internal::Style,
    source: &str,
    interner: &DefaultStringInterner,
) -> tsv_css::StyleSheet {
    // Create LocationTracker for the full source (for attributes)
    let full_loc = LocationTracker::new(source);

    // Extract the raw CSS content
    let styles = style.content_span.extract(source).to_string();

    // Delegate to tsv_css for CSS node conversion
    // Filter out comments to match Svelte's CSS parser output
    // (Our internal AST has comments for the formatter, but public JSON AST should match Svelte)
    let children: Vec<serde_json::Value> = style
        .css_stylesheet
        .nodes
        .iter()
        .filter(|node| !matches!(node, tsv_css::ast::internal::CssNode::Comment(_)))
        .map(|node| tsv_css::ast::convert::convert_css_node(node, source))
        .collect();

    tsv_css::StyleSheet {
        node_type: "StyleSheet".to_string(),
        start: style.span.start,
        end: style.span.end,
        attributes: style
            .attributes
            .iter()
            .map(|attr| {
                let public_attr = convert_attribute_node(attr, source, &full_loc, interner);
                to_json_value(&public_attr)
            })
            .collect(),
        children,
        content: tsv_css::StyleContent {
            start: style.content_span.start,
            end: style.content_span.end,
            styles,
            comment: None,
        },
    }
}

pub(super) fn convert_special_element(
    elem: &internal::SpecialElement,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::SpecialElement {
    let tag = elem
        .tag
        .as_ref()
        .map(|e| tsv_ts::ast::convert::convert_expression(e, source, loc, interner, 0));

    let expression = elem
        .expression
        .as_ref()
        .map(|e| tsv_ts::ast::convert::convert_expression(e, source, loc, interner, 0));

    public::SpecialElement {
        node_type: elem.kind.node_type().to_string(),
        start: elem.span.start,
        end: elem.span.end,
        name: elem.kind.tag_name().to_string(),
        attributes: elem
            .attributes
            .iter()
            .map(|attr| convert_attribute_node(attr, source, loc, interner))
            .collect(),
        fragment: convert_fragment(&elem.fragment, source, loc, interner),
        tag,
        expression,
    }
}
