<style>
	/* a `)` with no `(` to close is a token between two values: the separator space before
	it stays, and the rest of the value normalizes as any other, wherever the value reader runs */
	a {
		b: x ) y;
		c: x ) 0.5px;
		d: /* c */ x ) 0.5px;
		e: (a: 0.5px) ) y;
	}
	@import 'a.css' screen ) print;
	@import 'a.css' screen ) 0.5px;
	@import 'a.css' screen and (a: 0.5px) ) print, tv;
</style>
