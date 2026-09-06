<style>
	div {
		/* a quoted paren is string content, not a nesting paren — the list still normalizes */
		background: fn('(', .10);
		border-color: fn(')', .10);
		color: fn('()', .10);
		/* quoted paren inside a nested function */
		background-image: fn(fn('('), .10);
		/* quoted paren inside url() */
		mask-image: url('a(b');
		/* control: a string with no paren normalizes the same way */
		content: fn('x', .10);
	}
</style>
