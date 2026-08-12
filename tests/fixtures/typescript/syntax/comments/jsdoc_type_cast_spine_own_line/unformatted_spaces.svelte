<script>
    // An own-line cast comment hangs the value — and the cast need only be the value's
    // LEFTMOST LEAF, where its comment is still the first thing the value prints.
    const a =
        /** @type {A} */
        (expr).prop;

    b =
        /** @type {A} */
        (expr).prop;

    class C {
        a =
            /** @type {A} */
            (expr).prop;
    }

    // The cast IS the value: the shape the leaf cases have to match.
    const c =
        /** @type {A} */
        (expr);

    // Control, same dimension: the same cast glued to its `(` keeps the value on the
    // operator's line, leaf or not.
    const d = /** @type {A} */ (expr).prop;

    // A blank line the author wrote before the comment survives: the element's printed
    // content begins at the comment, not at its own first token.
    const arr = [
        a,

        /** @type {A} */
        (expr).prop
    ];
</script>
