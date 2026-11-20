//! The unreachable-code (TS7027) + unused-label (TS7028) shim — a **fast-path**
//! reader of the flow product's `NODE_FLAGS_UNREACHABLE` bit.
//!
//! This is the first slice that consumes the flow product ([`crate::binder::flow`])
//! and emits diagnostics. It ports the binder-set-bit branch of tsgo's
//! `checkSourceElementUnreachable` / `isSourceElementUnreachable` and its
//! `checkLabeledStatement` unused-label check — **only** the branch that reads the
//! binder's `Unreachable` flag. The type-dependent fallback
//! (`isReachableFlowNode` — never-returning signatures, assertion predicates,
//! exhaustive switches) is `deferred_cfa`, out of scope.
//!
//! ## Two phases, split across bind and check
//!
//! The flag bit, the run grouping, and the const-enum / module-instance
//! classification are **all syntactic** (bind-time, variant-independent). So the
//! candidate table is built **once per unit** in `bind_program` while the AST +
//! flow product are alive ([`build_candidates`]) and stored owned in the
//! `BoundUnit` — keeping the `BoundProgram` C15-relocatable. Then per-variant
//! [`UnreachableCandidates::emit`] applies the option filter (which members count
//! as executable) and routes each surviving run — **error** into `diagnostics`
//! (only when the option is explicit-`False`), **suggestion** into a separate
//! `suggestions` sink (the default `Unknown`), which the conformance gate's
//! expect-clean channel never inspects.
//!
//! ## Grouping (tsgo checker.go:2394-2439, forward scan only)
//!
//! Within a statement list, a maximal run of consecutive candidates (each a
//! potentially-executable statement carrying the `Unreachable` bit) is recorded
//! once. At emit time the run is split into sub-runs at members that fail the
//! option filter (a `const enum` at `preserveConstEnums:false`, a non-instantiated
//! module), and one TS7027 is emitted per surviving sub-run spanning
//! `first.start → last.end`. A reported run's children are **not** descended for
//! more runs — tsgo's `withinUnreachableCode` / `reportedUnreachableNodes`
//! suppression, which here falls out of not descending into a candidate statement.
//
// tsgo: internal/checker/checker.go checkSourceElementUnreachable (2380-2439),
//       isSourceElementUnreachable (2441-2459), checkLabeledStatement (4190-4206),
//       errorOrSuggestion/addErrorOrSuggestion (13937-13957);
//       internal/ast/utilities.go GetModuleInstanceState / IsInstantiatedModule
//       (2294-2428), IsPotentiallyExecutableNode (4210).

use crate::binder::flow::FlowProduct;
use crate::binder::{
    BoundFile, NODE_FLAGS_UNREACHABLE, NodeKind, addr_of, debug_assert_stored_key, statement_kind,
};
use crate::diag::Diagnostic;
use crate::ids::{FileId, NodeId};
use crate::options::{CheckOptions, Tristate};
use smallvec::SmallVec;
use tsv_lang::{Comment, Span};
use tsv_ts::ast::{
    ArrowFunctionBody, AstView, ClassBody, ClassMember, Decorator, ExportDefaultValue, ExprId,
    Expression, ExpressionKind, ForInOfLeft, ForInit, ObjectPatternProperty, ObjectProperty,
    ParsedRef, Run, Statement, StatementKind, StmtId, TSModuleDeclaration, TSModuleDeclarationBody,
    VariableDeclaration,
};

/// A namespace body's instantiation classification — tsgo's
/// `ModuleInstanceState`, a **pure syntactic** fold of the body's declarations.
///
/// # tsgo
/// `internal/ast/utilities.go` `ModuleInstanceState`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModuleInstanceState {
    /// Only interfaces / type aliases / non-exported imports — no value emitted.
    NonInstantiated,
    /// Produces a value (functions, classes, non-const enums, value exports).
    Instantiated,
    /// Contains only `const enum`s — instantiated iff `preserveConstEnums`.
    ConstEnumOnly,
}

/// The per-candidate classification that decides whether an unreachable member is
/// reportable under the option filter (tsgo `isSourceElementUnreachable`'s switch).
#[derive(Clone, Copy, Debug)]
enum CandidateKind {
    /// Always reportable (class, plain statement, instantiated module, non-const
    /// enum).
    Plain,
    /// A (possibly const) enum: reportable iff `!is_const || preserveConstEnums`.
    Enum {
        /// Whether this is a `const enum`.
        is_const: bool,
    },
    /// A namespace/module: reportable iff `IsInstantiatedModule(state, preserve)`.
    Module {
        /// The syntactic instantiation state.
        state: ModuleInstanceState,
    },
}

/// One member of a contiguous unreachable run: its reportable span + its filter
/// classification.
#[derive(Clone, Copy, Debug)]
struct RunMember {
    span: Span,
    kind: CandidateKind,
}

/// A maximal contiguous run of unreachable candidates in one statement list
/// (variant-independent; the option filter splits it at emit time).
#[derive(Clone, Debug)]
struct CandidateRun {
    members: SmallVec<[RunMember; 4]>,
}

