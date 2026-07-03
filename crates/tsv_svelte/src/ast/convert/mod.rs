// Svelte AST conversion - Core module
//
// Converts internal AST to public JSON-compatible representation.
// Matches Svelte's official parser output format.

use serde::Serialize;

use crate::ast::{internal, public};
use tsv_lang::{Comment, LocationMapper, LocationTracker, Span};
use tsv_ts::ast::convert::convert_expression;

// Module declarations
mod attach_typed;
mod attributes;
mod blocks;
mod comment_attachment;
mod directives;
mod fragments;
mod special;
mod tags;
mod translate_typed;
mod write;

// Imported into the module root so sibling modules can reach them via `super::`
use attributes::{convert_attribute_node, convert_attribute_value};
use blocks::*;
use directives::*;
use fragments::convert_expression_tag;
use special::{convert_special_element, convert_svelte_options};
use tags::*;

// Import functions for our own use
use fragments::convert_fragment;
use special::{convert_script, convert_style};

pub use attach_typed::attach_template_expression_comments_typed;
pub use comment_attachment::attach_template_expression_comments;
pub use translate_typed::translate_byte_to_char_offsets_typed;
pub(crate) use write::write_root_bytes;

/// Convert an internal `Span` to a public `NameLocation`
///
/// Computes line/column via `LocationTracker` and includes the byte offset as `character`.
fn span_to_name_loc(span: Span, loc: &LocationTracker) -> public::NameLocation {
    let start = loc.offset_to_position(span.start_usize());
    let end = loc.offset_to_position(span.end_usize());
    public::NameLocation {
        start: public::NamePosition {
            line: start.line,
            column: start.column,
            character: span.start,
        },
        end: public::NamePosition {
            line: end.line,
            column: end.column,
            character: span.end,
        },
    }
}

/// Serialize a value to JSON, panicking on failure.
///
/// Our AST types derive `Serialize` correctly, so serialization cannot fail.
/// This helper centralizes the `#[allow]` annotation and safety justification.
///
/// # Panics
///
/// Panics if serialization fails (indicates a bug in our Serialize impl).
#[allow(clippy::expect_used)]
fn to_json_value<T: Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).expect("AST types derive Serialize correctly")
}

/// Convert a pattern expression (used by each context, await value/error, const id).
///
/// Simple identifiers get `character` in loc via `inject_loc_character()`.
/// Destructure patterns get column +1 via `adjust_read_pattern_columns()`.
fn convert_pattern_expression(
    expr: &tsv_ts::ast::internal::Expression<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &string_interner::DefaultStringInterner,
) -> serde_json::Value {
    let mut converted = convert_expression(expr, source, LocationMapper::identity(loc), interner);
    let is_destructure = matches!(
        converted,
        tsv_ts::ast::public::Expression::ObjectPattern(_)
            | tsv_ts::ast::public::Expression::ArrayPattern(_)
    );
    let mut value = if is_destructure {
        let mut value = to_json_value(&converted);
        adjust_read_pattern_columns(&mut value);
        value
    } else {
        converted.inject_loc_character();
        to_json_value(&converted)
    };
    strip_type_annotation_loc(&mut value);
    value
}

/// Strip `loc` from TSTypeAnnotation nodes in block pattern context.
///
/// Svelte's block pattern parser doesn't include `loc` on TSTypeAnnotation,
/// though acorn-typescript (used in script/snippet context) does.
fn strip_type_annotation_loc(value: &mut serde_json::Value) {
    if let serde_json::Value::Object(obj) = value {
        if obj.get("type").and_then(|v| v.as_str()) == Some("TSTypeAnnotation") {
            obj.remove("loc");
        }
        for v in obj.values_mut() {
            strip_type_annotation_loc(v);
        }
    } else if let serde_json::Value::Array(arr) = value {
        for v in arr.iter_mut() {
            strip_type_annotation_loc(v);
        }
    }
}

/// Adjust `loc.*.column` values by +1 for nodes on the pattern's starting line.
///
/// Svelte's `read_pattern()` constructs a synthetic source for acorn where line 1 of the source
/// is shortened by 1 byte (to compensate for an added `(` wrapper). This shifts the start of
/// the pattern's line by -1, making columns +1 compared to real source positions — but ONLY
/// for content on that specific line. Lines within the pattern (for multi-line patterns)
/// are unaffected because the pattern bytes are at the same positions.
///
/// Our parser computes correct columns, so we add +1 to match Svelte's quirky output.
/// Only called for destructure patterns (ObjectPattern, ArrayPattern) parsed via `read_pattern`.
fn adjust_read_pattern_columns(value: &mut serde_json::Value) {
    // Find the pattern's starting line from the root node's loc
    let target_line = value
        .get("loc")
        .and_then(|loc| loc.get("start"))
        .and_then(|s| s.get("line"))
        .and_then(serde_json::Value::as_u64);

    // Only adjust for patterns on line > 1. On line 1, the `(` wrapper compensates
    // for the removed space on the same line, so columns are already correct.
    if let Some(line) = target_line
        && line > 1
    {
        adjust_columns_on_line(value, line);
    }
}

