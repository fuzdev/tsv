<style>
	/* An+B of S in a non-nth pseudo folds `of ` into the Nth value; S stays a sibling
	   selector (tsv matches Svelte here — only the dedicated :nth-*() path nests S) */
	:is(2n of .x) {
		color: red;
	}

	/* :has relative-list arg */
	:has(2n of .x) {
		color: blue;
	}

	/* the comma splits at the pseudo-arg level, not inside the of clause */
	:not(2n of .x, .y) {
		color: green;
	}

	/* leading-minus An+B, an of descendant compound, in an unknown functional pseudo */
	:foo(-n + 3 of .a .b) {
		color: teal;
	}
</style>
