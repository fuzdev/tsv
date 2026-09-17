//! Synthetic `tsv_ts` AST construction over the **hybrid appendix buffer**.
//!
//! The compiler generates JS by constructing `tsv_ts` internal-AST nodes and
//! printing them through the existing printer (`tsv_ts::format_canonical`).
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
//!   (`IdentName { escaped: Some(arena.alloc_str(name)), raw_len: 0 }`) — the
//!   name is an arena-allocated `&'arena str`, source-free. Their spans are still
//!   backed by minted text for debuggability (the appendix reads as the
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
//! Construction discipline: child collections are built as arena slices
//! (`bumpalo::collections::Vec` → `into_bump_slice`), single children via
//! `arena.alloc`. Borrowed nodes are never deep-copied; where a *wrapper* node
//! must be rebuilt with one field changed (the `$props()` init rewrite), the
//! by-value fields are shallow-cloned — children remain shared `&'arena` refs
//! into the parsed AST, and the original wrapper never enters the printed tree
//! (no duplicate spans in what the printer walks). Caveat for future
//! address-keyed side-tables (the printer's `chain_arg_share` pattern): a
//! shallow clone mints a NEW wrapper address while its children keep theirs,
//! so any map keyed by node pointer must be scoped to one printed tree, never
//! shared across the parsed AST and the synthetic program.

use bumpalo::Bump;
use tsv_lang::Span;
use tsv_ts::ast::internal::{
    ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression, AssignmentOperator,
    BinaryExpression, BinaryOperator, BlockStatement, CallExpression, Expression, ExpressionKind,
    ExpressionStatement, FunctionDeclaration, IdentName, Identifier, IfStatement,
    ImportDeclaration, ImportKind, ImportNamespaceSpecifier, ImportPhase, ImportSpecifier, Literal,
    LiteralValue, MemberExpression, ObjectExpression, Property, PropertyKind, Statement,
    StringCooked, TemplateCooked, TemplateElement, TemplateLiteral, UnaryExpression, UnaryOperator,
    UpdateExpression, UpdateOperator, VariableDeclaration, VariableDeclarationKind,
    VariableDeclarator,
};

/// The appendix-buffer bookkeeping plus arena access — everything node
/// constructors need. Owns the growing buffer; the arena is shared with the
/// parsed host AST so borrowed subtrees and synthetic nodes coexist in one graph.
pub(crate) struct Builder<'arena> {
    pub arena: &'arena Bump,
    /// Host source + appendix of minted lexemes. Passed to `format_canonical`
    /// as the source every span in the synthetic program indexes into.
    pub buffer: String,
}

impl<'arena> Builder<'arena> {
    pub fn new(arena: &'arena Bump, host_source: &str) -> Self {
        Self {
            arena,
            buffer: host_source.to_string(),
        }
    }

    /// Append minted text to the appendix, returning its span.
    pub fn mint(&mut self, text: &str) -> Span {
        let start = self.buffer.len() as u32;
        self.buffer.push_str(text);
        Span::new(start, self.buffer.len() as u32)
    }

