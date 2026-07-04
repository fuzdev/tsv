// Comment attachment for converted JSON ASTs
//
// The acorn comment-attachment DFS (`CommentAttachmentContext`,
// `attach_comments_recursively`) attaches leading/trailing comments to script
// Program JSON, matching `add_comments` in
// svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js.
//
// The walk runs over the `SkeletonTree` the writer records during an island's
// byte-space skeleton emit (`ast/convert/special.rs`'s
// `build_*_writer_comments`) — the exact wire node tree, synthetic wrappers
// included — and its assignments feed the span-keyed `WriterComments` map the
// fused writer consults at emit time. No `serde_json::Value` is materialized
// anywhere on this path (the retired emit→`from_slice`→mutate→collect
// round-trip cost O(script wire size) per comment-bearing island).

use std::borrow::Cow;
use std::collections::VecDeque;

use tsv_lang::{Comment, printing, source_scan::skip_comment};
use tsv_ts::ast::convert::{AttachedComment, SkeletonTree, WriterComments};

/// Context for the comment attachment process.
///
/// Holds a mutable queue of comments (sorted by position) that gets consumed
/// during the DFS walk, matching acorn's algorithm from:
/// svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js — plus the
/// per-node assignments the walk produces, in first-touch order.
pub(super) struct CommentAttachmentContext<'a, 's> {
    /// Comment queue sorted by start position. Comments are shifted from the
    /// front as they get attached to nodes during the DFS walk.
    comments: VecDeque<&'a Comment>,
    /// Full source string for slice checks (trailing comment whitespace detection)
    source: &'s str,
    /// Per-node assignments, in first-touch order (which, for two nodes
    /// sharing a span and type, matches their close order — the consume-once
    /// contract of `WriterComments`).
    nodes: Vec<NodeAssignments<'a>>,
}

/// One node's accumulated comment assignments.
struct NodeAssignments<'a> {
    node: u32,
    leading: Vec<&'a Comment>,
    trailing: Vec<&'a Comment>,
    /// The node's first assignment was trailing, so its `trailingComments`
    /// key precedes `leadingComments` in the wire (acorn appends object keys
    /// on first touch — a root that gets fallback trailing comments and then
    /// the preceding-HTML leading comment serializes trailing first).
    trailing_first: bool,
}

impl<'a, 's> CommentAttachmentContext<'a, 's> {
    pub(super) fn new(comments: VecDeque<&'a Comment>, source: &'s str) -> Self {
        Self {
            comments,
            source,
            nodes: Vec::new(),
        }
    }

    /// Record one comment assignment (matching the `Value` walk's
    /// `leadingComments`/`trailingComments` key insertion).
    fn assign(&mut self, node: u32, trailing: bool, comment: &'a Comment) {
        let idx = match self.nodes.iter().position(|n| n.node == node) {
            Some(idx) => idx,
            None => {
                self.nodes.push(NodeAssignments {
                    node,
                    leading: Vec::new(),
                    trailing: Vec::new(),
                    trailing_first: trailing,
                });
                self.nodes.len() - 1
            }
        };
        let entry = &mut self.nodes[idx];
        if trailing {
            entry.trailing.push(comment);
        } else {
            entry.leading.push(comment);
        }
    }

