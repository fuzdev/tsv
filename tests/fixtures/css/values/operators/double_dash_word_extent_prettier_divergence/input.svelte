<style>
	div {
		/* a `--` word with nothing after its two dashes ends at a `/`, and at a `-` that
		signs a number, where every other word takes both as content */
		a: --/2.50;
		b: ---2.50;
		/* the `+` a value's head welds onto that word rides the same extent */
		c: +--/2.50;
		d: +---2.50;
		/* one ident code point after the dashes and the word absorbs both again — the
		agreement cells on both sides of the weld */
		e: --a/1.50;
		f: --a-2.50;
		g: +--a/1.50;
		h: +--a-2.50;
	}
</style>
