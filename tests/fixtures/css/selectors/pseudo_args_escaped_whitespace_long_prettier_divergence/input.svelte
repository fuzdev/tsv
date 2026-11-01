<style>
	/* fits at 100: stays inline, the escaped space ending the last argument kept */
	div:not(.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, .bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, .cccc\ ) {
		color: red;
	}
	/* exceeds 100: wraps one-per-line, and the escaped space ending the last argument stays on
	   its line, ahead of the break */
	div:not(
		.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		.ccccc\ 
	) {
		color: blue;
	}
	/* the same with an escaped tab as the whole last argument */
	div:not(
		.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		\	
	) {
		color: green;
	}
</style>
