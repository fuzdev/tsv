// Svelte AST conversion
//
// Converts internal AST to public JSON-compatible representation.
// Matches Svelte's official parser output format.

use serde::Serialize;

use crate::ast::{internal, public};
use string_interner::DefaultStringInterner;
use tsv_lang::{InfallibleResolve, LocationTracker, printing};

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
fn attach_comments(
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
fn attach_comments_recursively(
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
fn comment_to_json(comment: &tsv_ts::ast::internal::Comment, source: &str) -> serde_json::Value {
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

    // Svelte Root start/end behavior (discovered through empirical testing)
    //
    // The Root node's start/end positions follow conditional rules based on fragment content:
    //
    // Rule 1: NULL when fragment is EMPTY (no nodes at all)
    //   - Script-only files: `<script>...</script>` → start=null, end=null (0 fragment nodes)
    //   - CSS-only files: `<style>...</style>` → start=null, end=null (0 fragment nodes)
    //   - Empty files or whitespace-only → start=null, end=null (0 fragment nodes)
    //
    // Rule 2: SET when fragment has ANY nodes (including Text nodes)
    //   - Text-only: `plain text` → start=0, end=10 (1 Text node)
    //   - Template-only: `<div></div>` → start=0, end=11 (1 Element node)
    //   - Script+template: `<script>...</script>\n<div></div>` → start=div_start, end=div_end
    //   - Any combination with fragment content (Text, Element, ExpressionTag, etc.)
    //
    // Rule 3: What the positions span
    //   - start: Position of the FIRST fragment node (Text, Element, or ExpressionTag)
    //   - end: Position AFTER the LAST fragment node (Text, Element, or ExpressionTag)
    //
    // Critical insight: Text nodes ARE fragment content
    //   - Text-only files get start/end set (e.g., "hello" → start=0, end=5)
    //   - Script/style-only files have empty fragments, so start=null, end=null
    //
    // Examples:
    //   `<div></div>` → start=0, end=11
    //   `\n\n<div></div>` → start=2 (skips leading text), end=13
    //   `<script>...</script>\n\n<div>{a}</div>` → start=53 (div), end=67 (after closing tag)
    //
    // Note: The Root span does NOT include script/CSS blocks - those are stored separately
    // in ast.instance, ast.module, and ast.css fields.
    //
    // ⚠️ SVELTE BUG REPLICATION (for exact compatibility):
    //
    // When there's script+CSS with NO template markup, Svelte has a bug where start > end:
    //   `<script>...</script>\n<style>...</style>` → start=34 (CSS start), end=33 (script end)
    //
    // This is clearly incorrect (start should never be greater than end), and the correct
    // behavior would be start=null, end=null (no template content).
    //
    // However, we replicate this bug EXACTLY for compatibility with Svelte's behavior.
    // This quirk is isolated to this conversion layer - our internal AST remains clean.
    //
    // Bug pattern:
    //   - Fragment has NO non-Text nodes (no template markup)
    //   - File has BOTH script (instance OR module) AND CSS
    //   - Result: start = css.span.start, end = script.span.end
    //   - This produces inverted positions since CSS comes after script
    //
    // See: tests/fixtures/3_svelte_parser/bug_script_style_without_markup/SVELTE_BUG.md
    //
    // NOTE: The parser now calculates start/end correctly in the internal AST (root.span),
    // so we use those values directly. The parser handles all the edge cases including:
    // - Script/style tags in any order
    // - Proper root.start positioning (first non-instance-script item)
    // - Maximum end across all top-level nodes
    let (start, end) = {
        // Check if fragment has ANY nodes (Text, Element, or ExpressionTag)
        // Text nodes ARE content - text-only files should get start/end set
        let has_fragment_content = !root.fragment.nodes.is_empty();

        if has_fragment_content {
            // Fragment has content (Text, Element, or ExpressionTag) - use parser's calculated span
            (Some(root.span.start), Some(root.span.end))
        } else {
            // Fragment is empty (script-only, CSS-only, or empty file)

            // ⚠️ BUG REPLICATION: Detect script+CSS with no template
            // Svelte produces inverted positions (start > end) in this case
            if let (Some(css), Some(script)) = (&root.css, &root.instance) {
                // BUG: start=css.start, end=script.end (inverted!)
                (Some(css.span.start), Some(script.span.end))
            } else if let (Some(css), Some(script)) = (&root.css, &root.module) {
                // BUG: start=css.start, end=module.end (inverted!)
                (Some(css.span.start), Some(script.span.end))
            } else if root.instance.is_some() || root.module.is_some() || root.css.is_some() {
                // Script-only or CSS-only: null/null
                (None, None)
            } else {
                // Empty file (no script, no CSS, no fragment content)
                // Svelte returns null/null for completely empty files
                (None, None)
            }
        }
    };

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
        options: None,
        comments: root
            .comments
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

fn convert_fragment(
    fragment: &internal::Fragment,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::Fragment {
    public::Fragment {
        node_type: "Fragment".to_string(),
        nodes: fragment
            .nodes
            .iter()
            .map(|node| convert_fragment_node(node, source, loc, interner))
            .collect(),
    }
}

fn convert_fragment_node(
    node: &internal::FragmentNode,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::FragmentNode {
    match node {
        internal::FragmentNode::Element(elem) => {
            let converted = convert_element(elem, source, loc, interner);
            // Return appropriate variant based on element kind
            match elem.kind {
                internal::ElementKind::Component => public::FragmentNode::Component(converted),
                internal::ElementKind::Html => public::FragmentNode::RegularElement(converted),
            }
        }
        internal::FragmentNode::ExpressionTag(tag) => {
            public::FragmentNode::ExpressionTag(convert_expression_tag(tag, source, loc, interner))
        }
        internal::FragmentNode::Text(text) => public::FragmentNode::Text(convert_text(text)),
        internal::FragmentNode::Comment(comment) => {
            public::FragmentNode::Comment(convert_comment(comment))
        }
        internal::FragmentNode::IfBlock(block) => {
            public::FragmentNode::IfBlock(convert_if_block(block, source, loc, interner))
        }
        internal::FragmentNode::EachBlock(block) => {
            public::FragmentNode::EachBlock(convert_each_block(block, source, loc, interner))
        }
        internal::FragmentNode::AwaitBlock(block) => {
            public::FragmentNode::AwaitBlock(convert_await_block(block, source, loc, interner))
        }
        internal::FragmentNode::KeyBlock(block) => {
            public::FragmentNode::KeyBlock(convert_key_block(block, source, loc, interner))
        }
    }
}

fn convert_comment(comment: &internal::HtmlComment) -> public::Comment {
    // Note: internal uses `content`, public uses `data` (Svelte's naming)
    public::Comment {
        node_type: "Comment".to_string(),
        start: comment.span.start,
        end: comment.span.end,
        data: comment.content.clone(),
    }
}

fn convert_element(
    elem: &internal::Element,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::Element {
    // Set node_type based on element kind
    let node_type = match elem.kind {
        internal::ElementKind::Component => "Component",
        internal::ElementKind::Html => "RegularElement",
    };

    public::Element {
        node_type: node_type.to_string(),
        start: elem.span.start,
        end: elem.span.end,
        name: interner.must_resolve(elem.name).to_string(),
        kind: elem.kind,
        attributes: elem
            .attributes
            .iter()
            .map(|attr| convert_attribute(attr, source, loc, interner))
            .collect(),
        fragment: convert_fragment(&elem.fragment, source, loc, interner),
    }
}

fn convert_expression_tag(
    tag: &internal::ExpressionTag,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::ExpressionTag {
    // Delegate to tsv_ts for expression conversion
    let ts_expr =
        tsv_ts::ast::convert::convert_expression(&tag.expression, source, loc, interner, 0);

    public::ExpressionTag {
        node_type: "ExpressionTag".to_string(),
        start: tag.span.start,
        end: tag.span.end,
        expression: ts_expr,
    }
}

fn convert_attribute(
    attr: &internal::Attribute,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::Attribute {
    // Extract attribute name from interner
    let name = interner.must_resolve(attr.name).to_string();

    // Convert attribute value following Svelte's JSON format:
    // - Boolean attributes (no value): serialize as `true`
    // - Text attributes (contain Text nodes): serialize as array
    // - Pure expression (single ExpressionTag): serialize as object
    // - Multiple expressions: serialize as array
    let value = match &attr.value {
        None => Some(serde_json::Value::Bool(true)), // Boolean attribute
        Some(values) => {
            // Check if any value is Text (string content)
            let has_text = values
                .iter()
                .any(|v| matches!(v, internal::AttributeValue::Text(_)));

            if has_text {
                // Has text content: always serialize as array (even if single Text value)
                let converted: Vec<_> = values
                    .iter()
                    .map(|v| convert_attribute_value(v, source, loc, interner))
                    .collect();
                Some(to_json_value(&converted))
            } else if values.len() == 1 {
                // Single expression only: serialize as object
                let converted = convert_attribute_value(&values[0], source, loc, interner);
                Some(to_json_value(&converted))
            } else {
                // Multiple expressions: serialize as array
                let converted: Vec<_> = values
                    .iter()
                    .map(|v| convert_attribute_value(v, source, loc, interner))
                    .collect();
                Some(to_json_value(&converted))
            }
        }
    };

    public::Attribute {
        node_type: "Attribute".to_string(),
        start: attr.span.start,
        end: attr.span.end,
        name,
        value,
    }
}

fn convert_attribute_value(
    value: &internal::AttributeValue,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::AttributeValue {
    match value {
        internal::AttributeValue::Text(text) => public::AttributeValue::Text(convert_text(text)),
        internal::AttributeValue::ExpressionTag(tag) => public::AttributeValue::ExpressionTag(
            convert_expression_tag(tag, source, loc, interner),
        ),
    }
}

fn convert_text(text: &internal::Text) -> public::Text {
    // raw contains original source with entities (&lt;, &#65;, etc.)
    // data contains decoded text (<, A, etc.)
    public::Text {
        node_type: "Text".to_string(),
        start: text.span.start,
        end: text.span.end,
        raw: text.raw.clone(),
        data: text.data.clone(),
    }
}

fn convert_script(
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
            .map(|attr| convert_attribute(attr, source, &loc, interner))
            .collect(),
    }
}

fn convert_style(
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
                let public_attr = convert_attribute(attr, source, &full_loc, interner);
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

fn convert_if_block(
    block: &internal::IfBlock,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::IfBlock {
    let ts_expr = tsv_ts::ast::convert::convert_expression(&block.test, source, loc, interner, 0);

    public::IfBlock {
        node_type: "IfBlock".to_string(),
        start: block.span.start,
        end: block.span.end,
        elseif: block.elseif,
        test: ts_expr,
        consequent: convert_fragment(&block.consequent, source, loc, interner),
        alternate: block
            .alternate
            .as_ref()
            .map(|f| convert_fragment(f, source, loc, interner)),
    }
}

fn convert_each_block(
    block: &internal::EachBlock,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::EachBlock {
    let expression =
        tsv_ts::ast::convert::convert_expression(&block.expression, source, loc, interner, 0);
    let context = block
        .context
        .as_ref()
        .map(|c| tsv_ts::ast::convert::convert_expression(c, source, loc, interner, 0));
    let key = block
        .key
        .as_ref()
        .map(|k| tsv_ts::ast::convert::convert_expression(k, source, loc, interner, 0));

    public::EachBlock {
        node_type: "EachBlock".to_string(),
        start: block.span.start,
        end: block.span.end,
        expression,
        context,
        index: block.index.clone(),
        key,
        body: convert_fragment(&block.body, source, loc, interner),
        fallback: block
            .fallback
            .as_ref()
            .map(|f| convert_fragment(f, source, loc, interner)),
    }
}

fn convert_await_block(
    block: &internal::AwaitBlock,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::AwaitBlock {
    let expression =
        tsv_ts::ast::convert::convert_expression(&block.expression, source, loc, interner, 0);
    let value = block
        .value
        .as_ref()
        .map(|v| tsv_ts::ast::convert::convert_expression(v, source, loc, interner, 0));
    let error = block
        .error
        .as_ref()
        .map(|e| tsv_ts::ast::convert::convert_expression(e, source, loc, interner, 0));

    public::AwaitBlock {
        node_type: "AwaitBlock".to_string(),
        start: block.span.start,
        end: block.span.end,
        expression,
        value,
        error,
        pending: block
            .pending
            .as_ref()
            .map(|f| convert_fragment(f, source, loc, interner)),
        then_block: block
            .then
            .as_ref()
            .map(|f| convert_fragment(f, source, loc, interner)),
        catch_block: block
            .catch
            .as_ref()
            .map(|f| convert_fragment(f, source, loc, interner)),
    }
}

fn convert_key_block(
    block: &internal::KeyBlock,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::KeyBlock {
    let expression =
        tsv_ts::ast::convert::convert_expression(&block.expression, source, loc, interner, 0);

    public::KeyBlock {
        node_type: "KeyBlock".to_string(),
        start: block.span.start,
        end: block.span.end,
        expression,
        fragment: convert_fragment(&block.fragment, source, loc, interner),
    }
}
