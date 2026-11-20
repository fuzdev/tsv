//! Synthetic `tsv_ts` AST construction over the **hybrid appendix buffer**.
//!
//! The compiler generates JS by constructing `tsv_ts` internal-AST rows and
//! printing them through the existing printer (`tsv_ts::__private::format_canonical`).
//! That printer recovers most leaf text by slicing spans out of its `source`
//! argument, so the generator maintains a *buffer* = the host `.svelte` source
//! followed by an **appendix** of minted lexemes:
//!
//! - **Borrowed user subtrees** (script statements, template expressions) keep
//!   their real host spans — they print verbatim through the normal machinery.
//! - **Minted literals / template quasis** get spans pointing into the
//!   appendix, which contains their exact text (appended monotonically, so
//!   every span is in-bounds and on char boundaries).
//! - **Synthetic identifiers** ride the `IdentName` escape-hatch channel
//!   (`IdentName { escaped: Some(push_escaped_name(stores, name)), raw_len: 0 }`) —
//!   the name is copied into the document's text pool, source-free. Their spans are
//!   still backed by minted text for debuggability (the appendix reads as the
//!   generated skeleton).
//! - Keywords/punctuation are printer statics and need no buffer text; the
//!   skeleton around them is minted anyway so node spans cover plausible text.
//!
//! **Print-time position facts** (`params_start`, `arrow_token`) are chosen so the
//! printer's comment windows around them come out *empty*, not so they point at the
//! minted glyph. The printer reads them as window endpoints, and a window running from
//! a borrowed host span to an appendix span would sweep every comment in between —
//! hoisting carried script comments into a synthetic arrow's parameter list. So a
//! synthetic arrow's `arrow_token` anchors on its body start (where the printer's own
//! signature-end math lands when the paren scan finds nothing), collapsing both the
//! signature and body windows to empty.
//!
//! Construction discipline: every row is pushed into the host document's TypeScript
//! stores (`tsv_ts::__private`'s write surface), beside the parse's own rows, so
//! borrowed subtrees and synthetic nodes coexist in one tree. A child is its id, a
//! child list a run pushed from the ids or rows the caller gathered. Borrowed nodes
//! are never deep-copied; where a *wrapper* node must be rebuilt with one field
//! changed (the `$props()` init rewrite), the wrapper's row is copied, changed and
//! pushed as a new row — its children keep their ids into the parsed tree, and the
//! original wrapper never enters the printed tree (no duplicate spans in what the
//! printer walks). The printer keys some memos by a row's store address (the
//! chain-share and type-head memos), scoped to one print: a rebuilt row is a new row
//! at its own address, so it never shares a key with the row it was copied from, and
//! every row a constructor hands the printer is pushed first, never a stack copy.
//!
//! # The node contract
//!
//! Every constructor here and every writer built on it (the rewriters, the emitters)
//! follows one shape, so a host id passes through without a copy-out and re-push:
//!
//! - **A writer takes a TypeScript node by id** (`ExprId`, `StmtId`), or a freshly
//!   built `Copy` row by value, never a reference into the stores' view (a reference
//!   cannot be held across a push) and never a host row copied out of it
//!   (`*view.expr(id)`) only to be passed on unchanged — pass its id. A row is copied
//!   out only to be rebuilt with a change and pushed as a new row. An id edge of the
//!   tree (a callee, an operand, a call's or template's argument list) takes ids — a
//!   freshly built node is pushed first (`let arg = b.push(row);`) — and a value-run
//!   edge (parameters, object properties, declarators, template quasis, import
//!   specifiers) takes the rows themselves, pushed with `push_list`.
//! - **A loop that pushes while walking a run** reads by index
//!   (`let id = b.view().expr_list(run)[i];`) or copies the ids out first
//!   (`.to_vec()`), and a lazy rebuild follows the frame rules of `tsv_ts::__private`'s
//!   write surface (`open` / `extend_from_run` / `flush`, last in first out).
//! - **An `Err` may leave frames open.** A writer that fails mid-rebuild abandons its
//!   frames; they sit below every later frame and are never flushed. A caller that
//!   catches an `Err` from a pushing call and goes on (the census's per-statement
//!   loops, parity's fallback) holds no frame of its own across that call and never
//!   flushes a mark taken before it.
//! - **A reader takes the view first** (`AstView`) and a row by reference
//!   (`view.expr(id)`), returning owned data or ids, never a reference it keeps.
//! - **Template and CSS nodes** are read through the template view (`SvelteView`,
//!   `Copy`, its accessors returning `&'a` references that outlive any push), by
//!   reference. The TypeScript half is never reached through it while the compile
//!   holds the stores apart: `sv.ts()` reads an empty stand-in there, and every
//!   TypeScript read goes through the builder's view (`b.view()`) instead.

