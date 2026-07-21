//! Which host comments carry into the synthetic program, and which classes
//! refuse.
//!
//! Server-specific: every rule here is reasoning about the *oracle's printer*
//! (esrap) — where its single `comment_index` sits when a given block opens, and
//! which synthetic span windows a carried comment would fall into. A client
//! transform emits different blocks and would need its own answer, so this is
//! deliberately not filed with the target-independent script analyses.
//!
//! See [`crate::transform_server`] for the orchestration that calls this, and
//! `../../docs/checklist_svelte_compiler.md` for the probed boundaries.

use tsv_svelte::ast::internal::{ElementKind, FragmentNode, Root};
use tsv_ts::ast::internal::{
    ArrowFunctionBody, ClassBody, ClassMember, ExportDefaultValue, Expression, ForInOfLeft,
    ForInit, ObjectPatternProperty, ObjectProperty, Statement, VariableDeclaration,
};

use crate::analyze::{is_effect_call, is_inspect_call};
use crate::transform_server::unsupported;
use crate::{CompileError, Refusal};

/// Collect the comments carried into the synthetic program: exactly the host
/// comments inside the instance script's content span. Classes that can't
/// converge refuse:
///
/// - comments outside the script (template-expression comments) — the emitters
///   don't thread them yet;
/// - a fragment node *before* the script end (template-before-script) — the
///   `$.escape`/`$.html` wrapper windows would sweep script comments;
/// - format-ignore directives — they'd switch the printer to raw-source
///   emission of synthetic spans.
///
/// Comments inside the **module** script's content span are NOT handled here —
/// this function neither carries nor refuses them (it simply skips them), leaving
/// the whole module-comment class to [`collect_module_script_comments`], with one
/// exception: with the module script placed **second** the oracle re-attaches its
/// comment into a template expression (esrap's index re-seeks BACKWARD onto a
/// comment that FOLLOWS the instance script), which tsv can't reproduce, so that
/// ordering refuses here ([`Refusal::ModuleCommentAfterInstanceScript`]). Because
/// this runs before [`collect_module_script_comments`], that refusal is the reason
/// the latter only ever sees the module-first ordering.
pub(crate) fn collect_script_comments(
    root: &Root<'_>,
    source: &str,
    instance_body: &[Statement<'_>],
) -> Result<Vec<tsv_lang::Comment>, CompileError> {
    if root.comments.is_empty() {
        return Ok(Vec::new());
    }
    // Module-script comments are handled by `collect_module_script_comments`, not
    // here: this loop only refuses the module-AFTER-instance ordering (below) and
    // otherwise SKIPS them. A comment the oracle keeps (recovered by a preceding
    // block) carries there; the rest drop.
    let module_content = root.module.map(|module| module.content.span);
    let in_module = |comment: &tsv_lang::Comment| {
        module_content.is_some_and(|m| comment.span.start >= m.start && comment.span.end <= m.end)
    };
    let Some(script) = root.instance else {
        // No instance script to carry into: any comment that is not a (dropped)
        // module comment is a template comment we don't thread — refuse.
        for comment in &root.comments {
            if !in_module(comment) {
                return Err(unsupported(Refusal::TemplateComments));
            }
        }
        return Ok(Vec::new());
    };
    let content = script.content.span;
    // Source order of the two scripts — the whole trigger for
    // [`Refusal::ModuleCommentAfterInstanceScript`] below. The tags cannot nest,
    // so comparing content starts is a total order.
    let module_after_instance = module_content.is_some_and(|m| m.start > content.start);
    // A comment at or past the last SURVIVING statement has no statement left to
    // lead — an `import` hoists to the comment-free module program and a
    // statement-position `$effect`/`$inspect` drops, so neither anchors one. The
    // bound is the last surviving statement's end, `content.start` when nothing
    // survives (an import-only script).
    //
    // Such a comment still carries: the oracle re-attaches it into the template
    // (trailing the final push, or nested inside the next emitted node — an
    // `{#if}` condition, an `$.ensure_array_like(…)` / `$.attr(…)` argument) while
    // tsv's printer lands it at the end of the synthetic function body (the body
    // block's span runs `[content.start, rbrace_end)`, so the block's trailing
    // window captures it exactly once). The placements differ, but the parity bar
    // grades comment DROP / COUNT / CONTENT, not position.
    //
    // The one shape that does NOT converge is a template emitting a nested block —
    // see [`template_emits_nested_block`].
    let survives = |stmt: &Statement<'_>| match stmt {
        Statement::ImportDeclaration(_) => false,
        Statement::ExpressionStatement(expr_stmt) => {
            is_effect_call(&expr_stmt.expression, source).is_none()
                && is_inspect_call(&expr_stmt.expression, source).is_none()
        }
        _ => true,
    };
    let last_stmt_end = instance_body
        .iter()
        .filter(|stmt| survives(stmt))
        .map(|stmt| stmt.span().end)
        .max()
        .unwrap_or(content.start);
    let nested_block = template_emits_nested_block(root.fragment.nodes);
    // A leading comment glued to the `<script>` line (no newline before it) shares
    // its source line with the function's synthetic opening brace, so the printer
    // trails it after the `{` instead of onto its own line — refuse the class
    // (prettier-formatted input always puts a leading comment on its own line, so
    // the covered fixtures are unaffected).
    let first_stmt_start = instance_body
        .first()
        .map_or(content.end, |stmt| stmt.span().start);
    let mut comments = Vec::with_capacity(root.comments.len());
    for comment in &root.comments {
        // A module-script comment drops — but ONLY when the module script comes
        // FIRST. The oracle's drop is not a rule about module scripts; it is
        // where esrap's single comment index happens to be. The component body
        // block carries the instance script's `loc`, and opening it re-seeks that
        // index ABSOLUTELY — forward past a comment that precedes the instance
        // script (the drop tsv reproduces), but BACKWARD onto one that follows it.
        // A recovered comment is then flushed into the next loc-bearing node the
        // printer reaches, which is a template expression it has nothing to do
        // with. tsv drops it either way, so the second ordering is a comment
        // PRESENCE difference — a mismatch, not a tolerated position one.
        if in_module(comment) {
            if module_after_instance {
                return Err(unsupported(Refusal::ModuleCommentAfterInstanceScript));
            }
            continue;
        }
        if comment.span.start < content.start || comment.span.end > content.end {
            return Err(unsupported(Refusal::TemplateComments));
        }
        // A multi-line block comment carries verbatim, but the oracle (esrap)
        // re-indents its interior lines to the emit position, so the two diverge on
        // any interior line whose source indentation differs from the target — refuse
        // until the printer re-indents block-comment interiors to match. Checked
        // before the after-last rule below so this independent gate keeps its own
        // refusal bucket whatever the template emits.
        if comment.multiline {
            return Err(unsupported(Refusal::MultilineBlockComment));
        }
        if nested_block && comment.span.start >= last_stmt_end {
            return Err(unsupported(Refusal::CommentAfterLastStatementWithBlock));
        }
        if comment.span.end <= first_stmt_start {
            let gap = &source[content.start as usize..comment.span.start as usize];
            if !gap.contains('\n') {
                return Err(unsupported(Refusal::LeadingCommentGluedToScript));
            }
        }
        let text = comment.content(source);
        if text.contains("prettier-ignore") || text.contains("format-ignore") {
            return Err(unsupported(Refusal::FormatIgnoreComment));
        }
        let mut comment = comment.clone();
        // Release a JSDoc cast's comment back to the positional machinery. `tsv_ts`
        // binds it to its `JsdocCast` node (`Comment::owned_by_node`) so a synthesized
        // paren can't land between the comment and the `(` it glues to — the owning
        // node becomes the only thing that prints it, and the range lookups skip it.
        // Erasure unwraps *every* `JsdocCast` (the compile path matches the oracle,
        // which has no such node and drops the parens), so in the emitted program that
        // owner does not exist: left owned, the comment is printed by nothing and
        // silently dropped. Un-owned, it prints from its gap exactly as the oracle
        // prints it — `const x = /** @type {number} */ 1`.
        comment.owned_by_node = false;
        comments.push(comment);
    }
    for node in root.fragment.nodes {
        // A whitespace-only text node — e.g. the run between a module `</script>`
        // and the instance `<script>`, or leading/trailing template whitespace —
        // is not real markup, so it doesn't force the refusal. Any genuine
        // element / expression / comment / block before the instance script's end
        // still refuses (its emitter's comment window would sweep the carried
        // script comments). A Unicode-whitespace-only text (`is_ascii_ws_only ==
        // false`) is content and correctly still refuses.
        if let FragmentNode::Text(text) = node
            && text.is_ascii_ws_only
        {
            continue;
        }
        if node.span().start < content.end {
            return Err(unsupported(Refusal::CommentsWithTemplateBeforeScript));
        }
    }
    Ok(comments)
}

