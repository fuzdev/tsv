// Expression-to-string conversion utilities for TypeScript
//
// Provides string serialization for all expression types. Used by:
// - Object inline printing (comment-aware path)
// - Nested expression flattening
// - Property key normalization
//
// This is a "best-effort" serializer that reconstructs minimal string
// representations without comments or complex formatting.

use super::Printer;
use crate::ast::internal::{self, Expression, Literal, LiteralValue};
use tsv_lang::SymbolResolver;
use tsv_lang::printing::StringFormatOptions;

/// Check if a string is a valid JS identifier
///
/// Valid identifiers:
/// - Start with a letter, underscore, or dollar sign
/// - Contain only letters, digits, underscores, or dollar signs
/// - Can be reserved words (prettier outputs them without quotes)
pub(super) fn is_valid_js_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let mut chars = s.chars();

    // First character must be letter, underscore, or dollar sign
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }

    // Rest can include digits
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' && c != '$' {
            return false;
        }
    }

    true
}

/// Escape a string for single-quoted output
///
/// Escapes single quotes and backslashes in the string content.
pub(super) fn escape_single_quote_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\'' => result.push_str("\\'"),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            _ => result.push(c),
        }
    }
    result
}

impl<'a> Printer<'a> {
    /// Convert an expression to a string (for inline object building)
    pub(super) fn expression_to_string(&self, expr: &Expression) -> String {
        match expr {
            Expression::Literal(lit) => match &lit.value {
                LiteralValue::Number(_) => {
                    super::expressions::normalize_number_literal(lit.span.extract(self.source))
                }
                LiteralValue::String { content: _, quote } => {
                    let start = lit.span.start as usize;
                    let end = lit.span.end as usize;
                    let raw_literal = &self.source[start..end];
                    let raw_content = &raw_literal[1..raw_literal.len() - 1];
                    tsv_lang::printing::format_string_literal(
                        raw_content,
                        *quote,
                        StringFormatOptions::default(),
                    )
                }
                LiteralValue::BigInt(_) => {
                    super::expressions::normalize_number_literal(lit.span.extract(self.source))
                }
                LiteralValue::Boolean(b) => (if *b { "true" } else { "false" }).to_string(),
                LiteralValue::Null => "null".to_string(),
                LiteralValue::Undefined => "undefined".to_string(),
            },
            Expression::Identifier(id) => self.resolve_symbol(id.name),
            Expression::PrivateIdentifier(pid) => format!("#{}", self.resolve_symbol(pid.name)),
            Expression::ObjectExpression(obj) => {
                // Recursively build nested object inline
                let mut parts = Vec::new();
                parts.push("{".to_string());

                for (i, prop) in obj.properties.iter().enumerate() {
                    match prop {
                        internal::ObjectProperty::Property(p) => {
                            // For computed keys, use expression_to_string (preserves string quotes)
                            // For regular keys, use property_key_to_string (strips quotes)
                            let base_key = if p.computed {
                                format!("[{}]", self.expression_to_string(&p.key))
                            } else {
                                self.property_key_to_string(&p.key)
                            };
                            // Add getter/setter prefix if applicable
                            let key_str = match p.kind {
                                internal::PropertyKind::Get => format!("get {base_key}"),
                                internal::PropertyKind::Set => format!("set {base_key}"),
                                internal::PropertyKind::Init => base_key,
                            };
                            if matches!(
                                p.kind,
                                internal::PropertyKind::Get | internal::PropertyKind::Set
                            ) {
                                // Getter/setter: `get x() {}` or `set x(v) {}`
                                let value_str = self.expression_to_string(&p.value);
                                parts.push(format!("{key_str}{value_str}"));
                            } else if p.method {
                                // Method shorthand: `foo() {}`
                                let value_str = self.expression_to_string(&p.value);
                                parts.push(format!("{key_str}{value_str}"));
                            } else if p.shorthand {
                                // Handle shorthand with default value: {a = 1}
                                if let Expression::AssignmentExpression(assign) = &p.value {
                                    let default_str = self.expression_to_string(&assign.right);
                                    parts.push(format!("{key_str} = {default_str}"));
                                } else if let Expression::AssignmentPattern(pattern) = &p.value {
                                    let default_str = self.expression_to_string(&pattern.right);
                                    parts.push(format!("{key_str} = {default_str}"));
                                } else {
                                    parts.push(key_str);
                                }
                            } else {
                                let value_str = self.expression_to_string(&p.value);
                                parts.push(format!("{key_str}: {value_str}"));
                            }
                        }
                        internal::ObjectProperty::SpreadElement(s) => {
                            parts.push(format!("...{}", self.expression_to_string(&s.argument)));
                        }
                    }

                    if i < obj.properties.len() - 1 {
                        parts.push(", ".to_string());
                    }
                }

                parts.push("}".to_string());
                parts.concat()
            }
            Expression::ArrayExpression(arr) => {
                // Recursively build nested array inline
                let mut parts = Vec::new();
                parts.push("[".to_string());

                for (i, elem) in arr.elements.iter().enumerate() {
                    if let Some(e) = elem {
                        parts.push(self.expression_to_string(e));
                    }
                    if i < arr.elements.len() - 1 {
                        parts.push(", ".to_string());
                    }
                }

                parts.push("]".to_string());
                parts.concat()
            }
            Expression::UnaryExpression(unary) => {
                format!(
                    "{}{}",
                    unary.operator.as_str(),
                    self.expression_to_string(&unary.argument)
                )
            }
            Expression::UpdateExpression(update) => {
                if update.prefix {
                    format!(
                        "{}{}",
                        update.operator.as_str(),
                        self.expression_to_string(&update.argument)
                    )
                } else {
                    format!(
                        "{}{}",
                        self.expression_to_string(&update.argument),
                        update.operator.as_str()
                    )
                }
            }
            Expression::BinaryExpression(binary) => {
                let left = self.expression_to_string(&binary.left);
                let right = self.expression_to_string(&binary.right);

                // Wrap operands in parens if needed
                let left_str = if let Expression::BinaryExpression(child) = binary.left.as_ref() {
                    if super::operators::needs_parens_for_clarity(child, binary.operator, false) {
                        format!("({left})")
                    } else {
                        left
                    }
                } else {
                    left
                };

                let right_str = if let Expression::BinaryExpression(child) = binary.right.as_ref() {
                    if super::operators::needs_parens_for_clarity(child, binary.operator, true) {
                        format!("({right})")
                    } else {
                        right
                    }
                } else {
                    right
                };

                format!("{} {} {}", left_str, binary.operator.as_str(), right_str)
            }
            Expression::ArrowFunctionExpression(arrow) => self.arrow_function_to_string(arrow),
            Expression::SpreadElement(spread) => {
                format!("...{}", self.expression_to_string(&spread.argument))
            }
            Expression::CallExpression(call) => {
                let callee = self.expression_to_string(&call.callee);
                let args: Vec<String> = call
                    .arguments
                    .iter()
                    .map(|arg| self.expression_to_string(arg))
                    .collect();
                let opt = if call.optional { "?." } else { "" };
                format!("{}{}({})", callee, opt, args.join(", "))
            }
            Expression::MemberExpression(member) => {
                let obj = self.expression_to_string(&member.object);
                let prop = self.expression_to_string(&member.property);
                if member.computed {
                    let opt = if member.optional { "?." } else { "" };
                    format!("{obj}{opt}[{prop}]")
                } else {
                    let dot = if member.optional { "?." } else { "." };
                    format!("{obj}{dot}{prop}")
                }
            }
            Expression::ConditionalExpression(cond) => {
                format!(
                    "{} ? {} : {}",
                    self.expression_to_string(&cond.test),
                    self.expression_to_string(&cond.consequent),
                    self.expression_to_string(&cond.alternate)
                )
            }
            Expression::TemplateLiteral(template) => {
                let mut result = String::from("`");
                for (i, quasi) in template.quasis.iter().enumerate() {
                    result.push_str(&quasi.raw);
                    if i < template.expressions.len() {
                        result.push_str("${");
                        result.push_str(&self.expression_to_string(&template.expressions[i]));
                        result.push('}');
                    }
                }
                result.push('`');
                result
            }
            Expression::TaggedTemplateExpression(tagged) => {
                let tag = self.expression_to_string(&tagged.tag);
                let quasi =
                    self.expression_to_string(&Expression::TemplateLiteral(tagged.quasi.clone()));
                format!("{tag}{quasi}")
            }
            Expression::NewExpression(new_expr) => {
                let callee = self.expression_to_string(&new_expr.callee);
                let type_args = new_expr
                    .type_arguments
                    .as_ref()
                    .map(|ta| {
                        let types: Vec<_> =
                            ta.params.iter().map(|t| self.type_to_string(t)).collect();
                        format!("<{}>", types.join(", "))
                    })
                    .unwrap_or_default();
                let args: Vec<String> = new_expr
                    .arguments
                    .iter()
                    .map(|arg| self.expression_to_string(arg))
                    .collect();
                format!("new {}{}({})", callee, type_args, args.join(", "))
            }
            Expression::FunctionExpression(func) => {
                // Function expressions: () { return ...; }
                // params can be patterns, so extract from source instead of resolving names
                let params: Vec<String> = func
                    .params
                    .iter()
                    .map(|p| self.expression_to_string(p))
                    .collect();
                // For inline string, we just extract body from source
                let body_start = func.body.span.start as usize;
                let body_end = func.body.span.end as usize;
                let body_str = &self.source[body_start..body_end];
                format!("({}) {}", params.join(", "), body_str)
            }
            Expression::ClassExpression(class_expr) => {
                // Class expression: class [Name] [<T>] [extends Base] { ... }
                let class_start = class_expr.span.start as usize;
                let class_end = class_expr.span.end as usize;
                self.source[class_start..class_end].to_string()
            }
            Expression::AwaitExpression(await_expr) => {
                format!("await {}", self.expression_to_string(&await_expr.argument))
            }
            Expression::YieldExpression(yield_expr) => {
                let keyword = if yield_expr.delegate {
                    "yield*"
                } else {
                    "yield"
                };
                match &yield_expr.argument {
                    Some(arg) => format!("{} {}", keyword, self.expression_to_string(arg)),
                    None => keyword.to_string(),
                }
            }
            Expression::SequenceExpression(seq) => {
                let inner = seq
                    .expressions
                    .iter()
                    .map(|e| self.expression_to_string(e))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({inner})")
            }
            Expression::RegexLiteral(regex) => {
                format!("/{}/{}", regex.pattern, regex.flags)
            }
            Expression::Super(_) => "super".to_string(),
            Expression::AssignmentExpression(assign) => {
                format!(
                    "{} {} {}",
                    self.expression_to_string(&assign.left),
                    assign.operator.as_str(),
                    self.expression_to_string(&assign.right)
                )
            }
            Expression::ObjectPattern(obj) => {
                let props: Vec<String> = obj
                    .properties
                    .iter()
                    .map(|p| self.object_pattern_property_to_string(p))
                    .collect();
                format!("{{{}}}", props.join(", "))
            }
            Expression::ArrayPattern(arr) => {
                let elems: Vec<String> = arr
                    .elements
                    .iter()
                    .map(|e| match e {
                        Some(expr) => self.expression_to_string(expr),
                        None => String::new(),
                    })
                    .collect();
                format!("[{}]", elems.join(", "))
            }
            Expression::AssignmentPattern(pattern) => {
                format!(
                    "{} = {}",
                    self.expression_to_string(&pattern.left),
                    self.expression_to_string(&pattern.right)
                )
            }
            Expression::RestElement(rest) => {
                format!("...{}", self.expression_to_string(&rest.argument))
            }
            Expression::TSTypeAssertion(type_assert) => {
                format!(
                    "<{}>{}",
                    self.type_to_string(&type_assert.type_annotation),
                    self.expression_to_string(&type_assert.expression)
                )
            }
            Expression::TSAsExpression(as_expr) => {
                format!(
                    "{} as {}",
                    self.expression_to_string(&as_expr.expression),
                    self.type_to_string(&as_expr.type_annotation)
                )
            }
            Expression::TSSatisfiesExpression(sat_expr) => {
                format!(
                    "{} satisfies {}",
                    self.expression_to_string(&sat_expr.expression),
                    self.type_to_string(&sat_expr.type_annotation)
                )
            }
            Expression::TSInstantiationExpression(inst_expr) => {
                let type_args: Vec<_> = inst_expr
                    .type_arguments
                    .params
                    .iter()
                    .map(|t| self.type_to_string(t))
                    .collect();
                format!(
                    "{}<{}>",
                    self.expression_to_string(&inst_expr.expression),
                    type_args.join(", ")
                )
            }
            Expression::TSNonNullExpression(non_null_expr) => {
                format!("{}!", self.expression_to_string(&non_null_expr.expression))
            }
            Expression::ImportExpression(import_expr) => {
                format!("import({})", self.expression_to_string(&import_expr.source))
            }
            Expression::MetaProperty(meta) => {
                let meta_name = self.resolve_symbol(meta.meta.name);
                let prop_name = self.resolve_symbol(meta.property.name);
                format!("{meta_name}.{prop_name}")
            }
            Expression::TSParameterProperty(param_prop) => {
                // Parameter property: public x, private readonly y, etc.
                let mut parts = Vec::new();
                if let Some(acc) = &param_prop.accessibility {
                    parts.push(acc.as_str().to_string());
                }
                if param_prop.readonly {
                    parts.push("readonly".to_string());
                }
                parts.push(self.expression_to_string(&param_prop.parameter));
                parts.join(" ")
            }
        }
    }

