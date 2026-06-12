// Acorn comment duplication detection and handling
//
// When acorn-typescript re-parses certain type constructs, the onComment callback
// fires twice for comments in the re-parsed region. This module detects those ranges
// and provides utilities to duplicate comments accordingly.

use super::internal;
use tsv_lang::Comment;

/// Collect span ranges where acorn-typescript would re-parse, causing comment duplication.
///
/// In acorn + acorn-typescript, certain type constructs are parsed in a way that causes
/// the `onComment` callback to fire twice for comments in the re-parsed region. This
/// happens for:
/// - Type literal bodies: comments between `{` and the first member
/// - Mapped type bodies: comments between `{` and the type parameter
/// - Function type bodies: all comments within the function type span
/// - Arrow functions with typed params: comments between `(` and the arrow body
///
/// Each range is `(range_start, range_end)` — comments whose span falls entirely
/// within this range should be duplicated in the root-level comments array.
pub fn collect_acorn_type_reparse_ranges(
    program: &internal::Program,
    source: &str,
) -> Vec<(u32, u32)> {
    let mut ranges = Vec::new();
    for stmt in &program.body {
        collect_ranges_from_statement(stmt, &mut ranges);
    }
    let bytes = source.as_bytes();
    // Post-process: resolve mapped type ranges.
    // These were encoded as pairs of MAPPED_TYPE_MARKER entries:
    //   (mapped_start | MARKER, name_end), (constraint_start | MARKER, 0)
    // Find the `in` keyword between name_end and constraint_start, then create
    // a single (mapped_start, in_start) range covering `{` to `in`.
    let mut resolved = Vec::new();
    let mut i = 0;
    let mut keep = vec![true; ranges.len()];
    while i + 1 < ranges.len() {
        if ranges[i].0 & MAPPED_TYPE_MARKER != 0 && ranges[i + 1].0 & MAPPED_TYPE_MARKER != 0 {
            let mapped_start = (ranges[i].0 & !MAPPED_TYPE_MARKER) as usize;
            let name_end = ranges[i].1 as usize;
            let constraint_start = (ranges[i + 1].0 & !MAPPED_TYPE_MARKER) as usize;
            // Find `i` of `in` keyword between name_end and constraint_start
            if let Some(in_start) = tsv_lang::source_scan::find_char_skipping_comments(
                bytes,
                name_end,
                constraint_start,
                b'i',
            ) {
                resolved.push((mapped_start as u32, in_start as u32));
            }
            keep[i] = false;
            keep[i + 1] = false;
            i += 2;
        } else {
            i += 1;
        }
    }
    if !keep.iter().all(|&k| k) {
        let mut j = 0;
        ranges.retain(|_| {
            let k = keep[j];
            j += 1;
            k
        });
    }
    ranges.extend(resolved);
    // Resolve empty-param function-type ranges: strip the marker and shrink the
    // end from the `=>` down to the close paren so only in-paren comments are
    // duplicated (a pre-arrow comment lives after `)` and must not duplicate).
    for range in &mut ranges {
        if range.0 & EMPTY_PAREN_MARKER != 0 {
            let open = (range.0 & !EMPTY_PAREN_MARKER) as usize;
            let close = tsv_lang::source_scan::find_char_skipping_comments(
                bytes,
                open + 1,
                range.1 as usize,
                b')',
            )
            .map_or(range.1, |p| p as u32);
            range.0 = open as u32;
            range.1 = close;
        }
    }
    // Narrow computed method signature ranges: the broad (span.start, span.end) range
    // duplicates comments outside brackets. Find `]` and use it as the range end.
    for range in &mut ranges {
        // Detect method signature ranges by checking for `[` at range start
        if (range.0 as usize) < bytes.len()
            && bytes[range.0 as usize] == b'['
            && let Some(bracket_pos) = tsv_lang::source_scan::find_char_skipping_comments(
                bytes,
                range.0 as usize + 1,
                range.1 as usize,
                b']',
            )
        {
            range.1 = (bracket_pos + 1) as u32;
        }
    }
    ranges
}

/// Marker bit for mapped type name-to-in ranges (encoded in the start field).
/// Uses bit 31 of u32 — valid source positions are well below 2^31.
const MAPPED_TYPE_MARKER: u32 = 1 << 31;

