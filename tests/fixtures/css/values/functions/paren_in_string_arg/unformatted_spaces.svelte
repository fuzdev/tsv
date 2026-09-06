<style>
	div {
		/* a quoted paren is string content, not a nesting paren — the list still normalizes */
		background: fn( '(' ,  0.1 );
		border-color: fn( ')' ,  0.1 );
		color: fn( '()' ,  0.1 );
		/* quoted paren inside a nested function */
		background-image: fn( fn( '(' ) ,  0.1 );
		/* quoted paren inside url() */
		mask-image: url( 'a(b' );
		/* control: a string with no paren normalizes the same way */
		content: fn( 'x' ,  0.1 );
	}
</style>