    /// Fold the assignments into a `WriterComments` map. `html_leading`
    /// prepends the preceding-HTML `Line` comment (Svelte's positionless
    /// `{type: "Line", value}`) to the given node's `leadingComments` — in
    /// front of any attached leading comments, after the fact, so a node
    /// whose first attach touch was trailing keeps `trailingComments` first.
    pub(super) fn into_writer_comments(
        self,
        tree: &SkeletonTree,
        html_leading: Option<(u32, &str)>,
        out: &mut WriterComments,
    ) {
        let mut html_leading = html_leading;
        for entry in &self.nodes {
            let mut leading: Vec<AttachedComment> = entry
                .leading
                .iter()
                .map(|c| attached_comment(c, self.source))
                .collect();
            if let Some((node, value)) = html_leading
                && node == entry.node
            {
                leading.insert(0, html_attached_comment(value));
                html_leading = None;
            }
            out.insert_node(
                tree.node_type(entry.node),
                tree.start(entry.node),
                tree.end(entry.node),
                leading,
                entry
                    .trailing
                    .iter()
                    .map(|c| attached_comment(c, self.source))
                    .collect(),
                entry.trailing_first,
            );
        }
        // The HTML comment's node received no attached comments — it still
        // carries the synthetic leading comment.
        if let Some((node, value)) = html_leading {
            out.insert_node(
                tree.node_type(node),
                tree.start(node),
                tree.end(node),
                vec![html_attached_comment(value)],
                Vec::new(),
                false,
            );
        }
    }
}

/// An ordinary attached comment (byte positions; value via `get_comment_value`).
fn attached_comment(comment: &Comment, source: &str) -> AttachedComment {
    AttachedComment {
        is_block: comment.is_block,
        value: get_comment_value(comment, source).into_owned(),
        span: Some((comment.span.start, comment.span.end)),
    }
}

/// The synthetic preceding-HTML `Line` comment (no positions).
fn html_attached_comment(value: &str) -> AttachedComment {
    AttachedComment {
        is_block: false,
        value: value.to_string(),
        span: None,
    }
}

/// Attach comments to all nodes in a skeleton tree using acorn's DFS queue
/// algorithm.
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
    tree: &SkeletonTree,
    root: u32,
    ctx: &mut CommentAttachmentContext<'_, '_>,
) {
    if ctx.comments.is_empty() {
        return;
    }

    // DFS walk with parent tracking
    walk_node(tree, root, None, ctx);

    // Special case: Trailing comments after the root node
    // See acorn.js: "Special case: Trailing comments after the root node"
    if !ctx.comments.is_empty()
        && (ctx.comments[0].span.start >= tree.end(root) || tree.node_type(root) == "Program")
    {
        while let Some(comment) = ctx.comments.pop_front() {
            ctx.assign(root, true, comment);
        }
    }
}

/// Extracted parent context for comment attachment decisions.
///
/// Only what `walk_node` needs from its parent:
/// - `end`: for the `node.end != parent.end` guard
/// - `last_body_start`: start position of the last element in body/elements/properties
///   (None if the parent isn't BlockStatement/Program/ArrayExpression/ObjectExpression,
///   or if the relevant array is empty — or ends in an `ArrayExpression` hole)
struct ParentInfo {
    end: u32,
    last_body_start: Option<u32>,
}

/// Extract parent info from a skeleton node.
///
/// The four container types' body/elements/properties array is each type's
/// only node-valued key, so its last element is the node's last recorded
/// child — except an `ArrayExpression` whose trailing element is a hole
/// (`[a,,]`): the `Value` walk read the array's last entry (`null`, no
/// `start`), which the recorder captures as the `last_elem_hole` flag.
fn extract_parent_info(tree: &SkeletonTree, node: u32) -> ParentInfo {
    let last_body_start = match tree.node_type(node) {
        "BlockStatement" | "Program" | "ObjectExpression" => tree.last_child_start(node),
        "ArrayExpression" => {
            if tree.last_elem_hole(node) {
                None
            } else {
                tree.last_child_start(node)
            }
        }
        _ => None,
    };
    ParentInfo {
        end: tree.end(node),
        last_body_start,
    }
}

