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
//! The file also holds the rule's two **boundaries**, since each is a wording question about
//! the same braces and no fixture can hold one either: the attribute *position*, which is not
//! a sequence at all and where tsv answers in its own words rather than Svelte's, and an
//! attribute value whose quote never closes, where the lexer speaks first and the placement
//! rule never gets to.
//!
//! Verdict parity with canonical is fixture-side:
//! `svelte/elements/textarea_block_tag_placement/input_invalid_*`,
//! `svelte/attributes/value_block_tag_placement/input_invalid_*` and
//! `svelte/attributes/shorthand_block_marker_invalid/input_invalid_*` (both parsers reject).

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

/// A marker **separated** from its `{` is the same placement question, at every route.
///
/// This is the one place tsv is deliberately wider than `read_sequence`, which asks
/// `parser.match('#')` with no `allow_whitespace()` and so hands `{ #if}` to acorn. Both
/// parsers still reject; only the wording differs, and the wording is what the separated
/// spelling had no way to get right — it reached the TypeScript expression parser, which
/// answered about decorators, ran a quoted value onto a regex that never closes, or (for the
/// brand check, the one production where a private name is an operand) simply *parsed*.
///
/// That last one is why the gap cannot be assumed away: the printer normalizes `{ expr}` to
/// `{expr}`, so an accepted `{ #x in y}` was printed as `{#x in y}` and the glued reading
/// then rejected tsv's own output. Reading a byte at a fixed offset from the brace assumes a
/// gap of width zero, and closing that gap is exactly what the printer does.
#[test]
fn a_separated_marker_names_the_same_placement() {
    for (source, message) in [
        // RCDATA content. The brand check is the case that used to PARSE.
        (
            "<textarea>{ #x in y}</textarea>",
            "{#x ...} block cannot be inside <textarea>",
        ),
        (
            "<textarea>{ @debug e}</textarea>",
            "{@debug ...} tag cannot be inside <textarea>",
        ),
        // Unquoted value.
        (
            "<div data-attr={ #x in y}></div>",
            "{#x ...} block cannot be in attribute value",
        ),
        (
            "<div data-attr={ @html expr}></div>",
            "{@html ...} tag cannot be in attribute value",
        ),
        // Quoted value, and the closed-block shape whose `{/if}` used to open an
        // unterminated regex and kill the whole value as `Unterminated string literal`.
        (
            r#"<div data-attr="{ #x in y}"></div>"#,
            "{#x ...} block cannot be in attribute value",
        ),
        (
            r#"<div data-attr="{ #if a}text1{/if}"></div>"#,
            "{#if ...} block cannot be in attribute value",
        ),
        // The two directive arms, in both of their spellings — the quoted one parsed.
        (
            "<div on:click={ #x in y}></div>",
            "{#x ...} block cannot be in attribute value",
        ),
        (
            r#"<div on:click="{ #x in y}"></div>"#,
            "{#x ...} block cannot be in attribute value",
        ),
        (
            "<div style:color={ #x in y}></div>",
            "{#x ...} block cannot be in attribute value",
        ),
        (
            r#"<div style:color="{ #x in y}"></div>"#,
            "{#x ...} block cannot be in attribute value",
        ),
        // Any width, not just one space — `skip_svelte_ws` is Svelte's `allow_whitespace()`.
        (
            "<textarea>{\n\t#x in y}</textarea>",
            "{#x ...} block cannot be inside <textarea>",
        ),
    ] {
        assert_rejected_with(source, message);
    }
}

