<style>
	a {
		/* A no-break space (U+00A0) is not CSS whitespace, so one glued to a function name
		   is ident content: the name keeps it verbatim, and the `(` abutting it still opens
		   a real function whose arguments normalize */
		width:  calc(1.50px);
		color:  var(--x, red);
		/* ...and between the name and its `(`, where the space is the name's last code point */
		height: calc (1.50px);
		color: rgb (1, 2, 3);
		/* A keyword led by one is no keyword, so that run stays opaque */
		color:  red;
	}
</style>
