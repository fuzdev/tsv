<style>
	div {
		/* a hex escape takes up to 6 hex digits then ONE whitespace terminator, which
		   belongs to the escape — so this is a single ident `A2px`, not `\41` + `2px`,
		   and a value scanner must not split on that space */
		width: \41 2px;
		/* the terminator is optional: a non-hex byte ends the escape on its own */
		height: \41 b;
	}
</style>
