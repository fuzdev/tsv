<style>
	/* a value ending in a `)` that closes nothing: the token keeps its separator space and
	the rest of the value normalizes, wherever the value reader runs */
	a {
		b: x );
		c: /* c */ x );
		d: (a: 0.5px) );
	}
	@import 'a.css' screen );
	@import 'a.css' (a: 0.5px) );
</style>
