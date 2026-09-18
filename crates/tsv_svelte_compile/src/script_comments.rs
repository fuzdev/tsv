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
    ArrowFunctionBody, ClassBody, ClassMember, ExportDefaultValue, Expression, ExpressionKind,
    ForInOfLeft, ForInit, ObjectPatternProperty, ObjectProperty, Statement, StatementKind,
    VariableDeclaration,
};

use tsv_lang::Span;

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
    let in_module =
        |comment: &tsv_lang::Comment| module_content.is_some_and(|m| m.contains(comment.span));
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
    let survives = |stmt: &Statement<'_>| match &stmt.kind {
        StatementKind::ImportDeclaration(_) => false,
        StatementKind::ExpressionStatement(expr_stmt) => {
            is_effect_call(expr_stmt.expression, source).is_none()
                && is_inspect_call(expr_stmt.expression, source).is_none()
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
    // Walk only what the emitted body keeps: a hoisted import or a dropped effect
    // opens no block and flushes nothing in the oracle's output.
    let census = BlockCensus::of(instance_body.iter().filter(|stmt| survives(stmt)));
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
        // A multi-line block comment's interior lines diverge unless it is a `*`-gutter
        // comment (`multiline_comment_carries`). Checked before the after-last rule
        // below so this independent gate keeps its own refusal bucket whatever the
        // template emits.
        if comment.multiline && !multiline_comment_carries(comment, source) {
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
        refuse_comment_reflushed_into_block(comment, &census, source)?;
        let mut comment = *comment;
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
        // script comments). A Unicode-whitespace-only text (`is_collapsible_ws_only ==
        // false`) is content and correctly still refuses.
        if let FragmentNode::Text(text) = node
            && text.is_collapsible_ws_only
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
///    ([`BlockCensus::min_block_start`]). Those are exactly the nodes esrap opens with a
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
/// multi-line block comment the reprint doesn't rebuild ([`multiline_comment_carries`]), a comment
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
    module_erased_windows: &[Span],
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
    let census = BlockCensus::of(module_body);
    let min_block = census.min_block_start();
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
                !matches!(&stmt.kind, StatementKind::EmptyStatement(_))
                    && stmt.span().end > comment.span.end
            });
        if !block_before || !has_flush {
            continue;
        }
        // The oracle KEEPS `comment`. Refuse the reprint-divergent classes.
        if comment.multiline && !multiline_comment_carries(comment, source) {
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
        refuse_comment_reflushed_into_block(comment, &census, source)?;
        let mut comment = *comment;
        // Release JSDoc-cast ownership so the positional machinery prints it — its
        // owning `JsdocCast` node is erased in the emitted program (see the same
        // step in `collect_script_comments`).
        comment.owned_by_node = false;
        kept.push(comment);
    }
    Ok(kept)
}

/// Whether a multi-line block comment reprints identically on both sides of the parity
/// comparison.
///
/// The oracle rewrites its interior lines twice: Svelte's parse strips the comment's
/// start-line indentation from the start of every line of the value
/// ([`tsv_lang::Comment::wire_value`] models it), and esrap prints each later line after a
/// newline plus the emit indent. tsv carries the host text. Both outputs then pass through
/// `canonicalize_js`, whose printer rebuilds an indentable (`*`-gutter) comment from its
/// trimmed lines — erasing every leading-whitespace difference on the lines after the
/// first — and prints a preserved one verbatim. So an indentable comment converges iff its
/// first line (only end-trimmed) survived the strip, and a preserved one diverges whenever
/// the two indents differ, so it refuses.
///
/// A `\r` / U+2028 / U+2029 inside refuses: the strip's `m`-flag `^` starts a line there,
/// while esrap and the printer split on `\n` alone. Reproducing a preserved comment's
/// interior (strip + emit indent) in the compile path would reclaim the rest.
fn multiline_comment_carries(comment: &tsv_lang::Comment, source: &str) -> bool {
    let content = comment.content(source);
    if content.contains(['\r', '\u{2028}', '\u{2029}'])
        || !tsv_lang::is_indentable_block(source, comment)
    {
        return false;
    }
    let value = comment.wire_value(source, tsv_lang::AcornPrefix::DOCUMENT);
    content.split('\n').next() == value.split('\n').next()
}

