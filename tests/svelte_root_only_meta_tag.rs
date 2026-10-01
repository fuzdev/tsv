//! A root-only meta tag may break two rules at once, and which one is reported is the part
//! no fixture can hold.
//!
//! `input_invalid_*` asserts only that both parsers reject, so the cases under
//! `svelte/special_elements/{svelte_window,svelte_body,svelte_document,svelte_head,svelte_options_root_only}/`
//! stay green whatever tsv says about them. Svelte's parser asks the two questions in a
//! fixed order where it reads the tag name (`1-parse/state/element.js`): a second tag in
//! the component is `svelte_meta_duplicate`, and only then is one outside the root
//! `svelte_meta_invalid_placement`. This file pins that order, with Svelte's wording.

fn parse_error(source: &str) -> Option<String> {
    let arena = bumpalo::Bump::new();
    tsv_svelte::parse(source, &arena)
        .err()
        .map(|e| e.to_string())
}

#[track_caller]
fn assert_rejected_with(source: &str, message: &str) {
    let error = parse_error(source).unwrap_or_else(|| "<parsed successfully>".to_owned());
    assert!(
        error.contains(message),
        "expected {message:?} for {source:?}, got: {error}"
    );
}

/// A nested tag that is also a repeat reports the repeat: the duplicate question is asked
/// first, of every root-only tag — `<svelte:options>` included, which has no nested form.
#[test]
fn a_nested_repeat_is_a_duplicate() {
    assert_rejected_with(
        "<svelte:window /><div><svelte:window /></div>",
        "A component can only have one `<svelte:window>` element",
    );
    assert_rejected_with(
        "<svelte:options /><div><svelte:options /></div>",
        "A component can only have one `<svelte:options>` element",
    );
}

/// A nested tag is rejected where it stands, so one that would be repeated further on is
/// a placement error: nothing has been seen yet when the first is read.
#[test]
fn a_nested_first_tag_is_a_placement_error() {
    assert_rejected_with(
        "<div><svelte:window /></div><svelte:window />",
        "`<svelte:window>` tags cannot be inside elements or blocks",
    );
    assert_rejected_with(
        "<div><svelte:options /></div>",
        "`<svelte:options>` tags cannot be inside elements or blocks",
    );
}

/// One of each at the root parses, in any order and with anything between them.
#[test]
fn one_of_each_at_the_root_parses() {
    for source in [
        "<svelte:window /><svelte:body /><svelte:document /><svelte:head></svelte:head><svelte:options />",
        "<svelte:window />\n<div></div>\n<svelte:body />",
    ] {
        assert_eq!(parse_error(source), None, "{source:?}");
    }
}
