//! Byte→UTF-16 offset translation as a mutating walk over the typed public AST.
//!
//! Counterpart to `translate_byte_to_char_offsets` (the `serde_json::Value`
//! walk in `convert/mod.rs`): same translation semantics, but applied to the
//! typed tree so `convert_ast_json_string` can serialize multibyte sources
//! directly — no intermediate `Value` materialization on the wire hot path.
//!
//! Parity contract: output must be byte-identical to the `Value` walk. The
//! rules ported from there:
//!
//! - `start`/`end` translate via `ByteToCharMap` (UTF-16 code units, matching
//!   acorn).
//! - `loc.start`/`loc.end` columns translate delta-preserving: compute the
//!   expected byte column from the node's *original* byte offset, carry any
//!   pre-existing difference (e.g. +1 from Svelte's
//!   `adjust_read_pattern_columns`) onto the char column.
//! - A position's `character` field, when present, becomes the absolute
//!   UTF-16 offset.
//! - `Literal.value` / `RegexLiteral.value` are position-free leaves — not
//!   walked (the `Value` walk recurses into them but finds no `start`/`end`).
//! - `TSTypeParameterExtra.trailing_comma` translates like `start`/`end` —
//!   acorn emits it in UTF-16 code units (the `Value` walk translates the
//!   `extra.trailingComma` key the same way).
//!
//! Every struct with positions must be visited and every node-bearing field
//! recursed into; a missed field means silently untranslated offsets. Gates:
//! the fixture suite's string-path identity check plus its typed-walk parity
//! probes (`fixtures_validate` — synthesized multibyte variants and extracted
//! `<script>` contents give every fixture's AST shapes parity coverage) and
//! `json_profile`'s per-file `direct == value` comparison over the corpus.

use tsv_lang::{ByteToCharMap, LocationTracker};

use super::super::public::*;

/// Translate all byte-based positions in a typed public AST to UTF-16
/// code-unit positions, in place.
///
/// For ASCII-only sources this is a no-op (byte == UTF-16 offset).
pub fn translate_byte_to_char_offsets_typed(
    program: &mut Program,
    map: &ByteToCharMap,
    tracker: &LocationTracker,
) {
    if !map.has_multibyte() {
        return;
    }
    let t = Translator { map, tracker };
    t.program(program);
}

struct Translator<'a> {
    map: &'a ByteToCharMap,
    tracker: &'a LocationTracker,
}

/// Translate a node's `start`/`end`/`loc` triple in one call.
macro_rules! span {
    ($self:ident, $node:expr) => {
        $self.translate_span(&mut $node.start, &mut $node.end, &mut $node.loc)
    };
}

/// `ClassDeclaration` and `ClassExpression` share the same position-bearing
/// field shape; one macro body serves both visitors so a new field can't be
/// added to one and silently missed in the other.
macro_rules! class_visitor {
    ($name:ident, $ty:ty) => {
        fn $name(&self, n: &mut $ty) {
            span!(self, n);
            self.decorators_opt(n.decorators.as_mut());
            if let Some(id) = &mut n.id {
                self.identifier(id);
            }
            self.type_parameter_declaration_opt(n.type_parameters.as_mut());
            if let Some(super_class) = &mut n.super_class {
                self.expression(super_class);
            }
            self.type_parameter_instantiation_opt(n.super_type_parameters.as_mut());
            self.implements_opt(n.implements.as_mut());
            self.class_body(&mut n.body);
        }
    };
}

/// Same for `FunctionDeclaration`/`FunctionExpression`.
macro_rules! function_visitor {
    ($name:ident, $ty:ty) => {
        fn $name(&self, n: &mut $ty) {
            span!(self, n);
            if let Some(id) = &mut n.id {
                self.identifier(id);
            }
            self.type_parameter_declaration_opt(n.type_parameters.as_mut());
            for p in &mut n.params {
                self.expression(p);
            }
            self.type_annotation_opt(n.return_type.as_mut());
            self.block_statement(&mut n.body);
        }
    };
}