/// Refuse a comment the oracle prints TWICE: one inside a block that opens on the
/// line a statement (or class member) ends, ahead of that block.
///
/// esrap's `body()` flushes, after each statement, every comment that starts on the
/// statement's end line and ends before the NEXT statement's END — not its start — as
/// a trailing comment. On `let n = 1; if (n) { /* c */ n = 2; }` that writes `c` after
/// `let n = 1;`, reaching into the next statement. Opening the `if` block then runs
/// `reset_comment_index`, which finds the index already past a comment inside the
/// block, seeks back to it, and writes `c` again. tsv prints it once — a count
/// difference the parity bar grades. Prettier-formatted code never puts two statements
/// on one line, so real code rarely reaches this.
///
/// The test is a superset of esrap's condition, so it can only over-refuse: the
/// innermost block holding the comment, and any statement or member end at or before
/// that block's start with no `\n` between it and the comment (a `\n`-only line test
/// joins lines the ECMAScript line class splits, never the reverse).
fn refuse_comment_reflushed_into_block(
    comment: &tsv_lang::Comment,
    census: &BlockCensus,
    source: &str,
) -> Result<(), CompileError> {
    let Some(block) = census
        .blocks
        .iter()
        .filter(|block| block.start <= comment.span.start && comment.span.end <= block.end)
        .max_by_key(|block| block.start)
    else {
        return Ok(());
    };
    // The latest end at or before the block decides: any earlier end's gap to the
    // comment contains this one's, so it holds a `\n` whenever this one does.
    let flushed_ahead = census
        .item_ends
        .iter()
        .copied()
        .filter(|&end| end <= block.start)
        .max()
        .is_some_and(|end| {
            !Span::new(end, comment.span.start)
                .extract(source)
                .contains('\n')
        });
    if flushed_ahead {
        return Err(unsupported(Refusal::CommentReflushedIntoBlock));
    }
    Ok(())
}

/// The positions esrap's single comment index keys on, gathered in one exhaustive
/// walk: every `BlockStatement`, `ClassBody` and class static block — the nodes its
/// `body()` opens with a `loc`-bearing `reset_comment_index`, which can seek the index
/// BACKWARD — and every statement / class-member end, the points after which a
/// `body()` flushes a same-line comment as a trailing comment.
///
/// The walk only ever notes a genuine block node, so a reader of `blocks` alone never
/// OVER-reports; `item_ends` deliberately notes MORE than esrap flushes after (every
/// statement, not only a body-list child), which a reader may only use in the
/// refusing direction. Exhaustively matched so a new AST variant fails compilation
/// here rather than silently changing that balance.
#[derive(Default)]
struct BlockCensus {
    blocks: Vec<Span>,
    item_ends: Vec<u32>,
}

impl BlockCensus {
    fn of<'s, 'arena: 's>(statements: impl IntoIterator<Item = &'s Statement<'arena>>) -> Self {
        let mut census = Self::default();
        for stmt in statements {
            census_stmt(stmt, &mut census);
        }
        census
    }

    /// The earliest block start — the module comment's condition-1 anchor
    /// ([`collect_module_script_comments`]). `None` when the body holds no block.
    fn min_block_start(&self) -> Option<u32> {
        self.blocks.iter().map(|block| block.start).min()
    }
}

