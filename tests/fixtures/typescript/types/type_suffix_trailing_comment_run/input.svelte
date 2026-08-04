<script lang="ts">
	// a comment in an indexed-access index-to-`]` gap escapes the brackets and trails the
	// statement; a run of them stays distinct, one per line
	// (unformatted_inside_suffix carries the authored placement)
	type A1 = T[K]; // c1
	// c2

	// the same gap holding a single comment
	type A2 = T[K]; // c3

	// a redundant paren's trailing gap escapes the same way once the parens are stripped
	type B1 = U[]; // c4
	// c5

	type B2 = U[]; // c6
</script>
