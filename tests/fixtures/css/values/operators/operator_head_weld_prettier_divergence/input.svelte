<style>
	div {
		/* an operator with nothing before it glues to the member after it — but not where
		the glue would MERGE the two into one token, which rewrites the author's tokens
		rather than their spacing, so the gap stands */
		a: + 2.5;
		b: - 2.5;
		c: - a;
		d: - -a;
		e: - f(2.5);
		/* the same weld at the other head positions: after another operator, inside a
		function's arguments and after a comma */
		f: 1.5 / - a;
		g: 1.5 * - 2.5;
		h: f(- 2.5);
		i:
			a,
			- 2.5;
		/* the same two merges at every other spelling of what they merge into: §4.3.9's
		ident-start set is wider than the ASCII letters — an escape (whatever it spells),
		a non-ASCII code point, a `_` — and §4.3.10's number can open at a `.`, which the
		`unformatted_ours_numbers` variant authors */
		j: - \61;
		k: - \?;
		l: - \31;
		m: - é;
		n: - _a;
		o: - 0.5;
		p: + 0.5;
		/* the ident-start set is tsv's own lexer's, not css-syntax-3 §4.2's enumerated
		ranges: a code point at U+00A0 or above is ident content, which is the reading
		Svelte's `parseCss` takes too, so a `-` welds onto `×a` and `÷a` exactly as it welds
		onto `é` */
		q: - ×a;
		r: - ÷a;
		/* and after a GLUED pair of head operators, which is the `f` position again: the
		pair itself glues — a `-` merges into nothing — where the member after it keeps
		its gap */
		s: +- 2.5;
	}
</style>
