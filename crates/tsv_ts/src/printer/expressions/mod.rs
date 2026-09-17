// Expression printing for TypeScript
//
// This module coordinates expression printing and delegates to specialized submodules:
// - literals.rs: Literals, identifiers, regex, spread, normalize helper
// - functions.rs: Arrow functions and function expressions
// - blocks.rs: Block statements (reusable utility)
// - patterns.rs: All destructuring patterns (object, array, assignment, rest)
// - objects.rs: Object expressions and property handling
// - arrays.rs: Array expressions
// - operators.rs: Unary, binary, and update expressions
// - assignment.rs: Assignment layout engine (declarators, properties, returns)
// - conditional.rs: Ternary/conditional expressions
// - template_literal.rs: Template literals (both regular and tagged)
// - ../calls/: Call, new, and member-chain expressions
//
// This module handles:
// - Expression dispatch (print_expression, build_expression_doc)

mod arrays;
pub(in crate::printer) mod assignment;
pub(in crate::printer) mod blocks;
pub(in crate::printer) mod conditional;
pub(in crate::printer) mod functions;
pub(crate) mod literals;
mod objects;
pub(in crate::printer) mod operators;
mod patterns;
mod template_literal;

/// The chain printer's one reach into this module's private `conditional`: a
/// parenthesized chain base asks what its left-spine ternary wants from the parens
/// around it. Re-exported rather than opening the module, which holds a good deal
/// more than the chain has any business seeing.
pub(in crate::printer) use self::conditional::ChainBaseTernary;

use self::operators::{OperatorBuf, SeqLayout};
use crate::ast::internal::{BinaryExpression, Expression, ExpressionKind, TSType};
use crate::printer::ShareTag;
use crate::printer::comments::{CommentFilter, CommentSpacing};
use crate::printer::ignore::FrozenOperandPair;
use crate::printer::types::TrailingBlock;
use crate::printer::types::helpers::unwrap_parenthesized;
use crate::printer::{ParenContext, PatternContext, Printer, chain, class_expr_has_decorators};
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// What a comment-blind binary-chain layout builder gets to work with, produced by
/// `Printer::prepare_binary_chain_layout`.
enum BinaryChainLayout {
    /// A finished doc — the comment-aware twin's, or the ordinary binary doc for a chain
    /// that flattened to a single operand. Return it unchanged.
    Built(DocId),
    /// A comment-free chain of 2+ operands, flattened into operand docs plus the
    /// operators between them, ready to lay out.
    Operands(DocBuf, OperatorBuf),
}

impl<'a> Printer<'a> {
    /// Wrap `doc` in parens when `span` is the object/function/class node that
    /// starts the enclosing expression statement (set by `build_expression_statement_doc`
    /// via `leftmost_no_lookahead`). Consumes the target so it fires exactly once:
    /// `(class {}).foo` wraps the class, not the whole member expression.
    pub(in crate::printer) fn maybe_wrap_expr_stmt_paren(&self, span: Span, doc: DocId) -> DocId {
        // Matched by span, not consumed: a chain may rebuild its base across
        // conditional-group variants (`({a: 1}).b().c()`), so consuming the target on
        // the first (possibly discarded) build would leave the selected variant
        // unwrapped. The target is cleared once per statement in build_expression_statement.
        if self.expr_stmt_paren_target.get() == Some(span) {
            self.d().parens(doc)
        } else {
            doc
        }
    }

    /// Wrap `inner` in parens that break open and indent: `(⏎\t<inner>⏎)`. Used for a
    /// parenthesized *decorated* class expression, where the decorators force the
    /// break so prettier opens the parens — unlike an undecorated `(class {})`, which
    /// stays flat. Shared by the bare-statement form (`build_expression_statement_doc`)
    /// and the self-wrapping member/call form (`(@dec class {}).foo` / `()`, via the
    /// `ClassExpression` arm of `build_expression_doc`).
    pub(in crate::printer) fn build_break_open_parens(&self, inner: DocId) -> DocId {
        let d = self.d();
        d.concat(&[
            d.text("("),
            d.indent_hardline(inner),
            d.hardline(),
            d.text(")"),
        ])
    }

    /// Build a Doc for an expression (for use in object/array contexts and statements)
    ///
    /// Every expression funnels through here, which is what lets the owned-comment seam
    /// be a single line: a comment bound to `expr`'s first token is prepended *inside*
    /// the doc, so a paren any parent synthesizes around it lands outside the pair. See
    /// `comments/owned.rs`.
    pub(crate) fn build_expression_doc(&self, expr: &Expression<'_>) -> DocId {
        let doc = self.build_expression_doc_dispatch(expr);
        self.prepend_owned_leading_comment(expr, doc)
    }

    /// Build the doc for an embedding host's expression ROOT (a Svelte `{expr}` value).
    ///
    /// The single position where the host's `LayoutMode` decides binary chain style: an
    /// Embedded root binary takes ContinuationIndent — the template `{…}` value indents
    /// continuation lines one level past the first operand (prettier reaches the same
    /// shape through its svelte expression-root wrapper). Every NESTED binary inside the
    /// expression formats exactly as it would in a `<script>` — the parent position keys
    /// the style (assignment layouts flush, call args and array elements indent),
    /// mirroring prettier's parent-keyed `shouldNotIndent` chain (`print/binaryish.js`) — which
    /// is what keeps TS formatting context-free below the root. A Standalone-mode root
    /// (`{@const}`'s init, inheriting the host document's mode) takes the ordinary
    /// dispatch, where the host's assignment-value mark answers FLAT: that layout owns the
    /// indent, and a continuation indent would stack on top.
    ///
    /// Also where the embed's other build-time field lands: a host that cannot hang a
    /// leading cast (`EmbedContext::jsdoc_cast_cannot_hang` — a Svelte braced head) has
    /// the root's left-spine cast marked here, before any doc is built, so its
    /// comment→`(` break reflows in every authoring (`build_jsdoc_cast_lead_doc`). Both root
    /// entries — this doc builder and `print_expression`'s string path — pass through
    /// here, so the flag cannot behave differently per entry.
    ///
    /// The root NAMES the style rather than leaning on that default, for one reason: it
    /// must override the assignment-value mark a host may have set on the same expression
    /// (`build_assignment_value_expression_doc`), which would otherwise answer flat.
    pub(crate) fn build_root_expression_doc(&self, expr: &Expression<'_>) -> DocId {
        if self.embed.jsdoc_cast_cannot_hang {
            self.mark_jsdoc_cast_cannot_hang_gap(expr);
        }
        if !self.embed.is_embedded() {
            return self.build_expression_doc(expr);
        }
        if let ExpressionKind::BinaryExpression(binary) = &expr.kind {
            // The owned-comment obligation `build_flat_chain_expression_doc` states, on the
            // other style: reaching past `build_expression_doc` means discharging it here.
            let doc = self.build_binary_chain_doc_with_continuation_indent(binary, expr.span);
            return self.prepend_owned_leading_comment(expr, doc);
        }
        // ⚠️ A SEQUENCE root does NOT join the binary above by default, though it is the
        // same shape of chain: prettier's svelte expression-root wrapper reaches the
        // continuation indent through the binaryish parent rule, which a sequence's
        // `group(join([",", line]))` never consults — so prettier breaks `{(a,⏎b)}` flush
        // and tsv matches it at every head prettier width-wraps. The one head it does not
        // wrap at all — a block head — asks for the indent explicitly
        // (`EmbedContext::root_sequence_indents`), since there tsv's wrap is its own and
        // owes its own geometry rather than a shape inherited from a position prettier
        // answered.
        if self.embed.root_sequence_indents
            && let ExpressionKind::SequenceExpression(seq) = &expr.kind
        {
            // `build_sequence_doc` is not `build_expression_doc`, so the owned leading
            // comment seam is this caller's too, exactly as in the binary arm above.
            let doc = self.build_sequence_doc(seq, expr.span, SeqLayout::Indented);
            return self.prepend_owned_leading_comment(expr, doc);
        }
        self.build_expression_doc(expr)
    }

    /// The body of a pair of parens that **expand** onto their own lines when the
    /// content does not fit, keeping the content itself flat while it does:
    /// `(⏎\tcontent⏎)`. Prettier's `group([indent([softline, content]), softline])`.
    ///
    /// The parens themselves stay OUTSIDE this doc, so a caller that already emits its
    /// own `(` / `)` (a chain base, a cast operand) wraps this and nothing changes about
    /// where the parens come from. Written once here because three sites need the exact
    /// same shape and a hand-rolled fourth would drift.
    pub(in crate::printer) fn build_expanding_parens_body_doc(&self, content: DocId) -> DocId {
        let d = self.d();
        d.group(d.concat(&[d.indent(d.concat(&[d.softline(), content])), d.softline()]))
    }

    /// A pair of **expanding** parens whose own `(` / `)` are INSIDE the group —
    /// `group(["(", indent([softline, content]), softline, ")"])`, prettier's answer at the
    /// binaryish early return (`printBinaryishExpression`, `print/binaryish.js`: `group([indent([softline, ...parts]),
    /// softline])` with the parens supplied by `needs-parens`).
    ///
    /// The sibling of [`Self::build_expanding_parens_body_doc`], which spells the same layout
    /// with the parens *outside* the group. The two are not the same doc, but neither are
    /// they freely different: swapping the callee pair between them was measured
    /// byte-identical over the whole fixture suite and the whole gates corpus.
    ///
    /// **Which to reach for is a question about who emits the `(` / `)`, not about layout.**
    /// Use this one where the pair is the doc's own. Every caller of the other has its parens
    /// emitted by a *different* seam and so has only a body to wrap — a chain base's
    /// `build_owned_required_pair_doc` (whose bare outside parens also let the chain's
    /// `conditional_group` drive the break), a non-null's `)!` closer, a ternary operand's
    /// own pair, a sequence's own pair.
    ///
    /// The three positions prettier reaches through that early return build their operand
    /// with [`Printer::build_binary_chain_doc_ungrouped`] and wrap it here — a call callee,
    /// a `new` callee, and an uncommented unary argument. Each spelled the concat by hand
    /// before the call callee joined them, which is exactly the drift this seam exists to
    /// stop.
    pub(in crate::printer) fn build_expanding_parens_doc(&self, content: DocId) -> DocId {
        let d = self.d();
        d.group(d.concat(&[
            d.text("("),
            d.indent_softline(content),
            d.softline(),
            d.text(")"),
        ]))
    }

