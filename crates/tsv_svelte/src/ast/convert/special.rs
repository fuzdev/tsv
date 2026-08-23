// Svelte `<svelte:options>` + script-`lang` support for the wire-JSON writer.
//
// The writer (`ast/convert/write.rs`) composes these to reproduce
// `<svelte:options>` (scalar props + `customElement`) without materializing a
// typed options struct, and to make the component-global TypeScript decision
// that selects the `<script>` wire schema.
//
// The island comment attaches live in `comment_attachment.rs`.

use crate::ast::internal;
use std::borrow::Cow;

/// A script tag's `lang` attribute value, if it carries one (`<script lang="ts">` → `Some("ts")`,
/// plain `<script>` → `None`).
///
/// The value decides the wire schema: acorn-typescript context (`Some("ts")`) emits
/// `importKind`/`exportKind = "value"` and omits `attributes`; the Svelte context (anything else)
/// omits `importKind`/`exportKind` and always includes `attributes`. But the *choice* is
/// component-global, not per-script — this only feeds [`component_is_typescript`].
///
/// ⚠️ Reads the attribute's **raw** bytes, and is the one `lang` reader that does. The oracle
/// here is Svelte's own `this.ts`, which never builds an AST for the question: it runs a regex
/// over the template text (`1-parse/index.js`, `lang=(["'])?([^"' >]+)`) and compares the
/// captured bytes against `ts`. So `<script lang="&#116;s">` is NOT TypeScript to the wire,
/// where the *formatting* question — `internal::lang_attribute`, the reader behind
/// [`internal::EmbeddedLang::is_frozen`], which asks what language the body is in — decodes
/// and calls the same spelling `ts`. Two questions, two readers, on
/// purpose; `tests/fixtures/svelte/attributes/lang_entity` holds both answers.
fn script_lang<'s>(script: &internal::Script<'_>, source: &'s str) -> Option<&'s str> {
    for attr_node in script.attributes {
        let internal::AttributeNode::Attribute(attr) = attr_node else {
            continue;
        };
        let name = attr.name(source);
        if name == "lang"
            && let Some(values) = &attr.value
            && let Some(internal::AttributeValue::Text(text)) = values.first()
        {
            return Some(text.raw(source));
        }
    }
    None
}

/// Whether the component parses as TypeScript, matching Svelte's parser
/// (`1-parse/index.js`): TS is determined **once for the whole component** from the first
/// `<script>` tag (in source order) that carries a `lang` attribute — `lang="ts"` ⇒ every script
/// (module *and* instance) emits the acorn-typescript wire shape. So a plain `<script>` alongside
/// a `lang="ts"` sibling still emits `importKind`/`exportKind = "value"` and omits `attributes`.
/// A `<script>` with no `lang` attribute doesn't decide; nor does `<style lang=…>`.
pub(super) fn component_is_typescript(root: &internal::Root<'_>, source: &str) -> bool {
    // The two top-level scripts in source order — the first one carrying a `lang` decides.
    let mut scripts = [root.module, root.instance];
    scripts.sort_by_key(|s| s.map_or(u32::MAX, |script| script.span.start));
    scripts
        .into_iter()
        .flatten()
        .find_map(|script| script_lang(script, source))
        .is_some_and(|lang| lang == "ts")
}

/// Find a named attribute's value in `<svelte:options>` attributes.
///
/// `pub(super)` so the wire-JSON writer reproduces `<svelte:options>` extraction
/// (scalar props + `customElement`) without materializing a typed options struct.
pub(super) fn find_option_values<'arena>(
    attrs: &[internal::AttributeNode<'arena>],
    name: &str,
    source: &str,
) -> Option<&'arena [internal::AttributeValue<'arena>]> {
    attrs.iter().find_map(|attr| {
        if let internal::AttributeNode::Attribute(attr) = attr
            && attr.name(source) == name
        {
            attr.value
        } else {
            None
        }
    })
}

/// Extract a plain text value from attribute values.
///
/// `pub(super)` — shared with the wire-JSON writer's fused `<svelte:options>`.
pub(super) fn text_value<'src>(
    values: &[internal::AttributeValue<'_>],
    source: &'src str,
) -> Option<Cow<'src, str>> {
    values.iter().find_map(|v| {
        if let internal::AttributeValue::Text(text) = v {
            Some(text.data(source))
        } else {
            None
        }
    })
}

/// Find a boolean option — shorthand (`name`) or explicit (`name={true/false}`).
///
/// `pub(super)` — shared with the wire-JSON writer's fused `<svelte:options>`.
pub(super) fn bool_option(
    attrs: &[internal::AttributeNode<'_>],
    name: &str,
    source: &str,
) -> Option<bool> {
    attrs.iter().find_map(|attr| {
        if let internal::AttributeNode::Attribute(attr) = attr
            && attr.name(source) == name
        {
            match &attr.value {
                None => Some(true),
                Some(values) => values.iter().find_map(|v| {
                    if let internal::AttributeValue::ExpressionTag(expr) = v
                        && let tsv_ts::ast::internal::Expression::Literal(lit) = &expr.expression
                        && let tsv_ts::ast::internal::LiteralValue::Boolean(b) = lit.value
                    {
                        Some(b)
                    } else {
                        None
                    }
                }),
            }
        } else {
            None
        }
    })
}
