<script>
// An inner /* in the comment body does not end it: the comment starts with * and
// carries the @type tag, so it IS a cast and the parens are kept.
const a = /** z /* @type {T} */ (x);

// A single /* opener means the content starts with a space, not *, so this is NOT
// a cast even though @type appears later; the parens are ordinary grouping and get stripped.
const b = /* z /** @type {T} */ (y);
</script>