    /// `build_expression_doc` for a position whose binary chain takes **no continuation
    /// indent** — prettier's `shouldNotIndent` (`print/binaryish.js`), so the chain renders
    /// as `group(parts)` and its continuation lines sit at the first operand's own column.
    ///
    /// With [`Printer::mark_flat_chain`], the only way to reach the flat layout, which is
    /// no longer any position's default (`Printer::build_binary_chain_doc` states the rule
    /// and both exempt buckets).
    ///
    /// **THE `shouldNotIndent` POSITION LIST LIVES HERE**, whole and count-free — every
    /// other site that needs it points at this one, because a membership copied is a
    /// membership that goes stale on its own. Each entry is a term read off binaryish.js:
    ///
    /// - an arrow function's expression body (`node === parent.body && parent.type ===
    ///   "ArrowFunctionExpression"`);
    /// - a template-literal interpolation (`parent.type === "TemplateLiteral"`);
    /// - a `return`/`throw` argument in the hanging-paren form
    ///   (`isReturnOrThrowStatement(parent)` — the ordinary form's parent owns the group,
    ///   so it takes `build_binary_chain_doc_ungrouped` instead);
    /// - a unary operand carrying a comment (`key === "argument" && parent.type ===
    ///   "UnaryExpression"`; the uncommented one takes prettier's earlier expanding-paren
    ///   return, which `build_unary_doc` spells with `build_binary_chain_doc_ungrouped`);
    /// - a ternary test whose ternary is not itself a return/throw/call/new value
    ///   (`parent.type === "ConditionalExpression"` with the grandparent exclusions).
    ///
    /// Three positions reach the same layout through [`Printer::mark_flat_chain`] instead,
    /// because they hand their value to a generic builder and cannot name one: a ternary
    /// *branch* (its value goes through the paren shell, which owns the gap after it), a
    /// C-style `for` header clause (`node !== parent.body && parent.type ===
    /// "ForStatement"`; the clause's builder belongs to its caller, and a sequence clause's
    /// ELEMENTS are deliberately not marked — their parent is the sequence), and a
    /// `Boolean()` coercion's argument (`key === "arguments" &&
    /// isBooleanTypeCoercion(parent)`, marked by `calls::mark_boolean_coercion_argument`
    /// at both call-argument entry points; every call layout — plain or member-chain,
    /// commented or not — reaches the argument through
    /// [`Printer::build_arg_expression_doc`], which reads the mark).
    ///
    /// A non-binary expression is unaffected and takes the ordinary path.
    ///
    /// ⚠️ The chain builder does **not** prepend an owned leading comment (a JSDoc cast or
    /// bundler annotation glued to the first operand) — `build_expression_doc` owns that
    /// seam — so a position that reaches past it must discharge the obligation itself, or
    /// the comment is DROPPED (`docs/comments.md` hazard 1). That is the whole reason this
    /// is a shared seam rather than one call per site: the obligation is easy to forget,
    /// and every `shouldNotIndent` position inherits it by construction.
    pub(in crate::printer) fn build_flat_chain_expression_doc(
        &self,
        expr: &Expression<'_>,
    ) -> DocId {
        if let ExpressionKind::BinaryExpression(binary) = &expr.kind {
            let doc = self.build_binary_chain_doc_flat(binary, expr.span);
            return self.prepend_owned_leading_comment(expr, doc);
        }
        self.build_expression_doc(expr)
    }

    fn build_expression_doc_dispatch(&self, expr: &Expression<'_>) -> DocId {
        let d = self.d();

        // Take and clear is_expression_statement so it doesn't leak to sub-expressions.
        // Only chain formatting needs this flag (for the isShort merge heuristic).
        // Re-set it only for expression types that enter chain formatting:
        // CallExpression, MemberExpression, TSNonNullExpression.
        let was_expr_stmt = self.is_expression_statement.replace(false);

        match &expr.kind {
            ExpressionKind::Literal(lit) => self.build_literal_doc(lit),
            ExpressionKind::Identifier(id) => {
                // A contextual keyword heading an `as`/`satisfies` cast at statement
                // level wraps itself (`(type) as T;`) — see
                // `build_expression_statement_doc`. A no-op for every other identifier
                // (the target is None or a different span).
                let doc = self.build_identifier_doc(id);
                self.maybe_wrap_expr_stmt_paren(id.span, doc)
            }
            ExpressionKind::PrivateIdentifier(pid) => {
                self.build_private_identifier_doc(pid, expr.span)
            }
            ExpressionKind::ObjectExpression(obj) => {
                // Wrap in parens when this is the leftmost object of an arrow body
                // (`() => ({}) && a`). Matched by span (not consumed): a chain may rebuild
                // its base across conditional-group variants, and a nested call-argument
                // object has a different span so it never matches.
                let needs_arrow_parens =
                    self.arrow_body_leftmost_parens_target.get() == Some(expr.span);
                let doc = self.build_object_doc(obj, expr.span);
                let doc = if needs_arrow_parens {
                    self.d().parens(doc)
                } else {
                    doc
                };
                self.maybe_wrap_expr_stmt_paren(expr.span, doc)
            }
            ExpressionKind::ArrayExpression(arr) => self.build_array_doc(arr, expr.span),
            ExpressionKind::UnaryExpression(unary) => self.build_unary_doc(unary),
            ExpressionKind::UpdateExpression(update) => self.build_update_doc(update, expr.span),
            ExpressionKind::BinaryExpression(binary) => {
                self.build_binary_chain_doc(binary, expr.span)
            }
            ExpressionKind::CallExpression(call) => {
                self.is_expression_statement.set(was_expr_stmt);
                self.build_call_doc(call, expr.span)
            }
            ExpressionKind::NewExpression(new_expr) => self.build_new_doc(new_expr, expr.span),
            ExpressionKind::MemberExpression(member) => {
                self.is_expression_statement.set(was_expr_stmt);
                self.build_member_doc(member, expr.span)
            }
            ExpressionKind::ConditionalExpression(cond) => {
                self.build_conditional_doc(cond, expr.span)
            }
            ExpressionKind::ArrowFunctionExpression(arrow) => {
                self.build_arrow_doc(arrow, expr.span)
            }
            ExpressionKind::FunctionExpression(func) => {
                self.maybe_wrap_expr_stmt_paren(func.span, self.build_function_doc(func))
            }
            ExpressionKind::ClassExpression(class_expr) => {
                let doc = self.build_class_expression_doc(class_expr, expr.span);
                // A DECORATED class expression at the leftmost position of an arrow body
                // takes the same pair its object twin above takes, and for the same
                // reason: `@` is not a token `ConciseBody` admits, so `() => @dec class
                // {}.k` does not reparse. Only a decorated class is ever the target
                // (`leftmost_arrow_body_parens_span`), so the kind needs no second test
                // here. It breaks its parens open like the statement-start form below —
                // the decorator owns its own line either way.
                // A decorated class expression that self-wraps at an expression-statement
                // start (`(@dec class {}).foo`, `(@dec class {})()`) breaks its parens
                // open + indents, like the bare-statement form; an undecorated
                // `(class {}).foo` keeps the flat wrap.
                if self.arrow_body_leftmost_parens_target.get() == Some(expr.span)
                    || (self.expr_stmt_paren_target.get() == Some(expr.span)
                        && class_expr_has_decorators(class_expr))
                {
                    self.build_break_open_parens(doc)
                } else {
                    self.maybe_wrap_expr_stmt_paren(expr.span, doc)
                }
            }
            ExpressionKind::SpreadElement(spread) => self.build_spread_doc(spread),
            ExpressionKind::TemplateLiteral(template) => self.build_template_literal_doc(template),
            ExpressionKind::TaggedTemplateExpression(tagged) => {
                self.build_tagged_template_doc(tagged, expr.span)
            }
            ExpressionKind::AwaitExpression(await_expr) => {
                self.build_await_doc(await_expr, expr.span)
            }
            ExpressionKind::YieldExpression(yield_expr) => {
                self.build_yield_doc(yield_expr, expr.span)
            }
            // Prettier's default layout arm. The three positions that take another
            // (`ExpressionStatement`, the `for` head, a `return`/`throw` argument or arrow
            // body) intercept their sequence before this dispatch, since the layout is the
            // parent's question and this dispatch cannot see one.
            ExpressionKind::SequenceExpression(seq) => {
                self.build_sequence_doc(seq, expr.span, SeqLayout::Aligned)
            }
            ExpressionKind::RegexLiteral(regex) => self.build_regex_doc(regex),
            ExpressionKind::ThisExpression(_) => d.text("this"),
            ExpressionKind::Super(_) => d.text("super"),
            ExpressionKind::AssignmentExpression(assign) => {
                self.build_assignment_doc(assign, expr.span)
            }
            ExpressionKind::ObjectPattern(obj) => self.with_param_decorators(
                obj.decorators,
                self.build_object_pattern_doc(obj, expr.span),
                expr.span.start,
            ),
            ExpressionKind::ArrayPattern(arr) => self.with_param_decorators(
                arr.decorators,
                self.build_array_pattern_doc(arr, expr.span),
                expr.span.start,
            ),
            ExpressionKind::AssignmentPattern(pattern) => self.with_param_decorators(
                pattern.decorators,
                self.build_assignment_pattern_doc(pattern, expr.span),
                expr.span.start,
            ),
            ExpressionKind::RestElement(rest) => self.build_rest_element_doc(rest),
            ExpressionKind::TSTypeAssertion(type_assert) => {
                self.build_ts_type_assertion_doc(type_assert, expr.span)
            }
            ExpressionKind::TSAsExpression(as_expr) => self.build_binary_cast_doc(
                as_expr.expression,
                as_expr.type_annotation,
                "as",
                expr.span.start,
            ),
            ExpressionKind::TSSatisfiesExpression(sat_expr) => self.build_binary_cast_doc(
                sat_expr.expression,
                sat_expr.type_annotation,
                "satisfies",
                expr.span.start,
            ),
            ExpressionKind::TSInstantiationExpression(inst_expr) => {
                self.build_ts_instantiation_doc(inst_expr, expr.span)
            }
            ExpressionKind::TSNonNullExpression(non_null_expr) => {
                self.is_expression_statement.set(was_expr_stmt);
                self.build_ts_non_null_doc(non_null_expr, expr.span)
            }
            ExpressionKind::ImportExpression(import_expr) => {
                self.build_import_expression_doc(import_expr, expr.span)
            }
            ExpressionKind::MetaProperty(meta) => self.build_meta_property_doc(meta),
            ExpressionKind::TSParameterProperty(param_prop) => {
                self.build_ts_parameter_property_doc(param_prop, expr.span)
            }
            ExpressionKind::JsdocCast(cast) => self.build_jsdoc_cast_doc(cast, expr.span),
            // Preserved grouping parens — see [`Self::build_preserved_paren_doc`].
            ExpressionKind::ParenthesizedExpression(paren) => {
                self.is_expression_statement.set(was_expr_stmt);
                self.build_preserved_paren_doc(paren, expr.span)
            }
        }
    }

    /// Build a Doc for a preserved grouping pair (`ParenthesizedExpression`).
    ///
    /// The pair is layout-transparent: the inner renders and re-derives whatever parens
    /// it needs, matching prettier, which strips redundant parens and re-adds required
    /// ones. Only the wire AST keeps the node, and a Svelte `{#snippet}` parameter is the
    /// one place tsv parses one.
    ///
    /// ⚠️ **The erased pair still brackets TWO gaps.** Rendering only the inner left both
    /// with no emitter anywhere — the node's span hides them from whatever encloses it,
    /// and the inner's span ends before them (`docs/comments.md` hazard 4; the leading
    /// half was masked by ownership, a glued block surviving inside the inner's own doc).
    /// They take the same stripped-shell emitters the TypeScript path's own erased parens
    /// take, which is the whole target here: **a snippet parameter list must format
    /// identically to the equivalent function signature.**
    ///
    /// Both halves answer to the REPARSE, where the pair is gone and the enclosing gap
    /// reads the run instead:
    ///
    /// - The `(`→inner run takes the keyword→value gate's split
    ///   ([`Printer::comments_force_own_line_between`]: hang, or trail inline), the only
    ///   split that reproduces what that gap settles on. Emitting it at its authored
    ///   separators kept an own-line block's hardline against a value that pulls up, which
    ///   is not idempotent on its own output. The hang is
    ///   [`Printer::build_value_slot_continuation_indent`], never the keyword seam
    ///   ([`Printer::append_keyword_value_line_comments`]): the slot's separator is someone
    ///   else's, so that seam's own leading hardline would be a second break.
    /// - The inner→`)` run defers, and a deferred run must not leave the construct it was
    ///   written in — this pair prints no closer of its own to end the line, so a `//`
    ///   rode out past the whole `{#snippet …}` head and re-parsed there as template TEXT.
    ///   The flush-scoped break opens just the group the suffix flushes in, the parameter
    ///   list, which is where the TypeScript twin lands it too. It is gated on the
    ///   emitter's own report of whether it deferred: forcing the break for an inline
    ///   block opens a list that had no reason to open, and the reparse closes it again.
    fn build_preserved_paren_doc(
        &self,
        paren: &crate::ast::internal::ParenthesizedExpression<'_>,
        span: Span,
    ) -> DocId {
        let d = self.d();
        let inner_span = paren.expression.span();
        let inner = self.build_expression_doc(paren.expression);
        let inner = if self.comments_force_own_line_between(span.start, inner_span.start) {
            self.build_value_slot_continuation_indent(span.start, inner_span.start, inner)
        } else {
            match self
                .build_inline_comments_between_doc_trailing_space_opt(span.start, inner_span.start)
            {
                Some(run) => d.concat(&[run, inner]),
                None => inner,
            }
        };
        if !self.has_comments_to_emit_between(inner_span.end, span.end) {
            return inner;
        }
        let mut parts: DocBuf = smallvec![inner];
        if self.append_trailing_paren_comments(&mut parts, inner_span.end, span.end) {
            parts.push(d.flush_break());
        }
        d.concat(&parts)
    }