/// Marker bit for empty-param function-type ranges (encoded in the start field).
/// Uses bit 30 — resolved at the top level by finding the close paren so the range
/// covers only the parens, not a trailing pre-arrow comment.
const EMPTY_PAREN_MARKER: u32 = 1 << 30;

/// Build a comment list with acorn-typescript duplicates for type re-parse ranges.
///
/// Comments within reparse ranges are inserted as duplicates just before the first
/// original comment from each range, matching acorn's behavior.
pub fn build_comments_with_duplicates(
    comments: &[Comment],
    reparse_ranges: &[(u32, u32)],
    to_json: impl Fn(&Comment) -> serde_json::Value,
) -> Vec<serde_json::Value> {
    if reparse_ranges.is_empty() {
        return comments.iter().map(&to_json).collect();
    }

    // Determine which comments are duplicated and which range they belong to
    let comment_dup_range: Vec<Option<usize>> = comments
        .iter()
        .map(|comment| {
            reparse_ranges.iter().position(|&(range_start, range_end)| {
                comment.span.start > range_start && comment.span.end <= range_end
            })
        })
        .collect();

    let mut result = Vec::new();
    let mut emitted_range_dups = vec![false; reparse_ranges.len()];

    for (ci, comment) in comments.iter().enumerate() {
        if let Some(ri) = comment_dup_range[ci]
            && !emitted_range_dups[ri]
        {
            emitted_range_dups[ri] = true;
            for (dci, dup_comment) in comments.iter().enumerate() {
                if comment_dup_range[dci] == Some(ri) {
                    result.push(to_json(dup_comment));
                }
            }
        }
        result.push(to_json(comment));
    }

    result
}

/// Check if an expression (used as a function param) has a type annotation.
fn expr_has_type_annotation(expr: &internal::Expression) -> bool {
    use internal::Expression;
    match expr {
        Expression::Identifier(id) => id.type_annotation.is_some(),
        Expression::ObjectPattern(p) => p.type_annotation.is_some(),
        Expression::ArrayPattern(p) => p.type_annotation.is_some(),
        Expression::RestElement(r) => r.type_annotation.is_some(),
        Expression::AssignmentPattern(a) => expr_has_type_annotation(&a.left),
        Expression::TSParameterProperty(_) => true, // always has type context
        _ => false,
    }
}