/// DFS walk a single AST node, consuming comments from the queue
///
/// This is the core of acorn's `_` handler in the walk.
/// `parent_info` provides extracted parent context for trailing comment decisions.
///
/// Two of the `Value` walk's per-node guards are structural here: the skeleton
/// tree contains no `Block`/`Line` comment objects (a `Record` pass never
/// emits comments), and every recorded node carries `start`/`end`.
fn walk_node(
    tree: &SkeletonTree,
    node: u32,
    parent_info: Option<&ParentInfo>,
    ctx: &mut CommentAttachmentContext<'_, '_>,
) {
    let n_start = tree.start(node);
    let n_end = tree.end(node);

    // --- Leading comments: consume from queue while comment.start < node.start ---
    while ctx
        .comments
        .front()
        .is_some_and(|front| front.span.start < n_start)
    {
        let Some(comment) = ctx.comments.pop_front() else {
            break;
        };
        ctx.assign(node, false, comment);
    }

    // --- Recurse into children (next()) ---
    recurse_children(tree, node, ctx);

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

    let first_comment_start = ctx.comments[0].span.start;

    // Check is_last_in_body: node is last element in parent's body/elements/properties
    // See acorn.js lines 162-168
    let is_last_in_body = parent_info
        .and_then(|p| p.last_body_start)
        .is_some_and(|last_start| last_start == n_start);

    if is_last_in_body {
        // Last node in body: attach multiple trailing comments (can span newlines)
        // Stop at parent boundary
        while let Some(c_start) = ctx.comments.front().map(|c| c.span.start) {
            if let Some(p_end) = parent_end_val
                && c_start >= p_end
            {
                break;
            }
            let Some(comment) = ctx.comments.pop_front() else {
                break;
            };
            ctx.assign(node, true, comment);
        }
    } else if n_end <= first_comment_start {
        // Not last in body: attach at most ONE trailing comment on same line
        // Regex: /^[,) \t]*$/
        let slice = &ctx.source[n_end as usize..first_comment_start as usize];
        if slice.chars().all(|c| matches!(c, ',' | ')' | ' ' | '\t'))
            && let Some(comment) = ctx.comments.pop_front()
        {
            ctx.assign(node, true, comment);
        }
    }
}

/// Recurse into all child nodes of a given node, in acorn's property
/// iteration order (zimmerframe's `for key in node` — JS property insertion
/// order).
///
/// The recorded child order — the wire field order — already *is* acorn's
/// insertion order for every construct the writer emits (the writer's field
/// order reproduces each parser path's assignment order: SwitchCase
/// `consequent` before `test`, LabeledStatement `body` before `label`,
/// MethodDefinition/PropertyDefinition `key` → `typeParameters`/
/// `typeAnnotation` → `value` → `decorators`, NewExpression `callee` →
/// `typeArguments` → `arguments`, CallExpression `arguments` before
/// `typeArguments`), with ONE exception: the generic-async arrow
/// (`async <T>(…) => …`), whose wire order puts `typeParameters` first and
/// `returnType` after `params` — but acorn's arrow paths insert `returnType`
/// before `params`, and `tsTryParseGenericAsyncArrowFunction`'s
/// `typeParameters` walk last (the plain-arrow `<T>(…)` graft already emits
/// last, needing no reorder). So the walk visits
/// `[returnType?, params…, body, typeParameters]` for that shape.
fn recurse_children(tree: &SkeletonTree, node: u32, ctx: &mut CommentAttachmentContext<'_, '_>) {
    // Extract parent info BEFORE walking children (the Value walk computed it
    // before mutating the node).
    let parent_info = extract_parent_info(tree, node);

    let generic_async_arrow = tree.node_type(node) == "ArrowFunctionExpression"
        && tree
            .children(node)
            .next()
            .is_some_and(|first| tree.node_type(first) == "TSTypeParameterDeclaration");

    if generic_async_arrow {
        // Wire order [typeParameters, params…, returnType?, body] → visit
        // order [returnType?, params…, body, typeParameters]. The returnType
        // is the arrow's unique direct TSTypeAnnotation child (a param's own
        // annotation nests inside the param).
        let children: Vec<u32> = tree.children(node).collect();
        let return_type = children[1..]
            .iter()
            .copied()
            .find(|&c| tree.node_type(c) == "TSTypeAnnotation");
        if let Some(rt) = return_type {
            walk_node(tree, rt, Some(&parent_info), ctx);
        }
        for &child in &children[1..] {
            if Some(child) != return_type {
                walk_node(tree, child, Some(&parent_info), ctx);
            }
        }
        walk_node(tree, children[0], Some(&parent_info), ctx);
    } else {
        for child in tree.children(node) {
            walk_node(tree, child, Some(&parent_info), ctx);
        }
    }
}