    /// The cast's owned comment plus the separator it owes its own `(`.
    ///
    /// A cast's comment is `owned_by_node`, so every gap emitter and layout predicate
    /// skips it and this is the one place it is printed — which makes this method
    /// prettier's `printLeadingComment` for it, the same three-way rule
    /// [`Printer::push_leading_comment_run`] applies to every other leading comment:
    ///
    /// - **hardline** when the author isolated the comment on a line of its own
    ///   ([`Printer::jsdoc_cast_comment_own_line`]). That predicate also drives the enclosing
    ///   assignment to hang, so the `(` lands indented under it — the two must agree.
    /// - **space** when something follows the `*/` on its line
    ///   ([`Printer::comment_hugs_next`]) — the `(` itself, or a comment before it.
    /// - **soft `line`** otherwise: the author broke after the `*/`, and whether that
    ///   break survives is the enclosing group's to decide. A call argument that fits
    ///   pulls the `(` back up (matching prettier); a STATEMENT, whose list keeps lines,
    ///   keeps the break.
    ///
    /// That last arm is what a bare space could not express, and it is why a plain glued
    /// run (`/* c1 */ /* c2 */⏎b;`) and a bundler annotation both kept the author's break
    /// at statement position while a cast — alone among owned comments — collapsed it.
    ///
    /// Two gap categories override the default with a **rule**, via span-keyed printer
    /// cells the gap sets before the value is built. Both are read HERE rather than by
    /// the caller, and the caller must therefore invoke this before it builds the inner
    /// (see [`Printer::build_jsdoc_cast_doc`]):
    ///
    /// - a **value gap** is not one of the soft `line`'s gaps: there the break is answered
    ///   by rule, not by width ([`Printer::jsdoc_cast_value_gap_target`]), so it reflows to
    ///   the space and both authorings reach the one fixed point every wide cast already
    ///   takes. Deciding it by width instead broke the gap without hanging the value,
    ///   landing the `(` at the statement's own indent.
    /// - a **cannot-hang** gap (a Svelte braced head,
    ///   [`Printer::jsdoc_cast_cannot_hang_target`]) outranks even the hardline arm: the
    ///   host has no operator line to end, so the hardline the own-line authoring earns
    ///   everywhere else would strand the `(` at the head's own column — pass 1 forces the
    ///   head open, pass 2 reads the comment as mid-line and collapses it, no fixed point.
    ///   The reflow lands both authorings on the one-line form the glued authoring already
    ///   reaches. See docs/conformance_prettier_svelte.md §Svelte: Own-line JSDoc cast at
    ///   a braced head.
    ///
    /// ⚠️ The two arms that KEEP the author's break also keep the author's **blank** below
    /// it, through the same [`Printer::push_blank_preserving_separator`] every other
    /// leading comment's separator goes through. Ownership is why that has to be said at
    /// all — no gap emitter measures this gap, so the blank has no other claimant — but
    /// ownership decides who PRINTS the comment, never whether the gap below it is
    /// authoring, and prettier's `printLeadingComment` appends its blank hardline after
    /// all three separators alike. The two REFLOW arms are the exception by construction:
    /// a reflow says the break was layout rather than authoring, so the blank inside it
    /// goes with it. Pinned by `typescript/syntax/comments/jsdoc_type_cast_leading_run_blank`.
    fn build_jsdoc_cast_lead_doc(
        &self,
        cast: &crate::ast::internal::JsdocCast<'_>,
        span: Span,
    ) -> DocId {
        let d = self.d();
        let open = span.start; // the `(`
        let comment_end = cast.comment.span.end;
        // The cast scan accepts only ASCII whitespace back from the `(`
        // (`source_scan::block_comment_end_before` with `CommentGlue::AnyLine`), so this
        // gap can hold no comment for a blank scan to cross: the strict question is the
        // whole answer here, with no `blank_scan_end` ceiling to take.
        debug_assert!(
            self.comments_in_source_between(comment_end, open)
                .next()
                .is_none(),
            "a JSDoc cast's comment→`(` gap is whitespace-only by construction"
        );
        // The array's entry, not the node's copy: a nestled predecessor merges INTO the
        // cast's comment, and printing the copy would drop it ([`Self::jsdoc_cast_comment`]).
        // `jsdoc_cast_comment_own_line` below resolves the same entry again rather than
        // taking this one — the resolution stays inside that predicate so its two other
        // callers, which hold only the cast, cannot reach a different answer. Deliberate:
        // one predicate is worth more than one saved lookup on a path this rare.
        let comment = self.jsdoc_cast_comment(cast, span);
        let mut parts: DocBuf = smallvec![self.build_comment_doc(&comment)];
        if self.jsdoc_cast_in_cannot_hang_gap(span) {
            parts.push(d.text(" "));
        } else if self.jsdoc_cast_comment_own_line(cast, span) {
            self.push_blank_preserving_separator(&mut parts, comment_end, open, d.hardline());
        } else if self.comment_hugs_next(&comment) || self.jsdoc_cast_in_value_gap(span) {
            parts.push(d.text(" "));
        } else {
            self.push_blank_preserving_separator(&mut parts, comment_end, open, d.line());
        }
        d.concat(&parts)
    }

    /// Build a Doc for a JSDoc type cast: `/** @type {T} */ (inner)`.
    ///
    /// The cast **owns** its leading `@type`/`@satisfies` comment (the parser sets
    /// `Comment::owned_by_node`, so every gap emitter and layout predicate skips it)
    /// and prints it here, glued to its own `(`. That gluing is the correctness
    /// property: the comment and the `(` together *are* the cast, so a paren the
    /// printer synthesizes around an *enclosing* expression — `??` clarity parens
    /// under a ternary, a member/call base, a `new` callee, a `**` operand — lands
    /// **outside** the pair instead of between them. Printed from the enclosing gap
    /// instead (prettier's model, and its bug), that paren separates the two and the
    /// cast silently re-binds to the wider expression on reparse. See
    /// `docs/conformance_prettier_ts_comments.md` §JSDoc / paren semantics.
    ///
    /// A **nested** cast's comment lands the same way — the inner `JsdocCast` prints
    /// its own (`/** @type {A} */ (/** @type {B} */ (expr))`) — so the interior gap
    /// below carries only ordinary comments. The inner is built bare: the cast's own
    /// parens provide grouping, so `needs_parens` must not double-wrap it.
    ///
    /// Layout follows prettier-plugin-svelte's `ParenthesizedExpression`: an
    /// object/array inner **hugs** the parens (`({…})` / `([…])`), every other
    /// inner gets a breakable group (`(inner)` flat, `(⏎\tinner⏎)` when wide). A
    /// line comment in the gap forces a hardline layout so it can't swallow the
    /// inner and the closing `)`.
    fn build_jsdoc_cast_doc(
        &self,
        cast: &crate::ast::internal::JsdocCast<'_>,
        cast_span: Span,
    ) -> DocId {
        let d = self.d();
        let open = cast_span.start; // the `(`
        // Built BEFORE the inner, and that ordering is the reason it is a call and not a
        // block down where its doc is used: the gap marks it reads name ONE node, and the
        // inner may hold value gaps of its own (an object property, an arrow body) whose
        // own mark overwrites them — so an answer taken after the recursion below is the
        // innermost gap's, not this cast's, and every cast wrapping such a value lost its
        // reflow.
        let lead = self.build_jsdoc_cast_lead_doc(cast, cast_span);
        // Rule A inside the cast's own parens: a directive alone on its line in the
        // `(`→inner gap freezes the INNER verbatim, with the cast's comment and parens
        // printing around the frozen slice. Freezing the paren-stripped inner rather than
        // the whole shell is the same choice the type side makes
        // (`paren_interior_routed_inner`), and it is what keeps the shell's own comments
        // with the shell's emitters below. Prettier agrees on both the scope and the
        // preserved parens here.
        let frozen_inner = self.gap_frozen_span(open + 1, cast.inner.span());
        // A sequence inner rides this pair BARE (below), and the bare builder scans neither
        // edge of its operand run: the leading edge — the `(` the parser stripped from the
        // first operand and any comment inside it — and the gap past the last operand are
        // this cast's to emit, exactly as the restricted production's hanging pair emits
        // them (`build_restricted_production_paren_doc`). Scanned to the inner's own span
        // instead, a comment in either operand shell reached no emitter (`gaps:audit`).
        // ⚠️ Only for the UNFROZEN sequence: a frozen slice is the inner's whole span,
        // shells and their comments included, so the edges there are the span's own —
        // scanned from the operand run, a shell comment printed twice.
        let (lead_end, trail_start) = match (cast.inner, frozen_inner) {
            (
                Expression {
                    kind: ExpressionKind::SequenceExpression(seq),
                    ..
                },
                None,
            ) => operators::sequence_operand_run(seq),
            _ => (cast.inner.span().start, cast.inner.span().end),
        };
        let inner_doc = frozen_inner.map_or_else(
            || match &cast.inner.kind {
                // A sequence self-parenthesizes at every other position; here the cast's
                // pair IS its required pair, so it rides that one bare — the same rule the
                // restricted productions' hanging pair takes (`build_sequence_doc_bare`).
                // Built with its own pair it printed `/** @type {A} */ ((b, c))`, and the
                // reparse then read a cast around a redundant shell
                // (`jsdoc_type_cast_sequence`).
                ExpressionKind::SequenceExpression(seq) => {
                    self.build_sequence_doc_bare(seq, cast.inner.span, trail_start)
                }
                _ => self.build_expression_doc(cast.inner),
            },
            // Built bare, like the ordinary arm: the cast's parens already group it. The
            // owned-comment claim still applies — a block glued before the inner rides
            // inside its doc, which the slice replaces.
            |frozen| self.build_frozen_expression_doc(cast.inner, frozen),
        );

        let with_lead = |paren_doc: DocId| d.concat(&[lead, paren_doc]);

        // The cast synthesizes its own `(`…`)`, so the gap between the inner expression
        // and the closing `)` is ITS gap to claim — nothing outside can see between those
        // parens. Left unemitted, every comment there was silently DROPPED. Shares the
        // emitter with the stripped-paren restorer (`trailing_paren_comment_parts`).
        let (trailing_parts, trailing_needs_break) = self
            .trailing_paren_comment_parts(trail_start, cast_span.end)
            .unwrap_or_else(|| (DocBuf::new(), false));

        // A line comment on either side of the inner must force a hardline layout —
        // otherwise `(// c <inner>)` runs the inner and the `)` into the comment
        // (silent content loss). Mirrors `build_expression_doc_keep_paren_comments`.
        // A frozen inner joins the two content-loss triggers: the block spelling of an
        // honored directive would otherwise collapse onto the frozen value's line, which
        // is a glued — hence inert — placement, so the freeze would be lost on the second
        // pass. (The line spelling already lands here via the scan above.)
        if self.has_line_comments_between(open + 1, lead_end)
            || trailing_needs_break
            || frozen_inner.is_some()
        {
            let mut parts: DocBuf = smallvec![d.hardline()];
            for comment in self.comments_to_emit_between(open + 1, lead_end) {
                parts.push(self.build_comment_doc(comment));
                // A line comment runs to end-of-line, so it must break; a block
                // comment hugs the next token inline (`/** @type {B} */ (x)`) — unless
                // it is an honored directive, which must keep its own line for the
                // freeze to survive the next pass.
                parts.push(if comment.is_block && !self.is_honored_directive(comment) {
                    d.text(" ")
                } else {
                    d.hardline()
                });
            }
            parts.push(inner_doc);
            parts.extend(trailing_parts);
            return with_lead(d.concat(&[
                d.text("("),
                d.indent(d.concat(&parts)),
                d.hardline(),
                d.text(")"),
            ]));
        }

        // Ordinary comments between this cast's `(` and the inner expression — all
        // block comments here, so they hug inline. A nested cast's own `@type` comment
        // is NOT among them: that cast owns it and prints it itself.
        let interior = self.build_comments_between(open + 1, lead_end, CommentSpacing::Trailing);
        let mut body_parts: DocBuf = smallvec![interior, inner_doc];
        body_parts.extend(trailing_parts);
        let body = d.concat(&body_parts);

        // Object/array literals hug the parens; the inner's own group breaks it.
        if matches!(
            cast.inner.kind,
            ExpressionKind::ObjectExpression(_) | ExpressionKind::ArrayExpression(_)
        ) {
            with_lead(d.concat(&[d.text("("), body, d.text(")")]))
        } else {
            with_lead(d.group(d.concat(&[
                d.text("("),
                d.indent(d.concat(&[d.softline(), body])),
                d.softline(),
                d.text(")"),
            ])))
        }
    }