impl Translator<'_> {
    /// Translate `start`/`end` and both `loc` positions. The `loc` columns are
    /// computed from the *original* byte offsets, so they're captured before
    /// `start`/`end` are overwritten (matching the `Value` walk's
    /// read-before-mutate order).
    fn translate_span(&self, start: &mut u32, end: &mut u32, loc: &mut SourceLocation) {
        let orig_start = *start;
        let orig_end = *end;
        *start = self.map.byte_to_char(orig_start);
        *end = self.map.byte_to_char(orig_end);
        self.translate_position(&mut loc.start, orig_start);
        self.translate_position(&mut loc.end, orig_end);
    }

    /// Translate a `loc` position from byte-based to UTF-16-based, preserving
    /// any prior column adjustment — delegates the delta-preserving column
    /// math to the `Value` walk's `translate_column` so it exists once.
    #[allow(clippy::cast_possible_truncation)]
    fn translate_position(&self, pos: &mut Position, byte_offset: u32) {
        pos.column = super::translate_column(byte_offset, pos.column as u64, self.map, self.tracker)
            as usize;
        if pos.character.is_some() {
            pos.character = Some(self.map.byte_to_char(byte_offset));
        }
    }

    fn program(&self, n: &mut Program) {
        span!(self, n);
        for s in &mut n.body {
            self.statement(s);
        }
    }

    //
    // Statements
    //

    fn statement(&self, s: &mut Statement) {
        match s {
            Statement::ExpressionStatement(n) => {
                span!(self, n);
                self.expression(&mut n.expression);
            }
            Statement::VariableDeclaration(n) => self.variable_declaration(n),
            Statement::TSTypeAliasDeclaration(n) => {
                span!(self, n);
                self.identifier(&mut n.id);
                self.type_parameter_declaration_opt(n.type_parameters.as_mut());
                self.ts_type(&mut n.type_annotation);
            }
            Statement::TSInterfaceDeclaration(n) => {
                span!(self, n);
                self.identifier(&mut n.id);
                self.type_parameter_declaration_opt(n.type_parameters.as_mut());
                for h in &mut n.extends {
                    span!(self, h);
                    self.entity_name(&mut h.expression);
                    self.type_parameter_instantiation_opt(h.type_parameters.as_mut());
                }
                span!(self, n.body);
                for m in &mut n.body.body {
                    self.type_element(m);
                }
            }
            Statement::TSDeclareFunction(n) => self.declare_function(n),
            Statement::TSEnumDeclaration(n) => {
                span!(self, n);
                self.identifier(&mut n.id);
                for m in &mut n.members {
                    span!(self, m);
                    match &mut m.id {
                        TSEnumMemberId::Identifier(id) => self.identifier(id),
                        TSEnumMemberId::Literal(lit) => span!(self, lit),
                    }
                    if let Some(init) = &mut m.initializer {
                        self.expression(init);
                    }
                }
            }
            Statement::TSModuleDeclaration(n) => self.module_declaration(n),
            Statement::ReturnStatement(n) => {
                span!(self, n);
                if let Some(arg) = &mut n.argument {
                    self.expression(arg);
                }
            }
            Statement::BlockStatement(n) => self.block_statement(n),
            Statement::FunctionDeclaration(n) => self.function_declaration(n),
            Statement::ClassDeclaration(n) => self.class_declaration(n),
            Statement::ExportNamedDeclaration(n) => {
                span!(self, n);
                if let Some(decl) = &mut n.declaration {
                    self.statement(decl);
                }
                for spec in &mut n.specifiers {
                    span!(self, spec);
                    self.module_export_name(&mut spec.local);
                    self.module_export_name(&mut spec.exported);
                }
                if let Some(source) = &mut n.source {
                    span!(self, source);
                }
                self.import_attributes_opt(n.attributes.as_mut());
            }
            Statement::ExportDefaultDeclaration(n) => {
                span!(self, n);
                match &mut n.declaration {
                    ExportDefaultValue::Expression(e) => self.expression(e),
                    ExportDefaultValue::FunctionDeclaration(f) => self.function_declaration(f),
                    ExportDefaultValue::TSDeclareFunction(f) => self.declare_function(f),
                    ExportDefaultValue::ClassDeclaration(c) => self.class_declaration(c),
                }
            }
            Statement::ExportAllDeclaration(n) => {
                span!(self, n);
                if let Some(exported) = &mut n.exported {
                    self.module_export_name(exported);
                }
                span!(self, n.source);
                self.import_attributes_opt(n.attributes.as_mut());
            }
            Statement::TSExportAssignment(n) => {
                span!(self, n);
                self.expression(&mut n.expression);
            }
            Statement::ImportDeclaration(n) => {
                span!(self, n);
                for spec in &mut n.specifiers {
                    match spec {
                        ImportSpecifier::Default(s) => {
                            span!(self, s);
                            self.identifier(&mut s.local);
                        }
                        ImportSpecifier::Named(s) => {
                            span!(self, s);
                            self.module_export_name(&mut s.imported);
                            self.identifier(&mut s.local);
                        }
                        ImportSpecifier::Namespace(s) => {
                            span!(self, s);
                            self.identifier(&mut s.local);
                        }
                    }
                }
                span!(self, n.source);
                self.import_attributes_opt(n.attributes.as_mut());
            }
            Statement::TSImportEqualsDeclaration(n) => {
                span!(self, n);
                self.identifier(&mut n.id);
                match &mut n.module_reference {
                    TSModuleReference::ExternalModuleReference(r) => {
                        span!(self, r);
                        span!(self, r.expression);
                    }
                    TSModuleReference::EntityName(name) => self.entity_name(name),
                }
            }
            Statement::IfStatement(n) => {
                span!(self, n);
                self.expression(&mut n.test);
                self.statement(&mut n.consequent);
                if let Some(alt) = &mut n.alternate {
                    self.statement(alt);
                }
            }
            Statement::ForStatement(n) => {
                span!(self, n);
                match &mut n.init {
                    Some(ForInit::VariableDeclaration(d)) => self.variable_declaration(d),
                    Some(ForInit::Expression(e)) => self.expression(e),
                    None => {}
                }
                if let Some(test) = &mut n.test {
                    self.expression(test);
                }
                if let Some(update) = &mut n.update {
                    self.expression(update);
                }
                self.statement(&mut n.body);
            }
            Statement::ForInStatement(n) => {
                span!(self, n);
                self.for_in_of_left(&mut n.left);
                self.expression(&mut n.right);
                self.statement(&mut n.body);
            }
            Statement::ForOfStatement(n) => {
                span!(self, n);
                self.for_in_of_left(&mut n.left);
                self.expression(&mut n.right);
                self.statement(&mut n.body);
            }
            Statement::WhileStatement(n) => {
                span!(self, n);
                self.expression(&mut n.test);
                self.statement(&mut n.body);
            }
            Statement::DoWhileStatement(n) => {
                span!(self, n);
                self.statement(&mut n.body);
                self.expression(&mut n.test);
            }
            Statement::SwitchStatement(n) => {
                span!(self, n);
                self.expression(&mut n.discriminant);
                for case in &mut n.cases {
                    span!(self, case);
                    if let Some(test) = &mut case.test {
                        self.expression(test);
                    }
                    for s in &mut case.consequent {
                        self.statement(s);
                    }
                }
            }
            Statement::TryStatement(n) => {
                span!(self, n);
                self.block_statement(&mut n.block);
                if let Some(handler) = &mut n.handler {
                    span!(self, handler);
                    if let Some(param) = &mut handler.param {
                        self.expression(param);
                    }
                    self.block_statement(&mut handler.body);
                }
                if let Some(finalizer) = &mut n.finalizer {
                    self.block_statement(finalizer);
                }
            }
            Statement::ThrowStatement(n) => {
                span!(self, n);
                self.expression(&mut n.argument);
            }
            Statement::BreakStatement(n) => {
                span!(self, n);
                if let Some(label) = &mut n.label {
                    self.identifier(label);
                }
            }
            Statement::ContinueStatement(n) => {
                span!(self, n);
                if let Some(label) = &mut n.label {
                    self.identifier(label);
                }
            }
            Statement::LabeledStatement(n) => {
                span!(self, n);
                self.identifier(&mut n.label);
                self.statement(&mut n.body);
            }
            Statement::EmptyStatement(n) => span!(self, n),
            Statement::DebuggerStatement(n) => span!(self, n),
        }
    }

    fn block_statement(&self, n: &mut BlockStatement) {
        span!(self, n);
        for s in &mut n.body {
            self.statement(s);
        }
    }

    fn for_in_of_left(&self, left: &mut ForInOfLeft) {
        match left {
            ForInOfLeft::VariableDeclaration(d) => self.variable_declaration(d),
            ForInOfLeft::Pattern(e) => self.expression(e),
        }
    }

    fn variable_declaration(&self, n: &mut VariableDeclaration) {
        span!(self, n);
        for d in &mut n.declarations {
            span!(self, d);
            self.expression(&mut d.id);
            if let Some(init) = &mut d.init {
                self.expression(init);
            }
        }
    }

    function_visitor!(function_declaration, FunctionDeclaration);
    function_visitor!(function_expression, FunctionExpression);

    fn declare_function(&self, n: &mut TSDeclareFunction) {
        span!(self, n);
        self.identifier(&mut n.id);
        self.type_parameter_declaration_opt(n.type_parameters.as_mut());
        for p in &mut n.params {
            self.expression(p);
        }
        self.type_annotation_opt(n.return_type.as_mut());
    }

    fn module_declaration(&self, n: &mut TSModuleDeclaration) {
        span!(self, n);
        match &mut n.id {
            TSModuleName::Identifier(id) => self.identifier(id),
            TSModuleName::Literal(lit) => span!(self, lit),
        }
        match &mut n.body {
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                span!(self, block);
                for s in &mut block.body {
                    self.statement(s);
                }
            }
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                self.module_declaration(nested);
            }
            None => {}
        }
    }

    //
    // Classes
    //

    class_visitor!(class_declaration, ClassDeclaration);
    class_visitor!(class_expression, ClassExpression);

    fn class_body(&self, n: &mut ClassBody) {
        span!(self, n);
        for member in &mut n.body {
            match member {
                ClassMember::MethodDefinition(m) => {
                    span!(self, m);
                    self.decorators_opt(m.decorators.as_mut());
                    self.expression(&mut m.key);
                    self.type_parameter_declaration_opt(m.type_parameters.as_mut());
                    match &mut m.value {
                        MethodValue::FunctionExpression(f) => self.function_expression(f),
                        MethodValue::TSDeclareMethod(f) => {
                            span!(self, f);
                            if let Some(id) = &mut f.id {
                                self.identifier(id);
                            }
                            for p in &mut f.params {
                                self.expression(p);
                            }
                            self.type_annotation_opt(f.return_type.as_mut());
                        }
                    }
                }
                ClassMember::PropertyDefinition(p) => {
                    span!(self, p);
                    self.decorators_opt(p.decorators.as_mut());
                    self.expression(&mut p.key);
                    self.type_annotation_opt(p.type_annotation.as_mut());
                    if let Some(value) = &mut p.value {
                        self.expression(value);
                    }
                }
                ClassMember::StaticBlock(b) => {
                    span!(self, b);
                    for s in &mut b.body {
                        self.statement(s);
                    }
                }
                ClassMember::TSIndexSignature(sig) => self.index_signature(sig),
            }
        }
    }

    fn decorators_opt(&self, decorators: Option<&mut Vec<Decorator>>) {
        if let Some(decorators) = decorators {
            for d in decorators {
                span!(self, d);
                self.expression(&mut d.expression);
            }
        }
    }

    fn implements_opt(&self, implements: Option<&mut Vec<TSExpressionWithTypeArguments>>) {
        if let Some(implements) = implements {
            for i in implements {
                span!(self, i);
                self.expression(&mut i.expression);
                self.type_parameter_instantiation_opt(i.type_parameters.as_mut());
            }
        }
    }

    fn module_export_name(&self, name: &mut ModuleExportName) {
        match name {
            ModuleExportName::Identifier(id) => self.identifier(id),
            ModuleExportName::Literal(lit) => span!(self, lit),
        }
    }

    fn import_attributes_opt(&self, attributes: Option<&mut Vec<ImportAttribute>>) {
        if let Some(attributes) = attributes {
            for a in attributes {
                span!(self, a);
                match &mut a.key {
                    ImportAttributeKey::Identifier(id) => self.identifier(id),
                    ImportAttributeKey::Literal(lit) => span!(self, lit),
                }
                span!(self, a.value);
            }
        }
    }

    //
    // Expressions
    //

    fn identifier(&self, n: &mut Identifier) {
        span!(self, n);
        self.type_annotation_opt(n.type_annotation.as_mut());
        for d in &mut n.decorators {
            span!(self, d);
            self.expression(&mut d.expression);
        }
    }

    fn expression(&self, e: &mut Expression) {
        match e {
            Expression::Literal(n) => span!(self, n),
            Expression::Identifier(n) => self.identifier(n),
            Expression::PrivateIdentifier(n) => span!(self, n),
            Expression::ObjectExpression(n) => {
                span!(self, n);
                for prop in &mut n.properties {
                    match prop {
                        ObjectProperty::Property(p) => self.property(p),
                        ObjectProperty::SpreadElement(s) => {
                            span!(self, s);
                            self.expression(&mut s.argument);
                        }
                    }
                }
            }
            Expression::ArrayExpression(n) => {
                span!(self, n);
                for element in n.elements.iter_mut().flatten() {
                    self.expression(element);
                }
            }
            Expression::UnaryExpression(n) => self.unary_expression(n),
            Expression::UpdateExpression(n) => {
                span!(self, n);
                self.expression(&mut n.argument);
            }
            Expression::BinaryExpression(n) => {
                span!(self, n);
                self.expression(&mut n.left);
                self.expression(&mut n.right);
            }
            Expression::CallExpression(n) => {
                span!(self, n);
                self.expression(&mut n.callee);
                for arg in &mut n.arguments {
                    self.expression(arg);
                }
                self.type_parameter_instantiation_opt(n.type_arguments.as_mut());
            }
            Expression::NewExpression(n) => {
                span!(self, n);
                self.expression(&mut n.callee);
                for arg in &mut n.arguments {
                    self.expression(arg);
                }
                self.type_parameter_instantiation_opt(n.type_arguments.as_mut());
            }
            Expression::MemberExpression(n) => {
                span!(self, n);
                self.expression(&mut n.object);
                self.expression(&mut n.property);
            }
            Expression::ConditionalExpression(n) => {
                span!(self, n);
                self.expression(&mut n.test);
                self.expression(&mut n.consequent);
                self.expression(&mut n.alternate);
            }
            Expression::ArrowFunctionExpression(n) => {
                span!(self, n);
                for p in &mut n.params {
                    self.expression(p);
                }
                match &mut n.body {
                    ArrowFunctionBody::Expression(e) => self.expression(e),
                    ArrowFunctionBody::BlockStatement(b) => self.block_statement(b),
                }
                self.type_parameter_declaration_opt(n.type_parameters.as_mut());
                self.type_annotation_opt(n.return_type.as_mut());
            }
            Expression::FunctionExpression(n) => self.function_expression(n),
            Expression::ClassExpression(n) => self.class_expression(n),
            Expression::SpreadElement(n) => {
                span!(self, n);
                self.expression(&mut n.argument);
            }
            Expression::TemplateLiteral(n) => self.template_literal(n),
            Expression::TaggedTemplateExpression(n) => {
                span!(self, n);
                self.expression(&mut n.tag);
                self.template_literal(&mut n.quasi);
                self.type_parameter_instantiation_opt(n.type_arguments.as_mut());
            }
            Expression::AwaitExpression(n) => {
                span!(self, n);
                self.expression(&mut n.argument);
            }
            Expression::YieldExpression(n) => {
                span!(self, n);
                if let Some(arg) = &mut n.argument {
                    self.expression(arg);
                }
            }
            Expression::SequenceExpression(n) => {
                span!(self, n);
                for e in &mut n.expressions {
                    self.expression(e);
                }
            }
            Expression::RegexLiteral(n) => span!(self, n),
            Expression::ThisExpression(n) => span!(self, n),
            Expression::Super(n) => span!(self, n),
            Expression::AssignmentExpression(n) => {
                span!(self, n);
                self.expression(&mut n.left);
                self.expression(&mut n.right);
            }
            Expression::ObjectPattern(n) => {
                span!(self, n);
                for prop in &mut n.properties {
                    match prop {
                        ObjectPatternProperty::Property(p) => self.property(p),
                        ObjectPatternProperty::RestElement(r) => self.rest_element(r),
                    }
                }
                self.type_annotation_opt(n.type_annotation.as_mut());
            }
            Expression::ArrayPattern(n) => {
                span!(self, n);
                for element in n.elements.iter_mut().flatten() {
                    self.expression(element);
                }
                self.type_annotation_opt(n.type_annotation.as_mut());
            }
            Expression::AssignmentPattern(n) => {
                span!(self, n);
                self.expression(&mut n.left);
                self.expression(&mut n.right);
            }
            Expression::RestElement(n) => self.rest_element(n),
            Expression::TSTypeAssertion(n) => {
                span!(self, n);
                self.ts_type(&mut n.type_annotation);
                self.expression(&mut n.expression);
            }
            Expression::TSAsExpression(n) => {
                span!(self, n);
                self.expression(&mut n.expression);
                self.ts_type(&mut n.type_annotation);
            }
            Expression::TSSatisfiesExpression(n) => {
                span!(self, n);
                self.expression(&mut n.expression);
                self.ts_type(&mut n.type_annotation);
            }
            Expression::TSInstantiationExpression(n) => {
                span!(self, n);
                self.expression(&mut n.expression);
                self.type_parameter_instantiation(&mut n.type_arguments);
            }
            Expression::TSNonNullExpression(n) => {
                span!(self, n);
                self.expression(&mut n.expression);
            }
            Expression::ImportExpression(n) => {
                span!(self, n);
                self.expression(&mut n.source);
                for arg in &mut n.arguments {
                    self.expression(arg);
                }
            }
            Expression::MetaProperty(n) => {
                span!(self, n);
                self.identifier(&mut n.meta);
                self.identifier(&mut n.property);
            }
            Expression::TSParameterProperty(n) => {
                span!(self, n);
                self.expression(&mut n.parameter);
            }
            Expression::ChainExpression(n) => {
                span!(self, n);
                self.expression(&mut n.expression);
            }
        }
    }

    fn unary_expression(&self, n: &mut UnaryExpression) {
        span!(self, n);
        self.expression(&mut n.argument);
    }

    fn property(&self, n: &mut Property) {
        span!(self, n);
        self.expression(&mut n.key);
        self.expression(&mut n.value);
    }

    fn rest_element(&self, n: &mut RestElement) {
        span!(self, n);
        self.expression(&mut n.argument);
        self.type_annotation_opt(n.type_annotation.as_mut());
    }

    fn template_literal(&self, n: &mut TemplateLiteral) {
        span!(self, n);
        for e in &mut n.expressions {
            self.expression(e);
        }
        for q in &mut n.quasis {
            span!(self, q);
        }
    }

    //
    // Types
    //

    fn type_annotation(&self, n: &mut TSTypeAnnotation) {
        span!(self, n);
        self.ts_type(&mut n.type_annotation);
    }

    fn type_annotation_opt(&self, annotation: Option<&mut TSTypeAnnotation>) {
        if let Some(annotation) = annotation {
            self.type_annotation(annotation);
        }
    }

    fn type_parameter_declaration_opt(&self, decl: Option<&mut TSTypeParameterDeclaration>) {
        if let Some(decl) = decl {
            span!(self, decl);
            if let Some(extra) = &mut decl.extra {
                extra.trailing_comma = self.map.byte_to_char(extra.trailing_comma);
            }
            for p in &mut decl.params {
                self.type_parameter(p);
            }
        }
    }

    fn type_parameter(&self, n: &mut TSTypeParameter) {
        span!(self, n);
        if let Some(constraint) = &mut n.constraint {
            self.ts_type(constraint);
        }
        if let Some(default) = &mut n.default {
            self.ts_type(default);
        }
    }

    fn type_parameter_instantiation(&self, n: &mut TSTypeParameterInstantiation) {
        span!(self, n);
        for t in &mut n.params {
            self.ts_type(t);
        }
    }

    fn type_parameter_instantiation_opt(&self, inst: Option<&mut TSTypeParameterInstantiation>) {
        if let Some(inst) = inst {
            self.type_parameter_instantiation(inst);
        }
    }

    fn entity_name(&self, name: &mut TSEntityName) {
        match name {
            TSEntityName::Identifier(id) => self.identifier(id),
            TSEntityName::QualifiedName(q) => self.qualified_name(q),
        }
    }

    fn qualified_name(&self, n: &mut TSQualifiedName) {
        span!(self, n);
        self.entity_name(&mut n.left);
        self.identifier(&mut n.right);
    }

    fn type_element(&self, element: &mut TSTypeElement) {
        match element {
            TSTypeElement::PropertySignature(n) => {
                span!(self, n);
                self.expression(&mut n.key);
                self.type_annotation_opt(n.type_annotation.as_mut());
            }
            TSTypeElement::MethodSignature(n) => {
                span!(self, n);
                self.expression(&mut n.key);
                self.type_parameter_declaration_opt(n.type_parameters.as_mut());
                for p in &mut n.parameters {
                    self.expression(p);
                }
                self.type_annotation_opt(n.return_type.as_mut());
            }
            TSTypeElement::CallSignature(n) => {
                span!(self, n);
                self.type_parameter_declaration_opt(n.type_parameters.as_mut());
                for p in &mut n.params {
                    self.expression(p);
                }
                self.type_annotation_opt(n.return_type.as_mut());
            }
            TSTypeElement::ConstructSignature(n) => {
                span!(self, n);
                self.type_parameter_declaration_opt(n.type_parameters.as_mut());
                for p in &mut n.params {
                    self.expression(p);
                }
                self.type_annotation_opt(n.return_type.as_mut());
            }
            TSTypeElement::IndexSignature(n) => self.index_signature(n),
        }
    }

    fn index_signature(&self, n: &mut TSIndexSignature) {
        span!(self, n);
        for p in &mut n.parameters {
            self.identifier(p);
        }
        self.type_annotation(&mut n.type_annotation);
    }

    #[allow(clippy::too_many_lines)]
    fn ts_type(&self, t: &mut TSType) {
        match t {
            TSType::TSNumberKeyword(n) => span!(self, n),
            TSType::TSStringKeyword(n) => span!(self, n),
            TSType::TSBooleanKeyword(n) => span!(self, n),
            TSType::TSAnyKeyword(n) => span!(self, n),
            TSType::TSVoidKeyword(n) => span!(self, n),
            TSType::TSUndefinedKeyword(n) => span!(self, n),
            TSType::TSNullKeyword(n) => span!(self, n),
            TSType::TSNeverKeyword(n) => span!(self, n),
            TSType::TSUnknownKeyword(n) => span!(self, n),
            TSType::TSObjectKeyword(n) => span!(self, n),
            TSType::TSSymbolKeyword(n) => span!(self, n),
            TSType::TSBigIntKeyword(n) => span!(self, n),
            TSType::TSThisType(n) => span!(self, n),
            TSType::TSLiteralType(n) => {
                span!(self, n);
                match &mut n.literal {
                    TSLiteralTypeLiteral::TemplateLiteral(tpl) => {
                        span!(self, tpl);
                        for e in &mut tpl.expressions {
                            self.ts_type(e);
                        }
                        for q in &mut tpl.quasis {
                            span!(self, q);
                        }
                    }
                    TSLiteralTypeLiteral::UnaryExpression(u) => self.unary_expression(u),
                    TSLiteralTypeLiteral::Literal(lit) => span!(self, lit),
                }
            }
            TSType::TSArrayType(n) => {
                span!(self, n);
                self.ts_type(&mut n.element_type);
            }
            TSType::TSUnionType(n) => {
                span!(self, n);
                for t in &mut n.types {
                    self.ts_type(t);
                }
            }
            TSType::TSIntersectionType(n) => {
                span!(self, n);
                for t in &mut n.types {
                    self.ts_type(t);
                }
            }
            TSType::TSTypeReference(n) => {
                span!(self, n);
                self.entity_name(&mut n.type_name);
                self.type_parameter_instantiation_opt(n.type_arguments.as_mut());
            }
            TSType::TSTypeLiteral(n) => {
                span!(self, n);
                for m in &mut n.members {
                    self.type_element(m);
                }
            }
            TSType::TSFunctionType(n) => {
                span!(self, n);
                self.type_parameter_declaration_opt(n.type_parameters.as_mut());
                for p in &mut n.params {
                    self.expression(p);
                }
                self.type_annotation(&mut n.return_type);
            }
            TSType::TSConstructorType(n) => {
                span!(self, n);
                self.type_parameter_declaration_opt(n.type_parameters.as_mut());
                for p in &mut n.params {
                    self.expression(p);
                }
                self.type_annotation(&mut n.return_type);
            }
            TSType::TSTupleType(n) => {
                span!(self, n);
                for t in &mut n.element_types {
                    self.ts_type(t);
                }
            }
            TSType::TSParenthesizedType(n) => {
                span!(self, n);
                self.ts_type(&mut n.type_annotation);
            }
            TSType::TSTypePredicate(n) => {
                span!(self, n);
                match &mut n.parameter_name {
                    TSTypePredicateParameterName::Identifier(id) => self.identifier(id),
                    TSTypePredicateParameterName::TSThisType(t) => span!(self, t),
                }
                if let Some(annotation) = &mut n.type_annotation {
                    self.type_annotation(annotation);
                }
            }
            TSType::TSConditionalType(n) => {
                span!(self, n);
                self.ts_type(&mut n.check_type);
                self.ts_type(&mut n.extends_type);
                self.ts_type(&mut n.true_type);
                self.ts_type(&mut n.false_type);
            }
            TSType::TSMappedType(n) => {
                span!(self, n);
                span!(self, n.type_parameter);
                if let Some(constraint) = &mut n.type_parameter.constraint {
                    self.ts_type(constraint);
                }
                if let Some(name_type) = &mut n.name_type {
                    self.ts_type(name_type);
                }
                if let Some(annotation) = &mut n.type_annotation {
                    self.ts_type(annotation);
                }
            }
            TSType::TSTypeOperator(n) => {
                span!(self, n);
                self.ts_type(&mut n.type_annotation);
            }
            TSType::TSImportType(n) => self.import_type(n),
            TSType::TSTypeQuery(n) => {
                span!(self, n);
                match &mut n.expr_name {
                    TSTypeQueryExprName::Identifier(id) => self.identifier(id),
                    TSTypeQueryExprName::QualifiedName(q) => self.qualified_name(q),
                    TSTypeQueryExprName::Import(i) => self.import_type(i),
                }
                self.type_parameter_instantiation_opt(n.type_arguments.as_mut());
            }
            TSType::TSIndexedAccessType(n) => {
                span!(self, n);
                self.ts_type(&mut n.object_type);
                self.ts_type(&mut n.index_type);
            }
            TSType::TSRestType(n) => {
                span!(self, n);
                self.ts_type(&mut n.type_annotation);
            }
            TSType::TSOptionalType(n) => {
                span!(self, n);
                self.ts_type(&mut n.type_annotation);
            }
            TSType::TSNamedTupleMember(n) => {
                span!(self, n);
                self.identifier(&mut n.label);
                self.ts_type(&mut n.element_type);
            }
            TSType::TSInferType(n) => {
                span!(self, n);
                self.type_parameter(&mut n.type_parameter);
            }
        }
    }

    fn import_type(&self, n: &mut TSImportType) {
        span!(self, n);
        span!(self, n.argument);
        if let Some(options) = &mut n.options {
            self.expression(options);
        }
        if let Some(qualifier) = &mut n.qualifier {
            self.entity_name(qualifier);
        }
        self.type_parameter_instantiation_opt(n.type_arguments.as_mut());
    }
}
