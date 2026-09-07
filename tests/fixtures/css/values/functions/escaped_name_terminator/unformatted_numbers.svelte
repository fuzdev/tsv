<style>
	a {
		/* a hex escape takes an optional whitespace TERMINATOR that belongs to the escape
		(CSS Syntax 3 §4.3.7), so the ident sequence ends against the `(` and the whole
		thing is one function token — the space is inside the name, not beside it */
		color: c\41 (.10);
		/* the same escape ended by a literal instead needs no terminator, so the space
		here would be a real separator — there is none */
		border-color: c\41 x(.10);
		/* a name that IS the escape */
		outline-color: \41 (.10);
	}
</style>