/// The widening is scoped to **sequences**. In template position a separated marker opens a
/// real block — Svelte's `tag()` does run `allow_whitespace()` after the `{` — so the guard
/// must not reach there, and neither must the quoted-value scan that shares its lookup.
///
/// A comment between the brace and the marker is likewise not whitespace: it leaves the
/// interior an expression on both sides of tsv, and the brand check inside it stays a
/// deferred early error rather than becoming a placement claim.
#[test]
fn template_position_and_a_comment_lead_are_untouched() {
    for source in [
        "{ #each items as item}<p>{item}</p>{/each}",
        "{ #if cond}<p>text1</p>{ :else}<p>text2</p>{ /if}",
        "{ @html expr}",
        "<textarea>{/* c */ #x in y}</textarea>",
        "<div data-attr={/* c */ #x in y}></div>",
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

/// The attribute **position** is not a sequence, and the marker there answers in tsv's own
/// words rather than in Svelte's.
///
/// Svelte reaches every one of these through one reader — `read_attribute` eats the `{`, runs
/// `allow_whitespace()`, then `read_identifier()` — so a marker simply leaves the identifier
/// empty and it reports the same message for all of them: `attribute_empty_shorthand`,
/// "Attribute shorthand cannot be empty". tsv's lexer classifies the marker brace first, so
/// the dispatch names the construct the author actually wrote: a non-attach `{@…}` reaches the
/// attach reader and fails on its keyword, every other marker reaches the shorthand reader and
/// fails on its interior. Both parsers reject either way — verdict parity is fixture-side
/// (`svelte/attributes/shorthand_block_marker_invalid/input_invalid_*`), so what is pinned
/// here is the wording alone, and the divergence is deliberate: Svelte's message names a
/// shorthand the author never wrote.
///
/// The **separated** spelling is the same message by a different route, and needs its own pin
/// for that reason: the attach reader used to read the author's space as the keyword's first
/// byte, and now skips the gap and reads `@html` — one message standing for two questions
/// until `<div { @attach fn}>` (valid, and what prettier emits) proved they were different.
#[test]
fn a_brace_attribute_marker_answers_in_its_own_words() {
    for (source, message) in [
        // The attach reader, on its keyword — every `{@…}` that is not `{@attach}`.
        ("<div {@html x}></div>", "Expected 'attach' keyword"),
        ("<div { @html x}></div>", "Expected 'attach' keyword"),
        ("<div {@debug x}></div>", "Expected 'attach' keyword"),
        ("<div { @debug x}></div>", "Expected 'attach' keyword"),
        // The shorthand reader, on its interior — the open, the continuation and the close.
        (
            "<div {#if x}></div>",
            "Invalid shorthand attribute: '#if x'",
        ),
        (
            "<div { #if x}></div>",
            "Invalid shorthand attribute: '#if x'",
        ),
        (
            "<div {:else}></div>",
            "Invalid shorthand attribute: ':else'",
        ),
        (
            "<div { :else}></div>",
            "Invalid shorthand attribute: ':else'",
        ),
        ("<div {/if}></div>", "Invalid shorthand attribute: '/if'"),
        ("<div { /if}></div>", "Invalid shorthand attribute: '/if'"),
    ] {
        assert_rejected_with(source, message);
    }
}

/// Where the guard stops: an attribute value whose quote never closes dies in the **lexer**,
/// before there is a value for the placement rule to speak about.
///
/// This is the last spelling of the accident the guard removed from the closed case — a
/// quoted value used to run past `{/if}`'s `/` onto a regex that never terminated and come
/// back `Unterminated string literal in template`. With a closing quote every route now names
/// the placement (the cases above); with none, the string genuinely does not close, so tsv
/// reports the enclosing failure and the interior is never judged. Svelte answers from the
/// other end — its `read_sequence` runs to EOF and reports the marker it passed on the way
/// (`block_invalid_placement`), or `js_parse_error` for the separated spelling — so the
/// verdicts agree and only the wording differs
/// (`svelte/attributes/value_block_tag_placement/input_invalid_quoted_block_unterminated.svelte`
/// holds that parity).
///
/// The pin is on the ordering, not on a preference: it is the notice that fires if a later
/// change lets the placement rule reach here. Truncation alone does not — an unquoted value
/// and RCDATA content both still name the placement at EOF.
#[test]
fn an_unterminated_value_dies_in_the_lexer_before_the_placement_rule() {
    for (source, message) in [
        (
            r#"<div data-attr="{#if a}></div>"#,
            "Unterminated string literal in template",
        ),
        (
            r#"<div data-attr="{ #if a}></div>"#,
            "Unterminated string literal in template",
        ),
        (
            r#"<div data-attr="{@html expr}></div>"#,
            "Unterminated string literal in template",
        ),
        // The same truncation without the quote keeps the rule: the unquoted reader and
        // RCDATA content both reach the marker before they reach the end of the input.
        (
            "<div data-attr={#if a}",
            "{#if ...} block cannot be in attribute value",
        ),
        (
            "<textarea>{#if a}",
            "{#if ...} block cannot be inside <textarea>",
        ),
    ] {
        assert_rejected_with(source, message);
    }
}
