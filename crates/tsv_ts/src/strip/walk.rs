//! The strip's one walk over the AST: it collects the type-only ranges (with their hazard
//! substitutions) and the refusals, and touches no source text itself.
//!
//! Every node kind is matched explicitly, with no catch-all arm, so a new statement,
//! expression, class-member or module-item kind is a compile error here until it has a
//! verdict. Types are never walked into: a type is blanked whole, through the span of the
//! annotation, argument list or declaration that holds it.
//!
//! **The `;` state.** A statement blanked whole after JS that does not end in `;` starts
//! with a `;`, or the next line could continue that JS. The state is ts-blank-space's
//! `semicolonNeeded`, reproduced exactly so the two agree byte for byte: it is set from the
//! last byte of every JS node the walk completes (`Walk::mark`), cleared on entering a
//! function body and restored on leaving it, and left alone by a blanked node. A class's
//! name is the one JS node not marked, as ts-blank-space never visits it.

use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use smallvec::SmallVec;

use crate::ast::internal::{
    ArrowFunctionBody, ArrowFunctionExpression, BinaryExpression, BinaryOperator, BlockStatement,
    ClassBody, ClassDeclaration, ClassExpression, ClassMember, Decorator, ExportDefaultDeclaration,
    ExportDefaultValue, ExportKind, ExportNamedDeclaration, Expression, ExpressionKind,
    ForInOfLeft, ForInit, FunctionDeclaration, FunctionExpression, Identifier, ImportDeclaration,
    ImportKind, ImportSpecifier, MethodDefinition, MethodKind, ObjectPatternProperty,
    ObjectProperty, Program, Property, PropertyDefinition, PropertyModifier, RestElement, Span,
    Statement, StatementKind, TSInterfaceHeritage, TSModuleDeclaration, TSModuleDeclarationBody,
    TSModuleDeclarationKind, TSModuleName, TSParameterProperty, TSType, TSTypeAnnotation,
    TSTypeParameterDeclaration, TSTypeParameterInstantiation, VariableDeclaration,
};

use super::blank::{Edit, Mark};
use super::scan::{Continuation, EXPONENT_PRECEDENCE, Source};
use super::{StripError, UnsupportedKind};

/// Where a statement stands: an element of a statement list, or the lone body of an `if` /
/// loop / label / `with`, where a statement blanked whole always becomes an empty statement
/// (`;`) so the statement after it cannot become the body.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Position {
    List,
    Nested,
}

/// The walk's state: the edits it has collected, the first refusal, and the state the
/// hazard substitutions read.
pub(super) struct Walk<'s, 'b> {
    src: Source<'s>,
    edits: BumpVec<'b, Edit>,
    /// The refusal with the earliest start — the one ts-blank-space reports first.
    refusal: Option<(UnsupportedKind, Span)>,
    /// The end of the last JS node marked (`Walk::mark`), `None` where nothing is marked yet (the
    /// start of the program, and of a function body): ts-blank-space's `semicolonNeeded` (module docs), read through
    /// `Walk::semicolon_needed`.
    marked_end: Option<u32>,
    /// The end of the innermost statement-list element (statement or class member) — where
    /// a statement-final `as` / `satisfies` would otherwise let the next line continue it.
    statement_end: Option<u32>,
    /// The end of the last statement-final assertion blanked with a leading `;`, which
    /// terminates the statement or field it ends.
    terminated_at: Option<u32>,
    /// How the last JS left standing in the current statement list or class body ends — a
    /// statement or a field initializer ending in an expression with no `;` after it is open
    /// to a following line that continues it. Narrower than `Walk::semicolon_needed`, which a `}`
    /// or a `)` also sets; it decides the `;` that ts-blank-space never writes
    /// (`blank_signature`, and a member key spelled like an operator), so each is written
    /// only where the program would otherwise change.
    open: Tail,
    /// The chain-link operands open around the node being walked ([`Walk::link_operand`]).
    chain_frames: u32,
}

/// The chain-link operands [`Walk::link_operand`] walks by recursion, a frame each, before
/// a chain is walked by [`Walk::chain`]'s loop: the common chain is a few links long and
/// its recursion is the cheaper walk, while a chain past them costs the stack of this many
/// link frames whatever its length.
const CHAIN_FRAMES: u32 = 16;

/// How the JS before a blanked range ends, for what a next line could continue.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tail {
    /// Nothing continues it: a `;`, a declaration's `}`, a bare `yield`, an arrow's block
    /// body, ASI's restricted productions.
    Closed,
    /// A postfix `++` / `--`: only a binary operator continues it.
    Operand,
    /// An expression any continuation extends: a call's `(`, `[`, a template, an operator.
    Open,
}

impl Tail {
    /// Whether a next line opening with `continuation` would continue this tail.
    fn continued_by(self, continuation: Option<Continuation>) -> bool {
        match (self, continuation) {
            (Self::Open, Some(_)) | (Self::Operand, Some(Continuation::Operator)) => true,
            (Self::Closed, _) | (_, None) | (Self::Operand, Some(Continuation::Postfix)) => false,
        }
    }
}

impl<'s, 'b> Walk<'s, 'b> {
    pub fn new(source: &'s str, arena: &'b Bump) -> Self {
        Self {
            src: Source::new(source),
            edits: BumpVec::new_in(arena),
            refusal: None,
            marked_end: None,
            statement_end: None,
            terminated_at: None,
            open: Tail::Closed,
            chain_frames: 0,
        }
    }

