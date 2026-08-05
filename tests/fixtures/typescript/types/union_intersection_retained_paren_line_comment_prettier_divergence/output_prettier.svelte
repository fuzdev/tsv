<script lang="ts">
	// trailing line comment inside a retained parenthesized union member (first member)
	type A1 =
		| (
				| a
				| b // c
		  )
		| c;

	// trailing line comment inside a retained parenthesized union member (middle member)
	type A2 =
		| a
		| (
				| b
				| c // c
		  )
		| d;

	// the same union-fit layout in an INTERSECTION member's retained parens - the
	// comment trails the last inner member and the inner union re-fits inline
	type A3 = a &
		(
			| b
			| c // c
		) &
		d;

	// control: a comment BETWEEN inner members forces the one-member-per-line
	// layout in both formatters - only the trailing position re-collapses
	type A4 = a &
		(
			| b // c
			| c
		) &
		d;
</script>
