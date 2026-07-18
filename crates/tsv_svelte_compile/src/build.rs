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
//! - **Synthetic identifiers** ride the interned-name channel
//!   (`IdentName { escaped: Some(symbol), raw_len: 0 }`) — resolved through the
//!   shared interner, source-free. Their spans are still backed by minted text
//!   for debuggability (the appendix reads as the generated skeleton).
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
use tsv_lang::{SharedInterner, Span};
use tsv_ts::ast::internal::{
    ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression, AssignmentOperator,
    BinaryExpression, BinaryOperator, BlockStatement, CallExpression, Expression,
    ExpressionStatement, FunctionDeclaration, IdentName, Identifier, IfStatement,
    ImportDeclaration, ImportKind, ImportNamespaceSpecifier, ImportPhase, ImportSpecifier, Literal,
    LiteralValue, MemberExpression, ObjectExpression, Statement, StringCooked, TemplateCooked,
    TemplateElement, TemplateLiteral, UnaryExpression, UnaryOperator, UpdateExpression,
    UpdateOperator, VariableDeclaration, VariableDeclarationKind, VariableDeclarator,
};

/// The appendix-buffer bookkeeping plus interner access — everything node
/// constructors need. Owns the growing buffer; the arena and interner are
/// shared with the parsed host AST so borrowed subtrees and synthetic nodes
/// coexist in one graph.
pub(crate) struct Builder<'arena> {
    pub arena: &'arena Bump,
    /// Host source + appendix of minted lexemes. Passed to `format_canonical`
    /// as the source every span in the synthetic program indexes into.
    pub buffer: String,
    /// The parse's interner — synthetic identifier names are interned here so
    /// the printer's symbol resolver (built from `Program.interner`) finds them.
    pub interner: SharedInterner,
}

impl<'arena> Builder<'arena> {
    pub fn new(arena: &'arena Bump, host_source: &str, interner: SharedInterner) -> Self {
        Self {
            arena,
            buffer: host_source.to_string(),
            interner,
        }
    }

    /// Append minted text to the appendix, returning its span.
    pub fn mint(&mut self, text: &str) -> Span {
        let start = self.buffer.len() as u32;
        self.buffer.push_str(text);
        Span::new(start, self.buffer.len() as u32)
    }