    /// The collected edits, or the refusal.
    pub fn finish(self) -> Result<BumpVec<'b, Edit>, StripError> {
        match self.refusal {
            Some((kind, span)) => Err(StripError::Unsupported { kind, span }),
            None => Ok(self.edits),
        }
    }

    pub fn program(&mut self, program: &Program<'_>) {
        self.statement_list(program.body, false);
    }

    //
    // Recording
    //

    fn blank(&mut self, start: u32, end: u32) {
        self.blank_marked(start, end, Mark::Plain);
    }

    fn blank_span(&mut self, span: Span) {
        self.blank(span.start, span.end);
    }

    fn blank_marked(&mut self, start: u32, end: u32, mark: Mark) {
        if start < end {
            self.edits.push(Edit { start, end, mark });
        }
    }

    /// Blank a statement or class member whole, `;`-led when `semicolon` — which closes what
    /// came before it.
    fn blank_whole(&mut self, span: Span, semicolon: bool) {
        if semicolon {
            self.open = Tail::Closed;
            self.blank_marked(span.start, span.end, Mark::Semicolon);
        } else {
            self.blank_marked(span.start, span.end, Mark::Plain);
        }
    }

    /// Blank a statement or class member whole, with a leading `;` where JS before it could
    /// otherwise run on into what follows (ts-blank-space's `blankStatement`).
    fn blank_statement(&mut self, span: Span, position: Position) {
        self.blank_whole(
            span,
            self.semicolon_needed() || position == Position::Nested,
        );
    }

    /// Blank a bodiless signature — an overload, an abstract method or accessor, an index
    /// signature. ts-blank-space blanks these with no `;` ever, so an expression left open
    /// before one (`Walk::open`) runs on into a following line that continues it (`(`, `[`,
    /// a template, a generic arrow's `<`, a binary operator, a generator's `*`, a member key
    /// `in` / `instanceof`); the `;` goes in exactly then.
    fn blank_signature(&mut self, span: Span, position: Position) {
        let hazard = self.open.continued_by(self.src.continuation(span.end));
        self.blank_whole(span, hazard || position == Position::Nested);
    }

    fn refuse(&mut self, kind: UnsupportedKind, span: Span) {
        if self
            .refusal
            .is_none_or(|(_, earlier)| span.start < earlier.start)
        {
            self.refusal = Some((kind, span));
        }
    }

    /// A JS node ending at `end` was walked: the `;` state follows its last byte. Only the end
    /// is stored, and the byte read where the state is asked (`Walk::semicolon_needed`): the
    /// source never changes, so the byte read then is the byte at marking, and most marks are
    /// overwritten unread.
    fn mark(&mut self, end: u32) {
        self.marked_end = Some(end);
    }

    /// ts-blank-space's `semicolonNeeded`: a JS node was marked, and its last byte is not `;`.
    fn semicolon_needed(&self) -> bool {
        self.marked_end
            .is_some_and(|end| !self.src.ends_with_semicolon(end))
    }

    //
    // Statements
    //

    fn statement_list(&mut self, list: &[Statement<'_>], function_body: bool) {
        let outer_marked = self.marked_end;
        if function_body {
            self.marked_end = None;
        }
        let outer_end = self.statement_end;
        let outer_open = std::mem::replace(&mut self.open, Tail::Closed);
        for statement in list {
            self.statement_end = Some(statement.span.end);
            self.statement(statement, statement.span, Position::List);
        }
        self.statement_end = outer_end;
        self.open = outer_open;
        if function_body {
            self.marked_end = outer_marked;
        }
    }

    fn nested_statement(&mut self, statement: &Statement<'_>) {
        self.statement(statement, statement.span, Position::Nested);
    }

    /// Walk `statement`, whose full extent is `outer` — its own span, or an `export`
    /// declaration's around it, which is what blanks or refuses when it is TypeScript.
    fn statement(&mut self, statement: &Statement<'_>, outer: Span, position: Position) {
        if self.statement_kind(statement, outer, position) {
            // a statement whose last byte is a `;` is closed, whatever its kind:
            // `statement_tail`'s every arm closes, reaches `unterminated_tail` at that same
            // last byte (which closes on it), or recurses into the declaration or body
            // statement the statement ends with — which ends at that byte too
            self.open = if self.src.ends_with_semicolon(statement.span.end) {
                Tail::Closed
            } else {
                self.statement_tail(statement)
            };
            self.mark(outer.end);
        }
    }

    /// The verdict on one statement; `true` when JS stays (anything not blanked whole).
    fn statement_kind(
        &mut self,
        statement: &Statement<'_>,
        outer: Span,
        position: Position,
    ) -> bool {
        match &statement.kind {
            StatementKind::ExpressionStatement(s) => {
                self.expr(s.expression);
                true
            }
            StatementKind::VariableDeclaration(decl) => {
                if decl.declare {
                    self.blank_statement(outer, position);
                    return false;
                }
                self.variable_declaration(decl);
                true
            }
            StatementKind::TSTypeAliasDeclaration(_) | StatementKind::TSInterfaceDeclaration(_) => {
                self.blank_statement(outer, position);
                false
            }
            StatementKind::TSDeclareFunction(func) => {
                if func.declare {
                    self.blank_statement(outer, position);
                } else {
                    // an overload signature
                    self.blank_signature(outer, position);
                }
                false
            }
            StatementKind::TSEnumDeclaration(decl) => {
                if decl.declare {
                    self.blank_statement(outer, position);
                    return false;
                }
                self.refuse(UnsupportedKind::Enum, outer);
                true
            }
            StatementKind::TSModuleDeclaration(decl) => {
                if module_is_type_only(decl) {
                    self.blank_statement(outer, position);
                    return false;
                }
                self.refuse(UnsupportedKind::ValueNamespace, outer);
                true
            }
            StatementKind::ReturnStatement(s) => {
                if let Some(argument) = s.argument {
                    self.expr(argument);
                }
                true
            }
            StatementKind::BlockStatement(block) => {
                self.statement_list(block.body, false);
                true
            }
            StatementKind::FunctionDeclaration(func) => {
                self.function(FunctionView::declaration(func));
                true
            }
            StatementKind::ClassDeclaration(class) => {
                if class.declare {
                    self.blank_statement(outer, position);
                    return false;
                }
                self.class(ClassView::declaration(class), outer.start);
                true
            }
            StatementKind::ExportNamedDeclaration(export) => {
                self.export_named(export, outer, position)
            }
            StatementKind::ExportDefaultDeclaration(export) => {
                self.export_default(export, outer, position)
            }
            StatementKind::ExportAllDeclaration(export) => {
                if export.export_kind == ExportKind::Type {
                    self.blank_statement(outer, position);
                    return false;
                }
                true
            }
            StatementKind::TSExportAssignment(_) => {
                self.refuse(UnsupportedKind::ExportEquals, outer);
                true
            }
            // `export as namespace N;` is kept verbatim, as both oracles keep it: it declares
            // a UMD global, which only a declaration file may (tsc's TS1315)
            StatementKind::TSNamespaceExportDeclaration(_) => true,
            StatementKind::ImportDeclaration(import) => self.import(import, outer, position),
            StatementKind::TSImportEqualsDeclaration(_) => {
                self.refuse(UnsupportedKind::ImportEquals, outer);
                true
            }
            StatementKind::IfStatement(s) => {
                self.expr(s.test);
                self.nested_statement(s.consequent);
                if let Some(alternate) = s.alternate {
                    self.nested_statement(alternate);
                }
                true
            }
            StatementKind::ForStatement(s) => {
                if let Some(init) = s.init {
                    match init {
                        ForInit::VariableDeclaration(decl) => self.variable_declaration(decl),
                        ForInit::Expression(e) => self.expr(e),
                    }
                }
                if let Some(test) = s.test {
                    self.expr(test);
                }
                if let Some(update) = s.update {
                    self.expr(update);
                }
                self.nested_statement(s.body);
                true
            }
            StatementKind::ForInStatement(s) => {
                self.for_left(s.left);
                self.expr(s.right);
                self.nested_statement(s.body);
                true
            }
            StatementKind::ForOfStatement(s) => {
                self.for_left(s.left);
                self.expr(s.right);
                self.nested_statement(s.body);
                true
            }
            StatementKind::WhileStatement(s) => {
                self.expr(s.test);
                self.nested_statement(s.body);
                true
            }
            StatementKind::DoWhileStatement(s) => {
                self.nested_statement(s.body);
                self.expr(s.test);
                true
            }
            StatementKind::WithStatement(s) => {
                self.expr(s.object);
                self.nested_statement(s.body);
                true
            }
            StatementKind::SwitchStatement(s) => {
                self.expr(s.discriminant);
                for case in s.cases {
                    if let Some(test) = case.test {
                        self.expr(test);
                    }
                    self.statement_list(case.consequent, false);
                    self.mark(case.span.end);
                }
                true
            }
            StatementKind::TryStatement(s) => {
                self.block(&s.block);
                if let Some(handler) = s.handler {
                    if let Some(param) = &handler.param {
                        self.expr(param);
                    }
                    self.block(&handler.body);
                    self.mark(handler.span.end);
                }
                if let Some(finalizer) = &s.finalizer {
                    self.block(finalizer);
                }
                true
            }
            StatementKind::ThrowStatement(s) => {
                self.expr(s.argument);
                true
            }
            StatementKind::BreakStatement(s) => {
                if let Some(label) = &s.label {
                    self.mark(label.span.end);
                }
                true
            }
            StatementKind::ContinueStatement(s) => {
                if let Some(label) = &s.label {
                    self.mark(label.span.end);
                }
                true
            }
            StatementKind::LabeledStatement(s) => {
                self.mark(s.label.span.end);
                self.nested_statement(s.body);
                true
            }
            StatementKind::EmptyStatement(_) | StatementKind::DebuggerStatement(_) => true,
        }
    }

    /// How a kept statement ends (`Walk::open`): in an expression with no `;` after it, or
    /// closed.
    fn statement_tail(&self, statement: &Statement<'_>) -> Tail {
        let end = statement.span.end;
        match &statement.kind {
            StatementKind::ExpressionStatement(s) => self.unterminated_tail(s.expression, end),
            StatementKind::ThrowStatement(s) => self.unterminated_tail(s.argument, end),
            StatementKind::ReturnStatement(s) => s.argument.map_or(Tail::Closed, |argument| {
                self.unterminated_tail(argument, end)
            }),
            StatementKind::VariableDeclaration(decl) => match decl.declarations.last() {
                Some(declarator) if !decl.declare => declarator
                    .init
                    .map_or(Tail::Closed, |init| self.unterminated_tail(init, end)),
                _ => Tail::Closed,
            },
            StatementKind::ExportNamedDeclaration(export) => export
                .declaration
                .map_or(Tail::Closed, |declaration| self.statement_tail(declaration)),
            StatementKind::ExportDefaultDeclaration(export) => match &export.declaration {
                ExportDefaultValue::Expression(e) => self.unterminated_tail(e, end),
                ExportDefaultValue::FunctionDeclaration(_)
                | ExportDefaultValue::TSDeclareFunction(_)
                | ExportDefaultValue::ClassDeclaration(_)
                | ExportDefaultValue::TSInterfaceDeclaration(_) => Tail::Closed,
            },
            StatementKind::IfStatement(s) => {
                self.statement_tail(s.alternate.unwrap_or(s.consequent))
            }
            StatementKind::ForStatement(s) => self.statement_tail(s.body),
            StatementKind::ForInStatement(s) => self.statement_tail(s.body),
            StatementKind::ForOfStatement(s) => self.statement_tail(s.body),
            StatementKind::WhileStatement(s) => self.statement_tail(s.body),
            StatementKind::WithStatement(s) => self.statement_tail(s.body),
            StatementKind::LabeledStatement(s) => self.statement_tail(s.body),
            StatementKind::TSTypeAliasDeclaration(_)
            | StatementKind::TSInterfaceDeclaration(_)
            | StatementKind::TSDeclareFunction(_)
            | StatementKind::TSEnumDeclaration(_)
            | StatementKind::TSModuleDeclaration(_)
            | StatementKind::BlockStatement(_)
            | StatementKind::FunctionDeclaration(_)
            | StatementKind::ClassDeclaration(_)
            | StatementKind::ExportAllDeclaration(_)
            | StatementKind::TSExportAssignment(_)
            | StatementKind::TSNamespaceExportDeclaration(_)
            | StatementKind::ImportDeclaration(_)
            | StatementKind::TSImportEqualsDeclaration(_)
            | StatementKind::DoWhileStatement(_)
            | StatementKind::SwitchStatement(_)
            | StatementKind::TryStatement(_)
            | StatementKind::BreakStatement(_)
            | StatementKind::ContinueStatement(_)
            | StatementKind::EmptyStatement(_)
            | StatementKind::DebuggerStatement(_) => Tail::Closed,
        }
    }

    /// The tail of expression `e` closing a statement or field that ends at `end`: closed by
    /// a `;` in the source or by a statement-final assertion's.
    fn unterminated_tail(&self, e: &Expression<'_>, end: u32) -> Tail {
        if self.src.ends_with_semicolon(end) || self.terminated_at == Some(end) {
            Tail::Closed
        } else {
            expression_tail(e, end)
        }
    }

    fn block(&mut self, block: &BlockStatement<'_>) {
        self.statement_list(block.body, false);
        self.mark(block.span.end);
    }

    fn variable_declaration(&mut self, decl: &VariableDeclaration<'_>) {
        for declarator in decl.declarations {
            self.expr(declarator.id);
            if declarator.definite {
                // `let a!: T` — the `!` follows the name, ahead of the annotation
                let end = declarator
                    .init
                    .map_or(declarator.span.end, |init| init.span.start);
                if let Some(bang) = self.src.find_char(declarator.id.span.start, end, b'!') {
                    self.blank(bang, bang + 1);
                }
            }
            if let Some(init) = declarator.init {
                self.expr(init);
            }
        }
    }

    fn for_left(&mut self, left: &ForInOfLeft<'_>) {
        match left {
            ForInOfLeft::VariableDeclaration(decl) => self.variable_declaration(decl),
            ForInOfLeft::Pattern(pattern) => self.expr(pattern),
        }
    }

    /// `export …`: an `export type` declaration blanks whole; an exported declaration takes
    /// the declaration's verdict over the whole statement, `export` included; inline `type`
    /// specifiers blank with their trailing commas.
    fn export_named(
        &mut self,
        export: &ExportNamedDeclaration<'_>,
        outer: Span,
        position: Position,
    ) -> bool {
        if export.export_kind == ExportKind::Type {
            self.blank_statement(outer, position);
            return false;
        }
        if let Some(declaration) = export.declaration {
            return self.statement_kind(declaration, outer, position);
        }
        for specifier in export.specifiers {
            if specifier.export_kind == ExportKind::Type {
                let end = self.src.end_with_trailing_comma(specifier.span.end);
                self.blank(specifier.span.start, end);
            }
        }
        true
    }

    fn export_default(
        &mut self,
        export: &ExportDefaultDeclaration<'_>,
        outer: Span,
        position: Position,
    ) -> bool {
        match &export.declaration {
            ExportDefaultValue::Expression(e) => {
                self.expr(e);
                true
            }
            ExportDefaultValue::FunctionDeclaration(func) => {
                self.function(FunctionView::declaration(func));
                true
            }
            ExportDefaultValue::TSDeclareFunction(func) => {
                if func.declare {
                    self.blank_statement(outer, position);
                } else {
                    self.blank_signature(outer, position);
                }
                false
            }
            ExportDefaultValue::ClassDeclaration(class) => {
                if class.declare {
                    self.blank_statement(outer, position);
                    return false;
                }
                self.class(ClassView::declaration(class), outer.start);
                true
            }
            ExportDefaultValue::TSInterfaceDeclaration(_) => {
                self.blank_statement(outer, position);
                false
            }
        }
    }

    /// `import …`: `import type` blanks whole; inline `type` specifiers blank with their
    /// trailing commas, and the declaration stays even when its braces end up empty — it
    /// still loads the module, as `verbatimModuleSyntax` and both oracles have it.
    fn import(&mut self, import: &ImportDeclaration<'_>, outer: Span, position: Position) -> bool {
        if import.import_kind == ImportKind::Type {
            self.blank_statement(outer, position);
            return false;
        }
        for specifier in import.specifiers {
            match specifier {
                ImportSpecifier::Named(named) => {
                    if named.import_kind == ImportKind::Type {
                        let end = self.src.end_with_trailing_comma(named.span.end);
                        self.blank(named.span.start, end);
                    }
                }
                ImportSpecifier::Default(_) | ImportSpecifier::Namespace(_) => {}
            }
        }
        true
    }

    //
    // Expressions
    //

    /// Walk expression `e`. A leaf holding no TypeScript — a literal, `this`, `super`, a private
    /// name, a meta property, or an identifier with no binding state (no `extra`, not
    /// `optional`: `Walk::identifier` would blank nothing) — only marks the `;` state, which is
    /// all the full walk ([`Self::expr_inner`]) does for it; it is answered here, inlined at
    /// each call site, without entering that walk's frame.
    #[inline]
    fn expr(&mut self, e: &Expression<'_>) {
        match &e.kind {
            ExpressionKind::Literal(_)
            | ExpressionKind::RegexLiteral(_)
            | ExpressionKind::ThisExpression(_)
            | ExpressionKind::Super(_)
            | ExpressionKind::PrivateIdentifier(_)
            | ExpressionKind::MetaProperty(_) => self.mark(e.span.end),
            ExpressionKind::Identifier(id) if id.extra.is_none() && !id.optional => {
                self.mark(e.span.end);
            }
            _ => self.expr_inner(e),
        }
    }

    /// A binary and the binaries down its left spine, walked without recursing down that
    /// spine — the chain's length is bounded only by `SPINE_LIMIT`, so a recursion per link
    /// would spend a frame per operator: the leftmost operand, then each right operand
    /// innermost first, the source order a recursion visits them in.
    #[inline(never)]
    fn binary_chain(&mut self, binary: &BinaryExpression<'_>) {
        let mut rights: SmallVec<[&Expression<'_>; 8]> = SmallVec::new();
        rights.push(binary.right);
        let mut left = binary.left;
        while let ExpressionKind::BinaryExpression(inner) = &left.kind {
            rights.push(inner.right);
            left = inner.left;
        }
        self.expr(left);
        for right in rights.iter().rev() {
            self.expr(right);
        }
    }

    /// The operand of a chain link — a member's object, a call's or a `new`'s callee, a tag,
    /// the operand of a `!`, an instantiation or an assertion — walked as [`Self::expr`]
    /// walks it, by recursion, while fewer than [`CHAIN_FRAMES`] link operands enclose
    /// it; past them, a chain is walked by [`Self::chain`]'s loop, so a chain's length
    /// (bounded only by `SPINE_LIMIT`) costs a bounded stack. Only the operand's walk is
    /// counted: a link's arguments are a level of nesting, which the nesting limit bounds.
    #[inline]
    fn link_operand(&mut self, operand: &Expression<'_>) {
        match &operand.kind {
            ExpressionKind::Literal(_)
            | ExpressionKind::RegexLiteral(_)
            | ExpressionKind::ThisExpression(_)
            | ExpressionKind::Super(_)
            | ExpressionKind::PrivateIdentifier(_)
            | ExpressionKind::MetaProperty(_) => self.mark(operand.span.end),
            ExpressionKind::Identifier(id) if id.extra.is_none() && !id.optional => {
                self.mark(operand.span.end);
            }
            _ => self.link_operand_inner(operand),
        }
    }

    /// [`Self::link_operand`] past the leaves.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    fn link_operand_inner(&mut self, operand: &Expression<'_>) {
        if self.chain_frames >= CHAIN_FRAMES && is_link(operand) {
            self.chain(operand);
        } else {
            self.chain_frames += 1;
            self.expr_inner(operand);
            self.chain_frames -= 1;
        }
    }

    /// A chain of links ([`is_link`]) and the base under them, walked without recursing
    /// down the chain. The links are collected on the way down, outermost first, each
    /// assertion's grouping check with them; the base is walked; then each link's work past
    /// its operand and its mark run innermost first — the order the recursion does each in
    /// ([`Self::expr_inner`]), so the edits, the refusal and the `;` state do not depend on
    /// which one walked a link.
    #[inline(never)]
    fn chain(&mut self, e: &Expression<'_>) {
        let mut links: SmallVec<[&Expression<'_>; 16]> = SmallVec::new();
        let mut node = e;
        let mut refused = false;
        loop {
            let operand = match &node.kind {
                ExpressionKind::CallExpression(call) => call.callee,
                ExpressionKind::NewExpression(new) => new.callee,
                ExpressionKind::MemberExpression(member) => member.object,
                ExpressionKind::TaggedTemplateExpression(tagged) => tagged.tag,
                ExpressionKind::TSInstantiationExpression(instantiation) => {
                    instantiation.expression
                }
                ExpressionKind::TSNonNullExpression(non_null) => non_null.expression,
                ExpressionKind::TSAsExpression(assertion) => {
                    refused = self
                        .assertion_head(
                            node.span,
                            assertion.expression,
                            assertion.type_annotation,
                            b"as",
                        )
                        .is_none();
                    assertion.expression
                }
                ExpressionKind::TSSatisfiesExpression(assertion) => {
                    refused = self
                        .assertion_head(
                            node.span,
                            assertion.expression,
                            assertion.type_annotation,
                            b"satisfies",
                        )
                        .is_none();
                    assertion.expression
                }
                _ => break,
            };
            links.push(node);
            // a refused assertion's operand is not walked: the chain ends at it
            if refused {
                break;
            }
            node = operand;
        }
        if !refused {
            self.expr(node);
        }
        for (i, link) in links.iter().rev().enumerate() {
            // a refused assertion is the innermost link: nothing of it is left to walk
            if !(refused && i == 0) {
                self.link_tail(link);
            }
            self.mark(link.span.end);
        }
    }

    /// A chain link's work past its operand ([`Self::chain`]), as [`Self::expr_inner`]
    /// does it.
    fn link_tail(&mut self, link: &Expression<'_>) {
        match &link.kind {
            ExpressionKind::CallExpression(call) => {
                self.type_arguments(call.type_arguments.as_ref());
                for argument in call.arguments {
                    self.expr(argument);
                }
            }
            ExpressionKind::NewExpression(new) => {
                self.type_arguments(new.type_arguments.as_ref());
                for argument in new.arguments {
                    self.expr(argument);
                }
            }
            ExpressionKind::MemberExpression(member) => self.expr(member.property),
            ExpressionKind::TaggedTemplateExpression(tagged) => {
                self.type_arguments(tagged.type_arguments.as_ref());
                for expression in tagged.quasi.expressions {
                    self.expr(expression);
                }
            }
            ExpressionKind::TSInstantiationExpression(instantiation) => {
                self.blank_span(instantiation.type_arguments.span);
            }
            // the `!` is the node's last byte, past any paren around the operand
            ExpressionKind::TSNonNullExpression(_) => {
                self.blank(link.span.end - 1, link.span.end);
            }
            ExpressionKind::TSAsExpression(assertion) => {
                let blank_from = self.assertion_blank_start(
                    assertion.expression,
                    assertion.type_annotation,
                    b"as",
                );
                self.assertion_tail(link.span, blank_from);
            }
            ExpressionKind::TSSatisfiesExpression(assertion) => {
                let blank_from = self.assertion_blank_start(
                    assertion.expression,
                    assertion.type_annotation,
                    b"satisfies",
                );
                self.assertion_tail(link.span, blank_from);
            }
            // [`Self::chain`] collects no other kind ([`is_link`])
            _ => {}
        }
    }

    /// The full walk of expression `e`, every kind matched; [`Self::expr`] answers the leaves
    /// before reaching it.
    #[inline(never)]
    fn expr_inner(&mut self, e: &Expression<'_>) {
        match &e.kind {
            ExpressionKind::Literal(_)
            | ExpressionKind::RegexLiteral(_)
            | ExpressionKind::ThisExpression(_)
            | ExpressionKind::Super(_)
            | ExpressionKind::PrivateIdentifier(_)
            | ExpressionKind::MetaProperty(_) => {}
            ExpressionKind::Identifier(id) => self.identifier(id),
            ExpressionKind::ObjectExpression(object) => {
                for property in object.properties {
                    match property {
                        ObjectProperty::Property(property) => self.property(property),
                        ObjectProperty::SpreadElement(spread) => self.expr(spread.argument),
                    }
                }
            }
            ExpressionKind::ArrayExpression(array) => {
                for element in array.elements.iter().flatten() {
                    self.expr(element);
                }
            }
            ExpressionKind::UnaryExpression(unary) => self.expr(unary.argument),
            ExpressionKind::UpdateExpression(update) => self.expr(update.argument),
            ExpressionKind::BinaryExpression(binary) => self.binary_chain(binary),
            ExpressionKind::CallExpression(call) => {
                self.link_operand(call.callee);
                self.type_arguments(call.type_arguments.as_ref());
                for argument in call.arguments {
                    self.expr(argument);
                }
            }
            ExpressionKind::NewExpression(new) => {
                self.link_operand(new.callee);
                self.type_arguments(new.type_arguments.as_ref());
                for argument in new.arguments {
                    self.expr(argument);
                }
            }
            ExpressionKind::MemberExpression(member) => {
                self.link_operand(member.object);
                self.expr(member.property);
            }
            ExpressionKind::ConditionalExpression(conditional) => {
                self.expr(conditional.test);
                self.expr(conditional.consequent);
                self.expr(conditional.alternate);
            }
            ExpressionKind::ArrowFunctionExpression(arrow) => {
                self.function(FunctionView::arrow(arrow));
            }
            ExpressionKind::FunctionExpression(func) => {
                self.function(FunctionView::expression(func));
            }
            ExpressionKind::ClassExpression(class) => {
                self.class(ClassView::expression(class), e.span.start);
            }
            ExpressionKind::SpreadElement(spread) => self.expr(spread.argument),
            ExpressionKind::TemplateLiteral(template) => {
                for expression in template.expressions {
                    self.expr(expression);
                }
            }
            ExpressionKind::TaggedTemplateExpression(tagged) => {
                self.link_operand(tagged.tag);
                self.type_arguments(tagged.type_arguments.as_ref());
                for expression in tagged.quasi.expressions {
                    self.expr(expression);
                }
            }
            ExpressionKind::AwaitExpression(e) => self.expr(e.argument),
            ExpressionKind::YieldExpression(e) => {
                if let Some(argument) = e.argument {
                    self.expr(argument);
                }
            }
            ExpressionKind::SequenceExpression(sequence) => {
                for expression in sequence.expressions {
                    self.expr(expression);
                }
            }
            ExpressionKind::AssignmentExpression(assignment) => {
                self.expr(assignment.left);
                self.expr(assignment.right);
            }
            ExpressionKind::ObjectPattern(pattern) => {
                self.decorators(pattern.decorators);
                for property in pattern.properties {
                    match property {
                        ObjectPatternProperty::Property(property) => self.property(property),
                        ObjectPatternProperty::RestElement(rest) => self.rest_element(rest),
                    }
                }
                self.pattern_tail(e.span, pattern.optional, pattern.type_annotation.as_ref());
            }
            ExpressionKind::ArrayPattern(pattern) => {
                self.decorators(pattern.decorators);
                for element in pattern.elements.iter().flatten() {
                    self.expr(element);
                }
                self.pattern_tail(e.span, pattern.optional, pattern.type_annotation.as_ref());
            }
            ExpressionKind::AssignmentPattern(pattern) => {
                self.decorators(pattern.decorators);
                self.expr(pattern.left);
                self.expr(pattern.right);
            }
            ExpressionKind::RestElement(rest) => self.rest_element(rest),
            ExpressionKind::TSTypeAssertion(assertion) => {
                self.refuse(UnsupportedKind::TypeAssertion, e.span);
                self.expr(assertion.expression);
            }
            ExpressionKind::TSAsExpression(assertion) => {
                self.assertion(
                    e.span,
                    assertion.expression,
                    assertion.type_annotation,
                    b"as",
                );
            }
            ExpressionKind::TSSatisfiesExpression(assertion) => {
                self.assertion(
                    e.span,
                    assertion.expression,
                    assertion.type_annotation,
                    b"satisfies",
                );
            }
            ExpressionKind::TSInstantiationExpression(instantiation) => {
                self.link_operand(instantiation.expression);
                self.blank_span(instantiation.type_arguments.span);
            }
            ExpressionKind::TSNonNullExpression(non_null) => {
                self.link_operand(non_null.expression);
                // the `!` is the node's last byte, past any paren around the operand
                self.blank(e.span.end - 1, e.span.end);
            }
            ExpressionKind::TSParameterProperty(property) => {
                self.parameter_property(e.span, property);
                self.expr(property.parameter);
            }
            ExpressionKind::ImportExpression(import) => {
                self.expr(import.source);
                if let Some(options) = import.options {
                    self.expr(options);
                }
            }
            ExpressionKind::CommentBoundPair(cast) => self.expr(cast.inner),
            ExpressionKind::ParenthesizedExpression(parens) => {
                self.expr(parens.unreachable_contents());
            }
        }
        self.mark(e.span.end);
    }

    /// A binding identifier's TypeScript: parameter decorators stay (walked for the type
    /// arguments in them), the optional `?` and the annotation blank. A reference carries
    /// none of these.
    fn identifier(&mut self, id: &Identifier<'_>) {
        let decorators = id.decorators();
        self.decorators(decorators);
        let annotation = id.type_annotation();
        if id.optional {
            let from = decorators
                .and_then(<[Decorator<'_>]>::last)
                .map_or(id.span.start, |last| last.span.end);
            let end = annotation.map_or(id.span.end, |a| a.span.start);
            if let Some(question) = self.src.find_char(from, end, b'?') {
                self.blank(question, question + 1);
            }
        }
        if let Some(annotation) = annotation {
            self.blank_span(annotation.span);
        }
    }

    /// An object-literal or object-pattern property: a computed key and the value.
    fn property(&mut self, property: &Property<'_>) {
        if property.computed {
            self.expr(property.key);
        }
        self.expr(property.value);
    }

    fn rest_element(&mut self, rest: &RestElement<'_>) {
        self.expr(rest.argument);
        self.pattern_tail(rest.span, rest.optional, rest.type_annotation.as_ref());
    }

    /// A destructuring pattern's optional `?` (the last `?` ahead of its annotation, past
    /// the pattern's closing bracket) and annotation.
    fn pattern_tail(
        &mut self,
        span: Span,
        optional: bool,
        annotation: Option<&TSTypeAnnotation<'_>>,
    ) {
        if optional {
            let end = annotation.map_or(span.end, |a| a.span.start);
            if let Some(question) = self.src.rfind_char(span.start, end, b'?') {
                self.blank(question, question + 1);
            }
        }
        if let Some(annotation) = annotation {
            self.blank_span(annotation.span);
        }
    }

    fn decorators(&mut self, decorators: Option<&[Decorator<'_>]>) {
        for decorator in decorators.unwrap_or_default() {
            self.expr(&decorator.expression);
        }
    }

    fn type_arguments(&mut self, arguments: Option<&TSTypeParameterInstantiation<'_>>) {
        if let Some(arguments) = arguments {
            self.blank_span(arguments.span);
        }
    }

    /// `x as T` / `x satisfies T`: the keyword and the type blank, from the end of the
    /// operand (past any paren around it). Where the assertion ends its statement and no `;`
    /// follows, the blank starts with a `;`. Refused where blanking would regroup the
    /// expression around it.
    fn assertion(&mut self, span: Span, operand: &Expression<'_>, ty: &TSType<'_>, keyword: &[u8]) {
        if let Some(blank_from) = self.assertion_head(span, operand, ty, keyword) {
            self.link_operand(operand);
            self.assertion_tail(span, blank_from);
        }
    }

    /// An assertion ahead of its operand's walk: where its blank starts, or `None` —
    /// refused — where blanking would regroup the expression around it.
    fn assertion_head(
        &mut self,
        span: Span,
        operand: &Expression<'_>,
        ty: &TSType<'_>,
        keyword: &[u8],
    ) -> Option<u32> {
        let blank_from = self.assertion_blank_start(operand, ty, keyword);
        if self.assertion_regroups(span, operand, blank_from != operand.span.end) {
            self.refuse(UnsupportedKind::AssertionGrouping, span);
            return None;
        }
        Some(blank_from)
    }

    /// An assertion past its walked operand ([`Self::assertion`]): the blank from
    /// `blank_from`, `;`-led where the assertion ends its statement.
    fn assertion_tail(&mut self, span: Span, blank_from: u32) {
        let mark = if self.statement_end == Some(span.end)
            && self.src.bytes.get(span.end as usize) != Some(&b';')
        {
            self.terminated_at = Some(span.end);
            Mark::Semicolon
        } else {
            Mark::Plain
        };
        self.blank_marked(blank_from, span.end, mark);
    }

    /// Where an assertion's blank starts: past the last `)` between its operand and its
    /// keyword (a paren the parser stripped from the operand), else at the operand's end.
    fn assertion_blank_start(
        &self,
        operand: &Expression<'_>,
        ty: &TSType<'_>,
        keyword: &[u8],
    ) -> u32 {
        let operand_end = operand.span.end;
        let keyword_at = self
            .src
            .find_keyword(operand_end, ty.span().start, keyword)
            .unwrap_or(operand_end);
        self.src
            .rfind_char(operand_end, keyword_at, b')')
            .map_or(operand_end, |paren| paren + 1)
    }

    /// Whether blanking the assertion at `span` would change how the expression around it
    /// groups (ts-blank-space's `assertionChainWouldChangeBinaryGrouping`, plus the unary
    /// operand of `**`, where it is wrong): the assertion's operand — through any chain of
    /// assertions, and unless parenthesized — is a binary expression whose operator binds
    /// looser than the operator after the assertion (or as tight, with `**` on either
    /// side), or a unary / `await` expression followed by `**`.
    fn assertion_regroups(
        &self,
        span: Span,
        operand: &Expression<'_>,
        operand_wrapped: bool,
    ) -> bool {
        if operand_wrapped {
            return false;
        }
        let mut base = operand;
        loop {
            let (inner, ty, keyword): (&Expression<'_>, &TSType<'_>, &[u8]) = match &base.kind {
                ExpressionKind::TSAsExpression(a) => (a.expression, a.type_annotation, b"as"),
                ExpressionKind::TSSatisfiesExpression(s) => {
                    (s.expression, s.type_annotation, b"satisfies")
                }
                _ => break,
            };
            if self.assertion_blank_start(inner, ty, keyword) != inner.span.end {
                return false;
            }
            base = inner;
        }
        let next = self.src.binary_operator_precedence(span.end);
        match &base.kind {
            ExpressionKind::BinaryExpression(binary) => {
                let Some(next) = next else {
                    return false;
                };
                let own = precedence(binary.operator);
                next > own
                    || (next == own && (own == EXPONENT_PRECEDENCE || next == EXPONENT_PRECEDENCE))
            }
            ExpressionKind::UnaryExpression(_) | ExpressionKind::AwaitExpression(_) => {
                next == Some(EXPONENT_PRECEDENCE)
            }
            _ => false,
        }
    }

    /// `constructor(public x)` and its kin: refused, the span the first modifier.
    fn parameter_property(&mut self, span: Span, property: &TSParameterProperty<'_>) {
        let end = property.parameter.span.start;
        let keywords = [
            property.accessibility.map(|a| a.as_str().as_bytes()),
            property.readonly.then_some(b"readonly".as_slice()),
            property.r#override.then_some(b"override".as_slice()),
        ];
        let first = keywords
            .into_iter()
            .flatten()
            .filter_map(|keyword| {
                let at = self.src.find_keyword(span.start, end, keyword)?;
                Some(Span::new(at, at + keyword.len() as u32))
            })
            .min_by_key(|found| found.start)
            .unwrap_or_else(|| Span::new(span.start, end));
        self.refuse(UnsupportedKind::ParameterProperty, first);
    }

    //
    // Functions and classes
    //

    fn function(&mut self, func: FunctionView<'_, '_>) {
        if let Some(type_parameters) = func.type_parameters {
            let span = type_parameters.span;
            // a line break between the `<` and the parameters' `(` would let ASI end a
            // `return` (or strand an `async`): the `(` moves to the `<`
            match func.params_open {
                Some(open) if self.src.line_break_between(span.start + 1, open + 1) => {
                    self.blank_marked(span.start, span.end, Mark::OpenParen);
                    self.blank(open, open + 1);
                }
                _ => self.blank_span(span),
            }
        }
        for (index, param) in func.params.iter().enumerate() {
            if index == 0 && self.is_this_parameter(param) {
                let end = self.src.end_with_trailing_comma(param.span.end);
                self.blank(param.span.start, end);
                continue;
            }
            self.expr(param);
        }
        if let Some(return_type) = func.return_type {
            let params_end = func.params.last().map_or_else(
                || {
                    func.params_open
                        .map_or(return_type.span.start, |open| open + 1)
                },
                |last| last.span.end,
            );
            // a line break before an arrow's `=>` is a syntax error: the `)` moves to the
            // end of the blanked return type
            let close = (func.arrow
                && self
                    .src
                    .line_break_between(params_end, return_type.span.end))
            .then(|| self.src.find_char(params_end, return_type.span.start, b')'))
            .flatten();
            match close {
                Some(close) => self.blank_marked(close, return_type.span.end, Mark::CloseParen),
                None => self.blank_span(return_type.span),
            }
        }
        match func.body {
            FunctionBody::Block(block) => self.statement_list(block.body, true),
            FunctionBody::Expression(body) => self.expr(body),
        }
    }

    fn is_this_parameter(&self, param: &Expression<'_>) -> bool {
        matches!(&param.kind, ExpressionKind::Identifier(id) if id.name(self.src.text) == "this")
    }

    /// A class: decorators stay, `abstract`, type parameters, superclass type arguments
    /// and `implements` blank, and the members take their own verdicts. `outer_start` is
    /// where the class's modifiers can start (an `export` included).
    fn class(&mut self, class: ClassView<'_, '_>, outer_start: u32) {
        self.decorators(class.decorators);
        let modifiers_from = class
            .decorators
            .and_then(<[Decorator<'_>]>::last)
            .map_or(outer_start, |last| last.span.end.max(outer_start));
        if class.r#abstract
            && let Some(at) =
                self.src
                    .find_keyword(modifiers_from, class.body.span.start, b"abstract")
        {
            self.blank(at, at + b"abstract".len() as u32);
        }
        if let Some(type_parameters) = class.type_parameters {
            self.blank_span(type_parameters.span);
        }
        if let Some(super_class) = class.super_class {
            self.expr(super_class);
        }
        self.type_arguments(class.super_type_parameters);
        if let (Some(first), Some(last)) = (class.implements.first(), class.implements.last())
            && let Some(at) =
                self.src
                    .rfind_keyword(modifiers_from, first.span.start, b"implements")
        {
            self.blank(at, last.span.end);
        }
        self.class_members(class.body);
    }

    fn class_members(&mut self, body: &ClassBody<'_>) {
        let outer_end = self.statement_end;
        // open: the last member left standing is a field whose initializer has no `;`,
        // which a following `[`, `*`, `in` or `instanceof` would continue
        let outer_open = std::mem::replace(&mut self.open, Tail::Closed);
        for member in body.body {
            let span = member.span();
            self.statement_end = Some(span.end);
            if self.class_member(member) {
                self.mark(span.end);
                self.open = match member {
                    ClassMember::PropertyDefinition(PropertyDefinition {
                        value: Some(value),
                        ..
                    }) => self.unterminated_tail(value, span.end),
                    _ => Tail::Closed,
                };
            }
        }
        self.statement_end = outer_end;
        self.open = outer_open;
    }

    /// The verdict on one member; `true` when JS stays.
    fn class_member(&mut self, member: &ClassMember<'_>) -> bool {
        // read before the member's own walk, which can reach a nested class body
        let open_field = self.open != Tail::Closed;
        match member {
            ClassMember::PropertyDefinition(property) => {
                self.property_definition(property, open_field)
            }
            ClassMember::MethodDefinition(method) => self.method_definition(method, open_field),
            ClassMember::StaticBlock(block) => {
                // a decorated block's decorators stay, as any member's do (their type
                // arguments blank)
                self.decorators(block.decorators);
                self.statement_list(block.body, false);
                true
            }
            ClassMember::IndexSignature(signature) => {
                self.blank_signature(signature.span, Position::List);
                false
            }
        }
    }

    fn property_definition(&mut self, property: &PropertyDefinition<'_>, open_field: bool) -> bool {
        // `declare x: T` and `abstract x: T` declare no runtime field
        if property.declare || property.r#abstract {
            self.blank_statement(property.span, Position::List);
            return false;
        }
        self.decorators(property.decorators);
        let kept = [
            property.is_static.then_some(b"static".as_slice()),
            property.accessor.then_some(b"accessor".as_slice()),
        ];
        // a key named `in` or `instanceof` would continue an open field's initializer as the
        // operator once a modifier ahead of it blanks (`a = x⏎private in⏎y` reads `x in y`)
        let operator_key = open_field
            && kept.iter().all(Option::is_none)
            && self.is_operator_key(&property.key, property.computed);
        self.member_modifiers(MemberModifiers {
            from: decorators_end(property.decorators, property.span.start),
            to: property.key.span.start,
            decorated: property.decorators.is_some(),
            semicolon: property.computed || operator_key,
            removed: [
                property.accessibility.map(|a| a.as_str().as_bytes()),
                property.readonly.then_some(b"readonly".as_slice()),
                property.r#override.then_some(b"override".as_slice()),
            ],
            kept,
        });
        if property.computed {
            self.expr(&property.key);
        }
        let annotation = property.type_annotation.as_ref();
        let mark = match property.modifier {
            PropertyModifier::None => None,
            PropertyModifier::Optional => Some(b'?'),
            PropertyModifier::Definite => Some(b'!'),
        };
        if let Some(mark) = mark {
            let end = annotation
                .map(|a| a.span.start)
                .or_else(|| property.value.as_ref().map(|v| v.span.start))
                .unwrap_or(property.span.end);
            if let Some(at) = self.src.find_char(property.key.span.end, end, mark) {
                self.blank(at, at + 1);
            }
        }
        if let Some(annotation) = annotation {
            self.blank_span(annotation.span);
        }
        if let Some(value) = &property.value {
            self.expr(value);
        }
        true
    }

    fn method_definition(&mut self, method: &MethodDefinition<'_>, open_field: bool) -> bool {
        let func = &method.value;
        // an overload signature or an abstract method or accessor: no body
        if func.body.body.is_empty() && func.body.span.start == func.body.span.end {
            self.blank_signature(method.span, Position::List);
            return false;
        }
        self.decorators(method.decorators);
        let kept = [
            method.is_static.then_some(b"static".as_slice()),
            func.r#async.then_some(b"async".as_slice()),
        ];
        // a generator's `*` would multiply an open field's initializer once a modifier
        // ahead of it blanks (`a = 1⏎public *g() {}`), as a computed key's `[` would index it
        let generator = func.generator && open_field && kept.iter().all(Option::is_none);
        // so would a key named `in` or `instanceof`, read as the operator (`a = x⏎public in()`)
        let operator_key = open_field
            && method.kind == MethodKind::Method
            && !func.generator
            && kept.iter().all(Option::is_none)
            && self.is_operator_key(&method.key, method.computed);
        self.member_modifiers(MemberModifiers {
            from: decorators_end(method.decorators, method.span.start),
            to: method.key.span.start,
            decorated: method.decorators.is_some(),
            semicolon: method.computed || generator || operator_key,
            removed: [
                method.accessibility.map(|a| a.as_str().as_bytes()),
                method.r#override.then_some(b"override".as_slice()),
                None,
            ],
            kept,
        });
        if method.computed {
            self.expr(&method.key);
        }
        if method.optional {
            let end = func
                .type_parameters
                .as_ref()
                .map_or(func.params_start, |tp| tp.span.start);
            if let Some(question) = self.src.find_char(method.key.span.end, end, b'?') {
                self.blank(question, question + 1);
            }
        }
        self.function(FunctionView::expression(func));
        true
    }

    /// Whether a member key is an identifier spelled like a binary operator (`in`,
    /// `instanceof`): once a modifier ahead of it blanks, an open field's initializer reads
    /// the key as that operator and the next member as its right operand.
    fn is_operator_key(&self, key: &Expression<'_>, computed: bool) -> bool {
        !computed
            && matches!(&key.kind, ExpressionKind::Identifier(id)
                if matches!(id.name(self.src.text), "in" | "instanceof"))
    }

    /// Blank a kept member's TypeScript modifiers keyword by keyword (`static` and
    /// `accessor` stay). With `semicolon`, the first modifier — when it is one that blanks
    /// and no decorator precedes it — starts with a `;` (ts-blank-space's `addSemi`).
    fn member_modifiers(&mut self, modifiers: MemberModifiers) {
        let MemberModifiers {
            from,
            to,
            decorated,
            semicolon,
            removed,
            kept,
        } = modifiers;
        let first_kept = kept
            .into_iter()
            .flatten()
            .filter_map(|keyword| self.src.find_keyword(from, to, keyword))
            .min()
            .unwrap_or(u32::MAX);
        let found = removed.map(|keyword| {
            let keyword = keyword?;
            let at = self.src.find_keyword(from, to, keyword)?;
            Some(Span::new(at, at + keyword.len() as u32))
        });
        let first_removed = found.iter().flatten().map(|s| s.start).min();
        for span in found.into_iter().flatten() {
            let mark = if semicolon
                && !decorated
                && Some(span.start) == first_removed
                && span.start < first_kept
            {
                Mark::Semicolon
            } else {
                Mark::Plain
            };
            self.blank_marked(span.start, span.end, mark);
        }
    }
}

/// The modifier keywords of one kept class member.
struct MemberModifiers {
    /// Where the modifiers can start (past any decorator).
    from: u32,
    /// The member's key.
    to: u32,
    decorated: bool,
    /// Whether the first modifier, when it blanks, starts with a `;`.
    semicolon: bool,
    /// The keywords that blank, those present.
    removed: [Option<&'static [u8]>; 3],
    /// The keywords that stay, those present.
    kept: [Option<&'static [u8]>; 2],
}

/// Whether `e` is a chain link: a member access, a call, a `new`, a tagged template, a `!`,
/// an instantiation or an assertion, whose operand ([`Walk::link_operand`]) runs down the
/// chain.
fn is_link(e: &Expression<'_>) -> bool {
    matches!(
        e.kind,
        ExpressionKind::CallExpression(_)
            | ExpressionKind::NewExpression(_)
            | ExpressionKind::MemberExpression(_)
            | ExpressionKind::TaggedTemplateExpression(_)
            | ExpressionKind::TSAsExpression(_)
            | ExpressionKind::TSSatisfiesExpression(_)
            | ExpressionKind::TSInstantiationExpression(_)
            | ExpressionKind::TSNonNullExpression(_)
    )
}

fn decorators_end(decorators: Option<&[Decorator<'_>]>, start: u32) -> u32 {
    decorators
        .and_then(<[Decorator<'_>]>::last)
        .map_or(start, |last| last.span.end)
}

/// The parts of a function the walk reads, from any of its node shapes.
struct FunctionView<'n, 'a> {
    type_parameters: Option<&'n TSTypeParameterDeclaration<'a>>,
    params: &'n [Expression<'a>],
    /// The parameter list's `(`; `None` for a bare arrow parameter (`x => x`).
    params_open: Option<u32>,
    return_type: Option<&'n TSTypeAnnotation<'a>>,
    body: FunctionBody<'n, 'a>,
    arrow: bool,
}

enum FunctionBody<'n, 'a> {
    Block(&'n BlockStatement<'a>),
    Expression(&'n Expression<'a>),
}

impl<'n, 'a> FunctionView<'n, 'a> {
    fn declaration(func: &'n FunctionDeclaration<'a>) -> Self {
        Self {
            type_parameters: func.type_parameters.as_ref(),
            params: func.params,
            params_open: Some(func.params_start),
            return_type: func.return_type.as_ref(),
            body: FunctionBody::Block(&func.body),
            arrow: false,
        }
    }

    fn expression(func: &'n FunctionExpression<'a>) -> Self {
        Self {
            type_parameters: func.type_parameters.as_ref(),
            params: func.params,
            params_open: Some(func.params_start),
            return_type: func.return_type.as_ref(),
            body: FunctionBody::Block(&func.body),
            arrow: false,
        }
    }

    fn arrow(arrow: &'n ArrowFunctionExpression<'a>) -> Self {
        Self {
            type_parameters: arrow.type_parameters.as_ref(),
            params: arrow.params,
            params_open: arrow.params_start,
            return_type: arrow.return_type.as_ref(),
            body: match &arrow.body {
                ArrowFunctionBody::Expression(body) => FunctionBody::Expression(body),
                ArrowFunctionBody::BlockStatement(block) => FunctionBody::Block(block),
            },
            arrow: true,
        }
    }
}

/// The parts of a class the walk reads, from a declaration or an expression.
struct ClassView<'n, 'a> {
    decorators: Option<&'a [Decorator<'a>]>,
    r#abstract: bool,
    type_parameters: Option<&'n TSTypeParameterDeclaration<'a>>,
    super_class: Option<&'a Expression<'a>>,
    super_type_parameters: Option<&'n TSTypeParameterInstantiation<'a>>,
    implements: &'a [TSInterfaceHeritage<'a>],
    body: &'n ClassBody<'a>,
}

impl<'n, 'a> ClassView<'n, 'a> {
    fn declaration(class: &'n ClassDeclaration<'a>) -> Self {
        Self {
            decorators: class.decorators,
            r#abstract: class.r#abstract,
            type_parameters: class.type_parameters.as_ref(),
            super_class: class.super_class,
            super_type_parameters: class.super_type_parameters.as_ref(),
            implements: class.implements,
            body: &class.body,
        }
    }

    fn expression(class: &'n ClassExpression<'a>) -> Self {
        Self {
            decorators: class.decorators,
            r#abstract: class.r#abstract,
            type_parameters: class.type_parameters.as_ref(),
            super_class: class.super_class,
            super_type_parameters: class.super_type_parameters.as_ref(),
            implements: class.implements,
            body: &class.body,
        }
    }
}

/// How expression `e` ends when it closes a statement or field ending at `end`
/// (`Walk::open`). Its rightmost operand decides — unless a paren the parser stripped closes
/// it (its span ends short of `end`), which any continuation extends.
fn expression_tail(e: &Expression<'_>, end: u32) -> Tail {
    if e.span.end != end {
        return Tail::Open;
    }
    match &e.kind {
        ExpressionKind::UpdateExpression(update) if !update.prefix => Tail::Operand,
        ExpressionKind::UpdateExpression(update) => expression_tail(update.argument, end),
        ExpressionKind::YieldExpression(e) => e
            .argument
            .map_or(Tail::Closed, |argument| expression_tail(argument, end)),
        ExpressionKind::ArrowFunctionExpression(arrow) => match &arrow.body {
            ArrowFunctionBody::Expression(body) => expression_tail(body, end),
            ArrowFunctionBody::BlockStatement(_) => Tail::Closed,
        },
        ExpressionKind::BinaryExpression(binary) => expression_tail(binary.right, end),
        ExpressionKind::AssignmentExpression(assignment) => expression_tail(assignment.right, end),
        ExpressionKind::ConditionalExpression(conditional) => {
            expression_tail(conditional.alternate, end)
        }
        ExpressionKind::SequenceExpression(sequence) => sequence
            .expressions
            .last()
            .map_or(Tail::Open, |last| expression_tail(last, end)),
        ExpressionKind::UnaryExpression(unary) => expression_tail(unary.argument, end),
        ExpressionKind::AwaitExpression(e) => expression_tail(e.argument, end),
        // every other expression ends in a token a next line extends — the safe default,
        // which at worst writes a `;` the program does not need
        _ => Tail::Open,
    }
}

/// A binary operator's precedence on ts-blank-space's scale (`**` 15 down to `??` 4).
const fn precedence(operator: BinaryOperator) -> u8 {
    match operator {
        BinaryOperator::StarStar => EXPONENT_PRECEDENCE,
        BinaryOperator::Star | BinaryOperator::Slash | BinaryOperator::Percent => 14,
        BinaryOperator::Plus | BinaryOperator::Minus => 13,
        BinaryOperator::LeftShift
        | BinaryOperator::RightShift
        | BinaryOperator::UnsignedRightShift => 12,
        BinaryOperator::LessThan
        | BinaryOperator::GreaterThan
        | BinaryOperator::LessThanEquals
        | BinaryOperator::GreaterThanEquals
        | BinaryOperator::Instanceof
        | BinaryOperator::In => 11,
        BinaryOperator::EqualsEquals
        | BinaryOperator::EqualsEqualsEquals
        | BinaryOperator::BangEquals
        | BinaryOperator::BangEqualsEquals => 10,
        BinaryOperator::Ampersand => 9,
        BinaryOperator::Caret => 8,
        BinaryOperator::Pipe => 7,
        BinaryOperator::AmpersandAmpersand => 6,
        BinaryOperator::PipePipe => 5,
        BinaryOperator::QuestionQuestion => 4,
    }
}

/// Whether a `namespace` / `module` declaration is type-only — blanked whole — rather than
/// refused (ts-blank-space's `visitModule`): `declare global`, an ambient `module 'x'`, and
/// a `namespace` that is `declare`d or holds no value. A `module` keyword with an
/// identifier name is refused even when ambient, as both oracles refuse it.
fn module_is_type_only(decl: &TSModuleDeclaration<'_>) -> bool {
    decl.global
        || matches!(decl.id, TSModuleName::Literal(_))
        || (decl.kind == TSModuleDeclarationKind::Namespace
            && (decl.declare || !namespace_holds_value(decl)))
}

/// Whether a `namespace`'s body holds a value (ts-blank-space's `valueNamespaceWorker`).
fn namespace_holds_value(decl: &TSModuleDeclaration<'_>) -> bool {
    match &decl.body {
        None => false,
        Some(TSModuleDeclarationBody::TSModuleDeclaration(inner)) => {
            nested_module_holds_value(inner)
        }
        Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
            block.body.iter().any(namespace_member_holds_value)
        }
    }
}

/// A module nested in a namespace: one not spelled `namespace` (an ambient `module 'x'`,
/// `global`, a `module` keyword) counts as a value, as ts-blank-space counts it.
fn nested_module_holds_value(decl: &TSModuleDeclaration<'_>) -> bool {
    decl.global || decl.kind != TSModuleDeclarationKind::Namespace || namespace_holds_value(decl)
}

/// Whether one statement of a namespace body is a value: anything but a type alias, an
/// interface, a type-only nested namespace, or an unexported import-equals alias.
fn namespace_member_holds_value(statement: &Statement<'_>) -> bool {
    match &statement.kind {
        StatementKind::TSTypeAliasDeclaration(_) | StatementKind::TSInterfaceDeclaration(_) => {
            false
        }
        StatementKind::TSModuleDeclaration(decl) => nested_module_holds_value(decl),
        StatementKind::TSImportEqualsDeclaration(decl) => decl.is_export,
        StatementKind::ExportNamedDeclaration(export) => match export.declaration {
            Some(declaration) => match &declaration.kind {
                StatementKind::TSTypeAliasDeclaration(_)
                | StatementKind::TSInterfaceDeclaration(_) => false,
                StatementKind::TSModuleDeclaration(decl) => nested_module_holds_value(decl),
                StatementKind::TSImportEqualsDeclaration(_)
                | StatementKind::ExpressionStatement(_)
                | StatementKind::VariableDeclaration(_)
                | StatementKind::TSDeclareFunction(_)
                | StatementKind::TSEnumDeclaration(_)
                | StatementKind::ReturnStatement(_)
                | StatementKind::BlockStatement(_)
                | StatementKind::FunctionDeclaration(_)
                | StatementKind::ClassDeclaration(_)
                | StatementKind::ExportNamedDeclaration(_)
                | StatementKind::ExportDefaultDeclaration(_)
                | StatementKind::ExportAllDeclaration(_)
                | StatementKind::TSExportAssignment(_)
                | StatementKind::TSNamespaceExportDeclaration(_)
                | StatementKind::ImportDeclaration(_)
                | StatementKind::IfStatement(_)
                | StatementKind::ForStatement(_)
                | StatementKind::ForInStatement(_)
                | StatementKind::ForOfStatement(_)
                | StatementKind::WhileStatement(_)
                | StatementKind::DoWhileStatement(_)
                | StatementKind::WithStatement(_)
                | StatementKind::SwitchStatement(_)
                | StatementKind::TryStatement(_)
                | StatementKind::ThrowStatement(_)
                | StatementKind::BreakStatement(_)
                | StatementKind::ContinueStatement(_)
                | StatementKind::LabeledStatement(_)
                | StatementKind::EmptyStatement(_)
                | StatementKind::DebuggerStatement(_) => true,
            },
            None => true,
        },
        StatementKind::ExpressionStatement(_)
        | StatementKind::VariableDeclaration(_)
        | StatementKind::TSDeclareFunction(_)
        | StatementKind::TSEnumDeclaration(_)
        | StatementKind::ReturnStatement(_)
        | StatementKind::BlockStatement(_)
        | StatementKind::FunctionDeclaration(_)
        | StatementKind::ClassDeclaration(_)
        | StatementKind::ExportDefaultDeclaration(_)
        | StatementKind::ExportAllDeclaration(_)
        | StatementKind::TSExportAssignment(_)
        | StatementKind::TSNamespaceExportDeclaration(_)
        | StatementKind::ImportDeclaration(_)
        | StatementKind::IfStatement(_)
        | StatementKind::ForStatement(_)
        | StatementKind::ForInStatement(_)
        | StatementKind::ForOfStatement(_)
        | StatementKind::WhileStatement(_)
        | StatementKind::DoWhileStatement(_)
        | StatementKind::WithStatement(_)
        | StatementKind::SwitchStatement(_)
        | StatementKind::TryStatement(_)
        | StatementKind::ThrowStatement(_)
        | StatementKind::BreakStatement(_)
        | StatementKind::ContinueStatement(_)
        | StatementKind::LabeledStatement(_)
        | StatementKind::EmptyStatement(_)
        | StatementKind::DebuggerStatement(_) => true,
    }
}