fn census_stmts(statements: &[Statement<'_>], census: &mut BlockCensus) {
    for stmt in statements {
        census_stmt(stmt, census);
    }
}

fn census_stmt(stmt: &Statement<'_>, census: &mut BlockCensus) {
    census.item_ends.push(stmt.span().end);
    match &stmt.kind {
        StatementKind::BlockStatement(block) => {
            census.blocks.push(block.span);
            census_stmts(block.body, census);
        }
        StatementKind::FunctionDeclaration(f) => {
            for param in f.params {
                census_expr(param, census);
            }
            census.blocks.push(f.body.span);
            census_stmts(f.body.body, census);
        }
        StatementKind::ClassDeclaration(c) => census_class_body(&c.body, census),
        StatementKind::ExpressionStatement(s) => census_expr(s.expression, census),
        StatementKind::VariableDeclaration(d) => census_var_decl(d, census),
        StatementKind::ReturnStatement(s) => {
            if let Some(arg) = s.argument.as_ref() {
                census_expr(arg, census);
            }
        }
        StatementKind::ThrowStatement(s) => census_expr(s.argument, census),
        StatementKind::IfStatement(s) => {
            census_expr(s.test, census);
            census_stmt(s.consequent, census);
            if let Some(alt) = s.alternate {
                census_stmt(alt, census);
            }
        }
        StatementKind::ForStatement(s) => {
            match &s.init {
                Some(ForInit::VariableDeclaration(d)) => census_var_decl(d, census),
                Some(ForInit::Expression(e)) => census_expr(e, census),
                None => {}
            }
            if let Some(test) = s.test.as_ref() {
                census_expr(test, census);
            }
            if let Some(update) = s.update.as_ref() {
                census_expr(update, census);
            }
            census_stmt(s.body, census);
        }
        StatementKind::ForInStatement(s) => {
            census_for_left(s.left, census);
            census_expr(s.right, census);
            census_stmt(s.body, census);
        }
        StatementKind::ForOfStatement(s) => {
            census_for_left(s.left, census);
            census_expr(s.right, census);
            census_stmt(s.body, census);
        }
        StatementKind::WhileStatement(s) => {
            census_expr(s.test, census);
            census_stmt(s.body, census);
        }
        StatementKind::DoWhileStatement(s) => {
            census_stmt(s.body, census);
            census_expr(s.test, census);
        }
        // Unreachable in practice: a Svelte `<script>` is Module code, so it is strict
        // and the parser refuses `with` there.
        StatementKind::WithStatement(s) => {
            census_expr(s.object, census);
            census_stmt(s.body, census);
        }
        StatementKind::SwitchStatement(s) => {
            census_expr(s.discriminant, census);
            for case in s.cases {
                if let Some(test) = case.test.as_ref() {
                    census_expr(test, census);
                }
                census_stmts(case.consequent, census);
            }
        }
        StatementKind::TryStatement(s) => {
            census.blocks.push(s.block.span);
            census_stmts(s.block.body, census);
            if let Some(handler) = &s.handler {
                if let Some(param) = &handler.param {
                    census_expr(param, census);
                }
                census.blocks.push(handler.body.span);
                census_stmts(handler.body.body, census);
            }
            if let Some(finalizer) = &s.finalizer {
                census.blocks.push(finalizer.span);
                census_stmts(finalizer.body, census);
            }
        }
        StatementKind::LabeledStatement(s) => census_stmt(s.body, census),
        StatementKind::ExportNamedDeclaration(s) => {
            if let Some(decl) = &s.declaration {
                census_stmt(decl, census);
            }
        }
        StatementKind::ExportDefaultDeclaration(s) => match &s.declaration {
            ExportDefaultValue::Expression(e) => census_expr(e, census),
            ExportDefaultValue::FunctionDeclaration(f) => {
                for param in f.params {
                    census_expr(param, census);
                }
                census.blocks.push(f.body.span);
                census_stmts(f.body.body, census);
            }
            ExportDefaultValue::ClassDeclaration(c) => census_class_body(&c.body, census),
            ExportDefaultValue::TSDeclareFunction(_)
            | ExportDefaultValue::TSInterfaceDeclaration(_) => {}
        },
        StatementKind::TSExportAssignment(s) => census_expr(&s.expression, census),
        // No block-bearing children (or a TypeScript-only statement dropped before
        // this runs).
        StatementKind::BreakStatement(_)
        | StatementKind::ContinueStatement(_)
        | StatementKind::EmptyStatement(_)
        | StatementKind::DebuggerStatement(_)
        | StatementKind::ImportDeclaration(_)
        | StatementKind::ExportAllDeclaration(_)
        | StatementKind::TSNamespaceExportDeclaration(_)
        | StatementKind::TSImportEqualsDeclaration(_)
        | StatementKind::TSTypeAliasDeclaration(_)
        | StatementKind::TSInterfaceDeclaration(_)
        | StatementKind::TSDeclareFunction(_)
        | StatementKind::TSEnumDeclaration(_)
        | StatementKind::TSModuleDeclaration(_) => {}
    }
}

fn census_var_decl(decl: &VariableDeclaration<'_>, census: &mut BlockCensus) {
    for declarator in decl.declarations {
        census_expr(declarator.id, census);
        if let Some(init) = declarator.init.as_ref() {
            census_expr(init, census);
        }
    }
}

fn census_for_left(left: &ForInOfLeft<'_>, census: &mut BlockCensus) {
    match left {
        ForInOfLeft::VariableDeclaration(d) => census_var_decl(d, census),
        ForInOfLeft::Pattern(p) => census_expr(p, census),
    }
}

fn census_class_body(body: &ClassBody<'_>, census: &mut BlockCensus) {
    census.blocks.push(body.span);
    for member in body.body {
        census.item_ends.push(member.span().end);
        match member {
            ClassMember::MethodDefinition(m) => {
                if m.computed {
                    census_expr(&m.key, census);
                }
                for param in m.value.params {
                    census_expr(param, census);
                }
                census.blocks.push(m.value.body.span);
                census_stmts(m.value.body.body, census);
            }
            ClassMember::PropertyDefinition(p) => {
                if p.computed {
                    census_expr(&p.key, census);
                }
                if let Some(value) = p.value.as_ref() {
                    // esrap's `PropertyDefinition` flushes after the VALUE with no
                    // upper bound, and the value can end a line above the member.
                    census.item_ends.push(value.span().end);
                    census_expr(value, census);
                }
            }
            ClassMember::StaticBlock(b) => {
                census.blocks.push(b.span);
                census_stmts(b.body, census);
            }
            ClassMember::IndexSignature(_) => {}
        }
    }
}

fn census_exprs(exprs: &[&Expression<'_>], census: &mut BlockCensus) {
    for expr in exprs {
        census_expr(expr, census);
    }
}

fn census_expr(expr: &Expression<'_>, census: &mut BlockCensus) {
    match &expr.kind {
        ExpressionKind::ArrowFunctionExpression(a) => {
            for param in a.params {
                census_expr(param, census);
            }
            match &a.body {
                ArrowFunctionBody::Expression(e) => census_expr(e, census),
                ArrowFunctionBody::BlockStatement(b) => {
                    census.blocks.push(b.span);
                    census_stmts(b.body, census);
                }
            }
        }
        ExpressionKind::FunctionExpression(f) => {
            for param in f.params {
                census_expr(param, census);
            }
            census.blocks.push(f.body.span);
            census_stmts(f.body.body, census);
        }
        ExpressionKind::ClassExpression(c) => census_class_body(&c.body, census),
        ExpressionKind::NewExpression(e) => {
            census_expr(e.callee, census);
            census_exprs(e.arguments, census);
        }
        ExpressionKind::CallExpression(e) => {
            census_expr(e.callee, census);
            census_exprs(e.arguments, census);
        }
        ExpressionKind::MemberExpression(e) => {
            census_expr(e.object, census);
            if e.computed {
                census_expr(e.property, census);
            }
        }
        ExpressionKind::ObjectExpression(obj) => {
            for prop in obj.properties {
                match prop {
                    ObjectProperty::Property(p) => {
                        if p.computed {
                            census_expr(p.key, census);
                        }
                        census_expr(p.value, census);
                    }
                    ObjectProperty::SpreadElement(s) => census_expr(s.argument, census),
                }
            }
        }
        ExpressionKind::ArrayExpression(arr) => {
            for element in arr.elements {
                if let Some(e) = element.as_ref() {
                    census_expr(e, census);
                }
            }
        }
        ExpressionKind::UnaryExpression(u) => census_expr(u.argument, census),
        ExpressionKind::UpdateExpression(u) => census_expr(u.argument, census),
        ExpressionKind::BinaryExpression(b) => {
            census_expr(b.left, census);
            census_expr(b.right, census);
        }
        ExpressionKind::ConditionalExpression(c) => {
            census_expr(c.test, census);
            census_expr(c.consequent, census);
            census_expr(c.alternate, census);
        }
        ExpressionKind::SpreadElement(s) => census_expr(s.argument, census),
        ExpressionKind::TemplateLiteral(t) => census_exprs(t.expressions, census),
        ExpressionKind::TaggedTemplateExpression(t) => {
            census_expr(t.tag, census);
            census_exprs(t.quasi.expressions, census);
        }
        ExpressionKind::AwaitExpression(a) => census_expr(a.argument, census),
        ExpressionKind::YieldExpression(y) => {
            if let Some(arg) = y.argument {
                census_expr(arg, census);
            }
        }
        ExpressionKind::SequenceExpression(s) => census_exprs(s.expressions, census),
        ExpressionKind::AssignmentExpression(a) => {
            census_expr(a.left, census);
            census_expr(a.right, census);
        }
        ExpressionKind::ObjectPattern(p) => {
            for prop in p.properties {
                match prop {
                    ObjectPatternProperty::Property(prop) => {
                        if prop.computed {
                            census_expr(prop.key, census);
                        }
                        census_expr(prop.value, census);
                    }
                    ObjectPatternProperty::RestElement(rest) => census_expr(rest.argument, census),
                }
            }
        }
        ExpressionKind::ArrayPattern(p) => {
            for element in p.elements {
                if let Some(e) = element.as_ref() {
                    census_expr(e, census);
                }
            }
        }
        ExpressionKind::AssignmentPattern(p) => {
            census_expr(p.left, census);
            census_expr(p.right, census);
        }
        ExpressionKind::RestElement(r) => census_expr(r.argument, census),
        ExpressionKind::TSTypeAssertion(t) => census_expr(t.expression, census),
        ExpressionKind::TSAsExpression(t) => census_expr(t.expression, census),
        ExpressionKind::TSSatisfiesExpression(t) => census_expr(t.expression, census),
        ExpressionKind::TSInstantiationExpression(t) => census_expr(t.expression, census),
        ExpressionKind::TSNonNullExpression(t) => census_expr(t.expression, census),
        ExpressionKind::TSParameterProperty(t) => census_expr(t.parameter, census),
        ExpressionKind::ImportExpression(i) => {
            census_expr(i.source, census);
            if let Some(options) = i.options {
                census_expr(options, census);
            }
        }
        ExpressionKind::JsdocCast(j) => census_expr(j.inner, census),
        ExpressionKind::ParenthesizedExpression(p) => census_expr(p.expression, census),
        // Leaves — no children, no blocks.
        ExpressionKind::Literal(_)
        | ExpressionKind::Identifier(_)
        | ExpressionKind::PrivateIdentifier(_)
        | ExpressionKind::RegexLiteral(_)
        | ExpressionKind::ThisExpression(_)
        | ExpressionKind::Super(_)
        | ExpressionKind::MetaProperty(_) => {}
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
                    !matches!(child, FragmentNode::Text(text) if text.is_collapsible_ws_only)
                })
            }
            ElementKind::Html => template_emits_nested_block(element.fragment.nodes),
        },
    })
}