/// The owned, AST-free per-unit unreachable/unused-label candidate table, built
/// at bind time and consumed by [`UnreachableCandidates::emit`] per variant.
/// C15-relocatable by construction (spans are `Copy`; nothing borrows the AST).
#[derive(Clone, Debug, Default)]
pub(crate) struct UnreachableCandidates {
    /// Maximal unreachable-statement runs (TS7027 candidates).
    runs: Vec<CandidateRun>,
    /// Unreferenced-label identifier spans (TS7028 candidates), pre-order.
    unused_labels: Vec<Span>,
    /// Byte ranges of source lines suppressed by a preceding
    /// `@ts-ignore` / `@ts-expect-error` directive (source-fixed, so
    /// option-independent). A reachability diagnostic whose start falls in one is
    /// dropped, matching tsc's `getDiagnosticsWithPrecedingDirectives`.
    suppressed_ranges: Vec<(u32, u32)>,
}

impl UnreachableCandidates {
    /// Emit the reachability diagnostics for one unit under `options`, routing each
    /// to `diagnostics` (error category) or `suggestions` (suggestion category):
    /// TS7027 per surviving unreachable sub-run, TS7028 per unreferenced label.
    /// A `True` option skips its probe entirely; `False` → error; `Unknown` →
    /// suggestion.
    // `&CheckOptions` mirrors the `check_bound` threading (uniform + future-proof).
    #[expect(clippy::trivially_copy_pass_by_ref)]
    pub(crate) fn emit(
        &self,
        file: FileId,
        options: &CheckOptions,
        diagnostics: &mut Vec<Diagnostic>,
        suggestions: &mut Vec<Diagnostic>,
    ) {
        // TODO: tsgo's `getDiagnosticsWithPrecedingDirectives` filters ALL bind+check
        // diagnostics; tsv suppresses only this flow family (TS7027/7028), so a
        // `@ts-ignore`'d dup-family diagnostic still emits as a family extra. Widening
        // is deferred — the clean form is a final per-file pass over the concatenated
        // bind+check(+merge) diagnostics, which entangles with multi-file merge
        // cross-file attribution (a P3/multi-file concern).
        // TS7027 — unreachable code. Skip the probe entirely at `True`.
        if options.allow_unreachable_code != Tristate::True {
            let is_error = options.allow_unreachable_code == Tristate::False;
            let preserve = options.preserve_const_enums;
            for run in &self.runs {
                let mut i = 0;
                while i < run.members.len() {
                    // Skip a filtered-out member (splits the run — the const-enum /
                    // non-instantiated-module gap).
                    if !passes(run.members[i].kind, preserve) {
                        i += 1;
                        continue;
                    }
                    // Absorb the contiguous filter-passing sub-run.
                    let start = run.members[i].span.start;
                    let mut end = run.members[i].span.end;
                    let mut j = i + 1;
                    while j < run.members.len() && passes(run.members[j].kind, preserve) {
                        end = run.members[j].span.end;
                        j += 1;
                    }
                    // A directive (`@ts-ignore` / `@ts-expect-error`) preceding the
                    // sub-run's start line drops an ERROR-category diagnostic; tsgo's
                    // suggestion collection bypasses directive filtering
                    // (`getSuggestionDiagnosticsWithChecker`), so a suggestion is kept.
                    if !(is_error && self.is_suppressed(start)) {
                        push(
                            diagnostics,
                            suggestions,
                            is_error,
                            file,
                            Span::new(start, end),
                            7027,
                            "Unreachable code detected.",
                        );
                    }
                    i = j;
                }
            }
        }
        // TS7028 — unused label. One per unreferenced label identifier (no run
        // grouping); span = the label identifier alone.
        if options.allow_unused_labels != Tristate::True {
            let is_error = options.allow_unused_labels == Tristate::False;
            for &span in &self.unused_labels {
                // Error-category only; a suggestion bypasses directive suppression.
                if is_error && self.is_suppressed(span.start) {
                    continue;
                }
                push(
                    diagnostics,
                    suggestions,
                    is_error,
                    file,
                    span,
                    7028,
                    "Unused label.",
                );
            }
        }
    }

    /// Whether `start` (a diagnostic's start offset) falls on a directive-suppressed
    /// line. The range list is tiny (usually empty), so a linear scan is fine.
    fn is_suppressed(&self, start: u32) -> bool {
        self.suppressed_ranges
            .iter()
            .any(|&(s, e)| start >= s && start < e)
    }
}

/// Route a reachability diagnostic to the error sink or the suggestion sink.
fn push(
    diagnostics: &mut Vec<Diagnostic>,
    suggestions: &mut Vec<Diagnostic>,
    is_error: bool,
    file: FileId,
    span: Span,
    code: u32,
    message: &str,
) {
    if is_error {
        diagnostics.push(Diagnostic::error(file, span, code, message));
    } else {
        suggestions.push(Diagnostic::suggestion(file, span, code, message));
    }
}

/// Whether an unreachable member is reportable under `preserve_const_enums`
/// (tsgo `isSourceElementUnreachable`'s enum/module switch).
fn passes(kind: CandidateKind, preserve: bool) -> bool {
    match kind {
        CandidateKind::Plain => true,
        CandidateKind::Enum { is_const } => !is_const || preserve,
        CandidateKind::Module { state } => is_instantiated_module(state, preserve),
    }
}

/// `IsInstantiatedModule` (tsgo utilities.go:2422).
fn is_instantiated_module(state: ModuleInstanceState, preserve: bool) -> bool {
    matches!(state, ModuleInstanceState::Instantiated)
        || (preserve && matches!(state, ModuleInstanceState::ConstEnumOnly))
}