    /// A synthetic identifier: arena-allocated name (source-free resolution) with
    /// its text minted into the appendix so the span is backed.
    pub fn ident(&mut self, name: &str) -> Identifier<'arena> {
        let span = self.mint(name);
        let ident_name = IdentName {
            escaped: Some(self.arena.alloc_str(name)),
            raw_len: 0,
            plain_ascii: false,
        };
        Identifier::simple(ident_name, span)
    }

    /// A synthetic identifier as an arena-allocated expression.
    pub fn ident_expr(&mut self, name: &str) -> &'arena Expression<'arena> {
        let ident = self.ident(name);
        self.arena.alloc(Expression::from_identifier(ident))
    }

    /// A synthetic identifier at a caller-chosen span (no minting). The
    /// arena-allocated name channel never extracts the span, so the span's only
    /// job is steering the printer's comment windows — a *fictional* low span keeps
    /// a synthetic header node's windows empty/inverted, and a *stolen* host span
    /// (the node it replaces, e.g. `$$props` over the original `$props()` call)
    /// keeps the surrounding gaps exactly the authored ones.
    pub fn ident_at(&self, name: &str, span: Span) -> Identifier<'arena> {
        let ident_name = IdentName {
            escaped: Some(self.arena.alloc_str(name)),
            raw_len: 0,
            plain_ascii: false,
        };
        Identifier::simple(ident_name, span)
    }

    /// [`Self::ident_at`] as an arena-allocated expression (no minting — the
    /// arena-allocated name channel supplies the text, so the span steers comment
    /// windows only).
    pub fn ident_expr_at(&self, name: &str, span: Span) -> &'arena Expression<'arena> {
        self.arena
            .alloc(Expression::from_identifier(self.ident_at(name, span)))
    }

    /// A single-quoted string literal minted into the appendix. `content` must
    /// not itself require escaping (module specifiers do not).
    pub fn string_literal(&mut self, content: &str) -> Literal<'arena> {
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
                value: LiteralValue::String(StringCooked::Decoded(self.arena.alloc_str(content))),
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
    pub fn import_namespace(&mut self, local: &str, specifier: &str) -> ImportDeclaration<'arena> {
        let start = self.mint("import * as ").start;
        let local = self.ident(local);
        let local_span = local.span;
        self.mint(" from ");
        let source = self.string_literal(specifier);
        let end = self.mint(";").end;
        let mut specifiers = bumpalo::collections::Vec::new_in(self.arena);
        specifiers.push(ImportSpecifier::Namespace(ImportNamespaceSpecifier {
            local,
            span: local_span,
        }));
        ImportDeclaration {
            specifiers: specifiers.into_bump_slice(),
            source,
            attributes: None,
            import_kind: ImportKind::Value,
            phase: ImportPhase::None,
            span: Span::new(start, end),
        }
    }

    /// `<object>.<property>(<arguments>)` — a call on a synthetic member chain.
    /// The arguments slice may hold borrowed expressions (host spans).
    pub fn member_call(
        &mut self,
        object: &str,
        property: &str,
        arguments: &'arena [Expression<'arena>],
    ) -> Expression<'arena> {
        let obj = self.ident_expr(object);
        self.mint(".");
        let prop = self.ident_expr(property);
        let member_span = Span::new(obj.span().start, prop.span().end);
        self.mint("(");
        let end = self.mint(")").end;
        let callee = self.arena.alloc(Expression {
            span: member_span,
            kind: ExpressionKind::MemberExpression(MemberExpression {
                object: obj,
                property: prop,
                computed: false,
                optional: false,
            }),
        });
        Expression {
            span: Span::new(member_span.start, end),
            kind: ExpressionKind::CallExpression(CallExpression {
                callee,
                type_arguments: None,
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
    pub fn template_literal(
        &mut self,
        texts: &[String],
        expressions: &'arena [Expression<'arena>],
    ) -> Expression<'arena> {
        debug_assert_eq!(texts.len(), expressions.len() + 1);
        let start = self.mint("`").start;
        let mut quasis = bumpalo::collections::Vec::new_in(self.arena);
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
        Expression::from_template_literal(TemplateLiteral {
            quasis: quasis.into_bump_slice(),
            expressions,
            span: Span::new(start, end),
        })
    }

    /// A call on a borrowed callee expression (`d()`): the callee keeps its host
    /// span, the `()` is minted for the appendix but the CallExpression node takes
    /// the callee's **tight** span. The printer prints `()` structurally, so the
    /// tight span is output-neutral — but it keeps the empty-arg comment window
    /// (`[callee.end, call.span.end]`, `calls/call_formatting.rs`) EMPTY. Without
    /// it, an appendix-end span would sweep every host comment after the callee
    /// into the minted parens (`full_path(/* c */)`), double-printing a carried
    /// script comment when this lowers a script-position `$derived` read.
    pub fn call_expr(
        &mut self,
        callee: &'arena Expression<'arena>,
        arguments: &'arena [Expression<'arena>],
    ) -> Expression<'arena> {
        self.mint("()");
        Expression {
            span: callee.span(),
            kind: ExpressionKind::CallExpression(CallExpression {
                callee,
                type_arguments: None,
                arguments,
                optional: false,
            }),
        }
    }

    /// A call on a borrowed callee with an argument list (`foo($$renderer, x)`
    /// or, when `optional`, `foo?.($$renderer, x)`): the callee keeps its host
    /// span, the `(…)` is minted. `arguments` may mix synthetic and borrowed
    /// expressions.
    pub fn call_of(
        &mut self,
        callee: &'arena Expression<'arena>,
        arguments: &'arena [Expression<'arena>],
        optional: bool,
    ) -> Expression<'arena> {
        self.mint(if optional { "?.(" } else { "(" });
        let end = self.mint(")").end;
        Expression {
            span: Span::new(callee.span().start, end),
            kind: ExpressionKind::CallExpression(CallExpression {
                callee,
                type_arguments: None,
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
    pub fn thunk_on_body(&self, body: &'arena Expression<'arena>) -> Expression<'arena> {
        let span = body.span();
        Expression {
            span,
            kind: ExpressionKind::ArrowFunctionExpression(self.arena.alloc(
                ArrowFunctionExpression {
                    type_parameters: None,
                    params: &[],
                    body: ArrowFunctionBody::Expression(body),
                    return_type: None,
                    r#async: false,
                    params_start: None,
                    arrow_token: span.start,
                },
            )),
        }
    }

    /// `<object>.<property>(<arguments>)` — the fictional-span form of
    /// [`Self::member_call`]. Every synthetic leaf (`<object>`, `<property>`) collapses
    /// onto `anchor.start` and the call *steals* `anchor`, the host span of what it
    /// replaces (a `$derived(...)` init, a store read), so the enclosing node's windows
    /// read the authored span and the call's own internal windows hold no host comment
    /// its arguments don't claim. The interned names and the static `.`/`(`/`)` supply
    /// the text, so the fictional spans never reach output — byte-identical to the
    /// appendix-spanned form when the script carries no comments. With a zero-width
    /// `anchor` and zero-width synthetic arguments the whole call sits at one host
    /// position (the prepended `$props.id()` / `$$slots` declarations).
    pub fn member_call_at(
        &self,
        object: &str,
        property: &str,
        arguments: &'arena [Expression<'arena>],
        anchor: Span,
    ) -> Expression<'arena> {
        let low = Span::new(anchor.start, anchor.start);
        let object = self.ident_expr_at(object, low);
        let property = self.ident_expr_at(property, low);
        let callee = self.arena.alloc(Expression {
            span: low,
            kind: ExpressionKind::MemberExpression(MemberExpression {
                object,
                property,
                computed: false,
                optional: false,
            }),
        });
        Expression {
            span: anchor,
            kind: ExpressionKind::CallExpression(CallExpression {
                callee,
                type_arguments: None,
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
    pub fn void_zero_at(&self, at: u32) -> Expression<'arena> {
        Expression::from_identifier(self.ident_at("void 0", Span::new(at, at)))
    }

    /// `(<params>) => { <stmts> }` — a block-bodied arrow (the
    /// `$$renderer.component(($$renderer) => { … })` wrapper and the `$.await`
    /// pending / then callbacks). `params` may be minted synthetic identifiers
    /// (`$$renderer`) or borrowed user patterns (a `{:then value}` binding).
    /// `block_span` is the span the block's comment windows anchor on (the
    /// caller decides — host-anchored when the body holds borrowed statements).
    pub fn arrow_block(
        &mut self,
        params: &'arena [Expression<'arena>],
        body: &'arena [Statement<'arena>],
        block_span: Span,
    ) -> Expression<'arena> {
        let start = self.mint("(").start;
        let params_start = start;
        self.mint(") => {");
        let end = self.mint("}").end;
        Expression {
            span: Span::new(start, end),
            kind: ExpressionKind::ArrowFunctionExpression(self.arena.alloc(
                ArrowFunctionExpression {
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
                },
            )),
        }
    }

    /// A zero-width synthetic span at the current appendix end. For a wrapper
    /// node (`if`/`for`/block statement) that needs a span but no backing text —
    /// its keywords print statically and, with block output, no comments are
    /// carried, so the span only has to stay in-bounds.
    pub fn here(&self) -> Span {
        let pos = self.buffer.len() as u32;
        Span::new(pos, pos)
    }

    /// `<object>.<name>` — a non-computed member on a synthetic property name
    /// (`each_array.length`).
    pub fn member_prop(
        &mut self,
        object: &'arena Expression<'arena>,
        name: &str,
    ) -> Expression<'arena> {
        self.mint(".");
        let prop = self.ident_expr(name);
        let span = Span::new(object.span().start, prop.span().end);
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
    pub fn member_computed(
        &mut self,
        object: &'arena Expression<'arena>,
        index: &'arena Expression<'arena>,
    ) -> Expression<'arena> {
        self.mint("[");
        let end = self.mint("]").end;
        let span = Span::new(object.span().start, end);
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
    pub fn binary(
        &mut self,
        left: &'arena Expression<'arena>,
        op: BinaryOperator,
        right: &'arena Expression<'arena>,
    ) -> Expression<'arena> {
        self.mint(&format!(" {} ", op.as_str()));
        let span = Span::new(left.span().start, right.span().end);
        Expression {
            span,
            kind: ExpressionKind::BinaryExpression(BinaryExpression {
                left,
                operator: op,
                right,
                // Minted code, not source the printer will re-lex: the builder emits its own
                // spelling, and no shape it mints opens a type-argument region.
                relexes_as_type_arguments: false,
            }),
        }
    }

    /// `<argument>++` / `<argument>--` (postfix) — an update expression.
    pub fn update(
        &mut self,
        argument: &'arena Expression<'arena>,
        op: UpdateOperator,
    ) -> Expression<'arena> {
        let text = if op == UpdateOperator::Increment {
            "++"
        } else {
            "--"
        };
        let end = self.mint(text).end;
        let span = Span::new(argument.span().start, end);
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
    pub fn array_of(
        &mut self,
        elements: &'arena [Option<Expression<'arena>>],
    ) -> Expression<'arena> {
        let start = self.mint("[").start;
        let end = self.mint("]").end;
        Expression {
            span: Span::new(start, end),
            kind: ExpressionKind::ArrayExpression(tsv_ts::ast::internal::ArrayExpression {
                elements,
                spread_trailing_comma: false,
            }),
        }
    }

    /// A numeric literal expression (`0`).
    pub fn number(&mut self, value: f64) -> Expression<'arena> {
        let span = self.mint(&format!("{value}"));
        Expression::from_literal(Literal {
            value: LiteralValue::Number(value),
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
    pub fn expression_statement(&self, expression: Expression<'arena>) -> Statement<'arena> {
        let span = expression.span();
        Statement::ExpressionStatement(ExpressionStatement {
            expression: self.arena.alloc(expression),
            span,
            is_directive: false,
        })
    }

    /// `$$renderer.push('<text>')` — a block anchor push with a *string-literal*
    /// argument (single-quoted after canonicalization), distinct from the
    /// template-literal pushes the `BodyBuilder` flushes. `text` is a hydration
    /// anchor comment (`<!--[0-->`, `<!--[-->`, …) — never needs escaping.
    pub fn push_string_stmt(&mut self, text: &str) -> Statement<'arena> {
        let arg = self.string_literal_expr(text);
        let arg_alloc = self.arena.alloc(arg);
        let call = self.member_call("$$renderer", "push", std::slice::from_ref(arg_alloc));
        self.expression_statement(call)
    }

    /// `void 0` — the oracle's spelling of an absent rune argument.
    pub fn void_zero(&mut self) -> Expression<'arena> {
        let span = self.mint("void 0");
        let zero = self.arena.alloc(Expression::from_literal(Literal {
            value: LiteralValue::Number(0.0),
            span: Span::new(span.end - 1, span.end),
        }));
        Expression::from_unary_expression(UnaryExpression {
            operator: UnaryOperator::Void,
            argument: zero,
            prefix: true,
            span,
        })
    }

    /// A `true` literal (the `$.attr(name, value, true)` boolean-attribute arg).
    pub fn true_literal(&mut self) -> Expression<'arena> {
        let span = self.mint("true");
        Expression::from_literal(Literal {
            value: LiteralValue::Boolean(true),
            span,
        })
    }

    /// A single-quoted string literal expression.
    pub fn string_literal_expr(&mut self, content: &str) -> Expression<'arena> {
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
    pub fn store_get(&self, base: &str, base_is_derived: bool, read: Span) -> Expression<'arena> {
        let at = Span::new(read.start, read.start);
        let mut args: bumpalo::collections::Vec<'arena, Expression<'arena>> =
            bumpalo::collections::Vec::new_in(self.arena);
        args.push(self.store_subs_assign(at));
        args.push(Expression::from_identifier(
            self.ident_at(&format!("'${base}'"), at),
        ));
        args.push(self.store_base_value(base, base_is_derived, at));
        self.member_call_at("$", "store_get", args.into_bump_slice(), read)
    }

    /// `($$store_subs ??= {})` zero-width at `at` ([`Self::store_get`]).
    fn store_subs_assign(&self, at: Span) -> Expression<'arena> {
        let obj = self.arena.alloc(Expression {
            span: at,
            kind: ExpressionKind::ObjectExpression(ObjectExpression {
                properties: &[],
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

    /// The store's value expression — `<base>()` when `base` is a `$derived` binding,
    /// else the bare `<base>` identifier — zero-width at `at` ([`Self::store_get`]).
    fn store_base_value(&self, base: &str, base_is_derived: bool, at: Span) -> Expression<'arena> {
        if base_is_derived {
            Expression {
                span: at,
                kind: ExpressionKind::CallExpression(CallExpression {
                    callee: self.ident_expr_at(base, at),
                    type_arguments: None,
                    arguments: &[],
                    optional: false,
                }),
            }
        } else {
            Expression::from_identifier(self.ident_at(base, at))
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
    pub fn store_set(
        &self,
        base: &str,
        value: Expression<'arena>,
        assign: Span,
    ) -> Expression<'arena> {
        let at = value.span().start;
        let base_ident = Expression::from_identifier(self.ident_at(base, Span::new(at, at)));
        let mut args: bumpalo::collections::Vec<'arena, Expression<'arena>> =
            bumpalo::collections::Vec::new_in(self.arena);
        args.push(base_ident);
        args.push(value);
        self.member_call_at("$", "store_set", args.into_bump_slice(), assign)
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
    pub fn update_store(
        &self,
        base: &str,
        prefix: bool,
        decrement: bool,
        update: Span,
    ) -> Expression<'arena> {
        let at = Span::new(update.start, update.start);
        let mut args: bumpalo::collections::Vec<'arena, Expression<'arena>> =
            bumpalo::collections::Vec::new_in(self.arena);
        args.push(self.store_subs_assign(at));
        args.push(Expression::from_identifier(
            self.ident_at(&format!("'${base}'"), at),
        ));
        args.push(Expression::from_identifier(self.ident_at(base, at)));
        if decrement {
            args.push(Expression::from_identifier(self.ident_at("-1", at)));
        }
        let property = if prefix {
            "update_store_pre"
        } else {
            "update_store"
        };
        self.member_call_at("$", property, args.into_bump_slice(), update)
    }

    /// `var $$store_subs;` — the store-subscription accumulator, injected as a
    /// component-body statement when any store read compiled. Zero-width at `at`, the
    /// body block's start, for the reason `transform_server.rs::build_props_id_decl`
    /// gives.
    pub fn store_subs_var_at(&self, at: u32) -> Statement<'arena> {
        let span = Span::new(at, at);
        let id = Expression::from_identifier(self.ident_at("$$store_subs", span));
        let declarator = VariableDeclarator {
            id: self.arena.alloc(id),
            init: None,
            definite: false,
            span,
        };
        let decls = std::slice::from_ref(self.arena.alloc(declarator));
        Statement::VariableDeclaration(VariableDeclaration {
            kind: VariableDeclarationKind::Var,
            declarations: decls,
            declare: false,
            span,
        })
    }

    /// `if ($$store_subs) $.unsubscribe_stores($$store_subs);` — the store
    /// cleanup, injected as the component body's last statement (before any
    /// `$.bind_props`).
    pub fn unsubscribe_stores_stmt(&mut self) -> Statement<'arena> {
        let test = Expression::from_identifier(self.ident("$$store_subs"));
        let test_start = test.span().start;
        let subs_arg = self.ident_expr("$$store_subs");
        let call = self.member_call("$", "unsubscribe_stores", std::slice::from_ref(subs_arg));
        let call_span = call.span();
        let consequent = self.arena.alloc(self.expression_statement(call));
        Statement::IfStatement(IfStatement {
            test: self.arena.alloc(test),
            consequent,
            alternate: None,
            span: Span::new(test_start, call_span.end),
        })
    }

    /// `function <name>(<params>) { <body> }` — a named function declaration
    /// (the emitted snippet function). `name` rides the interned-name channel;
    /// `params` may mix the synthetic `$$renderer` identifier with borrowed
    /// snippet parameter patterns (host spans). `block_span` anchors the body's
    /// comment windows (host-anchored when the body holds borrowed statements).
    ///
    /// **`type_parameters` is always `None`, and that is a contract, not an
    /// accident**: it is how a generic `{#snippet s<T>(x: T)}` erases its `<T>`.
    /// The clause is type-level only — the oracle emits `function s($$renderer, x)`
    /// either way — so *not reading it* IS the erasure. Threading a caller's type
    /// parameters through here would silently print them into the compiled JS.
    pub fn function_declaration(
        &mut self,
        name: &str,
        params: &'arena [Expression<'arena>],
        body: &'arena [Statement<'arena>],
        block_span: Span,
    ) -> Statement<'arena> {
        let start = self.mint("function ").start;
        let id = self.ident(name);
        let params_start = self.mint("(").start;
        self.mint(") {");
        let end = self.mint("}").end;
        Statement::FunctionDeclaration(self.arena.alloc(FunctionDeclaration {
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
            span: Span::new(start, end),
        }))
    }
}

/// One plain `key: value` object property — the shape all 14 synthetic-property
/// sites build (`kind: Init`, never computed, never a method).
///
/// `span` is an explicit parameter rather than derived from `key`: thirteen sites
/// use the key's own span, but `script_props`'s injected `$$slots`/`$$events`
/// property spans key→value, and deriving it would silently change that one site.
pub(crate) fn init_property<'arena>(
    arena: &'arena Bump,
    key: Expression<'arena>,
    value: Expression<'arena>,
    shorthand: bool,
    span: Span,
) -> Property<'arena> {
    Property {
        key: arena.alloc(key),
        value: arena.alloc(value),
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