/// Get comment value with indentation stripping applied (Svelte compatibility)
///
/// For multi-line block comments, strips leading indentation to match Svelte's behavior.
/// See: svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js:115-124
///
/// `pub(super)` so the wire-JSON writer emits the `value` field directly (no
/// intermediate allocation).
///
/// Returns `Cow` so the common single-line / non-dedented case borrows its
/// content slice verbatim — only the multi-line block dedent path (rare)
/// allocates.
pub(super) fn get_comment_value<'s>(comment: &Comment, source: &'s str) -> Cow<'s, str> {
    let content = comment.content(source);
    if comment.is_block && comment.multiline {
        Cow::Owned(printing::strip_comment_indentation(
            source,
            content,
            comment.span.start,
        ))
    } else {
        Cow::Borrowed(content)
    }
}

/// Whether a comment lies outside every `<script>` content span — i.e., it's a
/// template expression comment that the attachment passes may move into the
/// JSON tree.
pub(super) fn is_template_comment(comment: &Comment, script_spans: &[(u32, u32)]) -> bool {
    !script_spans
        .iter()
        .any(|&(s, e)| comment.span.start >= s && comment.span.end <= e)
}

/// The template comments inside an attach window `[start, end]`, as the
/// position-ordered queue the DFS consumes.
fn window_queue<'a>(
    template_comments: &[&'a Comment],
    start: u32,
    end: u32,
) -> VecDeque<&'a Comment> {
    template_comments
        .iter()
        .copied()
        .filter(|c| c.span.start >= start && c.span.end <= end)
        .collect()
}

/// Try to attach comments to a template expression skeleton, folding the
/// assignments into `out`.
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
    tree: &SkeletonTree,
    root: u32,
    template_comments: &[&Comment],
    source: &str,
    container_start: u32,
    container_end: u32,
    out: &mut WriterComments,
) {
    let expr_end = tree.end(root);

    // Compute the effective end of the expression's parsing window.
    // Acorn scans ahead after the expression looking for the next token,
    // encountering (and collecting) any comments along the way.
    // We scan source from expr.end, skipping whitespace and comments,
    // to find where acorn would stop.
    let effective_end = scan_past_trailing_comments(source, expr_end, container_end);

    // Filter comments within [container_start, effective_end)
    let comment_queue = window_queue(template_comments, container_start, effective_end);
    if comment_queue.is_empty() {
        return;
    }

    let mut ctx = CommentAttachmentContext::new(comment_queue, source);
    attach_comments_recursively(tree, root, &mut ctx);
    ctx.into_writer_comments(tree, None, out);
}

/// Attach comments across an expression list that canonical Svelte parses in
/// ONE acorn parse — snippet parameters (a function-parameter context) and
/// multi-identifier `{@debug}` (a `SequenceExpression`): one shared comment
/// queue walked sequentially through each item (the tree's roots), so an
/// inter-item comment lands exactly where acorn's single-parse walk puts it —
/// a same-line `[,) \t]*` gap trails the *preceding* item; anything else leads
/// the *following* item.
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
    tree: &SkeletonTree,
    template_comments: &[&Comment],
    source: &str,
    c_start: u32,
    range_end: u32,
    wrapper_end: Option<u32>,
    out: &mut WriterComments,
) {
    let comment_queue = window_queue(template_comments, c_start, range_end);
    if comment_queue.is_empty() {
        return;
    }
    let mut ctx = CommentAttachmentContext::new(comment_queue, source);
    let parent = wrapper_end.map(|end| ParentInfo {
        end,
        last_body_start: None,
    });
    for &root in tree.roots() {
        walk_node(tree, root, parent.as_ref(), &mut ctx);
    }
    ctx.into_writer_comments(tree, None, out);
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
