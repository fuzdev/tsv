// Function/method parameter and destructuring-pattern parsing.

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use crate::parser::expression_assignable::AssignableContext;
use tsv_lang::{ParseError, Span};

use super::Parser;

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Parse a simple parameter: identifier with optional `?`, type annotation, and default value.
    ///
    /// Accepts both regular identifiers and contextual keywords (`from`, `as`, etc.).
    /// Does NOT handle parameter property modifiers (`public`, `private`, `readonly`).
    fn parse_simple_param(&mut self) -> Result<Expression<'arena>, ParseError> {
        let (param_start, param_end) = self.current_pos();
        let name = self
            .try_param_name()
            .ok_or_else(|| self.error_expected("parameter name or destructuring pattern"))?;
        self.advance()?;

        // Check for optional marker: param? — a bare `?` extends the span.
        let (optional, param_end) = self.eat_optional_marker(param_end);

        // Check for type annotation: param: type
        let (type_annotation, id_end) = if self.check(&TokenKind::Colon) {
            let ta = self.parse_type_annotation()?;
            let end = ta.span.end;
            (Some(ta), end as usize)
        } else {
            (None, param_end)
        };

        // Build binding extra only when a type annotation is present (decorators,
        // if any, are attached later by the parameter-list caller).
        let extra = type_annotation.map(|ta| self.typed_extra(ta));

        let mut param = Expression::Identifier(Identifier {
            escaped_name: name.escaped,
            name_len: name.raw_len,
            optional,
            extra,
            span: Span::new(param_start as u32, id_end as u32),
        });

        // Check for default value: param = default
        if self.eat(TokenKind::Equals) {
            let default_value = self.parse_assignment_expression_ref()?;
            // prev_token_end covers a parenthesized default's closing `)`
            let assign_end = self.prev_token_end() as u32;
            param = Expression::AssignmentPattern(AssignmentPattern {
                left: self.alloc(param),
                right: default_value,
                decorators: None,
                span: Span::new(param_start as u32, assign_end),
            });
        }
        Ok(param)
    }

    /// Attach parameter decorators to a freshly-parsed parameter's **top-level
    /// binding node**, matching acorn: a parameter's decorators live on the
    /// `Identifier` / `AssignmentPattern` / `ObjectPattern` / `ArrayPattern` it
    /// binds — reaching *inside* a `TSParameterProperty` onto its `.parameter`
    /// (the property node's span still starts at the modifier). Notably the
    /// default-value case (`@dec a = 1`) attaches to the `AssignmentPattern`, not
    /// its `left`.
    fn attach_param_decorators(
        &self,
        mut param: Expression<'arena>,
        decorators: &'arena [Decorator<'arena>],
    ) -> Expression<'arena> {
        let arena = self.arena;
        match &mut param {
            // `@dec private a` — decorators ride the inner binding, not the
            // property wrapper. Recurse (the inner is an Identifier or, for
            // `@dec private a = 1`, an AssignmentPattern).
            Expression::TSParameterProperty(pp) => {
                let inner = self.attach_param_decorators((*pp.parameter).clone(), decorators);
                pp.parameter = arena.alloc(inner);
                param
            }
            Expression::Identifier(id) => {
                let type_annotation = id.type_annotation().cloned();
                id.extra = Some(arena.alloc(IdentifierParamExtra {
                    type_annotation,
                    decorators: Some(decorators),
                }));
                param
            }
            Expression::AssignmentPattern(ap) => {
                ap.decorators = Some(decorators);
                param
            }
            Expression::ObjectPattern(op) => {
                op.decorators = Some(decorators);
                param
            }
            Expression::ArrayPattern(arr) => {
                arr.decorators = Some(decorators);
                param
            }
            _ => param,
        }
    }

    /// Parse a bare array/object binding pattern (no trailing `: Type`).
    ///
    /// Current token must be `[` or `{`. The type annotation is left for the
    /// caller: a destructured *parameter* / *declarator* binds the type on the
    /// pattern (`parse_destructured_binding`), whereas a rest element binds it on
    /// the enclosing `RestElement` (`...[a, b]: T`), matching acorn.
    pub(super) fn parse_binding_pattern(&mut self) -> Result<Expression<'arena>, ParseError> {
        let expr = if self.check(&TokenKind::BracketOpen) {
            self.parse_array_expression()?
        } else {
            self.parse_object_expression()?
        };
        // Binding context: a type assertion is not a valid binding target
        // (`let [x as T] = …` / `function f([x as T])` reject, matching acorn).
        self.to_assignable(expr, AssignableContext::Binding)
    }

    /// Parse a destructuring binding (`[a, b]` / `{a, b}`) with an optional
    /// type annotation attached to the resulting pattern.
    ///
    /// Current token must be `[` or `{`. Shared by parameter lists and
    /// variable declarators; default values are handled by the caller.
    ///
    /// `allow_optional` permits a `?` after the pattern (`[a]?` / `{a}?`),
    /// extending the span and preceding any type annotation. It is set only in
    /// parameter positions: acorn accepts `[a]?` syntactically in every parameter
    /// context and rejects it *semantically* outside ambient/type contexts ("A
    /// binding pattern parameter cannot be optional in an implementation
    /// signature") — tsv parses it everywhere, deferring that named-semantic
    /// early-error (a sanctioned `canonical-fails-tsv-ok` divergence, like ambient
    /// initializers). Declarator/for-loop callers pass `false`, so `const []? = x`
    /// still rejects in both parsers.
    pub(super) fn parse_destructured_binding(
        &mut self,
        allow_optional: bool,
    ) -> Result<Expression<'arena>, ParseError> {
        let mut pattern = self.parse_binding_pattern()?;

        // Optional marker `?` (parameter position only), between the pattern and
        // any type annotation; extends the span to the `?` end (matching acorn).
        if allow_optional && self.eat(TokenKind::Question) {
            let q_end = self.prev_token_end() as u32;
            match &mut pattern {
                Expression::ArrayPattern(p) => {
                    p.optional = true;
                    p.span.end = q_end;
                }
                Expression::ObjectPattern(p) => {
                    p.optional = true;
                    p.span.end = q_end;
                }
                _ => {}
            }
        }

        // Check for type annotation: [a, b]: Type or {a, b}: Type
        if self.check(&TokenKind::Colon) {
            let type_annotation = self.parse_type_annotation()?;
            let end = type_annotation.span.end;
            match &mut pattern {
                Expression::ArrayPattern(p) => {
                    p.type_annotation = Some(type_annotation);
                    p.span.end = end;
                }
                Expression::ObjectPattern(p) => {
                    p.type_annotation = Some(type_annotation);
                    p.span.end = end;
                }
                _ => {}
            }
        }
        Ok(pattern)
    }

    /// Parse a rest parameter's trailing optional `?` and `: Type` (in that
    /// order), after its binding argument — whose natural end is `arg_end` — has
    /// been parsed. Both bind to the `RestElement`, never to the argument: the `?`
    /// is invalid TypeScript (TS1047) but a deferred grammar-check tsv preserves
    /// (see `RestElement`), and a rest param's annotation lives on the rest
    /// element. Returns `(optional, type_annotation, end)`, `end` being the rest
    /// element's end offset. Shared by the value-signature rest paths
    /// (`parse_parameters`) and the function-type rest path (`types.rs`), so a new
    /// rest-param position picks up the `?` handling by construction.
    pub(in crate::parser) fn parse_rest_param_tail(
        &mut self,
        arg_end: u32,
    ) -> Result<(bool, Option<TSTypeAnnotation<'arena>>, u32), ParseError> {
        let optional = self.eat(TokenKind::Question);
        if self.check(&TokenKind::Colon) {
            let ta = self.parse_type_annotation()?;
            let end = ta.span.end;
            Ok((optional, Some(ta), end))
        } else if optional {
            // The `?` is the last token consumed; it ends the rest element.
            Ok((optional, None, self.prev_token_end() as u32))
        } else {
            Ok((optional, None, arg_end))
        }
    }

    /// Consume the contextual keyword `kw` as a parameter-property modifier iff
    /// the current token is the identifier `kw` and the next token begins a
    /// parameter binding **on the same line** — otherwise `kw` is itself the
    /// parameter name (`constructor(readonly)`, `f(override: T)`,
    /// `constructor(public readonly)`). The same-line requirement is a
    /// `[no LineTerminator here]` rule mirroring tsc's `nextTokenCanFollowModifier`
    /// (`nextTokenIsOnSameLineAndCanFollowModifier` → `!hasPrecedingLineBreak`) and
    /// `class.rs`'s `eat_modifier_keyword` twin: a break between the modifier and
    /// the binding demotes the keyword, so `constructor(readonly⏎x)` rejects (the
    /// keyword becomes the binding, the next-line token is then unexpected). The
    /// parameter-list analog of `eat_modifier_keyword` (which keys on a
    /// class-member-name lookahead). Returns whether it was consumed.
    fn eat_param_modifier_keyword(&mut self, kw: &str) -> Result<bool, ParseError> {
        if matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == kw
            && self.peek_starts_parameter_binding()
            && !self.peek_preceded_by_line_terminator()
        {
            self.advance()?; // consume the modifier keyword
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Consume an accessibility keyword (`public`/`private`/`protected`) as a
    /// parameter-property modifier iff it is on the same line as the binding that
    /// follows. Unlike `override`/`readonly`, accessibility keywords are
    /// strict-mode reserved words that cannot be parameter names, so no
    /// binding-lookahead is needed — but the same `[no LineTerminator here]` rule
    /// applies (tsc's `nextTokenCanFollowModifier`): a break demotes the keyword,
    /// so `constructor(public⏎x)` rejects. The greedy `eat_contextual_keyword` has
    /// no line-break guard, so the accessibility call sites use this instead. When
    /// the guard fails the keyword falls through to be parsed as the binding and
    /// the next-line token is then unexpected → tsv rejects (the reserved-word
    /// early-error on the keyword itself stays deferred).
    fn eat_param_accessibility_keyword(&mut self, kw: &str) -> bool {
        matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == kw
            && !self.peek_preceded_by_line_terminator()
            && self.try_advance()
    }

    /// Parse a parenthesized parameter list: `(a, b, c)`, **allowing** parameter
    /// decorators (`@dec a`).
    ///
    /// Used by function declarations/expressions, class methods and the
    /// constructor, object-literal methods, and ambient `declare function`s —
    /// every position acorn's `parseAssignableListItem` parses decorators in.
    /// Consumes both opening and closing parentheses.
    pub(super) fn parse_parameter_list(
        &mut self,
    ) -> Result<bumpalo::collections::Vec<'arena, Expression<'arena>>, ParseError> {
        self.parse_parameter_list_inner(true)
    }

    /// Parse a parenthesized parameter list that **rejects** parameter decorators.
    ///
    /// Used where a decorator is a grammar error acorn also rejects: arrow-function
    /// parameters (acorn parses the `(…)` as a parenthesized expression first, where
    /// a leading `@` is a class decorator — "Leading decorators must be attached to a
    /// class declaration") and type-member signatures (interface / type-literal
    /// method, call, construct, and accessor signatures — "Unexpected character
    /// '@'"). tsc and prettier reject both ("Decorators are not valid here"), so this
    /// is an unconditional-local grammar violation rejected inline for drop-in parity.
    pub(super) fn parse_parameter_list_no_decorators(
        &mut self,
    ) -> Result<bumpalo::collections::Vec<'arena, Expression<'arena>>, ParseError> {
        self.parse_parameter_list_inner(false)
    }

    /// Shared parameter-list body. `allow_decorators` gates whether a leading `@` on
    /// a parameter is parsed as a decorator (`true`) or rejected (`false`).
    fn parse_parameter_list_inner(
        &mut self,
        allow_decorators: bool,
    ) -> Result<bumpalo::collections::Vec<'arena, Expression<'arena>>, ParseError> {
        self.expect(&TokenKind::ParenOpen)?;

        let mut params = self.bvec();
        if !self.check(&TokenKind::ParenClose) {
            loop {
                // Parameter decorators: `@dec1 @dec2 <param>`. Parsed up front so the
                // parameter body below can be any binding form — a parameter property
                // (`@dec private a`) or a destructuring pattern (`@dec { a }: T`), not
                // just a bare identifier — and the decorators are attached afterwards.
                let decorators: &'arena [Decorator<'arena>] = if self.check(&TokenKind::At) {
                    // Reject decorators where the position forbids them (arrow
                    // parameters, type-member signatures). Checked per-parameter, so
                    // a mid-list decorator (`(a, @dec b)`) rejects too.
                    if !allow_decorators {
                        return Err(self.error_msg("Decorators are not valid here"));
                    }
                    // Uses the shared restricted ES decorators grammar
                    // (`parse_decorator_expression`), NOT a full assignment
                    // expression — so `@dec [a, b]` reads as a decorator on `dec`
                    // plus an array-pattern parameter, not the computed member
                    // `dec[a, b]` (matching acorn).
                    let decorators = self.parse_decorators()?;
                    // Decorators are not valid on a rest parameter (acorn rejects `@d ...a`).
                    if self.check(&TokenKind::DotDotDot) {
                        return Err(self.error_msg("Decorators are not valid on a rest parameter"));
                    }
                    decorators.into_bump_slice()
                } else {
                    &[]
                };

                // Parse parameter: identifier, parameter property, or destructuring pattern
                let param = match self.current_kind() {
                    TokenKind::Identifier => {
                        let param_start = self.current_pos().0;

                        // Check for parameter property modifiers: public, private, protected, readonly.
                        // Accessibility keywords are strict-mode reserved words, so they cannot be
                        // ordinary parameter names (acorn rejects bare `public`/`private`/`protected`
                        // here too) — eating them is correct, but only on the same line as the
                        // binding (`[no LineTerminator here]`; see `eat_param_accessibility_keyword`).
                        // `override` and `readonly`, by contrast, are contextual keywords that ARE
                        // valid parameter names, so each is treated as a modifier only when a binding
                        // follows it on the same line (see below).
                        let accessibility = if self.eat_param_accessibility_keyword("public") {
                            Some(Accessibility::Public)
                        } else if self.eat_param_accessibility_keyword("private") {
                            Some(Accessibility::Private)
                        } else if self.eat_param_accessibility_keyword("protected") {
                            Some(Accessibility::Protected)
                        } else {
                            None
                        };

                        // Check for `override` then `readonly` modifiers (TS order:
                        // accessibility → override → readonly). Both are contextual
                        // keywords and valid parameter names, so each is a modifier
                        // only when a binding follows it (see `eat_param_modifier_keyword`).
                        let is_override = self.eat_param_modifier_keyword("override")?;
                        let readonly = self.eat_param_modifier_keyword("readonly")?;

                        // If we have modifiers, this is a parameter property
                        if accessibility.is_some() || is_override || readonly {
                            let parameter = self.parse_simple_param()?;
                            let param_end = parameter.span().end;
                            Expression::TSParameterProperty(TSParameterProperty {
                                accessibility,
                                readonly,
                                r#override: is_override,
                                parameter: self.alloc(parameter),
                                span: Span::new(param_start as u32, param_end),
                            })
                        } else {
                            // Simple identifier parameter (no modifiers)
                            self.parse_simple_param()?
                        }
                    }
                    // Keywords that can be used as parameter names (contextual keywords like `from`, `async`)
                    // Note: `await`, `yield`, `let` are NOT allowed as parameter names
                    TokenKind::Keyword(kw) if kw.can_be_binding_name() => {
                        self.parse_simple_param()?
                    }
                    // TypeScript `this` parameter: `function f(this: T) {}`
                    TokenKind::Keyword(KeywordKind::This) => self.parse_simple_param()?,
                    // `await` as a parameter name — valid only at Script goal in a
                    // `[~Await]` context (`parse_simple_param` reads its name via
                    // `try_param_name`).
                    TokenKind::Keyword(KeywordKind::Await) if self.await_is_identifier() => {
                        self.parse_simple_param()?
                    }
                    TokenKind::BracketOpen | TokenKind::BraceOpen => {
                        // Destructuring pattern: [a, b] / {a, b}, with optional `?`
                        // marker, type annotation and default value
                        let pattern = self.parse_destructured_binding(true)?;

                        // Check for default value
                        if self.eat(TokenKind::Equals) {
                            let pattern_start = pattern.span().start;
                            let default_value = self.parse_assignment_expression_ref()?;
                            // prev_token_end covers a parenthesized default's closing `)`
                            let assign_end = self.prev_token_end() as u32;
                            Expression::AssignmentPattern(AssignmentPattern {
                                left: self.alloc(pattern),
                                right: default_value,
                                decorators: None,
                                span: Span::new(pattern_start, assign_end),
                            })
                        } else {
                            pattern
                        }
                    }
                    TokenKind::DotDotDot => {
                        // Rest parameter: ...args, ...args: type, or a
                        // destructuring pattern (...[a, b] / ...{ a }). Per
                        // `BindingRestElement : ... BindingPattern`, the rest
                        // argument may itself be an array/object pattern.
                        let rest_start = self.current_pos().0;
                        self.advance()?; // consume '...'

                        if matches!(
                            self.current_kind(),
                            TokenKind::BracketOpen | TokenKind::BraceOpen
                        ) {
                            // ...[a, b] / ...{ a } — the rest argument is itself a
                            // binding pattern. A trailing `: Type` annotates the
                            // *rest element* (`...[a, b]: T`), not the inner pattern,
                            // so parse the bare pattern and bind the type here
                            // (matching acorn; `parse_destructured_binding` would
                            // wrongly attach it to the pattern).
                            let pattern = self.parse_binding_pattern()?;
                            // A trailing `?`/`: T` binds to the rest element (acorn's
                            // shape), never to the inner pattern.
                            let (optional, type_annotation, arg_end) =
                                self.parse_rest_param_tail(pattern.span().end)?;
                            Expression::RestElement(RestElement {
                                argument: self.alloc(pattern),
                                optional,
                                type_annotation,
                                span: Span::new(rest_start as u32, arg_end),
                            })
                        } else {
                            // ...args, ...args?, ...args: type, or ...args?: type.
                            // The argument span stays name-only (`id_start..id_end`);
                            // any `?`/`: T` binds to the rest element (acorn's shape).
                            let (id_start, id_end) = self.current_pos();
                            let name = self.current_ident_name();
                            self.expect(&TokenKind::Identifier)?;
                            let argument = Expression::Identifier(Identifier::simple(
                                name,
                                Span::new(id_start as u32, id_end as u32),
                            ));
                            let (optional, type_annotation, arg_end) =
                                self.parse_rest_param_tail(id_end as u32)?;
                            Expression::RestElement(RestElement {
                                argument: self.alloc(argument),
                                optional,
                                type_annotation,
                                span: Span::new(rest_start as u32, arg_end),
                            })
                        }
                    }
                    _ => {
                        return Err(
                            self.error_expected_found("parameter name or destructuring pattern")
                        );
                    }
                };

                // Attach any parameter decorators to the binding node (matching acorn).
                let param = if decorators.is_empty() {
                    param
                } else {
                    self.attach_param_decorators(param, decorators)
                };

                let is_rest = matches!(&param, Expression::RestElement(_));
                params.push(param);

                // A rest parameter must be the last in the list. Per the grammar
                // (`FormalParameters : FormalParameterList `,` FunctionRestParameter`)
                // nothing — not even a trailing comma — may follow it. acorn:
                // "Comma is not permitted after the rest element". Exception:
                // in an ambient (`declare`) context acorn tolerates a single
                // trailing comma (`declare function f(...a: T[], )`), consumed
                // here; a following parameter or second comma still rejects
                // via the `)` expectation below.
                if is_rest {
                    if self.check(&TokenKind::Comma) {
                        if !self.in_ambient_context {
                            return Err(
                                self.error_msg("A rest parameter must be last in a parameter list")
                            );
                        }
                        self.advance()?;
                    }
                    break;
                }

                // Check for comma or closing paren
                if !self.expect_list_separator(&TokenKind::Comma, &TokenKind::ParenClose)? {
                    break;
                }
            }
        }

        self.expect(&TokenKind::ParenClose)?;
        Ok(params)
    }

    /// Enforce accessor parameter arity: a getter takes no parameters and a
    /// setter takes exactly one non-rest parameter. A faithful port of acorn's
    /// getter/setter param checks (JS grammar, mode-independent) — these are
    /// unconditional-local early-errors acorn rejects at parse, so tsv rejects
    /// too for drop-in parity.
    ///
    /// `allow_this_param` mirrors acorn's per-context `this`-parameter handling:
    /// an object-literal accessor excludes a leading `this` pseudo-parameter
    /// (`this: T`) from the count — `get x(this)` / `set x(this, v)` are valid —
    /// whereas a class-member accessor counts it (acorn's class path has no such
    /// exclusion), and a type-member accessor likewise counts it (so `this`
    /// pushes a getter to 1 / a setter to 2 and rejects on count).
    ///
    /// NOT enforced here: the TS-checker-only rules on the single setter
    /// parameter (optional `set x(a?)` = TS1051, default `set x(a = 1)` =
    /// TS1052), which acorn accepts in value positions and tsv defers to the
    /// diagnostics layer — an optional/default param is an `Identifier` /
    /// `AssignmentPattern`, not a `RestElement`, so it passes unchanged. (A
    /// *type-member* setter's optional-param rejection, which acorn does enforce
    /// at parse, lives in `check_type_member_accessor_params`.) Shared by the
    /// class-member, object-literal, and type-member accessor parsers.
    pub(super) fn check_accessor_param_arity(
        &self,
        is_getter: bool,
        params: &[Expression<'arena>],
        allow_this_param: bool,
    ) -> Result<(), ParseError> {
        let mut expected = if is_getter { 0 } else { 1 };
        if allow_this_param && params.first().is_some_and(|p| self.is_this_param(p)) {
            expected += 1;
        }
        if params.len() != expected {
            return Err(self.error_msg(if is_getter {
                "getter should have no params"
            } else {
                "setter should have exactly one param"
            }));
        }
        if !is_getter && matches!(params.first(), Some(Expression::RestElement(_))) {
            return Err(self.error_msg("Setter cannot use rest params"));
        }
        Ok(())
    }

    /// Validate a **type-member accessor** signature's parameters — the
    /// type-member-specific composite over `check_accessor_param_arity`, matching
    /// the get/set checks in acorn's `tsParsePropertyOrMethodSignature`. Beyond
    /// the shared arity rule (getter: no params; setter: exactly one non-rest
    /// param; a `this` param counts toward arity here, `allow_this_param = false`),
    /// a type-member **setter** also rejects — at parse, unlike a value-position
    /// setter — a `this` parameter (`AccesorCannotDeclareThisParameter`; the getter
    /// form is already caught by the 0-param arity check) and an optional parameter
    /// (`SetAccesorCannotHaveOptionalParameter`, TS1051, which tsv otherwise defers).
    pub(super) fn check_type_member_accessor_params(
        &self,
        kind: MethodKind,
        params: &[Expression<'arena>],
    ) -> Result<(), ParseError> {
        let is_getter = matches!(kind, MethodKind::Get);
        self.check_accessor_param_arity(is_getter, params, false)?;
        if !is_getter {
            if self.is_this_param(&params[0]) {
                return Err(
                    self.error_msg("'get' and 'set' accessors cannot declare 'this' parameters")
                );
            }
            if let Expression::Identifier(id) = &params[0]
                && id.optional
            {
                return Err(self.error_msg("A 'set' accessor cannot have an optional parameter"));
            }
        }
        Ok(())
    }

    /// Restrict a **type-member signature**'s parameters to plain bindings —
    /// `Identifier` / `RestElement` / `ObjectPattern` / `ArrayPattern` — a
    /// faithful port of acorn's `tsParseBindingListForSignature`. A signature has
    /// no implementation, so a default (`AssignmentPattern`, `set a(v = 1)`) or a
    /// parameter property (`TSParameterProperty`, `m(public v)`) is an
    /// unconditional-local grammar error acorn rejects at parse; tsv matches for
    /// drop-in parity. Only type-member signatures (interface / type-literal
    /// method, call, construct, accessor) call this — an *implementation*, an
    /// *ambient* declaration (`declare function f(v = 1)`, TS1039 deferred), and an
    /// *overload* keep their defaults (all acorn-accepts). Function *types* reject
    /// defaults already via their own param parser (`parse_function_type_params`).
    pub(super) fn check_signature_params(
        &self,
        params: &[Expression<'arena>],
    ) -> Result<(), ParseError> {
        for param in params {
            if !matches!(
                param,
                Expression::Identifier(_)
                    | Expression::RestElement(_)
                    | Expression::ObjectPattern(_)
                    | Expression::ArrayPattern(_)
            ) {
                return Err(self.error_msg(
                    "Name in a signature must be an Identifier, ObjectPattern or ArrayPattern",
                ));
            }
        }
        Ok(())
    }

    /// Whether a parameter is the TypeScript `this` pseudo-parameter (`this: T`)
    /// — a bare `this` identifier binding, matching acorn's `isThisParam`. Never
    /// escaped (it's the reserved word), so a raw source-slice compare on the
    /// name sub-span suffices. The node span is in **host** coordinates
    /// (`base_offset` added), so it shifts back before slicing `self.source` (the
    /// local, possibly Svelte-embedded slice) — same discipline as
    /// `resolve_cooked`; `.get` keeps a stray span from panicking.
    pub(super) fn is_this_param(&self, param: &Expression<'arena>) -> bool {
        let Expression::Identifier(id) = param else {
            return false;
        };
        let name = id.name_span();
        let start = (name.start as usize).saturating_sub(self.base_offset);
        let end = (name.end as usize).saturating_sub(self.base_offset);
        self.source.get(start..end) == Some("this")
    }
}
