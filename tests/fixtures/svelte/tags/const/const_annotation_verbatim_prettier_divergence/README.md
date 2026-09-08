# const_annotation_verbatim_prettier_divergence

A `{@const}` whose binding carries a **type annotation** is printed **verbatim**
by prettier — whatever whitespace the author wrote in the head survives. tsv
normalizes it like any other declarator, so every authoring reaches one form.

```
                    // authored              // prettier            // tsv
{@const a1:T=expr}     {@const a1:T=expr}     {@const a1: T = expr}
```

The canonical form is the same on both sides, so `input.svelte` is a fixed point
for both formatters and the divergence is visible only from a non-canonical
authoring — the two `prettier_variant_*` files, each of which prettier keeps
stable and tsv normalizes to `input`.

## Reason

prettier-plugin-svelte does not print a `{@const}` head itself. It marks the
declarator as embedded JS and hands the **source slice** to prettier's own
printer wrapped as an *expression* (`forceIntoExpression`), then catches
whatever that throws and returns the raw slice instead:

```js
catch (e) { return getText(node, options, true); }
```

Without an annotation the slice is `a4 = expr`, which wraps to `(a4 = expr)` —
an assignment expression, so it parses and is reprinted. With one the slice is
`a1: T = expr`, and `(a1: T = expr)` is not an expression in any of the plugin's
parsers, so the print throws and the **verbatim fallback** takes over. The
freeze is the fallback, not a decision about annotations.

That makes the agreement on the canonical form accidental: the frozen text and
the normalized text simply coincide there. `a4` is the null control — the same
tag one annotation short, which prettier really does reprint, and which is why
neither variant can compact it.

tsv routes the head through its TypeScript printer in every case, the same
uniform normalization it already applies to the `{#each … as}` and
`{#await … then}` binding patterns prettier likewise prints from raw source.

See [conformance_prettier_svelte.md](../../../../../../docs/conformance_prettier_svelte.md) §Svelte: annotated `{@const}` head verbatim.