/// Build the per-unit candidate table from a parsed file's AST + the flow product
/// (the `Unreachable`-bit source). Called once per parsed unit in `bind_program`,
/// with the same `parsed` the `bound` file was bound from — the candidate lookups
/// resolve its `Statement` rows' addresses.
#[must_use]
pub(crate) fn build_candidates(
    parsed: ParsedRef<'_>,
    source: &str,
    bound: &BoundFile,
    flow: &FlowProduct,
) -> UnreachableCandidates {
    let ParsedRef { program, view } = parsed;
    let mut walk = CandidateWalk {
        view,
        bound,
        flow,
        runs: Vec::new(),
    };
    walk.visit_run(program.body);
    let unused_labels = collect_unused_labels(bound, flow);
    let suppressed_ranges = compute_suppressed_ranges(source, view.comments(program.comments));
    UnreachableCandidates {
        runs: walk.runs,
        unused_labels,
        suppressed_ranges,
    }
}

/// Compute the source lines suppressed by an `@ts-ignore` / `@ts-expect-error`
/// directive — tsc's `getDiagnosticsWithPrecedingDirectives`. A directive on line
/// `D` suppresses diagnostics on the following lines, scanning down through
/// blank/comment lines up to and including the first code line it protects. Empty
/// (the fast path) when the file has no directive comments.
fn compute_suppressed_ranges(source: &str, comments: &[Comment]) -> Vec<(u32, u32)> {
    // Key each directive off its comment's END offset: tsc attributes a directive
    // to its comment's LAST line (`lastLineStart`), so a multi-line
    // `/* … @ts-ignore */` protects the line after the `*/`, not after the `/*`. A
    // `//` directive can't span lines, so `end` == `start`'s line for those.
    let mut directive_ends: Vec<u32> = comments
        .iter()
        .filter(|c| is_directive_comment(c.content(source)))
        .map(|c| c.span.end)
        .collect();
    if directive_ends.is_empty() {
        return Vec::new();
    }
    directive_ends.sort_unstable();
    let bytes = source.as_bytes();
    let line_starts = compute_line_starts(source);
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for &directive_end in &directive_ends {
        let mut l = line_of(&line_starts, directive_end) + 1;
        while l < line_starts.len() {
            let line_start = line_starts[l];
            let line_end = line_starts
                .get(l + 1)
                .copied()
                .unwrap_or(source.len() as u32);
            ranges.push((line_start, line_end));
            if is_comment_or_blank_line(bytes, line_start as usize) {
                l += 1;
            } else {
                break; // the protected code line — stop
            }
        }
    }
    ranges.sort_unstable();
    ranges
}

/// Byte offsets of each line start (`[0, …after each '\n']`).
fn compute_line_starts(source: &str) -> Vec<u32> {
    let mut starts = vec![0u32];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            starts.push((i + 1) as u32);
        }
    }
    starts
}

/// The line index (0-based) containing `offset`.
fn line_of(line_starts: &[u32], offset: u32) -> usize {
    match line_starts.binary_search(&offset) {
        Ok(i) => i,
        // `line_starts[0] == 0 <= offset`, so the insertion point is `>= 1`.
        Err(i) => i - 1,
    }
}

/// `isCommentOrBlankLine` (tsgo program.go:1427) — a line that is whitespace-only
/// or begins (after indent) with `//`.
fn is_comment_or_blank_line(bytes: &[u8], line_start: usize) -> bool {
    let mut p = line_start;
    while p < bytes.len() && (bytes[p] == b' ' || bytes[p] == b'\t') {
        p += 1;
    }
    p == bytes.len()
        || bytes[p] == b'\r'
        || bytes[p] == b'\n'
        || (p + 1 < bytes.len() && bytes[p] == b'/' && bytes[p + 1] == b'/')
}

/// Whether a comment's (delimiter-stripped) content is an `@ts-ignore` /
/// `@ts-expect-error` directive (tsgo scanner.go `processCommentDirective`): skip
/// leading whitespace + extra `/`/`*`, then require `@ts-ignore` / `@ts-expect-error`.
fn is_directive_comment(content: &str) -> bool {
    let t = content
        .trim_start()
        .trim_start_matches(['/', '*'])
        .trim_start();
    t.strip_prefix('@')
        .is_some_and(|r| r.starts_with("ts-ignore") || r.starts_with("ts-expect-error"))
}

/// Scan the SoA columns for unreferenced-label identifiers: a node carrying the
/// `Unreachable` bit whose kind is `Identifier` and whose parent is a
/// `LabeledStatement` (the only `Identifier` child of a labeled statement is its
/// label). The bit is set on such a label only by `bindLabeledStatement` for an
/// unreferenced label, so this is exact and needs no traversal.
fn collect_unused_labels(bound: &BoundFile, flow: &FlowProduct) -> Vec<Span> {
    let mut out = Vec::new();
    for i in 0..bound.node_count as usize {
        if flow.node_flags[i] & NODE_FLAGS_UNREACHABLE == 0
            || bound.kinds[i] != NodeKind::Identifier
        {
            continue;
        }
        if let Some(parent) = bound.parents[i]
            && bound.kinds[parent.index()] == NodeKind::LabeledStatement
        {
            out.push(bound.spans[i]);
        }
    }
    out
}

/// The candidate-collection walk over the value structure of one file, and the view
/// it reads the tree through.
struct CandidateWalk<'a> {
    view: AstView<'a>,
    bound: &'a BoundFile,
    flow: &'a FlowProduct,
    runs: Vec<CandidateRun>,
}

