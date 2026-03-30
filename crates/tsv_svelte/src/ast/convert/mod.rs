// Svelte AST conversion - Core module
//
// Converts internal AST to public JSON-compatible representation.
// Matches Svelte's official parser output format.

use std::collections::VecDeque;

use serde::Serialize;

use crate::ast::{internal, public};
use tsv_lang::{Comment, LocationTracker, Span, printing};

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
use fragments::convert_expression_tag;
use special::{convert_special_element, convert_svelte_options};
use tags::*;

// Import functions for our own use
use fragments::convert_fragment;
use special::{convert_script, convert_style};

/// Convert an internal `Span` to a public `NameLocation`
///
/// Computes line/column via `LocationTracker` and includes the byte offset as `character`.
pub(crate) fn span_to_name_loc(span: Span, loc: &LocationTracker) -> public::NameLocation {
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
pub(crate) fn to_json_value<T: Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).expect("AST types derive Serialize correctly")
}

/// Convert a pattern expression (used by each context, await value/error, const id).
///
/// Simple identifiers get `character` in loc via `inject_loc_character()`.
/// Destructure patterns get column +1 via `adjust_read_pattern_columns()`.
pub(crate) fn convert_pattern_expression(
    expr: &tsv_ts::ast::internal::Expression,
    source: &str,
    loc: &LocationTracker,
    interner: &string_interner::DefaultStringInterner,
) -> serde_json::Value {
    let mut converted = tsv_ts::ast::convert::convert_expression(expr, source, loc, interner, 0);
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
pub(crate) fn adjust_read_pattern_columns(value: &mut serde_json::Value) {
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

/// Context for comment attachment process
///
/// Holds a mutable queue of comments (sorted by position) that gets consumed
/// during the DFS walk, matching acorn's algorithm from:
/// svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js
pub(crate) struct CommentAttachmentContext<'a> {
    /// Comment queue sorted by start position. Comments are shifted from the front
    /// as they get attached to nodes during the DFS walk.
    pub comments: VecDeque<serde_json::Value>,
    /// Full source string for slice checks (trailing comment whitespace detection)
    pub source: &'a str,
}

/// Get the `start` field from a comment JSON value
fn comment_start(c: &serde_json::Value) -> u32 {
    c.get("start")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as u32
}

/// Get the `start` field from an AST node JSON value
fn node_start(node: &serde_json::Value) -> Option<u32> {
    node.get("start")
        .and_then(serde_json::Value::as_u64)
        .map(|v| v as u32)
}

/// Get the `end` field from an AST node JSON value
fn node_end(node: &serde_json::Value) -> Option<u32> {
    node.get("end")
        .and_then(serde_json::Value::as_u64)
        .map(|v| v as u32)
}

/// Get the `type` field from an AST node JSON value
fn node_type(node: &serde_json::Value) -> Option<&str> {
    node.get("type").and_then(serde_json::Value::as_str)
}

/// Check if a JSON value is an AST node (object with `type` field)
fn is_ast_node(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .is_some_and(|obj| obj.contains_key("type"))
}

/// Attach comments to all nodes in a JSON AST using acorn's DFS queue algorithm
///
/// Matches the behavior of `add_comments` in:
/// svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js
///
/// Algorithm:
/// 1. Comments are sorted by position in a queue (VecDeque)
/// 2. DFS walk visits every AST node
/// 3. At each node: consume leading comments (before node.start) from queue front
/// 4. Recurse into children (which consume their own comments from the queue)
/// 5. After recursion: check for trailing comments based on context
/// 6. Remaining comments after full walk → trailing on root
pub(crate) fn attach_comments_recursively(
    root: &mut serde_json::Value,
    ctx: &mut CommentAttachmentContext<'_>,
) {
    if ctx.comments.is_empty() {
        return;
    }

    // DFS walk with parent tracking
    walk_node(root, None, ctx);

    // Special case: Trailing comments after the root node
    // See acorn.js: "Special case: Trailing comments after the root node"
    if !ctx.comments.is_empty() {
        let root_end = node_end(root).unwrap_or(0);
        let root_type = node_type(root).unwrap_or("");

        if comment_start(&ctx.comments[0]) >= root_end || root_type == "Program" {
            let remaining: Vec<serde_json::Value> = ctx.comments.drain(..).collect();
            if let Some(obj) = root.as_object_mut() {
                let trailing = obj
                    .entry("trailingComments")
                    .or_insert_with(|| serde_json::Value::Array(Vec::new()));
                if let serde_json::Value::Array(arr) = trailing {
                    arr.extend(remaining);
                }
            }
        }
    }
}

/// Extracted parent context for comment attachment decisions.
///
/// Avoids cloning the entire parent node — only stores what `walk_node` needs:
/// - `end`: for the `node.end != parent.end` guard
/// - `last_body_start`: start position of the last element in body/elements/properties
///   (None if parent isn't BlockStatement/Program/ArrayExpression/ObjectExpression,
///   or if the relevant array is empty)
struct ParentInfo {
    end: u32,
    last_body_start: Option<u32>,
}

/// Extract parent info from a JSON AST node
fn extract_parent_info(parent: &serde_json::Value) -> ParentInfo {
    let p_end = node_end(parent).unwrap_or(0);

    let parent_type = node_type(parent).unwrap_or("");
    let array_key = match parent_type {
        "BlockStatement" | "Program" => Some("body"),
        "ArrayExpression" => Some("elements"),
        "ObjectExpression" => Some("properties"),
        _ => None,
    };

    let last_body_start = array_key.and_then(|key| {
        parent
            .get(key)
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.last())
            .and_then(node_start)
    });

    ParentInfo {
        end: p_end,
        last_body_start,
    }
}