fn adjust_columns_on_line(value: &mut serde_json::Value, target_line: u64) {
    match value {
        serde_json::Value::Object(map) => {
            // Adjust loc.start.column when on target_line, loc.end.column when on target_line
            if let Some(serde_json::Value::Object(loc)) = map.get_mut("loc") {
                for key in &["start", "end"] {
                    if let Some(serde_json::Value::Object(pos)) = loc.get_mut(*key) {
                        let on_target_line = pos
                            .get("line")
                            .and_then(serde_json::Value::as_u64)
                            .is_some_and(|l| l == target_line);
                        if on_target_line
                            && let Some(serde_json::Value::Number(col)) = pos.get_mut("column")
                            && let Some(n) = col.as_u64()
                        {
                            *col = serde_json::Number::from(n + 1);
                        }
                    }
                }
            }
            // Recurse into child values, skipping `loc` (already handled above)
            for (k, v) in map.iter_mut() {
                if k != "loc" {
                    adjust_columns_on_line(v, target_line);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                adjust_columns_on_line(v, target_line);
            }
        }
        _ => {}
    }
}

/// Convert Svelte Root AST to public format
pub fn convert_root<'src>(root: &internal::Root<'_>, source: &'src str) -> public::Root<'src> {
    convert_root_with_tracker(root, source, &LocationTracker::new(source))
}

/// `convert_root` with a caller-provided `LocationTracker` for `source`.
///
/// The tracker is a full-source line-index scan; callers that also need one
/// for byte→char translation (`convert_ast_json`, `convert_ast_json_string`)
/// build it once and share it, instead of paying a second scan.
pub fn convert_root_with_tracker<'src>(
    root: &internal::Root<'_>,
    source: &'src str,
    loc: &LocationTracker,
) -> public::Root<'src> {
    let interner = root.interner.borrow();

    // Svelte 5.x: Root.start/end always span the entire source (0 to source.len())
    let source_len = source.len() as u32;
    let (start, end) = (0, source_len);

    // Helper: find the HTML comment immediately preceding a tag in the fragment.
    // Only matches when the text between the comment end and the tag start is pure whitespace.
    let find_preceding_comment = |tag_start: u32| -> Option<&internal::HtmlComment> {
        root.fragment.nodes.iter().find_map(|node| {
            if let internal::FragmentNode::Comment(comment) = node
                && comment.span.end <= tag_start
            {
                let between = &source[comment.span.end as usize..tag_start as usize];
                if between.trim().is_empty() {
                    return Some(comment);
                }
            }
            None
        })
    };

    // Find HTML comment immediately preceding the style tag (for css.content.comment)
    let style_comment = root
        .css
        .as_ref()
        .and_then(|style| find_preceding_comment(style.span.start));

    // Find HTML comment immediately preceding the instance script (for leadingComments on Program)
    let instance_comment = root
        .instance
        .as_ref()
        .and_then(|script| find_preceding_comment(script.span.start));

    // Find HTML comment immediately preceding the module script (for leadingComments on Program)
    let module_comment = root
        .module
        .as_ref()
        .and_then(|script| find_preceding_comment(script.span.start));

    public::Root {
        css: root
            .css
            .as_ref()
            .map(|style| convert_style(style, source, loc, &interner, style_comment)),
        js: vec![],
        start,
        end,
        node_type: "Root",
        fragment: convert_fragment(&root.fragment, source, loc, &interner),
        options: root
            .options
            .as_ref()
            .map(|opts| convert_svelte_options(opts, source, loc, &interner)),
        comments: {
            // Each comment is emitted once: tsv corrects acorn-typescript's backtrack-reparse
            // comment duplication rather than replicating it (see docs/conformance_svelte.md
            // §Comment Attachment Differences).
            //
            // Helper to convert a Comment to its JSON representation:
            // the shared type/value/start/end shape plus a `loc` field
            let comment_to_json_value = |comment: &Comment| {
                let mut value = comment_attachment::comment_to_json(comment, source);
                let location = loc.span_to_location(comment.span);
                let loc_value = if comment.emit_character_field {
                    serde_json::json!({
                        "start": {
                            "line": location.start.line,
                            "column": location.start.column,
                            "character": comment.span.start,
                        },
                        "end": {
                            "line": location.end.line,
                            "column": location.end.column,
                            "character": comment.span.end,
                        },
                    })
                } else {
                    serde_json::json!({
                        "start": {
                            "line": location.start.line,
                            "column": location.start.column,
                        },
                        "end": {
                            "line": location.end.line,
                            "column": location.end.column,
                        },
                    })
                };
                if let Some(map) = value.as_object_mut() {
                    map.insert("loc".to_string(), loc_value);
                }
                value
            };

            root.comments.iter().map(comment_to_json_value).collect()
        },
        instance: root
            .instance
            .as_ref()
            .map(|script| convert_script(script, source, loc, &interner, instance_comment)),
        module: root
            .module
            .as_ref()
            .map(|script| convert_script(script, source, loc, &interner, module_comment)),
    }
}