impl<'a> CandidateWalk<'a> {
    /// The `NodeId` of a statement if it carries the `Unreachable` bit (a
    /// candidate). Only potentially-executable statements ever get the bit
    /// (`bindChildren`'s dead path gates on `IsPotentiallyExecutableNode`), so a
    /// bit-bearing statement *is* a candidate. A safe (non-panicking) address
    /// lookup — a miss (never expected) simply treats the statement as
    /// non-candidate. `stmt` is a `Statement` row read in place through the view —
    /// the row the SoA walk keyed — which the store-range guard asserts, since a
    /// copy here would silently drop the candidate.
    #[track_caller]
    fn candidate_id(&self, stmt: &'a Statement) -> Option<NodeId> {
        let address = addr_of(stmt);
        let kind = statement_kind(stmt);
        debug_assert_stored_key(self.view, address, kind);
        let id = self.bound.address_map.get(&(address, kind)).copied()?;
        (self.flow.node_flags[id.index()] & NODE_FLAGS_UNREACHABLE != 0).then_some(id)
    }

    /// Process a statement list: group maximal runs of consecutive candidates
    /// (recorded, not descended — the suppression), and descend every
    /// non-candidate statement to find dead code in its nested lists / bodies.
    fn visit_list(&mut self, stmts: &[StmtId]) {
        let view = self.view;
        let mut i = 0;
        while i < stmts.len() {
            let stmt = view.stmt(stmts[i]);
            let Some(id) = self.candidate_id(stmt) else {
                self.descend(stmt);
                i += 1;
                continue;
            };
            // Start a maximal run; a candidate's subtree is suppressed (not
            // descended), matching tsgo's `withinUnreachableCode`.
            let mut members: SmallVec<[RunMember; 4]> = SmallVec::new();
            members.push(RunMember {
                span: self.bound.spans[id.index()],
                kind: classify(view, stmt),
            });
            i += 1;
            while i < stmts.len() {
                let stmt = view.stmt(stmts[i]);
                let Some(id) = self.candidate_id(stmt) else {
                    break;
                };
                members.push(RunMember {
                    span: self.bound.spans[id.index()],
                    kind: classify(view, stmt),
                });
                i += 1;
            }
            self.runs.push(CandidateRun { members });
        }
    }

    /// Process a statement-list run (a body, a block, a case's consequent).
    fn visit_run(&mut self, stmts: Run<StmtId>) {
        self.visit_list(self.view.stmt_list(stmts));
    }

    /// Process a single nested statement position (an `if` arm, a loop or label
    /// body) as a one-statement list.
    fn visit_one(&mut self, stmt: StmtId) {
        self.visit_list(std::slice::from_ref(&stmt));
    }

    /// Descend a **non-candidate** statement's nested statement positions and value
    /// expressions (an embedded arrow/function/class body can hide dead code).
    fn descend(&mut self, stmt: &'a Statement) {
        use StatementKind as S;
        let view = self.view;
        match &stmt.kind {
            S::ExpressionStatement(s) => self.visit_expr(s.expression),
            S::VariableDeclaration(d) => self.visit_declarators(d),
            &S::FunctionDeclaration(f) => {
                let f = view.fn_decl(f);
                self.visit_params(f.params);
                self.visit_run(f.body.body);
            }
            &S::ClassDeclaration(c) => {
                let c = view.class_decl(c);
                self.visit_class(&c.body, c.super_class, c.decorators);
            }
            &S::TSEnumDeclaration(e) => {
                for m in view.enum_members(view.enum_decl(e).members) {
                    if let Some(init) = m.initializer {
                        self.visit_expr(init);
                    }
                }
            }
            &S::TSModuleDeclaration(m) => self.visit_module(view.module_decl(m)),
            S::ReturnStatement(s) => {
                if let Some(a) = s.argument {
                    self.visit_expr(a);
                }
            }
            S::ThrowStatement(s) => self.visit_expr(s.argument),
            S::BlockStatement(b) => self.visit_run(b.body),
            S::IfStatement(s) => {
                self.visit_expr(s.test);
                self.visit_one(s.consequent);
                if let Some(alt) = s.alternate {
                    self.visit_one(alt);
                }
            }
            S::ForStatement(s) => {
                match view.opt_for_head(s.init) {
                    Some(ForInit::VariableDeclaration(d)) => self.visit_declarators(d),
                    Some(&ForInit::Expression(e)) => self.visit_expr(e),
                    None => {}
                }
                if let Some(t) = s.test {
                    self.visit_expr(t);
                }
                if let Some(u) = s.update {
                    self.visit_expr(u);
                }
                self.visit_one(s.body);
            }
            S::ForInStatement(s) => {
                self.visit_for_left(view.for_left(s.left));
                self.visit_expr(s.right);
                self.visit_one(s.body);
            }
            S::ForOfStatement(s) => {
                self.visit_for_left(view.for_left(s.left));
                self.visit_expr(s.right);
                self.visit_one(s.body);
            }
            S::WhileStatement(s) => {
                self.visit_expr(s.test);
                self.visit_one(s.body);
            }
            S::DoWhileStatement(s) => {
                self.visit_one(s.body);
                self.visit_expr(s.test);
            }
            S::WithStatement(s) => {
                self.visit_expr(s.object);
                self.visit_one(s.body);
            }
            S::SwitchStatement(s) => {
                self.visit_expr(s.discriminant);
                for case in view.cases(s.cases) {
                    if let Some(t) = case.test {
                        self.visit_expr(t);
                    }
                    self.visit_run(case.consequent);
                }
            }
            &S::TryStatement(s) => {
                let s = view.try_stmt(s);
                self.visit_run(s.block.body);
                if let Some(h) = view.opt_catch(s.handler) {
                    if let Some(p) = &h.param {
                        self.visit_expression(p);
                    }
                    self.visit_run(h.body.body);
                }
                if let Some(f) = &s.finalizer {
                    self.visit_run(f.body);
                }
            }
            &S::LabeledStatement(s) => self.visit_one(view.labeled_stmt(s).body),
            &S::ExportNamedDeclaration(e) => {
                if let Some(inner) = view.export_named(e).declaration {
                    self.visit_one(inner);
                }
            }
            &S::ExportDefaultDeclaration(e) => {
                self.visit_export_default(&view.export_default(e).declaration);
            }
            &S::TSExportAssignment(ea) => self.visit_expr(view.export_assign(ea).expression),
            // No value body to descend for dead code.
            S::TSDeclareFunction(_)
            | S::TSInterfaceDeclaration(_)
            | S::TSTypeAliasDeclaration(_)
            | S::ExportAllDeclaration(_)
            | S::TSNamespaceExportDeclaration(_)
            | S::ImportDeclaration(_)
            | S::TSImportEqualsDeclaration(_)
            | S::BreakStatement(_)
            | S::ContinueStatement(_)
            | S::EmptyStatement(_)
            | S::DebuggerStatement(_) => {}
        }
    }

