<script lang="ts">
// A comment between an import type's specifier and its `)`. A block on the specifier's
// line trails it inline, and a pair the author glued onto one line keeps that line; a
// comment on its own line, and a line comment, open the parens. An author blank line
// before an own-line comment is preserved.
type T = import('./a'/* c */);

type U = import('./b'/* c1 *//* c2 */);

type V = import('./c'
/* c */);

type W = import('./d'
// c
);

type X = import('./e'

/* c */);
</script>
