// Module path pattern detection for TypeScript call expressions
//
// Handles special formatting for module-related calls:
// - `require.resolve(string)` - don't break args, let assignment break
// - `require.resolve.paths(string)` - break before `.paths`
// - `import.meta.resolve(string)` - break before `.resolve`
//
// Note: Plain `require(string)` is NOT special-cased - it wraps at print width like
// any other call. This diverges from Prettier which keeps require() on one line
// regardless of length.

use super::super::{Printer, is_string_literal};
use crate::ast::internal;
use tsv_lang::SymbolResolver;

/// Check if this is a `Boolean(...)` call
///
/// Prettier doesn't add continuation indent for binary expressions inside Boolean() calls.
/// This appears to be a specific quirk, treating Boolean() like !!() for type coercion.
pub(super) fn is_boolean_call(call: &internal::CallExpression, printer: &Printer) -> bool {
    if let internal::Expression::Identifier(id) = call.callee.as_ref() {
        return printer.resolve_symbol(id.name) == "Boolean";
    }
    false
}

/// These calls keep the module path on the same line as the method.
///
/// Patterns:
/// - `require.resolve(string)` → don't break args, let assignment break
pub(super) fn is_module_path_no_break(call: &internal::CallExpression, printer: &Printer) -> bool {
    // Must have exactly 1 argument that is a string literal
    if call.arguments.len() != 1 || !is_string_literal(&call.arguments[0]) {
        return false;
    }

    // Check for `require.resolve()`
    if let internal::Expression::MemberExpression(member) = call.callee.as_ref()
        && !member.computed
        && !member.optional
        && let internal::Expression::Identifier(resolve_id) = member.property.as_ref()
        && printer.resolve_symbol(resolve_id.name) == "resolve"
        && let internal::Expression::Identifier(require_id) = member.object.as_ref()
        && printer.resolve_symbol(require_id.name) == "require"
    {
        return true;
    }

    false
}

/// Module path call patterns where prettier breaks at the chain rather than at args.
/// Returns (base_expr, method_name) if this is a module path call that should break at chain.
///
/// Patterns:
/// - `require.resolve.paths(string)` → break before `.paths`
/// - `import.meta.resolve(string)` → break before `.resolve`
pub(super) fn get_module_path_chain_break<'a>(
    call: &'a internal::CallExpression,
    printer: &Printer,
) -> Option<(&'a internal::Expression, &'a internal::Identifier)> {
    // Must have exactly 1 argument that is a string literal
    if call.arguments.len() != 1 || !is_string_literal(&call.arguments[0]) {
        return None;
    }

    // Callee must be a member expression (not computed, not optional)
    let internal::Expression::MemberExpression(member) = call.callee.as_ref() else {
        return None;
    };
    if member.computed || member.optional {
        return None;
    }

    // Property must be an identifier
    let internal::Expression::Identifier(method_name) = member.property.as_ref() else {
        return None;
    };

    let method_str = printer.resolve_symbol(method_name.name);

    // Check for `require.resolve.paths()`
    if method_str == "paths" {
        // Object should be `require.resolve`
        if let internal::Expression::MemberExpression(obj_member) = member.object.as_ref()
            && !obj_member.computed
            && !obj_member.optional
            && let internal::Expression::Identifier(resolve_id) = obj_member.property.as_ref()
            && printer.resolve_symbol(resolve_id.name) == "resolve"
            && let internal::Expression::Identifier(require_id) = obj_member.object.as_ref()
            && printer.resolve_symbol(require_id.name) == "require"
        {
            return Some((&member.object, method_name));
        }
    }

    // Check for `import.meta.resolve()`
    if method_str == "resolve"
        && let internal::Expression::MetaProperty(meta) = member.object.as_ref()
    {
        let meta_name = printer.resolve_symbol(meta.meta.name);
        let prop_name = printer.resolve_symbol(meta.property.name);
        if meta_name == "import" && prop_name == "meta" {
            return Some((&member.object, method_name));
        }
    }

    None
}