    fn visit_declarators(&mut self, decl: &'a VariableDeclaration) {
        for d in self.view.declarators(decl.declarations) {
            self.visit_expr(d.id);
            if let Some(init) = d.init {
                self.visit_expr(init);
            }
        }
    }

    fn visit_for_left(&mut self, left: &'a ForInOfLeft) {
        match left {
            ForInOfLeft::VariableDeclaration(d) => self.visit_declarators(d),
            &ForInOfLeft::Pattern(e) => self.visit_expr(e),
        }
    }

    fn visit_export_default(&mut self, v: &'a ExportDefaultValue) {
        use ExportDefaultValue as V;
        match v {
            &V::Expression(e) => self.visit_expr(e),
            V::FunctionDeclaration(f) => {
                self.visit_params(f.params);
                self.visit_run(f.body.body);
            }
            V::ClassDeclaration(c) => self.visit_class(&c.body, c.super_class, c.decorators),
            V::TSDeclareFunction(_) | V::TSInterfaceDeclaration(_) => {}
        }
    }

    fn visit_class(
        &mut self,
        body: &'a ClassBody,
        super_class: Option<ExprId>,
        decorators: Option<Run<Decorator>>,
    ) {
        let view = self.view;
        self.visit_decorators(decorators);
        if let Some(sc) = super_class {
            self.visit_expr(sc);
        }
        for member in view.class_members(body.body) {
            match member {
                ClassMember::MethodDefinition(m) => {
                    self.visit_decorators(m.decorators);
                    self.visit_params(m.value.params);
                    self.visit_run(m.value.body.body);
                }
                ClassMember::PropertyDefinition(p) => {
                    self.visit_decorators(p.decorators);
                    if let Some(v) = p.value {
                        self.visit_expr(v);
                    }
                }
                ClassMember::StaticBlock(s) => self.visit_run(s.body),
                ClassMember::IndexSignature(_) => {}
            }
        }
    }