use tsv_lang::Span;
use tsv_lang::store::{Id, ListMark, Run};
use tsv_ts::__private::{
    MintList, MintRow, extend_from_run, flush, open, push, push_escaped_name, push_item, push_list,
    push_text,
};
use tsv_ts::ast::{
    ArrayExpression, ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression,
    AssignmentOperator, AstStores, AstView, BinaryExpression, BinaryOperator, BlockStatement,
    CallExpression, ExprId, Expression, ExpressionKind, ExpressionStatement, FunctionDeclaration,
    IdentName, Identifier, IfStatement, ImportDeclaration, ImportKind, ImportNamespaceSpecifier,
    ImportPhase, ImportSpecifier, Literal, LiteralValue, MemberExpression, ObjectExpression,
    PackedF64, Property, PropertyKind, Statement, StatementKind, StmtId, StringCooked,
    TemplateCooked, TemplateElement, TemplateLiteral, UnaryExpression, UnaryOperator,
    UpdateExpression, UpdateOperator, VariableDeclaration, VariableDeclarationKind,
    VariableDeclarator,
};

use crate::CompileError;

/// One element's outcome in a rebuilt list.
pub(crate) enum Item<T> {
    /// Unchanged — the element is the input's.
    Keep,
    /// Rebuilt — the element is this one.
    Replace(T),
    /// Gone — the list loses the element.
    Drop,
}

impl<T> From<Option<T>> for Item<T> {
    #[inline]
    fn from(item: Option<T>) -> Self {
        item.map_or(Item::Keep, Item::Replace)
    }
}

/// A rewriter writing into the host stores: what [`rebuild_run`] reaches them by.
pub(crate) trait HasStores {
    /// The stores the rewriter reads the input from and pushes its rebuild into.
    fn stores(&mut self) -> &mut AstStores;
}

/// Rebuild `run` through a per-element rewrite, sharing the input when no element
/// changed (`None`). The lazy-prefix rebuild of the write surface: until the first
/// change nothing is written; the first changed (or dropped) element opens a frame
/// and copies the unchanged prefix in one block, and every later element is pushed
/// as it comes. The first changed element's `each` runs before the frame opens;
/// every later `each` runs with the frame open beneath it, and any frame it opens is
/// flushed before it returns `Ok`, so the frames nest last in, first out. An `Err`
/// abandons the open frame and whatever `each` left above it (the node contract's
/// "an `Err` may leave frames open").
///
/// `read` is the view accessor of `T`'s family; each element is read by index and
/// copied out, so no view is held across `each`.
pub(crate) fn rebuild_run<C: HasStores, T: MintList>(
    ctx: &mut C,
    run: Run<T>,
    read: for<'v> fn(AstView<'v>, Run<T>) -> &'v [T],
    mut each: impl FnMut(&mut C, T) -> Result<Item<T>, CompileError>,
) -> Result<Option<Run<T>>, CompileError> {
    let mut mark: Option<ListMark<T>> = None;
    for i in 0..run.len() {
        let item = read(ctx.stores().view(), run)[i];
        let out = each(ctx, item)?;
        let stores = ctx.stores();
        match out {
            Item::Keep => {
                if mark.is_some() {
                    push_item(stores, item);
                }
            }
            Item::Replace(new) => {
                if mark.is_none() {
                    mark = Some(open_with_prefix(stores, run, i));
                }
                push_item(stores, new);
            }
            Item::Drop => {
                if mark.is_none() {
                    mark = Some(open_with_prefix(stores, run, i));
                }
            }
        }
    }
    Ok(mark.map(|mark| flush(ctx.stores(), mark)))
}

/// Open a frame for a rebuild of `run` and copy its unchanged prefix `0..index`.
fn open_with_prefix<T: MintList>(stores: &mut AstStores, run: Run<T>, index: usize) -> ListMark<T> {
    let mark = open::<T>(stores);
    extend_from_run(stores, run, 0..index);
    mark
}

/// The appendix-buffer bookkeeping plus the stores every node constructor pushes
/// into. Owns the growing buffer; the stores are the host document's TypeScript half
/// (handed out by `tsv_svelte::__private::with_ts_stores`), so borrowed subtrees and
/// synthetic rows coexist in one tree.
pub(crate) struct Builder<'a> {
    /// The host document's TypeScript stores: the parse's rows, and every row pushed
    /// here beside them.
    pub stores: &'a mut AstStores,
    /// Host source + appendix of minted lexemes. Passed to `format_canonical`
    /// as the source every span in the synthetic program indexes into.
    pub buffer: String,
}

impl<'a> Builder<'a> {
    pub(crate) fn new(stores: &'a mut AstStores, host_source: &str) -> Self {
        Self {
            stores,
            buffer: host_source.to_string(),
        }
    }