fn collect_ranges_from_statement(stmt: &internal::Statement, ranges: &mut Vec<(u32, u32)>) {
    use internal::Statement;
    match stmt {
        Statement::TSTypeAliasDeclaration(decl) => {
            collect_ranges_from_type(&decl.type_annotation, ranges);
            if let Some(tp) = &decl.type_parameters {
                collect_ranges_from_type_param_decl(tp, ranges);
            }
        }
        Statement::TSInterfaceDeclaration(decl) => {
            // Interface bodies themselves do NOT cause comment duplication
            // (unlike type literals and mapped types), but individual members may
            let body = &decl.body;
            for member in &body.body {
                collect_ranges_from_type_element(member, ranges);
            }
            if let Some(tp) = &decl.type_parameters {
                collect_ranges_from_type_param_decl(tp, ranges);
            }
            for heritage in &decl.extends {
                if let Some(tp) = &heritage.type_arguments {
                    collect_ranges_from_type_param_inst(tp, ranges);
                }
            }
        }
        Statement::VariableDeclaration(decl) => {
            collect_ranges_from_variable_declaration(decl, ranges);
        }
        Statement::ExpressionStatement(stmt) => {
            collect_ranges_from_expression(&stmt.expression, ranges);
        }
        Statement::ReturnStatement(stmt) => {
            if let Some(arg) = &stmt.argument {
                collect_ranges_from_expression(arg, ranges);
            }
        }
        Statement::ExportNamedDeclaration(decl) => {
            if let Some(d) = &decl.declaration {
                collect_ranges_from_statement(d, ranges);
            }
        }
        Statement::ExportDefaultDeclaration(decl) => {
            use internal::ExportDefaultValue;
            match &decl.declaration {
                ExportDefaultValue::Expression(expr) => {
                    collect_ranges_from_expression(expr, ranges);
                }
                ExportDefaultValue::FunctionDeclaration(f) => {
                    collect_ranges_from_function_like(
                        &f.params,
                        f.return_type.as_ref(),
                        f.type_parameters.as_ref(),
                        ranges,
                    );
                    collect_ranges_from_block_statement(&f.body, ranges);
                }
                ExportDefaultValue::TSDeclareFunction(f) => {
                    collect_ranges_from_function_like(
                        &f.params,
                        f.return_type.as_ref(),
                        f.type_parameters.as_ref(),
                        ranges,
                    );
                }
                ExportDefaultValue::ClassDeclaration(c) => {
                    collect_ranges_from_class_body(&c.body, ranges);
                    if let Some(tp) = &c.type_parameters {
                        collect_ranges_from_type_param_decl(tp, ranges);
                    }
                    if let Some(sc) = &c.super_class {
                        collect_ranges_from_expression(sc, ranges);
                    }
                    if let Some(tp) = &c.super_type_parameters {
                        collect_ranges_from_type_param_inst(tp, ranges);
                    }
                }
            }
        }
        Statement::FunctionDeclaration(f) => {
            collect_ranges_from_function_like(
                &f.params,
                f.return_type.as_ref(),
                f.type_parameters.as_ref(),
                ranges,
            );
            collect_ranges_from_block_statement(&f.body, ranges);
        }
        Statement::ClassDeclaration(c) => {
            collect_ranges_from_class_body(&c.body, ranges);
            if let Some(tp) = &c.type_parameters {
                collect_ranges_from_type_param_decl(tp, ranges);
            }
            if let Some(sc) = &c.super_class {
                collect_ranges_from_expression(sc, ranges);
            }
            if let Some(tp) = &c.super_type_parameters {
                collect_ranges_from_type_param_inst(tp, ranges);
            }
        }
        Statement::BlockStatement(block) => {
            collect_ranges_from_block_statement(block, ranges);
        }
        Statement::IfStatement(stmt) => {
            collect_ranges_from_expression(&stmt.test, ranges);
            collect_ranges_from_statement(&stmt.consequent, ranges);
            if let Some(alt) = &stmt.alternate {
                collect_ranges_from_statement(alt, ranges);
            }
        }
        Statement::ForStatement(stmt) => {
            if let Some(init) = &stmt.init {
                use internal::ForInit;
                match init {
                    ForInit::VariableDeclaration(decl) => {
                        collect_ranges_from_variable_declaration(decl, ranges);
                    }
                    ForInit::Expression(expr) => {
                        collect_ranges_from_expression(expr, ranges);
                    }
                }
            }
            if let Some(test) = &stmt.test {
                collect_ranges_from_expression(test, ranges);
            }
            if let Some(update) = &stmt.update {
                collect_ranges_from_expression(update, ranges);
            }
            collect_ranges_from_statement(&stmt.body, ranges);
        }
        Statement::ForInStatement(stmt) => {
            collect_ranges_from_expression(&stmt.right, ranges);
            collect_ranges_from_statement(&stmt.body, ranges);
        }
        Statement::ForOfStatement(stmt) => {
            collect_ranges_from_expression(&stmt.right, ranges);
            collect_ranges_from_statement(&stmt.body, ranges);
        }
        Statement::WhileStatement(stmt) => {
            collect_ranges_from_expression(&stmt.test, ranges);
            collect_ranges_from_statement(&stmt.body, ranges);
        }
        Statement::DoWhileStatement(stmt) => {
            collect_ranges_from_statement(&stmt.body, ranges);
            collect_ranges_from_expression(&stmt.test, ranges);
        }
        Statement::SwitchStatement(stmt) => {
            collect_ranges_from_expression(&stmt.discriminant, ranges);
            for case in &stmt.cases {
                if let Some(test) = &case.test {
                    collect_ranges_from_expression(test, ranges);
                }
                for s in &case.consequent {
                    collect_ranges_from_statement(s, ranges);
                }
            }
        }
        Statement::TryStatement(stmt) => {
            collect_ranges_from_block_statement(&stmt.block, ranges);
            if let Some(handler) = &stmt.handler {
                collect_ranges_from_block_statement(&handler.body, ranges);
            }
            if let Some(finalizer) = &stmt.finalizer {
                collect_ranges_from_block_statement(finalizer, ranges);
            }
        }
        Statement::ThrowStatement(stmt) => {
            collect_ranges_from_expression(&stmt.argument, ranges);
        }
        Statement::LabeledStatement(stmt) => {
            collect_ranges_from_statement(&stmt.body, ranges);
        }
        Statement::TSDeclareFunction(f) => {
            collect_ranges_from_function_like(
                &f.params,
                f.return_type.as_ref(),
                f.type_parameters.as_ref(),
                ranges,
            );
        }
        Statement::TSEnumDeclaration(_) => {}
        Statement::TSModuleDeclaration(decl) => {
            collect_ranges_from_ts_module_declaration(decl, ranges);
        }
        // Simple statements with no expressions or type constructs
        Statement::BreakStatement(_)
        | Statement::ContinueStatement(_)
        | Statement::EmptyStatement(_)
        | Statement::DebuggerStatement(_)
        | Statement::ImportDeclaration(_)
        | Statement::ExportAllDeclaration(_)
        | Statement::TSExportAssignment(_)
        | Statement::TSImportEqualsDeclaration(_) => {}
    }
}

