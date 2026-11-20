//! The template accumulator: alternating static text and interpolation
//! expressions, flushed into `$$renderer.push(…)` statements.
//!
//! A **shared primitive** of the emission layer, not part of any walk — a pure
//! leaf that calls back into nothing. Every emitter that produces template output
//! owns or borrows one: [`crate::fragment`]'s per-fragment walk, the control-flow
//! blocks ([`crate::blocks`]), [`crate::snippet_emit`], [`crate::element`], and
//! [`crate::attribute`].
//!
//! See [`crate::transform_server`] for the orchestration that builds the
//! component's root body.

use tsv_lang::store::Run;
use tsv_ts::ast::{ExprId, Expression, Statement, StmtId};

use crate::build::Builder;

/// A statement body under construction: the statements emitted so far plus the
/// pending template accumulator (alternating static text and interpolation
/// expressions, `texts.len() == exprs.len() + 1` — the
/// [`Builder::template_literal`] shape). Control-flow blocks `flush` the
/// pending template into a `$$renderer.push(…)` statement, emit their own
/// statements, and let closer-anchor text accumulate into the next template —
/// the oracle's multi-push output shape.
///
/// The statements are ids: a borrowed host statement goes in as its own id
/// ([`Self::push_statement_id`], never copied out and pushed again), a built one as
/// the id of the row [`Self::push_statement`] pushes, and [`Self::finish`] pushes them
/// as the body's run. The pending interpolations are built rows, pushed at the flush.
pub(crate) struct BodyBuilder {
    pub(crate) stmts: Vec<StmtId>,
    texts: Vec<String>,
    exprs: Vec<Expression>,
}

impl BodyBuilder {
    pub(crate) fn new() -> Self {
        Self {
            stmts: Vec::new(),
            texts: vec![String::new()],
            exprs: Vec::new(),
        }
    }

    /// Append an already template-escaped chunk to the current static part.
    ///
    /// **The cross-chunk `${` seam.** Each chunk is template-escaped on its own
    /// (`escape_template_text` rewrites `$` to `\$` only when it sees the `{`
    /// itself), so a literal `$` *ending* one chunk and a literal `{` *starting*
    /// the next slip through as a live interpolation — the emitted
    /// `` $$renderer.push(`… ${NAME} …`) `` would then evaluate `NAME`, or fail to
    /// parse. Real: `ssh ${'{'}DEPLOY_USER}` writes a shell variable by folding a
    /// `'{'` string literal into the text right after a `$`. The oracle escapes it
    /// (it assembles the whole string before escaping); tsv joins the seam here.
    pub(crate) fn push_text(&mut self, chunk: &str) {
        // Every element of `texts` exists by construction (starts with one entry;
        // `push_expr` appends the follower).
        #[expect(clippy::unwrap_used)]
        let current = self.texts.last_mut().unwrap();
        if current.ends_with('$') && chunk.starts_with('{') {
            // The trailing `$` is raw (any preceding backslash was already
            // doubled), so escaping it here is the identity escape `\$` — the
            // rendered text is unchanged, the interpolation is not.
            current.pop();
            current.push_str("\\$");
        }
        current.push_str(chunk);
    }

    pub(crate) fn push_expr(&mut self, expr: Expression) {
        self.exprs.push(expr);
        self.texts.push(String::new());
    }

    /// Flush the pending template (if any) into a `$$renderer.push(…)`
    /// statement.
    fn flush(&mut self, b: &mut Builder<'_>) {
        if self.exprs.is_empty() && self.texts.iter().all(String::is_empty) {
            return;
        }
        let texts = std::mem::replace(&mut self.texts, vec![String::new()]);
        let exprs = std::mem::take(&mut self.exprs);
        let ids: Vec<ExprId> = exprs.iter().map(|&expr| b.push(expr)).collect();
        let expressions = b.push_list(&ids);
        let template = b.template_literal(&texts, expressions);
        let template = b.push(template);
        let push_call = b.member_call("$$renderer", "push", &[template]);
        // Pushed directly rather than through `push_statement`: this IS the flush.
        let stmt = b.expression_statement(push_call);
        self.stmts.push(b.push(stmt));
    }

    /// Flush the pending template, then push a built statement and append it. A
    /// borrowed host statement goes through [`Self::push_statement_id`]; passing a
    /// copy of one here pushes a duplicate row.
    pub(crate) fn push_statement(&mut self, b: &mut Builder<'_>, stmt: Statement) {
        self.flush(b);
        self.stmts.push(b.push(stmt));
    }

    /// Flush the pending template, then append a statement that already has an id —
    /// a borrowed host statement, or one pushed earlier.
    pub(crate) fn push_statement_id(&mut self, b: &mut Builder<'_>, stmt: StmtId) {
        self.flush(b);
        self.stmts.push(stmt);
    }

    /// Flush the pending template, then append `expression` in statement
    /// position — the shape every block/element emitter ends on.
    pub(crate) fn push_expression_statement(
        &mut self,
        b: &mut Builder<'_>,
        expression: Expression,
    ) {
        let stmt = b.expression_statement(expression);
        self.push_statement(b, stmt);
    }

    /// Finish: flush and return the statement run.
    pub(crate) fn finish(mut self, b: &mut Builder<'_>) -> Run<StmtId> {
        self.flush(b);
        b.push_list(&self.stmts)
    }
}
