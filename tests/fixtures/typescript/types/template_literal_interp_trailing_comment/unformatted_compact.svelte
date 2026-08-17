<script lang="ts">
// A comment between a template-literal type's interpolated type and its `}`. A block on
// the type's line trails it inline, and a pair the author glued onto one line keeps that
// line; a comment on its own line, and a line comment, expand the interpolation. An
// author blank line before an own-line comment is preserved.
type T = `a${B/* c */}`;

type U = `a${B/* c1 *//* c2 */}`;

type V = `a${ B
/* c */}`;

type W = `a${ B // c
}`;

type X = `a${ B

/* c */}`;
</script>