fn collect_ranges_from_block_statement(
    block: &internal::BlockStatement,
    ranges: &mut Vec<(u32, u32)>,
) {
    for stmt in &block.body {
        collect_ranges_from_statement(stmt, ranges);
    }
}

fn collect_ranges_from_variable_declaration(
    decl: &internal::VariableDeclaration,
    ranges: &mut Vec<(u32, u32)>,
) {
    for declarator in &decl.declarations {
        if let Some(init) = &declarator.init {
            collect_ranges_from_expression(init, ranges);
        }
        collect_ranges_from_expression(&declarator.id, ranges);
    }
}

fn collect_ranges_from_ts_module_declaration(
    decl: &internal::TSModuleDeclaration,
    ranges: &mut Vec<(u32, u32)>,
) {
    use internal::TSModuleDeclarationBody;
    if let Some(body) = &decl.body {
        match body {
            TSModuleDeclarationBody::TSModuleBlock(block) => {
                for s in &block.body {
                    collect_ranges_from_statement(s, ranges);
                }
            }
            TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
                collect_ranges_from_ts_module_declaration(nested, ranges);
            }
        }
    }
}

fn collect_ranges_from_function_like(
    params: &[internal::Expression],
    return_type: Option<&internal::TSTypeAnnotation>,
    type_parameters: Option<&internal::TSTypeParameterDeclaration>,
    ranges: &mut Vec<(u32, u32)>,
) {
    for param in params {
        collect_ranges_from_expression(param, ranges);
    }
    if let Some(rt) = return_type {
        collect_ranges_from_type(&rt.type_annotation, ranges);
    }
    if let Some(tp) = type_parameters {
        collect_ranges_from_type_param_decl(tp, ranges);
    }
}

