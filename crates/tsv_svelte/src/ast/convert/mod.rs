// Svelte AST conversion - Core module
//
// Converts internal AST to public JSON-compatible representation.
// Matches Svelte's official parser output format.

use serde::Serialize;

use crate::ast::{internal, public};
use tsv_lang::{LocationTracker, printing};

// Module declarations
mod attributes;
mod blocks;
mod directives;
mod fragments;
mod special;
mod tags;

// Re-export functions needed by other modules within convert/
// These are visible via super:: from sibling modules
use attributes::{convert_attribute_node, convert_attribute_value};
use blocks::*;
use directives::*;
use fragments::{convert_expression_tag, convert_text};
use special::{convert_special_element, convert_svelte_options};
use tags::*;

// Import functions for our own use
use fragments::convert_fragment;
use special::{convert_script, convert_style};

/// Serialize a value to JSON, panicking on failure.
///
/// Our AST types derive `Serialize` correctly, so serialization cannot fail.
/// This helper centralizes the `#[allow]` annotation and safety justification.
///
/// # Panics
///
/// Panics if serialization fails (indicates a bug in our Serialize impl).
#[allow(clippy::expect_used)]
pub(crate) fn to_json_value<T: Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).expect("AST types derive Serialize correctly")
}

/// Context for comment attachment process
///
/// Holds shared state and data used throughout the comment attachment traversal.
/// Reduces parameter count from 9 to 7 for better readability.
pub(crate) struct CommentAttachmentContext<'a> {
    pub all_comments: &'a [tsv_ts::ast::internal::Comment],
    pub source: &'a str,
    pub attached_indices: &'a mut std::collections::HashSet<usize>,
}

/// Attach comments to a node based on position
///
/// Returns (leadingComments, trailingComments) as JSON arrays.
///
/// **Leading comments**: All comments between prev_end and node_start
/// **Trailing comments**: Comments after node_end with only whitespace/punctuation between
///   - Matches Svelte's `/^[,) \t]*$/` pattern (commas, close parens, spaces, tabs)
///   - See: svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js:183
///
/// **Deduplication**: Uses `attached_indices` HashSet to prevent double-attachment
///
/// Called recursively for all nodes in the AST tree by `attach_comments_recursively`.
pub(crate) fn attach_comments(
    ctx: &mut CommentAttachmentContext,
    node_start: u32,
    node_end: u32,
    prev_end: Option<u32>,
    next_start: Option<u32>,
    parent_end: Option<u32>,
    is_last_in_array: bool, // True only if this is the last element in a parent array
                            // TODO: Consider using enum for better type safety:
                            // enum NodePosition { LastInArray, NotLast, Standalone }
) -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
    let mut leading: Vec<serde_json::Value> = Vec::new();
    let mut trailing: Vec<serde_json::Value> = Vec::new();

    for (idx, comment) in ctx.all_comments.iter().enumerate() {
        // Skip if already attached
        if ctx.attached_indices.contains(&idx) {
            continue;
        }

        let comment_start = comment.span.start;
        let comment_end = comment.span.end;

        // Leading: comments between previous node end and this node start
        if comment_end <= node_start {
            if let Some(prev) = prev_end {
                if comment_start >= prev {
                    leading.push(comment_to_json(comment, ctx.source));
                    ctx.attached_indices.insert(idx);
                }
            } else {
                // No previous node - all comments before this node are leading
                leading.push(comment_to_json(comment, ctx.source));
                ctx.attached_indices.insert(idx);
            }
        }
        // Trailing: comments after node end, with only whitespace/punctuation between
        // Matches Svelte's behavior: /^[,) \t]*$/ pattern (commas, close parens, spaces, tabs)
        // See: svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js:183
        //
        // Key insight: When attaching multiple trailing comments, we check from the END of the
        // last trailing comment, not from node_end. This allows subsequent comments on new lines
        // to attach correctly (e.g., comment3 and comment4 both trailing to same node).
        // See: svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js - uses `end` variable
        //      that updates to `comment.end` after each attachment.
        else if comment_start >= node_end {
            // Nothing to do here - trailing attachment happens in second pass below
        }
    }

    // Second pass: Attach trailing comments
    // We need a separate pass to track the "current end position" as we attach multiple trailing comments
    //
    // Svelte has two modes (see acorn.js):
    // 1. is_last_in_body = true: Node is last in parent array → can attach MULTIPLE trailing comments with newlines
    // 2. is_last_in_body = false: Node is NOT last → can only attach ONE trailing comment on same line
    //
    // TODO: Consider optimizing by combining both passes into one
    // Currently iterate through comments twice (once for leading, once for trailing).
    // Could combine with early-out logic, but current approach is clearer and performance
    // impact is negligible (comments list is typically small, <100 items).

    let mut trailing_end = node_end;
    for (idx, comment) in ctx.all_comments.iter().enumerate() {
        // Skip already attached comments
        if ctx.attached_indices.contains(&idx) {
            continue;
        }

        let comment_start = comment.span.start;
        let comment_end = comment.span.end;

        // Only consider comments after the current trailing end
        if comment_start >= trailing_end {
            if is_last_in_array {
                // Last node in parent: attach multiple trailing comments (can have newlines)
                // Only check that comment is before parent end
                let should_attach = if let Some(parent) = parent_end {
                    comment_end <= parent
                } else {
                    true // No parent boundary - attach
                };

                if should_attach {
                    trailing.push(comment_to_json(comment, ctx.source));
                    ctx.attached_indices.insert(idx);
                    trailing_end = comment_end;
                }
            } else {
                // Not last node: apply strict same-line check /^[,) \t]*$/
                // TODO: Extract this pattern check into a helper function
                // Pattern matches Svelte's /^[,) \t]*$/ regex (see acorn.js:183)
                // Could be: fn is_trailing_punctuation_only(s: &str) -> bool
                let slice = &ctx.source[trailing_end as usize..comment_start as usize];
                let is_trailing = slice.chars().all(|c| matches!(c, ',' | ')' | ' ' | '\t'));

                if is_trailing {
                    // Check if before next node
                    let should_attach = if let Some(next) = next_start {
                        comment_end <= next
                    } else {
                        true // No next node - attach trailing
                    };

                    if should_attach {
                        trailing.push(comment_to_json(comment, ctx.source));
                        ctx.attached_indices.insert(idx);
                        trailing_end = comment_end;
                    }
                }
            }
        }
    }

    (leading, trailing)
}

