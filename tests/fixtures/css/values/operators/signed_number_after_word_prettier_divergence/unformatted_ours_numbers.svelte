<style>
	div {
		/* a `+` followed by a digit is part of the number — `+1.5` is one token — so no
		operator stands between the word and it and the authored gap is kept */
		a: aa +1.50;
		b: aa +.50;
		c: f(aa +1.50);
		d: (aa +1.50);
		/* with a number on the left, or with `-` in place of `+`, the same gap stands */
		e: aa -1.50;
		f: 1.50 +2.50;
		g: 1.50 -2.50;
		/* a `+` that is NOT followed by a digit is an operator, and takes a word's gaps */
		h: aa + bb;
		i: aa -bb;
		/* prettier's pad is positional: only the `+` whose left neighbour is a WORD takes it,
		so a second signed number one member over keeps its sign */
		j: aa +1.50 +2.50;
		/* where the author GLUED the sign to the word, both formatters split the run — the
		word boundary is postcss's, and tsv transcribes it — and the glued pair after the
		number stays one token on both */
		k: aa + 1.50;
		l: aa + 1.50+2.50;
	}
</style>