fn collect_ranges_from_expression(expr: &internal::Expression, ranges: &mut Vec<(u32, u32)>) {
    use internal::Expression;
    match expr {
        Expression::ArrowFunctionExpression(arrow) => {
            // Arrow functions with typed params cause re-parsing in acorn:
            // acorn first parses (x: T) as expression, backtracks on `=>`.
            // During the backtrack, the return type region is re-parsed,
            // duplicating comments between the return type end and `=>`.
            // Only applies when params have type annotations (otherwise
            // acorn recognizes arrow params immediately without backtracking).
            if arrow.params.iter().any(expr_has_type_annotation)
                && let Some(rt) = &arrow.return_type
            {
                let body_start = arrow.body.span().start;
                ranges.push((rt.span.end, body_start));
            }
            // Recurse into params for nested type constructs
            for param in &arrow.params {
                collect_ranges_from_expression(param, ranges);
            }
            if let Some(rt) = &arrow.return_type {
                collect_ranges_from_type(&rt.type_annotation, ranges);
            }
            if let Some(tp) = &arrow.type_parameters {
                collect_ranges_from_type_param_decl(tp, ranges);
            }
            match &arrow.body {
                internal::ArrowFunctionBody::Expression(expr) => {
                    collect_ranges_from_expression(expr, ranges);
                }
                internal::ArrowFunctionBody::BlockStatement(block) => {
                    collect_ranges_from_block_statement(block, ranges);
                }
            }
        }
        Expression::CallExpression(call) => {
            collect_ranges_from_expression(&call.callee, ranges);
            for arg in &call.arguments {
                collect_ranges_from_expression(arg, ranges);
            }
            if let Some(ta) = &call.type_arguments {
                collect_ranges_from_type_param_inst(ta, ranges);
            }
        }
        Expression::NewExpression(new) => {
            collect_ranges_from_expression(&new.callee, ranges);
            for arg in &new.arguments {
                collect_ranges_from_expression(arg, ranges);
            }
            if let Some(ta) = &new.type_arguments {
                collect_ranges_from_type_param_inst(ta, ranges);
            }
        }
        Expression::FunctionExpression(f) => {
            collect_ranges_from_function_like(
                &f.params,
                f.return_type.as_ref(),
                f.type_parameters.as_ref(),
                ranges,
            );
            collect_ranges_from_block_statement(&f.body, ranges);
        }
        Expression::ObjectExpression(obj) => {
            for prop in &obj.properties {
                match prop {
                    internal::ObjectProperty::Property(p) => {
                        collect_ranges_from_expression(&p.key, ranges);
                        collect_ranges_from_expression(&p.value, ranges);
                    }
                    internal::ObjectProperty::SpreadElement(s) => {
                        collect_ranges_from_expression(&s.argument, ranges);
                    }
                }
            }
        }
        Expression::ArrayExpression(arr) => {
            for e in arr.elements.iter().flatten() {
                collect_ranges_from_expression(e, ranges);
            }
        }
        Expression::MemberExpression(m) => {
            collect_ranges_from_expression(&m.object, ranges);
            collect_ranges_from_expression(&m.property, ranges);
        }
        Expression::BinaryExpression(b) => {
            collect_ranges_from_expression(&b.left, ranges);
            collect_ranges_from_expression(&b.right, ranges);
        }
        Expression::AssignmentExpression(a) => {
            collect_ranges_from_expression(&a.left, ranges);
            collect_ranges_from_expression(&a.right, ranges);
        }
        Expression::ConditionalExpression(c) => {
            collect_ranges_from_expression(&c.test, ranges);
            collect_ranges_from_expression(&c.consequent, ranges);
            collect_ranges_from_expression(&c.alternate, ranges);
        }
        Expression::UnaryExpression(u) => {
            collect_ranges_from_expression(&u.argument, ranges);
        }
        Expression::UpdateExpression(u) => {
            collect_ranges_from_expression(&u.argument, ranges);
        }
        Expression::SequenceExpression(s) => {
            for expr in &s.expressions {
                collect_ranges_from_expression(expr, ranges);
            }
        }
        Expression::SpreadElement(s) => {
            collect_ranges_from_expression(&s.argument, ranges);
        }
        Expression::TemplateLiteral(t) => {
            for expr in &t.expressions {
                collect_ranges_from_expression(expr, ranges);
            }
        }
        Expression::TaggedTemplateExpression(t) => {
            collect_ranges_from_expression(&t.tag, ranges);
            if let Some(ta) = &t.type_arguments {
                collect_ranges_from_type_param_inst(ta, ranges);
            }
            // TemplateLiteral quasi contains expressions - recurse into them
            for expr in &t.quasi.expressions {
                collect_ranges_from_expression(expr, ranges);
            }
        }
        Expression::AwaitExpression(a) => {
            collect_ranges_from_expression(&a.argument, ranges);
        }
        Expression::YieldExpression(y) => {
            if let Some(arg) = &y.argument {
                collect_ranges_from_expression(arg, ranges);
            }
        }
        Expression::TSTypeAssertion(t) => {
            // Angle bracket type assertion `<T>expr` causes acorn to backtrack:
            // acorn first tries `<` as less-than operator, then recognizes type assertion.
            // Comments between `<` and the type are duplicated during reparse.
            let angle_start = t.span.start;
            let type_start = t.type_annotation.span().start;
            ranges.push((angle_start, type_start));
            collect_ranges_from_type(&t.type_annotation, ranges);
            collect_ranges_from_expression(&t.expression, ranges);
        }
        Expression::TSAsExpression(t) => {
            collect_ranges_from_expression(&t.expression, ranges);
            collect_ranges_from_type(&t.type_annotation, ranges);
        }
        Expression::TSSatisfiesExpression(t) => {
            collect_ranges_from_expression(&t.expression, ranges);
            collect_ranges_from_type(&t.type_annotation, ranges);
        }
        Expression::TSInstantiationExpression(t) => {
            collect_ranges_from_expression(&t.expression, ranges);
            collect_ranges_from_type_param_inst(&t.type_arguments, ranges);
        }
        Expression::TSNonNullExpression(t) => {
            collect_ranges_from_expression(&t.expression, ranges);
        }
        Expression::ClassExpression(c) => {
            collect_ranges_from_class_body(&c.body, ranges);
            if let Some(tp) = &c.type_parameters {
                collect_ranges_from_type_param_decl(tp, ranges);
            }
            if let Some(sc) = &c.super_class {
                collect_ranges_from_expression(sc, ranges);
            }
            if let Some(tp) = &c.super_type_parameters {
                collect_ranges_from_type_param_inst(tp, ranges);
            }
        }
        Expression::Identifier(id) => {
            if let Some(ta) = &id.type_annotation {
                collect_ranges_from_type(&ta.type_annotation, ranges);
            }
        }
        Expression::ObjectPattern(p) => {
            if let Some(ta) = &p.type_annotation {
                collect_ranges_from_type(&ta.type_annotation, ranges);
            }
            for prop in &p.properties {
                match prop {
                    internal::ObjectPatternProperty::Property(p) => {
                        collect_ranges_from_expression(&p.value, ranges);
                    }
                    internal::ObjectPatternProperty::RestElement(r) => {
                        collect_ranges_from_expression(&r.argument, ranges);
                    }
                }
            }
        }
        Expression::ArrayPattern(p) => {
            if let Some(ta) = &p.type_annotation {
                collect_ranges_from_type(&ta.type_annotation, ranges);
            }
            for e in p.elements.iter().flatten() {
                collect_ranges_from_expression(e, ranges);
            }
        }
        Expression::AssignmentPattern(p) => {
            collect_ranges_from_expression(&p.left, ranges);
            collect_ranges_from_expression(&p.right, ranges);
        }
        Expression::RestElement(r) => {
            collect_ranges_from_expression(&r.argument, ranges);
            if let Some(ta) = &r.type_annotation {
                collect_ranges_from_type(&ta.type_annotation, ranges);
            }
        }
        Expression::TSParameterProperty(p) => {
            collect_ranges_from_expression(&p.parameter, ranges);
        }
        Expression::ImportExpression(i) => {
            collect_ranges_from_expression(&i.source, ranges);
            if let Some(opts) = &i.options {
                collect_ranges_from_expression(opts, ranges);
            }
        }
        // Leaf expressions with no children to recurse into
        Expression::Literal(_)
        | Expression::PrivateIdentifier(_)
        | Expression::RegexLiteral(_)
        | Expression::ThisExpression(_)
        | Expression::Super(_)
        | Expression::MetaProperty(_) => {}
    }
}