    /// Build doc for function parameter expression, using FunctionParameter context for patterns
    pub(super) fn build_function_parameter_doc(&self, expr: &Expression<'_>) -> DocId {
        match &expr.kind {
            ExpressionKind::ObjectPattern(obj) => self.with_param_decorators(
                obj.decorators,
                self.build_object_pattern_doc_with_context(
                    obj,
                    expr.span,
                    PatternContext::FunctionParameter,
                ),
                expr.span.start,
            ),
            // For other expressions, use normal doc building
            _ => self.build_expression_doc(expr),
        }
    }

    /// Build doc for expression in call argument or array element context
    ///
    /// Binary/logical expressions get continuation indent when they break:
    /// ```text
    /// fn(
    ///     aaa &&
    ///         bbb,  // extra indent on continuation
    /// )
    /// ```
    ///
    /// Assignment expressions are wrapped in parens for clarity:
    /// `fn((a = b))` not `fn(a = b)`
    pub(super) fn build_arg_expression_doc(&self, expr: &Expression<'_>) -> DocId {
        // Member-chain arg-doc sharing: a chain builds the same group flat and expanded
        // across `conditional_group` candidates; reuse the one build instead of
        // re-recursing (kills the O(4^depth) rebuild — see the `chain_arg_share_active`
        // field doc). The key carries the builder and the `expandLastArg` state, so a hit
        // is byte-identical to a rebuild.
        let key = self.chain_share_key(expr, ShareTag::ArgExpression);
        self.chain_shared_doc(key, || self.build_arg_expression_doc_uncached(expr))
    }

    fn build_arg_expression_doc_uncached(&self, expr: &Expression<'_>) -> DocId {
        self.build_value_with_outermost_owned_comment(expr, || {
            self.build_arg_expression_doc_body(expr)
        })
    }

    /// [`Self::build_arg_expression_doc_uncached`] under the seam's owned-comment claim.
    ///
    /// ⚠️ **The two binaryish arms below REASSEMBLE the chain** — they reach past
    /// `build_expression_doc`, so the owned-comment seam never runs for the argument node
    /// itself and the comment is claimed by the chain's leftmost LEAF instead, *inside* the
    /// chain's group. A multi-line one then force-breaks a chain prettier keeps flat
    /// (`calls/arg_multiline_block_comment_flat`,
    /// `expressions/arrays/element_multiline_block_comment_flat` — array elements share this
    /// builder). The wrapper above is the claim those arms owe, stated once here rather than
    /// per arm.
    fn build_arg_expression_doc_body(&self, expr: &Expression<'_>) -> DocId {
        let d = self.d();
        // Assignment expressions need parens in argument context for clarity
        if self.needs_parens(expr, ParenContext::Argument) {
            return d.parens(self.build_expression_doc(expr));
        }

        match &expr.kind {
            // A `Boolean()` coercion's lone argument — the one argument position in
            // prettier's `shouldNotIndent` — arrives marked by its call printer
            // (`Printer::mark_flat_chain`, set at both call-argument entry points), and
            // takes the flat layout through the seam that owns the owned-comment prepend.
            // Read here, at the builder every argument layout shares, rather than at each
            // layout arm: the arms that named a flat builder themselves covered one layout
            // in six.
            ExpressionKind::BinaryExpression(binary)
                if self.flat_chain_target.get() == Some(expr.span) =>
            {
                self.build_flat_chain_expression_doc(expr)
            }
            ExpressionKind::BinaryExpression(binary) => {
                // NOT an opt-in to continuation indent — that is the default now
                // ([`Printer::build_binary_chain_doc`]). This builder differs in GROUP
                // STRUCTURE: it puts each continuation directly in the outer group, so a
                // break forced there breaks every operand, where the default's sub-group
                // lets the continuation re-evaluate and stay flat. Collapsing the two was
                // tried and measured: the whole fixture corpus stayed green and
                // `prettier/tests/format/js/comments/binary-expressions-parens.js` moved
                // match → unknown, where a multiline block comment ahead of the argument
                // forces the break prettier propagates through the chain.
                self.build_binary_chain_doc_indented(binary, expr.span)
            }
            ExpressionKind::ConditionalExpression(cond) => {
                // Ternary in call/new args: binary expressions in branches use
                // continuation indent. Matches Prettier's shouldNotIndent = false
                // when grandparent is CallExpression/NewExpression (`shouldNotIndent`, `print/binaryish.js`).
                self.build_conditional_doc_with_binary_test_indent(cond, expr.span)
            }
            // For other expressions, use normal doc building
            _ => self.build_expression_doc(expr),
        }
    }

    /// Build a Doc for an expression with forced expansion (hardlines).
    ///
    /// Used by chain arg formatting when we need the object/array to expand
    /// internally with hardlines so fits() can correctly measure the first line.
    /// For example, `.fn({prop})` should become `.fn({\n  prop,\n})` when expanded.
    ///
    /// ⚠️ The two literal arms reach their builders directly rather than through
    /// [`Self::build_expression_doc`], so the owned-comment seam is theirs to discharge
    /// (`docs/comments.md` hazard 1): a block glued to the `{`/`[` is bound to that node and
    /// printed by its doc, and `build_object_doc_expanded`'s own comment gate asks only about
    /// the object's INTERIOR — a comment one byte before `span.start` is outside every window
    /// it consults, so without the prepend it is DROPPED. The `_` arm needs none:
    /// `build_arg_expression_doc` routes there.
    pub(super) fn build_arg_expression_doc_expanded(&self, expr: &Expression<'_>) -> DocId {
        match &expr.kind {
            ExpressionKind::ObjectExpression(obj) => self.prepend_owned_leading_comment(
                expr,
                self.build_object_doc_expanded(obj, expr.span),
            ),
            ExpressionKind::ArrayExpression(arr) => self
                .prepend_owned_leading_comment(expr, self.build_array_doc_expanded(arr, expr.span)),
            // For other expressions, use normal doc building
            _ => self.build_arg_expression_doc(expr),
        }
    }

    //
    // TypeScript Type Assertions
    //

