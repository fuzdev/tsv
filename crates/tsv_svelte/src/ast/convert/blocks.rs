// Svelte block conversions
//
// Converts internal control flow block nodes to public format:
// - IfBlock: {#if test}...{:else}...{/if}
// - EachBlock: {#each items as item}...{/each}
// - AwaitBlock: {#await promise}...{:then}...{:catch}...{/await}
// - KeyBlock: {#key expression}...{/key}
// - SnippetBlock: {#snippet name(params)}...{/snippet}

use crate::ast::{internal, public};
use string_interner::DefaultStringInterner;
use tsv_lang::LocationTracker;
use tsv_ts::ast::convert::convert_expression;

use super::{convert_fragment, convert_pattern_expression};

pub(super) fn convert_if_block<'src>(
    block: &internal::IfBlock<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::IfBlock<'src> {
    let ts_expr = convert_expression(&block.test, source, loc, interner, 0);

    public::IfBlock {
        node_type: "IfBlock",
        elseif: block.elseif,
        start: block.span.start,
        end: block.span.end,
        test: ts_expr.into(),
        consequent: convert_fragment(&block.consequent, source, loc, interner),
        alternate: block
            .alternate
            .as_ref()
            .map(|f| convert_fragment(f, source, loc, interner)),
    }
}

pub(super) fn convert_each_block<'src>(
    block: &internal::EachBlock<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::EachBlock<'src> {
    let expression = convert_expression(&block.expression, source, loc, interner, 0);
    let context = block
        .context
        .as_ref()
        .map(|c| convert_pattern_expression(c, source, loc, interner));
    let key = block
        .key
        .as_ref()
        .map(|k| convert_expression(k, source, loc, interner, 0));

    public::EachBlock {
        node_type: "EachBlock",
        start: block.span.start,
        end: block.span.end,
        expression: expression.into(),
        body: convert_fragment(&block.body, source, loc, interner),
        context,
        index: block.index.map(str::to_string),
        key: key.map(Into::into),
        fallback: block
            .fallback
            .as_ref()
            .map(|f| convert_fragment(f, source, loc, interner)),
    }
}

pub(super) fn convert_await_block<'src>(
    block: &internal::AwaitBlock<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::AwaitBlock<'src> {
    let expression = convert_expression(&block.expression, source, loc, interner, 0);
    // Simple identifier bindings get `character` in loc from Svelte's read_identifier().
    // Destructure patterns go through read_pattern() which produces columns +1.
    let value = block
        .value
        .as_ref()
        .map(|v| convert_pattern_expression(v, source, loc, interner));
    let error = block
        .error
        .as_ref()
        .map(|e| convert_pattern_expression(e, source, loc, interner));

    public::AwaitBlock {
        node_type: "AwaitBlock",
        start: block.span.start,
        end: block.span.end,
        expression: expression.into(),
        value,
        error,
        pending: block
            .pending
            .as_ref()
            .map(|f| convert_fragment(f, source, loc, interner)),
        then_block: block
            .then
            .as_ref()
            .map(|f| convert_fragment(f, source, loc, interner)),
        catch_block: block
            .catch
            .as_ref()
            .map(|f| convert_fragment(f, source, loc, interner)),
    }
}

pub(super) fn convert_key_block<'src>(
    block: &internal::KeyBlock<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::KeyBlock<'src> {
    let expression = convert_expression(&block.expression, source, loc, interner, 0);

    public::KeyBlock {
        node_type: "KeyBlock",
        start: block.span.start,
        end: block.span.end,
        expression: expression.into(),
        fragment: convert_fragment(&block.fragment, source, loc, interner),
    }
}

pub(super) fn convert_snippet_block<'src>(
    block: &internal::SnippetBlock<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::SnippetBlock<'src> {
    // Svelte's read_identifier() adds `character` to loc for the snippet name.
    let mut expression = convert_expression(&block.expression, source, loc, interner, 0);
    expression.inject_loc_character();
    let parameters = block
        .parameters
        .iter()
        .map(|p| convert_expression(p, source, loc, interner, 0).into())
        .collect();

    public::SnippetBlock {
        node_type: "SnippetBlock",
        start: block.span.start,
        end: block.span.end,
        expression: expression.into(),
        parameters,
        body: convert_fragment(&block.body, source, loc, interner),
        type_params: block.type_params_raw.map(str::to_string),
    }
}
