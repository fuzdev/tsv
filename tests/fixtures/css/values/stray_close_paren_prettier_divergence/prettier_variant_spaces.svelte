<style>
	/* a `)` with no `(` to close is a token between two values: the separator space before
	it stays, and the rest of the value normalizes as any other, wherever the value reader runs */
	a {
		b: x  )  y;
		c: x  )  .50PX;
		d: /* c */ x  )  .50PX;
		e: (a: .50PX )  )  y;
	}
	@import 'a.css' screen  )  print;
	@import 'a.css' screen  )  .50PX;
	@import 'a.css' screen and (a: .50PX )  )  print , tv;
</style>
