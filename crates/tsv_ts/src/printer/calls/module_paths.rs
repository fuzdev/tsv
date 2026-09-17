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

/// These calls keep the module path on the same line as the method.
///
/// Patterns:
/// - `require.resolve(string)` → don't break args, let assignment break
pub(super) fn is_module_path_no_break(
    call: &internal::CallExpression<'_>,
    printer: &Printer<'_>,
) -> bool {
    // Must have exactly 1 argument that is a string literal
    if call.arguments.len() != 1 || !is_string_literal(&call.arguments[0]) {
        return false;
    }

    // Check for `require.resolve()`
    if let internal::ExpressionKind::MemberExpression(member) = &call.callee.kind
        && !member.computed
        && !member.optional
        && let internal::ExpressionKind::Identifier(resolve_id) = &member.property.kind
        && printer.with_ident_name(resolve_id, |s| s == "resolve")
        && let internal::ExpressionKind::Identifier(require_id) = &member.object.kind
        && printer.with_ident_name(require_id, |s| s == "require")
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
    call: &'a internal::CallExpression<'a>,
    printer: &Printer<'_>,
) -> Option<(&'a internal::Expression<'a>, &'a internal::Identifier<'a>)> {
    // Must have exactly 1 argument that is a string literal
    if call.arguments.len() != 1 || !is_string_literal(&call.arguments[0]) {
        return None;
    }

    // Callee must be a member expression (not computed, not optional)
    let internal::ExpressionKind::MemberExpression(member) = &call.callee.kind else {
        return None;
    };
    if member.computed || member.optional {
        return None;
    }

    // Property must be an identifier
    let internal::ExpressionKind::Identifier(method_name) = &member.property.kind else {
        return None;
    };

    let (is_paths, is_resolve) =
        printer.with_ident_name(method_name, |m| (m == "paths", m == "resolve"));

    // Check for `require.resolve.paths()`
    if is_paths {
        // Object should be `require.resolve`
        if let internal::ExpressionKind::MemberExpression(obj_member) = &member.object.kind
            && !obj_member.computed
            && !obj_member.optional
            && let internal::ExpressionKind::Identifier(resolve_id) = &obj_member.property.kind
            && printer.with_ident_name(resolve_id, |s| s == "resolve")
            && let internal::ExpressionKind::Identifier(require_id) = &obj_member.object.kind
            && printer.with_ident_name(require_id, |s| s == "require")
        {
            return Some((member.object, method_name));
        }
    }

    // Check for `import.meta.resolve()`
    if is_resolve && let internal::ExpressionKind::MetaProperty(meta) = &member.object.kind {
        let is_import_meta = printer.with_ident_name(&meta.meta, |m| m == "import")
            && printer.with_ident_name(&meta.property, |p| p == "meta");
        if is_import_meta {
            return Some((member.object, method_name));
        }
    }

    None
}