/// DFS walk a single AST node, consuming comments from the queue
///
/// This is the core of acorn's `_` handler in the walk.
/// `parent_info` provides extracted parent context for trailing comment decisions.
fn walk_node(
    node: &mut serde_json::Value,
    parent_info: Option<&ParentInfo>,
    ctx: &mut CommentAttachmentContext<'_>,
) {
    let Some(obj) = node.as_object() else {
        return;
    };

    // Skip Comment objects (type "Block" or "Line")
    if let Some(t) = obj.get("type").and_then(|v| v.as_str())
        && (t == "Block" || t == "Line")
    {
        return;
    }

    // Must have start/end to be a valid AST node for comment attachment
    let Some(n_start) = node_start(node) else {
        return;
    };
    let Some(n_end) = node_end(node) else {
        return;
    };

    // --- Leading comments: consume from queue while comment.start < node.start ---
    let mut leading: Vec<serde_json::Value> = Vec::new();
    while ctx
        .comments
        .front()
        .is_some_and(|front| comment_start(front) < n_start)
    {
        let Some(comment) = ctx.comments.pop_front() else {
            break;
        };
        leading.push(comment);
    }

    if !leading.is_empty()
        && let Some(obj) = node.as_object_mut()
    {
        obj.insert(
            "leadingComments".to_string(),
            serde_json::Value::Array(leading),
        );
    }

    // --- Recurse into children (next()) ---
    recurse_children(node, ctx);

    // --- Trailing comments: check after recursion ---
    if ctx.comments.is_empty() {
        return;
    }

    // Guard: skip if node.end === parent.end (prevents double-attachment)
    // See acorn.js: "if (parent === undefined || node.end !== parent.end)"
    let parent_end_val = parent_info.map(|p| p.end);
    if let Some(p_end) = parent_end_val
        && n_end == p_end
    {
        return;
    }

    let first_comment_start = comment_start(&ctx.comments[0]);

    // Check is_last_in_body: node is last element in parent's body/elements/properties
    // See acorn.js lines 162-168
    let is_last_in_body = parent_info
        .and_then(|p| p.last_body_start)
        .is_some_and(|last_start| last_start == n_start);

    if is_last_in_body {
        // Last node in body: attach multiple trailing comments (can span newlines)
        // Stop at parent boundary
        let mut trailing: Vec<serde_json::Value> = Vec::new();

        while let Some(c_start) = ctx.comments.front().map(comment_start) {
            if let Some(p_end) = parent_end_val
                && c_start >= p_end
            {
                break;
            }
            let Some(comment) = ctx.comments.pop_front() else {
                break;
            };
            trailing.push(comment);
        }

        if !trailing.is_empty()
            && let Some(obj) = node.as_object_mut()
        {
            let existing = obj
                .entry("trailingComments")
                .or_insert_with(|| serde_json::Value::Array(Vec::new()));
            if let serde_json::Value::Array(arr) = existing {
                arr.extend(trailing);
            }
        }
    } else if n_end <= first_comment_start {
        // Not last in body: attach at most ONE trailing comment on same line
        // Regex: /^[,) \t]*$/
        let slice = &ctx.source[n_end as usize..first_comment_start as usize];
        if slice.chars().all(|c| matches!(c, ',' | ')' | ' ' | '\t'))
            && let Some(comment) = ctx.comments.pop_front()
            && let Some(obj) = node.as_object_mut()
        {
            obj.insert(
                "trailingComments".to_string(),
                serde_json::Value::Array(vec![comment]),
            );
        }
    }
}