fn collect_ranges_from_class_body(body: &internal::ClassBody, ranges: &mut Vec<(u32, u32)>) {
    for member in &body.body {
        use internal::ClassMember;
        match member {
            ClassMember::MethodDefinition(m) => {
                collect_ranges_from_function_like(
                    &m.value.params,
                    m.value.return_type.as_ref(),
                    m.value.type_parameters.as_ref(),
                    ranges,
                );
                collect_ranges_from_block_statement(&m.value.body, ranges);
            }
            ClassMember::PropertyDefinition(p) => {
                if let Some(v) = &p.value {
                    collect_ranges_from_expression(v, ranges);
                }
                if let Some(ta) = &p.type_annotation {
                    collect_ranges_from_type(&ta.type_annotation, ranges);
                }
            }
            ClassMember::StaticBlock(s) => {
                for stmt in &s.body {
                    collect_ranges_from_statement(stmt, ranges);
                }
            }
            ClassMember::IndexSignature(sig) => {
                for param in &sig.parameters {
                    if let Some(ta) = &param.type_annotation {
                        collect_ranges_from_type(&ta.type_annotation, ranges);
                    }
                }
                collect_ranges_from_type(&sig.type_annotation.type_annotation, ranges);
            }
        }
    }
}

fn collect_ranges_from_type(ty: &internal::TSType, ranges: &mut Vec<(u32, u32)>) {
    use internal::TSType;
    match ty {
        TSType::TypeLiteral(lit) => {
            // Type literal body: comments between `{` and first member are duplicated
            let first_member_start = lit.members.first().map_or(lit.span.end, |m| m.span().start);
            ranges.push((lit.span.start, first_member_start));
            // Recurse into members
            for member in &lit.members {
                collect_ranges_from_type_element(member, ranges);
            }
        }
        TSType::Mapped(mapped) => {
            // Mapped type duplication: acorn duplicates all comments from `{` up to (but not
            // including) the `in` keyword. This covers comments between `{[` and the param name
            // AND comments between the param name and `in`. We encode this with
            // MAPPED_TYPE_MARKER for post-processing where the source string is available.
            let name_end =
                mapped.type_parameter.span.start + mapped.type_parameter.name.len() as u32;
            let constraint_start = mapped.type_parameter.constraint.span().start;
            // Encode: (mapped_start | MARKER, name_end), constraint_start as next entry
            ranges.push((mapped.span.start | MAPPED_TYPE_MARKER, name_end));
            ranges.push((constraint_start | MAPPED_TYPE_MARKER, 0));
            // Recurse into the mapped type's parts
            collect_ranges_from_type(&mapped.type_parameter.constraint, ranges);
            if let Some(name_type) = &mapped.name_type {
                collect_ranges_from_type(name_type, ranges);
            }
            if let Some(ta) = &mapped.type_annotation {
                collect_ranges_from_type(ta, ranges);
            }
        }
        TSType::Function(func) => {
            // Function types with untyped params cause duplication when acorn backtracks
            // from parenthesized expression to function type. Only duplicate when there
            // ARE params and NO param has a type annotation (if any param is typed, acorn
            // recognizes the function type immediately without backtracking).
            // Zero params `() =>` never cause backtracking — empty parens are unambiguous.
            let any_param_typed = func.params.iter().any(expr_has_type_annotation);
            if !func.params.is_empty() && !any_param_typed {
                ranges.push((func.span.start, func.span.end));
            } else if func.params.is_empty() {
                // Empty parens containing a comment (`(/* c */) =>`) still trigger
                // acorn's parenthesized-expression backtracking, duplicating the
                // in-paren comment. The range must cover only the parens — a
                // pre-arrow comment (`() /* c */ =>`) is parsed once, not
                // duplicated. The close-paren position needs source, so encode the
                // open-paren start with EMPTY_PAREN_MARKER and resolve it at the
                // top level (bounded by the `=>` at the return annotation's start).
                ranges.push((
                    func.span.start | EMPTY_PAREN_MARKER,
                    func.return_type.span.start,
                ));
            }
            // Recurse into nested types
            for param in &func.params {
                collect_ranges_from_expression(param, ranges);
            }
            collect_ranges_from_type(&func.return_type.type_annotation, ranges);
            if let Some(tp) = &func.type_parameters {
                collect_ranges_from_type_param_decl(tp, ranges);
            }
        }
        TSType::Constructor(ctor) => {
            // Constructor types: recurse but don't add duplication range
            // (constructor types use `new` keyword, no backtracking needed)
            for param in &ctor.params {
                collect_ranges_from_expression(param, ranges);
            }
            collect_ranges_from_type(&ctor.return_type.type_annotation, ranges);
            if let Some(tp) = &ctor.type_parameters {
                collect_ranges_from_type_param_decl(tp, ranges);
            }
        }
        // Recursive descent for types that contain other types
        TSType::Array(t) => collect_ranges_from_type(&t.element_type, ranges),
        TSType::Union(t) => {
            for ty in &t.types {
                collect_ranges_from_type(ty, ranges);
            }
        }
        TSType::Intersection(t) => {
            for ty in &t.types {
                collect_ranges_from_type(ty, ranges);
            }
        }
        TSType::TypeReference(t) => {
            if let Some(tp) = &t.type_arguments {
                collect_ranges_from_type_param_inst(tp, ranges);
            }
        }
        TSType::Tuple(t) => {
            for elem in &t.element_types {
                collect_ranges_from_type(elem, ranges);
            }
        }
        TSType::Parenthesized(t) => {
            collect_ranges_from_type(&t.type_annotation, ranges);
        }
        TSType::Conditional(t) => {
            collect_ranges_from_type(&t.check_type, ranges);
            collect_ranges_from_type(&t.extends_type, ranges);
            collect_ranges_from_type(&t.true_type, ranges);
            collect_ranges_from_type(&t.false_type, ranges);
        }
        TSType::TypeOperator(t) => {
            collect_ranges_from_type(&t.type_annotation, ranges);
        }
        TSType::IndexedAccess(t) => {
            collect_ranges_from_type(&t.object_type, ranges);
            collect_ranges_from_type(&t.index_type, ranges);
        }
        TSType::Rest(t) => {
            collect_ranges_from_type(&t.type_annotation, ranges);
        }
        TSType::Optional(t) => {
            collect_ranges_from_type(&t.type_annotation, ranges);
        }
        TSType::NamedTupleMember(t) => {
            collect_ranges_from_type(&t.element_type, ranges);
        }
        TSType::TypePredicate(t) => {
            if let Some(ta) = &t.type_annotation {
                collect_ranges_from_type(ta, ranges);
            }
        }
        TSType::Import(t) => {
            if let Some(tp) = &t.type_arguments {
                collect_ranges_from_type_param_inst(tp, ranges);
            }
        }
        TSType::TypeQuery(t) => {
            if let Some(tp) = &t.type_arguments {
                collect_ranges_from_type_param_inst(tp, ranges);
            }
        }
        TSType::Infer(t) => {
            if let Some(c) = &t.type_parameter.constraint {
                collect_ranges_from_type(c, ranges);
            }
        }
        TSType::Literal(lit) => {
            // Template literal types contain type expressions that may have reparse ranges
            if let internal::TSLiteralType::TemplateLiteral(template) = lit {
                for ty in &template.types {
                    collect_ranges_from_type(ty, ranges);
                }
            }
        }
        // Leaf types with no children
        TSType::Keyword(_) | TSType::ThisType(_) => {}
    }
}