/// Recursively attach comments to all nodes in a JSON AST
///
/// Walks the entire AST tree and attaches leading/trailing comments to any node
/// that has start/end positions, matching Svelte's behavior.
pub(crate) fn attach_comments_recursively(
    node: &mut serde_json::Value,
    ctx: &mut CommentAttachmentContext,
    parent_start: Option<u32>,
    parent_end: Option<u32>,
    is_last_in_array: bool, // True if this node is the last element in a parent array
) {
    // Only process objects (AST nodes)
    if let Some(obj) = node.as_object_mut() {
        // Skip Comment objects - they have type "Block" or "Line"
        // We don't want to attach comments to comments themselves
        if let Some(node_type) = obj.get("type").and_then(|v| v.as_str())
            && (node_type == "Block" || node_type == "Line")
        {
            return;
        }

        // Extract start/end if present
        let node_start = obj
            .get("start")
            .and_then(serde_json::Value::as_u64)
            .map(|v| v as u32);
        let node_end = obj
            .get("end")
            .and_then(serde_json::Value::as_u64)
            .map(|v| v as u32);

        if let (Some(start), Some(end)) = (node_start, node_end) {
            // Attach comments to this node
            // Note: parent_start/parent_end from recursively() are actually prev_end/next_start in array context
            let (leading, trailing) = attach_comments(
                ctx,
                start,
                end,
                parent_start,     // prev_end in array context
                parent_end, // next_start in array context (contains parent end for last element)
                parent_end, // parent boundary for trailing comment check
                is_last_in_array, // Passed down from parent context
            );

            if !leading.is_empty() {
                obj.insert(
                    "leadingComments".to_string(),
                    serde_json::Value::Array(leading),
                );
            }
            if !trailing.is_empty() {
                obj.insert(
                    "trailingComments".to_string(),
                    serde_json::Value::Array(trailing),
                );
            }
        }

        // Recursively process all child values
        // TODO: Consider filtering keys at collection time instead of during iteration
        // Current approach clones all keys then skips some; could filter earlier:
        // let keys: Vec<_> = obj.keys().filter(|k| k != "comments").cloned().collect();
        let keys: Vec<String> = obj.keys().cloned().collect();
        for key in keys {
            // Skip the "comments" array - these are Comment objects, not AST nodes
            // We don't want to attach comments to comments themselves
            if key == "comments" {
                continue;
            }

            if let Some(value) = obj.get_mut(&key) {
                match value {
                    serde_json::Value::Array(arr) => {
                        // First collect positions to avoid borrow checker issues
                        let positions: Vec<(Option<u32>, Option<u32>)> = arr
                            .iter()
                            .map(|item| {
                                let start = item
                                    .get("start")
                                    .and_then(serde_json::Value::as_u64)
                                    .map(|v| v as u32);
                                let end = item
                                    .get("end")
                                    .and_then(serde_json::Value::as_u64)
                                    .map(|v| v as u32);
                                (start, end)
                            })
                            .collect();

                        // Then process array elements with mutable iterator
                        for (i, item) in arr.iter_mut().enumerate() {
                            // Calculate prev/next siblings for this array
                            let prev_end = if i > 0 {
                                positions[i - 1].1
                            } else {
                                node_start // Use parent start if first element
                            };

                            let next_start = if i + 1 < positions.len() {
                                positions[i + 1].0
                            } else {
                                node_end // Use parent end if last element
                            };

                            // Check if this is the last element in the array
                            let is_last = i + 1 >= positions.len();

                            attach_comments_recursively(
                                item, ctx, prev_end, next_start,
                                is_last, // Pass true only for last element
                            );
                        }
                    }
                    serde_json::Value::Object(_) => {
                        // Process nested object (e.g., id, init, callee)
                        // Object children are not in arrays, so is_last_in_array = false
                        attach_comments_recursively(
                            value, ctx, node_start, node_end, false, // Not in array context
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Get comment value with indentation stripping applied (Svelte compatibility)
///
/// For multi-line block comments, strips leading indentation to match Svelte's behavior.
/// See: svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js:115-124
fn get_comment_value(comment: &tsv_ts::ast::internal::Comment, source: &str) -> String {
    if comment.is_block && comment.content.contains('\n') {
        printing::strip_comment_indentation(source, &comment.content, comment.span.start)
    } else {
        comment.content.clone()
    }
}

/// Convert Comment to JSON format (without loc - simplified for attachment)
pub(crate) fn comment_to_json(
    comment: &tsv_ts::ast::internal::Comment,
    source: &str,
) -> serde_json::Value {
    let comment_type = if comment.is_block { "Block" } else { "Line" };
    let value = get_comment_value(comment, source);

    serde_json::json!({
        "type": comment_type,
        "value": value,
        "start": comment.span.start,
        "end": comment.span.end,
    })
}

/// Convert Svelte Root AST to public format
pub fn convert_root(root: &internal::Root, source: &str) -> public::Root {
    let loc = LocationTracker::new(source);
    let interner = root.interner.borrow();

    // Svelte 5.x: Root.start/end always span the entire source (0 to source.len())
    let source_len = source.len() as u32;
    let (start, end) = (0, source_len);

    public::Root {
        css: root
            .css
            .as_ref()
            .map(|style| convert_style(style, source, &interner)),
        js: vec![],
        start,
        end,
        node_type: "Root".to_string(),
        fragment: convert_fragment(&root.fragment, source, &loc, &interner),
        options: root
            .options
            .as_ref()
            .map(|opts| convert_svelte_options(opts, source, &loc, &interner)),
        comments: root
            .ts_comments
            .iter()
            .map(|comment| {
                let comment_type = if comment.is_block { "Block" } else { "Line" };
                let location = loc.span_to_location(comment.span);

                // Apply Svelte's indentation stripping for multi-line block comments
                let value = if comment.is_block && comment.content.contains('\n') {
                    printing::strip_comment_indentation(
                        source,
                        &comment.content,
                        comment.span.start,
                    )
                } else {
                    comment.content.clone()
                };

                // Manually construct JSON object with specific field order: type, value, start, end, loc
                let mut map = serde_json::Map::new();
                map.insert(
                    "type".to_string(),
                    serde_json::Value::String(comment_type.to_string()),
                );
                map.insert("value".to_string(), serde_json::Value::String(value));
                map.insert(
                    "start".to_string(),
                    serde_json::Value::Number(comment.span.start.into()),
                );
                map.insert(
                    "end".to_string(),
                    serde_json::Value::Number(comment.span.end.into()),
                );
                map.insert(
                    "loc".to_string(),
                    serde_json::json!({
                        "start": {
                            "line": location.start.line,
                            "column": location.start.column,
                        },
                        "end": {
                            "line": location.end.line,
                            "column": location.end.column,
                        },
                    }),
                );
                serde_json::Value::Object(map)
            })
            .collect(),
        instance: root
            .instance
            .as_ref()
            .map(|script| convert_script(script, source, &interner)),
        module: root
            .module
            .as_ref()
            .map(|script| convert_script(script, source, &interner)),
    }
}