/// Get the child key visit order for a node type, matching acorn/acorn-typescript.
///
/// zimmerframe's walk iterates `for (const key in node)`, which uses JS property
/// insertion order. acorn-typescript inserts properties in a specific order that
/// can differ from our serde serialization order.
///
/// Returns None for node types where our default Map insertion order matches.
fn acorn_child_key_order(node_type: &str) -> Option<&'static [&'static str]> {
    match node_type {
        // acorn-typescript inserts returnType BEFORE params for arrow functions
        // (the TS plugin adds returnType to the node before acorn's base parser adds params)
        "ArrowFunctionExpression" => Some(&["returnType", "id", "params", "body"]),
        _ => None,
    }
}

/// Recurse into all child AST nodes of a given node
///
/// Visits children matching acorn's property iteration order (zimmerframe behavior).
/// For each property value that is an AST node or array of AST nodes, calls walk_node.
fn recurse_children(node: &mut serde_json::Value, ctx: &mut CommentAttachmentContext<'_>) {
    let Some(obj) = node.as_object() else {
        return;
    };

    let n_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");

    // Collect keys that have child AST nodes
    // Skip: "comments", "leadingComments"/"trailingComments" (comment-related)
    let is_child_key = |k: &str, obj: &serde_json::Map<String, serde_json::Value>| -> bool {
        if matches!(k, "comments" | "leadingComments" | "trailingComments") {
            return false;
        }
        if let Some(val) = obj.get(k) {
            match val {
                serde_json::Value::Object(_) => is_ast_node(val),
                serde_json::Value::Array(arr) => arr.iter().any(is_ast_node),
                _ => false,
            }
        } else {
            false
        }
    };

    // Build ordered key list matching acorn's property iteration order
    let child_keys: Vec<String> = if let Some(order) = acorn_child_key_order(n_type) {
        // Use acorn's known order, then append any remaining keys in Map order
        let mut keys: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // First: keys from the acorn order (if they exist and have child AST nodes)
        for &key in order {
            if is_child_key(key, obj) {
                keys.push(key.to_string());
                seen.insert(key.to_string());
            }
        }

        // Then: remaining keys in Map insertion order
        for key in obj.keys() {
            if !seen.contains(key.as_str()) && is_child_key(key, obj) {
                keys.push(key.clone());
            }
        }

        keys
    } else {
        // Default: Map insertion order (matches acorn for most node types)
        obj.keys()
            .filter(|k| is_child_key(k, obj))
            .cloned()
            .collect()
    };

    // Extract parent info BEFORE mutating the node (avoids full clone)
    let parent_info = extract_parent_info(node);

    let Some(obj) = node.as_object_mut() else {
        return;
    };

    for key in child_keys {
        let Some(value) = obj.get_mut(&key) else {
            continue;
        };

        match value {
            serde_json::Value::Array(arr) => {
                for item in arr.iter_mut() {
                    if is_ast_node(item) {
                        walk_node(item, Some(&parent_info), ctx);
                    }
                }
            }
            serde_json::Value::Object(_) => {
                if is_ast_node(value) {
                    walk_node(value, Some(&parent_info), ctx);
                }
            }
            _ => {}
        }
    }
}

