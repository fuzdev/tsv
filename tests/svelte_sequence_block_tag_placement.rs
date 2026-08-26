//! A `{#…}` block or `{@…}` tag is invalid wherever Svelte reads a *sequence* — a run of
//! text and `{expr}` chunks — and the **wording** of that rejection is the part no fixture
//! can hold.
//!
//! `input_invalid_*` asserts only that both parsers reject, so it cannot tell the placement
//! rule apart from the accident that used to stand in for it: with the guard removed,
//! `{@debug e}` reaches the TypeScript expression parser and comes back as `Expected 'class'
//! after 'decorator'`, and `{#if c}a{/if}` as `Expected 'in' after 'private name'` — right
//! verdict, wrong question, and an answer that names a language the author never wrote in.
//! (`{#x in y}` is worse still: the ergonomic brand check is the one production where a
//! private name is an operand, so it *parses*.) This file is the pin on the question actually
//! being asked.
//!
//! The other half of the rule is that tsv reaches those sequences by **five** routes where
//! Svelte has one `read_sequence` — RCDATA content, a quoted attribute value, an unquoted
//! one, and the two directive arms that take their `{…}` off the token stream instead. Each
//! case below names its route, because a guard landing on some of them is the bug this
//! rejection exists to close, one spelling over.
//!
//! Verdict parity with canonical is fixture-side:
//! `svelte/elements/textarea_block_tag_placement/input_invalid_*` and
//! `svelte/attributes/value_block_tag_placement/input_invalid_*` (both parsers reject).

// The expected messages are spelled out as LITERALS on purpose — that is the pin. Building
// them the way `check_sequence_placement` does would make this file a mirror of the code it
// grades, and a reworded message would move both sides together. Their `{#if}`-shaped braces
// read as format specs to clippy, so the lint is off for the file rather than the literals
// being reshaped around it.
#![allow(clippy::literal_string_with_formatting_args)]

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

#[track_caller]
fn assert_parses(source: &str) {
    let error = parse_error(source);
    assert!(
        error.is_none(),
        "expected {source:?} to parse, got: {}",
        error.unwrap_or_default()
    );
}

/// `<textarea>` is Svelte's sole RCDATA element, and its content is a sequence — so the
/// block and the tag are named the way Svelte names them, by construct and by place.
#[test]
fn a_block_or_tag_inside_textarea_names_its_placement() {
    for (source, message) in [
        (
            "<textarea>{#if c}a{/if}</textarea>",
            "{#if ...} block cannot be inside <textarea>",
        ),
        // The private-name brand check: an expression the TypeScript parser is happy to
        // accept, so before the guard this was an over-acceptance rather than a bad message.
        (
            "<textarea>{#x in y}</textarea>",
            "{#x ...} block cannot be inside <textarea>",
        ),
        (
            "<textarea>{@html expr}</textarea>",
            "{@html ...} tag cannot be inside <textarea>",
        ),
        (
            "<textarea>{@debug expr}</textarea>",
            "{@debug ...} tag cannot be inside <textarea>",
        ),
    ] {
        assert_rejected_with(source, message);
    }
}

/// Every route to an attribute value reports the same placement. Svelte reaches all five
/// through one `read_sequence`; tsv reaches them through three raw-byte readers and two
/// token-stream directive arms, so the agreement here is a relation between rejections
/// rather than one reader's behavior.
#[test]
fn a_block_or_tag_in_any_attribute_value_names_its_placement() {
    for (source, message) in [
        // Quoted value.
        (
            r#"<div data-attr="{#x in y}"></div>"#,
            "{#x ...} block cannot be in attribute value",
        ),
        (
            r#"<div data-attr="{@html expr}"></div>"#,
            "{@html ...} tag cannot be in attribute value",
        ),
        // A CLOSED block, Svelte's own `logic-block-in-attribute` shape. The `{/if}` is what
        // used to decide this one: the lexer skipped the head as an expression, then read the
        // close's `/` as a regex literal that never terminates, and the value died as
        // `Unterminated string literal in template` — a scan accident wearing the placement
        // rule's place. Nothing about the head changed; the value simply stops being scanned
        // as a sequence once a marker says it is not one.
        (
            r#"<div data-attr="{#if a}text1{/if}"></div>"#,
            "{#if ...} block cannot be in attribute value",
        ),
        // Unquoted value — a separate reader, and the spelling a component prop takes.
        (
            "<div data-attr={#x in y}></div>",
            "{#x ...} block cannot be in attribute value",
        ),
        (
            "<Comp prop={@html expr} />",
            "{@html ...} tag cannot be in attribute value",
        ),
        // A directive's `{…}` value, which arrives on the token stream.
        (
            "<div on:click={#x in y}></div>",
            "{#x ...} block cannot be in attribute value",
        ),
        (
            "<div on:click={@debug expr}></div>",
            "{@debug ...} tag cannot be in attribute value",
        ),
        // A style directive's, whose two value arms are its own.
        (
            "<div style:color={#x in y}></div>",
            "{#x ...} block cannot be in attribute value",
        ),
        (
            r#"<div style:color="{@html expr}"></div>"#,
            "{@html ...} tag cannot be in attribute value",
        ),
    ] {
        assert_rejected_with(source, message);
    }
}

/// The guard reads the byte glued to the `{`, not the lexer's `{#`/`{@` token — which
/// allows whitespace between the two, and Svelte's `parser.match('#')` does not. A separated
/// marker is therefore not a placement question at all on either side: Svelte hands
/// `{ #x in y}` to acorn, which rejects it as JS.
///
/// Both halves are asserted, because each alone is satisfied by the other's answer. A
/// separated `{@…}` still reaches the expression parser and is rejected there — but as an
/// expression, so the placement claim must be absent. A separated `{#x in y}` reaches the
/// same parser and *parses*: the brand check is the one production where a private name is
/// an operand, and what confines it is `AllPrivateIdentifiersValid` — a whole-Script early
/// error about BINDING, not containment, which tsv defers — pinned by
/// `typescript/expressions/private_brand_check_unbound_svelte_divergence`. Should that
/// deferral ever end, this case fails rather than silently turning the half above vacuous.
#[test]
fn a_separated_marker_is_not_a_placement_question() {
    for source in [
        "<div data-attr={ @html expr}></div>",
        "<textarea>{ @debug e}</textarea>",
    ] {
        let error = parse_error(source).unwrap_or_else(|| "<parsed successfully>".to_owned());
        assert!(
            !error.contains("cannot be"),
            "expected {source:?} to reject without a placement claim, got: {error}"
        );
    }
    for source in [
        "<textarea>{ #x in y}</textarea>",
        "<div data-attr={ #x in y}></div>",
    ] {
        assert_parses(source);
    }
}

/// The guard fires on the marker and on nothing else: an ordinary expression still
/// interpolates in both sequence contexts, and so does a regex literal, whose `/` is a block
/// close one position over.
#[test]
fn an_expression_still_interpolates_in_every_sequence() {
    for source in [
        "<textarea>{expr}</textarea>",
        "<textarea>{/a/}</textarea>",
        "<div data-attr={expr}></div>",
        "<div data-attr={/a/}></div>",
        r#"<div data-attr="text1{expr}text2"></div>"#,
        "<div on:click={expr}></div>",
        "<div style:color={expr}></div>",
        // `{@attach}` is an attribute POSITION, not an attribute value — the one `{@…}` that
        // belongs among a tag's attributes, and the guard must not reach it.
        "<div {@attach fn}></div>",
    ] {
        assert_parses(source);
    }
}
