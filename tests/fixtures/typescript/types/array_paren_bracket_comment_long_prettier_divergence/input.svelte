<script lang="ts">
	// At exactly 100 the comment stays inline in the brackets
	type A = (AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA & BBBBBBBBBBBBBBBBBBBBBBBBBBBB)[/* c */];
	// At 101 the brackets break open - the suffix owns the break, the element stays put
	type B = (AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA & BBBBBBBBBBBBBBBBBBBBBBBBBBBB)[
		/* c */
	];
	// A union element flips at the same boundary
	type C = (AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | BBBBBBBBBBBBBBBBBBBBBBBBBBBB)[/* c */];
	type D = (AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | BBBBBBBBBBBBBBBBBBBBBBBBBBBB)[
		/* c */
	];
</script>