    fn visit_module(&mut self, m: &'a TSModuleDeclaration) {
        match &m.body {
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => self.visit_run(block.body),
            &Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                self.visit_module(self.view.module_decl(nested));
            }
            None => {}
        }
    }

    fn visit_decorators(&mut self, decorators: Option<Run<Decorator>>) {
        if let Some(decs) = self.view.opt_decorators(decorators) {
            for d in decs {
                self.visit_expr(d.expression);
            }
        }
    }

    fn visit_params(&mut self, params: Run<Expression>) {
        for p in self.view.exprs(params) {
            self.visit_param(p);
        }
    }

    fn visit_param_id(&mut self, id: ExprId) {
        self.visit_param(self.view.expr(id));
    }

    fn visit_param(&mut self, param: &'a Expression) {
        use ExpressionKind as E;
        let view = self.view;
        match &param.kind {
            E::AssignmentPattern(a) => {
                self.visit_param_id(a.left);
                self.visit_expr(a.right);
            }
            E::ObjectPattern(op) => {
                for prop in view.pat_props(op.properties) {
                    match prop {
                        ObjectPatternProperty::Property(pr) => self.visit_param_id(pr.value),
                        ObjectPatternProperty::RestElement(r) => self.visit_param_id(r.argument),
                    }
                }
            }
            E::ArrayPattern(ap) => {
                for &el in view.expr_opt_list(ap.elements).iter().flatten() {
                    self.visit_param_id(el);
                }
            }
            E::RestElement(r) => self.visit_param_id(r.argument),
            E::TSParameterProperty(pp) => self.visit_param_id(pp.parameter),
            _ => {}
        }
    }

    /// Visit the expression `id` names, read in place from its store.
    fn visit_expr(&mut self, id: ExprId) {
        self.visit_expression(self.view.expr(id));
    }

    fn visit_expr_list(&mut self, list: Run<ExprId>) {
        for &e in self.view.expr_list(list) {
            self.visit_expr(e);
        }
    }

    /// Descend a value expression, looking for embedded function/arrow/class
    /// bodies (which hold their own statement lists / dead code).
    fn visit_expression(&mut self, expr: &'a Expression) {
        use ExpressionKind as E;
        let view = self.view;
        match &expr.kind {
            &E::FunctionExpression(f) => {
                let f = view.fn_expr(f);
                self.visit_params(f.params);
                self.visit_run(f.body.body);
            }
            &E::ArrowFunctionExpression(a) => {
                let a = view.arrow_fn(a);
                self.visit_params(a.params);
                match &a.body {
                    &ArrowFunctionBody::Expression(e) => self.visit_expr(e),
                    ArrowFunctionBody::BlockStatement(b) => self.visit_run(b.body),
                }
            }
            &E::ClassExpression(c) => {
                let c = view.class_expr(c);
                self.visit_class(&c.body, c.super_class, c.decorators);
            }
            E::TSAsExpression(t) => self.visit_expr(t.expression),
            E::TSSatisfiesExpression(t) => self.visit_expr(t.expression),
            E::TSTypeAssertion(t) => self.visit_expr(t.expression),
            E::TSInstantiationExpression(t) => self.visit_expr(t.expression),
            E::TSNonNullExpression(t) => self.visit_expr(t.expression),
            E::ParenthesizedExpression(p) => self.visit_expr(p.expression),
            &E::CommentBoundPair(c) => self.visit_expr(view.comment_pair(c).inner),
            E::UnaryExpression(u) => self.visit_expr(u.argument),
            E::UpdateExpression(u) => self.visit_expr(u.argument),
            E::AwaitExpression(a) => self.visit_expr(a.argument),
            E::YieldExpression(y) => {
                if let Some(a) = y.argument {
                    self.visit_expr(a);
                }
            }
            E::BinaryExpression(b) => {
                self.visit_expr(b.left);
                self.visit_expr(b.right);
            }
            E::AssignmentExpression(a) => {
                self.visit_expr(a.left);
                self.visit_expr(a.right);
            }
            E::ConditionalExpression(c) => {
                self.visit_expr(c.test);
                self.visit_expr(c.consequent);
                self.visit_expr(c.alternate);
            }
            E::SequenceExpression(s) => self.visit_expr_list(s.expressions),
            E::CallExpression(c) => {
                self.visit_expr(c.callee);
                self.visit_expr_list(c.arguments);
            }
            E::NewExpression(n) => {
                self.visit_expr(n.callee);
                self.visit_expr_list(n.arguments);
            }
            E::MemberExpression(m) => {
                self.visit_expr(m.object);
                self.visit_expr(m.property);
            }
            E::SpreadElement(s) => self.visit_expr(s.argument),
            E::ArrayExpression(a) => {
                for &e in view.expr_opt_list(a.elements).iter().flatten() {
                    self.visit_expr(e);
                }
            }
            E::ObjectExpression(o) => {
                for prop in view.obj_props(o.properties) {
                    match prop {
                        ObjectProperty::Property(pr) => {
                            self.visit_expr(pr.key);
                            self.visit_expr(pr.value);
                        }
                        ObjectProperty::SpreadElement(s) => self.visit_expr(s.argument),
                    }
                }
            }
            E::TemplateLiteral(t) => self.visit_expr_list(t.expressions),
            &E::TaggedTemplateExpression(t) => {
                let t = view.tagged_tpl(t);
                self.visit_expr(t.tag);
                self.visit_expr_list(t.quasi.expressions);
            }
            E::ImportExpression(i) => {
                self.visit_expr(i.source);
                if let Some(o) = i.options {
                    self.visit_expr(o);
                }
            }
            E::AssignmentPattern(a) => {
                self.visit_expr(a.left);
                self.visit_expr(a.right);
            }
            E::ObjectPattern(op) => {
                for prop in view.pat_props(op.properties) {
                    match prop {
                        ObjectPatternProperty::Property(pr) => {
                            self.visit_expr(pr.key);
                            self.visit_expr(pr.value);
                        }
                        ObjectPatternProperty::RestElement(r) => self.visit_expr(r.argument),
                    }
                }
            }
            E::ArrayPattern(ap) => {
                for &el in view.expr_opt_list(ap.elements).iter().flatten() {
                    self.visit_expr(el);
                }
            }
            E::RestElement(r) => self.visit_expr(r.argument),
            E::TSParameterProperty(pp) => self.visit_expr(pp.parameter),
            // Leaves (identifier / literal / this / super / meta / private / regex).
            _ => {}
        }
    }
}

/// Classify a candidate statement for the option filter.
fn classify(view: AstView<'_>, stmt: &Statement) -> CandidateKind {
    match &stmt.kind {
        StatementKind::TSEnumDeclaration(e) => CandidateKind::Enum {
            is_const: view.enum_decl(*e).r#const,
        },
        StatementKind::TSModuleDeclaration(m) => CandidateKind::Module {
            state: module_instance_state(view, view.module_decl(*m)),
        },
        _ => CandidateKind::Plain,
    }
}

/// `GetModuleInstanceState` (tsgo utilities.go:2302) — a namespace with no body
/// (`declare module 'x';`) is instantiated; otherwise fold its block.
fn module_instance_state(view: AstView<'_>, m: &TSModuleDeclaration) -> ModuleInstanceState {
    match &m.body {
        None => ModuleInstanceState::Instantiated,
        Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
            block_instance_state(view, view.stmt_list(block.body))
        }
        &Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
            module_instance_state(view, view.module_decl(nested))
        }
    }
}

