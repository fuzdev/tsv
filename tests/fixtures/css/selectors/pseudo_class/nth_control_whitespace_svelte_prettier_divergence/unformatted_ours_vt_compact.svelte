<style>
	/* A control character Svelte's `\s` counts as whitespace (VT U+000B, FF U+000C) around an
	   An+B operator normalizes to a single space, like every other selector gap */
	:nth-child(2n+1) {
		color: red;
	}

	/* ...and around the `of` keyword */
	:nth-child(2n + 1of.class1) {
		color: blue;
	}

	/* ...and inside the parens, before the term and after it, where it is dropped */
	:nth-child(2n + 1) {
		color: green;
	}

	/* the same term in a non-nth functional pseudo */
	:is(2n+1) {
		color: teal;
	}
</style>
