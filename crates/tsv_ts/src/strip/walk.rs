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
//!
//! **The import records.** The walk is also the one producer of [`crate::Imports`], in a
//! collecting mode ([`Mode::Imports`], [`Mode::StripWithImports`]): a record is pushed where
//! its node is visited, before any type-only early return, so the records come out in
//! source order (the walk is pre-order, and a chain walks its base before its links), and
//! `has_module_syntax` is set where its evidence is met. A [`Mode::Imports`] walk also
//! descends into the four value subtrees the strip refuses without walking — an enum's
//! member initializers, a value namespace's body, an `export =` operand and a regrouping
//! assertion's operand — and is otherwise the same walk; its edits and refusal are never
//! read. A [`Mode::Strip`] walk collects nothing: it pushes no
//! record and tracks no module syntax, each site asking the mode with a branch, so the
//! edits and the refusal are the same in every mode and the strip pays for no record. A
//! [`Mode::StripWithImports`] walk is the strip's walk collecting as it goes: the strip's
//! edits and refusal, and, over a source the strip accepts, the records an imports walk
//! collects (the two walks part only inside a subtree the strip refuses). The mode is a
//! runtime value rather than a type parameter, so the bindings link one walk.

use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use smallvec::SmallVec;

use crate::ast::{
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
use crate::ast::{AstView, Run, StmtId};
use crate::ast::{
    ExportAllDeclaration, ImportAttribute, ImportExpression, LiteralValue, TSEnumDeclaration,
    TSImportEqualsDeclaration, TSModuleReference, VariableDeclarationKind,
};
use crate::imports::{ByteSpan, ImportRecord, ImportRecordKind, Imports, TypeOnly};

use super::blank::{Edit, Mark};
use super::scan::{Continuation, EXPONENT_PRECEDENCE, Source};
use super::{StripError, UnsupportedConstruct};

/// Where a statement stands: an element of a statement list, or the lone body of an `if` /
/// loop / label / `with`, where a statement blanked whole always becomes an empty statement
/// (`;`) so the statement after it cannot become the body.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Position {
    List,
    Nested,
}

/// What a walk is for: the strip's edits, the import records alone, or both (module docs).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// The strip: no record is collected, and the subtrees it refuses are not walked, since
    /// a refused strip has no output.
    Strip,
    /// `parse_imports`: the records are collected, and the subtrees the strip refuses are
    /// walked for theirs.
    Imports,
    /// `strip_with_imports`: the strip's walk, collecting the records as it goes. The
    /// subtrees the strip refuses are not walked: a refusal returns no records either, so
    /// over every source the strip accepts the records are an imports walk's.
    StripWithImports,
}

/// The walk's state: the edits it has collected, the first refusal, the import records,
/// and the state the hazard substitutions read.
pub(crate) struct Walk<'s, 'b> {
    src: Source<'s>,
    /// Reads the expressions of the tree being walked.
    view: AstView<'b>,
    /// What the walk collects ([`Self::collects`]) and whether the subtrees the strip
    /// refuses are walked ([`Self::walks_refused`]).
    mode: Mode,
    edits: BumpVec<'b, Edit>,
    /// The import records, in source order — empty in [`Mode::Strip`].
    records: Vec<ImportRecord>,
    /// Whether module syntax was met: a top-level import or export declaration, an
    /// `import.meta`, a top-level `await` / `for await` / `await using`. Tracked by a
    /// collecting walk alone.
    module_syntax: bool,
    /// How many functions, class static blocks, field initializers, namespace bodies and
    /// enum member initializers enclose the walk: an `await` is a top-level one at zero (tsc
    /// reads the last two as function-like: TS1308). A count rather than a saved flag, so a
    /// function — the walk's most frequent boundary — pays an increment and a decrement.
    /// Kept by a collecting walk alone ([`Self::enter_function`]).
    function_depth: u32,
    /// Inside a namespace body (walked in [`Mode::Imports`] alone), where a static import,
    /// a re-export and an `import x = require('y')` are no records: tsc rejects them there
    /// (TS1147, TS1194). A dynamic import is a record.
    in_namespace: bool,
    /// The refusal with the earliest start — the one ts-blank-space reports first.
    refusal: Option<(UnsupportedConstruct, Span)>,
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
    pub(crate) fn new(source: &'s str, arena: &'b Bump, mode: Mode, view: AstView<'b>) -> Self {
        Self {
            src: Source::new(source),
            view,
            mode,
            edits: BumpVec::new_in(arena),
            records: Vec::new(),
            module_syntax: false,
            function_depth: 0,
            in_namespace: false,
            refusal: None,
            marked_end: None,
            statement_end: None,
            terminated_at: None,
            open: Tail::Closed,
            chain_frames: 0,
        }
    }

