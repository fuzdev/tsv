//! Transition/animate directive placement rules.

use super::support::*;

#[test]
fn compile_rejects_conflicting_transition_directives() {
    // The oracle's phase-2 placement check (shared/element.js:115-132): a
    // `transition:` claims BOTH intro and outro, `in:` only intro, `out:` only
    // outro; a channel claimed twice is `transition_duplicate`/`transition_conflict`.
    // tsv folds the whole union into one refusal — modifiers are irrelevant.
    assert_unsupported("<div transition:fade transition:fly></div>", "transition");
    assert_unsupported("<div in:fly transition:fade></div>", "transition");
    // Reverse order refuses the same.
    assert_unsupported("<div transition:fade in:fly></div>", "transition");
    assert_unsupported("<div in:a in:b></div>", "transition");
    assert_unsupported("<div out:a out:b></div>", "transition");
    assert_unsupported("<div transition:fade out:fly></div>", "transition");
    // Three directives claiming intro twice.
    assert_unsupported("<div in:a out:b in:c></div>", "transition");
    // A modifier does not change the direction, so it does not rescue the pair.
    assert_unsupported(
        "<div transition:fade|local transition:fly></div>",
        "transition",
    );
}

#[test]
fn compile_allows_legal_transition_directives() {
    // A single transition/in/out, or an in:+out: pair with no `transition:`, claims
    // each channel at most once — legal; the directive drops and the element compiles.
    let _ = compile_js("<div transition:fade></div>");
    let _ = compile_js("<div in:fly></div>");
    let _ = compile_js("<div out:fade></div>");
    let _ = compile_js("<div in:fly out:fade></div>");
}

#[test]
fn compile_rejects_invalid_animate_placement() {
    // `animate:` is legal only on the sole non-trivial child of a keyed `{#each}`
    // (shared/element.js:92-114). Every other placement refuses.
    // Duplicate `animate:` even in the sanctioned spot still refuses.
    assert_unsupported(
        "{#each xs as x (x)}<div animate:a animate:b></div>{/each}",
        "animate",
    );
    // Root (not inside any `{#each}`).
    assert_unsupported("<div animate:flip></div>", "animate");
    // Unkeyed each (missing key).
    assert_unsupported("{#each xs as x}<div animate:flip></div>{/each}", "animate");
    // A non-trivial sibling.
    assert_unsupported(
        "{#each xs as x (x)}<div animate:flip></div><span></span>{/each}",
        "animate",
    );
    // Not an immediate child — wrapped in `{#if}`.
    assert_unsupported(
        "{#each xs as x (x)}{#if x}<div animate:flip></div>{/if}{/each}",
        "animate",
    );
    // `{@html}` sibling counts as a non-trivial child.
    assert_unsupported(
        "{#each xs as x (x)}{@html s}<div animate:flip></div>{/each}",
        "animate",
    );
}

#[test]
fn compile_allows_valid_animate_placement() {
    // The sole non-trivial child of a keyed `{#each}` — comment/`{@const}`/
    // whitespace siblings are tolerated, an index-only key counts as keyed, and a
    // single `transition:` may coexist.
    let _ = compile_js("{#each xs as x (x)}<div animate:flip></div>{/each}");
    let _ = compile_js("{#each xs as x, i (i)}<div animate:flip></div>{/each}");
    let _ = compile_js("{#each xs as x (x)}<div animate:flip transition:fade></div>{/each}");
    let _ = compile_js("{#each xs as x (x)}<!--c-->\n<div animate:flip></div>{/each}");
    let _ = compile_js("{#each xs as x (x)}{@const y = 1}<div animate:flip></div>{/each}");
}