fn collect_ranges_from_type_element(elem: &internal::TSTypeElement, ranges: &mut Vec<(u32, u32)>) {
    use internal::TSTypeElement;
    match elem {
        TSTypeElement::PropertySignature(p) => {
            if let Some(ta) = &p.type_annotation {
                collect_ranges_from_type(&ta.type_annotation, ranges);
            }
        }
        TSTypeElement::MethodSignature(m) => {
            // Computed plain method signatures cause re-parsing in acorn-typescript:
            // acorn first tries to parse `[expr]` as a computed property, then backtracks
            // when it sees `(` and reparses as a method. This duplicates comments within
            // the computed key brackets. Getters/setters are identified by keyword early,
            // so no backtrack occurs. Non-computed methods don't have bracket comments.
            if m.computed && m.kind == internal::MethodKind::Method {
                ranges.push((m.span.start, m.span.end));
            }
            for param in &m.params {
                collect_ranges_from_expression(param, ranges);
            }
            if let Some(ta) = &m.return_type {
                collect_ranges_from_type(&ta.type_annotation, ranges);
            }
            if let Some(tp) = &m.type_parameters {
                collect_ranges_from_type_param_decl(tp, ranges);
            }
        }
        TSTypeElement::CallSignature(c) => {
            for param in &c.params {
                collect_ranges_from_expression(param, ranges);
            }
            if let Some(ta) = &c.return_type {
                collect_ranges_from_type(&ta.type_annotation, ranges);
            }
            if let Some(tp) = &c.type_parameters {
                collect_ranges_from_type_param_decl(tp, ranges);
            }
        }
        TSTypeElement::ConstructSignature(c) => {
            for param in &c.params {
                collect_ranges_from_expression(param, ranges);
            }
            if let Some(ta) = &c.return_type {
                collect_ranges_from_type(&ta.type_annotation, ranges);
            }
            if let Some(tp) = &c.type_parameters {
                collect_ranges_from_type_param_decl(tp, ranges);
            }
        }
        TSTypeElement::IndexSignature(sig) => {
            // Index signature parameters cause partial reparse: acorn-typescript first tries
            // [expr] as a computed property, then backtracks when it recognizes the index
            // signature pattern. Comments between the param name and the colon are duplicated,
            // but comments between the colon and the type are NOT (acorn recognizes the
            // index signature at the colon and doesn't reparse the type region).
            for param in &sig.parameters {
                if let Some(ta) = &param.type_annotation {
                    // Range covers param name to colon only (not the full type annotation)
                    ranges.push((param.span.start, ta.span.start));
                    collect_ranges_from_type(&ta.type_annotation, ranges);
                }
            }
            collect_ranges_from_type(&sig.type_annotation.type_annotation, ranges);
        }
    }
}

fn collect_ranges_from_type_param_decl(
    tp: &internal::TSTypeParameterDeclaration,
    ranges: &mut Vec<(u32, u32)>,
) {
    for param in &tp.params {
        if let Some(c) = &param.constraint {
            collect_ranges_from_type(c, ranges);
        }
        if let Some(d) = &param.default {
            collect_ranges_from_type(d, ranges);
        }
    }
}

fn collect_ranges_from_type_param_inst(
    tp: &internal::TSTypeParameterInstantiation,
    ranges: &mut Vec<(u32, u32)>,
) {
    for param in &tp.params {
        collect_ranges_from_type(param, ranges);
    }
}