    /// Build a Doc for a TypeScript angle-bracket type assertion: `<Type>expr`
    fn build_ts_type_assertion_doc(
        &self,
        type_assert: &crate::ast::internal::TSTypeAssertion<'_>,
        type_assert_span: Span,
    ) -> DocId {
        let d = self.d();
        let expr_needs_parens =
            self.needs_parens(type_assert.expression, ParenContext::AngleBracketAssertion);
        // Cast boundary positions: `<` … type … `>` … expression. The `>` is found
        // past any comment that itself contains a `>` (`<T /* > */>`).
        let open_pos = type_assert_span.start; // the `<`
        let angle_end = open_pos + 1; // after `<`
        let type_start = type_assert.type_annotation.span().start;
        let type_end = type_assert.type_annotation.span().end;
        let expr_start = type_assert.expression.span().start;
        let close_angle = self.find_assertion_close_angle(type_end, expr_start);
        // An alone-on-line format-ignore directive in the `<`→type gap freezes a
        // non-composite cast type verbatim (`single_child_frozen`; a composite
        // declines and freezes via its own leading-run walk). The broken-cast path
        // below already keeps the directive own-line (`build_leading_comments_multiline`),
        // so the freeze slots in as the type doc.
        let type_doc = if self.single_child_frozen(angle_end, type_assert.type_annotation) {
            self.build_frozen_single_child_doc(type_assert.type_annotation)
        } else {
            self.build_type_doc(type_assert.type_annotation)
        };

        // Comments in the cast stay where the author wrote them. Block comments hug
        // inline (`</* c */ T>`, `<T /* c */>`, `<T>/* c */ expr`); a `//` runs to
        // end-of-line, so it forces the cast to break — and where prettier relocates
        // it across the `<`/`>` boundary, tsv preserves position. See
        // conformance_prettier_ts_comments.md §Comment relocation (Angle-bracket type assertion)
        // and the `type_assertion_line_comment` /
        // `type_assertion_close_own_line_comment` divergence fixtures.
        let cast_doc = if self.has_line_comments_between(angle_end, type_start)
            || self.has_line_comments_between(type_end, close_angle)
        {
            self.build_assertion_broken_cast(open_pos, type_start, type_end, close_angle, type_doc)
        } else {
            // Mirror Prettier's `printTypeAssertion`: the cast `<Type>` is its own
            // group, breaking after `<` with the type on an indented line and `>`
            // back at the outer indent. Crucially, a union cast type prints *flat* on
            // that line — Prettier's `shouldIndentUnionType` returns false for
            // `TSTypeAssertion`, so it never gets the leading-`|` hanging indent that
            // `as`/`satisfies` casts use (see `build_union_hanging_indent_doc`).
            // The `<`→type gap takes the RUN-level reading a keyword→value gap takes: a
            // broke-after run holding a multi-line block
            // ([`Printer::broke_after_run_holds_multiline_block`]) cannot print flat, so
            // every separator the author broke opens and the type drops below the run —
            // which is prettier's own form here, at the bare authoring too. The
            // per-comment `Trailing` spacing below answers with a space for both halves of
            // `/* a⏎b */ /* c */⏎T` (the first is glued, the second single-line) and welded
            // the type onto the run's closing line; the shelled twin, whose stripped shell
            // hands this gap the same run, then converged only on its second pass.
            let comments_doc = if self.broke_after_run_holds_multiline_block(angle_end, type_start)
            {
                self.build_trailing_comments_hang_next(angle_end, type_start)
            } else {
                self.build_comments_between(angle_end, type_start, CommentSpacing::Trailing)
            };
            let before_close_doc = self.build_comments_between_filtered(
                type_end,
                close_angle,
                CommentSpacing::Leading,
                CommentFilter::BlockOnly,
            );
            d.group(d.concat(&[
                d.text("<"),
                d.indent(d.concat(&[d.softline(), comments_doc, type_doc, before_close_doc])),
                d.softline(),
                d.text(">"),
            ]))
        };

        // The operand→`)` gap: what the author wrote inside the operand's grouping
        // shell, before the assertion's own end (`<T>(x /* c */)`). The parser strips
        // such a shell, and the assertion's span is what ends at that `)` — so no
        // enclosing gap can reach these comments and nothing printed them. tsv RETAINS
        // the shell for them, on the criterion the `as`/`satisfies` operand shell
        // already uses: a shell is redundant only when the stripped form can still
        // express the comment's position, and here it cannot — stripping hands the
        // comment to the statement's terminator gap, which puts it after the `;`
        // (`<T>x; /* c */`), no longer trailing the operand it was written against, and
        // not even a fixed point on the way there. An empty gap still strips, so the
        // retention is the comment's doing (`build_paren_operand_comment_doc` → `None`).
        // See conformance_prettier_ts_comments.md §Comment relocation
        // (Angle-bracket assertion operand shell).
        let operand_gap_start = type_assert.expression.span().end;
        let operand_gap_end = type_assert_span.end;
        let retains_shell = self.has_comments_to_emit_between(operand_gap_start, operand_gap_end);

        // A retained shell is a *breaking* paren, so its operand takes the continuation
        // indent one gives — the non-null operand's rule. Without it a binary operand's
        // continuation lines snap back to the enclosing indent, outside the `(` that is
        // now being printed around them.
        let build_inner = || {
            if retains_shell {
                self.build_expression_doc_with_indent_on_break(type_assert.expression)
            } else {
                self.build_expression_doc(type_assert.expression)
            }
        };
        // A MULTI-LINE block the operand OWNS prints just inside the `(`, outside the
        // operand's own group ([`Printer::build_value_with_outermost_owned_comment`]).
        // BOTH pairs count: the precedence one below and the shell the operand→`)` gap
        // retains — either encloses the comment, and the shell's body is the one the
        // `retains_shell` branch prints, so a claim taken only for `expr_needs_parens`
        // would miss `<T>(/* c⏎d */ b + c + d /* t */)`.
        let inner_expr = if retains_shell || expr_needs_parens {
            self.build_value_with_outermost_owned_comment(type_assert.expression, build_inner)
        } else {
            build_inner()
        };
        let expr_doc = if expr_needs_parens {
            d.parens(inner_expr)
        } else {
            inner_expr
        };

        // A line comment after `>` drops the expression to a continuation line one
        // indent in (prettier instead relocates it into the cast — a divergence).
        // Each comment holds its position: a same-line `<T> // c` trails the `>`,
        // an own-line comment keeps its own line leading the expression
        // (`build_trailing_gap_comments`). A block comment in the gap stays
        // inline ahead of the expression.
        //
        // TODO: a binary-expression operand that *breaks across lines* misaligns — its
        // first operand sits at this `indent` level but the chain's continuation `line`s
        // snap back to the enclosing assignment-level indent (binary chains take their
        // continuation indent from the parent context, not a nested `indent`), so
        // `aaaa ||` lands one level deeper than `bbbb ||`. Idempotent and lossless, and
        // only reachable via cast + after-`>` line comment + a wrapping binary operand
        // (absent from any real corpus). A real fix means threading parent-indent context
        // into the chain printer — out of scope here. No fixture guards it on purpose:
        // `input.svelte` must be idempotent, so it could only bake the misaligned output
        // in as canonical, sanctioning a known imperfection as a deliberate divergence —
        // which the fixture rules forbid. The fix is to align it, after which an ordinary
        // fixture follows.
        if self.has_line_comments_between(close_angle + 1, expr_start) {
            let trailing = self.build_trailing_gap_comments(close_angle + 1, expr_start);
            // The retained shell supplies the operand's pair, so its body is the bare
            // expression — `expr_needs_parens` would make it a second one. The
            // `>`→operand comments are already in `trailing`, so they stay out of it.
            let operand_doc = self
                .build_paren_operand_comment_doc(
                    operand_gap_start,
                    operand_gap_end,
                    inner_expr,
                    inner_expr,
                    ")",
                )
                .unwrap_or(expr_doc);
            return d.concat(&[
                cast_doc,
                d.indent(d.concat(&[d.concat(&trailing), d.hardline(), operand_doc])),
            ]);
        }

        // A block comment after `>` leads the expression in every layout branch
        // below, so fold it onto the cast once rather than into each branch.
        let after_close_doc = self.build_comments_between_filtered(
            close_angle + 1,
            expr_start,
            CommentSpacing::Trailing,
            CommentFilter::BlockOnly,
        );

        // A retained shell carries its own parens, so it bypasses the cast's break
        // shell below rather than nesting inside it. The `>`→operand comments go
        // INSIDE it, where the author wrote them (`<T>(/* c */ x /* d */)`) — emitting
        // them onto the cast would move them out of the very parens being kept.
        // `None` falls THROUGH to the layouts below rather than defaulting to the body:
        // it means the gap had nothing to emit, and the body alone would have skipped
        // both `expr_needs_parens` and the cast's break shell.
        if retains_shell {
            // The operand has one rendering here, so both layouts take the same body.
            let body = d.concat(&[after_close_doc, inner_expr]);
            if let Some(shell) = self.build_paren_operand_comment_doc(
                operand_gap_start,
                operand_gap_end,
                body,
                body,
                ")",
            ) {
                return d.concat(&[cast_doc, shell]);
            }
        }

        let cast_group = d.concat(&[cast_doc, after_close_doc]);

        // `shouldBreakAfterCast`: object/array-literal expressions hug the cast
        // (they expand themselves), everything else may break the expression into
        // its own parenthesized block before the cast group itself breaks.
        let should_break_after_cast = !matches!(
            type_assert.expression.kind,
            ExpressionKind::ArrayExpression(_) | ExpressionKind::ObjectExpression(_)
        );

        if should_break_after_cast {
            let expr_contents = d.group_break(d.concat(&[
                d.if_break(d.text("("), d.empty()),
                d.indent(d.concat(&[d.softline(), expr_doc])),
                d.softline(),
                d.if_break(d.text(")"), d.empty()),
            ]));
            d.conditional_group(&[
                d.concat(&[cast_group, expr_doc]),
                d.concat(&[cast_group, expr_contents]),
                d.concat(&[cast_group, expr_doc]),
            ])
        } else {
            d.group(d.concat(&[cast_group, expr_doc]))
        }
    }

    /// Build a forced-break cast `<Type>` that preserves line comments in place.
    ///
    /// Used when a `//` sits between `<` and the type, or between the type and `>`
    /// — it runs to end-of-line, so the cast can't stay inline. Each comment holds
    /// the position the author gave it: a same-line `< // c` is pulled onto the `<`
    /// line (`delimiter_line_comment_prefix`, the open-delimiter family — prettier
    /// relocates it to its own line); own-line comments after `<` sit on their own
    /// lines; a trailing-type `T // c` stays on the type line and an own-line
    /// comment before `>` keeps its own line (`build_trailing_gap_comments`).
    /// See conformance_prettier_ts_comments.md §Comment relocation (Angle-bracket type assertion).
    ///
    /// Positions are the caller's already-computed cast boundaries, in source order:
    /// `open_pos` is the `<`, `type_start`/`type_end` bound the type, `close_angle`
    /// is the `>`. Taking them explicitly keeps this a pure doc assembler with no
    /// `TSTypeAssertion` dependency or re-derivation.
    fn build_assertion_broken_cast(
        &self,
        open_pos: u32,
        type_start: u32,
        type_end: u32,
        close_angle: u32,
        type_doc: DocId,
    ) -> DocId {
        let d = self.d();
        let angle_end = open_pos + 1; // after `<`
        let (angle_prefix, angle_pull_pos) =
            self.delimiter_line_comment_prefix(open_pos, type_start);
        let leading = self.build_leading_comments_multiline(angle_end, type_start, angle_pull_pos);
        let trailing = self.build_trailing_gap_comments(type_end, close_angle);
        // `group_break`, not a bare concat: this is the already-broken form, and saying
        // so contains the leading run's soft `line` (prettier's `printLeadingComment`)
        // here rather than letting it escape to the enclosing assignment group — which
        // would measure it against that group and wrongly pull the type up onto the
        // comment's line. Prettier gives the cast no per-element group, so the `line`
        // rides the broken group and breaks. The sibling angle/paren lists
        // (type params, type args, function-type params) are already grouped by their
        // own callers, which is why they land on the break without this.
        d.group_break(self.build_delimited_doc(
            d.text("<"),
            angle_prefix,
            d.indent(d.concat(&[
                d.hardline(),
                d.concat(&leading),
                type_doc,
                d.concat(&trailing),
            ])),
            d.hardline(),
            d.text(">"),
        ))
    }

