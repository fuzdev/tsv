//! The template accumulator: alternating static text and interpolation
//! expressions, flushed into `$$renderer.push(…)` statements.
//!
//! A **shared primitive** of the emission layer, not part of any walk — a pure
//! leaf that calls back into nothing. Every emitter that produces template output
//! owns or borrows one: [`crate::fragment`]'s per-fragment walk, the control-flow
//! blocks ([`crate::blocks`]), [`crate::snippet_emit`], [`crate::element`], and
//! [`crate::attribute`]. It is the single home of the oracle's
//! `b.block([...state.init, ...build_template(state.template)])` shape — the
//! init/template split that [`BodyBuilder::mark_init_end`] and
//! [`BodyBuilder::push_init_statement`] together model — so no emitter
//! reconstructs that ordering itself.
//!
//! See [`crate::transform_server`] for the orchestration that builds the
//! component's root body.

use bumpalo::collections::Vec as BumpVec;
use tsv_ts::ast::internal::{Expression, ExpressionStatement, Statement};

use crate::build::Builder;

/// A statement body under construction: the statements emitted so far plus the
/// pending template accumulator (alternating static text and interpolation
/// expressions, `texts.len() == exprs.len() + 1` — the
/// [`Builder::template_literal`] shape). Control-flow blocks `flush` the
/// pending template into a `$$renderer.push(…)` statement, emit their own
/// statements, and let closer-anchor text accumulate into the next template —
/// the oracle's multi-push output shape.
pub(crate) struct BodyBuilder<'arena> {
    pub(crate) stmts: BumpVec<'arena, Statement<'arena>>,
    texts: Vec<String>,
    exprs: BumpVec<'arena, Expression<'arena>>,
    /// How many leading [`stmts`](Self::stmts) belong to the oracle's **`init`**
    /// list rather than its template stream. The server `Fragment` visitor returns
    /// `b.block([...state.init, ...build_template(state.template)])`, so EVERY init
    /// statement precedes EVERY template push — the two lists are concatenated, not
    /// interleaved. tsv keeps one list and remembers where init ended
    /// ([`mark_init_end`](Self::mark_init_end)), so a statement discovered *during*
    /// the template walk that the oracle would have pushed to `init` can still be
    /// placed there ([`push_init_statement`](Self::push_init_statement)).
    init_len: usize,
}

impl<'arena> BodyBuilder<'arena> {
    pub(crate) fn new_in(arena: &'arena bumpalo::Bump) -> Self {
        Self {
            stmts: BumpVec::new_in(arena),
            texts: vec![String::new()],
            exprs: BumpVec::new_in(arena),
            init_len: 0,
        }
    }

    /// Close the init region: everything emitted from here on is template.
    ///
    /// Called once per **block-scope** fragment, after its hoisted `{@const}` /
    /// `<svelte:head>` / `<title>` / `{#snippet}` statements — the oracle's
    /// `for (const node of hoisted) context.visit(node, state)` loop. An
    /// element-child fragment shares the enclosing block's builder and must NOT
    /// move the mark.
    ///
    /// ⚠️ **Invariant: a fragment that OWNS its builder is exactly a fragment with
    /// `hoist_snippets`.** [`crate::fragment::emit_fragment`] gates this call on that
    /// flag, which is only sound while the two coincide — a builder-sharing fragment
    /// that also hoisted would move a mark it does not own, and every already-emitted
    /// template statement of the enclosing block would fall inside the init region,
    /// so a later [`push_init_statement`](Self::push_init_statement) would splice
    /// its declaration BELOW pushes the oracle puts it above. The coincidence holds
    /// by enumeration over the three [`FragmentCtx`](crate::fragment::FragmentCtx)
    /// construction sites, not by type: the component root (`transform_server.rs`)
    /// and `emit_child_body` each build a fresh [`BodyBuilder`] and pass
    /// `hoist_snippets: true`; the element-child fragment (`element.rs`) forwards the
    /// enclosing builder and passes `hoist_snippets: false`. A **fourth** site must
    /// preserve that pairing — or split the two facts apart, so ownership stops
    /// riding a flag that means something else.
    pub(crate) fn mark_init_end(&mut self) {
        self.init_len = self.stmts.len();
    }

    /// Insert a statement at the end of the init region, without flushing the
    /// pending template.
    ///
    /// The one caller is `<svelte:boundary>`'s `failed` snippet: the oracle's
    /// `SnippetBlock` visitor pushes its `function` declaration to `state.init`
    /// even though the boundary is reached mid-template, so the declaration lands
    /// above every push of the enclosing block rather than beside the boundary.
    /// Two boundaries in one fragment keep source order (each insert advances the
    /// mark), matching the oracle's append-to-`init`.
    pub(crate) fn push_init_statement(&mut self, stmt: Statement<'arena>) {
        self.stmts.insert(self.init_len, stmt);
        self.init_len += 1;
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
        #[allow(clippy::unwrap_used)]
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

    pub(crate) fn push_expr(&mut self, expr: Expression<'arena>) {
        self.exprs.push(expr);
        self.texts.push(String::new());
    }

    /// Flush the pending template (if any) into a `$$renderer.push(…)`
    /// statement.
    fn flush(&mut self, b: &mut Builder<'arena>, arena: &'arena bumpalo::Bump) {
        if self.exprs.is_empty() && self.texts.iter().all(String::is_empty) {
            return;
        }
        let texts = std::mem::replace(&mut self.texts, vec![String::new()]);
        let exprs = std::mem::replace(&mut self.exprs, BumpVec::new_in(arena));
        let template = b.template_literal(&texts, exprs.into_bump_slice());
        let template_alloc = arena.alloc(template);
        let push_call = b.member_call("$$renderer", "push", std::slice::from_ref(template_alloc));
        let span = push_call.span();
        self.stmts
            .push(Statement::ExpressionStatement(ExpressionStatement {
                expression: push_call,
                span,
                is_directive: false,
            }));
    }

    /// Flush the pending template, then append a statement.
    pub(crate) fn push_statement(
        &mut self,
        b: &mut Builder<'arena>,
        arena: &'arena bumpalo::Bump,
        stmt: Statement<'arena>,
    ) {
        self.flush(b, arena);
        self.stmts.push(stmt);
    }

    /// Finish: flush and return the statement slice.
    pub(crate) fn finish(
        mut self,
        b: &mut Builder<'arena>,
        arena: &'arena bumpalo::Bump,
    ) -> &'arena [Statement<'arena>] {
        self.flush(b, arena);
        self.stmts.into_bump_slice()
    }
}