/// The `ModuleBlock` fold of `getModuleInstanceStateWorker` (tsgo utilities.go:2363):
/// non-instantiated unless a member is const-enum-only (→ const-enum-only) or
/// instantiated (→ instantiated, short-circuit).
fn block_instance_state(view: AstView<'_>, stmts: &[StmtId]) -> ModuleInstanceState {
    let mut state = ModuleInstanceState::NonInstantiated;
    for &stmt in stmts {
        match statement_instance_state(view, view.stmt(stmt)) {
            ModuleInstanceState::NonInstantiated => {}
            ModuleInstanceState::ConstEnumOnly => state = ModuleInstanceState::ConstEnumOnly,
            ModuleInstanceState::Instantiated => return ModuleInstanceState::Instantiated,
        }
    }
    state
}

/// The per-statement classification of `getModuleInstanceStateWorker`'s switch.
fn statement_instance_state(view: AstView<'_>, stmt: &Statement) -> ModuleInstanceState {
    use StatementKind as S;
    match &stmt.kind {
        S::TSInterfaceDeclaration(_) | S::TSTypeAliasDeclaration(_) => {
            ModuleInstanceState::NonInstantiated
        }
        &S::TSEnumDeclaration(e) => {
            if view.enum_decl(e).r#const {
                ModuleInstanceState::ConstEnumOnly
            } else {
                ModuleInstanceState::Instantiated
            }
        }
        // A non-exported import/import-equals produces no value. tsv treats a bare
        // import as non-instantiated; the exported `export import x = …` form (tsgo:
        // instantiated) is a rare-in-dead-code residual, and under-reporting it is
        // safe (a missing, never an extra).
        S::ImportDeclaration(_) | S::TSImportEqualsDeclaration(_) => {
            ModuleInstanceState::NonInstantiated
        }
        &S::TSModuleDeclaration(nested) => module_instance_state(view, view.module_decl(nested)),
        &S::ExportNamedDeclaration(e) => match view.export_named(e).declaration {
            // `export interface` / `export const enum` / `export const` — classify
            // the wrapped declaration.
            Some(inner) => statement_instance_state(view, view.stmt(inner)),
            // Bare `export { … }` alias targets: a faithful resolution
            // (`getModuleInstanceStateForAliasTarget`) walks the enclosing scope per
            // name — a type-only re-export folds to NonInstantiated. tsv does not
            // resolve, so it takes the **NonInstantiated** default: under-reporting a
            // value re-export is safe (a missing), whereas assuming Instantiated
            // would over-report a type-only re-export as a false TS7027 extra (the
            // dangerous direction). The residual is a dead namespace whose only
            // member is a value re-export.
            None => ModuleInstanceState::NonInstantiated,
        },
        _ => ModuleInstanceState::Instantiated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binder::{bind_file, flow::build_flow};
    use crate::ids::FileId;
    use tsv_ts::ast::Parsed;

    fn parse(source: &str) -> Parsed {
        tsv_ts::__private::parse(source).expect("parse")
    }

    /// Emit both sinks under `options` and return them owned (the parse drops here
    /// — the candidate table and diagnostics borrow nothing from it).
    #[expect(clippy::trivially_copy_pass_by_ref)]
    fn emit_both(source: &str, opts: &CheckOptions) -> (Vec<Diagnostic>, Vec<Diagnostic>) {
        let parsed = parse(source);
        let bound = bind_file((&parsed).into(), source, FileId::ROOT);
        let flow = build_flow((&parsed).into(), source, &bound);
        let cands = build_candidates((&parsed).into(), source, &bound, &flow);
        let mut diags = Vec::new();
        let mut sugg = Vec::new();
        cands.emit(FileId::ROOT, opts, &mut diags, &mut sugg);
        (diags, sugg)
    }

    /// Emit TS7027/7028 as `(code, start, end)` (both sinks merged).
    fn emit_codes(
        source: &str,
        allow_unreachable: Tristate,
        allow_unused: Tristate,
        preserve: bool,
    ) -> Vec<(u32, u32, u32)> {
        let opts = CheckOptions {
            allow_unreachable_code: allow_unreachable,
            allow_unused_labels: allow_unused,
            preserve_const_enums: preserve,
        };
        let (diags, sugg) = emit_both(source, &opts);
        diags
            .iter()
            .chain(sugg.iter())
            .map(|d| (d.code, d.span.start, d.span.end))
            .collect()
    }

    #[test]
    fn contiguous_enum_run_merges_or_splits_by_preserve() {
        // `enum A` then `const enum B` after a return: one merged run when
        // preserving const enums, split (only A) otherwise.
        let src = "function f() { return; enum A { X } const enum B { Y } }";
        let merged = emit_codes(src, Tristate::False, Tristate::Unknown, true);
        assert_eq!(merged.len(), 1, "preserve merges the two into one run");
        let split = emit_codes(src, Tristate::False, Tristate::Unknown, false);
        assert_eq!(split.len(), 1, "no-preserve keeps only the non-const enum");
        // The merged span is strictly wider (absorbs `const enum B`).
        assert!(merged[0].2 > split[0].2);
    }

    #[test]
    fn lone_const_enum_reports_only_when_preserving() {
        let src = "function f() { return; const enum B { Y } }";
        assert_eq!(
            emit_codes(src, Tristate::False, Tristate::Unknown, false).len(),
            0
        );
        assert_eq!(
            emit_codes(src, Tristate::False, Tristate::Unknown, true).len(),
            1
        );
    }

    #[test]
    fn non_instantiated_module_never_reports() {
        // A dead namespace containing only a type is non-instantiated → no TS7027,
        // even preserving const enums.
        let src = "function f() { return; namespace N { interface I {} } }";
        assert_eq!(
            emit_codes(src, Tristate::False, Tristate::Unknown, false).len(),
            0
        );
        assert_eq!(
            emit_codes(src, Tristate::False, Tristate::Unknown, true).len(),
            0
        );
        // A namespace with a value export is instantiated → reports.
        let src2 = "function f() { return; namespace N { export const v = 1; } }";
        assert_eq!(
            emit_codes(src2, Tristate::False, Tristate::Unknown, false).len(),
            1
        );
    }

    #[test]
    fn midrun_const_enum_splits_run_into_two() {
        // `[plain, const enum, plain]` after a return: at preserve=false the const
        // enum fails the filter and SPLITS the run into two separate TS7027 (the
        // mid-run re-split path — a filtered gap in the middle of a run);
        // preserve=true merges all three into one.
        let src = "function f() { return; a; const enum B { Y } c; }";
        let split = emit_codes(src, Tristate::False, Tristate::Unknown, false);
        assert_eq!(
            split.len(),
            2,
            "the const enum splits the run into two TS7027"
        );
        assert!(split.iter().all(|d| d.0 == 7027));
        let merged = emit_codes(src, Tristate::False, Tristate::Unknown, true);
        assert_eq!(merged.len(), 1, "preserve merges all three into one run");
    }

    #[test]
    fn bare_export_alias_target_is_non_instantiated() {
        // Regression (F3 review): a dead namespace whose only members are a type
        // and a bare `export { … }` re-export must NOT over-report. tsv can't
        // resolve the alias, so it defaults to NonInstantiated — assuming
        // Instantiated would over-report a type-only re-export as a false TS7027
        // extra (the dangerous direction).
        let src = "function f() { return; namespace N { interface I {} export { I }; } }";
        assert_eq!(
            emit_codes(src, Tristate::False, Tristate::Unknown, false).len(),
            0
        );
        assert_eq!(
            emit_codes(src, Tristate::False, Tristate::Unknown, true).len(),
            0
        );
    }

    #[test]
    fn unused_label_span_is_the_identifier() {
        let src = "loop: while (true) { break; }";
        let out = emit_codes(src, Tristate::Unknown, Tristate::False, false);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, 7028);
        // Span covers `loop` (4 bytes at offset 0).
        assert_eq!((out[0].1, out[0].2), (0, 4));
    }

    #[test]
    fn suggestion_vs_error_routing() {
        let src = "function f() { return; foo(); }";
        // Unknown → suggestion sink; False → error sink; True → neither.
        for (opt, in_err, in_sugg) in [
            (Tristate::Unknown, 0, 1),
            (Tristate::False, 1, 0),
            (Tristate::True, 0, 0),
        ] {
            let opts = CheckOptions {
                allow_unreachable_code: opt,
                allow_unused_labels: Tristate::Unknown,
                preserve_const_enums: false,
            };
            let (diags, sugg) = emit_both(src, &opts);
            assert_eq!(diags.len(), in_err, "error sink for {opt:?}");
            assert_eq!(sugg.len(), in_sugg, "suggestion sink for {opt:?}");
        }
    }

    #[test]
    fn ts_ignore_directive_suppresses_unreachable() {
        // `@ts-ignore` / `@ts-expect-error` on the preceding line drops the TS7027
        // entirely (neither sink) — tsc's comment-directive suppression.
        let ignored =
            "function a() {\n\tthrow new Error('');\n\t// @ts-ignore\n\tconsole.log('x');\n}";
        assert_eq!(
            emit_codes(ignored, Tristate::False, Tristate::Unknown, false).len(),
            0
        );
        let expect =
            "function a() {\n\tthrow new Error('');\n\t// @ts-expect-error\n\tconsole.log('x');\n}";
        assert_eq!(
            emit_codes(expect, Tristate::False, Tristate::Unknown, false).len(),
            0
        );
        // A non-directive comment on the preceding line does NOT suppress.
        let plain =
            "function a() {\n\tthrow new Error('');\n\t// just a note\n\tconsole.log('x');\n}";
        assert_eq!(
            emit_codes(plain, Tristate::False, Tristate::Unknown, false).len(),
            1
        );
    }

    #[test]
    fn module_instance_state_classification() {
        // Interface/type only → non-instantiated; const enum only → const-enum-only;
        // a value → instantiated.
        let state_of = |src: &str| -> ModuleInstanceState {
            let parsed = parse(src);
            let view = parsed.view();
            match &view.stmt(view.stmt_list(parsed.program.body)[0]).kind {
                &StatementKind::TSModuleDeclaration(m) => {
                    module_instance_state(view, view.module_decl(m))
                }
                _ => panic!("expected a module"),
            }
        };
        assert_eq!(
            state_of("namespace N { interface I {} type T = number; }"),
            ModuleInstanceState::NonInstantiated
        );
        assert_eq!(
            state_of("namespace N { const enum E { A } }"),
            ModuleInstanceState::ConstEnumOnly
        );
        assert_eq!(
            state_of("namespace N { export function f() {} }"),
            ModuleInstanceState::Instantiated
        );
    }
}