/// Which `<script module>` comments carry into the module-scope program. The
/// oracle DROPS most of them, but its printer (esrap) KEEPS one whose comment
/// index it re-seeks BACKWARD over a preceding block-bearing statement — the "open
/// half" of the module-comment class. The rule, established by probe against the
/// pinned oracle (esrap 2.2.12), is bidirectionally exact — keeping a comment the
/// oracle drops over-emits, dropping one it keeps under-emits, both MISMATCHES with
/// no safe direction — so it is stated precisely:
///
/// A module comment `C` (physically inside the module content span) is **KEPT** iff
/// BOTH:
///
/// 1. **A block precedes it** — some `BlockStatement` / `ClassBody` / class static
///    block in the module body starts at a source position `< C.span.start`
///    ([`module_min_block_start`]). Those are exactly the nodes esrap opens with a
///    `loc`-bearing `reset_comment_index`; a block whose `{` sits AFTER the comment
///    (a comment in a param list, before the body) does NOT count, so the anchor is
///    the BLOCK's start, not its statement's. A `switch` has no `BlockStatement`
///    node and does not trigger; an `ObjectExpression`/`ArrayExpression` `{}`/`[]`
///    is not a block.
/// 2. **A flush target exists** — either some NON-empty module statement extends
///    PAST the comment (`span.end > C.span.end`: a later statement, or the enclosing
///    block's `}` for a comment sitting inside a block), OR an instance `<script>`
///    is present. The exported component function is NOT a flush target (a loc-less
///    reset discards the module comments before it prints), which is why a comment
///    after the last module statement drops without an instance script. A DROPPED
///    TypeScript statement (an `interface`/`type`, an `EmptyStatement` post-erase)
///    is not a flush target either.
///
/// Otherwise `C` DROPS. A kept `C` carries at its authored host span
/// (`format_canonical` places it by binary search); the oracle may re-attach it
/// into the component signature instead, a placement the parity bar tolerates.
///
/// A kept comment whose reprint would DIVERGE refuses instead (safe — a gap, not a
/// mismatch), mirroring [`collect_script_comments`]'s instance-side rules: a
/// multi-line block comment (esrap re-indents its interior lines), a comment
/// intersecting an erased TypeScript region (the oracle's surviving placement there
/// is an emergent stale-span artifact), and a format-ignore directive (would switch
/// the printer to raw-source emission).
///
/// ⚠️ Keyed to the pinned oracle's `reset_comment_index` behavior — re-probe the
/// keep condition if that pin moves. Runs AFTER [`collect_script_comments`], which
/// refuses the module-after-instance ordering, so this only sees module-first.
pub(crate) fn collect_module_script_comments(
    root: &Root<'_>,
    source: &str,
    module_body: &[Statement<'_>],
    module_erased_windows: &[tsv_lang::Span],
) -> Result<Vec<tsv_lang::Comment>, CompileError> {
    let Some(module) = root.module else {
        return Ok(Vec::new());
    };
    if root.comments.is_empty() {
        return Ok(Vec::new());
    }
    let module_content = module.content.span;
    // The module-AFTER-instance ordering is refused upstream in
    // `collect_script_comments`, which runs first — so this is a no-op guard, kept
    // so the keep set is never computed for that ordering if the call order changes.
    if let Some(instance) = root.instance
        && instance.content.span.start < module_content.start
    {
        return Ok(Vec::new());
    }
    let has_instance = root.instance.is_some();
    // Condition 1's substrate: the earliest block esrap can re-seek the index on.
    let min_block = module_min_block_start(module_body);
    let mut kept = Vec::new();
    for comment in &root.comments {
        // Only comments physically inside the module `<script>` content.
        if comment.span.start < module_content.start || comment.span.end > module_content.end {
            continue;
        }
        // Condition 1: a block starts before the comment.
        let block_before = min_block.is_some_and(|block_start| block_start < comment.span.start);
        // Condition 2: a flush target — a non-empty module statement extending past
        // the comment, or an instance script.
        let has_flush = has_instance
            || module_body.iter().any(|stmt| {
                !matches!(stmt, Statement::EmptyStatement(_)) && stmt.span().end > comment.span.end
            });
        if !block_before || !has_flush {
            continue;
        }
        // The oracle KEEPS `comment`. Refuse the reprint-divergent classes.
        if comment.multiline {
            return Err(unsupported(Refusal::MultilineBlockComment));
        }
        for window in module_erased_windows {
            if comment.span.start < window.end && comment.span.end > window.start {
                return Err(unsupported(Refusal::CommentInErasedTypeRegion));
            }
        }
        let text = comment.content(source);
        if text.contains("prettier-ignore") || text.contains("format-ignore") {
            return Err(unsupported(Refusal::FormatIgnoreComment));
        }
        let mut comment = comment.clone();
        // Release JSDoc-cast ownership so the positional machinery prints it — its
        // owning `JsdocCast` node is erased in the emitted program (see the same
        // step in `collect_script_comments`).
        comment.owned_by_node = false;
        kept.push(comment);
    }
    Ok(kept)
}

/// The earliest source position of a `BlockStatement`, `ClassBody`, or class
/// static block anywhere in the module body — the nodes esrap opens with a
/// `loc`-bearing `reset_comment_index` (the module comment's condition-1 anchor
/// above). `None` when the module holds no such block.
///
/// The walk only ever notes a genuine block node, so it can never OVER-report (keep
/// a comment the oracle drops); a missed descent can only UNDER-report (leave a
/// mismatch unclosed), never regress. Exhaustively matched so a new AST variant
/// fails compilation here rather than silently changing that balance.
fn module_min_block_start(statements: &[Statement<'_>]) -> Option<u32> {
    let mut min = None;
    block_min_stmts(statements, &mut min);
    min
}

fn note_block(min: &mut Option<u32>, start: u32) {
    match min {
        Some(current) => *current = (*current).min(start),
        None => *min = Some(start),
    }
}

fn block_min_stmts(statements: &[Statement<'_>], min: &mut Option<u32>) {
    for stmt in statements {
        block_min_stmt(stmt, min);
    }
}

fn block_min_stmt(stmt: &Statement<'_>, min: &mut Option<u32>) {
    match stmt {
        Statement::BlockStatement(block) => {
            note_block(min, block.span.start);
            block_min_stmts(block.body, min);
        }
        Statement::FunctionDeclaration(f) => {
            for param in f.params {
                block_min_expr(param, min);
            }
            note_block(min, f.body.span.start);
            block_min_stmts(f.body.body, min);
        }
        Statement::ClassDeclaration(c) => block_min_class_body(&c.body, min),
        Statement::ExpressionStatement(s) => block_min_expr(&s.expression, min),
        Statement::VariableDeclaration(d) => block_min_var_decl(d, min),
        Statement::ReturnStatement(s) => {
            if let Some(arg) = s.argument.as_ref() {
                block_min_expr(arg, min);
            }
        }
        Statement::ThrowStatement(s) => block_min_expr(&s.argument, min),
        Statement::IfStatement(s) => {
            block_min_expr(&s.test, min);
            block_min_stmt(s.consequent, min);
            if let Some(alt) = s.alternate {
                block_min_stmt(alt, min);
            }
        }
        Statement::ForStatement(s) => {
            match &s.init {
                Some(ForInit::VariableDeclaration(d)) => block_min_var_decl(d, min),
                Some(ForInit::Expression(e)) => block_min_expr(e, min),
                None => {}
            }
            if let Some(test) = s.test.as_ref() {
                block_min_expr(test, min);
            }
            if let Some(update) = s.update.as_ref() {
                block_min_expr(update, min);
            }
            block_min_stmt(s.body, min);
        }
        Statement::ForInStatement(s) => {
            block_min_for_left(&s.left, min);
            block_min_expr(&s.right, min);
            block_min_stmt(s.body, min);
        }
        Statement::ForOfStatement(s) => {
            block_min_for_left(&s.left, min);
            block_min_expr(&s.right, min);
            block_min_stmt(s.body, min);
        }
        Statement::WhileStatement(s) => {
            block_min_expr(&s.test, min);
            block_min_stmt(s.body, min);
        }
        Statement::DoWhileStatement(s) => {
            block_min_stmt(s.body, min);
            block_min_expr(&s.test, min);
        }
        Statement::SwitchStatement(s) => {
            block_min_expr(&s.discriminant, min);
            for case in s.cases {
                if let Some(test) = case.test.as_ref() {
                    block_min_expr(test, min);
                }
                block_min_stmts(case.consequent, min);
            }
        }
        Statement::TryStatement(s) => {
            note_block(min, s.block.span.start);
            block_min_stmts(s.block.body, min);
            if let Some(handler) = &s.handler {
                if let Some(param) = &handler.param {
                    block_min_expr(param, min);
                }
                note_block(min, handler.body.span.start);
                block_min_stmts(handler.body.body, min);
            }
            if let Some(finalizer) = &s.finalizer {
                note_block(min, finalizer.span.start);
                block_min_stmts(finalizer.body, min);
            }
        }
        Statement::LabeledStatement(s) => block_min_stmt(s.body, min),
        Statement::ExportNamedDeclaration(s) => {
            if let Some(decl) = &s.declaration {
                block_min_stmt(decl, min);
            }
        }
        Statement::ExportDefaultDeclaration(s) => match &s.declaration {
            ExportDefaultValue::Expression(e) => block_min_expr(e, min),
            ExportDefaultValue::FunctionDeclaration(f) => {
                for param in f.params {
                    block_min_expr(param, min);
                }
                note_block(min, f.body.span.start);
                block_min_stmts(f.body.body, min);
            }
            ExportDefaultValue::ClassDeclaration(c) => block_min_class_body(&c.body, min),
            ExportDefaultValue::TSDeclareFunction(_)
            | ExportDefaultValue::TSInterfaceDeclaration(_) => {}
        },
        Statement::TSExportAssignment(s) => block_min_expr(&s.expression, min),
        // No block-bearing children (or a TypeScript-only statement dropped before
        // this runs).
        Statement::BreakStatement(_)
        | Statement::ContinueStatement(_)
        | Statement::EmptyStatement(_)
        | Statement::DebuggerStatement(_)
        | Statement::ImportDeclaration(_)
        | Statement::ExportAllDeclaration(_)
        | Statement::TSNamespaceExportDeclaration(_)
        | Statement::TSImportEqualsDeclaration(_)
        | Statement::TSTypeAliasDeclaration(_)
        | Statement::TSInterfaceDeclaration(_)
        | Statement::TSDeclareFunction(_)
        | Statement::TSEnumDeclaration(_)
        | Statement::TSModuleDeclaration(_) => {}
    }
}

fn block_min_var_decl(decl: &VariableDeclaration<'_>, min: &mut Option<u32>) {
    for declarator in decl.declarations {
        block_min_expr(&declarator.id, min);
        if let Some(init) = declarator.init.as_ref() {
            block_min_expr(init, min);
        }
    }
}

fn block_min_for_left(left: &ForInOfLeft<'_>, min: &mut Option<u32>) {
    match left {
        ForInOfLeft::VariableDeclaration(d) => block_min_var_decl(d, min),
        ForInOfLeft::Pattern(p) => block_min_expr(p, min),
    }
}

fn block_min_class_body(body: &ClassBody<'_>, min: &mut Option<u32>) {
    note_block(min, body.span.start);
    for member in body.body {
        match member {
            ClassMember::MethodDefinition(m) => {
                if m.computed {
                    block_min_expr(&m.key, min);
                }
                for param in m.value.params {
                    block_min_expr(param, min);
                }
                note_block(min, m.value.body.span.start);
                block_min_stmts(m.value.body.body, min);
            }
            ClassMember::PropertyDefinition(p) => {
                if p.computed {
                    block_min_expr(&p.key, min);
                }
                if let Some(value) = p.value.as_ref() {
                    block_min_expr(value, min);
                }
            }
            ClassMember::StaticBlock(b) => {
                note_block(min, b.span.start);
                block_min_stmts(b.body, min);
            }
            ClassMember::IndexSignature(_) => {}
        }
    }
}

fn block_min_exprs(exprs: &[Expression<'_>], min: &mut Option<u32>) {
    for expr in exprs {
        block_min_expr(expr, min);
    }
}

fn block_min_expr(expr: &Expression<'_>, min: &mut Option<u32>) {
    match expr {
        Expression::ArrowFunctionExpression(a) => {
            for param in a.params {
                block_min_expr(param, min);
            }
            match &a.body {
                ArrowFunctionBody::Expression(e) => block_min_expr(e, min),
                ArrowFunctionBody::BlockStatement(b) => {
                    note_block(min, b.span.start);
                    block_min_stmts(b.body, min);
                }
            }
        }
        Expression::FunctionExpression(f) => {
            for param in f.params {
                block_min_expr(param, min);
            }
            note_block(min, f.body.span.start);
            block_min_stmts(f.body.body, min);
        }
        Expression::ClassExpression(c) => block_min_class_body(&c.body, min),
        Expression::NewExpression(e) => {
            block_min_expr(e.callee, min);
            block_min_exprs(e.arguments, min);
        }
        Expression::CallExpression(e) => {
            block_min_expr(e.callee, min);
            block_min_exprs(e.arguments, min);
        }
        Expression::MemberExpression(e) => {
            block_min_expr(e.object, min);
            if e.computed {
                block_min_expr(e.property, min);
            }
        }
        Expression::ObjectExpression(obj) => {
            for prop in obj.properties {
                match prop {
                    ObjectProperty::Property(p) => {
                        if p.computed {
                            block_min_expr(&p.key, min);
                        }
                        block_min_expr(&p.value, min);
                    }
                    ObjectProperty::SpreadElement(s) => block_min_expr(s.argument, min),
                }
            }
        }
        Expression::ArrayExpression(arr) => {
            for element in arr.elements {
                if let Some(e) = element.as_ref() {
                    block_min_expr(e, min);
                }
            }
        }
        Expression::UnaryExpression(u) => block_min_expr(u.argument, min),
        Expression::UpdateExpression(u) => block_min_expr(u.argument, min),
        Expression::BinaryExpression(b) => {
            block_min_expr(b.left, min);
            block_min_expr(b.right, min);
        }
        Expression::ConditionalExpression(c) => {
            block_min_expr(c.test, min);
            block_min_expr(c.consequent, min);
            block_min_expr(c.alternate, min);
        }
        Expression::SpreadElement(s) => block_min_expr(s.argument, min),
        Expression::TemplateLiteral(t) => block_min_exprs(t.expressions, min),
        Expression::TaggedTemplateExpression(t) => {
            block_min_expr(t.tag, min);
            block_min_exprs(t.quasi.expressions, min);
        }
        Expression::AwaitExpression(a) => block_min_expr(a.argument, min),
        Expression::YieldExpression(y) => {
            if let Some(arg) = y.argument {
                block_min_expr(arg, min);
            }
        }
        Expression::SequenceExpression(s) => block_min_exprs(s.expressions, min),
        Expression::AssignmentExpression(a) => {
            block_min_expr(a.left, min);
            block_min_expr(a.right, min);
        }
        Expression::ObjectPattern(p) => {
            for prop in p.properties {
                match prop {
                    ObjectPatternProperty::Property(prop) => {
                        if prop.computed {
                            block_min_expr(&prop.key, min);
                        }
                        block_min_expr(&prop.value, min);
                    }
                    ObjectPatternProperty::RestElement(rest) => block_min_expr(rest.argument, min),
                }
            }
        }
        Expression::ArrayPattern(p) => {
            for element in p.elements {
                if let Some(e) = element.as_ref() {
                    block_min_expr(e, min);
                }
            }
        }
        Expression::AssignmentPattern(p) => {
            block_min_expr(p.left, min);
            block_min_expr(p.right, min);
        }
        Expression::RestElement(r) => block_min_expr(r.argument, min),
        Expression::TSTypeAssertion(t) => block_min_expr(t.expression, min),
        Expression::TSAsExpression(t) => block_min_expr(t.expression, min),
        Expression::TSSatisfiesExpression(t) => block_min_expr(t.expression, min),
        Expression::TSInstantiationExpression(t) => block_min_expr(t.expression, min),
        Expression::TSNonNullExpression(t) => block_min_expr(t.expression, min),
        Expression::TSParameterProperty(t) => block_min_expr(t.parameter, min),
        Expression::ImportExpression(i) => {
            block_min_expr(i.source, min);
            if let Some(options) = i.options {
                block_min_expr(options, min);
            }
        }
        Expression::JsdocCast(j) => block_min_expr(j.inner, min),
        Expression::ParenthesizedExpression(p) => block_min_expr(p.expression, min),
        // Leaves — no children, no blocks.
        Expression::Literal(_)
        | Expression::Identifier(_)
        | Expression::PrivateIdentifier(_)
        | Expression::RegexLiteral(_)
        | Expression::ThisExpression(_)
        | Expression::Super(_)
        | Expression::MetaProperty(_) => {}
    }
}

/// Does the template emit a **synthetic block** — a `{ … }` body the oracle
/// builds with no source `loc`?
///
/// This decides whether a comment past the last surviving script statement can be
/// carried. The oracle's printer (esrap) walks one `comment_index` over the comment
/// list, and `body()` opens every block with `reset_comment_index(node)`. That reset
/// has two arms: a block with **no** `loc` sets the index to `comments.length`,
/// **discarding every comment not yet written**; a block that **has** a `loc`
/// re-seeks the index absolutely (`comments.findIndex(…)`), which can move it
/// **backward**. So a loc-less block annihilates the index and the next loc-bearing
/// block **recovers** it.
///
/// That recovery — not an exemption — is what carries an after-last comment through
/// to the component body. The body block is assigned the instance script's `loc`
/// (the transform's "trick esrap into including comments" line), and when the
/// component needs a context wrapper the transform **reassigns** `component_block`
/// to a fresh loc-LESS block around that loc-bearing one. The wrapper does annihilate
/// the index; the inner block then seeks back over the comment, so it still reaches
/// the body's closing flush. A template block gets no such recovery — it is loc-less,
/// reached after the body has already seeked, with nothing loc-bearing behind it to
/// seek back — so the comment is DROPPED, a divergence the parity bar grades, unlike
/// a mere position difference.
///
/// The scan is deliberately blunt: it answers "does a synthetic block exist
/// anywhere", not "is one reached before the comment would flush". A loc-bearing
/// expression emitted first (an `{#if}` test, an `{#each}` expression) flushes the
/// comment ahead of the block and the oracle keeps it — so `{#if x}` with an
/// after-last comment converges in practice, and this scan over-refuses it.
/// Tightening that costs an ordered next-emitted-node walk plus the oracle's
/// fold/rewrite rules for which expressions keep a `loc`; a safe over-refusal is
/// preferred to guessing.
///
/// The [`FragmentNode::SpecialElement`] arm is intentionally blanket-TRUE for the
/// same reason. Several kinds do emit a block and genuinely drop the comment
/// (`<svelte:head>`, `<svelte:element>`, `<svelte:boundary>`), but `<svelte:window>`
/// and `<slot>` emit no block at all and are knowingly over-refused — the blanket arm
/// buys a conservative safety margin, not a claim that every kind drops.
///
/// ⚠️ This TRUE/FALSE split is keyed to the **pinned** oracle's `reset_comment_index`
/// behavior (esrap 2.2.12, via the pinned Svelte compiler). If that pin moves, re-probe
/// the split against the new oracle rather than assuming it carries over.
///
/// Exhaustively matched so a new [`FragmentNode`] variant fails compilation here
/// rather than silently defaulting to "no block".
fn template_emits_nested_block(nodes: &[FragmentNode<'_>]) -> bool {
    nodes.iter().any(|node| match node {
        // Leaves and the tags that emit a bare call — no block.
        FragmentNode::Text(_)
        | FragmentNode::Comment(_)
        | FragmentNode::ExpressionTag(_)
        | FragmentNode::HtmlTag(_)
        | FragmentNode::RenderTag(_) => false,
        // Every block/closure emitter: `{#if}`/`{#each}` bodies, the `$.await` and
        // `$.head`/`$.element` closures, a `{#snippet}` function.
        FragmentNode::IfBlock(_)
        | FragmentNode::EachBlock(_)
        | FragmentNode::AwaitBlock(_)
        | FragmentNode::KeyBlock(_)
        | FragmentNode::SnippetBlock(_)
        | FragmentNode::SpecialElement(_)
        // Refused elsewhere; counted here so the scan never under-reports.
        | FragmentNode::ConstTag(_)
        | FragmentNode::DeclarationTag(_)
        | FragmentNode::DebugTag(_) => true,
        FragmentNode::Element(element) => match element.kind {
            // A component's children become a `children: ($$renderer) => { … }`
            // snippet prop — a block. Childless (or whitespace-only), it is a bare
            // `Foo($$renderer, {…})` call.
            ElementKind::Component => {
                element.fragment.nodes.iter().any(|child| {
                    !matches!(child, FragmentNode::Text(text) if text.is_ascii_ws_only)
                })
            }
            ElementKind::Html => template_emits_nested_block(element.fragment.nodes),
        },
    })
}