    /// The collected edits, or the refusal.
    pub(super) fn finish(self) -> Result<BumpVec<'b, Edit>, StripError> {
        debug_assert!(
            self.mode != Mode::Strip
                || (self.records.is_empty() && !self.module_syntax && self.function_depth == 0),
            "a strip walk collects nothing"
        );
        match self.refusal {
            Some((kind, span)) => Err(StripError::unsupported(kind, self.src.text, span.range())),
            None => Ok(self.edits),
        }
    }

    /// Whether the walk collects import records and module syntax: every walk but a
    /// [`Mode::Strip`] one.
    #[inline]
    fn collects(&self) -> bool {
        match self.mode {
            Mode::Strip => false,
            Mode::Imports | Mode::StripWithImports => true,
        }
    }

    /// Whether a static import, a re-export or an import-equals met here is a record: in a
    /// collecting walk, outside every namespace body ([`Self::in_namespace`]).
    #[inline]
    fn collects_static(&self) -> bool {
        self.collects() && !self.in_namespace
    }

    /// Whether the subtrees the strip refuses are walked: a [`Mode::Imports`] walk alone.
    #[inline]
    fn walks_refused(&self) -> bool {
        self.mode == Mode::Imports
    }

    /// Enter a function-like scope for `has_module_syntax`'s top-level `await`; paired with
    /// [`Self::leave_function`].
    #[inline]
    fn enter_function(&mut self) {
        if self.collects() {
            self.function_depth += 1;
        }
    }

    #[inline]
    fn leave_function(&mut self) {
        if self.collects() {
            self.function_depth -= 1;
        }
    }

    /// Whether an `await` met here is a top-level one, in a collecting walk.
    #[inline]
    fn top_level_await(&self) -> bool {
        self.collects() && self.function_depth == 0
    }

    /// The collected edits and import records, or the refusal — the
    /// [`Mode::StripWithImports`] product. A refusal discards the records.
    pub(super) fn finish_with_imports(self) -> Result<(BumpVec<'b, Edit>, Imports), StripError> {
        match self.refusal {
            Some((kind, span)) => Err(StripError::unsupported(kind, self.src.text, span.range())),
            None => Ok((
                self.edits,
                Imports {
                    records: self.records,
                    has_module_syntax: self.module_syntax,
                },
            )),
        }
    }

    /// The import records, whatever the walk refused — the [`Mode::Imports`] product.
    pub(crate) fn into_imports(self) -> Imports {
        Imports {
            records: self.records,
            has_module_syntax: self.module_syntax,
        }
    }

    pub(crate) fn program(&mut self, program: &Program<'_>) {
        // `statement_list`'s loop, plus the module-declaration half of `has_module_syntax`
        // asked of each top-level statement as it is walked (measured cheaper than a pass
        // of its own, and than asking at the import and export arms). The strip runs this
        // loop too, its mode asked per statement: measured cheaper for it than a call to
        // `statement_list`
        let outer_end = self.statement_end;
        let outer_open = std::mem::replace(&mut self.open, Tail::Closed);
        for statement in self.view.stmt_list(program.body) {
            let statement = self.view.stmt(*statement);
            if self.collects() {
                self.module_syntax |= is_module_declaration(statement);
            }
            self.statement_end = Some(statement.span.end);
            self.statement(statement, statement.span, Position::List);
        }
        self.statement_end = outer_end;
        self.open = outer_open;
    }

    //
    // Import records
    //
    // TODO: no record comes from a type, which every site blanks whole without walking it
    // (module docs), so a type-position import (`let x: import('./a').T`, `typeof
    // import('./a')`) is no record — the known gap `crate::imports` documents. A dependency
    // graph over types would need a walk of the type's own, in a collecting mode alone

    /// Room for more records: [`RECORDS_FIRST_CAPACITY`] at the first, then doubling —
    /// most files' records in one allocation, with no count taken ahead of the walk.
    #[cold]
    #[inline(never)]
    fn grow_records(&mut self) {
        self.records
            .reserve(self.records.capacity().max(RECORDS_FIRST_CAPACITY));
    }

    fn record(
        &mut self,
        kind: ImportRecordKind,
        span: Span,
        specifier: Option<ByteSpan>,
        type_only: TypeOnly,
        phase: crate::ImportPhase,
        attributes: Option<ByteSpan>,
    ) {
        if self.records.len() == self.records.capacity() {
            self.grow_records();
        }
        self.records.push(ImportRecord {
            kind,
            span: byte_span(span),
            specifier,
            type_only,
            phase,
            attributes,
        });
    }

    /// A static import's record, its `type_only` read by [`Self::import`] off the walk over
    /// its specifiers. None inside a namespace body.
    fn record_import(&mut self, import: &ImportDeclaration<'_>, span: Span, type_only: TypeOnly) {
        if !self.collects_static() {
            return;
        }
        let attributes = self.attributes_clause(import.source.span.end, import.attributes);
        self.record(
            ImportRecordKind::Static,
            span,
            Some(string_contents(import.source.span)),
            type_only,
            import.phase,
            attributes,
        );
    }

    /// A re-export's record — none for an export with no `from`, nor inside a namespace
    /// body — its `type_only` read by [`Self::export_named`] off the walk over its
    /// specifiers.
    fn record_re_export(
        &mut self,
        export: &ExportNamedDeclaration<'_>,
        span: Span,
        type_only: TypeOnly,
    ) {
        let Some(source) = &export.source else {
            return;
        };
        if !self.collects_static() {
            return;
        }
        let attributes = self.attributes_clause(source.span.end, export.attributes);
        self.record(
            ImportRecordKind::ReExport,
            span,
            Some(string_contents(source.span)),
            type_only,
            crate::ImportPhase::None,
            attributes,
        );
    }

    fn record_export_all(&mut self, export: &ExportAllDeclaration<'_>, span: Span) {
        if !self.collects_static() {
            return;
        }
        let type_only = if export.export_kind == ExportKind::Type {
            TypeOnly::Statement
        } else {
            TypeOnly::No
        };
        let attributes = self.attributes_clause(export.source.span.end, export.attributes);
        self.record(
            ImportRecordKind::ReExport,
            span,
            Some(string_contents(export.source.span)),
            type_only,
            crate::ImportPhase::None,
            attributes,
        );
    }

    fn record_import_equals(&mut self, import: &TSImportEqualsDeclaration<'_>, span: Span) {
        let TSModuleReference::ExternalModuleReference(reference) = &import.module_reference else {
            return;
        };
        if !self.collects_static() {
            return;
        }
        let type_only = if import.import_kind == ImportKind::Type {
            TypeOnly::Statement
        } else {
            TypeOnly::No
        };
        self.record(
            ImportRecordKind::ImportEquals,
            span,
            Some(string_contents(reference.expression.span)),
            type_only,
            crate::ImportPhase::None,
            None,
        );
    }

    /// A dynamic import: its record, then its operands. Out of line: the record's
    /// arguments, inlined into [`Self::expr_inner`], cost every expression's walk a register
    /// for a kind few expressions are.
    #[inline(never)]
    fn import_expression(&mut self, import: &ImportExpression, span: Span) {
        if self.collects() {
            self.record_dynamic(import, span);
        }
        self.expr(self.view.expr(import.source));
        if let Some(options) = self.view.opt(import.options) {
            self.expr(options);
        }
    }

    fn record_dynamic(&mut self, import: &ImportExpression, span: Span) {
        let specifier = match &self.view.expr(import.source).kind {
            ExpressionKind::Literal(literal)
                if matches!(literal.value, LiteralValue::String(_)) =>
            {
                Some(string_contents(literal.span))
            }
            ExpressionKind::TemplateLiteral(template) if template.expressions.is_empty() => {
                template
                    .quasis
                    .first()
                    .map(|quasi| byte_span(quasi.raw_span))
            }
            _ => None,
        };
        self.record(
            ImportRecordKind::Dynamic,
            span,
            specifier,
            TypeOnly::No,
            import.phase,
            import
                .options
                .map(|options| byte_span(self.view.expr(options).span)),
        );
    }

    /// A static import's or a re-export's attributes clause, from its `with` keyword —
    /// the first token past the source literal ending at `source_end` — through its `}`.
    /// The AST holds the attributes alone, so the clause's two ends are read off the
    /// source, past whitespace and comments.
    fn attributes_clause(
        &self,
        source_end: u32,
        attributes: Option<&[ImportAttribute<'_>]>,
    ) -> Option<ByteSpan> {
        let attributes = attributes?;
        let with = self.src.next_significant(source_end);
        let close = match attributes.last() {
            Some(last) => {
                let at = self.src.next_significant(last.span.end);
                if self.src.bytes.get(at as usize) == Some(&b',') {
                    self.src.next_significant(at + 1)
                } else {
                    at
                }
            }
            None => {
                let open = self.src.next_significant(with + b"with".len() as u32);
                debug_assert_eq!(self.src.bytes.get(open as usize), Some(&b'{'));
                self.src.next_significant(open + 1)
            }
        };
        debug_assert!(self.src.bytes[with as usize..].starts_with(b"with"));
        debug_assert_eq!(self.src.bytes.get(close as usize), Some(&b'}'));
        Some(ByteSpan {
            start: with,
            end: close + 1,
        })
    }

    /// A meta property: `import.meta` is module syntax, `new.target` is not.
    fn meta_property(&mut self, span: Span) {
        if self.collects() && self.src.bytes[span.start as usize] == b'i' {
            self.module_syntax = true;
        }
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

    fn refuse(&mut self, kind: UnsupportedConstruct, span: Span) {
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

    fn statement_list(&mut self, list: Run<StmtId>, function_body: bool) {
        let outer_marked = self.marked_end;
        if function_body {
            self.marked_end = None;
        }
        let outer_end = self.statement_end;
        let outer_open = std::mem::replace(&mut self.open, Tail::Closed);
        for statement in self.view.stmt_list(list) {
            let statement = self.view.stmt(*statement);
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
                self.expr(self.view.expr(s.expression));
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
            &StatementKind::TSDeclareFunction(func) => {
                let func = self.view.declare_fn(func);
                if func.declare {
                    self.blank_statement(outer, position);
                } else {
                    // an overload signature
                    self.blank_signature(outer, position);
                }
                false
            }
            &StatementKind::TSEnumDeclaration(decl) => {
                let decl = self.view.enum_decl(decl);
                if decl.declare {
                    self.blank_statement(outer, position);
                    return false;
                }
                self.refuse(UnsupportedConstruct::Enum, outer);
                if self.walks_refused() {
                    self.enum_initializers(decl);
                }
                true
            }
            &StatementKind::TSModuleDeclaration(decl) => {
                let decl = self.view.module_decl(decl);
                if module_is_type_only(self.view, decl) {
                    self.blank_statement(outer, position);
                    return false;
                }
                self.refuse(UnsupportedConstruct::ValueNamespace, outer);
                if self.walks_refused() && !decl.declare {
                    self.namespace_body(decl);
                }
                true
            }
            StatementKind::ReturnStatement(s) => {
                if let Some(argument) = self.view.opt(s.argument) {
                    self.expr(argument);
                }
                true
            }
            StatementKind::BlockStatement(block) => {
                self.statement_list(block.body, false);
                true
            }
            &StatementKind::FunctionDeclaration(func) => {
                let func = self.view.fn_decl(func);
                self.function(FunctionView::declaration(self.view, func));
                true
            }
            &StatementKind::ClassDeclaration(class) => {
                let class = self.view.class_decl(class);
                if class.declare {
                    self.blank_statement(outer, position);
                    return false;
                }
                self.class(ClassView::declaration(self.view, class), outer.start);
                true
            }
            &StatementKind::ExportNamedDeclaration(export) => {
                let export = self.view.export_named(export);
                self.export_named(export, outer, position)
            }
            &StatementKind::ExportDefaultDeclaration(export) => {
                let export = self.view.export_default(export);
                self.export_default(export, outer, position)
            }
            &StatementKind::ExportAllDeclaration(export) => {
                let export = self.view.export_all(export);
                self.record_export_all(export, outer);
                if export.export_kind == ExportKind::Type {
                    self.blank_statement(outer, position);
                    return false;
                }
                true
            }
            &StatementKind::TSExportAssignment(assignment) => {
                let assignment = self.view.export_assign(assignment);
                self.refuse(UnsupportedConstruct::ExportEquals, outer);
                if self.walks_refused() {
                    self.expr(self.view.expr(assignment.expression));
                }
                true
            }
            // `export as namespace N;` is kept verbatim, as both oracles keep it: it declares
            // a UMD global, which only a declaration file may (tsc's TS1315)
            StatementKind::TSNamespaceExportDeclaration(_) => true,
            &StatementKind::ImportDeclaration(import) => {
                self.import(self.view.import_decl(import), outer, position)
            }
            &StatementKind::TSImportEqualsDeclaration(import) => {
                let import = self.view.import_equals(import);
                self.record_import_equals(import, outer);
                self.refuse(UnsupportedConstruct::ImportEquals, outer);
                true
            }
            StatementKind::IfStatement(s) => {
                self.expr(self.view.expr(s.test));
                self.nested_statement(self.view.stmt(s.consequent));
                if let Some(alternate) = self.view.opt_stmt(s.alternate) {
                    self.nested_statement(alternate);
                }
                true
            }
            StatementKind::ForStatement(s) => {
                if let Some(init) = self.view.opt_for_head(s.init) {
                    match init {
                        ForInit::VariableDeclaration(decl) => self.variable_declaration(decl),
                        ForInit::Expression(e) => self.expr(self.view.expr(*e)),
                    }
                }
                if let Some(test) = self.view.opt(s.test) {
                    self.expr(test);
                }
                if let Some(update) = self.view.opt(s.update) {
                    self.expr(update);
                }
                self.nested_statement(self.view.stmt(s.body));
                true
            }
            StatementKind::ForInStatement(s) => {
                self.for_left(self.view.for_left(s.left));
                self.expr(self.view.expr(s.right));
                self.nested_statement(self.view.stmt(s.body));
                true
            }
            StatementKind::ForOfStatement(s) => {
                if s.r#await && self.top_level_await() {
                    self.module_syntax = true;
                }
                self.for_left(self.view.for_left(s.left));
                self.expr(self.view.expr(s.right));
                self.nested_statement(self.view.stmt(s.body));
                true
            }
            StatementKind::WhileStatement(s) => {
                self.expr(self.view.expr(s.test));
                self.nested_statement(self.view.stmt(s.body));
                true
            }
            StatementKind::DoWhileStatement(s) => {
                self.nested_statement(self.view.stmt(s.body));
                self.expr(self.view.expr(s.test));
                true
            }
            StatementKind::WithStatement(s) => {
                self.expr(self.view.expr(s.object));
                self.nested_statement(self.view.stmt(s.body));
                true
            }
            StatementKind::SwitchStatement(s) => {
                self.expr(self.view.expr(s.discriminant));
                for case in s.cases {
                    if let Some(test) = case.test {
                        self.expr(self.view.expr(test));
                    }
                    self.statement_list(case.consequent, false);
                    self.mark(case.span.end);
                }
                true
            }
            StatementKind::TryStatement(s) => {
                self.block(&s.block);
                if let Some(handler) = self.view.opt_catch(s.handler) {
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
                self.expr(self.view.expr(s.argument));
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
                self.nested_statement(self.view.stmt(s.body));
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
            StatementKind::ExpressionStatement(s) => {
                self.unterminated_tail(self.view.expr(s.expression), end)
            }
            StatementKind::ThrowStatement(s) => {
                self.unterminated_tail(self.view.expr(s.argument), end)
            }
            StatementKind::ReturnStatement(s) => s.argument.map_or(Tail::Closed, |argument| {
                self.unterminated_tail(self.view.expr(argument), end)
            }),
            StatementKind::VariableDeclaration(decl) => match decl.declarations.last() {
                Some(declarator) if !decl.declare => declarator.init.map_or(Tail::Closed, |init| {
                    self.unterminated_tail(self.view.expr(init), end)
                }),
                _ => Tail::Closed,
            },
            &StatementKind::ExportNamedDeclaration(export) => {
                let export = self.view.export_named(export);
                export.declaration.map_or(Tail::Closed, |declaration| {
                    self.statement_tail(self.view.stmt(declaration))
                })
            }
            &StatementKind::ExportDefaultDeclaration(export) => {
                match &self.view.export_default(export).declaration {
                    ExportDefaultValue::Expression(e) => {
                        self.unterminated_tail(self.view.expr(*e), end)
                    }
                    ExportDefaultValue::FunctionDeclaration(_)
                    | ExportDefaultValue::TSDeclareFunction(_)
                    | ExportDefaultValue::ClassDeclaration(_)
                    | ExportDefaultValue::TSInterfaceDeclaration(_) => Tail::Closed,
                }
            }
            StatementKind::IfStatement(s) => {
                self.statement_tail(self.view.stmt(s.alternate.unwrap_or(s.consequent)))
            }
            StatementKind::ForStatement(s) => self.statement_tail(self.view.stmt(s.body)),
            StatementKind::ForInStatement(s) => self.statement_tail(self.view.stmt(s.body)),
            StatementKind::ForOfStatement(s) => self.statement_tail(self.view.stmt(s.body)),
            StatementKind::WhileStatement(s) => self.statement_tail(self.view.stmt(s.body)),
            StatementKind::WithStatement(s) => self.statement_tail(self.view.stmt(s.body)),
            StatementKind::LabeledStatement(s) => self.statement_tail(self.view.stmt(s.body)),
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
            expression_tail(self.view, e, end)
        }
    }

    fn block(&mut self, block: &BlockStatement) {
        self.statement_list(block.body, false);
        self.mark(block.span.end);
    }

    fn variable_declaration(&mut self, decl: &VariableDeclaration<'_>) {
        // a top-level `await using` (in a `for` head too) is module syntax, as a top-level
        // `await` is
        if decl.kind == VariableDeclarationKind::AwaitUsing && self.top_level_await() {
            self.module_syntax = true;
        }
        for declarator in decl.declarations {
            self.expr(self.view.expr(declarator.id));
            if declarator.definite {
                // `let a!: T` — the `!` follows the name, ahead of the annotation
                let end = declarator
                    .init
                    .map_or(declarator.span.end, |init| self.view.expr(init).span.start);
                if let Some(bang) =
                    self.src
                        .find_char(self.view.expr(declarator.id).span.start, end, b'!')
                {
                    self.blank(bang, bang + 1);
                }
            }
            if let Some(init) = self.view.opt(declarator.init) {
                self.expr(init);
            }
        }
    }

    fn for_left(&mut self, left: &ForInOfLeft<'_>) {
        match left {
            ForInOfLeft::VariableDeclaration(decl) => self.variable_declaration(decl),
            ForInOfLeft::Pattern(pattern) => self.expr(self.view.expr(*pattern)),
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
            self.record_re_export(export, outer, TypeOnly::Statement);
            self.blank_statement(outer, position);
            return false;
        }
        if let Some(declaration) = self.view.opt_stmt(export.declaration) {
            return self.statement_kind(declaration, outer, position);
        }
        let mut every_type = !export.specifiers.is_empty();
        for specifier in export.specifiers {
            if specifier.export_kind == ExportKind::Type {
                let end = self.src.end_with_trailing_comma(specifier.span.end);
                self.blank(specifier.span.start, end);
            } else {
                every_type = false;
            }
        }
        let type_only = if every_type {
            TypeOnly::Specifiers
        } else {
            TypeOnly::No
        };
        self.record_re_export(export, outer, type_only);
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
                self.expr(self.view.expr(*e));
                true
            }
            ExportDefaultValue::FunctionDeclaration(func) => {
                self.function(FunctionView::declaration(self.view, func));
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
                self.class(ClassView::declaration(self.view, class), outer.start);
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
            self.record_import(import, outer, TypeOnly::Statement);
            self.blank_statement(outer, position);
            return false;
        }
        let mut every_type = !import.specifiers.is_empty();
        for specifier in import.specifiers {
            match specifier {
                ImportSpecifier::Named(named) if named.import_kind == ImportKind::Type => {
                    let end = self.src.end_with_trailing_comma(named.span.end);
                    self.blank(named.span.start, end);
                }
                ImportSpecifier::Named(_)
                | ImportSpecifier::Default(_)
                | ImportSpecifier::Namespace(_) => every_type = false,
            }
        }
        let type_only = if every_type {
            TypeOnly::Specifiers
        } else {
            TypeOnly::No
        };
        self.record_import(import, outer, type_only);
        true
    }

    //
    // Expressions
    //

    /// Walk expression `e`. A leaf holding no TypeScript — a literal, `this`, `super`, a private
    /// name, or an identifier with no binding state (no `extra`, not `optional`:
    /// `Walk::identifier` would blank nothing) — only marks the `;` state, which is all the
    /// full walk ([`Self::expr_inner`]) does for it; it is answered here, inlined at each call
    /// site, without entering that walk's frame. A meta property is a leaf too, but takes the
    /// full walk, which reads an `import.meta` as module syntax: a rare kind, kept out of the
    /// inlined set every call site tests.
    #[inline]
    fn expr(&mut self, e: &Expression<'_>) {
        match &e.kind {
            ExpressionKind::Literal(_)
            | ExpressionKind::RegexLiteral(_)
            | ExpressionKind::ThisExpression(_)
            | ExpressionKind::Super(_)
            | ExpressionKind::PrivateIdentifier(_) => self.mark(e.span.end),
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
    fn binary_chain(&mut self, binary: &BinaryExpression) {
        let mut rights: SmallVec<[&Expression<'_>; 8]> = SmallVec::new();
        rights.push(self.view.expr(binary.right));
        let mut left = self.view.expr(binary.left);
        while let ExpressionKind::BinaryExpression(inner) = &left.kind {
            rights.push(self.view.expr(inner.right));
            left = self.view.expr(inner.left);
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
            | ExpressionKind::PrivateIdentifier(_) => self.mark(operand.span.end),
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
    ///
    /// A refused assertion's work past its operand is skipped, as [`Self::assertion`]
    /// skips it. The strip walks nothing under one either: the chain ends at the first, so
    /// it is the innermost link, and the base is not walked. An imports walk walks on
    /// through every refused link to the base ([`Self::walks_refused`]). None is refused
    /// today: a chain past `CHAIN_FRAMES` is entered at a link's operand, so no assertion
    /// in it is followed by a binary operator. The flag keeps the loop's verdict the
    /// recursion's should that change.
    #[inline(never)]
    fn chain(&mut self, e: &Expression<'_>) {
        // each link with whether its grouping check refused it
        let mut links: SmallVec<[(&Expression<'_>, bool); 16]> = SmallVec::new();
        let mut node = e;
        let mut walks_base = true;
        loop {
            let (operand, refused) = match &node.kind {
                ExpressionKind::CallExpression(call) => (call.callee, false),
                ExpressionKind::NewExpression(new) => (new.callee, false),
                ExpressionKind::MemberExpression(member) => (member.object, false),
                &ExpressionKind::TaggedTemplateExpression(tagged) => {
                    (self.view.tagged_tpl(tagged).tag, false)
                }
                ExpressionKind::TSInstantiationExpression(instantiation) => {
                    (instantiation.expression, false)
                }
                ExpressionKind::TSNonNullExpression(non_null) => (non_null.expression, false),
                ExpressionKind::TSAsExpression(assertion) => {
                    let refused = self
                        .assertion_head(
                            node.span,
                            self.view.expr(assertion.expression),
                            self.view.ty(assertion.type_annotation),
                            b"as",
                        )
                        .is_none();
                    (assertion.expression, refused)
                }
                ExpressionKind::TSSatisfiesExpression(assertion) => {
                    let refused = self
                        .assertion_head(
                            node.span,
                            self.view.expr(assertion.expression),
                            self.view.ty(assertion.type_annotation),
                            b"satisfies",
                        )
                        .is_none();
                    (assertion.expression, refused)
                }
                _ => break,
            };
            links.push((node, refused));
            // a refused assertion's operand is not walked by the strip: the chain ends at it
            if refused && !self.walks_refused() {
                walks_base = false;
                break;
            }
            node = self.view.expr(operand);
        }
        if walks_base {
            self.expr(node);
        }
        for &(link, refused) in links.iter().rev() {
            if !refused {
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
                for argument in self.view.list(call.arguments) {
                    let argument = self.view.expr(*argument);
                    self.expr(argument);
                }
            }
            ExpressionKind::NewExpression(new) => {
                self.type_arguments(new.type_arguments.as_ref());
                for argument in self.view.list(new.arguments) {
                    self.expr(self.view.expr(*argument));
                }
            }
            ExpressionKind::MemberExpression(member) => self.expr(self.view.expr(member.property)),
            &ExpressionKind::TaggedTemplateExpression(tagged) => {
                let tagged = self.view.tagged_tpl(tagged);
                self.type_arguments(tagged.type_arguments.as_ref());
                for expression in self.view.list(tagged.quasi.expressions) {
                    let expression = self.view.expr(*expression);
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
                    self.view.expr(assertion.expression),
                    self.view.ty(assertion.type_annotation),
                    b"as",
                );
                self.assertion_tail(link.span, blank_from);
            }
            ExpressionKind::TSSatisfiesExpression(assertion) => {
                let blank_from = self.assertion_blank_start(
                    self.view.expr(assertion.expression),
                    self.view.ty(assertion.type_annotation),
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
            | ExpressionKind::PrivateIdentifier(_) => {}
            ExpressionKind::MetaProperty(_) => self.meta_property(e.span),
            ExpressionKind::Identifier(id) => self.identifier(id),
            ExpressionKind::ObjectExpression(object) => {
                for property in object.properties {
                    match property {
                        ObjectProperty::Property(property) => self.property(property),
                        ObjectProperty::SpreadElement(spread) => {
                            self.expr(self.view.expr(spread.argument));
                        }
                    }
                }
            }
            ExpressionKind::ArrayExpression(array) => {
                for element in self.view.opt_list(array.elements).iter().flatten() {
                    let element = self.view.expr(*element);
                    self.expr(element);
                }
            }
            ExpressionKind::UnaryExpression(unary) => self.expr(self.view.expr(unary.argument)),
            ExpressionKind::UpdateExpression(update) => self.expr(self.view.expr(update.argument)),
            ExpressionKind::BinaryExpression(binary) => self.binary_chain(binary),
            ExpressionKind::CallExpression(call) => {
                self.link_operand(self.view.expr(call.callee));
                self.type_arguments(call.type_arguments.as_ref());
                for argument in self.view.list(call.arguments) {
                    self.expr(self.view.expr(*argument));
                }
            }
            ExpressionKind::NewExpression(new) => {
                self.link_operand(self.view.expr(new.callee));
                self.type_arguments(new.type_arguments.as_ref());
                for argument in self.view.list(new.arguments) {
                    self.expr(self.view.expr(*argument));
                }
            }
            ExpressionKind::MemberExpression(member) => {
                self.link_operand(self.view.expr(member.object));
                self.expr(self.view.expr(member.property));
            }
            ExpressionKind::ConditionalExpression(conditional) => {
                self.expr(self.view.expr(conditional.test));
                self.expr(self.view.expr(conditional.consequent));
                self.expr(self.view.expr(conditional.alternate));
            }
            &ExpressionKind::ArrowFunctionExpression(arrow) => {
                self.function(FunctionView::arrow(self.view, self.view.arrow_fn(arrow)));
            }
            &ExpressionKind::FunctionExpression(func) => {
                self.function(FunctionView::expression(self.view, self.view.fn_expr(func)));
            }
            &ExpressionKind::ClassExpression(class) => {
                self.class(
                    ClassView::expression(self.view, self.view.class_expr(class)),
                    e.span.start,
                );
            }
            ExpressionKind::SpreadElement(spread) => self.expr(self.view.expr(spread.argument)),
            ExpressionKind::TemplateLiteral(template) => {
                for expression in self.view.list(template.expressions) {
                    let expression = self.view.expr(*expression);
                    self.expr(expression);
                }
            }
            &ExpressionKind::TaggedTemplateExpression(tagged) => {
                let tagged = self.view.tagged_tpl(tagged);
                self.link_operand(self.view.expr(tagged.tag));
                self.type_arguments(tagged.type_arguments.as_ref());
                for expression in self.view.list(tagged.quasi.expressions) {
                    self.expr(self.view.expr(*expression));
                }
            }
            ExpressionKind::AwaitExpression(e) => {
                self.module_syntax |= self.top_level_await();
                self.expr(self.view.expr(e.argument));
            }
            ExpressionKind::YieldExpression(e) => {
                if let Some(argument) = self.view.opt(e.argument) {
                    self.expr(argument);
                }
            }
            ExpressionKind::SequenceExpression(sequence) => {
                for expression in self.view.list(sequence.expressions) {
                    self.expr(self.view.expr(*expression));
                }
            }
            ExpressionKind::AssignmentExpression(assignment) => {
                self.expr(self.view.expr(assignment.left));
                self.expr(self.view.expr(assignment.right));
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
                for element in self.view.opt_list(pattern.elements).iter().flatten() {
                    self.expr(self.view.expr(*element));
                }
                self.pattern_tail(e.span, pattern.optional, pattern.type_annotation.as_ref());
            }
            ExpressionKind::AssignmentPattern(pattern) => {
                self.decorators(pattern.decorators);
                self.expr(self.view.expr(pattern.left));
                self.expr(self.view.expr(pattern.right));
            }
            ExpressionKind::RestElement(rest) => self.rest_element(rest),
            ExpressionKind::TSTypeAssertion(assertion) => {
                self.refuse(UnsupportedConstruct::TypeAssertion, e.span);
                self.expr(self.view.expr(assertion.expression));
            }
            ExpressionKind::TSAsExpression(assertion) => {
                self.assertion(
                    e.span,
                    self.view.expr(assertion.expression),
                    self.view.ty(assertion.type_annotation),
                    b"as",
                );
            }
            ExpressionKind::TSSatisfiesExpression(assertion) => {
                self.assertion(
                    e.span,
                    self.view.expr(assertion.expression),
                    self.view.ty(assertion.type_annotation),
                    b"satisfies",
                );
            }
            ExpressionKind::TSInstantiationExpression(instantiation) => {
                self.link_operand(self.view.expr(instantiation.expression));
                self.blank_span(instantiation.type_arguments.span);
            }
            ExpressionKind::TSNonNullExpression(non_null) => {
                self.link_operand(self.view.expr(non_null.expression));
                // the `!` is the node's last byte, past any paren around the operand
                self.blank(e.span.end - 1, e.span.end);
            }
            ExpressionKind::TSParameterProperty(property) => {
                self.parameter_property(e.span, property);
                self.expr(self.view.expr(property.parameter));
            }
            ExpressionKind::ImportExpression(import) => self.import_expression(import, e.span),
            ExpressionKind::CommentBoundPair(cast) => self.expr(self.view.expr(cast.inner)),
            ExpressionKind::ParenthesizedExpression(parens) => {
                self.expr(parens.unreachable_contents(self.view));
            }
        }
        self.mark(e.span.end);
    }

    /// A binding identifier's TypeScript: parameter decorators stay (walked for the type
    /// arguments in them), the optional `?` and the annotation blank. A reference carries
    /// none of these.
    fn identifier(&mut self, id: &Identifier<'_>) {
        let decorators = id.decorators(self.view);
        self.decorators(decorators);
        let annotation = id.type_annotation(self.view);
        if id.optional {
            let from = decorators
                .and_then(<[Decorator]>::last)
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
    fn property(&mut self, property: &Property) {
        if property.computed {
            self.expr(self.view.expr(property.key));
        }
        self.expr(self.view.expr(property.value));
    }

    fn rest_element(&mut self, rest: &RestElement) {
        self.expr(self.view.expr(rest.argument));
        self.pattern_tail(rest.span, rest.optional, rest.type_annotation.as_ref());
    }

    /// A destructuring pattern's optional `?` (the last `?` ahead of its annotation, past
    /// the pattern's closing bracket) and annotation.
    fn pattern_tail(&mut self, span: Span, optional: bool, annotation: Option<&TSTypeAnnotation>) {
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

    fn decorators(&mut self, decorators: Option<&[Decorator]>) {
        for decorator in decorators.unwrap_or_default() {
            self.expr(self.view.expr(decorator.expression));
        }
    }

    fn type_arguments(&mut self, arguments: Option<&TSTypeParameterInstantiation>) {
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
        } else if self.walks_refused() {
            self.link_operand(operand);
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
            self.refuse(UnsupportedConstruct::AssertionGrouping, span);
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
                ExpressionKind::TSAsExpression(a) => (
                    self.view.expr(a.expression),
                    self.view.ty(a.type_annotation),
                    b"as",
                ),
                ExpressionKind::TSSatisfiesExpression(s) => (
                    self.view.expr(s.expression),
                    self.view.ty(s.type_annotation),
                    b"satisfies",
                ),
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
    fn parameter_property(&mut self, span: Span, property: &TSParameterProperty) {
        let end = self.view.expr(property.parameter).span.start;
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
        self.refuse(UnsupportedConstruct::ParameterProperty, first);
    }

    //
    // Functions and classes
    //

    fn function(&mut self, func: FunctionView<'_, '_>) {
        self.enter_function();
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
        self.leave_function();
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
            .and_then(<[Decorator]>::last)
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
                    }) => self.unterminated_tail(self.view.expr(*value), span.end),
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
                self.enter_function();
                self.statement_list(block.body, false);
                self.leave_function();
                true
            }
            ClassMember::IndexSignature(signature) => {
                self.blank_signature(signature.span, Position::List);
                false
            }
        }
    }

    fn property_definition(&mut self, property: &PropertyDefinition<'_>, open_field: bool) -> bool {
        let key = self.view.expr(property.key);
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
            && self.is_operator_key(key, property.computed);
        self.member_modifiers(MemberModifiers {
            from: decorators_end(property.decorators, property.span.start),
            to: key.span.start,
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
            self.expr(key);
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
                .or_else(|| {
                    property
                        .value
                        .as_ref()
                        .map(|v| self.view.expr(*v).span.start)
                })
                .unwrap_or(property.span.end);
            if let Some(at) = self.src.find_char(key.span.end, end, mark) {
                self.blank(at, at + 1);
            }
        }
        if let Some(annotation) = annotation {
            self.blank_span(annotation.span);
        }
        if let Some(value) = self.view.opt(property.value) {
            self.enter_function();
            self.expr(value);
            self.leave_function();
        }
        true
    }

    fn method_definition(&mut self, method: &MethodDefinition<'_>, open_field: bool) -> bool {
        let key = self.view.expr(method.key);
        let func = &method.value;
        // an overload signature or an abstract method or accessor: no body
        if self.view.stmt_list(func.body.body).is_empty()
            && func.body.span.start == func.body.span.end
        {
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
            && self.is_operator_key(key, method.computed);
        self.member_modifiers(MemberModifiers {
            from: decorators_end(method.decorators, method.span.start),
            to: key.span.start,
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
            self.expr(key);
        }
        if method.optional {
            let end = func
                .type_parameters
                .as_ref()
                .map_or(func.params_start, |tp| tp.span.start);
            if let Some(question) = self.src.find_char(key.span.end, end, b'?') {
                self.blank(question, question + 1);
            }
        }
        self.function(FunctionView::expression(self.view, func));
        true
    }

    /// An enum's member initializers, walked in [`Mode::Imports`] alone (the strip refuses
    /// the enum). An `await` in one is not a top-level one (TS1308). Out of line, as
    /// [`Self::namespace_body`]: the strip never takes it.
    #[inline(never)]
    fn enum_initializers(&mut self, decl: &TSEnumDeclaration<'_>) {
        self.enter_function();
        for member in decl.members {
            if let Some(initializer) = self.view.opt(member.initializer) {
                self.expr(initializer);
            }
        }
        self.leave_function();
    }

    /// A value namespace's body, walked in [`Mode::Imports`] alone (the strip refuses the
    /// namespace): a nested `namespace A.B` down to its block. Its static imports,
    /// re-exports and import-equals are no records, and an `await` in it is not a
    /// top-level one (TS1308): tsc reads the body as function-like. Out of line: the strip
    /// never takes it, and `statement_kind` is every statement's frame.
    #[inline(never)]
    fn namespace_body(&mut self, decl: &TSModuleDeclaration<'_>) {
        let outer_in_namespace = std::mem::replace(&mut self.in_namespace, true);
        self.enter_function();
        let mut decl = decl;
        loop {
            match decl.body.as_ref() {
                None => break,
                Some(&TSModuleDeclarationBody::TSModuleDeclaration(inner)) => {
                    decl = self.view.module_decl(inner);
                }
                Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                    self.statement_list(block.body, false);
                    break;
                }
            }
        }
        self.leave_function();
        self.in_namespace = outer_in_namespace;
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

/// Whether a top-level statement is an ECMAScript module declaration: an import, or an
/// export other than TypeScript's `export =`, `export as namespace` and `export import`.
const fn is_module_declaration(statement: &Statement<'_>) -> bool {
    matches!(
        statement.kind,
        StatementKind::ImportDeclaration(_)
            | StatementKind::ExportNamedDeclaration(_)
            | StatementKind::ExportDefaultDeclaration(_)
            | StatementKind::ExportAllDeclaration(_)
    )
}

/// The records' first allocation: a file's requests usually fit (most files make a few),
/// and a smaller one grows, copying, in the files that make more — measured costlier than
/// the slack.
const RECORDS_FIRST_CAPACITY: usize = 16;

/// A node span as a record's.
const fn byte_span(span: Span) -> ByteSpan {
    ByteSpan {
        start: span.start,
        end: span.end,
    }
}

/// A string literal's contents: its span less the two quotes.
const fn string_contents(literal: Span) -> ByteSpan {
    ByteSpan {
        start: literal.start + 1,
        end: literal.end - 1,
    }
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

fn decorators_end(decorators: Option<&[Decorator]>, start: u32) -> u32 {
    decorators
        .and_then(<[Decorator]>::last)
        .map_or(start, |last| last.span.end)
}

/// The parts of a function the walk reads, from any of its node shapes.
struct FunctionView<'n, 'a> {
    type_parameters: Option<&'n TSTypeParameterDeclaration<'a>>,
    params: &'n [Expression<'n>],
    /// The parameter list's `(`; `None` for a bare arrow parameter (`x => x`).
    params_open: Option<u32>,
    return_type: Option<&'n TSTypeAnnotation>,
    body: FunctionBody<'n>,
    arrow: bool,
}

enum FunctionBody<'n> {
    Block(&'n BlockStatement),
    Expression(&'n Expression<'n>),
}

impl<'n, 'a> FunctionView<'n, 'a> {
    fn declaration(view: AstView<'n>, func: &'n FunctionDeclaration<'a>) -> Self {
        Self {
            type_parameters: func.type_parameters.as_ref(),
            params: view.run(func.params),
            params_open: Some(func.params_start),
            return_type: func.return_type.as_ref(),
            body: FunctionBody::Block(&func.body),
            arrow: false,
        }
    }

    fn expression(view: AstView<'n>, func: &'n FunctionExpression<'a>) -> Self {
        Self {
            type_parameters: func.type_parameters.as_ref(),
            params: view.run(func.params),
            params_open: Some(func.params_start),
            return_type: func.return_type.as_ref(),
            body: FunctionBody::Block(&func.body),
            arrow: false,
        }
    }

    fn arrow(view: AstView<'n>, arrow: &'n ArrowFunctionExpression<'a>) -> Self {
        Self {
            type_parameters: arrow.type_parameters.as_ref(),
            params: view.run(arrow.params),
            params_open: arrow.params_start,
            return_type: arrow.return_type.as_ref(),
            body: match &arrow.body {
                ArrowFunctionBody::Expression(body) => FunctionBody::Expression(view.expr(*body)),
                ArrowFunctionBody::BlockStatement(block) => FunctionBody::Block(block),
            },
            arrow: true,
        }
    }
}

/// The parts of a class the walk reads, from a declaration or an expression.
struct ClassView<'n, 'a> {
    decorators: Option<&'a [Decorator]>,
    r#abstract: bool,
    type_parameters: Option<&'n TSTypeParameterDeclaration<'a>>,
    super_class: Option<&'n Expression<'n>>,
    super_type_parameters: Option<&'n TSTypeParameterInstantiation>,
    implements: &'a [TSInterfaceHeritage<'a>],
    body: &'n ClassBody<'a>,
}

impl<'n, 'a> ClassView<'n, 'a> {
    fn declaration(view: AstView<'n>, class: &'n ClassDeclaration<'a>) -> Self {
        Self {
            decorators: class.decorators,
            r#abstract: class.r#abstract,
            type_parameters: class.type_parameters.as_ref(),
            super_class: view.opt(class.super_class),
            super_type_parameters: class.super_type_parameters.as_ref(),
            implements: class.implements,
            body: &class.body,
        }
    }

    fn expression(view: AstView<'n>, class: &'n ClassExpression<'a>) -> Self {
        Self {
            decorators: class.decorators,
            r#abstract: class.r#abstract,
            type_parameters: class.type_parameters.as_ref(),
            super_class: view.opt(class.super_class),
            super_type_parameters: class.super_type_parameters.as_ref(),
            implements: class.implements,
            body: &class.body,
        }
    }
}

/// How expression `e` ends when it closes a statement or field ending at `end`
/// (`Walk::open`). Its rightmost operand decides — unless a paren the parser stripped closes
/// it (its span ends short of `end`), which any continuation extends.
fn expression_tail(view: AstView<'_>, e: &Expression<'_>, end: u32) -> Tail {
    if e.span.end != end {
        return Tail::Open;
    }
    match &e.kind {
        ExpressionKind::UpdateExpression(update) if !update.prefix => Tail::Operand,
        ExpressionKind::UpdateExpression(update) => {
            expression_tail(view, view.expr(update.argument), end)
        }
        ExpressionKind::YieldExpression(e) => e.argument.map_or(Tail::Closed, |argument| {
            expression_tail(view, view.expr(argument), end)
        }),
        &ExpressionKind::ArrowFunctionExpression(arrow) => match &view.arrow_fn(arrow).body {
            ArrowFunctionBody::Expression(body) => expression_tail(view, view.expr(*body), end),
            ArrowFunctionBody::BlockStatement(_) => Tail::Closed,
        },
        ExpressionKind::BinaryExpression(binary) => {
            expression_tail(view, view.expr(binary.right), end)
        }
        ExpressionKind::AssignmentExpression(assignment) => {
            expression_tail(view, view.expr(assignment.right), end)
        }
        ExpressionKind::ConditionalExpression(conditional) => {
            expression_tail(view, view.expr(conditional.alternate), end)
        }
        ExpressionKind::SequenceExpression(sequence) => view
            .list(sequence.expressions)
            .last()
            .map_or(Tail::Open, |last| {
                expression_tail(view, view.expr(*last), end)
            }),
        ExpressionKind::UnaryExpression(unary) => {
            expression_tail(view, view.expr(unary.argument), end)
        }
        ExpressionKind::AwaitExpression(e) => expression_tail(view, view.expr(e.argument), end),
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
fn module_is_type_only(view: AstView<'_>, decl: &TSModuleDeclaration<'_>) -> bool {
    decl.global
        || matches!(decl.id, TSModuleName::Literal(_))
        || (decl.kind == TSModuleDeclarationKind::Namespace
            && (decl.declare || !namespace_holds_value(view, decl)))
}

/// Whether a `namespace`'s body holds a value (ts-blank-space's `valueNamespaceWorker`).
fn namespace_holds_value(view: AstView<'_>, decl: &TSModuleDeclaration<'_>) -> bool {
    match decl.body.as_ref() {
        None => false,
        Some(&TSModuleDeclarationBody::TSModuleDeclaration(inner)) => {
            let inner = view.module_decl(inner);
            nested_module_holds_value(view, inner)
        }
        Some(TSModuleDeclarationBody::TSModuleBlock(block)) => view
            .stmt_list(block.body)
            .iter()
            .any(|statement| namespace_member_holds_value(view, view.stmt(*statement))),
    }
}

/// A module nested in a namespace: one not spelled `namespace` (an ambient `module 'x'`,
/// `global`, a `module` keyword) counts as a value, as ts-blank-space counts it.
fn nested_module_holds_value(view: AstView<'_>, decl: &TSModuleDeclaration<'_>) -> bool {
    decl.global
        || decl.kind != TSModuleDeclarationKind::Namespace
        || namespace_holds_value(view, decl)
}

/// Whether one statement of a namespace body is a value: anything but a type alias, an
/// interface, a type-only nested namespace, or an unexported import-equals alias.
fn namespace_member_holds_value(view: AstView<'_>, statement: &Statement<'_>) -> bool {
    match &statement.kind {
        StatementKind::TSTypeAliasDeclaration(_) | StatementKind::TSInterfaceDeclaration(_) => {
            false
        }
        &StatementKind::TSModuleDeclaration(decl) => {
            nested_module_holds_value(view, view.module_decl(decl))
        }
        &StatementKind::TSImportEqualsDeclaration(decl) => view.import_equals(decl).is_export,
        &StatementKind::ExportNamedDeclaration(export) => {
            match view.opt_stmt(view.export_named(export).declaration) {
                Some(declaration) => match &declaration.kind {
                    StatementKind::TSTypeAliasDeclaration(_)
                    | StatementKind::TSInterfaceDeclaration(_) => false,
                    &StatementKind::TSModuleDeclaration(decl) => {
                        nested_module_holds_value(view, view.module_decl(decl))
                    }
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
            }
        }
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