    /// Convert an object pattern property to a string
    fn object_pattern_property_to_string(&self, prop: &internal::ObjectPatternProperty) -> String {
        match prop {
            internal::ObjectPatternProperty::Property(p) => {
                if p.shorthand {
                    // For shorthand with default value, the value is an AssignmentPattern
                    // For shorthand without default, key and value are the same identifier
                    match &p.value {
                        Expression::AssignmentPattern(pattern) => {
                            // {a = 1} - shorthand with default
                            format!(
                                "{} = {}",
                                self.expression_to_string(&p.key),
                                self.expression_to_string(&pattern.right)
                            )
                        }
                        _ => self.expression_to_string(&p.key),
                    }
                } else {
                    // {a: x} or {a: x = 1} or {[key]: x}
                    // For regular keys, use property_key_to_string to normalize string keys to identifiers
                    let key_str = if p.computed {
                        format!("[{}]", self.expression_to_string(&p.key))
                    } else {
                        self.property_key_to_string(&p.key)
                    };
                    format!("{}: {}", key_str, self.expression_to_string(&p.value))
                }
            }
            internal::ObjectPatternProperty::RestElement(r) => {
                format!("...{}", self.expression_to_string(&r.argument))
            }
        }
    }

    /// Convert a property key expression to a string
    ///
    /// String keys that are valid identifiers are output without quotes.
    pub(super) fn property_key_to_string(&self, key: &Expression) -> String {
        match key {
            Expression::Literal(Literal {
                value: LiteralValue::String { content, .. },
                ..
            }) => {
                if is_valid_js_identifier(content) {
                    content.clone()
                } else {
                    format!("'{}'", escape_single_quote_string(content))
                }
            }
            _ => self.expression_to_string(key),
        }
    }

    /// Convert an arrow function to a string (for inline building)
    pub(super) fn arrow_function_to_string(
        &self,
        arrow: &internal::ArrowFunctionExpression,
    ) -> String {
        let mut result = String::new();

        // Async keyword if present
        if arrow.r#async {
            result.push_str("async ");
        }

        // Parameters (can be patterns, so use expression_to_string)
        result.push('(');
        for (i, param) in arrow.params.iter().enumerate() {
            if i > 0 {
                result.push_str(", ");
            }
            result.push_str(&self.expression_to_string(param));
        }
        result.push(')');

        // Return type annotation
        if let Some(return_type) = &arrow.return_type {
            result.push_str(&self.type_annotation_to_string(return_type));
        }

        result.push_str(" => ");

        // Body
        match &arrow.body {
            internal::ArrowFunctionBody::Expression(expr) => {
                result.push_str(&self.expression_to_string(expr));
            }
            internal::ArrowFunctionBody::BlockStatement(block) => {
                // For inline formatting, extract raw block from source
                let raw = block.span.extract(self.source);
                result.push_str(raw);
            }
        }

        result
    }
}