    /// The stores' read view: the parse's rows and every row pushed so far.
    pub(crate) fn view(&self) -> AstView<'_> {
        self.stores.view()
    }

    /// Push `row` into its store and return its id.
    pub(crate) fn push<T: MintRow>(&mut self, row: T) -> Id<T> {
        push(self.stores, row)
    }

    /// `items` as one run of their store.
    pub(crate) fn push_list<T: MintList>(&mut self, items: &[T]) -> Run<T> {
        push_list(self.stores, items)
    }

    /// The span of the expression `id` names.
    fn expr_span(&self, id: ExprId) -> Span {
        self.view().expr(id).span
    }

    /// Append minted text to the appendix, returning its span.
    pub(crate) fn mint(&mut self, text: &str) -> Span {
        let start = self.buffer.len() as u32;
        self.buffer.push_str(text);
        Span::new(start, self.buffer.len() as u32)
    }

    /// A synthetic name: `name` copied into the text pool, read through the
    /// escape-hatch channel (source-free resolution).
    fn synthetic_name(&mut self, name: &str) -> IdentName {
        IdentName {
            escaped: Some(push_escaped_name(self.stores, name)),
            raw_len: 0,
            plain_ascii: false,
        }
    }

    /// A synthetic identifier: pooled name (source-free resolution) with its text
    /// minted into the appendix so the span is backed.
    pub(crate) fn ident(&mut self, name: &str) -> Identifier {
        let span = self.mint(name);
        let ident_name = self.synthetic_name(name);
        Identifier::simple(ident_name, span)
    }

    /// A synthetic identifier as a pushed expression.
    pub(crate) fn ident_expr(&mut self, name: &str) -> ExprId {
        let ident = self.ident(name);
        self.push(Expression::from_identifier(ident))
    }

    /// A synthetic identifier at a caller-chosen span (no minting). The pooled
    /// name channel never extracts the span, so the span's only job is steering the
    /// printer's comment windows — a *fictional* low span keeps a synthetic header
    /// node's windows empty/inverted, and a *stolen* host span (the node it
    /// replaces, e.g. `$$props` over the original `$props()` call) keeps the
    /// surrounding gaps exactly the authored ones.
    pub(crate) fn ident_at(&mut self, name: &str, span: Span) -> Identifier {
        let ident_name = self.synthetic_name(name);
        Identifier::simple(ident_name, span)
    }

    /// [`Self::ident_at`] as a pushed expression (no minting — the pooled name
    /// channel supplies the text, so the span steers comment windows only).
    pub(crate) fn ident_expr_at(&mut self, name: &str, span: Span) -> ExprId {
        let ident = self.ident_at(name, span);
        self.push(Expression::from_identifier(ident))
    }

    /// A single-quoted string literal minted into the appendix. `content` must
    /// not itself require escaping (module specifiers do not).
    pub(crate) fn string_literal(&mut self, content: &str) -> Literal {
        // Escape quote/backslash/newlines so any content is safe in release
        // builds too (module specifiers never need it, but safety must not be a
        // debug-only guarantee). When escaping fires, the minted raw text differs
        // from the decoded value, so the cooked channel switches to `Decoded`.
        if content.contains(['\'', '\\', '\n', '\r']) {
            let mut escaped = String::with_capacity(content.len() + 2);
            for c in content.chars() {
                match c {
                    '\'' => escaped.push_str("\\'"),
                    '\\' => escaped.push_str("\\\\"),
                    '\n' => escaped.push_str("\\n"),
                    '\r' => escaped.push_str("\\r"),
                    _ => escaped.push(c),
                }
            }
            let span = self.mint(&format!("'{escaped}'"));
            return Literal {
                value: LiteralValue::String(StringCooked::Decoded(push_text(self.stores, content))),
                span,
            };
        }
        let span = self.mint(&format!("'{content}'"));
        Literal {
            value: LiteralValue::String(StringCooked::Verbatim),
            span,
        }
    }

    /// `import * as <local> from '<specifier>';`
    pub(crate) fn import_namespace(&mut self, local: &str, specifier: &str) -> Statement {
        let start = self.mint("import * as ").start;
        let local = self.ident(local);
        let local_span = local.span;
        self.mint(" from ");
        let source = self.string_literal(specifier);
        let end = self.mint(";").end;
        let specifiers = self.push_list(&[ImportSpecifier::Namespace(ImportNamespaceSpecifier {
            local,
            span: local_span,
        })]);
        let import = self.push(ImportDeclaration {
            specifiers,
            source,
            attributes: None,
            import_kind: ImportKind::Value,
            phase: ImportPhase::None,
        });
        Statement {
            span: Span::new(start, end),
            kind: StatementKind::ImportDeclaration(import),
        }
    }

    /// `<object>.<property>(<arguments>)` — a call on a synthetic member chain.
    /// The arguments may hold borrowed expressions (host ids).
    pub(crate) fn member_call(
        &mut self,
        object: &str,
        property: &str,
        arguments: &[ExprId],
    ) -> Expression {
        let obj = self.ident_expr(object);
        self.mint(".");
        let prop = self.ident_expr(property);
        let member_span = Span::new(self.expr_span(obj).start, self.expr_span(prop).end);
        self.mint("(");
        let end = self.mint(")").end;
        let callee = self.push(Expression {
            span: member_span,
            kind: ExpressionKind::MemberExpression(MemberExpression {
                object: obj,
                property: prop,
                computed: false,
                optional: false,
            }),
        });
        let arguments = self.push_list(arguments);
        Expression {
            span: Span::new(member_span.start, end),
            kind: ExpressionKind::CallExpression(CallExpression {
                callee,
                type_args: None,
                arguments,
                optional: false,
            }),
        }
    }

    /// A template literal from alternating static parts and expressions
    /// (`texts.len() == expressions.len() + 1`). Static text is minted into the
    /// appendix (already template-escaped by the caller); expressions may be
    /// borrowed user subtrees. The `${`/`}` delimiters are minted as
    /// placeholders so the appendix stays a readable mirror of the output — the
    /// printer emits them statically and never reads those bytes.
    pub(crate) fn template_literal(
        &mut self,
        texts: &[String],
        expressions: Run<ExprId>,
    ) -> Expression {
        debug_assert_eq!(texts.len(), expressions.len() + 1);
        let start = self.mint("`").start;
        let mut quasis = Vec::with_capacity(texts.len());
        let last = texts.len() - 1;
        for (i, text) in texts.iter().enumerate() {
            let raw_span = self.mint(text);
            let tail = i == last;
            quasis.push(TemplateElement {
                raw_span,
                // Minted text is generated, never authored, so it holds no `<CR>`
                // for the TRV to normalize — the appendix slice IS the value.
                raw_trv: None,
                cooked: TemplateCooked::Verbatim,
                has_newline: text.contains('\n'),
                tail,
                span: raw_span,
            });
            if !tail {
                self.mint("${}");
            }
        }
        let end = self.mint("`").end;
        let quasis = self.push_list(&quasis);
        Expression::from_template_literal(
            Span::new(start, end),
            TemplateLiteral {
                quasis,
                expressions,
            },
        )
    }

    /// A call on a borrowed callee expression (`d()`): the callee keeps its host
    /// span, the `()` is minted for the appendix but the CallExpression node takes
    /// the callee's **tight** span. The printer prints `()` structurally, so the
    /// tight span is output-neutral — but it keeps the empty-arg comment window
    /// (`[callee.end, call.span.end]`, `calls/call_formatting.rs`) EMPTY. Without
    /// it, an appendix-end span would sweep every host comment after the callee
    /// into the minted parens (`full_path(/* c */)`), double-printing a carried
    /// script comment when this lowers a script-position `$derived` read.
    pub(crate) fn call_expr(&mut self, callee: ExprId, arguments: &[ExprId]) -> Expression {
        self.mint("()");
        let arguments = self.push_list(arguments);
        Expression {
            span: self.expr_span(callee),
            kind: ExpressionKind::CallExpression(CallExpression {
                callee,
                type_args: None,
                arguments,
                optional: false,
            }),
        }
    }

    /// A call on a borrowed callee with an argument list (`foo($$renderer, x)`
    /// or, when `optional`, `foo?.($$renderer, x)`): the callee keeps its host
    /// span, the `(…)` is minted. `arguments` may mix synthetic and borrowed
    /// expressions.
    pub(crate) fn call_of(
        &mut self,
        callee: ExprId,
        arguments: &[ExprId],
        optional: bool,
    ) -> Expression {
        self.mint(if optional { "?.(" } else { "(" });
        let end = self.mint(")").end;
        let arguments = self.push_list(arguments);
        Expression {
            span: Span::new(self.expr_span(callee).start, end),
            kind: ExpressionKind::CallExpression(CallExpression {
                callee,
                type_args: None,
                arguments,
                optional,
            }),
        }
    }

    /// `() => <body>` — the `$derived` thunk (and the destructured-`$derived`
    /// intermediates'), every synthetic position collapsed onto the `body`'s own span.
    /// The arrow's parameter and body windows are then empty: with no `params_start` the
    /// empty list prints `()` without the printer's source-wide `)` scan (which, from a
    /// body opening with `(`, claims that paren group), and the `=>`-to-body gap is
    /// inverted. So a comment inside the `$derived(…)` parens but outside `body` falls
    /// to the enclosing `$.derived(…)` call's argument windows ([`Self::member_call_at`])
    /// exactly once, and a comment inside `body` prints with `body`. A thunk anchored on
    /// the replaced init instead let that scan reach the host `$derived(`…`)` parens and
    /// print the argument's comments a second time inside `()`.
    pub(crate) fn thunk_on_body(&mut self, body: ExprId) -> Expression {
        let span = self.expr_span(body);
        let arrow = self.push(ArrowFunctionExpression {
            type_parameters: None,
            params: Run::EMPTY,
            body: ArrowFunctionBody::Expression(body),
            return_type: None,
            r#async: false,
            params_start: None,
            arrow_token: span.start,
        });
        Expression {
            span,
            kind: ExpressionKind::ArrowFunctionExpression(arrow),
        }
    }

    /// `<object>.<property>(<arguments>)` — the fictional-span form of
    /// [`Self::member_call`]. Every synthetic leaf (`<object>`, `<property>`) collapses
    /// onto `anchor.start` and the call *steals* `anchor`, the host span of what it
    /// replaces (a `$derived(...)` init, a store read), so the enclosing node's windows
    /// read the authored span and the call's own internal windows hold no host comment
    /// its arguments don't claim. The pooled names and the static `.`/`(`/`)` supply
    /// the text, so the fictional spans never reach output — byte-identical to the
    /// appendix-spanned form when the script carries no comments. With a zero-width
    /// `anchor` and zero-width synthetic arguments the whole call sits at one host
    /// position (the prepended `$props.id()` / `$$slots` declarations).
    pub(crate) fn member_call_at(
        &mut self,
        object: &str,
        property: &str,
        arguments: &[ExprId],
        anchor: Span,
    ) -> Expression {
        let low = Span::new(anchor.start, anchor.start);
        let object = self.ident_expr_at(object, low);
        let property = self.ident_expr_at(property, low);
        let callee = self.push(Expression {
            span: low,
            kind: ExpressionKind::MemberExpression(MemberExpression {
                object,
                property,
                computed: false,
                optional: false,
            }),
        });
        let arguments = self.push_list(arguments);
        Expression {
            span: anchor,
            kind: ExpressionKind::CallExpression(CallExpression {
                callee,
                type_args: None,
                arguments,
                optional: false,
            }),
        }
    }

    /// `void 0` as a zero-width fictional node at host position `at`, for a
    /// replaced argument-less rune call (`$state()`, `$bindable()`). [`Self::void_zero`]
    /// mints the literal into the appendix, and a numeric literal prints its own source
    /// slice, so its span cannot move; the window from the host `=` to that appendix
    /// start would then sweep every later carried comment. This spells the two tokens
    /// through the synthetic-identifier name channel instead, which prints the name
    /// and reads no source, so the node can sit at `at` — the end of the replaced
    /// call, leaving a comment inside the empty call to the window before it.
    pub(crate) fn void_zero_at(&mut self, at: u32) -> Expression {
        Expression::from_identifier(self.ident_at("void 0", Span::new(at, at)))
    }

    /// `(<params>) => { <stmts> }` — a block-bodied arrow (the
    /// `$$renderer.component(($$renderer) => { … })` wrapper and the `$.await`
    /// pending / then callbacks). `params` may be minted synthetic identifiers
    /// (`$$renderer`) or borrowed user patterns (a `{:then value}` binding).
    /// `block_span` is the span the block's comment windows anchor on (the
    /// caller decides — host-anchored when the body holds borrowed statements).
    pub(crate) fn arrow_block(
        &mut self,
        params: &[Expression],
        body: Run<StmtId>,
        block_span: Span,
    ) -> Expression {
        let start = self.mint("(").start;
        let params_start = start;
        self.mint(") => {");
        let end = self.mint("}").end;
        let params = self.push_list(params);
        let arrow = self.push(ArrowFunctionExpression {
            type_parameters: None,
            params,
            body: ArrowFunctionBody::BlockStatement(BlockStatement {
                body,
                span: block_span,
            }),
            return_type: None,
            r#async: false,
            params_start: Some(params_start),
            arrow_token: block_span.start,
        });
        Expression {
            span: Span::new(start, end),
            kind: ExpressionKind::ArrowFunctionExpression(arrow),
        }
    }

    /// A zero-width synthetic span at the current appendix end. For a wrapper
    /// node (`if`/`for`/block statement) that needs a span but no backing text —
    /// its keywords print statically and, with block output, no comments are
    /// carried, so the span only has to stay in-bounds.
    pub(crate) fn here(&self) -> Span {
        let pos = self.buffer.len() as u32;
        Span::new(pos, pos)
    }

    /// `<object>.<name>` — a non-computed member on a synthetic property name
    /// (`each_array.length`).
    pub(crate) fn member_prop(&mut self, object: ExprId, name: &str) -> Expression {
        self.mint(".");
        let prop = self.ident_expr(name);
        let span = Span::new(self.expr_span(object).start, self.expr_span(prop).end);
        Expression {
            span,
            kind: ExpressionKind::MemberExpression(MemberExpression {
                object,
                property: prop,
                computed: false,
                optional: false,
            }),
        }
    }

    /// `<object>[<index>]` — a computed member (`each_array[$$index]`).
    pub(crate) fn member_computed(&mut self, object: ExprId, index: ExprId) -> Expression {
        self.mint("[");
        let end = self.mint("]").end;
        let span = Span::new(self.expr_span(object).start, end);
        Expression {
            span,
            kind: ExpressionKind::MemberExpression(MemberExpression {
                object,
                property: index,
                computed: true,
                optional: false,
            }),
        }
    }

    /// `<left> <op> <right>` — a binary expression (`$$index < $$length`,
    /// `each_array.length !== 0`).
    pub(crate) fn binary(&mut self, left: ExprId, op: BinaryOperator, right: ExprId) -> Expression {
        self.mint(&format!(" {} ", op.as_str()));
        let span = Span::new(self.expr_span(left).start, self.expr_span(right).end);
        Expression {
            span,
            kind: ExpressionKind::BinaryExpression(BinaryExpression {
                left,
                operator: op,
                right,
                // Minted code, not source the printer will re-lex: the builder emits its own
                // spelling, and no shape it mints opens a type-argument region.
                relexes_as_type_arguments: false,
                may_close_type_arguments: false,
            }),
        }
    }

    /// `<argument>++` / `<argument>--` (postfix) — an update expression.
    pub(crate) fn update(&mut self, argument: ExprId, op: UpdateOperator) -> Expression {
        let text = if op == UpdateOperator::Increment {
            "++"
        } else {
            "--"
        };
        let end = self.mint(text).end;
        let span = Span::new(self.expr_span(argument).start, end);
        Expression {
            span,
            kind: ExpressionKind::UpdateExpression(UpdateExpression {
                operator: op,
                argument,
                prefix: false,
            }),
        }
    }

    /// `[<elements>]` — an array literal. The `[`/`]` are minted for span bounds;
    /// the elements (already built, their text elsewhere in the appendix) print
    /// structurally with the printer's own commas. Used for
    /// `$.exclude_from_object(o, ['a', 'b'])`.
    pub(crate) fn array_of(&mut self, elements: &[Option<ExprId>]) -> Expression {
        let start = self.mint("[").start;
        let end = self.mint("]").end;
        let elements = self.push_list(elements);
        Expression {
            span: Span::new(start, end),
            kind: ExpressionKind::ArrayExpression(ArrayExpression {
                elements,
                spread_trailing_comma: false,
            }),
        }
    }

    /// A numeric literal expression (`0`).
    pub(crate) fn number(&mut self, value: f64) -> Expression {
        let span = self.mint(&format!("{value}"));
        Expression::from_literal(Literal {
            value: LiteralValue::Number(PackedF64::new(value)),
            span,
        })
    }

    /// `<expr>;` — an expression in statement position, spanning exactly its
    /// expression.
    ///
    /// The span is the statement's only degree of freedom (`is_directive` is
    /// false for everything the generator emits — a directive is a string
    /// literal the parser flags, never a synthesized call), and taking it from
    /// the expression is the right default for a *synthetic* statement: the
    /// printer reads a statement's span as the endpoints of the comment windows
    /// around it, so a statement coextensive with its expression inherits
    /// exactly the expression's own windows and can sweep nothing the
    /// expression wouldn't. A statement wrapping a *fictional* span is
    /// deliberately steering those windows and builds its own node instead
    /// (`transform_server`'s `$$renderer.component` wrapper).
    pub(crate) fn expression_statement(&mut self, expression: Expression) -> Statement {
        let span = expression.span();
        Statement {
            span,
            kind: StatementKind::ExpressionStatement(ExpressionStatement {
                expression: self.push(expression),
                is_directive: false,
            }),
        }
    }

    /// `$$renderer.push('<text>')` — a block anchor push with a *string-literal*
    /// argument (single-quoted after canonicalization), distinct from the
    /// template-literal pushes the `BodyBuilder` flushes. `text` is a hydration
    /// anchor comment (`<!--[0-->`, `<!--[-->`, …) — never needs escaping.
    pub(crate) fn push_string_stmt(&mut self, text: &str) -> Statement {
        let arg = self.string_literal_expr(text);
        let arg = self.push(arg);
        let call = self.member_call("$$renderer", "push", &[arg]);
        self.expression_statement(call)
    }

    /// `void 0` — the oracle's spelling of an absent rune argument.
    pub(crate) fn void_zero(&mut self) -> Expression {
        let span = self.mint("void 0");
        let zero = self.push(Expression::from_literal(Literal {
            value: LiteralValue::Number(PackedF64::new(0.0)),
            span: Span::new(span.end - 1, span.end),
        }));
        Expression::from_unary_expression(UnaryExpression {
            operator: UnaryOperator::Void,
            argument: zero,
            span,
        })
    }

    /// A `true` literal (the `$.attr(name, value, true)` boolean-attribute arg).
    pub(crate) fn true_literal(&mut self) -> Expression {
        let span = self.mint("true");
        Expression::from_literal(Literal {
            value: LiteralValue::Boolean(true),
            span,
        })
    }

    /// A single-quoted string literal expression.
    pub(crate) fn string_literal_expr(&mut self, content: &str) -> Expression {
        Expression::from_literal(self.string_literal(content))
    }

    /// `$.store_get(($$store_subs ??= {}), '$<base>', <base>)` — the oracle's SSR
    /// store auto-subscription read (`Identifier.js` → `serialize_get_binding` for a
    /// `store_sub` binding). `base` is the `$`-stripped store name; the string key
    /// keeps the leading `$` (`'$count'`); a `$derived` base reads `<base>()` (the
    /// store the derived currently holds). The printer parenthesizes the `??=`
    /// assignment argument to match the canonical form.
    ///
    /// A fictional-span node: the call takes `read`, the replaced `$<base>`
    /// identifier's host span, and every synthetic leaf sits zero-width at its start,
    /// so no window in or around the call holds a host comment the enclosing node's
    /// windows don't already claim. With appendix spans a window from a host
    /// neighbour into the call swept every later carried script comment. The
    /// `'$<base>'` key is spelled through the identifier name channel, as
    /// [`Self::void_zero_at`] spells `void 0`: a string literal prints its own source
    /// slice, so it cannot move off the appendix.
    pub(crate) fn store_get(
        &mut self,
        base: &str,
        base_is_derived: bool,
        read: Span,
    ) -> Expression {
        let at = Span::new(read.start, read.start);
        let subs = self.store_subs_assign(at);
        let args = [
            self.push(subs),
            self.ident_expr_at(&format!("'${base}'"), at),
            self.store_base_value(base, base_is_derived, at),
        ];
        self.member_call_at("$", "store_get", &args, read)
    }

    /// `($$store_subs ??= {})` zero-width at `at` ([`Self::store_get`]).
    fn store_subs_assign(&mut self, at: Span) -> Expression {
        let obj = self.push(Expression {
            span: at,
            kind: ExpressionKind::ObjectExpression(ObjectExpression {
                properties: Run::EMPTY,
                spread_trailing_comma: false,
            }),
        });
        Expression {
            span: at,
            kind: ExpressionKind::AssignmentExpression(AssignmentExpression {
                left: self.ident_expr_at("$$store_subs", at),
                operator: AssignmentOperator::NullishAssign,
                right: obj,
            }),
        }
    }

    /// The store's value expression, pushed — `<base>()` when `base` is a `$derived`
    /// binding, else the bare `<base>` identifier — zero-width at `at`
    /// ([`Self::store_get`]).
    fn store_base_value(&mut self, base: &str, base_is_derived: bool, at: Span) -> ExprId {
        if base_is_derived {
            let callee = self.ident_expr_at(base, at);
            self.push(Expression {
                span: at,
                kind: ExpressionKind::CallExpression(CallExpression {
                    callee,
                    type_args: None,
                    arguments: Run::EMPTY,
                    optional: false,
                }),
            })
        } else {
            self.ident_expr_at(base, at)
        }
    }

    /// `$.store_set(<base>, <value>)` — the oracle's SSR store write
    /// (`AssignmentExpression.js` → `serialize_set_binding` for a `store_sub`
    /// binding). `base` is the `$`-stripped store name (the store object is
    /// referenced bare, never `$$store_subs`); `value` is the already-rewritten
    /// right-hand side (a compound `+=` is reconstructed as `store_get(...) <op>
    /// rhs` by the caller).
    ///
    /// A script rewrite, so fictional-span throughout ([`Self::store_get`]): the
    /// call takes `assign`, the replaced assignment's host span, its callee sits at the
    /// assignment's start, and `<base>` zero-width at `value`'s start — so the call's
    /// leading-argument window is the authored target-and-`=` run and the one after
    /// `value` the authored tail, each claimed once. (Between two arguments the call
    /// printer looks for their comma in the source; with none there, a `<base>` at the
    /// target's end left the `=` gap to no window, dropping its comment.)
    pub(crate) fn store_set(&mut self, base: &str, value: ExprId, assign: Span) -> Expression {
        let at = self.expr_span(value).start;
        let base_ident = self.ident_expr_at(base, Span::new(at, at));
        self.member_call_at("$", "store_set", &[base_ident, value], assign)
    }

    /// `$.update_store[_pre](($$store_subs ??= {}), '$<base>', <base>[, -1])` — the
    /// oracle's SSR store increment/decrement (`UpdateExpression.js`). `prefix`
    /// selects `update_store_pre` (`++$x` / `--$x`) over `update_store`
    /// (`$x++` / `$x--`); `decrement` appends the trailing `-1` argument
    /// (increment elides it). The printer parenthesizes the `??=` assignment
    /// argument, like [`store_get`](Self::store_get).
    ///
    /// Fictional-span like [`Self::store_get`]: the call takes `update`, the replaced
    /// update expression's host span, with every leaf zero-width at its start (`-1`
    /// spelled through the identifier name channel for the same reason as the key).
    pub(crate) fn update_store(
        &mut self,
        base: &str,
        prefix: bool,
        decrement: bool,
        update: Span,
    ) -> Expression {
        let at = Span::new(update.start, update.start);
        let subs = self.store_subs_assign(at);
        let mut args = vec![
            self.push(subs),
            self.ident_expr_at(&format!("'${base}'"), at),
            self.ident_expr_at(base, at),
        ];
        if decrement {
            args.push(self.ident_expr_at("-1", at));
        }
        let property = if prefix {
            "update_store_pre"
        } else {
            "update_store"
        };
        self.member_call_at("$", property, &args, update)
    }

    /// `var $$store_subs;` — the store-subscription accumulator, injected as a
    /// component-body statement when any store read compiled. Zero-width at `at`, the
    /// body block's start, for the reason `transform_server.rs::build_props_id_decl`
    /// gives.
    pub(crate) fn store_subs_var_at(&mut self, at: u32) -> Statement {
        let span = Span::new(at, at);
        let id = self.ident_expr_at("$$store_subs", span);
        let declarations = self.push_list(&[VariableDeclarator {
            id,
            init: None,
            definite: false,
            span,
        }]);
        Statement::from_variable_declaration(VariableDeclaration {
            kind: VariableDeclarationKind::Var,
            declarations,
            declare: false,
            span,
        })
    }

    /// `if ($$store_subs) $.unsubscribe_stores($$store_subs);` — the store
    /// cleanup, injected as the component body's last statement (before any
    /// `$.bind_props`).
    pub(crate) fn unsubscribe_stores_stmt(&mut self) -> Statement {
        let test = Expression::from_identifier(self.ident("$$store_subs"));
        let test_start = test.span().start;
        let subs_arg = self.ident_expr("$$store_subs");
        let call = self.member_call("$", "unsubscribe_stores", &[subs_arg]);
        let call_span = call.span();
        let consequent = self.expression_statement(call);
        let consequent = self.push(consequent);
        let test = self.push(test);
        Statement {
            span: Span::new(test_start, call_span.end),
            kind: StatementKind::IfStatement(IfStatement {
                test,
                consequent,
                alternate: None,
            }),
        }
    }

    /// `function <name>(<params>) { <body> }` — a named function declaration
    /// (the emitted snippet function). `name` rides the pooled-name channel;
    /// `params` may mix the synthetic `$$renderer` identifier with borrowed
    /// snippet parameter patterns (host spans). `block_span` anchors the body's
    /// comment windows (host-anchored when the body holds borrowed statements).
    ///
    /// **`type_parameters` is always `None`, and that is a contract, not an
    /// accident**: it is how a generic `{#snippet s<T>(x: T)}` erases its `<T>`.
    /// The clause is type-level only — the oracle emits `function s($$renderer, x)`
    /// either way — so *not reading it* IS the erasure. Threading a caller's type
    /// parameters through here would silently print them into the compiled JS.
    pub(crate) fn function_declaration(
        &mut self,
        name: &str,
        params: &[Expression],
        body: Run<StmtId>,
        block_span: Span,
    ) -> Statement {
        let start = self.mint("function ").start;
        let id = self.ident(name);
        let params_start = self.mint("(").start;
        self.mint(") {");
        let end = self.mint("}").end;
        let params = self.push_list(params);
        let span = Span::new(start, end);
        let func = self.push(FunctionDeclaration {
            id: Some(id),
            type_parameters: None,
            params,
            return_type: None,
            body: BlockStatement {
                body,
                span: block_span,
            },
            generator: false,
            r#async: false,
            params_start,
            span,
        });
        Statement::from_function_declaration(span, func)
    }
}

/// One plain `key: value` object property — the shape all 14 synthetic-property
/// sites build (`kind: Init`, never computed, never a method). `key` and `value` are
/// ids, so a borrowed value passes through as itself and a built one is pushed first.
///
/// `span` is an explicit parameter rather than derived from `key`: thirteen sites
/// use the key's own span, but `script_props`'s injected `$$slots`/`$$events`
/// property spans key→value, and deriving it would silently change that one site.
pub(crate) fn init_property(key: ExprId, value: ExprId, shorthand: bool, span: Span) -> Property {
    Property {
        key,
        value,
        kind: PropertyKind::Init,
        shorthand,
        computed: false,
        method: false,
        span,
    }
}

/// Escape static text for inclusion in a template-literal quasi: backslash,
/// backtick, and `${` (the `$` is escaped only when a `{` follows).
pub(crate) fn escape_template_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push_str("\\\\"),
            '`' => out.push_str("\\`"),
            '$' if chars.peek() == Some(&'{') => out.push_str("\\$"),
            _ => out.push(c),
        }
    }
    out
}
