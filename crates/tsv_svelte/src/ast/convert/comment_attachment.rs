// Comment attachment for converted JSON ASTs
//
// The acorn comment-attachment DFS (`CommentAttachmentContext`,
// `attach_comments_recursively`) attaches leading/trailing comments to script
// Program JSON, matching `add_comments` in
// svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js.
//
// The per-island machinery (`try_attach_comments_to_node` and the `attach_*`
// helpers) is consumed by the wire-JSON writer's skeleton path
// (`ast/convert/special.rs`'s `build_*_writer_comments`): each comment-bearing
// template island is skeletonized to byte-space wire JSON, attached here, and
// read back into a `WriterComments` map the fused writer consults at emit time.

use std::collections::VecDeque;

use tsv_lang::{Comment, printing, source_scan::skip_comment};

/// Context for comment attachment process
///
/// Holds a mutable queue of comments (sorted by position) that gets consumed
/// during the DFS walk, matching acorn's algorithm from:
/// svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js
pub(super) struct CommentAttachmentContext<'a> {
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
pub(super) fn attach_comments_recursively(
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
        // acorn inserts consequent before test in SwitchCase nodes
        // (affects comment attachment: comments between test and colon become
        // leadingComments on the first consequent, not trailingComments on test)
        "SwitchCase" => Some(&["consequent", "test"]),
        // acorn inserts body before label in LabeledStatement nodes
        // (affects comment attachment: comments between label and colon become
        // leadingComments on the body, not trailingComments on the label)
        "LabeledStatement" => Some(&["body", "label"]),
        // acorn inserts key before decorators in class members
        // (affects comment attachment: comments between decorators and the member key
        // become leadingComments on the key, not trailingComments on decorators)
        // typeAnnotation is inserted by acorn-typescript before value, so comments
        // between type annotation and `=` attach as typeAnnotation.trailingComments
        "PropertyDefinition" => Some(&["key", "typeAnnotation", "value", "decorators"]),
        // acorn-typescript sets a method's typeParameters between key and value, so a
        // comment trailing the method type-param `<` walks onto the first type parameter
        // (not the `value` FunctionExpression, whose span starts after the comment).
        "MethodDefinition" => Some(&["key", "typeParameters", "value", "decorators"]),
        // acorn-typescript `parseNew` sets callee, then typeArguments, then arguments,
        // so a comment trailing the type-arg `<` walks onto the first type argument
        // (not the call argument). CallExpression needs no entry: `parseSubscript`
        // keeps arguments before typeArguments, which matches our default Map order.
        "NewExpression" => Some(&["callee", "typeArguments", "arguments"]),
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
///
/// `pub(super)` so the wire-JSON writer emits the `value` field directly (no
/// intermediate `comment_to_json` `Value`).
pub(super) fn get_comment_value(comment: &Comment, source: &str) -> String {
    let content = comment.content(source);
    if comment.is_block && comment.multiline {
        printing::strip_comment_indentation(source, content, comment.span.start)
    } else {
        content.to_string()
    }
}

/// Convert Comment to JSON format (without loc - simplified for attachment)
pub(super) fn comment_to_json(comment: &Comment, source: &str) -> serde_json::Value {
    let comment_type = if comment.is_block { "Block" } else { "Line" };
    let value = get_comment_value(comment, source);

    serde_json::json!({
        "type": comment_type,
        "value": value,
        "start": comment.span.start,
        "end": comment.span.end,
    })
}

/// Whether a comment lies outside every `<script>` content span — i.e., it's a
/// template expression comment that the attachment passes may move into the
/// JSON tree.
pub(super) fn is_template_comment(comment: &Comment, script_spans: &[(u32, u32)]) -> bool {
    !script_spans
        .iter()
        .any(|&(s, e)| comment.span.start >= s && comment.span.end <= e)
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
pub(super) fn try_attach_comments_to_node(
    node_json: &mut serde_json::Value,
    template_comments: &[&Comment],
    source: &str,
    container_start: u32,
    container_end: u32,
) {
    let Some(expr_end) = node_end(node_json) else {
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

    attach_comments_recursively(node_json, &mut ctx);
}

/// Attach comments across an expression list that canonical Svelte parses in
/// ONE acorn parse — snippet parameters (a function-parameter context) and
/// multi-identifier `{@debug}` (a `SequenceExpression`): one shared comment
/// queue walked sequentially through each item, so an inter-item comment lands
/// exactly where acorn's single-parse walk puts it — a same-line `[,) \t]*`
/// gap trails the *preceding* item; anything else leads the *following* item.
///
/// `wrapper_end` is the discarded parse wrapper's `end` for acorn's
/// `node.end == parent.end` trailing suppression: the last identifier's end
/// for `{@debug}`'s `SequenceExpression` (so its last item never claims a
/// trailing comment), `None` for snippet params (the function wrapper ends
/// past every param, so the guard never fires). Leftover comments belonged to
/// the discarded wrapper and stay unattached — they still emit in the root
/// `comments` array. (A single-identifier `{@debug}` has no wrapper — the
/// identifier is the parse root itself — so it takes the
/// `try_attach_comments_to_node` path with its root-fallback trailing, not
/// this one.)
pub(super) fn attach_expression_list(
    items: &mut [serde_json::Value],
    template_comments: &[&Comment],
    source: &str,
    c_start: u32,
    range_end: u32,
    wrapper_end: Option<u32>,
) {
    let comment_queue: VecDeque<serde_json::Value> = template_comments
        .iter()
        .filter(|c| c.span.start >= c_start && c.span.end <= range_end)
        .map(|c| comment_to_json(c, source))
        .collect();
    if comment_queue.is_empty() {
        return;
    }
    let mut ctx = CommentAttachmentContext {
        comments: comment_queue,
        source,
    };
    let parent = wrapper_end.map(|end| ParentInfo {
        end,
        last_body_start: None,
    });
    for item in items.iter_mut() {
        walk_node(item, parent.as_ref(), &mut ctx);
    }
}

/// Attach comments to a `{@const id = init}` declaration `Value`.
///
/// Svelte hand-builds the VariableDeclaration and runs `add_comments(init)` on
/// the **init expression directly**, so comments attach to the init's subtree,
/// not the whole declaration.
pub(super) fn attach_const_tag_declaration(
    decl: &mut serde_json::Value,
    template_comments: &[&Comment],
    source: &str,
    c_start: u32,
    c_end: u32,
) {
    if let Some(declarations) = decl.get_mut("declarations").and_then(|d| d.as_array_mut())
        && let Some(declarator) = declarations.first_mut()
        && let Some(init) = declarator.get_mut("init")
    {
        try_attach_comments_to_node(init, template_comments, source, c_start, c_end);
    }
}

/// Attach comments to a `{const id = init}` / `{let …}` declaration `Value`.
///
/// These are acorn-parsed, so comments attach across the **whole
/// VariableDeclaration tree** (every declarator and its id/init) per acorn's
/// recursive attachment — attaching only to the first init left a comment
/// leading a later declarator (`{let a = 1, /* c */ b}`) unattached.
pub(super) fn attach_declaration_tag_declaration(
    decl: &mut serde_json::Value,
    template_comments: &[&Comment],
    source: &str,
    c_start: u32,
    c_end: u32,
) {
    try_attach_comments_to_node(decl, template_comments, source, c_start, c_end);
}

/// Scan source after an expression's end to find the effective end of comment collection
///
/// Acorn's token scanner reads past whitespace and comments when looking for the next token.
/// This function mimics that: starting at `pos`, skip whitespace and block/line comments,
/// and return the position after the last skipped comment. If no comments are found, returns `pos`.
///
/// `skip_comment` is passed `bytes.len()` (not `limit`) as its bound, and its
/// past-`end` return on an unterminated block comment is unreachable here:
/// this runs only after a successful parse, and every comment in the scanned
/// window was already lexed as terminated. Expression tags track comments in
/// their closing-brace scan (unterminated → no `}` found → parse error);
/// block tags hand their content to the TS parser, whose one-token lookahead
/// lexes all trivia after the expression and hard-errors on an unterminated
/// block comment. This scanner's trivia set (` \t\r\n` + JS comments) is a
/// subset of the lexer's, so it can never walk past that validated region.
fn scan_past_trailing_comments(source: &str, start: u32, limit: u32) -> u32 {
    let bytes = source.as_bytes();
    let mut pos = start as usize;
    let limit = (limit as usize).min(bytes.len());
    let mut last_comment_end = start;

    while pos < limit {
        match bytes[pos] {
            b' ' | b'\t' | b'\r' | b'\n' => {
                pos += 1;
            }
            _ => match skip_comment(bytes, pos, bytes.len()) {
                Some(next) => {
                    pos = next;
                    last_comment_end = pos as u32;
                }
                // Non-whitespace, non-comment — stop scanning
                None => break,
            },
        }
    }

    last_comment_end
}

#[cfg(all(test, feature = "convert"))]
mod tests {
    use serde_json::Value;

    /// Parse a `<script lang="ts">` body and return the public JSON AST.
    fn convert_ts(body: &str) -> Value {
        let source = format!("<script lang=\"ts\">\n{body}\n</script>");
        // Test inputs are hardcoded valid sources; a parse failure should panic
        let arena = bumpalo::Bump::new();
        #[allow(clippy::expect_used)]
        let root = crate::parse(&source, &arena).expect("parse");
        crate::convert_ast_json(&root, &source)
    }

    /// The first statement's expression in the instance `<script>`.
    fn first_expression(ast: &Value) -> &Value {
        &ast["instance"]["content"]["body"][0]["expression"]
    }

    /// The single leading comment value on a node, if any.
    fn leading_comment(node: &Value) -> Option<&str> {
        node.get("leadingComments")?
            .as_array()?
            .first()?
            .get("value")?
            .as_str()
    }

    // For `new Foo< // c\n A, B>(x)`, acorn (`parseNew` sets `callee`,
    // `typeArguments`, then `arguments`) walks the type arguments before the call
    // arguments, so the `<`-trailing line comment attaches as a leadingComment of
    // the FIRST type argument — never the call argument.
    #[test]
    fn new_expression_type_arg_open_angle_comment_attaches_to_first_type_arg() {
        let ast = convert_ts("new Foo< // c\n\tA,\n\tB\n>(x);");
        let expr = first_expression(&ast);
        assert_eq!(expr["type"], "NewExpression");

        let first_type_arg = &expr["typeArguments"]["params"][0];
        assert_eq!(first_type_arg["typeName"]["name"], "A");
        assert_eq!(
            leading_comment(first_type_arg),
            Some(" c"),
            "comment trailing `<` should attach to the first type argument"
        );
        assert_eq!(
            leading_comment(&expr["arguments"][0]),
            None,
            "comment must not land on the call argument for a `new` expression"
        );
    }

    // Sibling parity (already correct): for a CALL expression, acorn (`parseSubscript`
    // sets `callee`, `arguments`, then `typeArguments`) walks the call arguments first,
    // so the same comment attaches to the call ARGUMENT, not the type argument.
    #[test]
    fn call_expression_type_arg_open_angle_comment_attaches_to_call_arg() {
        let ast = convert_ts("foo< // c\n\tA,\n\tB\n>(x);");
        let expr = first_expression(&ast);
        assert_eq!(expr["type"], "CallExpression");

        assert_eq!(leading_comment(&expr["arguments"][0]), Some(" c"));
        assert_eq!(leading_comment(&expr["typeArguments"]["params"][0]), None);
    }

    // For a class method `m< // c\n T>(p) {}`, acorn-typescript sets the
    // MethodDefinition's `key`, then `typeParameters`, then `value`, so the
    // `<`-trailing line comment walks onto the first type PARAMETER — not the
    // method's `value` FunctionExpression (whose span begins after the comment).
    #[test]
    fn class_method_type_param_open_angle_comment_attaches_to_first_type_param() {
        let ast = convert_ts("class C {\n\tm< // c\n\t\tT\n\t>(p: T) {}\n}");
        let method = &ast["instance"]["content"]["body"][0]["body"]["body"][0];
        assert_eq!(method["type"], "MethodDefinition");

        let first_type_param = &method["typeParameters"]["params"][0];
        assert_eq!(first_type_param["name"], "T");
        assert_eq!(
            leading_comment(first_type_param),
            Some(" c"),
            "comment trailing the method type-param `<` should attach to the first type parameter"
        );
        assert_eq!(
            leading_comment(&method["value"]),
            None,
            "comment must not land on the method's FunctionExpression value"
        );
    }

    // Sibling parity (already correct): an interface method is a TSMethodSignature
    // whose `typeParameters` already precede the rest, so the same comment attaches
    // to the first type parameter — confirming the class-method gap is localized to
    // MethodDefinition's child-walk order, not the type-parameter path itself.
    #[test]
    fn interface_method_type_param_open_angle_comment_attaches_to_first_type_param() {
        let ast = convert_ts("interface I {\n\tm< // c\n\t\tT\n\t>(p: T): void;\n}");
        let sig = &ast["instance"]["content"]["body"][0]["body"]["body"][0];
        assert_eq!(sig["type"], "TSMethodSignature");
        assert_eq!(
            leading_comment(&sig["typeParameters"]["params"][0]),
            Some(" c")
        );
    }
}