/// Get comment value with indentation stripping applied (Svelte compatibility)
///
/// For multi-line block comments, strips leading indentation to match Svelte's behavior.
/// See: svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js:115-124
fn get_comment_value(comment: &Comment, source: &str) -> String {
    if comment.is_block && comment.content.contains('\n') {
        printing::strip_comment_indentation(source, &comment.content, comment.span.start)
    } else {
        comment.content.clone()
    }
}

/// Convert Comment to JSON format (without loc - simplified for attachment)
pub(crate) fn comment_to_json(comment: &Comment, source: &str) -> serde_json::Value {
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
            .map(|style| convert_style(style, source, &interner, style_comment)),
        js: vec![],
        start,
        end,
        node_type: "Root".to_string(),
        fragment: convert_fragment(&root.fragment, source, &loc, &interner),
        options: root
            .options
            .as_ref()
            .map(|opts| convert_svelte_options(opts, source, &loc, &interner)),
        comments: {
            // Collect acorn type re-parse ranges from scripts.
            // Comments within these ranges get duplicated in acorn's output because
            // acorn-typescript re-parses certain type construct bodies, causing
            // the onComment callback to fire twice for comments in the re-parsed region.
            let mut reparse_ranges = Vec::new();
            if let Some(ref script) = root.instance {
                reparse_ranges.extend(tsv_ts::ast::convert::collect_acorn_type_reparse_ranges(
                    &script.content,
                ));
            }
            if let Some(ref script) = root.module {
                reparse_ranges.extend(tsv_ts::ast::convert::collect_acorn_type_reparse_ranges(
                    &script.content,
                ));
            }

            // Helper to convert a Comment to its JSON representation
            let comment_to_json_value = |comment: &Comment| {
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
                let loc_value = if comment.has_character_loc {
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
                map.insert("loc".to_string(), loc_value);
                serde_json::Value::Object(map)
            };

            tsv_ts::ast::convert::build_comments_with_duplicates(
                &root.comments,
                &reparse_ranges,
                comment_to_json_value,
            )
        },
        instance: root
            .instance
            .as_ref()
            .map(|script| convert_script(script, source, &interner, instance_comment)),
        module: root
            .module
            .as_ref()
            .map(|script| convert_script(script, source, &interner, module_comment)),
    }
}

/// Attach comments to template expressions in a converted Root JSON AST
///
/// Template expression comments are those in `root.comments` that fall outside `<script>` tags.
/// For each expression found in the Svelte template JSON, filters relevant comments and runs
/// the DFS comment attachment algorithm (matching Svelte's `parse_expression_at` → `add_comments`).
///
/// Must be called BEFORE `translate_byte_to_char_offsets` since comment positions are byte-based.
pub fn attach_template_expression_comments(
    root_json: &mut serde_json::Value,
    comments: &[Comment],
    script_spans: &[(u32, u32)],
    source: &str,
) {
    // Filter to comments outside script content spans (these are template expression comments)
    let template_comments: Vec<&Comment> = comments
        .iter()
        .filter(|c| {
            !script_spans
                .iter()
                .any(|&(s, e)| c.span.start >= s && c.span.end <= e)
        })
        .collect();

    if template_comments.is_empty() {
        return;
    }

    // Walk the Root JSON and find expression fields in Svelte template constructs
    walk_and_attach_expressions(root_json, &template_comments, source);
}

