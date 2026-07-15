<script lang="ts">
	// first union member is a parenthesized union with a leading line comment inside the
	// parens, kept inside the member - the comment takes its own line inside the parens,
	// above the inline union
	type First =
		| (
				// c
				A | B
			)
		| C;

	// first member is a retained-paren intersection with a trailing object, which
	// supplies its own aligned layout - the comment is kept inside the parens the same
	// way, hugging the `(` above the intersection
	type FirstIntersection =
		| (// c
			A & {
				a: 1;
			})
		| C;

	// a leading line comment inside a LATER member's parens relocates to trail the
	// previous member; only the first member - with no previous member to relocate
	// onto - keeps the comment inside the parens
	type Mid =
		| A // c
		| (B | C)
		| D;
</script>