    /// Build a Doc for a TypeScript binary cast expression — `expr as Type` or
    /// `expr satisfies Type`. Mirrors Prettier's `printBinaryCastExpression`,
    /// which prints both with one function (`isSatisfiesExpression ? "satisfies" : "as"`).
    ///
    /// `keyword` is the bare keyword (`"as"` / `"satisfies"`).
    ///
    /// Comments are preserved where the author wrote them — between the
    /// expression and the keyword, and between the keyword and the type. `as
    /// const` is no exception: its `const` is treated like any other cast type.
    /// (Prettier relocates an `as const` inner comment before the whole
    /// expression; tsv keeps it in place — a documented divergence.)
    fn build_binary_cast_doc(
        &self,
        expression: &Expression<'_>,
        type_annotation: &TSType<'_>,
        keyword: &'static str,
        cast_start: u32,
    ) -> DocId {
        let d = self.d();
        let mut parts = d.pooled_docbuf();

        // A transparent one-member composite is its member here, the `|` dropped and its
        // head gap folded into the keyword→type gap ([`Printer::transparent_value`]).
        let type_annotation = self.transparent_value(type_annotation);
        // Find the keyword position
        let expr_end = expression.span().end;
        let type_start = type_annotation.span().start;
        let keyword_pos = self.find_keyword_in_range(expr_end, type_start, keyword);

        // The operand→keyword gap is ASI-sensitive — `as`/`satisfies` may not start a
        // line — so a comment that spans lines keeps the operand's grouping parens
        // rather than being inlined (`asi_gap_needs_parens`), and the shell's own
        // `(`→operand gap is emitted with it (nothing else does, so it was dropped).
        // The paren frame supplies whatever parens the cast itself needs, so this arm
        // emits no second pair.
        //
        // One gap scan serves both arms. A cast is ubiquitous, and the shell
        // question and the inline emission below would otherwise each open with the
        // same binary search over `[expr_end, kw)`. The shell's two triggers are each implied by a
        // term of the guard: its trailing trigger needs a comment IN that gap, and its
        // leading trigger needs a `(` BEFORE the operand, which needs room for one.
        let gap_has_comments =
            keyword_pos.is_some_and(|kw| self.has_comments_to_emit_between(expr_end, kw));
        let shell = if gap_has_comments || cast_start < expression.span().start {
            keyword_pos.and_then(|kw| {
                self.build_asi_operand_shell_doc(
                    cast_start,
                    expression,
                    kw,
                    ParenContext::TypeAssertion,
                )
            })
        } else {
            None
        };

        if let Some(shell) = shell {
            parts.push(shell);
        } else {
            let needs_parens = self.needs_parens(expression, ParenContext::TypeAssertion);
            if needs_parens {
                parts.push(d.text("("));
            }
            // A ternary operand reached from one of prettier's `ancestorNameMap` value
            // positions expands its parens instead of hanging the `?`/`:` arms — the
            // shape a member base already takes. See `mark_ternary_extra_indent`.
            //
            // Asked BEFORE the operand's doc is built: building it recurses into the
            // ternary's own branches, and a nested value position there (a declarator in
            // an arrow body, an inner `await`) re-marks the target, so a read afterwards
            // would see the nested answer instead of this operand's.
            let expands = needs_parens && self.ternary_takes_extra_indent(expression);
            // A MULTI-LINE block the operand OWNS prints just inside the `(`, outside the
            // operand's own group ([`Printer::build_value_with_outermost_owned_comment`]);
            // left inside, its body's forced break explodes an operand prettier keeps flat.
            // Asked only where the pair is KEPT — see that seam's ⚠️.
            // The cast's own erased-paren freeze: `cast_start` opens at the grouping `(`
            // the parser dropped, so an alone-on-line directive inside it is the cast's to
            // honor ([`Printer::build_left_spine_operand_doc`]).
            let operand = self.build_left_spine_operand_doc(
                cast_start,
                expression,
                FrozenOperandPair::Emitted,
                || {
                    if needs_parens {
                        self.build_expression_doc_claiming_outermost(expression)
                    } else {
                        self.build_expression_doc(expression)
                    }
                },
            );
            parts.push(if expands {
                self.build_expanding_parens_body_doc(operand)
            } else {
                operand
            });
            if needs_parens {
                parts.push(d.text(")"));
            }

            // Comments between expression and keyword → place before the keyword. Skip the
            // `empty()` child on the comment-free `expr as` gap (ubiquitous). Byte-identical.
            if gap_has_comments && let Some(kw_pos) = keyword_pos {
                parts.push(self.build_inline_comments_between_doc(expr_end, kw_pos));
            }
        }

        // A comment between the keyword and the type that can't be inlined forces the
        // type onto the next line, keeping the comment with the cast: a line comment
        // (inlining would let `//` swallow the type — `x as // c A`), or a multiline
        // block comment. A single-line block comment (own-line, trailing, or glued)
        // collapses inline (`x as /* c */ A`). Applies uniformly, including `as const`.
        // See as_satisfies_value_line_comment / as_satisfies_value_own_line_block_comment.
        if let Some(kw_pos) = keyword_pos {
            let kw_end = kw_pos + keyword.len() as u32;
            // An alone-on-line format-ignore directive in the keyword→type gap freezes
            // a non-composite cast type verbatim (`single_child_frozen`; a
            // union/intersection type declines and freezes via its own leading-run
            // walk). The frozen path keeps the UNWIDENED window — an in-shell
            // directive stays on the ordinary paths — and the directive keeps its own
            // line (`append_keyword_value_line_comments` preserves own-line comments;
            // a keyword-trailing placement is inert, so the relocated form would lose
            // the freeze on the second pass). `head.frozen` joins the routing below so
            // a block-spelling alone-on-line directive takes the own-line branch too.
            // A redundant paren shell holding a leading line-comment run (`x as (// c\n A)`,
            // and the double-nested form) strips to the same hang the bare `x as // c\n A`
            // settles on — the shared keyword→value seam routes it so the paren form is
            // idempotent. Without this the gate below measures the OUTER paren and the
            // comment inside it is invisible, so the inline path relocates it at a differing
            // indent (a non-idempotency). A mixed (leading block) or trailing shell hoists
            // losslessly too — the leading run below, the trailing comment via
            // `with_stripped_paren_trailing`.
            let head = self.keyword_value_head(kw_end, type_annotation);
            // A line comment or multiline block hangs the type on its own line; a
            // single-line block comment collapses inline (the fall-through below).
            // Prettier relocates the collapsed comment before the keyword instead.
            //
            // The gate is the shared keyword→value one ([`Printer::keyword_value_hang_doc`]),
            // as at the annotation `:` and the function type's `=>`: a comment that forces
            // its own line, OR a block run the author BROKE AFTER whose break is forced
            // ([`Printer::breaking_value_leading_run`] — a multi-line block anywhere in the
            // run, or a type that breaks on its own). Asking only the first left the run
            // whose multi-line block is glued ahead of a single-line one
            // (`as /* a⏎b */ /* c */⏎T`) to the flat fall-through, which welded the type onto
            // the block's closing line — and once the shell claim widened to that run, the
            // shelled authoring took the hang on pass 1 and the weld on pass 2 (F1).
            // A cast is a value position: a trailing block lifted from the shell defers past
            // the statement `;` (`x as // c\n\tA; /* t */`), matching the declarator's own
            // value→`;` trailing handling.
            let hang_type_doc = if head.frozen {
                Some(self.build_keyword_value_doc(&head, TrailingBlock::Deferred))
            } else {
                self.keyword_value_hang_doc(kw_end, head.value_start, || {
                    self.build_keyword_value_doc(&head, TrailingBlock::Deferred)
                })
            };
            if let Some(type_doc) = hang_type_doc {
                parts.push(d.text(" "));
                parts.push(d.text(keyword));
                self.append_keyword_value_line_comments(
                    &mut parts,
                    kw_end,
                    head.value_start,
                    type_doc,
                );
                return d.concat(&parts);
            }
        }

        // Strip redundant comment-free parens so `(A | B)` / `(A & B)` cast types
        // get the same hanging layout as the bare form (prettier strips them too).
        let value_type = self.unwrap_redundant_parens(type_annotation);

        // Union cast types break after the keyword with a hanging indent.
        if let Some(tail) =
            self.cast_union_hanging_tail(keyword, keyword_pos, value_type, type_start)
        {
            parts.push(tail);
            return d.concat(&parts);
        }

        // Intersection cast types: the first member hugs the keyword, continuations
        // wrap with a hanging indent (mirrors the type-alias / annotation layout).
        if let Some(tail) =
            self.cast_intersection_hanging_tail(keyword, keyword_pos, value_type, type_start)
        {
            parts.push(tail);
            return d.concat(&parts);
        }

        parts.push(d.text(" "));
        parts.push(d.text(keyword));
        parts.push(d.text(" "));

        // Comments between keyword and type → kept in place, trailing the keyword
        // (uniform for every cast type, including `as const`).
        if let Some(kw_pos) = keyword_pos {
            let kw_end = kw_pos + keyword.len() as u32;
            // A cast is a value position. When the type is a redundant paren shell whose
            // trailing gap holds a block comment (`as (Z /* t */)`, `as (/* b */ Z /* t */)`,
            // and the double-nested forms — but no leading *line* comment, which hangs via
            // the branch above), strip the shell and defer that trailing block past the
            // statement `;` (`as Z; /* t */`) via `with_stripped_paren_trailing`, matching
            // the declarator's own value→`;` handling so the paren form is idempotent in one
            // pass. A leading block still trails the keyword inline. Without the defer the
            // cast emits the block before the `;` and the next pass relocates it.
            //
            // ⚠️ A trailing **line** comment declines the strip entirely
            // (`paren_retains_for_trailing_run`): a `//` deferred past the `;` lands on the
            // statement's own trailing line, welding with a comment authored in the
            // declarator's gap (`as (F // c4⏎) // c5` → `as F; // c4 // c5`, the second
            // `//` becoming text of the first). This is the cast's own second path to the
            // strip — the hang seam above is the first — and the retain rule has to be
            // asked at both. The two are genuinely different rules (that one hangs on a
            // leading `//`, this one defers a trailing block past the `;`) sharing one
            // precondition, not a duplicate.
            //
            // The cast peels a shell in a THIRD place — `unwrap_redundant_parens` above,
            // feeding the union / intersection hanging tails — and that one needs no ask:
            // it peels only a shell with no comments in either gap
            // (`paren_inner_comment_flags == (false, false)`), so it can never reach one
            // the retain rule retains. Satisfying the precondition structurally is the
            // stronger form; a fourth path that peels a COMMENTED shell has to ask.
            let inner = unwrap_parenthesized(type_annotation);
            if inner.span() != type_annotation.span()
                && !self.paren_retains_for_trailing_run(type_annotation)
                && self.has_comments_to_emit_between(inner.span().end, type_annotation.span().end)
            {
                for comment in self.comments_to_emit_between(kw_end, inner.span().start) {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.text(" "));
                }
                parts.push(self.build_hang_value_doc(
                    type_annotation,
                    inner,
                    TrailingBlock::Deferred,
                ));
            } else {
                // Skip the `empty()` child on the comment-free `as Type` gap. Byte-identical.
                if self.has_comments_to_emit_between(kw_end, type_start) {
                    parts.push(self.build_comments_between(
                        kw_end,
                        type_start,
                        CommentSpacing::Trailing,
                    ));
                }
                parts.push(self.build_type_doc(type_annotation));
            }
        } else {
            parts.push(self.build_type_doc(type_annotation));
        }

