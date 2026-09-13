<style>
	div {
		/* a `-` where an OPERAND is expected — the run's head, or straight after another
		operator — is the operator itself wherever the byte after it opens neither an
		ident (§4.3.9) nor a number (§4.3.10) for it. The head rule then glues it to the
		member after it, and every authoring of the run reaches that one form. The cells
		below are a SAMPLE of that rule, not its set: the catalog entry named in the
		README lists every printable-ASCII byte the two readings part on */
		a: + -[a];
		b: + -#a;
		c: + -.a;
		d: + -!a;
		e: + -%a;
		f: + -]a;
		g: + -~a;
		h: + ->a;
		i: + -^a;
		/* the same reading one operator further in, where the `/` already has an operand
		on its left: it glues before the `-` operator where it keeps its gap before a word */
		j: 1.5 / / -[a];
		/* the bound — the bytes that DO open an ident or a number for the `-`, where the
		`-` heads that token and the `+` takes the ordinary member gap */
		k: + -a;
		l: + -1.5;
		m: + -_a;
	}
</style>