/// Walk the Svelte Root JSON and attach comments to template expression fields
///
/// Identifies Svelte node types by their `type` field and processes their expression fields.
/// Uses the parent Svelte node's span to filter comments (not the expression's own span),
/// matching Svelte's `parse_expression_at` which filters comments to `start >= index`.
fn walk_and_attach_expressions(
    value: &mut serde_json::Value,
    template_comments: &[&Comment],
    source: &str,
) {
    match value {
        serde_json::Value::Object(map) => {
            let node_type = map
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();

            // Get this node's span for use as comment filter boundary
            let container_start = map
                .get("start")
                .and_then(serde_json::Value::as_u64)
                .map(|v| v as u32);
            let container_end = map
                .get("end")
                .and_then(serde_json::Value::as_u64)
                .map(|v| v as u32);

            // Process expression fields based on Svelte node type.
            // Uses this Svelte node's span as the container boundary for comment filtering,
            // since comments like `{/* c */ expr}` fall between the `{` and `}` but outside
            // the inner expression's span.
            //
            // For blocks with child content (IfBlock, EachBlock, etc.), tighten the range
            // end to the start of the first child content to avoid including comments from
            // sibling expression contexts (e.g., {:else if /* c */ b} comments shouldn't
            // bleed into the parent {#if a} test).
            match node_type.as_str() {
                // ExpressionTag: {expression}
                // HtmlTag: {@html expression}
                // RenderTag: {@render expression}
                // AttachTag: [attach expression]
                // SpreadAttribute: {...expression}
                "ExpressionTag" | "HtmlTag" | "RenderTag" | "AttachTag" | "SpreadAttribute" => {
                    if let (Some(c_start), Some(c_end)) = (container_start, container_end)
                        && let Some(expr) = map.get_mut("expression")
                    {
                        try_attach_comments_to_expression(
                            expr,
                            template_comments,
                            source,
                            c_start,
                            c_end,
                        );
                    }
                }

                // IfBlock: {#if test}...{/if}
                // Tighten range end to consequent start (expression is in the opening tag)
                "IfBlock" => {
                    if let Some(c_start) = container_start {
                        let range_end = first_child_start(map, &["consequent"])
                            .or(container_end)
                            .unwrap_or(0);
                        if let Some(test) = map.get_mut("test") {
                            try_attach_comments_to_expression(
                                test,
                                template_comments,
                                source,
                                c_start,
                                range_end,
                            );
                        }
                    }
                }

                // KeyBlock: {#key expression}...{/key}
                "KeyBlock" => {
                    if let Some(c_start) = container_start {
                        let range_end = first_child_start(map, &["fragment"])
                            .or(container_end)
                            .unwrap_or(0);
                        if let Some(expr) = map.get_mut("expression") {
                            try_attach_comments_to_expression(
                                expr,
                                template_comments,
                                source,
                                c_start,
                                range_end,
                            );
                        }
                    }
                }

                // EachBlock: {#each expression as context (key)}...{/each}
                // - expression: comments from container start to body start
                // - context: SKIP (parsed by read_pattern, no comment collection)
                // - key: comments from container start to body start (parsed by
                //   parse_expression_at within parentheses)
                "EachBlock" => {
                    if let Some(c_start) = container_start {
                        let range_end = first_child_start(map, &["body"])
                            .or(container_end)
                            .unwrap_or(0);
                        if let Some(expr) = map.get_mut("expression") {
                            try_attach_comments_to_expression(
                                expr,
                                template_comments,
                                source,
                                c_start,
                                range_end,
                            );
                        }
                        // context: skip (patterns don't collect comments)
                        if let Some(key) = map.get_mut("key") {
                            try_attach_comments_to_expression(
                                key,
                                template_comments,
                                source,
                                c_start,
                                range_end,
                            );
                        }
                    }
                }

                // AwaitBlock: {#await expression then value catch error}
                // - expression: comments from container start to pending/then/catch start
                // - value: SKIP (parsed by read_pattern, no comment collection)
                // - error: SKIP (parsed by read_pattern, no comment collection)
                "AwaitBlock" => {
                    if let Some(c_start) = container_start {
                        let range_end = first_child_start(map, &["pending", "then", "catch"])
                            .or(container_end)
                            .unwrap_or(0);
                        if let Some(expr) = map.get_mut("expression") {
                            try_attach_comments_to_expression(
                                expr,
                                template_comments,
                                source,
                                c_start,
                                range_end,
                            );
                        }
                        // value/error: skip (patterns don't collect comments)
                    }
                }

                // SnippetBlock: {#snippet name(params)}
                "SnippetBlock" => {
                    if let Some(c_start) = container_start {
                        let range_end = first_child_start(map, &["body"])
                            .or(container_end)
                            .unwrap_or(0);
                        if let Some(expr) = map.get_mut("expression") {
                            try_attach_comments_to_expression(
                                expr,
                                template_comments,
                                source,
                                c_start,
                                range_end,
                            );
                        }
                        if let Some(serde_json::Value::Array(params)) = map.get_mut("parameters") {
                            for param in params.iter_mut() {
                                try_attach_comments_to_expression(
                                    param,
                                    template_comments,
                                    source,
                                    c_start,
                                    range_end,
                                );
                            }
                        }
                    }
                }

                // ConstTag: {@const id = init}
                // Svelte runs `add_comments(init)` on the init expression directly,
                // THEN constructs the VariableDeclaration wrapper. So we need to attach
                // comments to the init expression, not the whole declaration.
                // Also update VariableDeclaration.end to match Svelte's (container_end - 1).
                "ConstTag" => {
                    if let (Some(c_start), Some(c_end)) = (container_start, container_end)
                        && let Some(decl) = map.get_mut("declaration")
                    {
                        // Update declaration.end to match Svelte (parser.index - 1 = closing `}`)
                        if let Some(obj) = decl.as_object_mut() {
                            obj.insert(
                                "end".to_string(),
                                serde_json::Value::Number((c_end - 1).into()),
                            );
                        }
                        // Attach comments to init expression inside first declarator
                        if let Some(declarations) =
                            decl.get_mut("declarations").and_then(|d| d.as_array_mut())
                            && let Some(declarator) = declarations.first_mut()
                            && let Some(init) = declarator.get_mut("init")
                        {
                            try_attach_comments_to_expression(
                                init,
                                template_comments,
                                source,
                                c_start,
                                c_end,
                            );
                        }
                    }
                }

                // DebugTag: {@debug identifiers}
                "DebugTag" => {
                    if let (Some(c_start), Some(c_end)) = (container_start, container_end)
                        && let Some(serde_json::Value::Array(ids)) = map.get_mut("identifiers")
                    {
                        for id in ids.iter_mut() {
                            try_attach_comments_to_expression(
                                id,
                                template_comments,
                                source,
                                c_start,
                                c_end,
                            );
                        }
                    }
                }

                // Directives with expression fields
                "OnDirective"
                | "UseDirective"
                | "TransitionDirective"
                | "AnimateDirective"
                | "LetDirective" => {
                    if let (Some(c_start), Some(c_end)) = (container_start, container_end)
                        && let Some(expr) = map.get_mut("expression")
                        && !expr.is_null()
                    {
                        try_attach_comments_to_expression(
                            expr,
                            template_comments,
                            source,
                            c_start,
                            c_end,
                        );
                    }
                }

                // BindDirective, ClassDirective: expression is serde_json::Value
                "BindDirective" | "ClassDirective" => {
                    if let (Some(c_start), Some(c_end)) = (container_start, container_end)
                        && let Some(expr) = map.get_mut("expression")
                        && expr.is_object()
                    {
                        try_attach_comments_to_expression(
                            expr,
                            template_comments,
                            source,
                            c_start,
                            c_end,
                        );
                    }
                }

                // StyleDirective: value can be ExpressionTag or array
                "StyleDirective" => {
                    if let Some(val) = map.get_mut("value") {
                        // value can be: true, ExpressionTag object, or array of parts
                        walk_and_attach_expressions(val, template_comments, source);
                    }
                }

                // SvelteElement/SvelteComponent: tag and expression
                "SvelteElement" => {
                    if let (Some(c_start), Some(c_end)) = (container_start, container_end)
                        && let Some(tag) = map.get_mut("tag")
                    {
                        try_attach_comments_to_expression(
                            tag,
                            template_comments,
                            source,
                            c_start,
                            c_end,
                        );
                    }
                }

                "SvelteComponent" => {
                    if let (Some(c_start), Some(c_end)) = (container_start, container_end)
                        && let Some(expr) = map.get_mut("expression")
                    {
                        try_attach_comments_to_expression(
                            expr,
                            template_comments,
                            source,
                            c_start,
                            c_end,
                        );
                    }
                }

                _ => {}
            }

            // Recurse into Attribute value (can contain ExpressionTag)
            if node_type == "Attribute"
                && let Some(val) = map.get_mut("value")
            {
                walk_and_attach_expressions(val, template_comments, source);
            }

            // Recurse into child Svelte structures (fragment, attributes, etc.)
            // Skip "content" (script content) and expression fields we already handled
            for key in &[
                "fragment",
                "nodes",
                "attributes",
                "consequent",
                "alternate",
                "body",
                "pending",
                "then",
                "catch",
                "fallback",
                "children",
            ] {
                if let Some(child) = map.get_mut(*key) {
                    walk_and_attach_expressions(child, template_comments, source);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                walk_and_attach_expressions(item, template_comments, source);
            }
        }
        _ => {}
    }
}

/// Get the start position of the first available child content field
///
/// Used to tighten comment attachment range for block nodes (IfBlock, EachBlock, etc.)
/// so that comments from sibling expression contexts (e.g., {:else if}) don't bleed
/// into the parent block's expression.
fn first_child_start(
    map: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<u32> {
    let mut earliest: Option<u32> = None;
    for &key in keys {
        if let Some(child) = map.get(key) {
            // The child could be a Fragment (object with nodes) or null
            let start = child
                .get("nodes")
                .and_then(|n| n.as_array())
                .and_then(|arr| arr.first())
                .and_then(node_start)
                // Or the child itself might have a start
                .or_else(|| node_start(child));
            if let Some(s) = start {
                earliest = Some(earliest.map_or(s, |e: u32| e.min(s)));
            }
        }
    }
    earliest
}

/// Try to attach comments to a template expression JSON node
///
/// Filters template comments to those that would be collected during acorn's
/// `parse_expression_at`. This includes:
/// - Comments from `container_start` up to and including the expression
/// - Comments immediately after the expression (trailing), up to the next
///   non-whitespace, non-comment token (acorn scans ahead during parsing)
///
/// The `container_start` is the Svelte node's start (e.g., ExpressionTag start).
/// The `container_end` bounds the maximum extent for trailing comment scanning.
fn try_attach_comments_to_expression(
    expr_json: &mut serde_json::Value,
    template_comments: &[&Comment],
    source: &str,
    container_start: u32,
    container_end: u32,
) {
    let Some(expr_end) = node_end(expr_json) else {
        return;
    };

    // Compute the effective end of the expression's parsing window.
    // Acorn scans ahead after the expression looking for the next token,
    // encountering (and collecting) any comments along the way.
    // We scan source from expr.end, skipping whitespace and comments,
    // to find where acorn would stop.
    let effective_end = scan_past_trailing_comments(source, expr_end, container_end);

    // Filter comments within [container_start, effective_end)
    let comment_queue: VecDeque<serde_json::Value> = template_comments
        .iter()
        .filter(|c| c.span.start >= container_start && c.span.end <= effective_end)
        .map(|c| comment_to_json(c, source))
        .collect();

    if comment_queue.is_empty() {
        return;
    }

    let mut ctx = CommentAttachmentContext {
        comments: comment_queue,
        source,
    };

    attach_comments_recursively(expr_json, &mut ctx);
}

/// Scan source after an expression's end to find the effective end of comment collection
///
/// Acorn's token scanner reads past whitespace and comments when looking for the next token.
/// This function mimics that: starting at `pos`, skip whitespace and block/line comments,
/// and return the position after the last skipped comment. If no comments are found, returns `pos`.
fn scan_past_trailing_comments(source: &str, start: u32, limit: u32) -> u32 {
    let bytes = source.as_bytes();
    let mut pos = start as usize;
    let limit = limit as usize;
    let mut last_comment_end = start;

    while pos < limit && pos < bytes.len() {
        match bytes[pos] {
            b' ' | b'\t' | b'\r' | b'\n' => {
                pos += 1;
            }
            b'/' if pos + 1 < bytes.len() => {
                if bytes[pos + 1] == b'*' {
                    // Block comment: /* ... */
                    pos += 2;
                    while pos + 1 < bytes.len() {
                        if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                            pos += 2;
                            break;
                        }
                        pos += 1;
                    }
                    last_comment_end = pos as u32;
                } else if bytes[pos + 1] == b'/' {
                    // Line comment: // ...
                    pos += 2;
                    while pos < bytes.len() && bytes[pos] != b'\n' {
                        pos += 1;
                    }
                    last_comment_end = pos as u32;
                } else {
                    // Not a comment, stop scanning
                    break;
                }
            }
            _ => {
                // Non-whitespace, non-comment — stop scanning
                break;
            }
        }
    }

    last_comment_end
}
