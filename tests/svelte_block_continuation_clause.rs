// The expected messages are spelled out as LITERALS on purpose — that is the pin. Deriving
// them the way `error_duplicate_clause` does would make this file a mirror of the code it
// grades, and a reworded message would move both sides together. Their `{:then}`-shaped
// braces read as format specs to clippy, so the lint is off for the file rather than the
// literals being reshaped around it.
#![allow(clippy::literal_string_with_formatting_args)]

//! A block continuation may fill its slot once, and the slot's **wording** is the part no
//! fixture can hold.
//!
//! `input_invalid_*` asserts only that both parsers reject, and `tsv_rejects.txt` pins a
//! message but requires a `_svelte_divergence` directory — one where the canonical parser
//! *accepts*. So for the duplicate-clause family, which canonical rejects too, the message
//! is unpinned by construction: the `{#await}` cases are `input_invalid_*` files that stay
//! green no matter what tsv says, and the two `{:else}` fixtures pin their own substring
//! but nothing relates them or covers the `{#each}` and shorthand-head spellings. This
//! file is that pin — a relation between rejections rather than one document's bytes.
//!
//! The distinction it exists to keep is between two different rule violations that land on
//! the same slot: a **repeat** (`Duplicate {:then} clause found`) versus a *different*
//! clause arriving at a slot its predecessor took (`{:else if} cannot follow {:else}`).
//! Collapsing the second into the first names a clause the author wrote exactly once.
//!
//! Verdict parity with canonical is fixture-side:
//! `svelte/blocks/await/{then_catch,then_shorthand,catch_shorthand,then_shorthand_catch}/input_invalid_duplicate_*`
//! for `{#await}` (both parsers reject) and the `_svelte_divergence` trio under
//! `svelte/blocks/{if,each}/` for `{:else}` (canonical accepts and silently drops a branch
//! — see `docs/conformance_svelte.md` §Block Continuation Corrections).

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

/// Every route to a filled `{#await}` slot reports the clause that is repeated — the full
/// form and both shorthand heads, which reach the guard through different arms of the
/// head/continuation split and once answered with `Unclosed {#await} block`.
#[test]
fn a_repeated_await_clause_names_itself() {
    for (source, message) in [
        (
            "{#await p}a{:then v}b{:then w}c{/await}",
            "Duplicate {:then} clause found",
        ),
        (
            "{#await p}a{:catch e}b{:catch f}c{/await}",
            "Duplicate {:catch} clause found",
        ),
        // Separated by the other clause: the guard is the slot, not adjacency.
        (
            "{#await p}a{:then v}b{:catch e}c{:then w}d{/await}",
            "Duplicate {:then} clause found",
        ),
        (
            "{#await p}a{:catch e}b{:then v}c{:catch f}d{/await}",
            "Duplicate {:catch} clause found",
        ),
        // Filled by the HEAD rather than by an earlier continuation.
        (
            "{#await p then v}a{:then w}b{/await}",
            "Duplicate {:then} clause found",
        ),
        (
            "{#await p catch e}a{:catch f}b{/await}",
            "Duplicate {:catch} clause found",
        ),
        (
            "{#await p then v}a{:catch e}b{:catch f}c{/await}",
            "Duplicate {:catch} clause found",
        ),
        (
            "{#await p catch e}a{:then v}b{:then w}c{/await}",
            "Duplicate {:then} clause found",
        ),
    ] {
        assert_rejected_with(source, message);
    }
}

/// A repeated `{:else}` is a duplicate in both block types that have an alternate.
#[test]
fn a_repeated_else_names_itself() {
    for source in [
        "{#if a}1{:else}2{:else}3{/if}",
        "{#each xs as x}1{:else}2{:else}3{/each}",
        // Reached through the nested `IfBlock` an `{:else if}` pushes, so the guard rides
        // the recursion rather than only the outermost block.
        "{#if a}1{:else if b}2{:else}3{:else}4{/if}",
    ] {
        assert_rejected_with(source, "Duplicate {:else} clause found");
    }
}

/// …but an `{:else if}` after an `{:else}` is NOT a repeat: it is the block's first
/// `{:else if}`, and only the alternate it lands on is taken. Naming the pair says which
/// two clauses collide; calling it a duplicate reports a clause written once.
#[test]
fn an_elseif_after_an_else_names_the_pair() {
    for source in [
        "{#if a}1{:else}2{:else if b}3{/if}",
        "{#if a}1{:else if b}2{:else}3{:else if c}4{/if}",
    ] {
        assert_rejected_with(source, "{:else if} cannot follow {:else}");
    }
}

/// The guard is scoped to the `{:else}` family: any other continuation at that position is
/// left to the unclosed-block error, because canonical rejects those too and the verdict
/// already matches. A stray guard here would be an over-rejection wearing a clause name.
#[test]
fn a_foreign_continuation_after_an_else_is_not_a_clause_error() {
    let error = parse_error("{#if a}1{:else}2{:catch e}3{/if}")
        .unwrap_or_else(|| "<parsed successfully>".to_owned());
    assert!(
        !error.contains("clause") && error.contains("Unclosed {#if} block"),
        "expected the unclosed-block error, got: {error}"
    );
}