        d.concat(&parts)
    }

    /// The keyword-plus-type tail for an `as`/`satisfies` cast when the cast type
    /// is a non-hugging union: it breaks after the keyword with a hanging indent
    /// (Prettier's `shouldIndentUnionType`). `keyword` is the bare keyword
    /// (`"as"` / `"satisfies"`).
    ///
    /// Returns `None` to fall through to the caller's inline layout — for
    /// non-union or hugging types. A block comment between the keyword and the
    /// union rides inside the hang (`build_union_hanging_indent_doc`): glued to the
    /// first member it declines the hug and lands after the pipe (`x as⏎\t| /* c */
    /// {…}⏎\t| null`, `union_hug_gap_block_comment_cast`), otherwise it leads the union
    /// on its own line. Prettier relocates a block the author broke after across the
    /// keyword (`x /* c */ as {`) — a comment-position divergence tsv keeps.
    fn cast_union_hanging_tail(
        &self,
        keyword: &'static str,
        keyword_pos: Option<u32>,
        type_annotation: &TSType<'_>,
        type_start: u32,
    ) -> Option<DocId> {
        // The discriminant first: it is free, and the overwhelmingly common cast
        // (`x as Foo`) is not a union — it paid a binary search only to learn that.
        // Deliberately a guard here rather than moving the gap search below
        // `build_union_hanging_indent_doc`: that function BUILDS the union doc before
        // it can return, so a reorder would build-then-discard on a commented gap.
        // This mirrors its own first check exactly (a bare match on the type, no paren
        // unwrapping), so it only ever retires work the callee would redo.
        if !matches!(type_annotation, TSType::Union(_)) {
            return None;
        }
        let keyword_len = keyword.len() as u32;
        // The seam's gap opens at the keyword's end; with no keyword position on hand
        // the gap is empty by construction and opens at the type itself.
        let gap_start = keyword_pos.map_or(type_start, |pos| pos + keyword_len);
        let hanging = self.build_union_hanging_indent_doc(gap_start, type_annotation)?;
        let d = self.d();
        Some(d.concat(&[d.text(" "), d.text(keyword), hanging]))
    }

    /// The keyword-plus-type tail for an `as`/`satisfies` cast when the cast type
    /// is an intersection: the first member hugs the keyword, continuation members
    /// wrap with a hanging indent (via the shared `intersection_hanging_with_indent`,
    /// the same layout the type-alias RHS arm uses).
    ///
    /// Returns `None` to fall through to the caller's inline layout — for
    /// non-intersection types, or when a comment sits between the keyword and the
    /// type.
    fn cast_intersection_hanging_tail(
        &self,
        keyword: &'static str,
        keyword_pos: Option<u32>,
        type_annotation: &TSType<'_>,
        type_start: u32,
    ) -> Option<DocId> {
        // Discriminant before the gap search: it is free, and both checks only ever
        // `return None`, so the order is unobservable. The overwhelmingly common cast
        // (`x as Foo`) is not an intersection, and paid a binary search to learn it.
        let TSType::Intersection(i) = type_annotation else {
            return None;
        };
        let keyword_len = keyword.len() as u32;
        if keyword_pos
            .is_some_and(|pos| self.has_comments_to_emit_between(pos + keyword_len, type_start))
        {
            return None;
        }
        let d = self.d();
        let body = self.intersection_hanging_with_indent(i);
        Some(d.concat(&[d.text(" "), d.text(keyword), d.text(" "), body]))
    }

    /// Build a Doc for a TypeScript instantiation expression
    fn build_ts_instantiation_doc(
        &self,
        inst_expr: &crate::ast::internal::TSInstantiationExpression<'_>,
        span: Span,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = DocBuf::new();
        // The shared operand-shell seam also emits the `^`→expression gap — the
        // authored `(`'s interior, which no other emitter reaches.
        parts.push(self.build_shell_operand_doc(
            span.start,
            inst_expr.expression,
            ParenContext::InstantiationExpression,
        ));
        // Preserve comments between expression and type args: `fn/* c */ <string>`
        let expr_end = inst_expr.expression.span().end;
        let ta_start = inst_expr.type_arguments.span.start;
        if let Some(doc) = self.build_name_to_type_params_comments_opt(
            expr_end,
            ta_start,
            CommentSpacing::Trailing,
        ) {
            parts.push(doc);
        }
        parts.push(self.build_type_parameter_instantiation_doc(&inst_expr.type_arguments));
        d.concat(&parts)
    }

    /// Build a Doc for a TypeScript non-null assertion expression
    ///
    /// When wrapping certain expressions in parens (binary, ternary, etc.),
    /// prettier indents continuations when the expression breaks:
    /// ```text
    /// (veryLongExpr ||
    ///     continuation)!
    /// ```
    fn build_ts_non_null_doc(
        &self,
        non_null_expr: &crate::ast::internal::TSNonNullExpression<'_>,
        span: Span,
    ) -> DocId {
        let d = self.d();
        // A comment that SPANS A LINE in the operand→`!` gap RETAINS the shell, even where
        // the parens are otherwise redundant. A `//` deferred instead is carried out of its
        // own statement (`(x + y // c1⏎)!; // c2` → `(x + y)!; // c1 // c2`), where it
        // MERGES with whatever already trails that line and the second comment stops
        // existing — the information-losing relocation §Comment Position Philosophy names
        // as its deciding test, and one the comment census measures directly. A
        // multi-line block inlined puts a real line break before the `!`, which is
        // `[no LineTerminator here]` (`(x /* a⏎b */)!` → `x /* a⏎b */!`, output that
        // does not reparse — the same dead form prettier emits, cataloged). Only a
        // single-line block needs no shell: it trails inline without ending the line.
        let needs_parens = self.needs_parens(non_null_expr.expression, ParenContext::NonNull)
            || self.has_line_spanning_comments_to_emit_between(
                non_null_expr.expression.span().end,
                span.end,
            );

        // The leading run from the operand's authored grouping parens. Where the
        // parens are STRIPPED (`(/* b */ x)!`) it is prepended before the operand,
        // matching prettier (`/* b */ x!`); where they are KEPT, the compact
        // fallback folds it inside the pair (`(/* b */ x + y)!`, where prettier
        // relocates it out front — a cataloged divergence). The expanded arm below
        // re-emits the range itself, with the `(`-glued split, and reads only this
        // doc's presence.
        let argument_start = non_null_expr.expression.span().start;
        let leading = self.build_rhs_comments_opt(span.start, argument_start);

        let core = if needs_parens {
            // For expressions that need parens, use a special doc structure
            // that indents continuations when breaking
            // Same rule as the cast operand: a ternary reached from an `ancestorNameMap`
            // value position expands these parens rather than hanging its arms. Without
            // it `(t)!` and `(t)!.prop` disagree about the very same base. Asked BEFORE
            // the operand is built, for the re-marking reason given at the cast site.
            let expands = self.ternary_takes_extra_indent(non_null_expr.expression);
            // A MULTI-LINE block the operand OWNS prints just inside the `(`, outside the
            // operand's own group ([`Printer::build_value_with_outermost_owned_comment`]).
            // The operand's erased-paren freeze on the RETAINED-pair path too: the pair
            // survives, so the directive keeps its own line inside it and the reparse reads
            // it in the same place ([`Printer::build_left_spine_operand_doc`]).
            let inner_doc = self.build_left_spine_operand_doc(
                span.start,
                non_null_expr.expression,
                FrozenOperandPair::Emitted,
                || {
                    self.build_value_with_outermost_owned_comment(non_null_expr.expression, || {
                        self.build_expression_doc_with_indent_on_break(non_null_expr.expression)
                    })
                },
            );
            let argument_end = non_null_expr.expression.span().end;
            // A leading run that occupies a line — a `//`, an own-line block — takes
            // the family's expanded shell: `( // c` glue, the operand one indent in,
            // the `)!` back out (prettier glues the comment to the `(` and keeps the
            // continuation flush — a rendering-only divergence, the run stays inside
            // the pair in both). A shell whose TRAILING gap also holds comments keeps
            // the anchored-run layout below, the leading run folded above the operand.
            // The shell is hard-expanded, so the ternary's width-driven expanding
            // parens have nothing left to decide and stay out of the body.
            if let Some(shell) = self.build_required_pair_leading_shell_doc(
                span.start,
                argument_start,
                Some((argument_end, span.end)),
                inner_doc,
                ")!",
            ) {
                return shell;
            }
            let inner_doc = if expands {
                self.build_expanding_parens_body_doc(inner_doc)
            } else {
                inner_doc
            };
            // Keep comments from the stripped grouping parens INSIDE them, where the
            // author wrote them — leading before the operand (`(/* b */ x + y)!`),
            // trailing before the `)` (`(x + y /* c */)!`). Prettier relocates them
            // outside (before `(` / between `)` and `!`); tsv preserves the position.
            // The operand has one rendering here, so both layouts take the same body —
            // the folded form only, since the shell above already declined and wants the
            // pre-`expands` body it was given.
            self.build_folded_required_pair_doc(
                (span.start, argument_start),
                Some((argument_end, span.end)),
                inner_doc,
                inner_doc,
                ")!",
            )
        } else if self.has_comments_to_emit_between(non_null_expr.expression.span().end, span.end) {
            // A comment between the operand and `!` (`p?.q /* c */!`, or from stripped
            // grouping parens `(x /* c */)!`) trails the operand — preserve it rather
            // than dropping it. The redundant grouping parens are stripped per tsv's
            // non-null seal canonicalization (`(p?.q)!` → `p?.q!`); prettier keeps them
            // when the source had them.
            //
            // Only a single-line block comment reaches this branch — a `//` or a
            // multi-line block in the gap makes `needs_parens` true above.
            // `ChainNode::NonNull` prints the same gap now, so a block comment would come
            // out identically there; a line-spanning comment in a MID-chain spelling of
            // this gap (`(x // c⏎)!.foo`) is the linearizer's to catch — it
            // retains the shell as a parenthesized base (the same multiline operand
            // layout the required-paren case above uses) rather than flattening the
            // operand into a region whose emitter is block-only.
            let argument_end = non_null_expr.expression.span().end;
            let inner_doc = self.build_expression_doc(non_null_expr.expression);
            let mut parts: DocBuf = smallvec![inner_doc];
            self.append_trailing_paren_comments(&mut parts, argument_end, span.end);
            parts.push(d.text("!"));
            d.concat(&parts)
        } else if Self::is_chain_expression(non_null_expr.expression) {
            // When inner expression is a chain (member or call), use chain architecture
            // to properly handle breaking. This ensures the outer `!` is included
            // in the linearized chain for proper segment grouping.
            let mut nodes = chain::ChainNodeVec::new();
            chain::linearize_chain_from_non_null_into(
                non_null_expr,
                span,
                self.linearize_input(),
                &mut nodes,
            );
            // The third chain ENTRY POINT, and the seam is what carries the head claim
            // every one of them owes ([`chain::build_linearized_chain_doc`]): the base's
            // stripped grouping parens are erased here and the run the author wrote inside
            // them has no other emitter. Reassembled by hand this arm omitted the claim and
            // DROPPED the whole run, and only for a `!` at the chain's OUTERMOST position —
            // every other spelling routes through the member or call door.
            //
            // The window opens at the OPERAND's start, not the non-null's: the region
            // between the two is `leading`'s above (the extra shell of `(( // c⏎a).b())!`),
            // so the two claims partition the head rather than both taking it
            // (`docs/comments.md` hazard 3).
            chain::build_linearized_chain_doc(
                &nodes,
                non_null_expr.expression,
                argument_start,
                span,
                self,
            )
        } else {
            // The OPERAND's erased-paren freeze, over the same region `leading` above emits
            // ([`Printer::build_left_spine_operand_doc`]): with the parens stripped the
            // directive hangs ahead of the operand, which is where the reparse reads it, so
            // the first pass must freeze what the second one would.
            let inner_doc = self.build_left_spine_operand_doc(
                span.start,
                non_null_expr.expression,
                FrozenOperandPair::position(ParenContext::NonNull),
                || self.build_expression_doc(non_null_expr.expression),
            );
            d.concat(&[inner_doc, d.text("!")])
        };

        // For paren-stripped branches the leading comment goes before the operand
        // (parens are gone, matching prettier); the needs_parens branch above already
        // placed it inside the kept parens.
        match leading {
            Some(lead) if !needs_parens => d.concat(&[lead, core]),
            _ => core,
        }
    }

    /// When `expr` is a non-null assertion sealing a parenthesized optional chain
    /// (`(a?.b)!` / `(a?.())!` — the `!` outside the source parens, detected via the
    /// span gap), render it as `(chain)!` with the parens kept. Returns `None` for
    /// any other expression.
    ///
    /// Used in always-required-parens positions (`new` callee, tagged-template tag)
    /// where an optional chain may not appear unsealed (`` a?.b!`x` `` /
    /// `new a?.b!()` are syntax errors). The standalone non-null path strips the
    /// now-redundant parens (`(a?.b)!` → `a?.b!`), so they are restored per-context
    /// here. Normalizes to the `!`-outside form, matching the Sprint-2 sealed-base
    /// rendering (`push_sealed_chain_base` / the chain linearizer's non-null arm).
    pub(crate) fn build_sealed_non_null_paren_doc(&self, expr: &Expression<'_>) -> Option<DocId> {
        let ExpressionKind::TSNonNullExpression(non_null) = &expr.kind else {
            return None;
        };
        if non_null.seals_optional_chain(expr.span) {
            let inner_doc = self.build_expression_doc_with_indent_on_break(non_null.expression);
            let expr_start = non_null.expression.span().start;
            let inner_start = non_null.expression.span().end;
            // The shell's LEADING gap — the seal's span covers the `(`, so the
            // comments between it and the chain (`new ( // c⏎a?.b)!()`) are this
            // doc's to print too; nothing else can reach them. A run that occupies
            // a line takes the family's expanded shell — the `( // c` glue, the
            // chain one indent in, the `)!` back out — where prettier glues the
            // comment to the `(` and keeps the continuation flush: both keep the
            // run inside the pair, so the divergence is the rendering alone. An
            // inline block run leads the chain flat, matching prettier. A shell
            // whose TRAILING gap also holds comments keeps the anchored-run layout
            // below, the run folded above the chain.
            // These positions never enter a chain, so nothing else scans either gap —
            // the comments the author wrote inside the parens (`new ( // c⏎a?.b)!()`,
            // `new (a?.b /* c */)!()`) are this doc's to print, on the same seam the
            // chain's parenthesized base uses.
            Some(self.build_owned_required_pair_doc(
                (expr.span.start, expr_start),
                Some((inner_start, expr.span.end)),
                inner_doc,
                inner_doc,
                ")!",
            ))
        } else {
            None
        }
    }

    /// Check if an expression is part of a chain (member, call, or non-null)
    fn is_chain_expression(expr: &Expression<'_>) -> bool {
        matches!(
            expr.kind,
            ExpressionKind::MemberExpression(_)
                | ExpressionKind::CallExpression(_)
                | ExpressionKind::TSNonNullExpression(_)
        )
    }

    /// Build expression doc with indentation added to line breaks
    /// Used when expression is inside inline parens like `(expr)!`
    pub(crate) fn build_expression_doc_with_indent_on_break(&self, expr: &Expression<'_>) -> DocId {
        match &expr.kind {
            ExpressionKind::BinaryExpression(binary) => {
                // Build binary chain with indented continuations
                self.build_binary_chain_doc_indented(binary, expr.span)
            }
            _ => self.build_expression_doc(expr),
        }
    }

    /// Build binary chain doc with indented continuations
    /// Used when the binary expression is inside inline parens
    fn build_binary_chain_doc_indented(&self, binary: &BinaryExpression<'_>, span: Span) -> DocId {
        let d = self.d();
        d.group(self.build_binary_chain_parts_indented(binary, span))
    }

    /// Build binary chain parts with indented continuations (no group wrapper)
    ///
    /// Returns the concat without a group wrapper, for cases where the caller
    /// wants to control the grouping (e.g., chain printing).
    pub(crate) fn build_binary_chain_parts_indented(
        &self,
        binary: &BinaryExpression<'_>,
        span: Span,
    ) -> DocId {
        let d = self.d();
        // The comment-aware twin keeps comments and their line breaks: `fn(a && // c\n b)`.
        // Continuation-indent style, matching this builder's own.
        let (operands, operators) = match self.prepare_binary_chain_layout(binary, span, |p| {
            p.build_binary_chain_parts_with_continuation_indent(binary, span)
        }) {
            BinaryChainLayout::Built(doc) => return doc,
            BinaryChainLayout::Operands(operands, operators) => (operands, operators),
        };

        // Build with indented continuations for chains:
        // "first +
        //     second -
        //     third"
        //
        // When shouldGroup is true (operand types differ from current node type,
        // e.g., `(LogicalExpr) + (ConditionalExpr)`), wrap each continuation in
        // its own sub-group so it can independently evaluate whether it fits on
        // the current line when the outer group breaks. This matches Prettier's
        // binaryish.js where shouldGroup controls whether `right` gets a group.
        //
        // When shouldGroup is false (operands are same AST type category, e.g.,
        // `(BinaryExpr) * 100`), all continuations break together with the outer
        // group. This matches Prettier's behavior for same-type chains.
        let should_group = Self::should_group_binary_continuation(binary);
        // shouldInlineLogicalExpression: when the outermost logical has a non-empty
        // object/array on the right, keep operator and RHS on the same line.
        // Prettier ref: `printBinaryishExpressions` + `shouldInlineLogicalExpression` (`print/binaryish.js`)
        let should_inline_last = assignment::should_inline_logical_expression(binary);
        let mut parts = d.pooled_docbuf();

        for (i, operand) in operands.iter().enumerate() {
            let is_last = i == operands.len() - 1;
            if i == 0 {
                parts.push(*operand);
            } else if is_last && should_inline_last {
                // shouldInlineLogicalExpression: keep operator and object/array on same line
                // Use indent with space (no line break) instead of indent_line.
                // For 2-operand chains: prettier returns group(parts) with no indent
                //   (shouldInline && !samePrecedence → flat). We skip indent.
                // For 3+ operand chains: prettier uses indent(rest) which applies to all
                //   continuation operands. We need indent to match the level.
                // Prettier ref: `printBinaryishExpressions` + `printBinaryishExpression` (`print/binaryish.js`)
                let is_chained = operands.len() > 2;
                let op_and_operand = if is_chained {
                    // In a chain, use indent (matches other continuations' indent level)
                    // but space instead of line (keeps operator and object on same line)
                    d.concat(&[
                        d.text(" "),
                        d.text(operators[i - 1].as_str()),
                        d.indent(d.concat(&[d.text(" "), *operand])),
                    ])
                } else {
                    // 2-operand: flat, no indent (prettier returns group(parts) directly)
                    d.concat(&[
                        d.text(" "),
                        d.text(operators[i - 1].as_str()),
                        d.text(" "),
                        *operand,
                    ])
                };
                if should_group {
                    parts.push(d.group(op_and_operand));
                } else {
                    parts.push(op_and_operand);
                }
            } else if should_group {
                // Sub-group for independent fitting
                parts.push(d.group(d.concat(&[
                    d.text(" "),
                    d.text(operators[i - 1].as_str()),
                    d.indent_line(*operand),
                ])));
            } else {
                parts.push(d.text(" "));
                parts.push(d.text(operators[i - 1].as_str()));
                parts.push(d.indent_line(*operand));
            }
        }

        d.concat(&parts)
    }

    /// Prepare a binary chain for one of the two comment-blind layout builders
    /// (`build_binary_chain_parts_indented`, `build_binary_chain_for_parens`).
    ///
    /// Both lay a chain out as operand docs plus raw operator text, with nothing emitted
    /// in the operand→operator gaps — so a comment anywhere in the chain would be dropped
    /// (the comment-blind container builder, `docs/comments.md` hazard 4). Routing both
    /// through this preparation makes the hand-off unskippable: neither can reach an
    /// operand list without having first answered the comment question. `build_commented`
    /// names which comment-aware twin this builder's layout wants — they differ in
    /// indent style, which is the only thing the two callers disagree on.
    fn prepare_binary_chain_layout(
        &self,
        binary: &BinaryExpression<'_>,
        span: Span,
        build_commented: impl FnOnce(&Self) -> DocId,
    ) -> BinaryChainLayout {
        if self.has_comments_to_emit_between(span.start, span.end) {
            return BinaryChainLayout::Built(build_commented(self));
        }

        let mut operands = DocBuf::new();
        let mut operators = OperatorBuf::new();
        self.collect_binary_operands_for_indent(binary, &mut operands, &mut operators);

        if operands.len() <= 1 {
            // Nothing chained after flattening — the ordinary binary doc says it better.
            return BinaryChainLayout::Built(self.build_binary_chain_doc(binary, span));
        }

        BinaryChainLayout::Operands(operands, operators)
    }

    /// Collect operands and operators from a binary chain (helper for indented version)
    ///
    /// Uses `can_flatten_with()` to determine which operators can be chained together.
    /// Flattens both left and right sides when operators are compatible.
    ///
    /// The result carries no spans, so a caller laying it out **cannot** emit comments in
    /// the gaps between operands — reach it only through `prepare_binary_chain_layout`,
    /// which answers the comment question first.
    fn collect_binary_operands_for_indent(
        &self,
        expr: &BinaryExpression<'_>,
        operands: &mut DocBuf,
        operators: &mut OperatorBuf,
    ) {
        // Recursively flatten left side if it can be chained with current operator
        // (`flattenable_left`, shared with `collect_binary_chain_with_spans`); otherwise
        // build the operand with parens if needed.
        if let Some(left_binary) = self.flattenable_left(expr) {
            self.collect_binary_operands_for_indent(left_binary, operands, operators);
        } else {
            operands.push(self.build_binary_operand_doc(expr.left, expr.operator, false));
        }

        // Add current operator
        operators.push(expr.operator);

        // Also flatten the right side where prettier REBALANCES it, removing the redundant
        // parens ([`BinaryOperator::rebalances_with`] — logical operators only; arithmetic
        // preserves right-side parens).
        if let ExpressionKind::BinaryExpression(right_binary) = &expr.right.kind
            && expr.operator.rebalances_with(right_binary.operator)
        {
            self.collect_binary_operands_for_indent(right_binary, operands, operators);
            return;
        }

        // Right operand can't be flattened - add as-is
        operands.push(self.build_binary_operand_doc(expr.right, expr.operator, true));
    }

    /// Build binary chain specifically for parenthesized context in chain printing
    ///
    /// Structure: operand1 " /", line, operand2 " /", line, operand3
    /// In flat: `a / b / c`
    /// In break: `a /\nb /\nc` (with outer indent providing indentation)
    pub(crate) fn build_binary_chain_for_parens(
        &self,
        binary: &BinaryExpression<'_>,
        span: Span,
    ) -> DocId {
        let d = self.d();
        // The *ungrouped* comment-aware twin, not the continuation-indent one: this
        // builder's caller wraps the result in `group(indent([softline, …]), softline)`,
        // which already supplies the one indent level the broken parens want. A
        // continuation indent on top of it would indent every operand after the first a
        // second time.
        let (operands, operators) = match self.prepare_binary_chain_layout(binary, span, |p| {
            p.build_binary_chain_doc_ungrouped(binary, span)
        }) {
            BinaryChainLayout::Built(doc) => return doc,
            BinaryChainLayout::Operands(operands, operators) => (operands, operators),
        };

        // For 2-operand chains, wrap in a group with line() so the binary can
        // independently decide whether to break at the operator. The group stays flat
        // when the operands fit; when they don't, line() fires and breaks at the
        // operator (e.g., `left +\nright`), preventing the operands' internal break
        // points (like member chain dots) from firing instead.
        //
        // Applies to every operator family. Excluding a logical operator here would
        // leave a parenthesized logical base breaking its operands where the
        // arithmetic one holds them together — see conformance_prettier_ts.md §TypeScript
        // (Parenthesized binary member base).
        if operands.len() == 2 {
            return d.group(d.concat(&[
                operands[0],
                d.text(" "),
                d.text(operators[0].as_str()),
                d.line(),
                operands[1],
            ]));
        }

        // For 3+ operand chains, use line breaks between operands:
        // operand1 " /", line, operand2 " /", line, operand3
        let mut parts: DocBuf = DocBuf::new();

        for (i, operand) in operands.iter().enumerate() {
            if i == 0 {
                // First operand
                parts.push(*operand);
            } else {
                // Subsequent operands: line break then operand
                parts.push(d.line()); // space in flat, newline in break
                parts.push(*operand);
            }

            // Add operator after operand (except for last)
            if i < operators.len() {
                parts.push(d.text(" "));
                parts.push(d.text(operators[i].as_str()));
            }
        }

        d.concat(&parts)
    }

    /// Build a Doc for a TypeScript parameter property
    fn build_ts_parameter_property_doc(
        &self,
        param_prop: &crate::ast::internal::TSParameterProperty<'_>,
        param_prop_span: Span,
    ) -> DocId {
        let mut parts: DocBuf = DocBuf::new();

        // The binding name (past any decorators — acorn stores those on the inner
        // binding, and its span starts at the name). Bounds the modifier scans.
        let name_start = param_prop.parameter.span().start;

        // Print modifiers in canonical TS order (accessibility → override →
        // readonly), preserving comments between them and before the binding name
        // (`readonly /* c */ x`) — the same cursor-based scan the class-body
        // `PropertyDefinition` printer uses. The property span starts at the first
        // modifier (decorators precede it in source but render separately below),
        // so the cursor begins there.
        let mut cursor = param_prop_span.start;
        if let Some(acc) = param_prop.accessibility {
            self.push_member_keyword_doc(&mut parts, acc.as_keyword(), &mut cursor, name_start);
        }
        if param_prop.r#override {
            self.push_member_keyword_doc(&mut parts, "override ", &mut cursor, name_start);
        }
        if param_prop.readonly {
            self.push_member_keyword_doc(&mut parts, "readonly ", &mut cursor, name_start);
        }
        // Comments between the last modifier and the binding name.
        self.push_pre_name_comments_doc(&mut parts, cursor, name_start);

        // Print the parameter. acorn stores a decorated property's decorators on
        // the inner binding (`@dec private a` → decorators on the Identifier),
        // but the source order is `@dec` before the modifiers — so render the
        // inner binding WITHOUT its decorators here, then prefix the whole
        // property with them below.
        let parameter = param_prop.parameter;
        let (inner_doc, decorators) = match &parameter.kind {
            ExpressionKind::Identifier(id) => {
                (self.build_identifier_doc_no_decorators(id), id.decorators())
            }
            ExpressionKind::AssignmentPattern(ap) => (
                self.build_assignment_pattern_doc(ap, parameter.span),
                ap.decorators,
            ),
            _ => (self.build_expression_doc(parameter), None),
        };
        parts.push(inner_doc);

        // `param_prop_span.start` is the first modifier (decorators render before it
        // but sit earlier in source) — the boundary for a `@dec /* c */ readonly x`
        // comment between the decorator and the modifier.
        self.with_param_decorators(decorators, self.d().concat(&parts), param_prop_span.start)
    }
}