    /// A synthetic identifier: interned name (source-free resolution) with its
    /// text minted into the appendix so the span is backed.
    pub fn ident(&mut self, name: &str) -> Identifier<'arena> {
        let span = self.mint(name);
        let ident_name = IdentName {
            escaped: Some(self.interner.borrow_mut().get_or_intern(name)),
            raw_len: 0,
        };
        Identifier::simple(ident_name, span)
    }

    /// A synthetic identifier as an arena-allocated expression.
    pub fn ident_expr(&mut self, name: &str) -> &'arena Expression<'arena> {
        let ident = self.ident(name);
        self.arena.alloc(Expression::Identifier(ident))
    }

    /// A synthetic identifier at a caller-chosen span (no minting). The
    /// interned-name channel never extracts the span, so the span's only job is
    /// steering the printer's comment windows — a *fictional* low span keeps a
    /// synthetic header node's windows empty/inverted, and a *stolen* host span
    /// (the node it replaces, e.g. `$$props` over the original `$props()` call)
    /// keeps the surrounding gaps exactly the authored ones.
    pub fn ident_at(&self, name: &str, span: Span) -> Identifier<'arena> {
        let ident_name = IdentName {
            escaped: Some(self.interner.borrow_mut().get_or_intern(name)),
            raw_len: 0,
        };
        Identifier::simple(ident_name, span)
    }

    /// [`Self::ident_at`] as an arena-allocated expression (no minting — the
    /// interned-name channel supplies the text, so the span steers comment
    /// windows only).
    pub fn ident_expr_at(&self, name: &str, span: Span) -> &'arena Expression<'arena> {
        self.arena
            .alloc(Expression::Identifier(self.ident_at(name, span)))
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
        let callee = self
            .arena
            .alloc(Expression::MemberExpression(MemberExpression {
                object: obj,
                property: prop,
                computed: false,
                optional: false,
                span: member_span,
            }));
        Expression::CallExpression(CallExpression {
            callee,
            type_arguments: None,
            arguments,
            optional: false,
            span: Span::new(member_span.start, end),
        })
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
        Expression::TemplateLiteral(TemplateLiteral {
            quasis: quasis.into_bump_slice(),
            expressions,
            span: Span::new(start, end),
        })
    }

    /// A call on a borrowed callee expression (`d()`): the callee keeps its
    /// host span, the `()` is minted.
    pub fn call_expr(
        &mut self,
        callee: &'arena Expression<'arena>,
        arguments: &'arena [Expression<'arena>],
    ) -> Expression<'arena> {
        let end = self.mint("()").end;
        Expression::CallExpression(CallExpression {
            callee,
            type_arguments: None,
            arguments,
            optional: false,
            span: Span::new(callee.span().start, end),
        })
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
        Expression::CallExpression(CallExpression {
            callee,
            type_arguments: None,
            arguments,
            optional,
            span: Span::new(callee.span().start, end),
        })
    }

    /// `() => <body>` for the `$derived` thunk, with every synthetic span
    /// collapsed onto `anchor` (the replaced `$derived(...)` init's host span)
    /// instead of the appendix. Only the borrowed `body` keeps its host span;
    /// `() => ` is static punctuation, so the fictional spans never reach output.
    /// This keeps the enclosing `$.derived(...)` call's argument comment windows
    /// empty for a carried script comment (see [`Self::derived_call`]).
    pub fn arrow_expr_at(
        &self,
        anchor: Span,
        body: &'arena Expression<'arena>,
    ) -> Expression<'arena> {
        Expression::ArrowFunctionExpression(ArrowFunctionExpression {
            type_parameters: None,
            params: &[],
            body: ArrowFunctionBody::Expression(body),
            return_type: None,
            r#async: false,
            params_start: Some(anchor.start),
            arrow_token: anchor.start,
            span: anchor,
        })
    }

    /// `$.derived(<argument>)` for a `$derived` / `$derived.by` rewrite. Every
    /// synthetic leaf (`$`, `derived`) collapses onto `anchor.start` and the
    /// outer call span *steals* `anchor` — the replaced `$derived(...)` init's
    /// host span — so the enclosing declarator's `=`-gap window and the call's
    /// own internal windows stay empty for a carried script comment (the same
    /// fictional-span discipline the `$$props` span-steal uses). The interned
    /// `$`/`derived` names and the static `.`/`(`/`)` supply the text, so the
    /// fictional spans never reach output: a comment after the declarator flows
    /// to the next statement instead of being swept into the `$.derived(...)`
    /// slot. Byte-identical to the appendix-spanned [`Self::member_call`] form
    /// when the script carries no comments.
    pub fn derived_call(
        &self,
        anchor: Span,
        argument: &'arena Expression<'arena>,
    ) -> Expression<'arena> {
        let low = Span::new(anchor.start, anchor.start);
        let object = self.ident_expr_at("$", low);
        let property = self.ident_expr_at("derived", low);
        let callee = self
            .arena
            .alloc(Expression::MemberExpression(MemberExpression {
                object,
                property,
                computed: false,
                optional: false,
                span: low,
            }));
        Expression::CallExpression(CallExpression {
            callee,
            type_arguments: None,
            arguments: std::slice::from_ref(argument),
            optional: false,
            span: anchor,
        })
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
        Expression::ArrowFunctionExpression(ArrowFunctionExpression {
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
            span: Span::new(start, end),
        })
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
        Expression::MemberExpression(MemberExpression {
            object,
            property: prop,
            computed: false,
            optional: false,
            span,
        })
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
        Expression::MemberExpression(MemberExpression {
            object,
            property: index,
            computed: true,
            optional: false,
            span,
        })
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
        Expression::BinaryExpression(BinaryExpression {
            left,
            operator: op,
            right,
            span,
        })
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
        Expression::UpdateExpression(UpdateExpression {
            operator: op,
            argument,
            prefix: false,
            span,
        })
    }

    /// A numeric literal expression (`0`).
    pub fn number(&mut self, value: f64) -> Expression<'arena> {
        let span = self.mint(&format!("{value}"));
        Expression::Literal(Literal {
            value: LiteralValue::Number(value),
            span,
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
        let span = call.span();
        Statement::ExpressionStatement(ExpressionStatement {
            expression: call,
            span,
            is_directive: false,
        })
    }

    /// `void 0` — the oracle's spelling of an absent rune argument.
    pub fn void_zero(&mut self) -> Expression<'arena> {
        let span = self.mint("void 0");
        let zero = self.arena.alloc(Expression::Literal(Literal {
            value: LiteralValue::Number(0.0),
            span: Span::new(span.end - 1, span.end),
        }));
        Expression::UnaryExpression(UnaryExpression {
            operator: UnaryOperator::Void,
            argument: zero,
            prefix: true,
            span,
        })
    }

    /// A `true` literal (the `$.attr(name, value, true)` boolean-attribute arg).
    pub fn true_literal(&mut self) -> Expression<'arena> {
        let span = self.mint("true");
        Expression::Literal(Literal {
            value: LiteralValue::Boolean(true),
            span,
        })
    }

    /// A single-quoted string literal expression.
    pub fn string_literal_expr(&mut self, content: &str) -> Expression<'arena> {
        Expression::Literal(self.string_literal(content))
    }

    /// `$$store_subs ??= {}` — the store-subscription accumulator argument shared
    /// by [`store_get`](Self::store_get) and [`update_store`](Self::update_store).
    /// The printer parenthesizes this `??=` assignment to match the canonical
    /// form (`($$store_subs ??= {})`).
    fn store_subs_assign(&mut self) -> Expression<'arena> {
        let subs_left = self.ident_expr("$$store_subs");
        self.mint(" ??= ");
        let obj_span = self.mint("{}");
        let obj = self
            .arena
            .alloc(Expression::ObjectExpression(ObjectExpression {
                properties: &[],
                spread_trailing_comma: false,
                span: obj_span,
            }));
        Expression::AssignmentExpression(AssignmentExpression {
            left: subs_left,
            operator: AssignmentOperator::NullishAssign,
            right: obj,
            span: Span::new(subs_left.span().start, obj_span.end),
        })
    }

    /// The store's value expression — `<base>()` when `base` is a `$derived`
    /// binding (the store the derived currently holds), else the bare `<base>`
    /// identifier.
    fn store_base_value(&mut self, base: &str, base_is_derived: bool) -> Expression<'arena> {
        if base_is_derived {
            let callee = self.ident_expr(base);
            self.call_expr(callee, &[])
        } else {
            Expression::Identifier(self.ident(base))
        }
    }

    /// `$.store_get(($$store_subs ??= {}), '$<base>', <base>)` — the oracle's SSR
    /// store auto-subscription read (`Identifier.js` → `serialize_get_binding` for a
    /// `store_sub` binding). `base` is the `$`-stripped store name; the string key
    /// keeps the leading `$` (`'$count'`). The printer parenthesizes the `??=`
    /// assignment argument to match the canonical form.
    pub fn store_get(&mut self, base: &str, base_is_derived: bool) -> Expression<'arena> {
        let assign = self.store_subs_assign();
        let name_lit = self.string_literal_expr(&format!("${base}"));
        // The store base is read like any binding: a `$derived` base reads `d()`.
        let base_value = self.store_base_value(base, base_is_derived);
        let mut args: bumpalo::collections::Vec<'arena, Expression<'arena>> =
            bumpalo::collections::Vec::new_in(self.arena);
        args.push(assign);
        args.push(name_lit);
        args.push(base_value);
        self.member_call("$", "store_get", args.into_bump_slice())
    }

    /// `$.store_set(<base>, <value>)` — the oracle's SSR store write
    /// (`AssignmentExpression.js` → `serialize_set_binding` for a `store_sub`
    /// binding). `base` is the `$`-stripped store name (the store object is
    /// referenced bare, never `$$store_subs`); `value` is the already-rewritten
    /// right-hand side (a compound `+=` is reconstructed as `store_get(...) <op>
    /// rhs` by the caller).
    pub fn store_set(&mut self, base: &str, value: Expression<'arena>) -> Expression<'arena> {
        let base_ident = Expression::Identifier(self.ident(base));
        let mut args: bumpalo::collections::Vec<'arena, Expression<'arena>> =
            bumpalo::collections::Vec::new_in(self.arena);
        args.push(base_ident);
        args.push(value);
        self.member_call("$", "store_set", args.into_bump_slice())
    }

    /// `$.update_store[_pre](($$store_subs ??= {}), '$<base>', <base>[, -1])` — the
    /// oracle's SSR store increment/decrement (`UpdateExpression.js`). `prefix`
    /// selects `update_store_pre` (`++$x` / `--$x`) over `update_store`
    /// (`$x++` / `$x--`); `decrement` appends the trailing `-1` argument
    /// (increment elides it). The printer parenthesizes the `??=` assignment
    /// argument, like [`store_get`](Self::store_get).
    pub fn update_store(
        &mut self,
        base: &str,
        prefix: bool,
        decrement: bool,
    ) -> Expression<'arena> {
        let assign = self.store_subs_assign();
        let name_lit = self.string_literal_expr(&format!("${base}"));
        let base_ident = Expression::Identifier(self.ident(base));
        let mut args: bumpalo::collections::Vec<'arena, Expression<'arena>> =
            bumpalo::collections::Vec::new_in(self.arena);
        args.push(assign);
        args.push(name_lit);
        args.push(base_ident);
        if decrement {
            args.push(self.number(-1.0));
        }
        let property = if prefix {
            "update_store_pre"
        } else {
            "update_store"
        };
        self.member_call("$", property, args.into_bump_slice())
    }

    /// `var $$store_subs;` — the store-subscription accumulator, injected as a
    /// component-body statement when any store read compiled.
    pub fn store_subs_var(&mut self) -> Statement<'arena> {
        let id = Expression::Identifier(self.ident("$$store_subs"));
        let span = id.span();
        let declarator = VariableDeclarator {
            id,
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
        let test = Expression::Identifier(self.ident("$$store_subs"));
        let test_start = test.span().start;
        let subs_arg = self.ident_expr("$$store_subs");
        let call = self.member_call("$", "unsubscribe_stores", std::slice::from_ref(subs_arg));
        let call_span = call.span();
        let consequent = self
            .arena
            .alloc(Statement::ExpressionStatement(ExpressionStatement {
                expression: call,
                span: call_span,
                is_directive: false,
            }));
        Statement::IfStatement(IfStatement {
            test,
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
        Statement::FunctionDeclaration(FunctionDeclaration {
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
        })
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
